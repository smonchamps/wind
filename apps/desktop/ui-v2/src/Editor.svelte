<script>
  // Editor.svelte — extracted from Compose.svelte (PLAN-AUDIT-V3 E7,
  // Chief-Engineer decision D7): the contenteditable rich-text region + the
  // formatting bar, their handlers, and the deprecated
  // `execCommand`/`queryCommandState` pair they still rest on
  // (R4, Chief-Engineer decisions D1-D3 — comments travel with the code below).
  //
  // Contract with Compose: the body's truth lives in the DOM
  // (`bodyField`) HERE, never in Compose — Compose's draft
  // save/resume flow calls this component's exported methods, it
  // never reaches for the editor's DOM refs itself:
  //   - `getLoaded()` — what saving/sending hand back to Rust
  //     ({ body, bodyHtml }, anti-churn included).
  //   - `set(html, { initialText, htmlInitial })` — sets the body
  //     (async: waits a `tick()` so the node exists), and resets the
  //     formatting bar's own local state (selection snapshot, open
  //     swatch) — every call from Compose marks a fresh card.
  //   - `isModified()` — has the user typed since the last `set`.
  //   - `getText()` — the body's plain text (`textContent`), for the
  //     “is this draft empty” check.
  //   - `getVersion()` — the body's reactive pulse: Compose does not
  //     see writes to this component's DOM, it sees this counter
  //     (read it inside a `$derived`/effect to track it).
  //   - `focus()` / `focusStart()` — plain focus vs. top-posting
  //     (cursor at the very start, scrolled into view).
  //   - `important` (prop) / `onImportantToggle` (callback) — the
  //     “important” toggle lives IN the formatting bar (R3, field
  //     2026-08-21) but the marker itself is a MESSAGE state, owned
  //     by Compose: this component only displays it and reports the
  //     gesture back up.
  //   - `oninput` (callback) — every gesture that changes the body
  //     (keystroke, a format command) reports it, so Compose can
  //     schedule its autosave.
  //   - `children` (the implicit default snippet) — renders BETWEEN
  //     the editor and the formatting bar, exactly where Compose's
  //     attachments row/refusal notice sat in the original markup:
  //     pixel-identical stacking order, still authored by Compose.
  import Icon from './Icon.svelte';
  import { tick } from 'svelte';
  import { t } from './lib/text.svelte.js';

  let {
    important = false,
    onImportantToggle = () => {},
    oninput = () => {},
    children = null,
  } = $props();

  let bodyField = $state(null);
  let bodyArea = $state(null);

  // The body lives in the `contenteditable`'s DOM (`bodyField`), not
  // in Svelte state. As long as `bodyModified` is false, `getLoaded`
  // re-emits the INITIAL values (set by `set`) — the anti-churn; from
  // the first keystroke, `innerHTML` becomes the truth. `bodyVersion`
  // is the body's reactive pulse: Compose's derived values do not see
  // this DOM, they see this counter through `getVersion`.
  let bodyModified = false;
  let initialBodyText = '';
  let initialBodyHtml = null;
  let bodyVersion = $state(0);

  const bodyHtmlNow = () => bodyField?.innerHTML ?? '';

  // What saving and sending hand back to Rust. Without a keystroke,
  // the INITIAL values go back byte for byte (anti-churn, all
  // drafts — text AND rich: the browser's re-serialization is never
  // faithful). Modified: the editor's HTML alone — the fallback text
  // is derived on the Rust side (`frontiere_corps`), the `body`
  // passed would be discarded, so it is not computed.
  export function getLoaded() {
    if (!bodyModified) {
      return { body: initialBodyText, bodyHtml: initialBodyHtml };
    }
    return { body: '', bodyHtml: bodyHtmlNow() };
  }

  export function isModified() {
    return bodyModified;
  }

  export function getText() {
    return bodyField?.textContent ?? '';
  }

  export function getVersion() {
    return bodyVersion;
  }

  // Sets the editor's content. `tick()` first: the node only exists
  // once the overlay is rendered — setting it before would be lost.
  // `htmlInitial: null` = TEXT draft (saving without a keystroke must
  // not convert it); by default, the HTML set is the initial one.
  // Every call marks a fresh card (open/openDraft): the formatting
  // bar's own local state — the selection snapshot, the open color
  // swatch — resets here too, so Compose never has to know it exists.
  export async function set(html, { initialText = '', htmlInitial = html } = {}) {
    bodyModified = false;
    initialBodyText = initialText;
    initialBodyHtml = htmlInitial;
    bodySelection = null;
    showColors = false;
    await tick();
    if (bodyField) bodyField.innerHTML = html;
    bodyVersion += 1;
  }

  export function focus() {
    bodyField?.focus();
  }

  // Top-posting: places the cursor at the very start of the body and
  // scrolls the body's OWN scroll container (`.body-area` — the
  // editor's own scrollTop is always 0) back to the top. Focus may
  // have jumped to the end caret before being reset to 0 — the
  // opener must be VISIBLE, not merely first.
  export function focusStart() {
    if (!bodyField) return;
    bodyField.focus();
    const selection = window.getSelection();
    const range = document.createRange();
    range.setStart(bodyField, 0);
    range.collapse(true);
    selection.removeAllRanges();
    selection.addRange(range);
    if (bodyArea) bodyArea.scrollTop = 0;
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
    oninput();
  }

  // --- The formatting bar (R4, Chief-Engineer decisions D1-D3) --------------------
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
    if (!bodyField) return;
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
    bodyVersion += 1;
    oninput();
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

