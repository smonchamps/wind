// E4 bench (PLAN-RETOURS-V3 R4): measures the thickness RESERVED by
// scrollbars in the real app, launched by the e2e harness
// with production arguments (browser-args.mjs).
//
// Two probes on an off-screen div:
// - `native`: NON-DEFAULT `scrollbar-color` set — Chromium switches
//   the element onto the standard path, the scrollbar is the browser's
//   (`auto` wouldn't be enough: that's the default value, a possible
//   webkit rule would still apply). Overlay => 0; classic => ~15 px.
// - `custom`: property removed — the path the app actually
//   uses. MUST render 0 since A44: a non-zero value signs the
//   return of a `::-webkit-scrollbar` rule or a classic scrollbar,
//   the exact regression the amendment forbids.
//
// Verdicts on record (2026-08-16, PLAN-RETOURS-V3 § 2 bis): webkit A7
// 10 px; classic 15 px; msOverlayScrollbarWinStyle alone 15 px (not
// honored); OverlayScrollbar 0 px — adopted.
//
// Usage: node measure-scrollbar.mjs [fluent]
//   `fluent` retries the Fluent Windows style ON TOP OF the overlay —
//   the feature list is rewritten WHOLE: a repeated `--enable-features`
//   is not merged by Chromium, the last one wins.
import { launchAppV2, closeApp } from './launch.mjs';

if (process.argv.includes('fluent')) {
  process.env.WIND_E2E_ARGS_EXTRA =
    '--enable-features=OverlayScrollbar,msOverlayScrollbarWinStyle';
}

const { app, browser, page } = await launchAppV2();
try {
  const measure = await page.evaluate(() => {
    const d = document.createElement('div');
    d.style.cssText =
      'position:fixed;left:-500px;top:0;width:100px;height:100px;' +
      'overflow:scroll;scrollbar-color:rgb(1,2,3) rgb(4,5,6);';
    document.body.appendChild(d);
    const native = d.offsetWidth - d.clientWidth;
    d.style.scrollbarColor = '';
    const custom = d.offsetWidth - d.clientWidth;
    d.remove();
    return { native, custom };
  });
  console.log(JSON.stringify(measure));
} finally {
  await closeApp({ app, browser });
}
