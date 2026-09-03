# Changelog

Every notable change to Wind is recorded here.

The format is inspired by [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
and the project follows [semantic versioning](https://semver.org/lang/fr/):
the `MAJOR.MINOR.PATCH` increment rule specific to Wind (Wind
exposing no public API) is fixed in
[`docs/HANDOVER.md`](docs/HANDOVER.md) §2.9.

The signed packages and their notes live in the
[GitHub Releases](https://github.com/smonchamps/wind/releases); the update
is automatic and signed (minisign, ADR 0013).

## [0.17.0] - 2026-09-02

Wind moves faster wherever it was counting its steps, and says what it
cannot do.

### Added

- **A single menu, by keyboard.** The eight menus of the product (row
  gestures, Feed cards, Screener, Cleanup, Paper trail, sort,
  Settings, "Move to…") share the same design and are navigated with
  the arrow keys: Enter plays, Escape closes and returns focus.
  Settings opens on its first control.
- **A message that could not load says so** and replays with a click
  ("Try again") — before, an empty frame until closing.
- **"Reply" follows `Reply-To`**: lists and notifications that ask
  for a different reply address are finally heard.
- A send that fails five times in a row on a transient outage is
  refused with its reason and frees the queue; the next ones go out.

### Changed

- **Bulk gestures go out in a single call**, all or nothing: a
  failure in the middle leaves nothing half done.
- **"Forward" no longer loads a remote image** in the composer;
  the recipient gets the whole message, images included.
- Spring cleaning and the Screener respond in a fraction of a
  second on a large mailbox (covering index: the group list
  380 → 67 ms on 200,000 messages and 5,000 senders).
- Each command no longer opens the database "from scratch": the
  opening cost drops from 36 ms to under a millisecond; indexing a
  heavy body weighs a third less memory.
- The Feed keeps its cards between two polls (the card being read
  no longer jumps section) and only keeps alive those near the
  screen.
- **The thread bars.** In three-pane, the sort bar (Archive,
  Mark as spam, Pin) is stuck under the thread header and
  stays at the top while scrolling; in full screen, its gestures
  live in the header bar, aligned with the message column; the
  Reply / Reply all / Forward / Delete bar floats at the bottom of
  each message as it scrolls.
- The initial sync resumes where it stopped; special folders are
  recognized by the role announced by the server ("[Google
  Mail]" included); an address in angle brackets is no longer
  mistaken for a thread id; the echo of a send no longer disappears
  before the Sent poll.
- Fewer network round trips and idle probes: three header fields
  instead of the whole block, body batches capped at 32 MB, a
  single `LIST` and a single `CAPABILITY` per session, one status
  probe instead of three.

### Fixed

- An unreadable mailbox no longer passes for "no Sent folder".
- A server without UIDPLUS no longer accumulates duplicates on move.
- A read error on a body is no longer final.
- In organized Inbox, the "Already read" band no longer overlaps a
  row past twenty rows (the height templates ignored the mailbox
  block and the ⋯ of organized mode).

## [0.16.0] - 2026-09-02

Wind protects itself better from itself: a single instance, gestures
that no longer get lost silently, waits that end.

### Added

- **A single Wind window at a time.** A second launch says
  "Wind is already open." and withdraws — no more duplicate
  notifications or sends competing with each other.
- **Actions the server refuses become visible.** When a
  move or a mark is refused by the server (folder gone, for
  example), Wind says so in the notice line instead of
  silently blocking all following gestures on that mailbox; the
  message stays where it was, and a new gesture on it replaces
  the previous one.
- **A light log next to the database** (`wind.log`, one megabyte at
  most, never a subject or an address) to understand after the
  fact a poll or a send.

### Fixed

- **A mailbox emptied then refilled notifies again** — a full
  emptying made the mailbox look "never synced", hence silent.
- **The real-time watch can no longer freeze forever** on a server
  or network that acknowledges without responding: it expires and
  reconnects.
- **A token expired mid-series of sends no longer makes "refuse"**
  a healthy message; replies carry the full reference chain, to
  stay in the conversation on the recipient's side.
- **Adding an account no longer waits indefinitely** for a consent
  that never comes: five minutes, then it can be retried.
- **Granted images only load over HTTPS**, without revealing the
  origin of the request; clicking a link no longer blocks the
  window while the browser opens.
- Local purges made atomic and complete (no more leftover
  attachments or invitations after a server-side deletion).

## [0.15.0] - 2026-08-30

The history depth is chosen, and the great cleanup arrives.

### Added

- **The history depth is chosen when adding an account.** From
  one month to the whole history (one year by default). The list,
  search and headers stay complete over the whole history: only
  the bodies of older messages stay on the server, and open on
  demand, with a click. The choice is then adjustable in
  Settings > Accounts; existing accounts keep everything.
- **Spring cleaning** (organized mode). A page to do the
  cleanup by sender: a range (from three months to everything) and a
  chosen scope, each group is sliced with one gesture — the verdict
  applies to mail already there AND to upcoming messages, in the
  vocabulary of the Screener. Never a permanent deletion: the
  trash stays recoverable. The session resumes where it was
  left, a gauge tracks progress.
- **The Screener's click follows adjustable defaults.** Yes sends to
  the Inbox, No to the trash — changeable in
  Settings > Screener, in both modes.
- **The Feed remembers what has been read.** New items
  appear unfolded, already-read folds into piles by sender;
  a check mark marks everything read. Its icon is redrawn.

### Changed

- **Twelve pieces of feedback on Organized mode**: the main mailbox
  is named "Inbox" in organized mode, and it is returned to after
  the Screener; the Screener's header is revised (welcome text,
  history); the Screener's and the Feed's headers share the
  same format; "Dark — Automatic" moves to the top of Themes;
  the Settings rail is aligned; the Paper trail says "Moved
  automatically to the trash".

## [0.14.0] - 2026-08-30

Organized mode: Wind sorts your mail — you keep the hand.

### Added

- **Organized mode, by invitation.** A setting, reversible at any
  time: Wind then arranges mail into three places — the
  **Inbox** for people, the **Feed** for newsletters,
  the **Paper trail** for notifications and receipts.
  "Move to" fixes a sorting with one gesture, and Wind remembers
  it for next time. Nothing is ever lost: everything stays in
  your folders, only the presentation changes.
- **The Screener.** Messages from unknown senders wait at the
  door instead of cluttering the Inbox. A simple page — Yes or
  No — decides for each one; the history keeps track of every
  verdict and allows changing one's mind.
- **No acts for real.** Saying No in the Screener can also mark
  as junk, archive or send to the trash the sender's next
  messages — never a permanent deletion, and everything
  switches off if the mode is disabled.
- **The sectioned Inbox.** "New for you" then "Already
  read": what awaits your attention separates from what is read.
- **Set aside.** A conversation to keep at hand joins a
  discreet pile at the top of the Inbox, with one gesture, and leaves it
  the same way.
- **The Feed in already-open cards.** Newsletters are
  read in place, full body, while scrolling — no click needed. Each
  card keeps image guarding and its gestures: collapse, set
  aside, move, dismiss.
- **The Innamoramento theme.** The Mona theme becomes Innamoramento,
  in light and night — garnet accent, contrasts verified.

## [0.13.0] - 2026-08-29

The message header says everything, and the small snags disappear.

### Added

- **A message header says who, to whom, in full.** At the top
  of each open message: the sender with their address
  ("Camille Rousseau <c.rousseau@…>"), then "To:" with each
  recipient — and "Cc:" when there is one. Wind finds the names
  of recipients from the contacts it already knows.

### Fixed

- **An account added while Wind is open says it is connected.**
  Settings > Accounts marked it "Disconnected" until the next
  startup, even though it had just been connected.
- **The Wind logo in the top left is more present** (28 px instead
  of 24).

## [0.12.0] - 2026-08-28

Wind remembers your image choices, and the beta gets tooled up.

### Added

- **Wind remembers your choice to show images.** Clicking
  "Show images" on a message now holds for good:
  reopening that message no longer asks again. And a new button,
  "Always show images from this sender", displays
  the images of all their messages by default, without a banner. The
  list of these senders can be viewed and removed in
  Settings > Display. Remote images of other messages
  stay blocked by default, as before.
- **A Feedback button top right.** Write what is wrong or
  what is missing: your message goes out by email from your account,
  with the Wind version — we read every piece of feedback. The
  first-launch onboarding gains a step introducing it.
- **"Made in EU"** with the flag of the European Union, in
  Settings > About.

## [0.11.0] - 2026-08-27

Multi-select arrives in the list, and the Wind brand asserts itself.

### Added

- **Select several conversations at once.** Ctrl-click adds
  a conversation to the selection, Shift-click extends the range from
  the chosen conversation, and a checkbox appears on hover over
  each row. As soon as a selection exists, the list bar
  transforms: mark read or unread, archive, mark as junk
  (or "Not spam" in the Junk folder),
  delete, undo — a single confirmation message for the
  whole batch, and the `e` / `Delete` shortcuts apply to the checked
  batch.
  A bulk gesture carries each conversation **entirely** — all
  the messages of its threads, not just the last one.

### Changed

- **The application icon is now the current Wind brand**
  (the flap envelope) — in the taskbar, on
  the executable and at install.
- **The brand top left of the window is more present**
  (24 px instead of 20).
- **The left pane icons align better with their
  labels** — an optical alignment chosen on a proof sheet.

## [0.10.2] - 2026-08-27

An update failure is now visible, instead of closing the application.

### Fixed

- **Clicking "Install" can no longer close Wind without doing anything.**
  When Windows refused to launch the downloaded installer (this is the
  case on PCs where *Smart App Control* is
  active), the application closed without a word and nothing installed.
  Now the failure shows in the banner, with its reason, and the
  "Install" button stays there to retry. On these PCs,
  installation stays blocked by Windows until Wind is
  signed with a publisher certificate — that is the next job; in
  the meantime, at least, Wind tells you.
- **An update that stalls eventually says so.** The
  download had no time limit: a connection that stalled
  left the banner on "Installing…" forever. After
  ten minutes, the failure shows and retries.
- **Wind installs exactly the version the banner announces** —
  never another one published in the meantime.

## [0.10.1] - 2026-08-26

Startup stops freezing.

### Fixed

- **Wind no longer freezes a few seconds after opening.** On
  a large mailbox, the application stopped responding about three
  seconds after launch, and for nearly **nine seconds**:
  impossible to scroll the list, open a message or
  switch folders. The window itself kept moving — which
  made it all the more confusing. That is over. Measured on a
  mailbox of 251,000 messages: **8.9 seconds of waiting became a
  tenth of a second**.
- **The list shows three times sooner.** It used to request its first
  page in twelfth position, behind all the startup
  checks; it now goes first. From the window opening to the
  messages on screen: **1.2 seconds before, 0.4 today**.

**Only once, at this update**: the first launch will take
about two seconds longer than usual. Wind reorganizes an index
of its database — that is what makes everything above possible. The
following launches are instant.

## [0.10.0] - 2026-08-25

The list says where each message came from, and you choose its look.

### Added

- **Each message says which mailbox it arrived on**: when your
  accounts mix — "All mailboxes", or a search —, the
  row spells it out in full behind the sender's name,
  "Camille Roux on Work", with the account's marker in its color.
  No more need to remember a color or a logo, and an account
  without a marker states its mailbox like the others. The mention is
  found identically on open messages; where the account is already
  known — a single-mailbox view, or a single account configured —,
  it says nothing and disappears.
- **Three spacing levels for the list** (Settings > Display):
  "Low" — what you had until now, to the pixel —, "Medium"
  and "High". More air between messages if you prefer to breathe,
  as much as before if you prefer to see many. The change
  applies instantly, and the list stays where you left it:
  the message you were looking at does not move on screen.

### Changed

- **The initials thumbnail leaves the list**: the sender's
  name, written right above, already said what it said —
  the space returns to the message. It stays where it works: on
  the messages of a conversation, and in the Drafts folder.
- **An account's marker is drawn in outline** in navigation, in
  place of the solid dot: the exact same mark as that
  of the row. The dot stays in Settings, where it serves
  to choose.

## [0.9.0] - 2026-08-24

Wind changes skin: the "Elements" direction.

### Changed

- **A new face, drawn with a single hand**: sharp corners
  everywhere, an original icon set drawn for Wind (no more
  embedded icon font), a new brand, and the teal disc
  that says in one glance what is unread and what is working.
- **Two themes instead of twenty-eight**: "Elements" (light) and
  "Elements · night", composed and measured together. Your old
  choice migrates on its own — a dark theme stays dark. Windows'
  light/dark tracking works as before.
- **The list says unread with the disc**: a teal dot in front
  of the sender, in addition to the bold — and the navigation
  counter becomes a discreet number.

### Removed

- The twenty-six Wada themes and the calligraphic stroke of the brand.

## [0.8.0] - 2026-08-23

Name your accounts, and connect without any configuration.

### Added

- **Give your accounts a name**: in Settings > Accounts, click
  an account's address to give it a name ("Work",
  "Personal"…). The name displays in navigation, in
  the tooltips of the unified mailbox and in the sender picker
  of the composer — the address stays visible in settings, and your
  messages always go out with your address, never the name.

### Improved

- **Account sign-in no longer requires any configuration**:
  installed versions of Wind now carry everything
  needed to connect to Google and Microsoft — nothing left
  to set up on the machine.
- **The account removal button says its name**: "Remove
  account", spelled out next to the icon. Removing an account
  deletes nothing on the server, as before.

## [0.7.0] - 2026-08-23

Reply to meeting invitations without leaving your mailbox.

### Added

- **Meeting invitations are handled in Wind**: an invitation
  received (Google Calendar, Outlook, etc.) displays as a readable card in
  the conversation: title, date and time in your timezone,
  organizer, location, recurrence. Three buttons: Accept, Tentative,
  Decline; your reply goes out by email to the organizer, the same way
  other mail clients do. You can change your mind: the last reply sent
  is the one that counts, and the list row carries a chip "Accepted",
  "Tentative" or "Declined".
- **Reply from the list**: the three buttons appear directly on the
  conversation row in the center pane, without opening the message.
- **A cancelled meeting shows as cancelled**: the cancellation notice
  marks the original invitation "Cancelled", even if it arrives in a
  different conversation; the reply buttons are removed.
- **Invitations already received earn their card**: on the first
  launch of this version, Wind passes over existing mail once to
  recognize invitations that arrived earlier.

### Changed

- **The "Delete" button now lives in every message** of the
  conversation, next to Reply and Forward: it deletes THAT message;
  the conversation stays open if any remain.

## [0.6.0] - 2026-08-22

A marker for each mailbox, a first guided startup, and Wind is now
available in two Windows editions, arm64 and x64.

### Added

- **A marker for each mailbox**: in Settings > Accounts, give each
  address an icon and a color. The marker shows in the navigation
  panel in place of the generic icon, and, in "All mailboxes" mode as
  in search results, as a badge under the initial of each message: you
  can tell at a glance which account a message arrived on or was sent
  from. Twelve icons, twelve colors, all readable across the 28
  themes, light as well as dark.
- **A first guided startup, in four steps**: add your addresses,
  choose your window layout on real previews, choose your theme,
  review your choices. Each step applies immediately, and every choice
  stays editable afterward in Settings. An interrupted run resumes on
  the next launch; existing installs never see it.
- **Wind for Windows x64**: every version is now published in two
  editions: arm64 (native Snapdragon) and x64 (Intel/AMD PCs), with
  the same signed automatic update.

## [0.5.0] - 2026-08-21

Pin your conversations, and a clearer reading of both attachments and
open conversations.

### Added

- **Pin a conversation**: in the Inbox, a "Pin" button in the
  conversation bar keeps it **always at the top of the list**, marked
  "Pinned" and in the hue of the selected mailbox. It leaves its place
  in the date thread (never duplicated). "Unpin" brings it back.
  Your pins survive a restart; they stay on your machine.

### Changed

- **Attachments now display at the top of the message**, right under
  its header: no more need to scroll through the whole mail to find
  them.
- **Hovering an attachment states the action**: the chip is covered by
  a veil reading "Save" with its download arrow: you know what will
  happen before you click.
- **The open conversation ("Open") is now flat**, like the reading
  pane: each message in its own card, the page scrolls as one, in a
  comfortable reading column: no more enclosing frame.

## [0.4.0] - 2026-08-21

Your signature, sending at a chosen time, and marking as important.

### Added

- **Signature per account** (Settings > Signature): write a signature
  (formatting included) for each account; it adds itself automatically
  to the bottom of your new messages, and, if you enable it, to your
  replies and forwards (between your text and the quoted message).
  "Apply to all accounts" copies the signature and that choice
  everywhere in one action. In the composer, switching the sending
  account reloads the matching signature as long as you have not
  written anything yet: your text is never rewritten.
- **Send later**: next to "Send", choose a date and time: the message
  goes out at that moment **if Wind is open** (otherwise, on the next
  launch; Wind tells you so when you schedule it). The status bar
  shows the planned departure, and a banner lets you **cancel** at any
  time: the message then returns to your drafts, attachments included:
  nothing is lost.
- **Mark a message as important**: a "!" button in the composer's
  formatting bar. The message goes out with the standard priority
  headers (Outlook and Thunderbird show the "!" to the recipient;
  Gmail web ignores it in the display: that is its own behavior, the
  header is indeed present). The marking follows your drafts.

### Changed

- **The "New message" window header** now carries the same color as
  Wind's footer: the card is framed top and bottom in the same hue.

## [0.3.0] - 2026-08-21

Addresses complete themselves, and a send in progress tells the truth.

### Added

- **Address autocomplete** in the To, Cc and Bcc fields: from two
  letters typed, Wind suggests the addresses it knows, the people who
  write to you (with their name) and the ones you have written to,
  ranked from the most recent and frequent to the rest. Arrow keys then
  Enter, or a click, insert the address; Escape closes the menu.
  Senders from your Junk and Trash are never suggested.

### Fixed

- **A send pending synchronization now displays correctly** in
  "Sent": right after a send, the temporary entry (which waits for
  your mail server to file its own copy) used to say "To: sent" and
  showed an empty "Attached files" section. It now states the real
  recipient and the name and size of each attachment sent (not
  downloadable during this short wait: the real copy takes over a
  moment later).

## [0.2.1] - 2026-08-20

The list no longer freezes, and everything displays faster.

### Fixed

- **Scrolling fast on the scrollbar no longer blocks the app**:
  dragging the scrollbar quickly through a large folder (Archive, for
  example) used to leave the list in "…" blocks, then wrongly show
  "No messages here." in every folder for several minutes. The list
  now only requests what it shows: when the gesture stops, the
  messages appear at once, and switching folders responds immediately.
- **The empty screen no longer lies**: "No messages here." only shows
  once the mailbox has actually been checked; while loading, waiting
  rows say so honestly.

### Changed

- **Startup and a folder's first display are now immediate**: the
  internal counts (including the most costly one, Gmail Archive) no
  longer delay the display of messages: the item count and the
  scrollbar adjust right after, on their own.

## [0.2.0] - 2026-08-20

Formatting arrives in the composer.

### Added

- **A real formatting bar** in the compose window: font (sans serif,
  serif, monospace), size (four steps), bold, italic, underline,
  strikethrough, text color (a twelve-hue swatch), left/center/right
  alignment, bulleted and numbered lists, indent, and "Clear
  formatting". The Ctrl+B/I/U shortcuts also work.
- Your messages now go out **as HTML with an automatic text
  fallback**: recipients see your formatting, and text clients still
  receive a readable version.
- **Quoting a reply** appears in a block with a left rule, as in mature
  mail clients; your reply is written above it.
- **Drafts keep their formatting**, even after a round trip through
  your mail server's Drafts folder.
- **Reconnect an account**: when an account's connection expires or is
  revoked, Settings > Accounts flags it ("Disconnected") and a
  "Reconnect" button restarts authorization in the browser, without
  losing anything or re-syncing. The onboarding notice leads directly
  to this page.

### Changed

- **Remote images, depending on the gesture**: quoting a reply
  replaces them with a neutral pixel (no tracker from the quoted
  message loads without your knowledge); a forward, on the other hand,
  passes on the whole message, images included.

## [0.1.11] - 2026-08-19

Three findings from the field.

### Changed

- **Saving an attachment**: clicking an attachment now opens a "Save
  as" window where you choose the folder and file name, instead of a
  silent save to Downloads.
- The **name and size of an attachment** are now combined into a
  single chip, more readable at a glance.

### Fixed

- On **dark themes**, message bodies now display on a light
  background: email text (often composed for a white background, like
  newsletters) becomes readable again, instead of sometimes appearing
  black on a dark background.

## [0.1.10] - 2026-08-18

Four findings from the field.

### Added

- **Report a mail as junk**, and the reverse: a "Report as spam"
  button moves the conversation to your mail server's junk folder:
  it is the one that learns. From the Junk folder, "Not spam" brings
  it back to the Inbox.
- **Delete a draft** directly from the compose window, in one gesture
  and after confirmation (distinct from "Cancel", which keeps the
  draft).
- **Reply message by message**: the Reply, Reply all and Forward
  buttons are now at the bottom of each message in a conversation, to
  reply precisely to the one you are reading, including your own
  messages, in which case the reply goes back out to the original
  recipients.

### Changed

- **Message backfill** shows a progress percentage in the status bar,
  next to the count of remaining messages.

## [0.1.9] - 2026-08-17

Four findings from the field.

### Added

- **Cc and Bcc** in the composer: add recipients in copy and in blind
  copy. The blind copy stays blind (it never appears in the message
  received by the others); "Reply all" places the original Cc back in
  Cc.

### Changed

- **Gmail synchronization is much lighter**: the full sweep of the
  folders, which could take a while and used to repeat every 5
  minutes, now runs every 30 minutes. The arrival of new mail, though,
  stays **instant**: nothing changes in what you receive, only in the
  background load.
- The **loading animation** (the bar) is simplified: one full, smooth
  animation as soon as an action is in progress, instead of a bar that
  could stay stuck.

### Removed

- The "Detach" button of the composer, which did nothing, is removed
  (the detached compose window will return later).

## [0.1.8] - 2026-08-16

Four fixes on real mail, reported from the field.

### Fixed

- Subjects and sender names no longer show the stray backslashes of
  quoted IMAP strings (e.g. `Test \"Sent\"`); already synchronized
  messages are repaired on first launch.
- The "Sent" folder finally shows the real recipient ("To: …"), both
  in the list and when reading, instead of repeating your own address;
  the information is backfilled on already synchronized sends.
- "Reply all" prefills "To" instantly, from the stored recipients,
  without waiting for a server poll on every click.
- The subject no longer displays twice at the top of the body of
  certain newsletters (the title of their HTML header no longer leaks
  into the message).

## [0.1.7] - 2026-08-16

The full redesign on the workstation: System v2 "Wada" and its
expansion, UI v3 and its CE feedback, on a window that no longer
freezes.

### Added

- Three display modes to choose from — three panes (unchanged
  default), two panes, or one pane with a navigation drawer
  (PLAN-VOLETS).
- Visual System v2 "Wada": palette remapped to a constant-use hue, the
  hitofude stroke as signature and sole progress indicator, nav and
  list drawn like tracks, 119 tokens (PLAN-WADA).
- 28 themes and automatic dark via a `-night` variant
  (PLAN-WADA-ELARGI).
- UI v3: list banner, avatars, the reading pane becomes the thread;
  mouse-resizable panes, native bars (PLAN-UI-V3, PLAN-RETOURS-V3).

### Changed

- Reading pane drawn to the exact design of the Classic mockup; the
  Expand/Collapse toggle derived from state, body height fitted to
  content, lighter compose header, "All" labels (CE feedback
  A44-A47).

### Removed

- The v1 interface: the redesign is complete (PLAN-RETRAIT-V1).

### Fixed

- The window no longer freezes: no blocking command on the main
  thread, never any CPU inside the write-lock window, `busy_timeout`
  raised to 30 s (PLAN-GELS, ADR 0019).
- A link in the body opens the system browser and the body never
  moves; the iframe stays inert (A37, invariant S1).
- The language reads without adopting the database; the migration
  modal stays the first surface to pay for the adoption (ADR 0012).
- Two simultaneous e2e suites no longer step on each other: a free
  CDP port per suite, sweep bounded to the worktree
  (PLAN-ISOLATION-E2E).

## [0.1.6] - 2026-08-14

### Fixed

- Display responsiveness (PLAN-REACTIVITE), validated in the field: no
  more waiting rows during a synchronization; delete, archive and send
  visible in their folder in under a second, offline included (local
  echo); the preview arrives with the row, in a single display.

## [0.1.5] - 2026-08-14

### Fixed

- Icons of rare notices (including the update banner): font extended
  to 43 glyphs.
- The Sent copy polls as soon as the send is accepted (`sync_sent`).

## [0.1.4] - 2026-08-14

### Added

- Attachments: real send and forward.

### Fixed

- Attachment display on first opening (field finding of 2026-08-14).

### Security

- First update signed under the new key (signing key rotation of
  2026-08-14).

## [0.1.3] - 2026-08-14

### Changed

- Discovery becomes **Wind** (PLAN-WIND) — the database relocates
  automatically on first launch.
- Native arm64 channel.

### Security

- Signing key rotation: manual install required from discovery 0.1.2
  onward; the auto-update chain resumes after that.

## [0.1.2] - 2026-07-26

### Fixed

- `latest.json` fixed: BOM removed and URL to the bare tag — the
  auto-update succeeds (ADR 0013).

## [0.1.1] - 2026-07-26

### Added

- First published version (discovery): NSIS installer and minisign
  signed update, driven from Rust (ADR 0013).

[0.1.11]: https://github.com/smonchamps/wind/releases/tag/0.1.11
[0.1.10]: https://github.com/smonchamps/wind/releases/tag/0.1.10
[0.1.9]: https://github.com/smonchamps/wind/releases/tag/0.1.9
[0.1.8]: https://github.com/smonchamps/wind/releases/tag/0.1.8
[0.1.7]: https://github.com/smonchamps/wind/releases/tag/0.1.7
[0.1.6]: https://github.com/smonchamps/wind/releases/tag/0.1.6
[0.1.5]: https://github.com/smonchamps/wind/releases/tag/0.1.5
[0.1.4]: https://github.com/smonchamps/wind/releases/tag/0.1.4
[0.1.3]: https://github.com/smonchamps/wind/releases/tag/0.1.3
[0.1.2]: https://github.com/smonchamps/wind/releases/tag/0.1.2
[0.1.1]: https://github.com/smonchamps/wind/releases/tag/0.1.1
