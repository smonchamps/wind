// PLAN-RETOURS-10 R1 : la sélection multiple de la liste — Ctrl-clic,
// Shift-clic, case au survol — et la barre d'actions groupées (D1-D4,
// D6, verdicts terrain du 2026-08-27).
//
// Le filet vise ce que l'utilisateur VOIT (leçon PLAN-ESPACEMENT : un
// filet se prouve en le cassant) : la barre transformée et son compte,
// la case cochée, la pastille de non-lus de la nav, les lignes qui
// quittent la boîte. ORDRE PENSÉ (suite sérielle, base isolée par
// spec) : le marquage lu/non-lu joue EN PREMIER — depuis le terrain
// R1-1, le Ctrl-clic déplace le focus de lecture donc MARQUE LU, et
// tout test qui le joue avant fausserait la pastille ; les gestes
// destructifs jouent EN DERNIER.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const lignes = () => page.locator('[data-testid="ligne"]');
const barre = () => page.locator('[data-testid="barre-selection"]');
const cochees = () =>
  page.locator('[data-testid="ligne-case"][aria-checked="true"]');
const caseDe = (i) => lignes().nth(i).locator('[data-testid="ligne-case"]');
const dossier = (categorie) =>
  page.locator(`[data-testid="nav-dossier"][data-categorie="${categorie}"]`);
const toast = () => page.locator('[data-testid="toast"]');

// La couleur résolue de --sel, pour comparer des fonds calculés.
const teinteSel = () =>
  page.evaluate(() => {
    const d = document.createElement('div');
    d.style.background = 'var(--sel)';
    document.body.appendChild(d);
    const v = getComputedStyle(d).backgroundColor;
    d.remove();
    return v;
  });

test('marquer lu groupé par les cases : la pastille tombe — puis non-lu la relève', async () => {
  // Le décor Clarity porte 4 non-lus en Réception (refonte-ecran02).
  const pastille = dossier('reception').locator('.pastille');
  await expect(pastille).toHaveText('4');
  // On coche par la CASE (elle ne choisit pas, donc ne marque rien au
  // passage), et par la case non cochée RESTANTE — jamais par index
  // sur un locator vivant : les rangées sont clées par index et une
  // resservie en cours de boucle ferait viser (et DÉ-cocher) une autre
  // rangée — le profil de flake local exact (revue).
  const aCocher = page.locator(
    '[data-testid="ligne"].nonlu [data-testid="ligne-case"][aria-checked="false"]',
  );
  while ((await aCocher.count()) > 0) {
    await aCocher.first().click();
  }
  await page.locator('[data-testid="barre-lu"]').click();
  await expect(toast()).toContainText('marquées lues');
  // Le geste abouti vide la sélection, et la nav dit le nouveau compte.
  await expect(barre()).toHaveCount(0);
  await expect(pastille).toHaveCount(0);
  // Non-lu groupé sur une rangée SANS fil (un fil re-marqué non lu
  // compterait tous ses messages — D6) : la pastille remonte à 1.
  const simple = lignes()
    .filter({ hasNot: page.locator('.puce', { hasText: /message/ }) })
    .first();
  await simple.locator('[data-testid="ligne-case"]').click();
  await page.locator('[data-testid="barre-nonlu"]').click();
  await expect(pastille).toHaveText('1');
});

