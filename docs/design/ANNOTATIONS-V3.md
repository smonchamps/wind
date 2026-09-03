# CE annotations — toward a v3 UI

Annotation session on the `prototype-classique.html` mockup (Classic
mode, System v2 "Wada"), integrated browser panel. Each entry is a CE
verdict recorded as given; the synthesis and the v3 proposal come after
the session.

## 2026-08-16

### 1. Side navigation (`.nav`, 248 px rail)
**CE verdict: keep v2 as it is.**
Block concerned: the whole rail — mailboxes (Inbox/Sent/Drafts/
Archives/Junk/Trash, unread badge), Library section (Files, Contacts,
Collections, Journeys, Snippets, Templates), Labels, Mailboxes section
(All mailboxes, Work…). No change in v3.

### 2. Header (`header.entete`, 52 px)
**CE verdict: replace the v2 header with the Classic mockup's.**
Retained composition (as in `prototype-classique.html`):
- "Wind" brand 18 px + hitofude stroke (accent SVG, offset 3 px below
  the baseline, without the envelope tile — A30);
- central search field (max 520 px, placeholder « Chercher un message,
  une personne, un fichier »);
- on the right: accent button "Écrire" and button "Réglages".

### 3. List header band (`.listeTete`)
**CE verdict: add this band in v3, WITHOUT the "Tout marquer lu" button.**
We keep the mailbox title (h1 16 px, « Boîte de réception »…) at the
top of the list pane; the mini "Tout marquer lu" button is set aside.

### 4. Message list (`.liste` / `.ligne`)
**CE verdict: the v3 email list follows the mockup's format.**
Track drawing (A29.3/A30): continuous rows separated by a hairline,
14 px, avatar grid (28 px, clickable = batch selection) · name +
tabular time · subject · one-line preview. Unread at 700, hover in a
light tint, chosen row in tint + accent left edge. Chip row under the
preview when present: label (accent), « Brouillon : » in alert ink in
the preview, « Remonté ce matin », italic row note.

### 5. List pane filters (`.filtres` — Tous / Non lus / Brouillons)
**CE verdict: keep the v2 block as it is.**
The mockup's filter tabs (Tous / Non lus / Brouillons at the foot of
the list) do not replace the existing ones: v3 keeps the v2 filtering
device.

### 6. Reading pane (`.voletLecture` / `.lecture`)
**CE verdict: this layout replaces the v2's — subject to the
exceptions listed below (dictation in progress).**
Retained composition:
- thread title (h1 24 px);
- subtitle as chips: n messages, n files, labels (accent);
  bare button "Tout déplier" on the right;
- messages as cards (`.carteMsg`): older ones folded on one line
  (avatar · name · summary · when), the last one unfolded as a full
  card (header with address and recipient, body 68ch max, "Fichiers
  joints" section as chips);
- ~~private thread note on a yellow ground (« jamais transmise »)~~ →
  removed, see exception a.

**CE exceptions:**
- **a. Private thread note (`.noteFil`): not implemented.** The yellow
  "private note, never sent" block is not part of v3.
- **b. "More" button of the action bar (`.barreActions`): set aside at
  this stage.** The thread's action bar keeps its direct buttons
  (Répondre, Répondre à tous, Transférer, Supprimer…) without the
  "⋯ Plus" menu.
