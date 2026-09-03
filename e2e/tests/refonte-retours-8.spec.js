// PLAN-RETOURS-8 : R1 — le repère de compte (icône + teinte, Réglages >
// Comptes → nav → badge de liste, D3 : boîte unifiée seule) et R2 — le
// parcours de premier démarrage en cinq étapes (comptes, volets,
// thème, bêta, fin — A91). Décor : deux comptes réels — le badge n'a de sens qu'en
// multi-comptes.
//
// Hygiène : le profil WebView2 est PARTAGÉ entre suites — les clés
// localStorage touchées (accueil, volets, largeurs, thème) sont
// retirées avant ET après. Le parcours complet se force par la couture
// `__e2eOnboarding` (un décor semé est sinon réputé « déjà accueilli » —
// c'est le comportement de production voulu pour les mises à jour).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgerLocales } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });


test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [
      { email: 'un@exemple.fr', messages: 6 },
      { email: 'deux@exemple.fr', messages: 4 },
    ],
  }));
  await purgerLocales(page);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgerLocales(page);
  await closeApp({ app, browser });
});

const boiteNav = (libelle) =>
  page.locator('[data-testid="nav-mailbox"]', { hasText: libelle });

// ---------------------------------------------------------------- R2 --
// Le chemin de PRODUCTION d'une mise à jour : des comptes déjà là, la
// clé absente — l'installation est réputée accueillie, la clé se pose,
// aucun parcours (c'est l'état laissé par le beforeAll : purge + reload).

