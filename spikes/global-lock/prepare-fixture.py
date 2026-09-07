"""Prepare ONE disposable copy of the gate-3 fixture for a run.

  python prepare-fixture.py <template.db> <run.db> <attach-file> [big_uid]

- copies the template (never mutates the original);
- replaces the cached body of `big_uid` (default 199696, one of the 500
  cached bodies) with a ~10 MB synthetic HTML body: the "sanitize under
  the lock" heavy work of the spike (commands.rs `body_view`, D-1);
- writes a 25 MiB pseudo-random file for `attach_files` (exactly the
  per-draft budget MAX_ATTACHMENTS_BYTES, so one attach per fresh draft
  is accepted);
- prints, as JSON, the uids that carry a cached body (the gestures pick
  their "open a thread" rows among them: a missing body would go to the
  network and fail offline).
"""
import json
import os
import shutil
import sqlite3
import sys

template, run_db, attach = sys.argv[1], sys.argv[2], sys.argv[3]
big_uid = int(sys.argv[4]) if len(sys.argv) > 4 else 199696

for suffix in ("", "-wal", "-shm"):
    try:
        os.remove(run_db + suffix)
    except FileNotFoundError:
        pass
shutil.copyfile(template, run_db)

# ~10 MB of newsletter-like HTML: paragraphs, links, inline styles,
# remote images (the sanitizer has to rewrite every one of them).
block = (
    '<table style="width:100%;border:0"><tr><td style="padding:4px;font-family:Arial">'
    '<p style="color:#333"><b>Lorem ipsum</b> dolor sit amet, <a href="https://example.com/x?y=1">consectetur</a> '
    'adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. '
    '<img src="https://img.example.com/pixel.gif" width="1" height="1" alt=""> '
    '<span style="font-size:12px;color:#777">Ut enim ad minim veniam, quis nostrud exercitation.</span></p>'
    '<ul><li>un</li><li>deux</li><li>trois</li></ul></td></tr></table>\n'
)
target = 10 * 1024 * 1024
html = "<html><body>" + block * (target // len(block)) + "</body></html>"

conn = sqlite3.connect(run_db)
conn.execute("PRAGMA journal_mode=WAL")
before = conn.execute("SELECT length(html) FROM bodies WHERE mailbox_id=1 AND uid=?", (big_uid,)).fetchone()
assert before is not None, f"uid {big_uid} has no cached body in the template"
conn.execute("UPDATE bodies SET html=? WHERE mailbox_id=1 AND uid=?", (html, big_uid))
conn.commit()
uids = [r[0] for r in conn.execute("SELECT uid FROM bodies WHERE mailbox_id=1 ORDER BY uid DESC")]
conn.close()

size = 25 * 1024 * 1024
if not (os.path.exists(attach) and os.path.getsize(attach) == size):
    with open(attach, "wb") as f:
        f.write(os.urandom(size))

print(json.dumps({"big_uid": big_uid, "big_len": len(html), "body_uids": uids, "attach": attach}))
