// THROWAWAY: synthetic peer only, no production adapter changes.
use base64::Engine;
use imap_proto::types::{BodyStructure, SectionPath};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Write},
    net::TcpStream,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const CAP: usize = 2 * 1024 * 1024 + 8192;
const CHUNK: usize = 65536;
#[derive(Default)]
struct Meter {
    used: usize,
    total: usize,
    max_command: usize,
    max_read: usize,
    poisoned: bool,
    commands: usize,
}
struct Guard {
    socket: TcpStream,
    m: Arc<Mutex<Meter>>,
}
impl Read for Guard {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let mut m = self.m.lock().unwrap();
        if m.poisoned {
            return Err(io::Error::other("poisoned"));
        }
        if m.used == CAP {
            m.poisoned = true;
            return Err(io::Error::other("response quota"));
        }
        let nmax = b.len().min(CAP - m.used);
        let n = self.socket.read(&mut b[..nmax])?;
        m.used += n;
        m.total += n;
        m.max_command = m.max_command.max(m.used);
        m.max_read = m.max_read.max(n);
        Ok(n)
    }
}
impl Write for Guard {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.m.lock().unwrap().poisoned {
            return Err(io::Error::other("poisoned write"));
        }
        self.socket.write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}
fn arm(m: &Arc<Mutex<Meter>>) {
    let mut m = m.lock().unwrap();
    assert!(!m.poisoned);
    m.used = 0;
    m.commands += 1;
}
#[repr(C)]
#[derive(Default)]
struct Memory {
    cb: u32,
    faults: u32,
    peak_ws: usize,
    ws: usize,
    peak_paged: usize,
    paged: usize,
    peak_nonpaged: usize,
    nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}
