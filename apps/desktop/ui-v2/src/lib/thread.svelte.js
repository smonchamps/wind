// The state of the open THREAD — ONE object, TWO frames (UI v3,
// decision D4 from 2026-08-16: “a coexistence that is only a size
// change of the same objects”). The reading pane and screen 03 both
// mount Thread.svelte on THIS state; `frame` says which one holds the
// object — it is the ONLY switch, exclusivity is structural (v3
// review: three booleans reconciled by hand would fall out of sync at
// the first forgotten path — archiving by keyboard, layout toggle).
//
// Invariant S1 intact: each body comes from message_body/echo_body
// (sanitized on the core side), loaded only on unfolding, displayed
// in a sandbox iframe per message — never innerHTML.
import { call } from './transport.js';

const EMPTY = () => ({
  messages: [],
  expanded: {},
  body: {},
  // PLAN-AUDIT-V2 E10: a body that the core has not served — the guard
  // `corps[k] === undefined` refused any reload: an empty body lived
  // until the thread closed.
  errors: {},
  attachments: {},
  // The AFTER-SCAN attachment count per message (vue.attachment_count
  // from message_body): the list row carries the one from BEFORE
  // opening — trusting it opened freshly received attachments on
  // an empty row (CE field finding, 2026-08-14; regression caught at
  // the v3 review).
  attachmentCount: {},
  blockedImages: {},
  // The invitation card per message (PLAN-INVITATIONS): the view
  // arrives WITH the body (BodyView.invitation) — no dedicated
  // round trip. `undefined`/`null` = no card; object = the card.
  invitations: {},
  // R4 (PLAN-RETOURS-7): is the open conversation pinned?
  // Read by the THREAD on the core side (pin_state) on opening, kept
  // up to date by the gesture (App.epinglerFil). False by default —
  // the button says “Pin” as long as the core has not answered.
  pin: false,
});

export const thread = $state({
  // Which frame holds the object: null (none), 'pane', 'full'.
  frame: null,
  // The origin row (list selection) and the per-message state.
  row: null,
  ...EMPTY(),
  // The opening stopwatch (P1 bench, e2e): selection → body of the
  // last message set. Attachments stay OUTSIDE the stopwatch, as in
  // the pane from before v3.
  lastOpenMs: null,
});

let token = 0;

// R5 (PLAN-RETOURS-12): the last result of `address_names`, key =
// joined addresses. It lives HERE and not in the component: Fil is
// unmounted/remounted on every frame toggle (pane ↔ screen 03), and
// without this cache every toggle went back to RPC for the SAME
// addresses (review). A bare object is enough — the component's
// effect consults it and sets it back.
export const hiddenNames = { key: '', names: {} };

export const msgKey = (m) => `${m.account_id}/${m.mailbox}/${m.uid}`;

// A local echo (PLAN-REACTIVITE E3) is recognized by its synthetic
// mailbox — its body is local (echo_body), never from a thread.
export const isEcho = (m) =>
  typeof m?.mailbox === 'string' && m.mailbox.startsWith('echo:');

// Opens the thread of `newRow` in `frame` — ALWAYS reloaded: the
// first v3's memoization rendered a stale thread (its own reply
// missing after a send) and froze a loading failure.
// Enlarging does NOT go through here: that is `enlargeThread()`, zero
// reload — the frame changes, not the object (D4).
export async function openThread(newRow, frame = 'pane') {
  const t0 = performance.now();
  const mine = ++token;
  thread.frame = frame;
  thread.row = newRow;
  Object.assign(thread, EMPTY());
  // R4: the pin state comes from the SERVED row — exact by
  // construction in the Inbox, the only one to offer the gesture
  // (D4): a row from the flow is NEVER pinned (D5, the core excludes
  // it), a row from the section always is. No round trip to the core
  // on the opening path (review 2026-08-21: a pin_state per
  // opening, in the serialized queue, paid for a button most often
  // absent — and the state lied during the round trip).
  thread.pin = newRow.pinned ?? false;
  // E5: same rule for the pile — a row from an organized view is
  // NEVER set aside (the core excludes it), a card from the pile
  // always is (it carries `aside`). Never a round trip on opening.
  thread.aside = newRow.aside ?? false;
  // V-D2: without a thread — echo included — the MESSAGE ALONE is the thread.
  if (newRow.thread_id == null) {
    thread.messages = [newRow];
    await toggleMessage(newRow, true);
    if (mine === token) thread.lastOpenMs = performance.now() - t0;
    return thread.lastOpenMs;
  }
  try {
    const messages = await call('thread_messages', { threadId: newRow.thread_id });
    if (mine !== token) return thread.lastOpenMs;
    thread.messages = messages;
    const last = messages[messages.length - 1];
    if (last) await toggleMessage(last, true);
  } catch (err) {
    console.error('thread_messages :', err);
  }
  if (mine === token) thread.lastOpenMs = performance.now() - t0;
  return thread.lastOpenMs;
}

// The size change (D4): no reload, no token.
export function enlargeThread() {
  if (thread.row) thread.frame = 'full';
}
// The return: to the pane if the mode has one, otherwise closing.
export function shrinkThread(toPane) {
  if (toPane && thread.row) thread.frame = 'pane';
  else closeThread();
}

// R8' field finding (2026-08-23): “Delete” targets ONE message — the
// open thread removes it and stays in place if any are left. Returns
// the remaining count; 0 = nothing left to show, the caller closes. A
// message that does not belong to the open thread returns -1 (nothing
// is touched).
export function removeMessage(m) {
  const k = msgKey(m);
  if (!thread.messages.some((x) => msgKey(x) === k)) return -1;
  thread.messages = thread.messages.filter((x) => msgKey(x) !== k);
  delete thread.expanded[k];
  delete thread.body[k];
  delete thread.attachments[k];
  delete thread.attachmentCount[k];
  delete thread.blockedImages[k];
  delete thread.invitations[k];
  return thread.messages.length;
}

