// PLAN-RETOURS-7 : les quatre retours CE du 2026-08-21, joués sur le
// décor Clarity — (R1) le survol d'une pièce jointe DIT l'action,
// (R2) les fichiers joints AVANT le corps, (R3) l'écran 03 à plat
// comme le volet (A46 étendu), (R4) épingler une conversation en tête
// de la Réception (D3-D5).
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

test('les fichiers joints vivent AVANT le corps du message (R2)', async () => {
  // Le fil Vantis : son dernier message porte Contrat_Vantis_v4.pdf.
  await page.locator('[data-testid="row"]').first().click();
  const deplie = page.locator(
    '[data-testid="reading-pane"] [data-testid="message-expanded"]',
  );
  const fichiers = deplie.locator('[data-testid="reading-files"]');
  await expect(fichiers).toContainText('Contrat_Vantis_v4.pdf');
  // La preuve d'ORDRE (aucun test ne la portait) : la section des
  // fichiers PRÉCÈDE l'iframe du corps dans le flot du message.
  const avant = await deplie.evaluate((el) => {
    const fichiers = el.querySelector('[data-testid="reading-files"]');
    const corps = el.querySelector('iframe');
    return Boolean(
      fichiers.compareDocumentPosition(corps) & Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });
  expect(avant).toBe(true);
});

// (Les puces d'un ÉCHO restent inertes et sans voile — le voile n'est
// pas rendu sur un écho, et leur inertie est déjà gardée par « l'écho
// d'envoi dit ses destinataires et sa pièce », refonte-ecran02.)
test('un corps que le cœur ne sert pas se dit et se rejoue (PLAN-AUDIT-V2 E10)', async () => {
  // Le prochain `message_body` échoue (couture __e2eFailure) : le cadre
  // dit l'échec et offre « Réessayer » — avant, un cadre vide à vie.
  await page.evaluate(() => {
    window.__e2eFailure = ['message_body'];
  });
  await page.locator('[data-testid="row"]').nth(1).click();
  const echec = page.locator('[data-testid="reading-pane"] [data-testid="body-failure"]');
  await expect(echec).toBeVisible();
  await page.locator('[data-testid="body-retry"]').click();
  await expect(echec).toHaveCount(0);
  await expect(
    page.frameLocator('[data-testid="reading-pane"] [data-testid="message-expanded"] iframe').first().locator('body'),
  ).not.toBeEmpty();
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="reading-pane"] [data-testid="reading-files"]')).toBeVisible();
});

test('le survol d’une pièce jointe dit « Enregistrer » (R1, D1)', async () => {
  const piece = page
    .locator('[data-testid="reading-files"] [data-testid="attachment"]')
    .first();
  const voile = piece.locator('.veil');
  // Au repos : la puce dit le fichier, le voile n'existe pas à l'œil.
  await expect.poll(() => voile.evaluate((el) => getComputedStyle(el).display)).toBe('none');
  // Au survol : le voile COUVRE la puce — glyphe download + le mot du
  // produit (D1 : « Enregistrer », le clic ouvre « Enregistrer sous ») —
  // sans changer sa géométrie (la rangée ne reflue pas).
  const largeurAvant = await piece.evaluate((el) => el.offsetWidth);
  await piece.hover();
  // (`inline-flex` posé se calcule `flex` : l'absolu blockifie — on
  // asserte la présence, pas la valeur.)
  // PLAN-AUDIT-V2 E9 : réessayé — le voile suit le survol, pas l'instant.
  await expect.poll(() => voile.evaluate((el) => getComputedStyle(el).display)).not.toBe('none');
  await expect(voile).toContainText('Enregistrer');
  await expect(voile.locator('.ic')).toHaveAttribute('data-name', 'download');
  await expect.poll(() => piece.evaluate((el) => el.offsetWidth)).toBe(largeurAvant);
  // On quitte : le voile se retire.
  await page.locator('[data-testid="thread-subject"]').hover();
  await expect.poll(() => voile.evaluate((el) => getComputedStyle(el).display)).toBe('none');
});

