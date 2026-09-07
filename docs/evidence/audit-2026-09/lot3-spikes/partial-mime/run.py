"""Standalone synthetic loopback protocol; every measured client is its own process.
Run from this directory: python run.py. Corpus generator copied verbatim from raw-cap.
"""
from pathlib import Path
import socket, threading, subprocess, json, re, hashlib, email.parser, email.policy, base64
ROOT=Path(__file__).resolve().parent
EXE=ROOT/'target/release/lot3-partial-mime-spike.exe'
CORPUS=ROOT/'corpus'
OUT=ROOT/'results'; OUT.mkdir(exist_ok=True)

def quote(s): return '"'+str(s).replace('\\','\\\\').replace('"','\\"')+'"'
def model(raw):
    msg=email.parser.BytesParser(policy=email.policy.compat32).parsebytes(raw)
    parts={}
    def tree(p,path=''):
        typ=p.get_content_maintype().upper(); sub=p.get_content_subtype().upper()
        encoding=p.get('Content-Transfer-Encoding','7BIT').upper()
        name=p.get_filename(); disp=('('+quote(p.get_content_disposition().upper())+' '+ ('("FILENAME" '+quote(name)+')' if name else 'NIL')+')') if p.get_content_disposition() else 'NIL'
        if typ=='MESSAGE' and sub=='RFC822':
            inner=p.get_payload(0)
            # This fixture's message/rfc822 section MUST retain original bytes;
            # email.as_bytes() canonicalizes it and would confound equality.
            start=raw.index(b'From: nested@example.invalid\r\n')
            innerraw=raw[start:raw.index(b'\r\n--outer',start)]
            parts[path]=innerraw
            return f'("MESSAGE" "RFC822" NIL NIL NIL "{encoding}" {len(innerraw)} (NIL NIL NIL NIL NIL NIL NIL NIL NIL NIL) {tree(inner,path)} 1 NIL {disp})'
        if p.is_multipart():
            children=[]
            for i,c in enumerate(p.get_payload(),1): children.append(tree(c,(path+'.' if path else '')+str(i)))
            return '('+''.join(children)+' '+quote(sub)+' NIL '+disp+')'
        payload=p.get_payload().encode('ascii'); parts[path or '1']=payload
        core=f'({quote(typ)} {quote(sub)} NIL NIL NIL {quote(encoding)} {len(payload)}'
        if typ=='TEXT': core+=' '+str(payload.count(b'\n')+1)
        return core+' NIL '+disp+')'
    return tree(msg),parts

def run(name,filename,claim='honest',fault=None,part='raw'):
    raw=(CORPUS/filename).read_bytes(); bs,parts=model(raw) if filename!='malformed.eml' else ('',{})
    listener=socket.socket();listener.bind(('127.0.0.1',0));listener.listen();port=listener.getsockname()[1];commands=[]
    def serve():
        try:
            conn,_=listener.accept();conn.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
            with conn:
                conn.sendall(b'* OK synthetic only\r\n'); f=conn.makefile('rb')
                while line:=f.readline():
                    tag,cmd=line.decode().strip().split(' ',1);commands.append(cmd)
                    if 'BODYSTRUCTURE' in cmd: reply=f'* 1 FETCH (UID 1 BODYSTRUCTURE {bs})\r\n'.encode()
                    elif 'RFC822.SIZE' in cmd:
                        size='' if claim=='absent' else ' RFC822.SIZE '+str(len(raw) if claim=='honest' else 1024)
                        reply=f'* 1 FETCH (UID 1{size})\r\n'.encode()
                    elif 'BODY.PEEK[' in cmd:
                        path,offset,count=re.search(r'BODY.PEEK\[(.*?)\]<(\d+)\.(\d+)>',cmd).groups();offset=int(offset);count=int(count)
                        data=raw if not path else parts[path];body=data[offset:offset+count]
                        if fault=='wrong-offset':body=data[:count] if offset<len(data) else b'';offset=0
                        if fault=='oversized': body=b'x'*(8*1024*1024)
                        if fault=='overpartial': body=b'x'*70000
                        if fault=='no-newline': conn.sendall(b'* '+b'x'*(8*1024*1024));continue
                        reply=f'* 1 FETCH (UID 1 BODY[{path}]<{offset}> {{{len(body)}}}\r\n'.encode()+body+b')\r\n'
                        if fault=='aggregate':
                            body=b'x'*1048576
                            reply=b''.join(f'* {i} FETCH (UID {i} BODY[]<0> {{{len(body)}}}\r\n'.encode()+body+b')\r\n' for i in (1,2,3))
                    else: reply=b''
                    conn.sendall(reply+f'{tag} OK synthetic complete\r\n'.encode())
        except (BrokenPipeError,ConnectionResetError,ConnectionAbortedError): pass
        finally: listener.close()
    thread=threading.Thread(target=serve,daemon=True);thread.start()
    result=subprocess.run([str(EXE),f'127.0.0.1:{port}',part,str(OUT/(name+'.bin'))],capture_output=True,text=True,timeout=30)
    thread.join(2)
    if result.returncode: raise RuntimeError(name+': '+result.stderr)
    r=json.loads(result.stdout);r.update(case=name,fixture=filename,claim=claim,fault=fault,protocol_commands=commands,raw_fixture_sha256=hashlib.sha256(raw).hexdigest())
    if part=='raw' and r['status']=='ok':
        r['raw_equality']=r['encoded_sha256']==r['raw_fixture_sha256']
        oracle=subprocess.run([str(EXE),'oracle',str(CORPUS/filename)],capture_output=True,text=True,check=True)
        r['parser_equality']=r['parser']==json.loads(oracle.stdout)
    if part!='raw' and r['status']=='ok':
        expected=base64.b64decode(parts[part]) if r['selected_encoding']=='Base64' else parts[part]
        r['decoded_equality']=r['decoded_sha256']==hashlib.sha256(expected).hexdigest()
    print(name,r['status'],r['commands'],r['process_memory']['peak_working_set'])
    return r