<div class="body-area" bind:this={bodyArea}>
  <!-- The rich editor (R4): contenteditable, content set by `set`,
       read by `getLoaded` — never a bind. The placeholder lives in
       CSS (:empty::before). The selection is tracked only by the
       document's `selectionchange` (it covers KEYBOARD AND mouse —
       no onkeyup/onmouseup duplicate). -->
  <div class="body-editor" contenteditable="true" role="textbox" aria-multiline="true"
       tabindex="0"
       bind:this={bodyField} oninput={onBodyKeystroke}
       data-placeholder={t('compose.bodyPlaceholder')}
       aria-label={t('compose.bodyPlaceholder')}
       data-testid="compose-body"></div>
</div>

{@render children?.()}

<!-- The REAL bar (R4, D1: exactly the requested buttons — Link and
     Quote removed). `onmousedown` neutralized everywhere: a format
     button never steals the body's selection. -->
<div class="format" data-testid="compose-format">
  <select class="select-format" aria-label={t('compose.font')} title={t('compose.font')}
          data-testid="compose-format-font"
          onchange={(e) => selectCommand(e, 'fontName')}>
    <option value="" disabled selected hidden>{t('compose.font')}</option>
    <option value="sans-serif">{t('compose.fontSans')}</option>
    <option value="serif">{t('compose.fontSerif')}</option>
    <option value="monospace">{t('compose.fontMono')}</option>
  </select>
  <select class="select-format" aria-label={t('compose.size')} title={t('compose.size')}
          data-testid="compose-format-size"
          onchange={(e) => selectCommand(e, 'fontSize')}>
    <option value="" disabled selected hidden>{t('compose.size')}</option>
    <option value="2">{t('compose.sizeSmall')}</option>
    <option value="3">{t('compose.sizeNormal')}</option>
    <option value="4">{t('compose.sizeLarge')}</option>
    <option value="6">{t('compose.sizeVeryLarge')}</option>
  </select>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="button-format" class:active={activeFormats.bold}
          aria-label={t('compose.bold')} title={t('compose.bold')} aria-pressed={activeFormats.bold}
          data-testid="compose-format-bold"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('bold')}>
    <Icon name="format_bold" /></button>
  <button type="button" class="button-format" class:active={activeFormats.italic}
          aria-label={t('compose.italic')} title={t('compose.italic')} aria-pressed={activeFormats.italic}
          data-testid="compose-format-italic"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('italic')}>
    <Icon name="format_italic" /></button>
  <button type="button" class="button-format" class:active={activeFormats.underline}
          aria-label={t('compose.underline')} title={t('compose.underline')} aria-pressed={activeFormats.underline}
          data-testid="compose-format-underline"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('underline')}>
    <Icon name="format_underlined" /></button>
  <button type="button" class="button-format" class:active={activeFormats.strikethrough}
          aria-label={t('compose.strikethrough')} title={t('compose.strikethrough')} aria-pressed={activeFormats.strikethrough}
          data-testid="compose-format-bar"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('strikeThrough')}>
    <Icon name="strikethrough_s" /></button>
  <span class="group-color">
    <button type="button" class="button-format"
            aria-label={t('compose.color')} title={t('compose.color')}
            data-testid="compose-format-color"
            onmousedown={(e) => e.preventDefault()}
            onclick={() => (showColors = !showColors)}>
      <Icon name="format_color_text" /></button>
    {#if showColors}
      <div class="palette" data-testid="compose-palette">
        {#each COLORS as color (color)}
          <button type="button" class="hue" style="background:{color}"
                  aria-label={color}
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => command('foreColor', color)}></button>
        {/each}
      </div>
    {/if}
  </span>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="button-format"
          aria-label={t('compose.alignLeft')} title={t('compose.alignLeft')}
          data-testid="compose-format-left"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyLeft')}>
    <Icon name="format_align_left" /></button>
  <button type="button" class="button-format"
          aria-label={t('compose.alignCenter')} title={t('compose.alignCenter')}
          data-testid="compose-format-center"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyCenter')}>
    <Icon name="format_align_center" /></button>
  <button type="button" class="button-format"
          aria-label={t('compose.alignRight')} title={t('compose.alignRight')}
          data-testid="compose-format-right"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('justifyRight')}>
    <Icon name="format_align_right" /></button>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="button-format" class:active={activeFormats.bulletList}
          aria-label={t('compose.listBullets')} title={t('compose.listBullets')} aria-pressed={activeFormats.bulletList}
          data-testid="compose-format-bullets"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('insertUnorderedList')}>
    <Icon name="format_list_bulleted" /></button>
  <button type="button" class="button-format" class:active={activeFormats.numberedList}
          aria-label={t('compose.listNumbered')} title={t('compose.listNumbered')} aria-pressed={activeFormats.numberedList}
          data-testid="compose-format-numbered"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('insertOrderedList')}>
    <Icon name="format_list_numbered" /></button>
  <button type="button" class="button-format"
          aria-label={t('compose.indentLess')} title={t('compose.indentLess')}
          data-testid="compose-format-indent-less"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('outdent')}>
    <Icon name="format_indent_decrease" /></button>
  <button type="button" class="button-format"
          aria-label={t('compose.indentMore')} title={t('compose.indentMore')}
          data-testid="compose-format-indent-more"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('indent')}>
    <Icon name="format_indent_increase" /></button>
  <span class="sep" aria-hidden="true"></span>
  <button type="button" class="button-format"
          aria-label={t('compose.clearFormat')} title={t('compose.clearFormat')}
          data-testid="compose-format-clear"
          onmousedown={(e) => e.preventDefault()} onclick={() => command('removeFormat')}>
    <Icon name="format_clear" /></button>
  <span class="sep" aria-hidden="true"></span>
  <!-- R3 (field, 2026-08-21): “Important” lives IN the formatting
       bar, in the format of its neighbors (icon only) — a toggle of
       the message's state (aria-pressed), not an action. The marker
       itself is owned by Compose (a MESSAGE state, saved with the
       draft) — this button only displays it and reports the
       gesture back up. -->
  <button type="button" class="button-format" class:active={important}
          aria-label={t('compose.importantTitle')} title={t('compose.importantTitle')}
          aria-pressed={important} data-testid="compose-important"
          onmousedown={(e) => e.preventDefault()}
          onclick={onImportantToggle}>
    <Icon name="priority_high" /></button>