#[link(name = "psapi")]
unsafe extern "system" {
    fn GetProcessMemoryInfo(h: *mut std::ffi::c_void, p: *mut Memory, n: u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}
fn memory() -> Value {
    let mut p = Memory::default();
    p.cb = std::mem::size_of::<Memory>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut p,
            std::mem::size_of::<Memory>() as u32,
        )
    };
    json!({"available":ok!=0,"peak_working_set":p.peak_ws,"peak_pagefile":p.peak_pagefile,"private_bytes":p.private})
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn oracle(raw: &[u8]) -> Value {
    use mail_parser::{MimeHeaders, PartType};
    fn collect(msg: &mail_parser::Message<'_>, prefix: &str, out: &mut Vec<Value>) {
        for (i, p) in msg.attachments().enumerate() {
            let id = format!("{prefix}{i}");
            out.push(json!({"local_index":id,"name":p.attachment_name(),"bytes":p.contents().len(),"sha256":hash(p.contents())}));
            if let PartType::Message(inner) = &p.body {
                collect(inner, &format!("{id}/"), out);
            }
        }
    }
    let mut out = vec![];
    if let Some(m) = mail_parser::MessageParser::new().parse(raw) {
        collect(&m, "", &mut out);
        json!({"parsed":true,"attachments":out,"parts":m.parts.len()})
    } else {
        json!({"parsed":false})
    }
}
fn walk(bs: &BodyStructure<'_>, path: &str, out: &mut Vec<Value>) {
    match bs {
 BodyStructure::Multipart{bodies,..}=>for(i,b)in bodies.iter().enumerate(){let p=if path.is_empty(){(i+1).to_string()}else{format!("{path}.{}",i+1)};walk(b,&p,out)},
 BodyStructure::Message{body,other,common,..}=>{out.push(json!({"path":path,"encoding":format!("{:?}",other.transfer_encoding),"type":format!("{:?}",common.ty),"disposition":format!("{:?}",common.disposition),"encoded_size":other.octets}));walk(body,path,out)},
 BodyStructure::Basic{other,common,..}|BodyStructure::Text{other,common,..}=>out.push(json!({"path":if path.is_empty(){"1"}else{path},"encoding":format!("{:?}",other.transfer_encoding),"type":format!("{:?}",common.ty),"disposition":format!("{:?}",common.disposition),"encoded_size":other.octets}))
 }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a[1] == "oracle" {
        let raw = std::fs::read(&a[2]).unwrap();
        println!("{}", oracle(&raw));
        return;
    }
    let baseline = memory();
    let started = Instant::now();
    let socket = TcpStream::connect(&a[1]).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    socket.set_nodelay(true).unwrap();
    let meter = Arc::new(Mutex::new(Meter::default()));
    let mut client = imap::Client::new(Guard {
        socket,
        m: meter.clone(),
    });
    client.read_greeting().unwrap();
    let mut session = client
        .login("synthetic", "synthetic")
        .map_err(|x| x.0)
        .unwrap();
    let part = a.get(2).filter(|p| p.as_str() != "raw").cloned();
    let mut structure = vec![];
    if part.is_some() {
        arm(&meter);
        let f = session.uid_fetch("1", "(UID BODYSTRUCTURE)").unwrap();
        walk(
            f.iter().next().unwrap().bodystructure().unwrap(),
            "",
            &mut structure,
        );
    }
    let selected_encoding = part.as_ref().map(|p| {
        structure
            .iter()
            .find(|x| x["path"].as_str() == Some(p))
            .unwrap()["encoding"]
            .as_str()
            .unwrap()
            .to_owned()
    });
    assert!(
        selected_encoding
            .as_deref()
            .is_none_or(|e| e == "Base64" || e == "SevenBit")
    );
    // RFC822.SIZE intentionally observed but never trusted as memory admission or EOF.
    arm(&meter);
    let sizes = session.uid_fetch("1", "(UID RFC822.SIZE)").unwrap();
    let claimed = sizes.iter().next().and_then(|f| f.size);
    drop(sizes);
    let mut raw = Vec::new();
    let mut sha = Sha256::new();
    let mut decoded_sha = Sha256::new();
    let mut decoded_n = 0;
    let mut carry = Vec::new();
    let mut max_decode = 0;
    let mut max_decode_capacity = 0;
    let mut max_decoded_capacity = 0;
    let mut max_fetch = 0;
    let mut offset = 0;
    let mut status = "ok".to_string();
    let mut reused_rejected = false;
    let mut file = std::fs::File::create(&a[3]).unwrap();
    loop {
        arm(&meter);
        let query = format!(
            "(UID BODY.PEEK[{}]<{offset}.{CHUNK}>)",
            part.as_deref().unwrap_or("")
        );
        let fetched = match session.uid_fetch("1", query) {
            Ok(x) => x,
            Err(e) => {
                status = e.to_string();
                meter.lock().unwrap().poisoned = true;
                reused_rejected = session.noop().is_err();
                break;
            }
        };
        let f = fetched.iter().find(|f| f.uid == Some(1)).unwrap();
        let path = part
            .as_ref()
            .map(|p| SectionPath::Part(p.split('.').map(|x| x.parse().unwrap()).collect(), None));
        let body = match &path {
            Some(p) => f.section(p),
            None => f.body(),
        }
        .unwrap();
        max_fetch = max_fetch.max(body.len());
        if body.len() > CHUNK {
            status = "oversized partial literal".to_string();
            meter.lock().unwrap().poisoned = true;
            drop(fetched);
            reused_rejected = session.noop().is_err();
            break;
        }
        sha.update(body);
        if selected_encoding.as_deref() == Some("Base64") {
            carry.extend(body.iter().copied().filter(|b| !b.is_ascii_whitespace()));
            let n = carry.len() / 4 * 4;
            max_decode = max_decode.max(carry.len());
            max_decode_capacity = max_decode_capacity.max(carry.capacity());
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(&carry[..n])
                .unwrap();
            max_decoded_capacity = max_decoded_capacity.max(decoded.capacity());
            file.write_all(&decoded).unwrap();
            decoded_sha.update(&decoded);
            decoded_n += decoded.len();
            carry.drain(..n);
        } else if part.is_some() {
            file.write_all(body).unwrap();
            decoded_sha.update(body);
            decoded_n += body.len();
        } else {
            raw.extend_from_slice(body);
        }
        offset += body.len();
        if body.len() < CHUNK {
            break;
        }
    }
    let parse = if part.is_none() && status == "ok" {
        oracle(&raw)
    } else {
        Value::Null
    };
    let measured = memory();
    let m = meter.lock().unwrap();
    println!(
        "{}",
        json!({"status":status,"claimed_size":claimed,"part_path":part,"selected_encoding":selected_encoding,"bodystructure":structure,"commands":m.commands,"actual_admitted_bytes":m.total,"max_response_admitted":m.max_command,"max_transport_read":m.max_read,"max_returned_literal":max_fetch,"raw_len":raw.len(),"raw_capacity":raw.capacity(),"max_decode_input":max_decode,"max_decode_capacity":max_decode_capacity,"max_decoded_block_capacity":max_decoded_capacity,"encoded_bytes":offset,"encoded_sha256":format!("{:x}",sha.finalize()),"decoded_bytes":decoded_n,"decoded_sha256":format!("{:x}",decoded_sha.finalize()),"parser":parse,"poisoned":m.poisoned,"reuse_rejected":reused_rejected,"baseline":baseline,"process_memory":measured,"elapsed_ms":started.elapsed().as_secs_f64()*1000.0,"response_cap":CAP,"chunk":CHUNK})
    );
}