if __name__=='__main__':
    subprocess.run(['python',str(ROOT/'corpus.py')],check=True,capture_output=True)
    # Extra targeted case: inner part 8 MiB decoded, plus outer 4 MiB unrelated attachment.
    nested=(CORPUS/'nested.eml').read_bytes()
    _,parts=model(nested)
    def wrapped(data):
        s=base64.b64encode(data);return b'\r\n'.join(s[i:i+76] for i in range(0,len(s),76))
    big=bytes(range(256))*32768
    large=nested.replace(parts['2.2'],wrapped(big)).replace(parts['3'],wrapped(b'u'*(4*1024*1024)))
    (CORPUS/'nested-large.eml').write_bytes(large)
    results=[]
    for n in (65536,1048576,8388608):
        for claim in ('honest','absent','lying'):results.append(run(f'raw-{n}-{claim}',f'plain-{n}.eml',claim))
    for n in (2097151,2097152,2097153):
        results.append(run(f'raw-boundary-{n}',f'plain-{n}.eml','absent'))
    for f in ('malformed.eml','nested.eml'):results.append(run(f'reconstruct-{f}',f))
    for fault in ('oversized','overpartial','no-newline','aggregate'):
        results.append(run(f'fault-{fault}','plain-65536.eml','lying',fault))
        recovery=run(f'fresh-after-{fault}','plain-65536.eml');recovery['unrelated_progress_on_new_connection']=recovery['status']=='ok';results.append(recovery)
    results.append(run('fault-wrong-offset','plain-1048576.eml','honest','wrong-offset'))
    for fixture in ('nested.eml','nested-large.eml'):
        oracle=json.loads(subprocess.run([str(EXE),'oracle',str(CORPUS/fixture)],capture_output=True,text=True,check=True).stdout)
        for part,name in (('2.2','inner.bin'),('3','outer.bin'),('2','forwarded.eml')):
            r=run(f'part-{fixture}-{part}',fixture,part=part)
            expected=next(x for x in oracle['attachments'] if x['name']==name)
            r['current_parser_decoded_equality']=r['decoded_sha256']==expected['sha256'];r['current_parser_local_index']=expected['local_index']
            results.append(r)
    (OUT/'measurements.json').write_text(json.dumps(results,indent=2)+'\n')
    small=next(r for r in results if r['case']=='reconstruct-nested.eml')
    inner=next(x for x in small['parser']['attachments'] if x['name']=='inner.bin')
    direct=next(r for r in results if r['case']=='part-nested.eml-2.2')
    assert direct['decoded_sha256']==inner['sha256']
    assert all(r['raw_equality'] and r['parser_equality'] for r in results if 'raw_equality' in r and r['fault']!='wrong-offset')
    assert next(r for r in results if r['fault']=='wrong-offset')['raw_equality'] is False
    assert all(r['decoded_equality'] for r in results if 'decoded_equality' in r)
    assert all(r['current_parser_decoded_equality'] for r in results if 'current_parser_decoded_equality' in r)
    assert all(r['poisoned'] and r['reuse_rejected'] for r in results if r['fault'] and r['fault']!='wrong-offset')
    assert all(r['max_response_admitted']<=r['response_cap'] for r in results)
    print('Assertions passed; results:',OUT/'measurements.json')
