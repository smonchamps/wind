"""Synthetic corpus shared by Lot 3 spikes. No real messages/network."""
from pathlib import Path
import base64, hashlib, json

ROOT = Path(__file__).parent / 'corpus'
ROOT.mkdir(exist_ok=True)
def plain(n):
    h = b'From: synthetic@example.invalid\r\nTo: synthetic@example.invalid\r\nSubject: generated\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n'
    return h + b'x' * (n-len(h))
for n in (65536, 1048576, 2097151, 2097152, 2097153, 8388608):
    (ROOT / f'plain-{n}.eml').write_bytes(plain(n))
outer = bytes(range(256)) * 16
inner = b'inner attachment\x00\xff\r\n' * 31
def b64(data):
    encoded = base64.b64encode(data)
    return b'\r\n'.join(encoded[i:i+76] for i in range(0,len(encoded),76))
nested = (b'From: synthetic@example.invalid\r\nSubject: nested\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary="outer"\r\n\r\n'
    b'--outer\r\nContent-Type: text/plain\r\n\r\nvisible outer text\r\n'
    b'--outer\r\nContent-Type: message/rfc822\r\nContent-Disposition: attachment; filename="forwarded.eml"\r\n\r\n'
    b'From: nested@example.invalid\r\nSubject: forwarded\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary="inner"\r\n\r\n'
    b'--inner\r\nContent-Type: multipart/alternative; boundary="alt"\r\n\r\n'
    b'--alt\r\nContent-Type: text/plain\r\n\r\ninner text\r\n--alt\r\nContent-Type: text/html\r\n\r\n<b>inner text</b>\r\n--alt--\r\n'
    b'--inner\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename="inner.bin"\r\nContent-Transfer-Encoding: base64\r\n\r\n' + b64(inner) +
    b'\r\n--inner--\r\n--outer\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename="outer.bin"\r\nContent-Transfer-Encoding: base64\r\n\r\n' + b64(outer) + b'\r\n--outer--\r\n')
(ROOT / 'nested.eml').write_bytes(nested)
(ROOT / 'malformed.eml').write_bytes(b'Content-Type: multipart/mixed; boundary="missing"\r\n\r\nthis body has no delimiters\x00\xff')
(ROOT / 'outer.bin').write_bytes(outer)
(ROOT / 'inner.bin').write_bytes(inner)
manifest = {p.name: {'bytes':p.stat().st_size, 'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in ROOT.iterdir() if p.is_file() and p.suffix != '.json'}
(ROOT / 'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
print(json.dumps(manifest))
