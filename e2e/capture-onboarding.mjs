// REAL screenshots of the three layouts (field 2026-08-22,
// finding 5 of PLAN-RETOURS-8 R2): the "layout" step of the onboarding
// journey shows the application itself (Clarity decor, fake
// data), never an approximate drawing. To REPLAY when screen 02
// changes face, then commit the regenerated PNGs:
//
//   node capture-onboarding.mjs        (from e2e/)
//
// Writes apps/desktop/ui-v2/src/assets/accueil/disposition-{3,2,1}.png
// (imported by Onboarding.svelte, bundled by Vite).
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { launchAppV2, closeApp } from './launch.mjs';

const folder = path.resolve(
  import.meta.dirname,
  '..',
  'apps',
  'desktop',
  'ui-v2',
  'src',
  'assets',
  'accueil',
);
mkdirSync(folder, { recursive: true });

// The shipped illustrations are FRENCH screenshots (assets/accueil): the
// suite launches English since E6b (D22), this script pins French until the
// Chief Engineer decides their language (PLAN-ENGLISH-SWITCH D28).
const { app, browser, page } = await launchAppV2({ lang: 'fr' });
try {
  for (const panes of [3, 2, 1]) {
    await page.evaluate((v) => {
      localStorage.setItem('wind-volets', String(v));
      localStorage.setItem('wind-accueil-fait', '1');
    }, panes);
    await page.reload();
    await page.locator('[data-testid="row"]').first().waitFor();
    if (panes === 3) {
      // The reading pane shows an open message — the preview says the
      // real working window, not an empty column.
      await page.locator('[data-testid="row"]').first().click();
      await page.locator('[data-testid="reading-pane"]').waitFor();
    }
    // Paint settled (fonts, hitofude) before the photo.
    await page.waitForTimeout(500);
    // Cropped ABOVE the status bar: the fake accounts don't
    // sync, and "Sync unavailable" has no business
    // being in an onboarding preview.
    const status = await page.locator('[data-testid="status"]').boundingBox();
    const view = await page.evaluate(() => ({
      width: window.innerWidth,
      height: window.innerHeight,
    }));
    await page.screenshot({
      path: path.join(folder, `disposition-${panes}.png`),
      clip: { x: 0, y: 0, width: view.width, height: status?.y ?? view.height },
    });
    console.log(`disposition-${panes}.png captured`);
  }
} finally {
  await page
    .evaluate(() => {
      localStorage.removeItem('wind-volets');
      localStorage.removeItem('wind-accueil-fait');
    })
    .catch(() => {});
  await closeApp({ app, browser });
}