test('Ctrl-clic coche ET déplace le focus de lecture (terrain R1-1) ; Annuler vide', async () => {
  const sujet0 = (await lignes().nth(0).locator('.objet').textContent()).trim();
  await lignes().nth(0).click({ modifiers: ['Control'] });
  // La barre de la liste se transforme (D3) : le compte + les actions.
  await expect(barre()).toBeVisible();
  await expect(barre()).toContainText('1 sélectionné');
  await expect(cochees()).toHaveCount(1);
  // Terrain R1-1 : le liseré ET le volet suivent la rangée Ctrl-cliquée.
  await expect(lignes().nth(0)).toHaveClass(/choisie/);
  await expect(
    page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]'),
  ).toContainText(sujet0);
  // Ctrl-clic ailleurs ajoute — et le focus suit encore ; sur une
  // cochée, il retire (bascule).
  await lignes().nth(2).click({ modifiers: ['Control'] });
  await expect(barre()).toContainText('2 sélectionnés');
  await expect(lignes().nth(2)).toHaveClass(/choisie/);
  await expect(lignes().nth(0)).not.toHaveClass(/choisie/);
  await lignes().nth(2).click({ modifiers: ['Control'] });
  await expect(barre()).toContainText('1 sélectionné');
  // Annuler rend la liste au repos : bandeau de titre, zéro case.
  await page.locator('[data-testid="barre-annuler"]').click();
  await expect(barre()).toHaveCount(0);
  await expect(page.locator('[data-testid="liste-titre"]')).toBeVisible();
  await expect(cochees()).toHaveCount(0);
});

test('Shift-clic étend depuis la rangée sélectionnée (terrain R1-2)', async () => {
  // Le clic NU choisit la première rangée — c'est elle l'ancre : le
  // scénario exact du constat (le premier message sélectionné par
  // défaut, puis Shift-clic plus bas → toute la plage se coche).
  await lignes().nth(0).click();
  await lignes().nth(3).click({ modifiers: ['Shift'] });
  await expect(barre()).toContainText('4 sélectionnés');
  await expect(cochees()).toHaveCount(4);
  await page.locator('[data-testid="barre-annuler"]').click();
  await expect(barre()).toHaveCount(0);
});

test("la case vit au survol, coche sans choisir, et le contenu s'écarte (D4, terrain R1-3)", async () => {
  // Au repos la case est invisible (opacité 0 — elle reste au DOM) ;
  // le survol la révèle ET écarte le contenu (padding 16 → 34 px, la
  // hauteur ne bouge pas).
  const opacite = (loc) => loc.evaluate((el) => getComputedStyle(el).opacity);
  const padGauche = (i) =>
    lignes().nth(i).evaluate((el) => getComputedStyle(el).paddingLeft);
  expect(await opacite(caseDe(1))).toBe('0');
  expect(await padGauche(1)).toBe('16px');
  await lignes().nth(1).hover();
  await expect.poll(async () => opacite(caseDe(1))).toBe('1');
  await expect.poll(() => padGauche(1)).toBe('34px');
  await caseDe(1).click();
  await expect(barre()).toContainText('1 sélectionné');
  // La case ne choisit pas : le liseré n'a pas bougé sur cette rangée.
  await expect(lignes().nth(1)).not.toHaveClass(/choisie/);
  // Dès qu'une sélection existe, TOUTES les cases se montrent et
  // toutes les rangées s'écartent d'un bloc (D4) — mesuré sur une
  // rangée non survolée ni cochée.
  await expect.poll(() => opacite(caseDe(3))).toBe('1');
  await expect.poll(() => padGauche(3)).toBe('34px');
  await page.locator('[data-testid="barre-annuler"]').click();
});

test('la sélection se vide au changement de dossier', async () => {
  await caseDe(0).click();
  await expect(barre()).toBeVisible();
  await dossier('archives').click();
  await expect(barre()).toHaveCount(0);
  await dossier('reception').click();
  await expect(lignes().first()).toBeVisible();
  await expect(barre()).toHaveCount(0);
});

