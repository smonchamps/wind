# ADR 0022 — Rich body: body_html next to text, a single boundary, images by gesture

Date: 2026-08-20 · Status: accepted

## Context

R4 (PLAN-COMPOSITION-HTML) brings formatting to the composer. ADR 0003
had explicitly refused outgoing HTML ("plain text only"); lifting it
forces a decision on where the HTML lives, what has authority over the
fallback text, and what becomes of the remote images of quoted content
— three decisions that cross drafts, the send log, SMTP, pulling a
remote draft, and the editor.

## Decision

**1. `body_html` in a NULLable column NEXT TO the text** (`drafts`,
`outbox`) — never in its place. `body`/`body_text` are ALWAYS
populated: previews, search, `text/plain` fallback. `NULL` = the
historical text path, byte for byte (legacy databases do not move —
migration `add_missing_columns`, reversible).

**2. ONE entry boundary** (`frontiere_corps`, commands.rs) for EVERY
body that enters the database — composer, send, drag-out: sanitizes
via ammonia, DERIVES the fallback text from the same HTML (a single
authority, never two truths), falls back to the text path if the
render is empty (the residual `<br>` of an emptied editor does not
make for an empty `text/plain` send). The `contenteditable` editor
accepts HTML ONLY through this boundary — this is the exception
bounded by "never `innerHTML`" (STANDARD §6.4).

**3. Sending goes out as `multipart/alternative`** (text first — RFC
2046, simplest to most faithful), nested inside `mixed` with the
attachments; the Drafts mirror does the same (`draft_bytes`). Never
HTML alone.

**4. Remote images are decided AT THE GESTURE** (field verdict D5): a
**reply** quotes at neutral pixel — an `AllowRemote` quote reposted in
the editor (main document, CSP `img-src https:`) loaded the spy pixels
of the quoted message on a simple click of "Reply" (review of
2026-08-20); a **forward** keeps the images — the recipient receives
the whole message, composing the forward is itself an implicit "show
images." The boundary is `AllowRemote` (idempotence: it does not
re-neutralize what upstream already decided).

**5. The HTML vocabulary is the ammonia allowlist of READING** — the
editor emits via legacy `execCommand` (`styleWithCSS` off), whose
output (`b/i/u/strike`, `font color/face/size`, `align`, lists,
`blockquote`) is exactly what `sanitize.rs` keeps: nothing to widen,
no translation layer.

## Set aside

- Replacing `body` with the HTML (one column): breaks previews, search
  and fallback — heavy migration for nothing.
- Third-party editor (Quill, ProseMirror): heavy dependency, its own
  document model to re-map onto ammonia — against ADR 0015.
- `BlockRemote` on forward (first form): overturned in the field (D5)
  — a forward stripped of its images does not carry the message.

## Consequences and watch points

- A draft reopened then closed WITHOUT typing must re-emit the stored
  values byte for byte: the browser's `innerHTML` re-serialization is
  never faithful, and the core's "identical content" detection would
  silently repush a Gmail copy on every open (STANDARD §9, the
  contenteditable traps).
- Pulling a draft back (`import_remote_draft`) carries the HTML:
  without it, a rich draft pushed then pulled back silently lost its
  formatting.
- Rule tests: `save_draft_roundtrips_body_html`,
  `body_html_change_marks_the_draft_dirty`,
  `enqueue_roundtrips_body_html`,
  `html_body_travels_as_multipart_alternative_with_plain_fallback`,
  `html_body_with_pieces_nests_alternative_inside_mixed`,
  `l_echo_d_un_envoi_riche_porte_le_html_compose`,
  `import_remote_draft_keeps_the_rich_body`, e2e "formatting."