</div>

<style>
  .body-area {
    padding:20px 22px; display:flex; flex-direction:column;
    min-height:220px; flex:1; overflow:auto;
  }
  .body-editor {
    flex:1; width:100%; min-height:180px; font-size:15px; line-height:1.65;
    color:var(--ink); border:none; outline:none;
    background:transparent; font-family:inherit;
    overflow-wrap:break-word;
  }
  /* The textarea's placeholder, redone: visible as long as the body
     is empty, in the muted hue. */
  .body-editor:empty::before {
    content:attr(data-placeholder); color:var(--muted); pointer-events:none;
  }
  /* The rich quote: the left net that `quote_reply_html` sets as an
     inline style is the reference; this only styles the blockquotes
     born from the indent, with no style of its own. */
  .body-editor :global(blockquote) { margin:0 0 0 0.8ex; }

  .format {
    flex:none; padding:8px 18px; border-top:1px solid var(--border);
    background:var(--bg); display:flex; align-items:center; gap:6px;
    flex-wrap:wrap;
  }
  .button-format {
    height:32px; min-width:32px; padding:0 6px; display:inline-flex;
    align-items:center; justify-content:center; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .button-format:hover { background:var(--sel); color:var(--ink); }
  /* The active state states what the selection carries (aria-pressed
     likewise). */
  .button-format.active {
    background:var(--sel); color:var(--accent); border-color:var(--accent);
  }
  .button-format :global(.ic) { width:18px; height:18px; }
  .select-format {
    height:32px; padding:0 8px; font:inherit; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-control);
  }
  .select-format option { background:var(--surface); color:var(--ink); }
  .sep {
    width:1px; height:20px; background:var(--border); flex:none;
    margin:0 4px;
  }
  /* The color swatch (D3): twelve fixed hues, above the bar. */
  .group-color { position:relative; display:inline-flex; }
  .palette {
    position:absolute; bottom:38px; left:0; z-index:1;
    display:grid; grid-template-columns:repeat(6, 22px); gap:6px;
    padding:10px; background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-control);
    box-shadow:var(--shadow);
  }
  .hue {
    height:22px; width:22px; min-width:0; padding:0;
    border:1px solid var(--border); border-radius:var(--r-control); cursor:pointer;
  }
  .hue:hover { outline:2px solid var(--accent); outline-offset:1px; }
</style>
