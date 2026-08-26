// L'ordre de la rafale de démarrage (PLAN-DEMARRAGE, E2).
//
// Constat terrain du 2026-08-26, run FROID sur la base réelle (12,84 Go,
// 251 524 enveloppes, 64 boîtes) : `list_category` était ÉMISE à 89,6 ms
// et SERVIE à 440,6 ms — 350 ms, pour un travail propre de 28 ms. Entre
// les deux, sept sondes de démarrage tenaient le verrou global à tour de
// rôle. La seule commande dont l'utilisateur attend le résultat était la
// dernière servie.
//
// La cause n'était pas un mauvais ordre écrit : c'est que `prete = true`
// (App.svelte) ne flushe PAS tout de suite — Svelte planifie le rendu par
// microtâche. Les dix appels qui suivent partaient donc AVANT que
// `<Liste>` ne soit monté, et la première page arrivait dixième. Un
// `await tick()` rend la main au flush : la liste demande sa page la
// PREMIÈRE, seule, sans concurrent au verrou.
//
// CE QUE CE TEST PROUVE — et rien de plus : l'ordre d'ÉMISSION des
// commandes au démarrage. Il ne prouve PAS l'ordre de SERVICE :
// `hors_pompe` prend un `std::sync::Mutex` depuis un `spawn_blocking`, et
// un mutex n'est pas équitable. La preuve opposable du gain reste le
// palier mesuré au banc sur base à masse réelle.
//
// CE QU'IL N'OBSERVE PAS : le lanceur s'attache à une page déjà vivante,
// donc l'assertion porte sur une RECHARGE — cache WebView2 chaud, base
// déjà adoptée, comptes déjà connectés. Le démarrage à froid se mesure au
// banc (`spikes/demarrage`), jamais ici.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

// Les sondes du démarrage qui prennent le verrou global. Liste FIGÉE, et
// écrite en dur à dessein : une formule du genre « aucune sonde
// périodique » serait vraie par construction (les intervalles ne tirent
// qu'à 5 s au plus tôt) et ne pourrait jamais échouer.
const SONDES = [
  'nav_snapshot',
  'reperes_get',
  'noms_get',
  'sync_progress',
  'outbox_status',
  'list_drafts',
  'telemetry_pending',
  'telemetry_consent_get',
];

test('la première page de la liste est demandée avant les sondes du démarrage', async () => {
  // Le journal doit exister AVANT la première ligne de JS applicative :
  // `appel()` le lit à chaque appel, et un journal posé après le
  // démarrage serait VIDE de tout ce qu'on veut observer — le test
  // passerait alors à vide. D'où `addInitScript` + `reload`.
  await page.addInitScript(() => {
    window.__e2eJournal = [];
  });
  await page.reload();
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 30000 });

  const journal = await page.evaluate(() =>
    window.__e2eJournal.map((releve) => releve.commande),
  );

  // --- Gardes anti-vacuité, AVANT toute assertion d'ordre -------------
  // Sans elles, un journal vide ou tronqué rendrait tout le reste vert.
  expect(journal.length).toBeGreaterThan(5);
  // `lang_get` part depuis main.js, AVANT le montage de App : s'il manque,
  // c'est que l'injection n'a pas devancé le module d'entrée et que le
  // journal ne voit pas le démarrage.
  expect(journal[0]).toBe('lang_get');
  // La frontière de `prete` : rien ne part avant que la sonde de
  // migration n'ait répondu (A41).
  expect(journal).toContain('migration_check');
  expect(journal).toContain('list_category');

  // --- L'assertion : la liste devant chaque sonde ---------------------
  // Sur l'INDEX, jamais sur `depart`/`arrivee` : sur ce décor tout se
  // règle en quelques millisecondes et une comparaison de temps serait
  // soit flaky, soit verte par bruit. L'ordre du tableau est le seul
  // contrat que `journal.push` garantit.
  const iListe = journal.indexOf('list_category');
  for (const sonde of SONDES) {
    const iSonde = journal.indexOf(sonde);
    // Trois commandes court-circuitent sous e2e : absente = rien à dire.
    if (iSonde === -1) continue;
    expect(
      iSonde,
      `${sonde} est émise AVANT la première page de la liste `
        + `(${iSonde} < ${iListe}) — la liste refait la queue derrière les sondes`,
    ).toBeGreaterThan(iListe);
  }
});
