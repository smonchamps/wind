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
  // The body lives in the `contenteditable`'s DOM (`bodyField`), not in
  // Svelte state. As long as `bodyModified` is false, saving re-emits
  // the INITIAL values (set by `setBody`) — the anti-churn; from the
  // first keystroke, `innerHTML` becomes the truth. `bodyVersion` is
  // the body's reactive pulse: the Svelte derived values do not see
  // the DOM, they see this counter.
  let bodyModified = false;
  let initialBodyText = '';
  let initialBodyHtml = null;
  let bodyVersion = $state(0);
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
  let bodyField = $state(null);
  let bodyArea = $state(null);
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

  const bodyHtml = () => bodyField?.innerHTML ?? '';

  // What saving and sending hand back to Rust. Without a keystroke,
  // the INITIAL values go back byte for byte (anti-churn, all
  // drafts — text AND rich: the browser's re-serialization is never
  // faithful). Modified: the editor's HTML alone — the fallback text
  // is derived on the Rust side (`frontiere_corps`), the `body`
  // passed would be discarded, so it is not computed.
  function bodyLoaded() {
    if (!bodyModified) {
      return { body: initialBodyText, bodyHtml: initialBodyHtml };
    }
    return { body: '', bodyHtml: bodyHtml() };
  }

  // Sets the editor's content. `tick()` first: the node only exists
  // once the overlay is rendered — setting it before would be lost.
  // `htmlInitial: null` = TEXT draft (saving without a keystroke must
  // not convert it); by default, the HTML set is the initial one.
  async function setBody(html, { initialText = '', htmlInitial = html } = {}) {
    bodyModified = false;
    initialBodyText = initialText;
    initialBodyHtml = htmlInitial;
    await tick();
    if (bodyField) bodyField.innerHTML = html;
    bodyVersion += 1;
  }

  function onBodyKeystroke() {
    // Chromium leaves an orphan <br> after “select all then
    // delete”: the body is empty but no longer `:empty` — without
    // this renormalization, the placeholder would never come back.
    if (bodyField && !bodyField.textContent && bodyField.innerHTML !== '') {
      bodyField.innerHTML = '';
    }
    bodyModified = true;
    bodyVersion += 1;
    scheduleSave();
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
    if (bodyModified || !bodyTemplate) return;
    const mine = ++signatureToken;
    let loaded = null;
    try {
      loaded = await call('signature_get', { accountId: chosen.account_id });
    } catch (err) {
      console.error('signature_get :', err);
    }
    if (mine !== signatureToken || !visible || bodyModified) return;
    // D4: on reply/forward, the SCOPE of the new account decides.
    const sig = loaded?.html ?? null;
    const applicable = mode === 'new' || loaded?.replies ? sig : null;
    await setBody(bodyTemplate(applicable));
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
    // The color swatch and the selection snapshot are MODULE states:
    // they would survive the card's closing — a Range from the
    // previous body would color a phantom.
    showColors = false;
    bodySelection = null;
    sender = source
      ? accountOf(source.account_id)
      : accountOf(account) ?? (accounts.length > 0 ? accountOf(accounts[0].account_id) : null);
    closeSuggestions();
    visible = true;
    await setBody('');

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
      if (sig && !bodyModified) {
        await setBody(bodyTemplate(sig));
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
          if (!bodyModified) await setBody(content);
          replyToMailbox = source.mailbox;
          replyToUid = source.uid;
        } else {
          // Forward: the signature (if the account's scope puts it
          // in reply/forward) precedes the forwarded block, which
          // brings its own separators. Same recomposable template.
          const block = context.body_html ?? '';
          bodyTemplate = (s) => (s ? `<div><br></div>${s}${block}` : block);
          if (!bodyModified) await setBody(bodyTemplate(repliesSig));
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
        if (repliesSig && !bodyModified) {
          await setBody(bodyTemplate(repliesSig));
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
      if (a && bodyField) {
        bodyField.focus();
        // The contenteditable equivalent of the textarea's
        // `setSelectionRange(0, 0)`: a Range collapsed at the very
        // start of the body.
        const selection = window.getSelection();
        const range = document.createRange();
        range.setStart(bodyField, 0);
        range.collapse(true);
        selection.removeAllRanges();
        selection.addRange(range);
        // Focus may have scrolled to the end caret before being
        // reset to 0 — and the SCROLL CONTAINER is `.zone-corps`,
        // not the editor (the editor's scrollTop is always 0): the
        // opener must be VISIBLE, not merely first.
        if (bodyArea) bodyArea.scrollTop = 0;
      } else {
        toField?.focus();
      }
    }, 0);
  }

  // Resume a local draft (notice slot, debt §6): the content comes
  // back as is, autosave restarts from ITS epoch — the edit
  // conflict stays covered.
  export function openDraft(draft) {
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
    // Rich draft: its HTML as is. Text draft: converted for the
    // editor (`htmlInitial: null` — without a keystroke it does not
    // become rich). In both cases the anti-churn will re-emit what
    // is stored.
    setBody(draft.body_html ?? textAsHtml(draft.body), {
      initialText: draft.body,
      htmlInitial: draft.body_html ?? null,
    });
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
    showColors = false;
    bodySelection = null;
    closeSuggestions();
    visible = true;
    setTimeout(() => {
      // Same guard as `open()`: a focus already placed takes priority.
      if (card?.contains(document.activeElement)) return;
      bodyField?.focus();
    }, 0);
  }

  // A draft without text but with an attachment is NOT empty:
  // closing it keeps it, the drafts contract covers the bytes. The
  // body is judged on its TEXT (`textContent` — no reflow); reading
  // `bodyVersion` makes the function REACTIVE to the body's
  // keystrokes, which Svelte does not see in the DOM (without it,
  // “Delete draft” only appeared at autosave).
  function empty() {
    void bodyVersion;
    // R1: a body that WIND alone has set (the signature) without a
    // keystroke from the user counts as empty — otherwise every
    // compose window opened then closed would seed a phantom draft.
    const bodyEmpty =
      (autoBody && !bodyModified) || !(bodyField?.textContent ?? '').trim();
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
      const { body, bodyHtml } = bodyLoaded();
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
      const { body, bodyHtml } = bodyLoaded();
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

  // --- The formatting bar (R4, CE decisions D1-D3) --------------------
  //
  // `execCommand` in legacy mode: `styleWithCSS` turned off at every
  // gesture, so the output (<b>, <font>, align…) stays the exact
  // vocabulary of the ammonia allowlist — never a generated CSS
  // style to translate.
  //
  // The selection SURVIVES the controls that take focus (the
  // Font/Size <select>s): captured on every `selectionchange` in
  // the body, restored before every command.
  let bodySelection = null;
  let activeFormats = $state({});
  // D3: fixed color swatch — twelve safe hues on the body's light
  // slate (mail is composed for a white background, A61).
  const COLORS = [
    '#000000',
    '#666666',
    '#cc0000',
    '#e69138',
    '#bf9000',
    '#38761d',
    '#45818e',
    '#3d85c6',
    '#1155cc',
    '#674ea7',
    '#a64d79',
    '#85200c',
  ];
  let showColors = $state(false);

  function onSelection() {
    if (!visible || !bodyField) return;
    const selection = window.getSelection();
    if (selection.rangeCount > 0 && bodyField.contains(selection.anchorNode)) {
      bodySelection = selection.getRangeAt(0).cloneRange();
      activeUpdates();
    }
  }

  function activeUpdates() {
    activeFormats = {
      bold: document.queryCommandState('bold'),
      italic: document.queryCommandState('italic'),
      underline: document.queryCommandState('underline'),
      strikethrough: document.queryCommandState('strikeThrough'),
      bulletList: document.queryCommandState('insertUnorderedList'),
      numberedList: document.queryCommandState('insertOrderedList'),
    };
  }

  function command(name, value = null) {
    if (!bodyField) return;
    bodyField.focus();
    if (bodySelection) {
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(bodySelection);
    }
    document.execCommand('styleWithCSS', false, false);
    document.execCommand(name, false, value);
    bodyModified = true;
    showColors = false;
    activeUpdates();
    scheduleSave();
  }

  // The <select>s return to their label after the gesture: they
  // are COMMANDS (apply a font to the selection), not states — a
  // mixed selection has no single font to show.
  function selectCommand(event, name) {
    const value = event.target.value;
    event.target.value = '';
    if (value) command(name, value);
  }
</script>

<svelte:document onselectionchange={onSelection} />

<!-- The suggestions menu (PLAN-RETOURS-5): display name shown,
     BARE address inserted (D3). Only one menu at a time, under the
     active field; `onmousedown` neutralized so the click does not
     take the focus away (the blur would close the menu before the
     click). -->
{#snippet menuSuggestions()}
  <ul class="suggestions" role="listbox" aria-label={t('compose.suggestions')}
      data-testid="composition-suggestions">
    {#each suggestions as pick, i (pick.address)}
      <li role="option" aria-selected={i === suggestedChoice}>
        <button type="button" class="suggestion" class:choisie={i === suggestedChoice}
                data-testid="suggestion-adresse" tabindex="-1"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => insertSuggestion(pick)}>
          {#if pick.name}<span class="nom">{pick.name}</span>{/if}
          <span class="adresse">{pick.address}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

{#if visible}
  <div class="scrim" data-testid="composition">
    <div class="carte" bind:this={card} role="dialog" aria-modal="true" aria-label={t(KICKERS[mode])}>
      <!-- Field A46: the header no longer repeats the subject — the
           Subject field states it, just below. -->
      <div class="tete">
        <span class="kicker" data-testid="composition-kicker">{t(KICKERS[mode])}</span>
        <span class="essor"></span>
        <button type="button" class="fermer" aria-label={t('action.close')} onclick={close}>
          <Icon name="close" /></button>
      </div>
      <div class="champs">
        <div class="rang">
          <span class="etiquette">{t('conv.from')}</span>
          {#if accounts.length > 1}
            <!-- A10: the sending account IS CHOSEN (field verdict) —
                 the prototype froze the row, v1 had the selector. -->
            <select class="valeur" data-testid="composition-de" aria-label={t('compose.sendingAccount')}
                    value={sender?.email ?? ''}
                    onchange={(e) => changeSender(e.target.value)}>
              {#each accounts as c (c.account_id)}
                <option value={c.email}>{labelFor(c)}</option>
              {/each}
            </select>
          {:else}
            <span class="valeur" data-testid="composition-de">{labelFor(sender)}</span>
          {/if}
        </div>
        <div class="rang">
          <span class="etiquette">{t('conv.to')}</span>
          <input type="text" bind:this={toField} bind:value={a}
                 oninput={(e) => onAddressKeystroke('a', e.currentTarget.value)}
                 onkeydown={addressKeyboard} onblur={closeSuggestions}
                 placeholder={t('compose.recipient')} data-testid="composition-a">
          {#if suggestedField === 'a'}{@render menuSuggestions()}{/if}
          <!-- A54: Cc/Bcc open their row on request (or automatically
               if content is already there — resumption, “Reply
               all”). -->
          {#if !showCc}
            <button type="button" class="puce" data-testid="composition-bouton-cc"
                    onclick={() => { showCc = true; setTimeout(() => ccField?.focus(), 0); }}>
              <Icon name="group_add" />{t('compose.cc')}</button>
          {/if}
          {#if !showBcc}
            <button type="button" class="puce" data-testid="composition-bouton-cci"
                    onclick={() => { showBcc = true; setTimeout(() => bccField?.focus(), 0); }}>
              <Icon name="visibility_off" />{t('compose.bcc')}</button>
          {/if}
        </div>
        {#if showCc}
          <div class="rang">
            <span class="etiquette">{t('compose.cc')}</span>
            <input type="text" bind:this={ccField} bind:value={cc}
                   oninput={(e) => onAddressKeystroke('cc', e.currentTarget.value)}
                   onkeydown={addressKeyboard} onblur={closeSuggestions}
                   placeholder={t('compose.recipient')} data-testid="composition-cc">
            {#if suggestedField === 'cc'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        {#if showBcc}
          <div class="rang">
            <span class="etiquette">{t('compose.bcc')}</span>
            <input type="text" bind:this={bccField} bind:value={cci}
                   oninput={(e) => onAddressKeystroke('cci', e.currentTarget.value)}
                   onkeydown={addressKeyboard} onblur={closeSuggestions}
                   placeholder={t('compose.recipient')} data-testid="composition-cci">
            {#if suggestedField === 'cci'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        <div class="rang">
          <span class="etiquette">{t('conv.subject')}</span>
          <input type="text" bind:value={subject} oninput={scheduleSave}
                 placeholder={t('compose.subjectPlaceholder')} data-testid="composition-objet">
        </div>
      </div>
      <div class="zone-corps" bind:this={bodyArea}>
        <!-- The rich editor (R4): contenteditable, content set by
             `setBody`, read by `bodyLoaded` — never a bind. The
             placeholder lives in CSS (:empty::before). The selection
             is tracked only by the document's `selectionchange` (it
             covers KEYBOARD AND mouse — no onkeyup/onmouseup
             duplicate). -->
        <div class="corps-editeur" contenteditable="true" role="textbox" aria-multiline="true"
             tabindex="0"
             bind:this={bodyField} oninput={onBodyKeystroke}
             data-placeholder={t('compose.bodyPlaceholder')}
             aria-label={t('compose.bodyPlaceholder')}
             data-testid="composition-corps"></div>
      </div>
      {#if attachments.length > 0 || retrievals.length > 0}
        <div class="fichiers" data-testid="composition-pieces">
          {#each attachments as attachment (attachment.id)}
            <span class="piece" data-testid="piece-compo">
              <Icon name="description" />
              <span class="nom">{attachment.name}</span><span class="taille">{attachment.human}</span>
              <button type="button" class="retrait" data-testid="piece-retrait"
                      aria-label={t('compose.removeAttachment', { name: attachment.name })}
                      onclick={() => remove(attachment)}>
                <Icon name="close" /></button>
            </span>
          {/each}
          {#each retrievals as entry (entry.index)}
            {#if entry.status === 'encours'}
              <span class="piece attente" data-testid="piece-rapatriement">
                <Icon name="hourglass_empty" />
                {t('compose.retrieving', { name: entry.name })}</span>
            {:else}
              <span class="piece echec" data-testid="piece-echec">
                <Icon name="description" />
                <span class="nom">{entry.name}</span>
                <button type="button" class="reessayer" data-testid="piece-reessayer"
                        onclick={() => retry(entry)}>{t('action.retry')}</button>
                <button type="button" class="retrait" data-testid="piece-renoncer"
                        aria-label={t('compose.removeAttachment', { name: entry.name })}
                        onclick={() => giveUp(entry)}>
                  <Icon name="close" /></button>
              </span>
            {/if}
          {/each}
          {#if attachments.length > 0}
            <span class="poids" data-testid="composition-poids">
              {t('compose.totalWeight', { poids: humanWeight(totalWeight) })}</span>
          {/if}
        </div>
      {/if}
      {#if refusal}
        <div class="refus" data-testid="composition-refus">
          <Icon name="warning" />{refusal}
        </div>
      {/if}
      <!-- The REAL bar (R4, D1: exactly the requested buttons — Link
           and Quote removed). `onmousedown` neutralized everywhere:
           a format button never steals the body's selection. -->
      <div class="format" data-testid="composition-format">
        <select class="select-format" aria-label={t('compose.font')} title={t('compose.font')}
                data-testid="composition-format-police"
                onchange={(e) => selectCommand(e, 'fontName')}>
          <option value="" disabled selected hidden>{t('compose.font')}</option>
          <option value="sans-serif">{t('compose.fontSans')}</option>
          <option value="serif">{t('compose.fontSerif')}</option>
          <option value="monospace">{t('compose.fontMono')}</option>
        </select>
        <select class="select-format" aria-label={t('compose.size')} title={t('compose.size')}
                data-testid="composition-format-taille"
                onchange={(e) => selectCommand(e, 'fontSize')}>
          <option value="" disabled selected hidden>{t('compose.size')}</option>
          <option value="2">{t('compose.sizeSmall')}</option>
          <option value="3">{t('compose.sizeNormal')}</option>
          <option value="4">{t('compose.sizeLarge')}</option>
          <option value="6">{t('compose.sizeVeryLarge')}</option>
        </select>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format" class:actif={activeFormats.bold}
                aria-label={t('compose.bold')} title={t('compose.bold')} aria-pressed={activeFormats.bold}
                data-testid="composition-format-gras"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('bold')}>
          <Icon name="format_bold" /></button>
        <button type="button" class="bouton-format" class:actif={activeFormats.italic}
                aria-label={t('compose.italic')} title={t('compose.italic')} aria-pressed={activeFormats.italic}
                data-testid="composition-format-italique"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('italic')}>
          <Icon name="format_italic" /></button>
        <button type="button" class="bouton-format" class:actif={activeFormats.underline}
                aria-label={t('compose.underline')} title={t('compose.underline')} aria-pressed={activeFormats.underline}
                data-testid="composition-format-souligne"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('underline')}>
          <Icon name="format_underlined" /></button>
        <button type="button" class="bouton-format" class:actif={activeFormats.strikethrough}
                aria-label={t('compose.strikethrough')} title={t('compose.strikethrough')} aria-pressed={activeFormats.strikethrough}
                data-testid="composition-format-barre"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('strikeThrough')}>
          <Icon name="strikethrough_s" /></button>
        <span class="groupe-couleur">
          <button type="button" class="bouton-format"
                  aria-label={t('compose.color')} title={t('compose.color')}
                  data-testid="composition-format-couleur"
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => (showColors = !showColors)}>
            <Icon name="format_color_text" /></button>
          {#if showColors}
            <div class="palette" data-testid="composition-palette">
              {#each COLORS as color (color)}
                <button type="button" class="teinte" style="background:{color}"
                        aria-label={color}
                        onmousedown={(e) => e.preventDefault()}
                        onclick={() => command('foreColor', color)}></button>
              {/each}
            </div>
          {/if}
        </span>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format"
                aria-label={t('compose.alignLeft')} title={t('compose.alignLeft')}
                data-testid="composition-format-gauche"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyLeft')}>
          <Icon name="format_align_left" /></button>
        <button type="button" class="bouton-format"
                aria-label={t('compose.alignCenter')} title={t('compose.alignCenter')}
                data-testid="composition-format-centre"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyCenter')}>
          <Icon name="format_align_center" /></button>
        <button type="button" class="bouton-format"
                aria-label={t('compose.alignRight')} title={t('compose.alignRight')}
                data-testid="composition-format-droite"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyRight')}>
          <Icon name="format_align_right" /></button>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format" class:actif={activeFormats.bulletList}
                aria-label={t('compose.listBullets')} title={t('compose.listBullets')} aria-pressed={activeFormats.bulletList}
                data-testid="composition-format-puces"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('insertUnorderedList')}>
          <Icon name="format_list_bulleted" /></button>
        <button type="button" class="bouton-format" class:actif={activeFormats.numberedList}
                aria-label={t('compose.listNumbered')} title={t('compose.listNumbered')} aria-pressed={activeFormats.numberedList}
                data-testid="composition-format-numerotee"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('insertOrderedList')}>
          <Icon name="format_list_numbered" /></button>
        <button type="button" class="bouton-format"
                aria-label={t('compose.indentLess')} title={t('compose.indentLess')}
                data-testid="composition-format-retrait-moins"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('outdent')}>
          <Icon name="format_indent_decrease" /></button>
        <button type="button" class="bouton-format"
                aria-label={t('compose.indentMore')} title={t('compose.indentMore')}
                data-testid="composition-format-retrait-plus"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('indent')}>
          <Icon name="format_indent_increase" /></button>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format"
                aria-label={t('compose.clearFormat')} title={t('compose.clearFormat')}
                data-testid="composition-format-effacer"
                onmousedown={(e) => e.preventDefault()} onclick={() => command('removeFormat')}>
          <Icon name="format_clear" /></button>
        <span class="sep" aria-hidden="true"></span>
        <!-- R3 (field, 2026-08-21): “Important” lives IN the
             formatting bar, in the format of its neighbors (icon
             only) — a toggle of the message's state (aria-pressed),
             not an action. -->
        <button type="button" class="bouton-format" class:actif={important}
                aria-label={t('compose.importantTitle')} title={t('compose.importantTitle')}
                aria-pressed={important} data-testid="composition-important"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => { important = !important; scheduleSave(); }}>
          <Icon name="priority_high" /></button>
      </div>
      {#if deleteRequest}
        <!-- R3/D3: the confirmation lives IN the footer, in the
             buttons' place — a discarded draft does not come back,
             the gesture states what it does before doing it. -->
        <div class="pied confirmation" data-testid="composition-suppr-carte">
          <span class="avert-suppr">{t('compose.deleteConfirm')}</span>
          <span class="essor"></span>
          <button type="button" class="danger" data-testid="composition-suppr-confirmer"
                  onclick={deleteDraft}>
            <Icon name="delete" />{t('action.delete')}</button>
          <button type="button" class="annuler" data-testid="composition-suppr-annuler"
                  onclick={() => (deleteRequest = false)}>{t('action.cancel')}</button>
        </div>
      {:else}
        <div class="pied">
          <button type="button" class="principal" data-testid="composition-envoyer"
                  disabled={sendInProgress} onclick={send}>
            <Icon name="send" />{t('action.send')}</button>
          <!-- R2: “Send later” — the card opens above the footer
               (same idiom as the color swatch), deadline preset to
               +1 h, native date+time control. -->
          <span class="groupe-differe">
            <button type="button" data-testid="composition-plus-tard"
                    disabled={sendInProgress} onclick={openDeferred}>
              <Icon name="schedule_send" />{t('compose.later')}</button>
            {#if showDeferred}
              <div class="differe" data-testid="composition-differe">
                <label class="differe-label">{t('compose.deferredWhen')}
                  <input type="datetime-local" bind:value={deferredDate}
                         data-testid="composition-differe-date">
                </label>
                <!-- D1: the local semantics is STATED — never a
                     server promise we do not keep. -->
                <p class="differe-note">{t('compose.deferredNote')}</p>
                <div class="differe-actions">
                  <button type="button" class="principal" data-testid="composition-differe-confirmer"
                          onclick={scheduleSend}>
                    <Icon name="schedule_send" />{t('compose.schedule')}</button>
                  <button type="button" class="annuler" data-testid="composition-differe-annuler"
                          onclick={() => (showDeferred = false)}>{t('action.cancel')}</button>
                </div>
              </div>
            {/if}
          </span>
          <button type="button" onclick={attach} data-testid="composition-joindre">
            <Icon name="attach_file" />{t('compose.attach')}</button>
          <button type="button" onclick={saveDraft} data-testid="composition-brouillon">
            <Icon name="drafts" />{t('compose.saveDraft')}</button>
          <span class="essor"></span>
          {#if canDelete}
            <!-- The destructive gesture on the RIGHT, detached from
                 the send cluster (less carelessness), before
                 “Cancel” which, itself, keeps. -->
            <button type="button" class="supprimer" data-testid="composition-supprimer"
                    onclick={() => (deleteRequest = true)}>
              <Icon name="delete" />{t('compose.deleteDraft')}</button>
          {/if}
          <button type="button" class="annuler" data-testid="composition-annuler"
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
  .carte {
    width:860px; max-height:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  /* A66: the header carries the same background as Wind's page
     footer (the status bar — --bg since V3, --panel is dead) — and
     as the formatting bar at the bottom of the card: the card is
     framed top/bottom in the same hue. */
  .tete {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
    background:var(--bg);
  }
  .kicker {
    font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600; white-space:nowrap;
  }
  .essor { flex:1; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); white-space:nowrap;
    flex:none;
  }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex; flex:none;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }

  /* A46: the header → “From” gap matches the prototype composer's
     drawing (.ccorps: 6 px), plus the former 18 px. */
  .champs { padding:6px 22px 0; display:flex; flex-direction:column; }
  .rang {
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
    border-radius:var(--r-controle); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:2px;
  }
  .suggestion {
    width:100%; display:flex; align-items:baseline; gap:8px;
    padding:6px 8px; border:none; background:transparent; border-radius:var(--r-controle);
    cursor:pointer; font-size:13px; text-align:left; font-family:inherit;
  }
  .suggestion:hover { background:var(--hover); }
  .suggestion.choisie { background:var(--sel); }
  .suggestion .nom { color:var(--ink); font-weight:600; white-space:nowrap; }
  .suggestion .adresse { color:var(--muted); overflow:hidden; text-overflow:ellipsis; }
  .etiquette { width:52px; font-size:13px; color:var(--muted); flex:none; }
  .valeur { flex:1; font-size:13px; color:var(--ink); }
  select.valeur {
    border:none; background:transparent; cursor:pointer; padding:0;
    font:inherit; font-size:13px; color:var(--ink); min-width:0;
  }
  select.valeur option { background:var(--surface); color:var(--ink); }
  .rang input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }

  .zone-corps {
    padding:20px 22px; display:flex; flex-direction:column;
    min-height:220px; flex:1; overflow:auto;
  }
  .corps-editeur {
    flex:1; width:100%; min-height:180px; font-size:15px; line-height:1.65;
    color:var(--ink); border:none; outline:none;
    background:transparent; font-family:inherit;
    overflow-wrap:break-word;
  }
  /* The textarea's placeholder, redone: visible as long as the body
     is empty, in the muted hue. */
  .corps-editeur:empty::before {
    content:attr(data-placeholder); color:var(--muted); pointer-events:none;
  }
  /* The rich quote: the left net that `quote_reply_html` sets as an
     inline style is the reference; this only styles the blockquotes
     born from the indent, with no style of its own. */
  .corps-editeur :global(blockquote) { margin:0 0 0 0.8ex; }

  .fichiers { padding:0 22px 14px; display:flex; gap:10px; flex-wrap:wrap; align-items:center; }

  /* The chip of an attachment to add (mockup §1): name + size +
     removal in the SAME chip — one manipulable object, not two
     reads. Symmetric margins (A33): 12 px on both sides — the
     removal cross does not reduce the margin on its side. */
  .piece {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); white-space:nowrap;
  }
  .piece .nom { color:var(--ink); }
  .piece .taille { font-size:12px; color:var(--muted); }
  .retrait {
    height:22px; width:22px; padding:0; display:inline-flex; align-items:center;
    justify-content:center; color:var(--muted); background:transparent;
    border:none; border-radius:var(--r-controle); cursor:pointer;
  }
  .retrait:hover { background:var(--sel); color:var(--ink); }
  .retrait :global(.ic) { width:13px; height:13px; }
  /* The retrieval states (mockup §3): waiting muted italic, failure
     with an --alert border and “Retry”. */
  .piece.attente { color:var(--muted); font-style:italic; }
  .piece.echec { border-color:var(--alert); }
  .piece.echec .nom { color:var(--alert); font-weight:600; }
  .reessayer {
    height:22px; padding:0 8px; display:inline-flex; align-items:center;
    font-size:12px; font-family:inherit; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border); border-radius:var(--r-controle);
    cursor:pointer;
  }
  .reessayer:hover { background:var(--sel); color:var(--ink); }
  .poids { margin-left:auto; font-size:12.5px; color:var(--muted); white-space:nowrap; }
  .refus {
    padding:0 22px 14px; font-size:13px; color:var(--alert);
    display:flex; align-items:center; gap:8px;
  }
  .refus :global(.ic) { width:14px; height:14px; }

  .format {
    flex:none; padding:8px 18px; border-top:1px solid var(--border);
    background:var(--bg); display:flex; align-items:center; gap:6px;
    flex-wrap:wrap;
  }
  .bouton-format {
    height:32px; min-width:32px; padding:0 6px; display:inline-flex;
    align-items:center; justify-content:center; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .bouton-format:hover { background:var(--sel); color:var(--ink); }
  /* The active state states what the selection carries (aria-pressed
     likewise). */
  .bouton-format.actif {
    background:var(--sel); color:var(--accent); border-color:var(--accent);
  }
  .bouton-format :global(.ic), .supprimer :global(.ic) { width:18px; height:18px; }
  .select-format {
    height:32px; padding:0 8px; font:inherit; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .select-format option { background:var(--surface); color:var(--ink); }
  .sep {
    width:1px; height:20px; background:var(--border); flex:none;
    margin:0 4px;
  }
  /* The color swatch (D3): twelve fixed hues, above the bar. */
  .groupe-couleur { position:relative; display:inline-flex; }
  .palette {
    position:absolute; bottom:38px; left:0; z-index:1;
    display:grid; grid-template-columns:repeat(6, 22px); gap:6px;
    padding:10px; background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
    box-shadow:var(--shadow);
  }
  .teinte {
    height:22px; width:22px; min-width:0; padding:0;
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  .teinte:hover { outline:2px solid var(--accent); outline-offset:1px; }

  .pied {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center; gap:12px;
  }
  /* Field, 2026-08-21: a button's label NEVER wraps onto two lines —
     the footer wraps by whole button if it lacks room. */
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
    white-space:nowrap;
  }
  .pied { flex-wrap:wrap; }
  button:hover { background:var(--sel); }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
  .principal:disabled { opacity:.6; cursor:default; }
  .annuler {
    margin-left:auto; height:auto; padding:0; border:none;
    background:transparent; font-size:13px; color:var(--muted);
    text-decoration:underline; cursor:pointer;
  }
  .annuler:hover { background:transparent; color:var(--ink2); }
  /* The spring pushes the destructive gesture and “Cancel” to the
     right, separated from the Send/Attach/Save cluster. */
  .essor { flex:1; }
  /* R2: the “Send later” card, above the footer — the same local
     overlay idiom as the color swatch. */
  .groupe-differe { position:relative; display:inline-flex; }
  .differe {
    position:absolute; bottom:40px; left:0; z-index:3; width:320px;
    padding:14px; background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:10px;
  }
  .differe-label {
    display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); white-space:nowrap;
  }
  .differe-label input {
    flex:1; min-width:0; height:32px; padding:0 8px; font:inherit;
    font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .differe-note { margin:0; font-size:12px; color:var(--muted); line-height:1.5; }
  .differe-actions { display:flex; align-items:center; gap:12px; }

  /* R3: “Delete draft” and its confirmation — alert hue, never the
     accent color (which invites the click). */
  .supprimer { color:var(--alert); border-color:var(--border); }
  .supprimer:hover { background:var(--alert); color:var(--onAccent); border-color:var(--alert); }
  .confirmation .avert-suppr { font-size:13px; color:var(--alert); font-weight:600; }
  .danger {
    font-weight:600; color:var(--onAccent); background:var(--alert);
    border-color:var(--alert);
  }
  .danger:hover { background:var(--alert); border-color:var(--alert); filter:brightness(1.08); }
</style>
