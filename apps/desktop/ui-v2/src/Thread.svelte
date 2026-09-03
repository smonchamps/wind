<script>
  // The THREAD in cards — the single reading object (UI v3, CE verdict
  // of 2026-08-16, decision D4; line drawing of the mockup from field
  // A45): title, inventory chips, bare buttons on the right,
  // avatar cards — collapsed to one row, expanded to a full card
  // ("Name <address>" / "To: …", long time — A92) —, attachments,
  // thread draft, bar of five direct gestures (D5 — never a
  // "More" menu, exception b of the annotations; the per-message "⋯"
  // of the mockup awaits its actions). The reading pane
  // and screen 03 mount it AS IS: two frames, one object — the state
  // lives in lib/thread.svelte.js and survives the frame change.
  //
  // Invariant S1: a sandboxed iframe `allow-same-origin` WITHOUT
  // allow-scripts per expanded message, body served by the core,
  // links intercepted (lib/links.js).
  import Icon from './Icon.svelte';
  import ThreadBar from './ThreadBar.svelte';
  import {
    thread,
    msgKey,
    toggleMessage,
    allExpand,
    allCollapse,
    showImages,
    alwaysShowImages,
    isEcho,
    hiddenNames,
    retry,
  } from './lib/thread.svelte.js';
  import { call, chooseDestination } from './lib/transport.js';
  import { wireLinks } from './lib/links.js';
  import {
    invitationTile, whenInvitation, invitationKicker, invitationStatus,
    attendeeLine, organizerLocation,
  } from './lib/invitation.js';
  import { when, whenLong } from './lib/when.js';
  import { mailboxBlock } from './lib/mailbox.js';
  import { initials } from './lib/initiales.js';
  import { activation } from './lib/keyboard.js';
  import { t } from './lib/text.svelte.js';
  import { autoBody } from './lib/body.js';

  let {
    drafts = [],
    // A80/D5: the mailbox block repeats down the thread, behind the
    // sender's name — expanded card AND collapsed row. `accounts` serves
    // the only D7 guard that matters here: is there more than one account?
    markers = {},
    names = {},
    accounts = [],
    // Does the current view mix accounts? (Only App knows: it holds
    // both the chosen account AND the search state.)
    mixed = false,
    onresume = () => {},
    onarchive = () => {},
    ondelete = () => {},
    onreply = () => {},
    onreplyall = () => {},
    onforward = () => {},
    // R2 (PLAN-RETOURS-3, D2): flag a thread as junk, or bring it
    // back to Inbox. The gesture lives in the thread bar, per thread;
    // `isJunk` toggles the label according to the current view.
    onspam = () => {},
    onnotspam = () => {},
    isJunk = false,
    onflash = () => {},
    // The pane frame passes the enlarge gesture; screen 03 does not.
    onenlarge = null,
    // R4 (PLAN-RETOURS-7, D3/D4): "Pin" lives in the thread bar,
    // offered only by Inbox (pins only appear at the top of
    // Inbox — D4); App decides.
    pinnable = false,
    onpin = () => {},
    // PLAN-MODE-ORGANISE E1: "Move to…" — the WHOLE sender
    // changes destination (Inbox / Feed / Paper trail), its
    // messages follow by construction (the query reads the routing).
    // Offered only by organized mode; App decides.
    organized = false,
    onmove = () => {},
    onsetaside = () => {},
  } = $props();

  // The "Move to…" menu — closed by the choice, a second
  // click, or a thread CHANGE (E1 review: the component survives a
  // row change — without this mirroring, thread A's menu would stay
  // open over thread B and a stray click would route B).

  // Since R4 (PLAN-RETOURS-3, D4) Reply / Reply all /
  // Forward target EACH message (bar per message) — no more single
  // `target()` at the thread level. The `r` keyboard shortcut, though,
  // stays on the list selection (App.svelte), unchanged.
  //
  // The FRESH attachment count (after-scan, CE field 2026-08-14): the
  // row carries the one from BEFORE opening — the store keeps the one
  // from message_body as soon as the body is served.
  const attachmentCountOf = (m) => thread.attachmentCount[msgKey(m)] ?? m.attachment_count;

  // A80/D5: the account is read from the MESSAGE (m.account_id — the
  // canonical identity, STANDARD invariant 2); every message of a
  // thread comes from the same mailbox, the fallback address is that
  // of the open thread. Normal weight: it is the name that carries
  // the authority. THE rule of the block (lib/mailbox.js), shared with
  // the list. The account is read from the MESSAGE (canonical
  // identity, STANDARD invariant 2); the fallback address is that of
  // the open thread.
  //
  // Field verdict of 2026-08-25 (point 12): the pane follows the SAME
  // view guard as the list — in a single-account view, the list stayed
  // silent while the pane still spoke. `mixed` comes down from App,
  // the only one that knows the current view and the search state.
  const mailboxOf = (m) =>
    !mixed
      ? null
      : mailboxBlock({
        accountId: m.account_id,
        address: thread.line?.account_email ?? '',
        markers,
        names,
        accounts,
      });

  // RETOURS-14 R4 (D5): the sign of the "mixed thread" — the golden
  // rule keeps a whole thread in Inbox as soon as ONE message comes
  // from a known sender; an unknown sender replying to it waits at the
  // Screener WHILE their message is read. The badge says so, instead
  // of letting it look like the desk was bypassed. Loaded when the
  // thread opens, organized mode only — the desk is short, the call
  // counts in ms.
  let screenerPending = $state(new Set());
  $effect(() => {
    void thread.line;
    if (!organized || !thread.line) {
      screenerPending = new Set();
      return;
    }
    let stale = false;
    call('screener_addresses')
      .then((addresses) => {
        if (!stale) screenerPending = new Set(addresses);
      })
      .catch(() => {});
    return () => {
      stale = true;
    };
  });
  // The desk's key is `sender_norm` (lower(trim()) SQLite, hence
  // ASCII); JS's toLowerCase is Unicode — divergence ASSUMED on a
  // non-ASCII uppercase letter in the address (the same limit as
  // `adresse_images` on the core side): the badge can be missing,
  // never lying.
  const pending = (m) =>
    !!m.sender_address && screenerPending.has(m.sender_address.trim().toLowerCase());

  // The draft of the open thread — the most recent one (B-D5).
  const threadDraft = $derived.by(() => {
    if (!thread.line || thread.line.thread_id == null) return null;
    let kept = null;
    for (const b of drafts) {
      if (b.thread_id !== thread.line.thread_id) continue;
      if (!kept || b.updated_epoch > kept.updated_epoch) kept = b;
    }
    return kept;
  });

  // The mockup's inventory chips (field A45): n messages ALWAYS
  // said — "1 message" included —, files SUMMED across the thread
  // (the row only carries the count of ITS OWN message).
  const messageCount = $derived(thread.messages.length || thread.line?.thread_size || 1);
  // The toggle is DERIVED from the real state (field A47): everything
  // expanded → "Collapse all" — a one-message thread thus opens on it.
  const allExpanded = $derived(
    thread.messages.length > 0 && thread.messages.every((m) => thread.expanded[msgKey(m)]),
  );
  const attachmentsTotal = $derived(
    thread.messages.length
      ? thread.messages.reduce((n, m) => n + (attachmentCountOf(m) || 0), 0)
      : (thread.line ? attachmentCountOf(thread.line) : 0),
  );

  const own = (m) => thread.line && m.sender_address === thread.line.account_email;

  // R5 (PLAN-RETOURS-12, decision D4): recipient names come from the
  // contacts DIRECTORY — `to_addrs`/`cc_addrs` only store bare
  // addresses (address_literal), and mail already read has already
  // learned the names. ONE request per set of addresses, bounded to
  // the thread's To/Cc (never a sweep of envelopes — lesson A64);
  // unknown address: bare. The key memoizes: a re-render of
  // `thread.messages` or a remount (pane ↔ screen 03 toggle) with the
  // same addresses does not repeat the RPC (review). The invalidation
  // lives in the effect's cleanup — the lifecycle Svelte provides.
  let directory = $state({});
  $effect(() => {
    const addresses = [...new Set(
      thread.messages.flatMap((m) => [...(m.to_addrs ?? []), ...(m.cc_addrs ?? [])]),
    )];
    const key = addresses.join('\n');
    // The cache survives the component (hiddenNames, lib/thread.svelte.js):
    // a re-render of thread.messages or a remount with the same
    // addresses does not repeat the RPC.
    if (key === hiddenNames.key) {
      directory = hiddenNames.names;
      return;
    }
    if (!addresses.length) {
      hiddenNames.key = '';
      hiddenNames.names = {};
      directory = {};
      return;
    }
    let stale = false;
    call('address_names', { addresses: addresses }).then(
      (names) => {
        hiddenNames.key = key;
        hiddenNames.names = names;
        if (!stale) directory = names;
      },
      // The failure is SAID (review): the visible fallback is the bare
      // address, without this signal a regression of the command would
      // be silent.
      (err) => console.error('address_names :', err),
    );
    return () => { stale = true; };
  });
  // THE "Name <address>" form — a single rule for the three header
  // lines: name missing, empty or equal to the address → bare address.
  const label = (name, address) =>
    (name && name !== address ? `${name} <${address}>` : address);
  // The name comes from the directory (key in lowercase, its own form).
  const nameAddr = (address) => label(directory[address.trim().toLowerCase()], address);
  // The name is already CARRIED (the sender of a thread copy).
  const carriedNameAddr = (name, address) => (address ? label(name, address) : (name ?? ''));
  // R4 (PLAN-RETOURS-MAIL): stored recipients (`to_addrs`, drawn from
  // the same ENVELOPE at sync) say who the message went to. Fallback
  // for messages predating their storage: the prototype's heuristic —
  // a message of our own → the thread's first other contact, another's
  // message → our own copy (Sent), otherwise the account's address.
  function recipients(m) {
    if ((m.to_addrs?.length ?? 0) > 0) {
      return m.to_addrs.map(nameAddr).join(', ');
    }
    const target = own(m)
      ? thread.messages.find((x) => x.sender_address && x.sender_address !== m.sender_address)
      : thread.messages.find((x) => own(x));
    if (target) return carriedNameAddr(target.sender, target.sender_address);
    // Last fallback: the account's address, bare — the honest fact
    // (the core does not know our name, and the directory is only
    // queried on the thread's To/Cc).
    return thread.line?.account_email ?? '';
  }

  // E5bis: `autoBody` lives in lib/body.js — Feed in cards measures the
  // same bodies, a single gate (A47/S1).

  // Replying to an invitation (D5-D6): subject and body in the UI's
  // language, the iTIP email logged on the core side (reply_invitation),
  // then a flush kicked off — offline, it goes out at the next launch
  // (the semantics stated in PLAN-RETOURS-6).
  let repliesInFlight = $state({});
  async function replyInvitation(m, reply) {
    const k = msgKey(m);
    if (repliesInFlight[k]) return;
    repliesInFlight[k] = true;
    // OPTIMISTIC (field R3'a): the button marks itself the instant of
    // the click — the log follows; a failure restores the prior state
    // and says so.
    const before = thread.invitations[k].status;
    thread.invitations[k].status = reply;
    try {
      const subject = t(`inv.subject_${reply}`, { title: thread.invitations[k].title });
      const view = await call('reply_invitation', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        reply,
        subject,
        body: subject,
      });
      if (view) thread.invitations[k] = view;
      call('flush_outbox').catch(() => {});
    } catch (err) {
      thread.invitations[k].status = before;
      onflash(t('error.invitation', { err }));
    } finally {
      repliesInFlight[k] = false;
    }
  }

  // Transient in-flight state: local to the component, nothing to
  // share between frames (v3 review — the store only carries the
  // thread's state).
  let savesInFlight = $state({});
  async function save(m, attachment) {
    const k = `${msgKey(m)}#${attachment.index}`;
    if (savesInFlight[k]) return;
    savesInFlight[k] = true;
    try {
      // R1 (PLAN-RETOURS-4, D2): the suggested path (Downloads + a name
      // sanitized by the core), then the "Save as" dialog — the user
      // chooses both folder AND name. Cancel = nothing, no toast, no
      // error; the bytes are only fetched after the choice (never a
      // useless fetch if the user backs out).
      const fallback = await call('suggested_save_path', { name: attachment.name });
      const dest = await chooseDestination(fallback);
      if (!dest) return;
      const path = await call('save_attachment', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        index: attachment.index,
        dest,
      });
      onflash(t('toast.attachmentSaved', { path }));
    } catch (err) {
      onflash(t('error.save', { err }));
    } finally {
      savesInFlight[k] = false;
    }
  }