test('une installation existante est réputée accueillie — jamais de parcours', async () => {
  await expect(page.locator('[data-testid="onboarding"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe('1');
});

// ---------------------------------------------------------------- R1 --

test('poser un repère depuis Réglages > Comptes le montre dans la nav', async () => {
  // Sans repère : les boîtes de compte portent le glyphe neutre, aucune
  // pastille nulle part.
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(0);

  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-marker"]').first().click();
  await expect(page.locator('[data-testid="settings-marker"]')).toBeVisible();

  // Un repère n'existe qu'ENTIER : l'icône seule ne pose rien.
  await page.locator('[data-testid="marker-icon"][data-icon="home"]').click();
  await page.locator('[data-testid="marker-hue"][data-color="blue"]').click();
  // La rangée reflète l'état persisté (la pastille remplace `person`).
  await expect(
    page.locator('[data-testid="account-marker"] .marker').first(),
  ).toHaveAttribute('data-hue', 'blue');
  await page.locator('[data-testid="settings-done"]').click();

  // La nav : la boîte du compte porte le TRACÉ du repère (A82 — glyphe
  // nu à la teinte, plus jamais une pastille pleine), l'autre compte
  // reste au glyphe neutre.
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveClass(/bare-marker/);
  await expect(page.locator('[data-testid="nav-marker"]')).toHaveAttribute(
    'data-hue',
    'blue',
  );
  await expect(page.locator('[data-testid="nav-marker"] .ic')).toHaveAttribute('data-name', 'home');
});

test('le bloc de boîte ne vit qu’en boîte unifiée (D3/D7) — et sur TOUTES les rangées (D8)', async () => {
  // Boîte unifiée (défaut) : CHAQUE rangée dit sa boîte en toutes
  // lettres (A80/D8 — un compte sans repère n'est plus indiscernable) ;
  // le tracé, lui, n'apparaît que sur les lignes du compte au repère.
  const blocs = page.locator('[data-testid="row-mailbox"]');
  await expect(blocs.first()).toBeVisible();
  const nLignes = await page.locator('[data-testid="row"]').count();
  await expect(blocs).toHaveCount(nLignes);
  const traces = page.locator('[data-testid="row-mailbox"] .bare-marker');
  await expect(traces.first()).toHaveAttribute('data-hue', 'blue');
  const nTraces = await traces.count();
  expect(nTraces).toBeGreaterThan(0);
  expect(nTraces).toBeLessThan(nLignes);

  // Le volet de lecture porte le MÊME objet (D5/A82) : le tracé du
  // repère, pas une pastille — c'est la seule surface où le tracé du
  // fil est vérifié.
  const auRepere = page
    .locator('[data-testid="row"]')
    .filter({ has: page.locator('[data-testid="row-mailbox"] .bare-marker') })
    .first();
  await auRepere.click();
  const volet = page.locator('[data-testid="reading-pane"]');
  await expect(volet.locator('.mailbox .bare-marker').first()).toBeVisible();
  await expect(volet.locator('.mailbox .bare-marker').first()).toHaveAttribute(
    'data-hue',
    'blue',
  );

  // Vue d'un seul compte : le bloc n'a plus rien à dire — aucun (D7).
  await boiteNav('un@exemple.fr').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(0);

  // Retour à la boîte unifiée pour la suite.
  await boiteNav('Toutes les boîtes').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('retirer le repère rend le glyphe neutre — le bloc reste, sans tracé', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="account-marker"]').first().click();
  await page.locator('[data-testid="marker-remove"]').click();
  await expect(page.locator('[data-testid="marker-remove"]')).toHaveCount(0);
  await page.locator('[data-testid="settings-done"]').click();

  await expect(page.locator('[data-testid="nav-marker"]')).toHaveCount(0);
  // A80/D8 : retirer le repère retire le TRACÉ, jamais le bloc — la
  // boîte se dit en toutes lettres, repère ou non.
  const nLignes = await page.locator('[data-testid="row"]').count();
  await expect(page.locator('[data-testid="row-mailbox"]')).toHaveCount(nLignes);
  await expect(page.locator('[data-testid="row-mailbox"] .bare-marker')).toHaveCount(0);
});

// ---------------------------------------------------------------- R2 --

test('le parcours de premier démarrage : cinq étapes, retour compris', async () => {
  // La couture force le parcours sur ce décor semé — et sous elle,
  // RIEN ne s'écrit dans le profil (accueil.js) : le vrai chemin de la
  // clé est prouvé par le test « installation existante » ci-dessus et
  // par le test de reprise ci-dessous.
  await page.addInitScript(() => {
    window.__e2eOnboarding = true;
  });
  // La clé posée par la décision du beforeAll est retirée : l'assertion
  // finale « toujours null » prouve alors que la couture n'écrit RIEN.
  await page.evaluate(() => localStorage.removeItem('wind-accueil-fait'));
  await page.reload();

  // Étape 1 : les comptes existants sont listés, Continuer actif (D4 :
  // au moins un compte — il y en a deux).
  const accueil = page.locator('[data-testid="onboarding"]');
  await expect(accueil).toBeVisible();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  await expect(page.locator('[data-testid="onboarding-accounts"]')).toContainText(
    'un@exemple.fr',
  );
  // Constat 2 (terrain 2026-08-22) : des adresses existent — la barre
  // d'ajout est repliée derrière « Ajouter une autre adresse email »,
  // et le clic la rouvre.
  await expect(page.locator('[data-testid="onboarding-address"]')).toHaveCount(0);
  await page.locator('[data-testid="onboarding-add-other"]').click();
  await expect(page.locator('[data-testid="onboarding-address"]')).toBeVisible();
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Étape 2 : les trois aperçus de volets. Retour d'abord : l'étape 1
  // revient, comptes toujours là — la progression ne se perd pas.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 2/5',
  );
  await page.locator('[data-testid="onboarding-back"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  await expect(page.locator('[data-testid="onboarding-accounts"]')).toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Choisir « deux volets » s'applique immédiatement (appliquerVolets)
  // et l'aperçu UNIQUE (2e passe, constat 3) suit le choix.
  await expect(page.locator('[data-testid="onboarding-pane"]')).toHaveCount(3);
  await page.locator('[data-testid="onboarding-pane"][data-panes="2"]').click();
  await expect(
    page.locator('[data-testid="onboarding-pane"][data-panes="2"]'),
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-testid="onboarding-preview"]')).toHaveAttribute(
    'data-panes',
    '2',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Étape 3 : les quatre fiches en aperçu (V7 amendée, A94) ; choisir
  // « Elements · nuit » pose le thème sur l'instant (data-theme sur la
  // racine).
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 3/5',
  );
  await expect(page.locator('[data-testid="onboarding-theme"]')).toHaveCount(4);
  await page.locator('[data-testid="onboarding-theme"][data-theme-id="elements-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'elements-nuit');
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Étape 4 (RETOURS-11, terrain bêta) : Wind est en bêta — l'étape
  // présente le bouton Feedback de l'entête et ce qu'il fait.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 4/5',
  );
  await expect(page.locator('[data-testid="onboarding-beta"]')).toContainText('Feedback');
  await page.locator('[data-testid="onboarding-continue"]').click();

  // Étape 5 : le récapitulatif (constat 8) — les trois choix, chacun
  // porte vers son étape. Le clic sur « Disposition » y RETOURNE, puis
  // le parcours revient.
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 5/5',
  );
  const recap = page.locator('[data-testid="onboarding-recap"]');
  await expect(recap).toContainText('un@exemple.fr');
  await expect(page.locator('[data-testid="recap-panes"]')).toContainText(
    'Deux volets',
  );
  await expect(page.locator('[data-testid="recap-theme"]')).toContainText(
    'Elements · nuit',
  );
  await page.locator('[data-testid="recap-panes"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 2/5',
  );
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 5/5',
  );
  // Terminer ouvre la fenêtre standard — en DEUX volets (le choix de
  // l'étape 2 a tenu) : pas de volet de lecture dans la grille. Sous la
  // couture, la clé ne se pose PAS (aucune pollution du profil).
  await page.locator('[data-testid="onboarding-finish"]').click();
  await expect(accueil).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe(null);
});

test('un parcours abandonné à mi-course REPREND — jamais réputé accueilli', async () => {
  // Le vrai chemin, SANS couture : la marque « commencé » est là (le
  // parcours s'était affiché), la clé « fait » absente (jamais de
  // Terminer), des comptes existent (ajouté à l'étape 1 avant de
  // quitter). Au lancement suivant, le parcours reprend — l'heuristique
  // « des comptes ⇒ déjà accueilli » ne l'avale pas (revue 2026-08-22).
  await page.addInitScript(() => {
    delete window.__e2eOnboarding;
  });
  await page.evaluate(() => {
    localStorage.removeItem('wind-accueil-fait');
    localStorage.setItem('wind-accueil-commence', '1');
  });
  await page.reload();
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  await expect(page.locator('[data-testid="onboarding-progress"]')).toHaveText(
    'Étape 1/5',
  );
  // Terminer proprement : la clé se pose (le VRAI chemin d'écriture),
  // l'app revient aux suites suivantes.
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-continue"]').click();
  await page.locator('[data-testid="onboarding-finish"]').click();
  await expect(page.locator('[data-testid="onboarding"]')).toHaveCount(0);
  expect(
    await page.evaluate(() => localStorage.getItem('wind-accueil-fait')),
  ).toBe('1');
});
