<script>
  import { modal } from './lib/modal.js';
  import Brand from './Brand.svelte';
  import { t } from './lib/text.svelte.js';
  // The "What's new" window (PLAN-BATCH-2026-09 E2, decision D4:
  // modal). Shown ONCE per version, on the first launch after an
  // update — `whats_new_check` compares the binary's version with the
  // seen preference; Continue acknowledges through `whats_new_ack`.
  // The notes arrive as the changelog section's raw markdown, parsed
  // HERE into headings/bullets/paragraphs and rendered as DOM — never
  // {@html}: nothing to inject, even from a trusted source.

  let { info, onclose = () => {} } = $props();

  // Inline markdown the changelog actually uses: links become their
  // text, bold markers and backticks fall away (the bullet's bold
  // LEAD is handled separately, before this pass).
  const clean = (s) =>
    s.replace(/\[([^\]]+)\]\([^)]+\)/g, '$1').replace(/\*\*/g, '').replace(/`/g, '');

  // The changelog's shapes only: `### heading`, `- bullet` (with
  // wrapped continuation lines), blank-line-separated paragraphs.
  function parseNotes(markdown) {
    const blocks = [];
    for (const raw of markdown.split('\n')) {
      const line = raw.trimEnd();
      if (!line.trim()) {
        blocks.push(null);
        continue;
      }
      const heading = line.match(/^#{2,4}\s+(.*)$/);
      if (heading) {
        blocks.push({ kind: 'heading', text: heading[1] });
        continue;
      }
      const bullet = line.match(/^-\s+(.*)$/);
      if (bullet) {
        blocks.push({ kind: 'bullet', text: bullet[1] });
        continue;
      }
      const last = blocks[blocks.length - 1];
      if (last && (last.kind === 'bullet' || last.kind === 'text')) {
        last.text += ` ${line.trim()}`;
      } else {
        blocks.push({ kind: 'text', text: line.trim() });
      }
    }
    return blocks.filter(Boolean).map((block) => {
      if (block.kind !== 'bullet') return { ...block, text: clean(block.text) };
      // `- **Lead.** rest` — the changelog's idiom for a named change.
      const lead = block.text.match(/^\*\*(.+?)\*\*\s*(.*)$/);
      return lead
        ? { ...block, lead: clean(lead[1]), text: clean(lead[2]) }
        : { ...block, lead: null, text: clean(block.text) };
    });
  }

  const blocks = $derived(parseNotes(info.notes));
</script>

<div class="scrim" data-testid="whats-new-modal">
  <div use:modal={{ close: onclose }} class="card" role="dialog" aria-modal="true"
       aria-label={t('whatsnew.aria')}>
    <span class="brand-band"><Brand tile size={28} /><b>Wind</b></span>
    <h3 class="title" data-testid="whats-new-title">
      {t('whatsnew.title', { version: info.version })}</h3>
    <div class="notes" data-testid="whats-new-notes">
      {#each blocks as block}
        {#if block.kind === 'heading'}
          <h4>{block.text}</h4>
        {:else if block.kind === 'bullet'}
          <!-- The literal {' '} space: Svelte trims the whitespace at
               a block boundary, and the lead would run into the text
               ("Your data.Save…" — seen at the visual pass). -->
          <p class="bullet">{#if block.lead}<b>{block.lead}</b>{' '}{/if}{block.text}</p>
        {:else}
          <p>{block.text}</p>
        {/if}
      {/each}
    </div>
    <button type="button" class="main" data-testid="whats-new-continue"
            onclick={onclose}>{t('action.continue')}</button>
  </div>
</div>

<style>
  /* The Feedback card's dressing (surface, border, shadow, 520 px) at
     the migration modal's moment; silence at the prototype, the System
     fills in. z-index 4 = the migration scrim's layer, above the
     overlays at 2. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:4;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .card {
    width:520px; max-width:100%; max-height:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:14px; padding:22px 24px 18px;
  }
  .brand-band { display:flex; align-items:center; gap:8px; color:var(--ink); }
  .title {
    margin:0; font-size:22px; line-height:1.2; font-weight:600;
    letter-spacing:-.01em; color:var(--ink);
  }
  .notes { overflow-y:auto; min-height:0; display:flex; flex-direction:column; gap:8px; }
  .notes h4 {
    margin:8px 0 0; font-size:12px; font-weight:600; text-transform:uppercase;
    letter-spacing:.04em; color:var(--muted);
  }
  .notes p { margin:0; font-size:13px; line-height:1.5; color:var(--ink2); }
  .notes .bullet { padding-left:14px; position:relative; }
  .notes .bullet::before { content:'·'; position:absolute; left:2px; color:var(--muted); }
  .notes b { color:var(--ink); font-weight:600; }
  .main {
    height:32px; padding:0 16px; align-self:flex-end; font-size:13px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-control); cursor:pointer;
  }
  .main:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