export function closeThread() {
  token += 1;
  thread.frame = null;
  thread.row = null;
  Object.assign(thread, EMPTY());
  thread.lastOpenMs = null;
}

async function loadMessage(m, withImages = false) {
  const k = msgKey(m);
  const mine = token;
  if (thread.body[k] === undefined || withImages) {
    if (thread.body[k] === undefined) thread.body[k] = '';
    try {
      const view = isEcho(m)
        ? await call('echo_body', {
            id: Number(m.mailbox.slice(5)),
            showImages: withImages,
          })
        : await call('message_body', {
            accountId: m.account_id,
            mailbox: m.mailbox,
            uid: m.uid,
            showImages: withImages,
          });
      // The opening token guards every write: a late reply
      // (images granted then the selection changed) never overwrites
      // the state of a more recent thread.
      if (mine !== token) return;
      delete thread.errors[k];
      thread.body[k] = view.document;
      thread.blockedImages[k] = withImages ? 0 : view.remote_images_blocked;
      thread.attachmentCount[k] = view.attachment_count;
      // The invitation card travels with the body — same freshness as
      // the attachment count, no extra round trip (review).
      thread.invitations[k] = view.invitation ?? null;
    } catch (err) {
      console.error('message_body :', err);
      // Reloadable: the `''` marker set before the call falls away, the
      // error is stated in the frame (“Retry”).
      if (mine === token && thread.body[k] === '') {
        delete thread.body[k];
        thread.errors[k] = true;
      }
    }
  }
  // Attachment metadata: OUTSIDE the measured path (it arrives
  // after the body, never before), gated on the after-scan count.
  // An echo pulls it from the send log (echo_attachments — name and
  // size only, the bytes are purged): never an “Attachments”
  // heading with nothing underneath (PLAN-RETOURS-5, D2).
  const nb = thread.attachmentCount[k] ?? m.attachment_count;
  if (nb > 0 && thread.attachments[k] === undefined) {
    thread.attachments[k] = [];
    const reading = isEcho(m)
      ? call('echo_attachments', { id: Number(m.mailbox.slice(5)) })
      : call('message_attachments', {
          accountId: m.account_id,
          mailbox: m.mailbox,
          uid: m.uid,
        });
    reading
      .then((fetched) => {
        if (mine === token) thread.attachments[k] = fetched;
      })
      .catch((err) => console.error('message_attachments :', err));
  }
}

export function retry(m) {
  delete thread.errors[msgKey(m)];
  return loadMessage(m);
}

export function toggleMessage(m, value = null) {
  const k = msgKey(m);
  const newValue = value ?? !thread.expanded[k];
  thread.expanded[k] = newValue;
  return newValue ? loadMessage(m) : Promise.resolve();
}

export function allExpand() {
  for (const m of thread.messages) toggleMessage(m, true);
}

// The reverse gesture (field finding A46): EVERYTHING collapses, the
// last one included. The “Expand all”/“Collapse all” toggle is NOT a
// flag: it is DERIVED from the real state of the unfoldings (field
// finding A47 — a single-message thread opens unfolded, the button
// says “Collapse all”; manual unfolds make it follow).
export function allCollapse() {
  for (const m of thread.messages) toggleMessage(m, false);
}

// Remote images: blocked by DEFAULT (the invariant that remains), with
// two EXPLICIT and PERSISTENT exceptions (RETOURS-11, D1 reverses
// A43 “the opt-in does not survive the selection”): per message here,
// per sender below. The write goes out fire-and-forget and the
// reload happens IN THE SAME TURN: the immediate render does not need
// the write (`showImages: true` is enough for the session), the
// core's serialized queue sets the write before any future read, and
// `loadMessage` captures its token on click — an `await` here made
// the anti-race guard vacant (review 2026-08-28). If the write
// fails, the session's images display anyway and the failure is
// stated. A local echo stays out of memory (an ephemeral key by nature).
export function showImages(m) {
  if (!isEcho(m)) {
    call('allow_images_message', {
      accountId: m.account_id,
      mailbox: m.mailbox,
      uid: m.uid,
    }).catch((err) => console.error('allow_images_message :', err));
  }
  return loadMessage(m, true);
}

// D3: “Always show images from this sender” — the address is
// resolved by the CORE from the envelope (the UI never parses an
// address); the rule is global to the workstation and is revoked in
// Settings (D4). It does NOT write a per-message choice: revoking it
// undoes everything. The core's reply is READ: `null` = envelope
// without an address, nothing was written — this must be stated,
// otherwise the button's promise breaks in silence (review 2026-08-28).
export function alwaysShowImages(m) {
  // Never offered on an echo (the template already guards it — belt).
  if (isEcho(m)) return Promise.resolve();
  const mine = token;
  call('allow_images_sender', {
    accountId: m.account_id,
    mailbox: m.mailbox,
    uid: m.uid,
  })
    .then((address) => {
      if (address == null) {
        console.error(
          'allow_images_sender: envelope without address, no rule set',
        );
        return;
      }
      if (mine !== token) return;
      // The rule covers the OTHER messages of the thread whose banner
      // is raised: reload them without opt-in — the core arbitrates,
      // a third party's message re-renders identically.
      for (const other of thread.messages) {
        const ka = msgKey(other);
        if (ka !== msgKey(m) && (thread.blockedImages[ka] ?? 0) > 0) {
          delete thread.body[ka];
          loadMessage(other);
        }
      }
    })
    .catch((err) => console.error('allow_images_sender :', err));
  return loadMessage(m, true);
}
