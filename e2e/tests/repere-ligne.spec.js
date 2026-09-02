// PLAN-REPERE-LIGNE (A80-A82) : la boîte se dit en toutes lettres, sur
// la ligne de l'expéditeur — « Expéditeur sur ▣ Libellé ». Le bloc vit
// là où les comptes se mélangent (D7), il ne demande PAS de repère
// (D8 : le mot suffit), il se répète au volet de lecture (D5), et la
// troncature protège l'heure et le nom (D4, plafond au tiers mesuré).
// Décor Clarity : deux comptes réels, un fil de trois messages —
// aucun repère posé, c'est le cas D8 (le tracé est tenu par
// refonte-retours-8).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgerLocales } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

// Le profil WebView2 est PARTAGÉ entre suites : la largeur de liste
// touchée par le test de troncature se purge avant ET après.

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  await purgerLocales(page, ['wind-largeurs']);
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgerLocales(page, ['wind-largeurs']);
  await closeApp({ app, browser });
});

const boiteNav = (libelle) =>
  page.locator('[data-testid="nav-boite"]', { hasText: libelle });

test('la ligne dit la boîte en toutes lettres — sur toutes les rangées (A80, D8)', async () => {
  // Boîte unifiée (défaut) : chaque rangée porte le bloc « sur
  // <libellé> » ; aucun compte n'a de nom personnalisé ni de repère
  // sur ce décor — le libellé est l'adresse (D8), sans tracé.
  const blocs = page.locator('[data-testid="ligne-boite"]');
  await expect(blocs.first()).toBeVisible();
  const nLignes = await page.locator('[data-testid="ligne"]').count();
  await expect(blocs).toHaveCount(nLignes);
  await expect(blocs.first().locator('.mot')).toHaveText('sur');
  await expect(blocs.first().locator('.lib')).toContainText('@');
  // D8 : compte sans repère — le bloc est là, le tracé non.
  await expect(page.locator('[data-testid="ligne-boite"] .repere-nu')).toHaveCount(0);
  // L'infobulle donne le libellé entier même tronqué (D4). Aucun compte
  // du décor n'a de nom personnalisé : libellé ET adresse sont la même
  // chaîne, et elle ne se dit qu'UNE fois — « adresse — adresse »
  // serait un bégaiement (revue du 2026-08-25). La forme « nom —
  // adresse » du cas nommé est tenue par retours-9-nom-compte.
  const titre = await blocs.first().getAttribute('title');
  expect(titre).toMatch(/^[^ ]+@[^ ]+$/);
  expect(titre).toBe((await blocs.first().locator('.lib').innerText()).trim());
});

