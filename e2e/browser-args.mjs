// Browser arguments for the e2e launchers and the benches — ONE source:
// `additionalBrowserArgs` from apps/desktop/tauri.conf.json (review
// 2026-08-16). The WebView2 loader has the config OVERWRITTEN by the
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS environment variable: any process
// that sets it must therefore REUSE the production arguments, or
// else it tests a different browser than the one shipped (SmartScreen and
// OOBE active, classic scrollbars instead of the A44 overlay).
//
// Chromium trap: a repeated flag (two `--enable-features=`) is
// not merged — the LAST one wins. A supplement that wants to add a
// feature must rewrite the complete list, commas included.
import { readFileSync } from 'node:fs';
import path from 'node:path';

export function browserArgs(root, port, extra = '', lang = 'en') {
  const conf = JSON.parse(
    readFileSync(path.join(root, 'apps', 'desktop', 'tauri.conf.json'), 'utf8'),
  );
  const prod = conf.app.windows[0].additionalBrowserArgs ?? '';
  return [prod, `--remote-debugging-port=${port}`, `--lang=${lang}`, extra]
    .filter(Boolean)
    .join(' ');
}
