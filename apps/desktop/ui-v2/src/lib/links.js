// Links in a message's body (field finding 2026-08-15): a click
// inside the sandbox iframe navigated the FRAME to the site — refused
// by the sites (X-Frame-Options / frame-ancestors) and by the CSP,
// WebView2 replaced the body with its “This content has been blocked”
// page.
//
// The remedy: the `allow-same-origin` token (always WITHOUT
// allow-scripts — the content stays inert, invariant S1 holds) makes
// the iframe's document accessible to the parent; the click is
// intercepted HERE, ALWAYS canceled (the iframe never navigates, even
// for a refused link), and the SYSTEM browser receives the link via
// `open_link` — whose Rust guard revalidates the scheme, this filter
// is only a convenience.
import { call } from './transport.js';

const SCHEMAS = new Set(['http:', 'https:', 'mailto:']);

// e2e seam (same pattern as __e2eAttachments): an array set on
// `window.__e2eLinks` captures the URLs instead of opening a real
// browser — the whole upstream path (iframe, interception, filter) is
// the real one. Outside e2e the variable does not exist.
function open(url) {
  // Compiled out of a release build (E7, see transport.js).
  const captured =
    import.meta.env.VITE_E2E === '1' ? globalThis.window?.__e2eLinks : undefined;
  if (captured !== undefined) {
    captured.push(url);
    return;
  }
  call('open_link', { url }).catch((err) => console.error('open_link :', err));
}

// To be wired to the iframe's `onload`: each srcdoc assignment
// loads a NEW document, the listener is set again on every load.
// In capture phase: no content can slip in ahead of it.
export function wireLinks(iframe) {
  const doc = iframe?.contentDocument;
  if (!doc) return;
  // PLAN-AUDIT-V2 E11: the product's shortcuts (e, Delete, /, Escape,
  // j/k…) live on the PARENT window; a click in a body moved the focus
  // there and made them inert. Every key struck in the iframe's
  // document is REPLAYED on the parent window — same key, same
  // modifiers, without hindering the native behavior (copy, scroll).
  doc.addEventListener('keydown', (ev) => {
    window.dispatchEvent(new KeyboardEvent('keydown', {
      key: ev.key, code: ev.code, ctrlKey: ev.ctrlKey, shiftKey: ev.shiftKey,
      altKey: ev.altKey, metaKey: ev.metaKey, bubbles: true, cancelable: true,
    }));
  });
  doc.addEventListener(
    'click',
    (ev) => {
      // No `instanceof Element`: the target lives in the iframe's
      // realm, not the parent's.
      const anchor = ev.target?.closest?.('a[href]');
      if (!anchor) return;
      ev.preventDefault();
      let link;
      try {
        // The raw attribute, never the resolved property: a relative
        // href points nowhere in a mail — ignored.
        link = new URL(anchor.getAttribute('href'));
      } catch {
        return;
      }
      if (SCHEMAS.has(link.protocol)) open(link.href);
    },
    true,
  );
}
