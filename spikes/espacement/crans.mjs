// Spike JETABLE (PLAN-ESPACEMENT, STOP visuel d'E1) — les trois crans
// d'espacement, capturés dans l'APPLICATION RÉELLE avec son décor, et
// non sur une maquette : c'est le rendu que le Chef Ingénieur jugera.
//
//   node spikes/espacement/crans.mjs
//
// Écrit trois PNG dans ce répertoire (cran-faible/moyen/eleve.png). Le
// jeton était forcé à la main sur le cadre au moment du STOP visuel
// d'E1 : le réglage d'interface n'existait pas encore, et l'œil n'avait
// pas à l'attendre pour trancher.
import path from 'node:path';
import { launchAppV2, closeApp } from '../../e2e/launch.mjs';

const CRANS = [
  ['faible', 13],
  ['moyen', 19],
  ['eleve', 25],
];

// Les PNG restent DANS le répertoire du spike : la racine du dépôt
// n'est pas ignorée par git, et les autres bancs versés écrivent tous
// chez eux (revue du 2026-08-25).
const racine = import.meta.dirname;
const { app, browser, page } = await launchAppV2();

try {
  await page.locator('[data-testid="ligne"]').first().waitFor();
  // La liste seule : c'est d'elle qu'on juge l'air, pas de la fenêtre.
  const liste = page.locator('[data-testid="liste"]');

  for (const [nom, px] of CRANS) {
    await page.evaluate((v) => {
      document.querySelector('.cadre')?.style.setProperty('--rangee-pad', `${v}px`);
    }, px);
    // Laisser le rendu se poser avant de photographier.
    await page.waitForTimeout(150);
    const fichier = path.join(racine, `cran-${nom}.png`);
    await liste.screenshot({ path: fichier });
    const h = await page.evaluate(() => {
      const l = document.querySelector('[data-testid="ligne"]');
      return l ? l.offsetHeight : null;
    });
    console.log(`${nom.padEnd(7)} padding ${String(px).padStart(2)} px  ->  `
      + `rangee ${h} px  ->  ${path.relative(racine, fichier)}`);
  }
} finally {
  await closeApp({ app, browser });
}
