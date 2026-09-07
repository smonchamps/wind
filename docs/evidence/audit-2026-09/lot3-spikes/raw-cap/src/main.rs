use std::{collections::VecDeque, fs::File, io::{self, Read, Write, Cursor}, path::PathBuf, sync::{Arc, Mutex}, time::{Instant, Duration}};
use serde_json::{json, Value};
use mail_parser::MimeHeaders;
const RAW: usize = 2*1024*1024;

#[derive(Default)]
struct Stats { admitted: usize, max_read: usize, commands: usize, poisoned: bool, reason: String, armed: bool, deadline: Option<Instant>, remaining: usize }
enum Piece { Bytes(Cursor<Vec<u8>>), File(File), Repeat { left: usize } }
struct Server { queue: VecDeque<Piece>, command: Vec<u8>, case: String, fixture: PathBuf, stats: Arc<Mutex<Stats>>, bounded: bool }
impl Server {
 fn bytes(&mut self, bytes: Vec<u8>) { self.queue.push_back(Piece::Bytes(Cursor::new(bytes))); }
 fn literal(&mut self, tag: &str, count: usize) {
   let len = std::fs::metadata(&self.fixture).unwrap().len();
   for i in 1..=count {
    self.bytes(format!("* {i} FETCH (UID {i} BODY[] {{{len}}}\r\n").into_bytes());
    self.queue.push_back(Piece::File(File::open(&self.fixture).unwrap()));
    self.bytes(b")\r\n".to_vec());
   }
   self.bytes(format!("{tag} OK fetch\r\n").into_bytes());
 }
}
impl Read for Server {
 fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
   if out.is_empty() { return Ok(0); }
   let mut stats = self.stats.lock().unwrap();
   if stats.poisoned { return Err(io::Error::other("poisoned")); }
   if self.case == "deadline" && stats.armed {
     // Synthetic transport honours the requested remaining absolute deadline.
     std::thread::sleep(Duration::from_millis(5).min(stats.deadline.unwrap().saturating_duration_since(Instant::now())));
   }
   if stats.armed && self.bounded {
     let reason = if stats.deadline.is_some_and(|d| Instant::now() >= d) { "deadline" } else if stats.remaining == 0 { "quota" } else { "" };
     if !reason.is_empty() { stats.poisoned = true; stats.reason = reason.into(); return Err(io::Error::other(reason)); }
   }
   let limit = if stats.armed && self.bounded { out.len().min(stats.remaining) } else { out.len() };
   let n = loop {
    let Some(piece) = self.queue.front_mut() else { break 0; };
    let n = match piece {
     Piece::Bytes(c) => c.read(&mut out[..limit])?,
     Piece::File(f) => f.read(&mut out[..limit])?,
     Piece::Repeat { left } => { let n = (*left).min(limit); out[..n].fill(b'x'); *left -= n; n }
    };
    if n != 0 { break n; } self.queue.pop_front();
   };
   if stats.armed { stats.admitted += n; stats.max_read = stats.max_read.max(n); if self.bounded { stats.remaining -= n; } }
   Ok(n)
 }
}
impl Write for Server {
 fn write(&mut self, data: &[u8]) -> io::Result<usize> {
  if self.stats.lock().unwrap().poisoned { return Err(io::Error::other("poisoned")); }
  self.command.extend_from_slice(data); Ok(data.len())
 }
 fn flush(&mut self) -> io::Result<()> {
  if self.command.is_empty() { return Ok(()); }
  let command = String::from_utf8(std::mem::take(&mut self.command)).unwrap();
  let tag = command.split_whitespace().next().unwrap();
  self.stats.lock().unwrap().commands += 1;
  if command.contains("LOGIN") || command.contains("NOOP") { self.bytes(format!("{tag} OK done\r\n").into_bytes()); }
  else if command.contains("RFC822.SIZE") {
   let len = std::fs::metadata(&self.fixture).unwrap().len();
   let size = if self.case.contains("missing") { String::new() } else { format!(" RFC822.SIZE {}", if self.case.contains("lying") {1024} else {len}) };
   self.bytes(format!("* 1 FETCH (UID 1{size})\r\n{tag} OK size\r\n").into_bytes());
  } else if self.case == "long-line" { self.queue.push_back(Piece::Repeat { left: 8*1024*1024 }); }
  else { self.literal(tag, if self.case == "aggregate" {3} else {1}); }
  Ok(())
 }
}
fn check_attachments(message: &mail_parser::Message<'_>, corpus: &std::path::Path, result: &mut Vec<Value>) {
 for part in &message.parts {
  if let Some(inner) = part.message() { check_attachments(inner, corpus, result); }
  if let Some(name) = part.attachment_name() { if name == "inner.bin" || name == "outer.bin" {
   let expected = std::fs::read(corpus.join(name)).unwrap();
   result.push(json!({"name":name,"decoded_bytes":part.contents().len(),"equal":part.contents()==expected}));
  }}
 }
}
fn run(case: &str, bounded: bool, corpus: &std::path::Path, raw_cap: usize, convert: bool) -> Value {
 let response_cap=raw_cap+8192;
 let filename = if case.contains("mixed") {format!("mixed-{raw_cap}.eml")} else if case.contains("html") {format!("html-{raw_cap}.eml")} else if case == "nested" {"nested.eml".into()} else if case == "malformed" {"malformed.eml".into()} else {
  let size = if case.contains("abuse64") {64*1024*1024} else if case.contains("cap-minus") {raw_cap-1} else if case.contains("cap-plus") {raw_cap+1} else if case.contains("cap-exact") {raw_cap} else if case.contains("64k") {65536} else if case.contains("1m") || case=="aggregate" || case=="deadline" {1048576} else {8388608}; format!("plain-{size}.eml")
 };
 let stats = Arc::new(Mutex::new(Stats::default()));
 let server = Server { queue:VecDeque::new(),command:vec![],case:case.into(),fixture:corpus.join(filename),stats:stats.clone(),bounded };
 let client = imap::Client::new(server);
 let mut session = client.login("synthetic", "synthetic").map_err(|e|e.0).unwrap();
 let start = Instant::now();
 { let mut s=stats.lock().unwrap(); s.armed=true; s.remaining=response_cap; s.deadline=Some(Instant::now()+Duration::from_secs(60)); }
 let sizes = session.uid_fetch("1", "(UID RFC822.SIZE)").unwrap();
 let announced = sizes.iter().next().and_then(|f| f.size);
 let metadata_bytes=stats.lock().unwrap().admitted;
 drop(sizes);
 let mut returned_max=0; let mut returned_sum=0; let mut parsed=0; let mut attachments=vec![];
 let mut parse_decode_ms=0.; let mut conversion_ms=0.; let mut decoded_attachment_bytes=0; let mut copied_attachment_bytes=0;
 let mut owned_text_bytes=0; let mut owned_html_bytes=0; let mut sanitized_html_bytes=0;
 let mut status;
 if bounded && announced.is_some_and(|s| s as usize>raw_cap) && case != "long-line" { status="preflight-refused".to_string(); }
 else {
  { let mut s=stats.lock().unwrap(); s.remaining=response_cap; s.admitted=0; s.max_read=0; s.deadline=Some(Instant::now()+if case=="deadline" {Duration::from_millis(20)} else {Duration::from_secs(60)}); }
  match session.uid_fetch(if case=="aggregate" {"1:3"} else {"1"}, "(UID BODY.PEEK[])") {
   Ok(fetches) => {
    status="complete".into();
    for f in fetches.iter() { if let Some(raw)=f.body() { returned_max=returned_max.max(raw.len()); returned_sum+=raw.len();
     if bounded && (raw.len()>raw_cap || returned_sum>raw_cap) { status="raw-ceiling-refused".into(); break; }
     let phase=Instant::now();
     let message=mail_parser::MessageParser::default().parse(raw);
     parse_decode_ms+=phase.elapsed().as_secs_f64()*1000.;
     if let Some(message)=message {
      parsed+=1; check_attachments(&message,corpus,&mut attachments);
      decoded_attachment_bytes+=message.attachments().map(|a|a.contents().len()).sum::<usize>();
      if case.contains("mixed") {
       for part in message.attachments() { let bytes=part.contents();
        let equal=bytes.iter().enumerate().all(|(i,b)|*b==(i%256) as u8);
        attachments.push(json!({"name":part.attachment_name(),"decoded_bytes":bytes.len(),"equal":equal}));
       }
      }
      if convert {
       let phase=Instant::now();
       // Public equivalents of owned body/draft forms plus current production sanitizer.
       // Keep the forms live together to measure this explicit overlap; not a whole-app model.
       let html=message.body_html(0).map(|s|s.into_owned()).unwrap_or_default();
       let text=message.body_text(0).map(|s|s.into_owned()).unwrap_or_default();
       let copies:Vec<Vec<u8>>=message.attachments().map(|a|a.contents().to_vec()).collect();
       let sanitized=mail_render::sanitize(&html);
       owned_html_bytes+=html.len(); owned_text_bytes+=text.len(); sanitized_html_bytes+=sanitized.html.len();
       copied_attachment_bytes+=copies.iter().map(Vec::len).sum::<usize>();
       std::hint::black_box((&html,&text,&copies,&sanitized));
       conversion_ms+=phase.elapsed().as_secs_f64()*1000.;
      }
     }
    }}
   },
   Err(e) => { status=format!("error: {e}"); }
  }
 }
 let elapsed_ms=start.elapsed().as_secs_f64()*1000.;
 let (admitted,max_read,poisoned,reason,commands)={let s=stats.lock().unwrap(); (s.admitted,s.max_read,s.poisoned,s.reason.clone(),s.commands)};
 let reuse_attempt=session.noop().is_ok();
 drop(session);
 json!({"case":case,"bounded":bounded,"raw_cap":raw_cap,"response_cap":response_cap,"conversion_enabled":convert,"parse_decode_ms":parse_decode_ms,"conversion_ms":conversion_ms,"decoded_attachment_bytes":decoded_attachment_bytes,"copied_attachment_bytes":copied_attachment_bytes,"owned_text_bytes":owned_text_bytes,"owned_html_bytes":owned_html_bytes,"sanitized_html_bytes":sanitized_html_bytes,"announced":announced,"status":status,"metadata_bytes":metadata_bytes,"body_command_admitted_bytes":if status=="preflight-refused" {0}else{admitted},"max_transport_read":max_read,"returned_body_max":returned_max,"returned_body_sum":returned_sum,"parsed_messages":parsed,"attachments":attachments,"elapsed_ms":elapsed_ms,"commands_including_login":commands,"poisoned":poisoned,"poison_reason":reason,"noop_reuse_succeeded":reuse_attempt})
}
fn main() {
 let args:Vec<_>=std::env::args().collect();
 let corpus=PathBuf::from(&args[3]);
 let raw_cap=args.get(4).map(|s|s.parse().unwrap()).unwrap_or(RAW);
 let convert=args.get(5).is_some_and(|s|s=="convert");
 let mut result=run(&args[1],args[2]=="bounded",&corpus,raw_cap,convert);
 // Demonstrates queue progress on a replacement session after resource refusal.
 let next=run("honest-64k",true,&corpus,raw_cap,false);
 result["unrelated_after_replacement_complete"]=json!(next["status"]=="complete");
 result["replacement_commands"]=next["commands_including_login"].clone();
 println!("{result}");
 // Let external OS sampler read peak working set and sampled private commit.
 std::thread::sleep(Duration::from_millis(400));
}