test('l’écran 03 est À PLAT : chaque message dans son élévation, la conversation sans (R3, D2)', async () => {
  await page.locator('[data-testid="see-conversation"]').click();
  const conv = page.locator('[data-testid="conversation"]');
  await expect(conv.locator('[data-testid="thread-subject"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // Aucune élévation ni surface englobante entre la racine de l'écran
  // et l'objet-fil — « l'écran 03 garde sa carte pleine » (A46) est
  // renversé (RETOURS-7 R3).
  const englobantes = await conv.evaluate((racine) => {
    const objet = racine.querySelector('.thread-subject');
    const fautives = [];
    for (let el = objet.parentElement; el && el !== racine; el = el.parentElement) {
      const s = getComputedStyle(el);
      if (s.boxShadow !== 'none' || s.borderTopWidth !== '0px') {
        fautives.push(el.className);
      }
    }
    return fautives;
  });
  expect(englobantes).toEqual([]);
  // La tête du fil sans filet, comme au volet (garde jumelle d'A46).
  expect(
    await conv.locator('.head').first().evaluate((el) => getComputedStyle(el).borderBottomWidth),
  ).toBe('0px');
  // La scène défile en un seul flot, colonne de lecture bornée (D2).
  expect(await conv.locator('.scene').evaluate((el) => getComputedStyle(el).overflowY)).toBe(
    'auto',
  );
  expect(
    await conv.locator('.column').evaluate((el) => getComputedStyle(el).maxWidth),
  ).toBe('960px');
  // Les cartes de message, ELLES, gardent leur élévation.
  const ombre = await conv
    .locator('[data-testid="message-expanded"]')
    .first()
    .evaluate((el) => getComputedStyle(el).boxShadow);
  expect(ombre).not.toBe('none');
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(conv).toHaveCount(0);
});

test('épingler met la conversation en tête de la Réception — une seule ligne, réversible (R4, D3-D5)', async () => {
  // Une ligne du MILIEU de la liste : l'effet « remonte en tête » se
  // voit. On retient son objet pour la suivre.
  const ligne = page.locator('[data-testid="row"]').nth(2);
  const objet = (await ligne.locator('.subject').innerText()).trim();
  await ligne.click();
  const epingler = page.locator('[data-testid="reading-pane"] [data-testid="pin"]');
  await expect(epingler).toContainText('Épingler');
  await expect(epingler).toHaveAttribute('aria-pressed', 'false');
  await epingler.click();
  // Le bouton bascule, la section épinglée s'ouvre en tête avec SA
  // ligne — marquée « Épinglé » — et le flot ne la montre plus (D5 :
  // jamais deux fois la même conversation).
  await expect(epingler).toContainText('Désépingler');
  await expect(epingler).toHaveAttribute('aria-pressed', 'true');
  const section = page.locator('[data-testid="pins"]');
  await expect(section.locator('[data-testid="row"]')).toHaveCount(1);
  await expect(section).toContainText(objet);
  await expect(section.locator('[data-testid="chips-row"]')).toContainText('Épinglé');
  // Terrain (2026-08-21) : la ligne épinglée porte le DESSIN de la
  // tuile de la boîte en cours (nav, W2-D5) — même fond calculé.
  const fondDe = (loc) => loc.evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(await fondDe(section.locator('[data-testid="row"]'))).toBe(
    await fondDe(page.locator('[data-testid="nav-mailbox"][aria-current="true"]')),
  );
  await expect(
    page.locator('[data-testid="row"]', { hasText: objet }),
  ).toHaveCount(1);
  // Désépingler : la section se referme, la ligne reprend sa place de
  // date dans le flot.
  await epingler.click();
  await expect(epingler).toContainText('Épingler');
  await expect(section).toHaveCount(0);
  await expect(
    page.locator('[data-testid="row"]', { hasText: objet }),
  ).toHaveCount(1);
});

test('hors Réception, la barre du fil n’offre pas l’épingle (R4, D4)', async () => {
  await page
    .locator('[data-testid="nav-folder"][data-category="archive"]')
    .click();
  await page.locator('[data-testid="row"]').first().click();
  await expect(
    page.locator('[data-testid="reading-pane"] [data-testid="thread-subject"]'),
  ).toBeVisible();
  await expect(page.locator('[data-testid="pin"]')).toHaveCount(0);
});

test('« e » frappé dans le corps d’un message archive la conversation', async () => {
  // PLAN-AUDIT-V2 E11 : les raccourcis vivent sur la fenêtre PARENTE ;
  // un clic dans un corps les rendait inertes — chaque touche frappée
  // dans le cadre est rejouée sur la fenêtre.
  const rangee = page.locator('[data-testid="row"]').first();
  const sujet = (await rangee.locator('.subject').textContent()).trim();
  await rangee.click();
  // Le focus entre dans le cadre du corps (sans script, S1 : Playwright
  // ne peut rien y évaluer — on le focalise depuis le parent), puis la
  // VRAIE touche : c'est le rejeu vers la fenêtre parente qui est éprouvé.
  const cadre = page.locator('[data-testid="reading-pane"] [data-testid="message-expanded"] iframe').first();
  await expect(cadre).toBeVisible();
  await cadre.focus();
  await page.keyboard.press('e');
  await expect
    .poll(async () =>
      (await page.locator('[data-testid="row"] .subject').allTextContents()).map((s) => s.trim()).includes(sujet))
    .toBe(false);
});