test('une épinglée cochée se teinte comme les autres (terrain R1-7)', async () => {
  // Épingler la première rangée par la barre du fil, puis la cocher
  // dans sa section : son fond doit être LA teinte de sélection — le
  // sol --tuile d'A73 cède à la coche (verdict terrain).
  await lignes().nth(0).click();
  await page.locator('[data-testid="epingler"]').click();
  const ep = page.locator('[data-testid="epingles"] [data-testid="ligne"]').first();
  await expect(ep).toBeVisible();
  await ep.locator('[data-testid="ligne-case"]').click();
  await expect(barre()).toContainText('1 sélectionné');
  expect(await ep.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(
    await teinteSel(),
  );
  await page.locator('[data-testid="barre-annuler"]').click();
  // Remise en état : désépingler (la rangée est encore la sélection de
  // lecture, la barre du fil est ouverte sur elle).
  await page.locator('[data-testid="epingler"]').click();
  await expect(page.locator('[data-testid="epingles"]')).toHaveCount(0);
});

test("le raccourci « e » archive le LOT coché (terrain R1-8)", async () => {
  const sujets = await lignes().locator('.objet').allTextContents();
  const partants = [sujets[2].trim(), sujets[3].trim()];
  await caseDe(2).click();
  await caseDe(3).click();
  await page.keyboard.press('e');
  await expect(toast()).toContainText('2 conversations archivées');
  await expect(barre()).toHaveCount(0);
  await expect
    .poll(async () => {
      const restants = (await lignes().locator('.objet').allTextContents()).map((s) => s.trim());
      return partants.filter((s) => restants.includes(s)).length;
    })
    .toBe(0);
});

// D6 (CE, 2026-08-27) : un geste de masse emporte le FIL ENTIER — la
// rangée 0 du décor est un fil de 3 messages (Vantis) : c'est LE cas
// qui a fait échouer la première version de ce test (le fil revenait,
// amputé d'un message) — le filet est prouvé non-vacant par cette
// histoire, ne pas le re-filtrer sur des rangées « simples ».
test('archiver groupé : un seul toast, les fils partent ENTIERS (D6)', async () => {
  const sujets = await lignes().locator('.objet').allTextContents();
  const partants = [sujets[0].trim(), sujets[1].trim()];
  await caseDe(0).click();
  await caseDe(1).click();
  // PLAN-AUDIT-V2 E6 : UN appel au cœur pour le lot — plus N × k
  // commandes unitaires en série (250 + 50 IPC pour 50 conversations).
  await page.evaluate(() => {
    window.__e2eJournal = [];
  });
  await page.locator('[data-testid="barre-archiver"]').click();
  await expect(toast()).toContainText('2 conversations archivées');
  const gestes = await page.evaluate(() => {
    const commandes = window.__e2eJournal.map((releve) => releve.commande);
    delete window.__e2eJournal;
    return commandes;
  });
  expect(gestes.filter((c) => c === 'act_on_group')).toHaveLength(1);
  expect(gestes).not.toContain('archive_message');
  expect(gestes).not.toContain('thread_messages');
  await expect(barre()).toHaveCount(0);
  // Les deux sujets ont quitté la Réception…
  await expect(lignes().first()).toBeVisible();
  await expect
    .poll(async () => {
      const restants = (await lignes().locator('.objet').allTextContents()).map((s) => s.trim());
      return partants.filter((s) => restants.includes(s)).length;
    })
    .toBe(0);
  // …et se retrouvent en Archives.
  await dossier('archives').click();
  await expect(lignes().first()).toBeVisible();
  await expect
    .poll(async () => {
      const archives = (await lignes().locator('.objet').allTextContents()).map((s) => s.trim());
      return partants.filter((s) => archives.includes(s)).length;
    })
    .toBe(2);
  await dossier('reception').click();
  await expect(lignes().first()).toBeVisible();
});

test('supprimer groupé : les lignes rejoignent la corbeille', async () => {
  const sujets = await lignes().locator('.objet').allTextContents();
  const partant = sujets[0].trim();
  await caseDe(0).click();
  await page.locator('[data-testid="barre-supprimer"]').click();
  await expect(toast()).toContainText('supprimé');
  await expect
    .poll(async () => {
      const restants = (await lignes().locator('.objet').allTextContents()).map((s) => s.trim());
      return restants.includes(partant);
    })
    .toBe(false);
  await dossier('corbeille').click();
  await expect(lignes().first()).toBeVisible();
  await expect
    .poll(async () => {
      const corbeille = (await lignes().locator('.objet').allTextContents()).map((s) => s.trim());
      return corbeille.includes(partant);
    })
    .toBe(true);
});
