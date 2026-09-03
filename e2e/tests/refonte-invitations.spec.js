// PLAN-INVITATIONS — la carte d'invitation jouée sur le décor Clarity,
// en ISOLATION (sa propre instance) : « Atelier de septembre » (Sofia,
// compte travail) porte une invitation à répondre. Le décor est HORS
// LIGNE par construction : répondre journalise l'email iTIP dans la
// boîte d'envoi (règles d'or), la carte et le rang de liste disent la
// réponse — c'est tout le chemin réel sauf la remise SMTP. L'ICS du
// décor traverse le VRAI parseur (mail-ical) en UTC : « 14:30 – 16:00 »
// s'affiche en heure du poste, déterministe quel que soit le run.
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

const ligneAtelier = () =>
  page.locator('[data-testid="row"]', { hasText: 'Atelier de septembre' }).first();

const ouvrirAtelier = async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  await ligneAtelier().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Atelier de septembre');
  return page.locator('[data-testid="reading-pane"] [data-testid="invitation"]');
};

test('la carte d’invitation se montre : titre, horaire local, organisateur, trois gestes', async () => {
  const carte = await ouvrirAtelier();
  await expect(carte).toBeVisible();
  await expect(carte.locator('[data-testid="invitation-title"]')).toHaveText(
    'Atelier de septembre',
  );
  // L'HORAIRE traverse le vrai parseur (ICS en UTC → heure du poste),
  // la virgule de LOCATION est DÉSÉCHAPPÉE au passage.
  await expect(carte).toContainText('14:30 – 16:00');
  await expect(carte).toContainText('Grande salle, Atelier Nord');
  await expect(carte).toContainText('Sofia Nardi');
  await expect(carte.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous n’avez pas répondu',
  );
  // Trois boutons NEUTRES (D4), aucun pressé.
  for (const geste of ['inv-accept', 'inv-tentative', 'inv-refuse']) {
    await expect(carte.locator(`[data-testid="${geste}"]`)).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  }
  // La carte PRÉCÈDE le corps dans le contenu (A76 : elle est l'objet
  // du message) — même garantie d'ordre DOM qu'A71.
  const ordre = await page
    .locator('[data-testid="reading-pane"] [data-testid="message-expanded"] .content > *')
    .evaluateAll((noeuds) => noeuds.map((n) => n.dataset.testid ?? n.tagName));
  expect(ordre[0]).toBe('invitation');
});

test('R10 : répondre DEPUIS la liste — le rang porte les gestes, puis la puce', async () => {
  // Quitter le fil de l'atelier : le geste se joue SANS l'ouvrir.
  await page
    .locator('[data-testid="row"]', { hasText: 'Planning de la semaine 33' })
    .first()
    .click();
  // R3'c : les gestes occupent leur RANG à eux (puces-invitation).
  const gestes = ligneAtelier().locator('[data-testid="chips-invitation"]');
  await expect(gestes.locator('[data-testid="list-accept"]')).toBeVisible();
  await expect(gestes.locator('[data-testid="list-refuse"]')).toBeVisible();

  await gestes.locator('[data-testid="list-tentative"]').click();
  // La puce remplace les gestes À L'INSTANT (optimiste) — et la ligne
  // n'a PAS été choisie.
  await expect(
    ligneAtelier().locator('[data-testid="invitation-chip"]'),
  ).toContainText('Provisoire');
  await expect(gestes).toHaveCount(0);
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toHaveText('Planning de la semaine 33');

  // La carte relit la même vérité (la base, pas un état d'écran).
  const carte = await ouvrirAtelier();
  await expect(carte.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez répondu provisoirement',
  );
  await expect(carte.locator('[data-testid="inv-tentative"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
});

test('D6 : changer d’avis depuis la carte — refuser puis accepter', async () => {
  const carte = await ouvrirAtelier();
  await carte.locator('[data-testid="inv-refuse"]').click();
  await expect(carte.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez refusé',
  );
  await carte.locator('[data-testid="inv-accept"]').click();
  await expect(carte.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez accepté',
  );
  await expect(carte.locator('[data-testid="inv-refuse"]')).toHaveAttribute(
    'aria-pressed',
    'false',
  );
});

test('R11 : la liste rechargée dit la réponse en puce — la réponse survit à la navigation', async () => {
  // Une page fraîche de la Réception (aller-retour de dossier) : le
  // rang vient de l'enrichissement du cœur, pas d'un état local.
  await page.locator('[data-testid="nav-folder"][data-category="archive"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(
    ligneAtelier().locator('[data-testid="invitation-chip"]'),
  ).toContainText('Acceptée');
  const carte = await ouvrirAtelier();
  await expect(carte.locator('[data-testid="invitation-status"]')).toHaveText(
    'Vous avez accepté',
  );
});
