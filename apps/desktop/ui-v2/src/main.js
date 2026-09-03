// v2 mount — theme AND language restored BEFORE the first render (no
// flash; the language is a read-only PROBE, sub-millisecond: before
// the migration modal, nothing must open the database — ADR 0012),
// measurement hooks exposed for the P1 bench (mesure-v2.mjs) and the e2e.
import './system.css';
import { mount } from 'svelte';
import { applyTheme, restoreTheme, THEMES } from './lib/theme.js';
import { restoreLanguage } from './lib/text.svelte.js';
import { restorePanes } from './lib/panes.svelte.js';
import { restoreWidths } from './lib/widths.svelte.js';
import { restoreSpacing } from './lib/spacing.svelte.js';
import App from './App.svelte';

restoreTheme();
restorePanes();
restoreWidths();
// A83: BEFORE the mount — this way the first probe already measures the
// right notch, and the list does not redraw itself at startup.
restoreSpacing();
await restoreLanguage();

const app = mount(App, { target: document.getElementById('app') });

// Startup: first page visible -> #perf carries data-startup, as in
// v1 — the bench waits for this signal.
const pending = setInterval(() => {
  const { list } = app.api();
  // `exactTotal` too: since PLAN-DEFILEMENT-PROFOND the count follows
  // the rows — the perf line must not freeze the FLOOR of the first
  // rows as if it were the number of conversations.
  const state = list?.snapshot();
  if (state && state.firstPageMs !== null && state.exactTotal) {
    clearInterval(pending);
    app.markStartup();
  }
}, 16);

// Measurement bench: page (jump + serve + render, forced reflow), theme
// (hot toggle), opening (message_body -> iframe).
window.__mesure = {
  themes: THEMES,
  state() {
    // The reading pane is only mounted in 3-pane mode (PLAN-VOLETS)
    // — the bench always measures the default, the guard just avoids
    // a crash if the state has been toggled by hand.
    const { list, reading } = app.api();
    return { ...list.snapshot(), ...(reading ? reading.snapshot() : {}) };
  },
  async page(index) {
    const { list } = app.api();
    return list.goAndServe(index);
  },
  theme(name) {
    const t0 = performance.now();
    applyTheme(name);
    void document.body.offsetHeight;
    return performance.now() - t0;
  },
  async open(index) {
    const { list, reading } = app.api();
    await list.goAndServe(index);
    const line = list.rowAt(index);
    if (!line) throw new Error(`no row served at index ${index}`);
    return reading.open(line);
  },
  // The reload that the cycle and the gestures trigger (PLAN-REACTIVITE
  // E1) — exposed for the assertion “never a wait on rows already
  // served”, played with the transport on hold (__e2eRetenue).
  reload() {
    const { list } = app.api();
    list.reload();
  },
};