test('la vue d’un seul compte ne dit rien (D7) — liste ET volet de lecture', async () => {
  await boiteNav('paul.merand@atelier-nord.fr').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne-boite"]')).toHaveCount(0);

  // Verdict terrain du 2026-08-25 (point 12) : le VOLET suit la liste.
  // D5 disait « le même schéma au volet » ; le terrain a montré
  // l'asymétrie — la liste se taisait, le volet disait encore la boîte.
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  await expect(volet.locator('[data-testid="message-deplie"]')).toBeVisible();
  await expect(volet.locator('.boite')).toHaveCount(0);

  // Retour à la boîte unifiée pour la suite.
  await boiteNav('Toutes les boîtes').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test('la recherche dit la boîte même depuis la vue d’un seul compte (exception D7)', async () => {
  // La recherche TRAVERSE les comptes : c'est la seule vue ou le bloc
  // s'affiche alors que la nav est bornee a un compte. Sans cette
  // garde, inverser la condition de `boiteDe` laisserait tout vert.
  await boiteNav('paul.merand@atelier-nord.fr').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne-boite"]')).toHaveCount(0);

  await page.locator('[data-testid="champ-recherche"]').fill('Vantis');
  await expect(page.locator('[data-testid="resultats"]')).toBeVisible();
  const resultats = page.locator('[data-testid="resultats"] [data-testid="ligne"]');
  await expect(resultats.first()).toBeVisible();
  const nResultats = await resultats.count();
  await expect(
    page.locator('[data-testid="resultats"] [data-testid="ligne-boite"]'),
  ).toHaveCount(nResultats);

  // Retour a l'etat de depart : la suite est serielle.
  await page.locator('[data-testid="champ-recherche"]').press('Escape');
  await expect(page.locator('[data-testid="resultats"]')).toHaveCount(0);
  await boiteNav('Toutes les boîtes').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test('le dossier Brouillons garde sa tuile (D9) et son heure au bord droit', async () => {
  // A81 ne retire la tuile que de la rangee de LISTE : au dossier
  // Brouillons elle dit le destinataire, et la rangee garde donc sa
  // colonne de tete (classe `tuilee`). Rien ne le tenait avant cette
  // garde — supprimer la tuile laissait toute la gate verte.
  await page.locator('[data-testid="nav-dossier"][data-categorie="brouillons"]').click();
  const rangee = page.locator('[data-testid="ligne-brouillon"]').first();
  await expect(rangee).toBeVisible();
  await expect(rangee.locator('.avatar')).toBeVisible();
  await expect(rangee).toHaveClass(/tuilee/);
  // Le dossier ne melange pas les comptes a l'affichage : pas de bloc.
  await expect(rangee.locator('[data-testid="ligne-boite"]')).toHaveCount(0);

  // Et son heure tient le bord DROIT : `.exp` ne grandit plus depuis
  // A80, c'est l'essor qui pousse — la rangee brouillon doit l'avoir
  // comme les autres, sinon l'heure se colle au destinataire.
  const cadre = await rangee.boundingBox();
  const heure = await rangee.locator('.heure').boundingBox();
  expect(cadre.x + cadre.width - (heure.x + heure.width)).toBeLessThan(24);

  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test('le volet de lecture dit la boîte, carte dépliée et rangée repliée (D5)', async () => {
  // Le premier fil du décor (3 messages : 1 déplié, 2 repliés).
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  const deplie = volet.locator('[data-testid="message-deplie"] .boite');
  await expect(deplie).toBeVisible();
  await expect(deplie.locator('.mot')).toHaveText('sur');
  await expect(deplie.locator('.lib')).toHaveText('paul.merand@atelier-nord.fr');
  // Les rangées repliées la disent aussi — derrière le nom.
  await expect(volet.locator('[data-testid="message-replie"] .boite')).toHaveCount(2);
});

test('la troncature protège l’heure et le nom à la borne basse (D4, 300 px)', async () => {
  // La poignée à fond à gauche : liste à 300 px (borne basse de
  // BORNES.liste), posée par la préférence persistée — le vrai canal.
  await page.evaluate(() => {
    localStorage.setItem('wind-largeurs', JSON.stringify({ nav: 248, liste: 300 }));
  });
  await page.reload();
  const premiere = page.locator('[data-testid="ligne"]').first();
  await expect(premiere).toBeVisible();
  // Le libellé long s'ellipse (CSS seule : le texte entier reste au
  // DOM, lu par les technologies d'assistance).
  const lib = page
    .locator('[data-testid="ligne-boite"] .lib', { hasText: 'atelier-nord' })
    .first();
  await expect(lib).toBeVisible();
  expect(
    await lib.evaluate((el) => el.scrollWidth > el.clientWidth),
  ).toBe(true);
  // L'heure ne cède jamais : visible, entière, dans la colonne.
  const heure = premiere.locator('.heure');
  await expect(heure).toBeVisible();
  const colonne = await page.locator('[data-testid="liste"]').boundingBox();
  const boite = await heure.boundingBox();
  expect(boite.x + boite.width).toBeLessThanOrEqual(colonne.x + colonne.width + 1);

  // Le plafond du tiers est le chiffre du dessin (§1.3) : le bloc ne
  // prend jamais plus d'un tiers de sa ligne, quelle que soit la
  // largeur du volet.
  const l1 = await premiere.locator('.l1').boundingBox();
  const bloc = await premiere.locator('[data-testid="ligne-boite"]').boundingBox();
  expect(bloc.width).toBeLessThanOrEqual(l1.width / 3 + 1);

  // ET rien ne se peint PAR-DESSUS l'heure. C'est la garde de la panne
  // trouvee en revue : avec un min-width:0 sur le bloc, « sur » et le
  // trace (tous deux flex:none) debordaient d'un bloc ecrase a 0 px et
  // recouvraient l'heure. Le dossier Envoyes est le pire cas du decor —
  // sa colonne dit « A : <adresse> », bien plus long qu'un nom.
  await page.locator('[data-testid="nav-dossier"][data-categorie="envoyes"]').click();
  const envoi = page.locator('[data-testid="ligne"]').first();
  await expect(envoi).toBeVisible();
  const blocEnvoi = await envoi.locator('[data-testid="ligne-boite"]').boundingBox();
  const heureEnvoi = await envoi.locator('.heure').boundingBox();
  expect(blocEnvoi.x + blocEnvoi.width).toBeLessThanOrEqual(heureEnvoi.x + 1);
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Retour au défaut pour les suites suivantes.
  await purgerLocales(page, ['wind-largeurs']);
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});
