<script>
  // Compose overlay from the prototype: 860 px, three modes
  // (new / reply / forward), wired to the real flows.
  //
  // Prefills: prototype forms (“Re:” / “Fwd:”, opener
  // “Hello FirstName,”) ; the quote is REAL — `reply_context` /
  // `forward_context` from the core prepare it from the actual body.
  // The attachments of a forward are REAL too (PJ-D4): retrieved from
  // the server, poured into the draft, three states per chip; a reply
  // shows none — mail usage does not transmit the original attachments
  // in a reply, the prototype's chip was lying.
  //
  // Sending goes through the outbox (golden rules: logged BEFORE any
  // network attempt, then flushed) — the prototype's toast says
  // “Message sent.” as soon as it is queued; the visible send incident
  // is the notice slot, debt of switch P5.
  //
  // The v1 autosave is kept under the button: draft saved 2 s after
  // the keystroke, edit conflict (`forked`) NEVER hidden, closing =
  // keeping (content emptied by the user is the only case where
  // closing discards).
  //
  // The formatting bar is REAL (PLAN-COMPOSITION-HTML, R4): the body
  // is a `contenteditable` driven by `execCommand` in legacy mode
  // (`styleWithCSS` off) — its output (b/i/u/strike, font
  // color/face/size, align, lists, blockquote) is word for word the
  // vocabulary the ammonia allowlist keeps. The HTML is SANITIZED on
  // the Rust side at every write (save_draft, queue_send); the
  // fallback text is DERIVED there too — one single authority. Link
  // and Quote are REMOVED from the bar (CE decision D1, strict R4
  // scope). (“Make independent” REMOVED — A53, D2. Cc/Bcc WIRED —
  // A54.) A stated gap: the “From” row shows only the address — the
  // core stores neither a display name nor an account label.
  //
  // A draft reopened then closed WITHOUT A KEYSTROKE comes back
  // BYTE FOR BYTE (the stored values are re-emitted as is, never
  // reread from the DOM): the browser re-serializes a normalized
  // `innerHTML` (styles, entities) — rereading the editor would mark
  // the draft modified on every opening and would re-push a copy to
  // Gmail, the exact churn that the core's “identical content”
  // detection came to kill.
  //
  // “Attach” is REAL (PLAN-PIECES-JOINTES E2): native picker, bytes
  // copied to the draft as soon as the gesture happens (PJ-D1 — the
  // anchor draft is born at the first file), refusal at the cap stated
  // under the row (PJ-D3), removal per chip, total weight. Each
  // gesture returns the draft's epoch and it is ADOPTED: without that,
  // the next autosave would see a phantom conflict and fork the draft.
  import Icon from './Icon.svelte';
  import Editor from './Editor.svelte';
  import { tick } from 'svelte';
  import { call, chooseFiles } from './lib/transport.js';
  import { t } from './lib/text.svelte.js';
  import { whenLong } from './lib/when.js';

  let {
    accounts = [],
    account = null,
    // PLAN-RETOURS-9 (D4): in the composer the selector says
    // “Name — address” — the address stays the functional sending
    // data (value unchanged), the name is only a label.
    names = {},

    onflash = () => {},
    onsent = () => {},
    // Every gesture that changes drafts reports it: the list
    // (folder, mention on the thread) resounds without waiting the 10 s.
    ondraft = () => {},
    // E2 (PLAN-REACTIVITE): the Sent copy just ENTERED the database
    // (report of the targeted poll) — the App reserves list and nav
    // right away, without waiting for the generation probe.
    onmail = () => {},
  } = $props();

  // The “Name — address” format lives in ONE place: the selector
  // option and the fixed single-account text cannot derive from
  // one another (review 2026-08-23).
  const labelFor = (c) =>
    c ? (names[c.account_id] ? `${names[c.account_id]} (${c.email})` : c.email) : '';

  let visible = $state(false);
  let mode = $state('new');
  let sender = $state(null); // { account_id, email }
  let a = $state('');
  // Cc and Bcc (A54): their rows only show on request
  // (`showCc`/`showBcc`) — or automatically if content arrives (draft
  // resumption, “Reply all” that restores the original Cc, D3).
  let cc = $state('');
  let cci = $state('');
  let showCc = $state(false);
  let showBcc = $state(false);
  let ccField = $state(null);
  let bccField = $state(null);
  let subject = $state('');
  // The body's truth lives in the Editor component (Editor.svelte,
  // PLAN-AUDIT-V3 E7): its DOM, never Compose's — `editorRef` below
  // is the sole handle, called through its exported contract
  // (getLoaded/set/isModified/getText/getVersion/focus/focusStart).
  // R1 (PLAN-RETOURS-6): true when the body carries only what WIND
  // has set (the signature) — not one word from the user. Without
  // this marker, every “New message” opened then closed would seed a
  // phantom draft (the signature makes the body non-empty).
  let autoBody = false;
  // The draft's REAL attachments (metadata) — what the composer
  // shows is what the message carries, without exception (PJ-D4).
  let attachments = $state([]);
  // The retrieval of a forward's original attachments: one entry
  // per not-yet-acquired attachment — { index, name, status } with
  // status 'encours' | 'echec'. An entry that succeeds becomes an
  // attachment.
  let retrievals = $state([]);
  // The refusal at the cap, shown under the row; cleared on the
  // next gesture that succeeds (addition accepted or removal).
  let refusal = $state(null);
  let sendInProgress = $state(false);
  // R3 (PLAN-RETOURS-6): the “important” marker — a state of the
  // MESSAGE (saved with the draft, carried by the send log,
  // priority headers on the SMTP side), not a screen state.
  let important = $state(false);
  // R2: the “Send later” card (deadline in local time).
  let showDeferred = $state(false);
  let deferredDate = $state('');
  // The source of a forward (account_id, mailbox, uid) — “Retry”
  // needs to know where to retrieve from.
  let sourceForward = null;
  let replyToMailbox = null;
  let replyToUid = null;
  // $state: the visibility of the “Delete draft” gesture is derived
  // from the existence of a persisted draft (R3) — a bare `let` would
  // not refresh the footer.
  let draftId = $state(null);
  let draftEpoch = null;
  // R3 (PLAN-RETOURS-3, D3): the VOLUNTARY deletion of a draft from
  // the compose window goes through a confirmation — an irreversible
  // act never leaves on the first click (same rule as account removal).
  // “Cancel”, on the other hand, KEEPS: the two gestures are never
  // confused.
  let deleteRequest = $state(false);
  let timer;
  // The save IN FLIGHT: its promise, as long as it runs. Saves are
  // SERIALIZED behind it, and the gestures that decide the draft's
  // fate (close, send) wait for it — without which a save started
  // BEFORE an “empty then close” would resurrect the draft the
  // gesture had just deleted (phantom in the folder, found twice by
  // the e2e suite under load, always at the same gesture).
  let saveFlight = null;
  let token = 0;

  let toField = $state(null);
  let editorRef = $state(null);
  let card = $state(null);

  // Address autocompletion (PLAN-RETOURS-5, D3-D4): the menu follows
  // the active field (To, Cc, Bcc), suggests the contacts directory
  // on the CURRENT segment (after the last comma) and inserts the
  // BARE address (D3 — the name shows, it is not inserted).
  // 150 ms debounce + last-prefix-wins (token): a fast keystroke
  // never bursts into the serialized queue (lesson from
  // PLAN-DEFILEMENT-PROFOND).
  let suggestions = $state([]);
  let suggestedField = $state(null); // 'a' | 'cc' | 'cci' | null
  let suggestedChoice = $state(0);
  let suggestToken = 0;
  let suggestTimer = null;

  const currentSegment = (value) => value.split(',').pop().trim();

  function closeSuggestions() {
    suggestToken += 1;
    clearTimeout(suggestTimer);
    suggestions = [];
    suggestedField = null;
    suggestedChoice = 0;
  }

  function onAddressKeystroke(field, value) {
    scheduleSave();
    const prefix = currentSegment(value);
    clearTimeout(suggestTimer);
    if (prefix.length < 2) {
      closeSuggestions();
      return;
    }
    suggestTimer = setTimeout(async () => {
      const mine = ++suggestToken;
      try {
        const found = await call('complete_addresses', { prefix, limit: 8 });
        if (mine !== suggestToken || !visible) return;
        suggestions = found;
        suggestedField = found.length > 0 ? field : null;
        suggestedChoice = 0;
      } catch (err) {
        console.error('complete_addresses :', err);
      }
    }, 150);
  }

  function insertSuggestion(pick) {
    const field = suggestedField;
    if (!field || !pick) return;
    const values = { a, cc, cci };
    const parts = values[field].split(',');
    parts[parts.length - 1] = ` ${pick.address}`;
    const fresh = parts.join(',').replace(/^ /, '');
    if (field === 'a') a = fresh;
    else if (field === 'cc') cc = fresh;
    else cci = fresh;
    closeSuggestions();
    ({ a: toField, cc: ccField, cci: bccField })[field]?.focus();
    scheduleSave();
  }

  function addressKeyboard(ev) {
    if (!suggestedField || suggestions.length === 0) return;
    if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      suggestedChoice = (suggestedChoice + 1) % suggestions.length;
    } else if (ev.key === 'ArrowUp') {
      ev.preventDefault();
      suggestedChoice = (suggestedChoice - 1 + suggestions.length) % suggestions.length;
    } else if (ev.key === 'Enter' || ev.key === 'Tab') {
      ev.preventDefault();
      insertSuggestion(suggestions[suggestedChoice]);
    } else if (ev.key === 'Escape') {
      // The menu closes, the focus STAYS on the field: we cut off
      // the global Escape (App) that would return the focus to the
      // list.
      ev.stopPropagation();
      closeSuggestions();
    }
  }

  const KICKERS = {
    new: 'compose.new',
    reply: 'action.reply',
    reply_all: 'action.replyAll',
    forward: 'action.forward',
  };
  const COMMANDS = {
    reply: 'reply_context',
    reply_all: 'reply_all_context',
    forward: 'forward_context',
  };

  // Prototype forms, to the letter — the core produces “Re:” / “Fwd:”,
  // the surface speaks the interface's language (“Re :” / “Tr :” in
  // French, "Re:" / "Fwd:" in English — A15, decision L-4).
  const reSubject = (s) => (/^re\s*:/i.test(s ?? '') ? s : t('compose.re', { subject: s ?? '' }));
  const fwdSubject = (s) => (/^(tr|fwd|fw)\s*:/i.test(s ?? '') ? s : t('compose.tr', { subject: s ?? '' }));

  // Mirror of the core's `texte_en_html`: escaped, line breaks
  // preserved — resuming a TEXT draft (and only that) goes through
  // here.
  function textAsHtml(text) {
    const escaped = (text ?? '')
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;');
    return `<div>${escaped.replaceAll('\n', '<br>')}</div>`;
  }

  function accountOf(accountId) {
    const known = accounts.find((c) => c.account_id === accountId);
    return known ? { account_id: known.account_id, email: known.email } : null;
  }

  // Field, 2026-08-21: the signature FOLLOWS the sending account.
  // Changing the “From” reloads the new account's signature — as
  // long as the user has not touched the body (a keystroke already
  // made takes priority, its text is never overwritten). `bodyTemplate`
  // is the recipe set at opening: (signature|null) → body HTML — it
  // refills a reply's opener and quote identically, only the
  // signature changes. Resuming a draft has no template (its text
  // is the sole truth): no recomposition. Dedicated token: a fast
  // account change only sets the LAST signature, and never on a
  // card already closed.
  let bodyTemplate = null;
  // True when the template WITHOUT a signature renders an empty
  // body: the recomposed body then carries ONLY the signature — it
  // counts as empty (anti-churn guard), a quote remains real content.
  let templateAlone = false;
  let signatureToken = 0;
  async function changeSender(email) {
    const chosen = accounts.find((c) => c.email === email);
    if (!chosen) return;
    sender = { account_id: chosen.account_id, email: chosen.email };
    scheduleSave();
    if (editorRef.isModified() || !bodyTemplate) return;
    const mine = ++signatureToken;
    let loaded = null;
    try {
      loaded = await call('signature_get', { accountId: chosen.account_id });
    } catch (err) {
      console.error('signature_get :', err);
    }
    if (mine !== signatureToken || !visible || editorRef.isModified()) return;
    // D4: on reply/forward, the SCOPE of the new account decides.
    const sig = loaded?.html ?? null;
    const applicable = mode === 'new' || loaded?.replies ? sig : null;
    await editorRef.set(bodyTemplate(applicable));
    autoBody = templateAlone && Boolean(applicable);
  }

  export async function open(newMode, source = null) {
    const mine = ++token;
    mode = newMode;
    a = '';
    cc = '';
    cci = '';
    showCc = false;
    showBcc = false;
    subject = '';
    attachments = [];
    retrievals = [];
    refusal = null;
    sourceForward = null;
    replyToMailbox = null;
    replyToUid = null;
    draftId = null;
    draftEpoch = null;
    deleteRequest = false;
    important = false;
    showDeferred = false;
    deferredDate = '';
    autoBody = false;
    // A signature reload in flight (account change from a previous
    // session) must never land on THIS card; the previous card's
    // template dies with it.
    signatureToken += 1;
    bodyTemplate = null;
    templateAlone = false;
    sender = source
      ? accountOf(source.account_id)
      : accountOf(account) ?? (accounts.length > 0 ? accountOf(accounts[0].account_id) : null);
    closeSuggestions();
    visible = true;
    await tick();
    // `Editor.set` also resets the formatting bar's own local state
    // (the color swatch, the selection snapshot) — it would survive
    // the card's closing otherwise, a Range from the previous body
    // coloring a phantom.
    await editorRef.set('');

    // R1: the sending account's signature, read once at opening.
    // A failure means “no signature” — never a block.
    let signature = null;
    if (sender) {
      try {
        signature = await call('signature_get', { accountId: sender.account_id });
      } catch (err) {
        console.error('signature_get :', err);
      }
      if (mine !== token) return;
    }
    const sig = signature?.html ?? null;
    // D4: the “also in replies and forwards” scope is a per-account
    // setting — a new message always carries its signature, a reply
    // only if the account has chosen it.
    const repliesSig = sig && signature.replies ? sig : null;

    if (newMode === 'new') {
      // Two empty lines then the signature: the cursor stays at
      // the top. `autoBody` — closing without a keystroke seeds
      // nothing.
      bodyTemplate = (s) => (s ? `<div><br></div><div><br></div>${s}` : '');
      templateAlone = true;
      if (sig && !editorRef.isModified()) {
        await editorRef.set(bodyTemplate(sig));
        autoBody = true;
      }
    }

    if (newMode !== 'new' && source) {
      const replying = newMode === 'reply' || newMode === 'reply_all';
      try {
        const context = await call(COMMANDS[newMode], {
          accountId: source.account_id,
          mailbox: source.mailbox,
          uid: source.uid,
        });
        if (mine !== token) return;
        subject = replying ? reSubject(source.subject) : fwdSubject(source.subject);
        if (replying) {
          a = context.to;
          // D3: “Reply all” restores the original Cc IN Cc — the row
          // opens on its own if there are any.
          cc = context.cc ?? '';
          if (cc) showCc = true;
          const firstName = (source.sender ?? '').split(' ')[0];
          // The core's rich quote leads with two <br> (the cursor's
          // place); the opener already brings them — without this
          // trim, four empty lines would separate the opener from
          // the quote.
          const quote = context.body_html ?? '';
          // R1/D4: a reply's signature is set BETWEEN the opener and
          // the quote — the usage of mature clients. The TEMPLATE
          // survives the opening: changing the sending account
          // recomposes the same body with the new account's
          // signature (field, 2026-08-21), opener and quote refilled
          // identically.
          const cleanQuote = quote.replace(/^(<br>)+/, '');
          bodyTemplate = firstName
            ? (s) =>
                `${textAsHtml(t('compose.hello', { firstName }))}<div><br></div>${s ? `${s}<div><br></div>` : ''}${cleanQuote}`
            : (s) =>
                s ? `<div><br></div>${s}<div><br></div>${cleanQuote}` : quote;
          const content = bodyTemplate(repliesSig);
          // The keystroke already made TAKES PRIORITY: the context can
          // take seconds (body to retrieve) — overwriting what the
          // user typed in the meantime would be worse than a missing
          // quote.
          if (!editorRef.isModified()) await editorRef.set(content);
          replyToMailbox = source.mailbox;
          replyToUid = source.uid;
        } else {
          // Forward: the signature (if the account's scope puts it
          // in reply/forward) precedes the forwarded block, which
          // brings its own separators. Same recomposable template.
          const block = context.body_html ?? '';
          bodyTemplate = (s) => (s ? `<div><br></div>${s}${block}` : block);
          if (!editorRef.isModified()) await editorRef.set(bodyTemplate(repliesSig));
        }
      } catch (err) {
        if (mine !== token) return;
        if (newMode !== 'reply') {
          // Without a body, a forward would transmit nothing;
          // without the full list, an “all” would send to fewer
          // people than promised (the core rereads it on the
          // server): a clean failure.
          visible = false;
          onflash(
            newMode === 'forward'
              ? t('error.forward', { err })
              : t('error.replyAll', { err }),
          );
          return;
        }
        // Reply without a quote: the core allows it, we still
        // write — the signature too, if the account's scope says so.
        subject = reSubject(source.subject);
        replyToMailbox = source.mailbox;
        replyToUid = source.uid;
        bodyTemplate = (s) => (s ? `<div><br></div><div><br></div>${s}` : '');
        templateAlone = true;
        if (repliesSig && !editorRef.isModified()) {
          await editorRef.set(bodyTemplate(repliesSig));
          autoBody = true;
        }
      }
      // The forward transmits its attachments FOR REAL (PJ-D4): each
      // one is retrieved from the server and poured into the draft —
      // one chip per state. A reply, meanwhile, shows nothing: mail
      // usage has never transmitted the original attachments in a
      // reply, and the prototype's chip was promising a send that
      // did not exist.
      //
      // WITHOUT a guard on `source.attachment_count`: the row carries
      // the count from BEFORE the message was opened — on a message
      // just received, trusting it would silently skip the
      // retrieval, exactly the fault PJ-D4 forbids (CE field,
      // 2026-08-14). Reading the metadata is local: zero attachments
      // = zero cost.
      if (newMode === 'forward') {
        try {
          const fetched = await call('message_attachments', {
            accountId: source.account_id,
            mailbox: source.mailbox,
            uid: source.uid,
          });
          if (mine !== token) return;
          sourceForward = {
            account_id: source.account_id,
            mailbox: source.mailbox,
            uid: source.uid,
          };
          retrievals = fetched.map((attachment) => ({
            index: attachment.index,
            name: attachment.name,
            status: 'encours',
          }));
          // Without await: the keystroke does not wait on the
          // network — the chips change state as each attachment
          // arrives.
          retrieveAll([...retrievals], mine);
        } catch (err) {
          console.error('message_attachments :', err);
        }
      }
    }
    // Top-posting: the cursor is placed ABOVE the quote.
    setTimeout(() => {
      if (mine !== token || !visible) return;
      // NEVER steal a focus already placed in the card: if the
      // user started typing during the opening, the pre-focus no
      // longer has a reason to happen — otherwise their keystroke
      // would move field mid-word (a race seen in e2e: the body
      // ended up landing in the To). The card is held by reference,
      // never by a test attribute selector.
      if (card?.contains(document.activeElement)) return;
      if (a && editorRef) {
        // Top-posting: cursor collapsed at the very start of the
        // body, scrolled into view (Editor.focusStart).
        editorRef.focusStart();
      } else {
        toField?.focus();
      }
    }, 0);
  }

  // Resume a local draft (notice slot, debt §6): the content comes
  // back as is, autosave restarts from ITS epoch — the edit
  // conflict stays covered.
  export async function openDraft(draft) {
    token += 1;
    const mine = token;
    mode = 'new';
    sender = accountOf(draft.account_id);
    a = draft.to;
    // Cc/Bcc come back with the draft (A54) — their row opens if
    // there is content to show.
    cc = draft.cc ?? '';
    cci = draft.bcc ?? '';
    showCc = cc.trim() !== '';
    showBcc = cci.trim() !== '';
    subject = draft.subject;
    attachments = [];
    retrievals = [];
    refusal = null;
    sourceForward = null;
    // The chips come back with the text (PJ-D1): the bytes lived in
    // the draft, not in the composer's session.
    call('draft_attachments', { draftId: draft.id })
      .then((fetched) => {
        if (mine === token) attachments = fetched;
      })
      .catch((err) => console.error('draft_attachments :', err));
    // The mailbox comes back WITH the UID: the chain reply → draft →
    // resume → save must not lose the link to the thread (B-D2).
    replyToMailbox = draft.reply_to_mailbox ?? null;
    replyToUid = draft.reply_to_uid ?? null;
    draftId = draft.id;
    draftEpoch = draft.updated_epoch;
    deleteRequest = false;
    // R3: resuming restores the marker — the state lives in the draft.
    important = draft.important ?? false;
    showDeferred = false;
    deferredDate = '';
    autoBody = false;
    // A resumption has no template: its text is the sole truth —
    // changing account never recomposes anything there.
    signatureToken += 1;
    bodyTemplate = null;
    templateAlone = false;
    closeSuggestions();
    visible = true;
    await tick();
    // Rich draft: its HTML as is. Text draft: converted for the
    // editor (`htmlInitial: null` — without a keystroke it does not
    // become rich). In both cases the anti-churn will re-emit what
    // is stored. `Editor.set` also resets the formatting bar's own
    // local state (color swatch, selection snapshot).
    editorRef.set(draft.body_html ?? textAsHtml(draft.body), {
      initialText: draft.body,
      htmlInitial: draft.body_html ?? null,
    });
    setTimeout(() => {
      // Same guard as `open()`: a focus already placed takes priority.
      if (card?.contains(document.activeElement)) return;
      editorRef?.focus();
    }, 0);
  }

  // A draft without text but with an attachment is NOT empty:
  // closing it keeps it, the drafts contract covers the bytes. The
  // body is judged on its TEXT (`Editor.getText` — no reflow); reading
  // `Editor.getVersion()` makes the function REACTIVE to the body's
  // keystrokes, which Svelte does not see in the Editor's DOM
  // (without it, “Delete draft” only appeared at autosave).
  function empty() {
    void editorRef?.getVersion();
    // R1: a body that WIND alone has set (the signature) without a
    // keystroke from the user counts as empty — otherwise every
    // compose window opened then closed would seed a phantom draft.
    const bodyEmpty =
      (autoBody && !editorRef?.isModified()) || !(editorRef?.getText() ?? '').trim();
    return (
      !a.trim() &&
      !cc.trim() &&
      !cci.trim() &&
      !subject.trim() &&
      bodyEmpty &&
      attachments.length === 0
    );
  }

  // R3: the “Delete draft” gesture only makes sense if there is
  // something to discard — a draft already persisted, or content in
  // progress. On a blank compose window, “Cancel” is enough.
  const canDelete = $derived(draftId !== null || !empty());

  function scheduleSave() {
    clearTimeout(timer);
    timer = setTimeout(saveNow, 2000);
  }

  // The net: a crash only costs the last two seconds of typing.
  // Returns the report, or null if there was nothing to do.
  // Only one save at a time: each turn leaves behind the previous
  // flight, and `saveFlight` always carries the last turn.
  function saveNow() {
    clearTimeout(timer);
    const turn = (saveFlight ?? Promise.resolve()).then(saveAlone);
    saveFlight = turn;
    turn.finally(() => {
      if (saveFlight === turn) saveFlight = null;
    });
    return turn;
  }

  async function saveAlone() {
    if (!visible || empty() || !sender) return null;
    try {
      const { body, bodyHtml } = editorRef.getLoaded();
      const report = await call('save_draft', {
        accountId: sender.account_id,
        id: draftId,
        baseEpoch: draftEpoch,
        content: {
          to: a,
          cc,
          bcc: cci,
          subject: subject,
          body,
          bodyHtml,
          replyToUid,
          replyToMailbox,
          important,
        },
      });
      if (!visible) {
        // The panel closed during the save (a send left): do not
        // resurrect a draft already settled.
        await call('delete_draft', { id: report.id })
          .catch((err) => console.error('delete_draft (panel closed during the save):', err));
        ondraft();
        return null;
      }
      draftId = report.id;
      draftEpoch = report.updated_epoch;
      if (report.forked) {
        // NEVER hide this case: two texts now exist, only the user
        // can decide.
        onflash(t('toast.draftFork'));
      }
      ondraft();
      return report;
    } catch {
      // The next keystroke will retry — the net does not alarm for
      // nothing.
    }
    return null;
  }

  export function isOpen() {
    return visible;
  }

  // Closing = keeping: non-empty content becomes (or stays) a
  // draft; a draft emptied of its text is discarded — this is the
  // only case where closing deletes, and it is the user who erased.
  export async function close() {
    if (!visible) return;
    clearTimeout(timer);
    closeSuggestions();
    // The save in flight first: it can carry content from BEFORE
    // the emptying and resurrect what the gesture deletes — the
    // draft's fate is decided on still ground, never while a write
    // is running.
    if (saveFlight) await saveFlight;
    if (empty()) {
      if (draftId !== null) {
        await call('delete_draft', { id: draftId })
          .catch((err) => console.error('delete_draft (draft emptied):', err));
        ondraft();
      }
      visible = false;
      return;
    }
    const report = await saveNow();
    visible = false;
    if (!(report && report.forked)) onflash(t('toast.draftSaved'));
    // The mirroring leaves RIGHT AWAY, silently (R1, v1 sequence):
    // offline, the next cycle will retry — nothing to say.
    call('sync_drafts').catch(() => {});
  }

  async function saveDraft() {
    if (empty()) return;
    await close();
  }

  // R3 (PLAN-RETOURS-3, D3): DISCARD the current draft, upon
  // confirmation. The opposite of `close()` — which keeps: here we
  // delete the trace in the folder, whatever it contains.
  async function deleteDraft() {
    deleteRequest = false;
    clearTimeout(timer);
    // The still ground of `close()`: a save in flight can carry
    // content from BEFORE the gesture and resurrect what we are
    // deleting — we wait for it, then erase the FINAL id.
    if (saveFlight) await saveFlight;
    // `draftId` may have been set BY the save we just waited on —
    // we read it after, never before.
    const hadDraft = draftId !== null;
    if (hadDraft) {
      await call('delete_draft', { id: draftId })
        .catch((err) => console.error('delete_draft (suppression volontaire) :', err));
      ondraft();
    }
    // No id remains: reopening starts blank again, never on a
    // deleted draft.
    draftId = null;
    draftEpoch = null;
    visible = false;
    // “Deleted” is only said if a draft REALLY existed: on a
    // compose window never saved, there was nothing to delete.
    if (hadDraft) onflash(t('toast.draftDeleted'));
  }

  function send() {
    return sendWith(null);
  }

  // R2: opens the “Send later” card, preset to a round +1 h.
  const formatLocal = (d) =>
    `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` +
    `T${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;

  function openDeferred() {
    const inOneHour = new Date(Date.now() + 3600 * 1000);
    inOneHour.setSeconds(0, 0);
    deferredDate = formatLocal(inOneHour);
    showDeferred = true;
  }

  function scheduleSend() {
    const when = new Date(deferredDate);
    // A past (or unreadable) deadline schedules nothing: the
    // refusal is stated, the card stays open to correct it.
    if (Number.isNaN(when.getTime()) || when.getTime() <= Date.now()) {
      onflash(t('error.deferredPass'));
      return;
    }
    showDeferred = false;
    sendWith(Math.floor(when.getTime() / 1000));
  }

  // `deadline` (epoch seconds): null = immediate send; otherwise the
  // send is LOGGED right away (golden rule) and the flush will only
  // pick it up at the stated time (R2, filter on the core side).
  async function sendWith(deadline) {
    if (sendInProgress) return; // double-clic = un seul envoi
    if (!sender) {
      onflash(t('error.noAccount'));
      return;
    }
    // Some forward attachments are missing (in progress or failed):
    // leaving without them would be a silent omission (PJ-D4).
    // Wait, retry — or give up on one with an explicit gesture
    // (the cross).
    if (retrievals.length > 0) {
      onflash(t('error.attachmentsMissing'));
      return;
    }
    sendInProgress = true;
    // Same rule as closing: the save in flight is settled before
    // leaving — the anchor draft (`draftId`) must be its FINAL id,
    // not the one from before a write still in progress.
    clearTimeout(timer);
    if (saveFlight) await saveFlight;
    try {
      const { body, bodyHtml } = editorRef.getLoaded();
      await call('queue_send', {
        accountId: sender.account_id,
        to: a,
        cc,
        bcc: cci,
        subject: subject.trim(),
        body,
        bodyHtml,
        replyToMailbox,
        replyToUid,
        // The anchor draft: its attachments join the log in the
        // same transaction (PJ-D2).
        draftId: draftId,
        important,
        sendAtEpoch: deadline,
      });
    } catch (err) {
      onflash(t('error.send', { err }));
      return;
    } finally {
      sendInProgress = false;
    }
    // The send is logged: the draft has done its job.
    const rule = draftId;
    clearTimeout(timer);
    visible = false;
    // R2: the toast of a scheduled send states the DEADLINE, never
    // “sent” — nothing has left, the echo will only be born when it
    // does.
    onflash(
      deadline
        ? t('toast.scheduled', { when: whenLong(deadline) })
        : t('toast.sent'),
    );
    if (rule !== null) {
      await call('delete_draft', { id: rule })
        .catch((err) => console.error('delete_draft (after send):', err));
      ondraft();
    }
    if (deadline) {
      // Nothing to flush or reconcile now: the status bar will
      // state the scheduled send (10 s probe), and it is the one
      // that will trigger the flush at the deadline.
      onsent();
      return;
    }
    // Flush in the background; offline, the queue waits — the
    // visible incident is the notice slot (P5). Flush done AND
    // successful: targeted poll of the Sent folder (the copy the
    // server adds must show without waiting for the full cycle —
    // field 0.1.4), launched in PARALLEL with the rest: it retries
    // internally (+5 s, +15 s) if the async copy is not there yet
    // (E2), and neither the drafts mirroring nor the send report
    // has to wait for it. Its report says everything — incidents in
    // the console (the silent `.catch` from field 0.1.5 made the
    // instruction blind: never again), the reported mail reserves
    // the list via `onmail`.
    const sendAccount = sender.account_id;
    call('flush_outbox')
      .then((report) => {
        // A DEFERRED send (offline) has deposited nothing at the
        // server: nothing to reconcile, coming back online takes
        // care of it (R-D3) — and since the send never happened, it
        // has no echo.
        if (report.sent > 0) {
          // E3: the Sent echo is BORN at the flush (transaction of
          // the switch to `sent`) — the copy shows < 1 s, without
          // the server. The after-gesture pass reconciles behind it.
          onmail();
          call('sync_after_gesture', { accountId: sendAccount })
            .then((poll) => {
              for (const incident of poll.errors) {
                console.error('sync_after_gesture :', incident);
              }
              if (poll.fetched > 0 || poll.deleted > 0 || poll.reconciled > 0
                  || poll.swept > 0) {
                onmail();
              }
            })
            .catch((err) => console.error('sync_after_gesture :', err));
        }
        return call('sync_drafts').catch(() => {});
      })
      .catch((err) => console.error('flush_outbox :', err))
      .finally(() => onsent());
  }

  // Same form as the core's `human_size` (decimal point included):
  // the total weight must speak like the chips it sums.
  const KO = 1024;
  const MO = KO * 1024;
  function humanWeight(bytes) {
    if (bytes < KO) return `${bytes} o`;
    if (bytes < MO) return `${Math.round(bytes / KO)} Ko`;
    return `${(bytes / MO).toFixed(1)} Mo`;
  }
  const totalWeight = $derived(attachments.reduce((sum, attachment) => sum + attachment.size, 0));

  async function attach() {
    if (!sender) {
      onflash(t('error.noAccount'));
      return;
    }
    const paths = await chooseFiles().catch((err) => {
      onflash(t('error.attachment', { err }));
      return [];
    });
    if (paths.length === 0) return;
    try {
      const report = await call('attach_files', {
        accountId: sender.account_id,
        draftId: draftId,
        paths: paths,
      });
      // `null`: everything refused with no pre-existing draft —
      // nothing to adopt.
      draftId = report.draft_id ?? draftId;
      // The gesture's epoch, otherwise autosave would see a phantom
      // conflict.
      if (report.updated_epoch != null) draftEpoch = report.updated_epoch;
      attachments = report.attachments;
      refusal =
        report.refused.length > 0
          ? t('compose.attachmentRefused', {
              name: report.refused[0].name,
              remaining: report.refused[0].remaining,
            })
          : null;
      ondraft();
    } catch (err) {
      onflash(t('error.attachment', { err }));
      // A failure along the way may have left some attachments
      // entered: reread rather than guess.
      if (draftId !== null) {
        call('draft_attachments', { draftId: draftId })
          .then((fetched) => {
            attachments = fetched;
          })
          .catch(() => {});
      }
    }
  }

  async function remove(attachment) {
    try {
      const epoch = await call('detach_file', { attachmentId: attachment.id });
      attachments = attachments.filter((p) => p.id !== attachment.id);
      if (epoch != null) draftEpoch = epoch;
      refusal = null;
      ondraft();
    } catch (err) {
      onflash(t('error.attachment', { err }));
    }
  }

  // Retrieves ONE attachment from the original message (PJ-D4).
  // Three outcomes: poured in (it becomes a full chip), refused at
  // the cap (it disappears, the refusal is stated — final), network
  // failure (the chip goes to failed, “Retry” stays).
  async function retrieveOne(entry, mine) {
    try {
      const report = await call('fetch_source_attachment', {
        accountId: sourceForward.account_id,
        mailbox: sourceForward.mailbox,
        uid: sourceForward.uid,
        index: entry.index,
        draftId: draftId,
      });
      if (mine !== token) return;
      draftId = report.draft_id ?? draftId;
      if (report.updated_epoch != null) draftEpoch = report.updated_epoch;
      if (report.attachment) {
        attachments = [...attachments, report.attachment];
        retrievals = retrievals.filter((r) => r.index !== entry.index);
        ondraft();
      } else if (report.refused) {
        retrievals = retrievals.filter((r) => r.index !== entry.index);
        refusal = t('compose.attachmentRefused', {
          name: report.refused.name,
          remaining: report.refused.remaining,
        });
      }
    } catch (err) {
      if (mine !== token) return;
      console.error('fetch_source_attachment :', err);
      retrievals = retrievals.map((r) =>
        r.index === entry.index ? { ...r, status: 'echec' } : r,
      );
    }
  }

  // The sequence is SEQUENTIAL: the first attachment creates the
  // anchor draft, the following ones must know it.
  async function retrieveAll(entries, mine) {
    for (const entry of entries) {
      if (mine !== token) return;
      await retrieveOne(entry, mine);
    }
  }

  function retry(entry) {
    retrievals = retrievals.map((r) =>
      r.index === entry.index ? { ...r, status: 'encours' } : r,
    );
    retrieveOne({ ...entry, status: 'encours' }, token);
  }

  // Giving up on a failed attachment — the EXPLICIT gesture that
  // authorizes a send without it: never a silent omission (PJ-D4).
  function giveUp(entry) {
    retrievals = retrievals.filter((r) => r.index !== entry.index);
  }

</script>

<!-- The suggestions menu (PLAN-RETOURS-5): display name shown,
     BARE address inserted (D3). Only one menu at a time, under the
     active field; `onmousedown` neutralized so the click does not
     take the focus away (the blur would close the menu before the
     click). -->
{#snippet menuSuggestions()}
  <ul class="suggestions" role="listbox" aria-label={t('compose.suggestions')}
      data-testid="compose-suggestions">
    {#each suggestions as pick, i (pick.address)}
      <li role="option" aria-selected={i === suggestedChoice}>
        <button type="button" class="suggestion" class:chosen={i === suggestedChoice}
                data-testid="address-suggestion" tabindex="-1"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => insertSuggestion(pick)}>
          {#if pick.name}<span class="name">{pick.name}</span>{/if}
          <span class="address">{pick.address}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

{#if visible}
  <div class="scrim" data-testid="compose">
    <div class="card" bind:this={card} role="dialog" aria-modal="true" aria-label={t(KICKERS[mode])}>
      <!-- Field A46: the header no longer repeats the subject — the
           Subject field states it, just below. -->
      <div class="head">
        <span class="kicker" data-testid="compose-kicker">{t(KICKERS[mode])}</span>
        <span class="grow"></span>
        <button type="button" class="close" aria-label={t('action.close')} onclick={close}>
          <Icon name="close" /></button>
      </div>
      <div class="fields">
        <div class="rank">
          <span class="label">{t('conv.from')}</span>
          {#if accounts.length > 1}
            <!-- A10: the sending account IS CHOSEN (field verdict) —
                 the prototype froze the row, v1 had the selector. -->
            <select class="value" data-testid="compose-from" aria-label={t('compose.sendingAccount')}
                    value={sender?.email ?? ''}
                    onchange={(e) => changeSender(e.target.value)}>
              {#each accounts as c (c.account_id)}
                <option value={c.email}>{labelFor(c)}</option>
              {/each}
            </select>
          {:else}
            <span class="value" data-testid="compose-from">{labelFor(sender)}</span>
          {/if}
        </div>
        <div class="rank">
          <span class="label">{t('conv.to')}</span>
          <input type="text" bind:this={toField} bind:value={a}
                 oninput={(e) => onAddressKeystroke('a', e.currentTarget.value)}
                 onkeydown={addressKeyboard} onblur={closeSuggestions}
                 placeholder={t('compose.recipient')} data-testid="compose-to">
          {#if suggestedField === 'a'}{@render menuSuggestions()}{/if}
          <!-- A54: Cc/Bcc open their row on request (or automatically
               if content is already there — resumption, “Reply
               all”). -->
          {#if !showCc}
            <button type="button" class="chip" data-testid="compose-cc-button"
                    onclick={() => { showCc = true; setTimeout(() => ccField?.focus(), 0); }}>
              <Icon name="group_add" />{t('compose.cc')}</button>
          {/if}
          {#if !showBcc}
            <button type="button" class="chip" data-testid="compose-bcc-button"
                    onclick={() => { showBcc = true; setTimeout(() => bccField?.focus(), 0); }}>
              <Icon name="visibility_off" />{t('compose.bcc')}</button>
          {/if}
        </div>
        {#if showCc}
          <div class="rank">
            <span class="label">{t('compose.cc')}</span>
            <input type="text" bind:this={ccField} bind:value={cc}
                   oninput={(e) => onAddressKeystroke('cc', e.currentTarget.value)}
                   onkeydown={addressKeyboard} onblur={closeSuggestions}
                   placeholder={t('compose.recipient')} data-testid="compose-cc">
            {#if suggestedField === 'cc'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        {#if showBcc}
          <div class="rank">
            <span class="label">{t('compose.bcc')}</span>
            <input type="text" bind:this={bccField} bind:value={cci}
                   oninput={(e) => onAddressKeystroke('cci', e.currentTarget.value)}
                   onkeydown={addressKeyboard} onblur={closeSuggestions}
                   placeholder={t('compose.recipient')} data-testid="compose-cci">
            {#if suggestedField === 'cci'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        <div class="rank">
          <span class="label">{t('conv.subject')}</span>
          <input type="text" bind:value={subject} oninput={scheduleSave}
                 placeholder={t('compose.subjectPlaceholder')} data-testid="compose-subject">
        </div>
      </div>
      <!-- The rich editor + formatting bar (R4): extracted into
           Editor.svelte (PLAN-AUDIT-V3 E7) — Compose calls it
           through its exported contract, never through its DOM
           refs. The attachments row and the refusal notice are
           passed as its default snippet: they render BETWEEN the
           editor and the formatting bar, exactly where the original
           markup placed them (pixel-identical stacking order). -->
      <Editor bind:this={editorRef}
              important={important}
              onImportantToggle={() => { important = !important; scheduleSave(); }}
              oninput={scheduleSave}>
        {#if attachments.length > 0 || retrievals.length > 0}
          <div class="files" data-testid="compose-attachments">
            {#each attachments as attachment (attachment.id)}
              <span class="attachment" data-testid="attachment-compose">
                <Icon name="description" />
                <span class="name">{attachment.name}</span><span class="size">{attachment.human}</span>
                <button type="button" class="remove" data-testid="attachment-remove"
                        aria-label={t('compose.removeAttachment', { name: attachment.name })}
                        onclick={() => remove(attachment)}>
                  <Icon name="close" /></button>
              </span>
            {/each}
            {#each retrievals as entry (entry.index)}
              {#if entry.status === 'encours'}
                <span class="attachment pending" data-testid="attachment-retrieving">
                  <Icon name="hourglass_empty" />
                  {t('compose.retrieving', { name: entry.name })}</span>
              {:else}
                <span class="attachment failure" data-testid="attachment-failure">
                  <Icon name="description" />
                  <span class="name">{entry.name}</span>
                  <button type="button" class="retry" data-testid="attachment-retry"
                          onclick={() => retry(entry)}>{t('action.retry')}</button>
                  <button type="button" class="remove" data-testid="attachment-give-up"
                          aria-label={t('compose.removeAttachment', { name: entry.name })}
                          onclick={() => giveUp(entry)}>
                    <Icon name="close" /></button>
                </span>
              {/if}
            {/each}
            {#if attachments.length > 0}
              <span class="weight" data-testid="compose-weight">
                {t('compose.totalWeight', { poids: humanWeight(totalWeight) })}</span>
            {/if}
          </div>
        {/if}
        {#if refusal}
          <div class="refusal" data-testid="compose-refusal">
            <Icon name="warning" />{refusal}
          </div>
        {/if}
      </Editor>
      {#if deleteRequest}
        <!-- R3/D3: the confirmation lives IN the footer, in the
             buttons' place — a discarded draft does not come back,
             the gesture states what it does before doing it. -->
        <div class="foot confirmation" data-testid="compose-delete-card">
          <span class="warn-delete">{t('compose.deleteConfirm')}</span>
          <span class="grow"></span>
          <button type="button" class="danger" data-testid="compose-delete-confirm"
                  onclick={deleteDraft}>
            <Icon name="delete" />{t('action.delete')}</button>
          <button type="button" class="cancel" data-testid="compose-delete-cancel"
                  onclick={() => (deleteRequest = false)}>{t('action.cancel')}</button>
        </div>
      {:else}
        <div class="foot">
          <button type="button" class="main" data-testid="compose-send"
                  disabled={sendInProgress} onclick={send}>
            <Icon name="send" />{t('action.send')}</button>
          <!-- R2: “Send later” — the card opens above the footer
               (same idiom as the color swatch), deadline preset to
               +1 h, native date+time control. -->
          <span class="group-deferred">
            <button type="button" data-testid="compose-later"
                    disabled={sendInProgress} onclick={openDeferred}>
              <Icon name="schedule_send" />{t('compose.later')}</button>
            {#if showDeferred}
              <div class="deferred" data-testid="compose-deferred">
                <label class="deferred-label">{t('compose.deferredWhen')}
                  <input type="datetime-local" bind:value={deferredDate}
                         data-testid="compose-deferred-date">
                </label>
                <!-- D1: the local semantics is STATED — never a
                     server promise we do not keep. -->
                <p class="deferred-note">{t('compose.deferredNote')}</p>
                <div class="deferred-actions">
                  <button type="button" class="main" data-testid="compose-deferred-confirm"
                          onclick={scheduleSend}>
                    <Icon name="schedule_send" />{t('compose.schedule')}</button>
                  <button type="button" class="cancel" data-testid="compose-deferred-cancel"
                          onclick={() => (showDeferred = false)}>{t('action.cancel')}</button>
                </div>
              </div>
            {/if}
          </span>
          <button type="button" onclick={attach} data-testid="compose-attach">
            <Icon name="attach_file" />{t('compose.attach')}</button>
          <button type="button" onclick={saveDraft} data-testid="compose-draft">
            <Icon name="drafts" />{t('compose.saveDraft')}</button>
          <span class="grow"></span>
          {#if canDelete}
            <!-- The destructive gesture on the RIGHT, detached from
                 the send cluster (less carelessness), before
                 “Cancel” which, itself, keeps. -->
            <button type="button" class="delete" data-testid="compose-delete"
                    onclick={() => (deleteRequest = true)}>
              <Icon name="delete" />{t('compose.deleteDraft')}</button>
          {/if}
          <button type="button" class="cancel" data-testid="compose-cancel"
                  onclick={close}>{t('action.cancel')}</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* VERBATIM geometry of the prototype's compose overlay. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .card {
    width:860px; max-height:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  /* A66: the header carries the same background as Wind's page
     footer (the status bar — --bg since V3, --panel is dead) — and
     as the formatting bar at the bottom of the card: the card is
     framed top/bottom in the same hue. */
  .head {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
    background:var(--bg);
  }
  .kicker {
    font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600; white-space:nowrap;
  }
  .grow { flex:1; }
  .chip {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); white-space:nowrap;
    flex:none;
  }
  .close {
    height:32px; width:32px; padding:0; display:inline-flex; flex:none;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .close:hover { background:var(--sel); }

  /* A46: the header → “From” gap matches the prototype composer's
     drawing (.ccorps: 6 px), plus the former 18 px. */
  .fields { padding:6px 22px 0; display:flex; flex-direction:column; }
  .rank {
    height:44px; display:flex; align-items:center; gap:14px;
    border-bottom:1px solid var(--border);
    /* The suggestions menu anchors to ITS field's row. */
    position:relative;
  }
  .suggestions {
    position:absolute; top:100%; left:66px; z-index:5;
    min-width:280px; max-width:440px;
    margin:2px 0 0; padding:6px; list-style:none;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:2px;
  }
  .suggestion {
    width:100%; display:flex; align-items:baseline; gap:8px;
    padding:6px 8px; border:none; background:transparent; border-radius:var(--r-control);
    cursor:pointer; font-size:13px; text-align:left; font-family:inherit;
  }
  .suggestion:hover { background:var(--hover); }
  .suggestion.chosen { background:var(--sel); }
  .suggestion .name { color:var(--ink); font-weight:600; white-space:nowrap; }
  .suggestion .address { color:var(--muted); overflow:hidden; text-overflow:ellipsis; }
  .label { width:52px; font-size:13px; color:var(--muted); flex:none; }
  .value { flex:1; font-size:13px; color:var(--ink); }
  select.value {
    border:none; background:transparent; cursor:pointer; padding:0;
    font:inherit; font-size:13px; color:var(--ink); min-width:0;
  }
  select.value option { background:var(--surface); color:var(--ink); }
  .rank input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }

  .files { padding:0 22px 14px; display:flex; gap:10px; flex-wrap:wrap; align-items:center; }

  /* The chip of an attachment to add (mockup §1): name + size +
     removal in the SAME chip — one manipulable object, not two
     reads. Symmetric margins (A33): 12 px on both sides — the
     removal cross does not reduce the margin on its side. */
  .attachment {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); white-space:nowrap;
  }
  .attachment .name { color:var(--ink); }
  .attachment .size { font-size:12px; color:var(--muted); }
  .remove {
    height:22px; width:22px; padding:0; display:inline-flex; align-items:center;
    justify-content:center; color:var(--muted); background:transparent;
    border:none; border-radius:var(--r-control); cursor:pointer;
  }
  .remove:hover { background:var(--sel); color:var(--ink); }
  .remove :global(.ic) { width:13px; height:13px; }
  /* The retrieval states (mockup §3): waiting muted italic, failure
     with an --alert border and “Retry”. */
  .attachment.pending { color:var(--muted); font-style:italic; }
  .attachment.failure { border-color:var(--alert); }
  .attachment.failure .name { color:var(--alert); font-weight:600; }
  .retry {
    height:22px; padding:0 8px; display:inline-flex; align-items:center;
    font-size:12px; font-family:inherit; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border); border-radius:var(--r-control);
    cursor:pointer;
  }
  .retry:hover { background:var(--sel); color:var(--ink); }
  .weight { margin-left:auto; font-size:12.5px; color:var(--muted); white-space:nowrap; }
  .refusal {
    padding:0 22px 14px; font-size:13px; color:var(--alert);
    display:flex; align-items:center; gap:8px;
  }
  .refusal :global(.ic) { width:14px; height:14px; }

  /* `.button-format`'s icon size is shared with Editor.svelte's copy
     of the rule (the formatting bar lives there now) — `.delete`
     alone stays here. */
  .delete :global(.ic) { width:18px; height:18px; }

  .foot {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center; gap:12px;
  }
  /* Field, 2026-08-21: a button's label NEVER wraps onto two lines —
     the footer wraps by whole button if it lacks room. */
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
    white-space:nowrap;
  }
  .foot { flex-wrap:wrap; }
  button:hover { background:var(--sel); }
  .main {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .main:hover { background:var(--accentH); border-color:var(--accentH); }
  .main:disabled { opacity:.6; cursor:default; }
  .cancel {
    margin-left:auto; height:auto; padding:0; border:none;
    background:transparent; font-size:13px; color:var(--muted);
    text-decoration:underline; cursor:pointer;
  }
  .cancel:hover { background:transparent; color:var(--ink2); }
  /* The spring pushes the destructive gesture and “Cancel” to the
     right, separated from the Send/Attach/Save cluster. */
  .grow { flex:1; }
  /* R2: the “Send later” card, above the footer — the same local
     overlay idiom as the color swatch. */
  .group-deferred { position:relative; display:inline-flex; }
  .deferred {
    position:absolute; bottom:40px; left:0; z-index:3; width:320px;
    padding:14px; background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:10px;
  }
  .deferred-label {
    display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); white-space:nowrap;
  }
  .deferred-label input {
    flex:1; min-width:0; height:32px; padding:0 8px; font:inherit;
    font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .deferred-note { margin:0; font-size:12px; color:var(--muted); line-height:1.5; }
  .deferred-actions { display:flex; align-items:center; gap:12px; }

  /* R3: “Delete draft” and its confirmation — alert hue, never the
     accent color (which invites the click). */
  .delete { color:var(--alert); border-color:var(--border); }
  .delete:hover { background:var(--alert); color:var(--onAccent); border-color:var(--alert); }
  .confirmation .warn-delete { font-size:13px; color:var(--alert); font-weight:600; }
  .danger {
    font-weight:600; color:var(--onAccent); background:var(--alert);
    border-color:var(--alert);
  }
  .danger:hover { background:var(--alert); border-color:var(--alert); filter:brightness(1.08); }
</style>
