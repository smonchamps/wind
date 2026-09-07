// The height of a body in an iframe follows the CONTENT (field finding
// A47), never a fixed template: the iframe is same-origin WITHOUT
// scripts (invariant S1) — the parent measures the sanitized document
// and sets the height. Re-measured on load (srcdoc set, images
// granted) and on WIDTH change only (text re-flow) — never on
// its own height set, so as not to loop the observer.
//
// Extracted from Thread.svelte at E5bis (the Feed in card view shows the
// same bodies): ONE gateway, never two copies that drift apart.
export function autoBody(iframe) {
  let width = 0;
  const measure = () => {
    const doc = iframe.contentDocument;
    if (!doc?.documentElement) return;
    iframe.style.height = '0';
    iframe.style.height = `${doc.documentElement.scrollHeight}px`;
  };
  const onLoad = () => {
    width = iframe.offsetWidth;
    measure();
  };
  iframe.addEventListener('load', onLoad);
  const observer = new ResizeObserver(() => {
    if (iframe.offsetWidth === width) return;
    width = iframe.offsetWidth;
    measure();
  });
  observer.observe(iframe);
  return {
    destroy() {
      observer.disconnect();
      iframe.removeEventListener('load', onLoad);
    },
  };
}
