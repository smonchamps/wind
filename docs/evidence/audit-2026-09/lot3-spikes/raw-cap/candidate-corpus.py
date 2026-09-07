"""Extra ceiling candidates; synthetic fixture generation outside measured process."""
from pathlib import Path
import hashlib, json, base64
ROOT=Path(__file__).parent/'corpus'
header=b'From: synthetic@example.invalid\r\nSubject: candidate\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n'
def fill_file(path, prefix, size, suffix=b''):
    with path.open('wb') as f:
        f.write(prefix)
        remaining=size-len(prefix)-len(suffix)
        assert remaining>=0
        block=b'x'*65536
        while remaining:
            n=min(remaining,len(block));f.write(block[:n]);remaining-=n
        f.write(suffix)
for cap in (8*1048576,16*1048576,32*1048576):
    for size in (cap-1,cap,cap+1):
        # Preserve shared original fixture (same length, different synthetic header) if present.
        path=ROOT/f'plain-{size}.eml'
        if not path.exists():fill_file(path,header,size)
    fill_file(ROOT/f'html-{cap}.eml',header.replace(b'text/plain',b'text/html')+b'<html><body><p>',cap,b'</p></body></html>')
    decoded=bytes(range(256))*(cap//2//256)
    encoded=base64.b64encode(decoded)
    encoded=b'\r\n'.join(encoded[i:i+76] for i in range(0,len(encoded),76))
    prefix=b'From: synthetic@example.invalid\r\nSubject: candidate mixed\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary="candidate"\r\n\r\n--candidate\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>'
    suffix=b'</p>\r\n--candidate\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename="large.bin"\r\nContent-Transfer-Encoding: base64\r\n\r\n'+encoded+b'\r\n--candidate--\r\n'
    fill_file(ROOT/f'mixed-{cap}.eml',prefix,cap,suffix)
fill_file(ROOT/'plain-67108864.eml',header,67108864)
manifest={}
for p in ROOT.glob('*.eml'):
    h=hashlib.sha256()
    with p.open('rb') as f:
        while chunk:=f.read(1048576):h.update(chunk)
    manifest[p.name]={'bytes':p.stat().st_size,'sha256':h.hexdigest()}
(ROOT/'candidate-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(f'{len(manifest)} generated/preserved synthetic MIME files')