</script>

{#if thread.line}
  <!-- BOTH frames are FLAT (field A46, extended to screen 03
       by PLAN-RETOURS-7 R3): no enclosing elevation, no
       nets — only message cards raise, and everything scrolls
       in one flow EXCEPT the thread bar, sticky at the top (RETOURS-14 R1).
       "Screen 03 keeps its full card" (A46) is reversed: the
       full frame is a centered flat column (Conversation.svelte). -->
  <div class="objet-fil">
    <div class="tete">
      <h3 class="titre display" data-testid="fil-sujet">{thread.line.subject}</h3>
      <!-- The mockup's row (field A45): inventory chips on the
           left, BARE buttons on the right — "Expand all" at the edge. -->
      <div class="puces" data-testid="fil-puces">
        <span class="puce"><Icon name="forum" />{t('chip.messages', { n: messageCount })}</span>
        {#if attachmentsTotal > 0}
          <span class="puce"><Icon name="attach_file" />{t('chip.files', { n: attachmentsTotal })}</span>
        {/if}
        <span class="essor"></span>
        {#if onenlarge}
          <!-- V-D2: a SINGLE message has no conversation to open
               — the button stays, inert and says so. "Open" carries
               its own glyph (A46): one icon, one meaning (A3). -->
          <button type="button" class="nu" data-testid="voir-conversation"
                  class:inerte={thread.line.thread_id == null}
                  aria-disabled={thread.line.thread_id == null}
                  tabindex={thread.line.thread_id != null ? 0 : -1}
                  onclick={() => thread.line.thread_id != null && onenlarge(thread.line)}>
            <Icon name="open_in_full" />{t('reading.open')}</button>
        {/if}
        <!-- The toggle (A46, derived from A47): "Collapse all"
             when EVERYTHING is expanded — one-message thread included —,
             otherwise "Expand all"; manual expansions make it follow. -->
        {#if allExpanded}
          <button type="button" class="nu" data-testid="tout-replier" onclick={allCollapse}>
            <Icon name="unfold_less" />{t('conv.collapse')}</button>
        {:else}
          <button type="button" class="nu" data-testid="tout-deplier" onclick={allExpand}>
            <Icon name="unfold_more" />{t('conv.expand')}</button>
        {/if}
      </div>
    </div>

    <!-- RETOURS-14 R1 (D1): the thread bar lives AT THE TOP, sticky to
         scrolling — it stays visible at the bottom of a long thread, in
         BOTH frames (the scroll belongs to the frame, the sticky
         anchors to the scrollport of the pane as of the scene). SORT
         gestures only (D5): Reply/Reply all/Forward stay
         per message (D4). Report as spam falls in there (D2), or "Not
         spam" in the Junk view. -->
    <!-- THE thread bar (ThreadBar.svelte) — pane only: at
         screen 03 its buttons live in the scene's header bar
         (Conversation, field 2026-09-02). -->
    {#if thread.frame !== 'full'}
      <ThreadBar {isJunk} {pinnable} {organized}
                {onarchive} {onspam} {onnotspam} {onpin} {onmove} {onsetaside} />
    {/if}

    <div class="fil">
      {#each thread.messages as m (msgKey(m))}
        {@const k = msgKey(m)}
        {@const mailbox = mailboxOf(m)}
        {#if thread.expanded[k]}
          <article class="deplie" data-testid="message-deplie">
            <!-- The two-line header (PLAN-RETOURS-12 R5):
                 "Name <address> on Mailbox" then "To: Name <address>, …"
                 (and "Cc: …" if Cc exist, D6) — the From/To/Subject
                 block stays dead, the head says it all (A45). -->
            <div class="tete-message" role="button" tabindex="0"
                 aria-expanded="true"
                 onclick={() => toggleMessage(m)} onkeydown={activation(() => toggleMessage(m))}>
              <span class="avatar" aria-hidden="true">{initials(m.sender)}</span>
              <span class="qui">
                <!-- A80/D5: the mailbox behind the name — the same block
                     as the list row (system.css). -->
                <span class="rang-nom">
                  <span class="auteur">{m.sender}</span>
                  {#if m.sender_address && m.sender_address !== m.sender}
                    <span class="adr adr-exp">{`<${m.sender_address}>`}</span>
                  {/if}
                  {#if pending(m)}
                    <span class="attente-portier" data-testid="attente-portier">{t('thread.screenerPending')}</span>
                  {/if}
                  {#if mailbox}
                    <span class="boite" title={mailbox.title}>
                      <span class="mot">{t('list.on')}</span>
                      {#if mailbox.marker}
                        <span class="repere-nu" data-teinte={mailbox.marker.hue}
                              aria-hidden="true"><Icon name={mailbox.marker.icon} size={14} /></span>
                      {/if}
                      <span class="lib">{mailbox.label}</span>
                    </span>
                  {/if}
                </span>
                <span class="adr" data-testid="ligne-a">{t('conv.toLine', { list: recipients(m) })}</span>
                {#if (m.cc_addrs?.length ?? 0) > 0}
                  <span class="adr" data-testid="ligne-cc">{t('conv.ccLine', { list: m.cc_addrs.map(nameAddr).join(', ') })}</span>
                {/if}
              </span>
              <span class="quand">{whenLong(m.epoch)}</span>
            </div>
            <div class="contenu">
              <!-- The invitation card (PLAN-INVITATIONS, A76): AT THE
                   TOP of the content — it is the object of the message, before
                   the files. Date tile in tuile/tuileInk (the current
                   mailbox's drawing), three NEUTRAL buttons (D4 —
                   A14 intact, the card does not rank the reply),
                   the current reply said by aria-pressed. A floating
                   time displays as is, never converted
                   (guard D1). -->
              {#if thread.invitations[k]}
                {@const inv = thread.invitations[k]}
                {@const tile = invitationTile(inv)}
                {@const invWhen = whenInvitation(inv)}
                {@const orgLocation = organizerLocation(inv)}
                {@const invAttendee = attendeeLine(inv)}
                <div class="invitation" data-testid="invitation">
                  <div class="inv-tete">
                    <span class="inv-kicker" class:annulee={inv.cancelled}>{invitationKicker(inv)}</span>
                    {#if inv.method === 'request'}
                      <span class="inv-statut" data-testid="invitation-statut">{invitationStatus(inv)}</span>
                    {/if}
                  </div>
                  <div class="inv-corps">
                    {#if tile}
                      <span class="inv-tuile" class:eteinte={inv.cancelled} aria-hidden="true">
                        <span class="inv-mois">{tile.month}</span>
                        <span class="inv-jour">{tile.day}</span>
                      </span>
                    {/if}
                    <div class="inv-details">
                      <span class="inv-titre" class:barre={inv.cancelled}
                            data-testid="invitation-titre">{inv.title}</span>
                      {#if invWhen}
                        <span class="inv-quand">{invWhen}</span>
                      {/if}
                      {#if orgLocation}
                        <span class="inv-lieu">{orgLocation}</span>
                      {/if}
                      {#if inv.cancelled}
                        <span class="inv-annulee">{t('inv.cancelledText')}</span>
                      {:else if invAttendee}
                        <span class="inv-repondant" data-testid="invitation-repondant">{invAttendee}</span>
                      {/if}
                    </div>
                  </div>
                  {#if inv.can_reply}
                    <!-- R7/R9 (field 2026-08-23): the icon says the
                         reply, the color its meaning (accent / neutral /
                         alert) — the text always doubles it (A8). -->
                    <div class="inv-actions" data-testid="invitation-actions">
                      <button type="button" class="ton-accepted" data-testid="inv-accepter"
                              aria-pressed={inv.status === 'accepted'}
                              disabled={repliesInFlight[k]}
                              onclick={() => replyInvitation(m, 'accepted')}>
                        <Icon name="check_circle" />{t('action.accept')}</button>
                      <button type="button" class="ton-tentative" data-testid="inv-provisoire"
                              aria-pressed={inv.status === 'tentative'}
                              disabled={repliesInFlight[k]}
                              onclick={() => replyInvitation(m, 'tentative')}>
                        <Icon name="question_mark" />{t('action.tentative')}</button>
                      <button type="button" class="ton-declined" data-testid="inv-refuser"
                              aria-pressed={inv.status === 'declined'}
                              disabled={repliesInFlight[k]}
                              onclick={() => replyInvitation(m, 'declined')}>
                        <Icon name="cancel" />{t('action.decline')}</button>
                    </div>
                  {/if}
                </div>
              {/if}
              <!-- R2 (PLAN-RETOURS-7): attachments BEFORE the
                   body — under the message head, where the eye expects
                   them without scrolling the mail; the image guard stays
                   glued to the body it concerns. -->
              <!-- A GESTURE echo has no per-attachment metadata
                   (it dies with the source): the section only shows
                   if chips exist — never a title with nothing
                   beneath it (PLAN-RETOURS-5). -->
              {#if attachmentCountOf(m) > 0 && (!isEcho(m) || (thread.attachments[k] ?? []).length > 0)}
                <div class="fichiers" data-testid="lecture-fichiers">
                  <p class="titre-fichiers">{t('conv.attachedFiles')}</p>
                  <!-- R2 (PLAN-RETOURS-4, D4): name AND weight in the SAME
                       clickable chip — an accepted exception to "1 chip = 1
                       piece of information", a single icon (the same
                       manipulable object as the compose chip, not two
                       readings). -->
                  <!-- An echo's chips are INERT (PLAN-RETOURS-5,
                       D2): the bytes left the log at send time —
                       name and weight show, nothing gets saved
                       during the reconciliation window — and so they
                       carry NO veil (no promise). -->
                  <div class="puces">
                    {#each thread.attachments[k] ?? [] as attachment (attachment.index)}
                      <button type="button" class="puce bouton" data-testid="piece-jointe"
                              disabled={isEcho(m) || savesInFlight[`${k}#${attachment.index}`]}
                              onclick={() => !isEcho(m) && save(m, attachment)}
                              title={isEcho(m) ? undefined : t('reading.save')}>
                        <Icon name="description" />
                        <span class="nom">{attachment.name}</span><span class="taille">{attachment.size}</span>
                        <!-- R1 (PLAN-RETOURS-7, D1): on hover as on
                             keyboard focus, a veil covers the chip and SAYS
                             the action — "Save" (the product's
                             vocabulary: the click opens "Save as").
                             Same geometry, the row does not reflow. -->
                        {#if !isEcho(m)}
                          <span class="voile" aria-hidden="true">
                            <Icon name="download" />{t('reading.veilSave')}</span>
                        {/if}
                      </button>
                    {/each}
                  </div>
                </div>
              {/if}
              {#if (thread.blockedImages[k] ?? 0) > 0}
                <div class="garde-images" data-testid="garde-images">
                  <Icon name="visibility_off" />
                  <span class="garde-texte">{t('reading.blockedImages', { n: thread.blockedImages[k] })}</span>
                  <button type="button" data-testid="afficher-images"
                          onclick={() => showImages(m)}>
                    {t('reading.showImages')}</button>
                  <!-- D3 (RETOURS-11): the sender rule — never
                       on an echo (ourselves, no third-party sender). -->
                  {#if !isEcho(m)}
                    <button type="button" data-testid="toujours-afficher-images"
                            onclick={() => alwaysShowImages(m)}>
                      {t('reading.alwaysShowImages')}</button>
                  {/if}
                </div>
              {/if}
              {#if thread.errors[k]}
                <!-- PLAN-AUDIT-V2 E10: the core did not serve this body —
                     the image guard's grammar, with the gesture that
                     replays (before: an empty frame, final). -->
                <div class="garde-images" data-testid="corps-echec">
                  <Icon name="error" />
                  <span class="garde-texte">{t('reading.bodyFailure')}</span>
                  <button type="button" data-testid="corps-reessayer"
                          onclick={() => retry(m)}>
                    {t('action.retry')}</button>
                </div>
              {/if}
              <iframe class="corps" sandbox="allow-same-origin" srcdoc={thread.body[k] ?? ''}
                      title={t('reading.body')} use:autoBody
                      onload={(ev) => wireLinks(ev.currentTarget)}></iframe>
            </div>
            <!-- R4 (PLAN-RETOURS-3, D4): the reply gestures at the BOTTOM
                 of EACH message, targeting THIS message — you reply once
                 you're done reading (Gmail/Outlook convention). "Reply
                 all" between Reply and Forward (A14). -->
            <!-- R4 (field finding 2026-08-18): the THREE gestures on
                 EACH message, our own included — one sometimes replies
                 on their own message. The core then addresses the reply
                 to the original recipients (reply_context/reply_all), never
                 to ourselves. -->
            <div class="actions-message" data-testid="actions-message">
              <button type="button" class="principal" data-testid="repondre"
                      onclick={() => onreply(m)}>
                <Icon name="reply" />{t('action.reply')}</button>
              <button type="button" data-testid="repondre-tous"
                      onclick={() => onreplyall(m)}>
                <Icon name="reply_all" />{t('action.replyAll')}</button>
              <button type="button" data-testid="transferer"
                      onclick={() => onforward(m)}>
                <Icon name="reply" mirror />{t('action.forward')}</button>
              <!-- Field R8' (2026-08-23): "Delete" lives PER
                   message — you delete THIS message, not the
                   conversation; the thread stays open if it still has
                   messages left (App decides). On an echo, the gesture
                   says the wait for reconciliation, as before. -->
              <button type="button" data-testid="supprimer"
                      onclick={() => ondelete(m)}>
                <Icon name="delete" />{t('action.delete')}</button>
            </div>
          </article>
        {:else}
          <div class="replie" data-testid="message-replie"
               role="button" tabindex="0" aria-expanded="false"
               onclick={() => toggleMessage(m)} onkeydown={activation(() => toggleMessage(m))}>
            <span class="avatar petit" aria-hidden="true">{initials(m.sender)}</span>
            <span class="auteur">{m.sender}</span>
            {#if pending(m)}
              <span class="attente-portier" data-testid="attente-portier">{t('thread.screenerPending')}</span>
            {/if}
            <!-- A80/D5: the mailbox behind the name, here too. -->
            {#if mailbox}
              <span class="boite" title={mailbox.title}>
                <span class="mot">{t('list.on')}</span>
                {#if mailbox.marker}
                  <span class="repere-nu" data-teinte={mailbox.marker.hue}
                        aria-hidden="true"><Icon name={mailbox.marker.icon} size={14} /></span>
                {/if}
                <span class="lib">{mailbox.label}</span>
              </span>
            {/if}
            <span class="apercu">{m.preview ?? ''}</span>
            <span class="quand">{whenLong(m.epoch)}</span>
          </div>
        {/if}
      {/each}
      {#if threadDraft}
        <div class="replie brouillon" data-testid="conv-brouillon"
             role="button" tabindex="0"
             onclick={() => onresume(threadDraft)}
             onkeydown={activation(() => onresume(threadDraft))}>
          <span class="mention"><Icon name="edit_note" />{t('conv.draft')}</span>
          <span class="apercu">{threadDraft.body}</span>
          <span class="quand">{when(Math.floor(threadDraft.updated_epoch / 1000))}</span>
          <span class="reprendre">{t('action.resume')}</span>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* The FLAT form (A46, both frames since PLAN-RETOURS-7 R3) —
     the prototype's geometry (.voletLecture / .lecture): the thread
     scrolls in a single flow within its frame (the pane or the scene of
     screen 03), the nets and the elevation belong only to
     cards; only the available width changes between frames. */
  .objet-fil { display:flex; flex-direction:column; flex:none; min-height:100%; padding-top:var(--fil-haut, 0); }
  .tete { display:flex; flex-direction:column; flex:none; }
  /* V6: the title switches to the display register (weight 340,
     -.03em — global class .display); the size stays 24 px. */
  .titre {
    margin:2px 0 4px; font-size:24px; line-height:1.2;
    color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .puces { display:flex; align-items:center; gap:10px; flex-wrap:wrap; margin:0 0 4px; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); white-space:nowrap;
  }
  .puce.bouton { cursor:pointer; }
  .puce.bouton:hover { background:var(--sel); }
  /* The BARE buttons of the mockup (A45): border and background
     erased, mini 26 px template — "Expand all", "View the conversation". */
  .essor { flex:1; }
  .nu {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-control); cursor:pointer;
    white-space:nowrap;
  }
  .nu:hover { background:var(--sel); }
  .nu.inerte { cursor:default; opacity:.55; }
  .nu.inerte:hover { background:none; }
  /* The initials avatar of the cards (A45) — the list's drawing
     (E2): 28 px expanded, 26 px collapsed. */
  /* V4 — the square initials tile: ground --tile, ink --tileInk,
     1 px stroke (measured: without it the tile does not exist). */
  .avatar {
    width:28px; height:28px; border-radius:var(--r-tile);
    background:var(--tile);
    border:1px solid var(--border); display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--tileInk); flex:none;
  }
  .avatar.petit { width:26px; height:26px; }
  .fil { flex:none; overflow-y:visible; padding:0; }
  .replie {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    margin-top:12px; background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-surface); cursor:pointer;
    font-size:13px;
  }
  .replie:hover { background:var(--hover); }
  .replie .auteur { font-weight:600; color:var(--ink); flex:none; }
  .replie .apercu {
    flex:1; min-width:0; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .quand { margin-left:auto; color:var(--muted); font-size:12px; flex:none; white-space:nowrap; }
  .replie.brouillon { border:1.5px dashed var(--accent); background:none; }
  .replie.brouillon .mention {
    color:var(--alert); font-weight:600; display:inline-flex;
    align-items:center; gap:6px; flex:none;
  }
  .replie.brouillon .reprendre { color:var(--accent); font-weight:600; flex:none; }
  .deplie {
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow); margin-top:12px;
    display:flex; flex-direction:column;
  }
  /* The A92 header: avatar · (name <address> [on mailbox] / To: … / Cc: …) · when. */
  .tete-message {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    border-bottom:1px solid var(--border); cursor:pointer;
  }
  /* `flex:1 1 auto`: without it, .qui sized itself to content and the
     third-width cap of .boite resolved against this narrow group —
     the rule written in the System ("never more than a third of the
     ROW") did not describe what the thread rendered (review). */
  .tete-message .qui { min-width:0; flex:1 1 auto; display:flex; flex-direction:column; }
  /* A80/D5: name + mailbox block on the same line — the block
     (system.css) keeps its third-width cap and yields first. */
  .tete-message .rang-nom {
    display:flex; align-items:baseline; gap:6px; min-width:0;
  }
  .tete-message .auteur {
    flex:0 1 auto; min-width:0;
    font-size:15px; font-weight:600; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .tete-message .adr {
    font-size:12px; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* R5: the sender's address, on line 1 behind the name — it
     CARRIES .adr (the same ink as the To/Cc lines, structurally) and
     only adds its yield rule: THREE times faster than the name
     (A80's pattern: identity first, detail yields). */
  .tete-message .adr-exp { flex:0 3 auto; min-width:0; }
  .contenu { padding:14px 20px 18px; display:flex; flex-direction:column; gap:12px; }
  /* The invitation card (A76): a card WITHIN the message card —
     10 px surface radius, no elevation (it belongs to the content's
     flow, not the thread). The date tile reuses the current
     mailbox's --tile/--tileInk pair; a cancellation switches the
     tile to dimmed and the title to struck through. */
  .invitation { border:1px solid var(--border); border-radius:var(--r-surface); background:var(--surface); }
  .inv-tete { display:flex; align-items:center; gap:10px; padding:12px 14px 0; }
  .inv-kicker {
    font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); flex:1;
  }
  .inv-kicker.annulee { color:var(--alert); }
  .inv-statut { font-size:12px; color:var(--ink2); white-space:nowrap; }
  .inv-corps { display:flex; gap:14px; padding:12px 14px 14px; align-items:flex-start; }
  .inv-tuile {
    width:52px; height:52px; border-radius:var(--r-control); background:var(--tile);
    color:var(--tileInk); display:flex; flex-direction:column;
    align-items:center; justify-content:center; gap:1px; flex:none;
  }
  .inv-tuile.eteinte { background:var(--bg); color:var(--muted); }
  .inv-mois {
    font-size:10px; font-weight:600; letter-spacing:.08em;
    text-transform:uppercase;
  }
  .inv-jour { font-size:20px; font-weight:600; line-height:1; }
  .inv-details { display:flex; flex-direction:column; gap:4px; min-width:0; }
  .inv-titre { font-size:15px; font-weight:600; color:var(--ink); }
  .inv-titre.barre { color:var(--ink2); text-decoration:line-through; }
  .inv-quand { font-size:13px; color:var(--ink2); }
  .inv-lieu { font-size:13px; color:var(--muted); }
  .inv-annulee { font-size:13px; color:var(--alert); }
  .inv-repondant { font-size:13px; font-weight:600; color:var(--ink2); }
  /* Three NEUTRAL buttons (D4) at the message actions' template
     (30 px); the current reply is said by aria-pressed — --sel
     background and accent trim, A75's selection. */
  .inv-actions {
    display:flex; gap:10px; padding:12px 14px;
    border-top:1px solid var(--border); flex-wrap:wrap;
  }
  .inv-actions button:disabled { cursor:default; opacity:.55; }
  /* R9: the color says the meaning — carried by the icon, the text
     doubles it (A8). Gated pairs: accent/surface and alert/surface at
     3:1, muted/surface at 4.5:1, and their counterparts on --sel. */
  .inv-actions .ton-accepted :global(.ic) { color:var(--accent); }
  .inv-actions .ton-tentative :global(.ic) { color:var(--muted); }
  .inv-actions .ton-declined :global(.ic) { color:var(--alert); }
  .inv-actions button[aria-pressed='true'] {
    font-weight:600; background:var(--sel); border-color:var(--accent);
  }
  .garde-images {
    padding:10px 14px; display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); background:var(--bg);
    border:1px solid var(--border); border-radius:var(--r-control);
    /* Two buttons since RETOURS-11 (D3): in a narrow window they
       wrap to the next line rather than crushing the text. */
    flex-wrap:wrap;
  }
  .garde-images :global(.ic) { color:var(--muted); }
  .garde-texte { flex:1; }
  .garde-images button {
    height:26px; padding:0 10px; font-size:12px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); cursor:pointer;
  }
  .garde-images button:hover { background:var(--sel); }
  .corps {
    /* Overflows by 12 px: the sanitized document's internal gutter
       (mail-render) brings the text back in line with the card's
       padding. The background at the token — the document bakes the
       same value (review A42). The HEIGHT is not here: it follows the
       content, set by autoBody (A47) — never a fixed template. */
    border:none; background:var(--surface); display:block;
    margin-left:-12px; width:calc(100% + 24px); height:0;
  }
  .titre-fichiers {
    margin:0 0 8px; font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted);
  }
  .fichiers .puces { gap:8px; }
  .fichiers .puce { height:28px; }
  /* R2: name + weight in the chip (the compose chip's drawing) —
     the name in full ink, the weight dimmed, spacing by gap. */
  .fichiers .puce .nom { color:var(--ink); }
  .fichiers .puce .taille { font-size:12px; color:var(--muted); }
  /* RETOURS-14 R4 (D5): the "Waiting at the Screener" badge — a
     bare label in dimmed ink, border stroke, never an
     alert: the mail is legitimate, its verdict is just due. */
  .attente-portier {
    flex:none; padding:1px 6px; font-size:11px; color:var(--ink2);
    border:1px solid var(--border); border-radius:var(--r-control);
    white-space:nowrap;
  }
  /* E1: the "Move to…" menu — a card BELOW the button
     since the bar lives at the top (RETOURS-14 R1; the swatch
     picker idiom, A62), buttons at the bar's template, text on the left. */
  /* R4/D4: the reply bar of ONE message — at the bottom of the card.
     Field 2026-09-02 (CE, pass 3 of wave 2's STOP 2): it
     FLOATS at the bottom of the message — the product's floating
     object (A108: surface, border, control radius, --shadow shadow),
     sticky to the bottom of the scrollport while the message scrolls
     (12 px margin from the footer), and in place within the card
     once its end arrives, at 12/20/16 px from the elevation's edges.
     `align-self:flex-start`: it tightens around its buttons, it does
     not span the card. */
  .actions-message {
    position:sticky; bottom:12px; align-self:flex-start;
    margin:12px 20px 16px; padding:8px 10px;
    display:flex; gap:10px; flex-wrap:wrap;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); box-shadow:var(--shadow);
  }
  /* ONE template for the message buttons AND the invitation card's
     (A76 says "at the message actions' template"): keeping it by
     copy would diverge at the first re-tuning (review). */
  .actions-message button, .inv-actions button {
    height:30px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
  }
  .actions-message button:hover, .inv-actions button:hover { background:var(--sel); }
  .actions-message .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .actions-message .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* R1 (PLAN-RETOURS-7, D1): an attachment chip's veil — same
     geometry as the chip (absolute overlay, stable width, the
     row does not reflow), opaque --sel background (the ink/sel pair is
     that of the existing hover), download glyph + "Save".
     Shown on hover AND on keyboard focus (A8); never during an
     in-flight save (:disabled) nor on an echo (not rendered). */
  .puce.bouton { position:relative; }
  .puce .voile {
    position:absolute; inset:0; display:none; align-items:center;
    justify-content:center; gap:6px; font-size:12px; font-weight:600;
    color:var(--ink); background:var(--sel); border-radius:var(--r-control);
    white-space:nowrap; overflow:hidden;
  }
  .puce.bouton:hover .voile, .puce.bouton:focus-visible .voile { display:inline-flex; }
  .puce.bouton:disabled .voile { display:none; }
</style>
