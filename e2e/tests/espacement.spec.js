// PLAN-ESPACEMENT (A83) : trois crans d'air entre les messages —
// « Faible » (l'existant au pixel près), « Moyen », « Élevé ».
//
// Ce fichier est le FILET DE SÛRETÉ du chantier, et il vise une classe
// de bug précise : les gabarits de hauteur h1/h2 sont MESURÉS au rendu,
// et tout le fenêtrage en dépend. Une hauteur figée sur l'ancien cran
// ferait mentir la barre de défilement de 13,6 % à 27,3 % et pourrait
// poser la fenêtre 12 000 px à côté — un écran blanc.
//
// PREMIÈRE VERSION RÉÉCRITE (revue du 2026-08-25) : le filet précédent
// était en partie DÉCORATIF, et la revue l'a démontré —
//  · « la ligne du haut ne bouge pas » lisait l'état interne `premier`,
//    que rien ne recalcule quand h1 change hors section épinglée :
//    l'assertion passait même en supprimant tout le ré-ancrage ;
//  · « la barre dit la vraie hauteur » comparait deux membres tirés du
//    MÊME h1 — une identité arithmétique, increvable ;
//  · le décor n'avait ni épingle (le chemin du vrai défaut) ni rangée
//    porteuse (h2 jamais vérifié), et la fenêtre restait trop haute
//    pour qu'une barre fantôme puisse seulement se voir.
// Chaque test ci-dessous a été vérifié capable d'ÉCHOUER.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, purgerLocales } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });


// Les valeurs de la décision CE D1. Le delta est arithmétique : +6 px
// de padding = +12 px de rangée, sur les DEUX gabarits.
const CRANS = [
  { niveau: 'low', pad: 13, h1: 88, h2: 115 },
  { niveau: 'medium', pad: 19, h1: 100, h2: 127 },
  { niveau: 'high', pad: 25, h1: 112, h2: 139 },
];

// Une Réception PROFONDE : il faut de quoi défiler loin (le mensonge de
// géométrie est invisible sur dix lignes) ET pouvoir épingler, ce que
// seule la Réception permet (D4 d'A73). Le seeder fabrique des fils :
// le décor porte donc aussi des rangées PORTEUSES, sans quoi h2 ne
// serait jamais exercé.
test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'un@exemple.fr', messages: 400 }],
  }));
  await purgerLocales(page, ['wind-espacement']);
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await purgerLocales(page, ['wind-espacement']);
  await closeApp({ app, browser });
});

const poserCran = async (niveau) => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="affichage"]').click();
  await page.locator('[data-testid="display-spacing"]').selectOption(niveau);
  await page.locator('[data-testid="settings-done"]').click();
  // La bascule traverse un rendu, un layout et un ResizeObserver.
  await page.waitForTimeout(200);
};

const geometrie = () => page.evaluate(() => window.__mesure.state());

// CE QUE L'UTILISATEUR VOIT, et non ce que le composant croit : le
// sujet de la rangée réellement posée en haut du cadre. C'est la seule
// lecture qu'un ré-ancrage cassé ne peut pas satisfaire par accident.
const sujetAuSommet = () => page.evaluate(() => {
  const cadre = document.querySelector('.frame');
  const haut = cadre.getBoundingClientRect().top;
  let gagnant = null;
  let ecart = Infinity;
  for (const l of cadre.querySelectorAll('[data-testid="row"]')) {
    const d = Math.abs(l.getBoundingClientRect().top - haut);
    if (d < ecart) { ecart = d; gagnant = l; }
  }
  return gagnant?.querySelector('.subject')?.textContent?.trim() ?? null;
});

test('les trois crans donnent les DEUX gabarits attendus (D1)', async () => {
  for (const cran of CRANS) {
    await poserCran(cran.niveau);
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
    const pad = await page.evaluate(() => {
      const l = document.querySelector('[data-testid="row"]');
      return l ? getComputedStyle(l).paddingTop : null;
    });
    expect(pad).toBe(`${cran.pad}px`);
    // Les gabarits SONDÉS suivent — c'est eux que le fenêtrage utilise.
    // h2 compte autant que h1 : `extraPuce = h2 - h1` porte tout le
    // calcul des rangées porteuses.
    const { h1, h2 } = await geometrie();
    expect(h1).toBe(cran.h1);
    expect(h2).toBe(cran.h2);
  }
  await poserCran('low');
});

test('la barre de défilement dit la hauteur RÉELLE des rangées', async () => {
  // Non tautologique : la hauteur de référence est MESURÉE dans le DOM
  // (getBoundingClientRect), pas relue du gabarit qui a servi à la
  // calculer. Un gabarit figé sur l'ancien cran serait donc pris.
  for (const cran of CRANS) {
    await poserCran(cran.niveau);
    const { total } = await geometrie();
    // Les deux gabarits se mesurent SÉPARÉMENT dans le DOM : la
    // première rangée du décor est porteuse (elle a son rang de puces),
    // la confondre avec une nue ferait dire 115 là où on attend 88.
    const mesure = await page.evaluate(() => {
      const cadre = document.querySelector('.frame');
      const lignes = [...cadre.querySelectorAll('[data-testid="row"]')];
      const nue = lignes.find((l) => !l.querySelector('.chips'));
      const porteuse = lignes.find((l) => l.querySelector('.chips'));
      return {
        scrollHeight: cadre.scrollHeight,
        hNue: nue ? nue.getBoundingClientRect().height : null,
        hPorteuse: porteuse ? porteuse.getBoundingClientRect().height : null,
      };
    });
    expect(mesure.hNue).not.toBeNull();
    expect(Math.round(mesure.hNue)).toBe(cran.h1);
    if (mesure.hPorteuse !== null) {
      expect(Math.round(mesure.hPorteuse)).toBe(cran.h2);
    }
    // La barre couvre au moins toutes les rangées à leur hauteur réelle,
    // et pas beaucoup plus (les porteuses ajoutent leur rang de puces).
    expect(mesure.scrollHeight).toBeGreaterThanOrEqual(total * cran.h1);
    expect(mesure.scrollHeight).toBeLessThan(total * (cran.h1 + 30));
  }
  await poserCran('low');
});

test('bascule à chaud en profondeur : la ligne VUE ne bouge pas', async () => {
  await poserCran('low');
  await page.evaluate(() => window.__mesure.page(200));
  await page.waitForTimeout(200);

  const avantIndex = (await geometrie()).first;
  const avantSujet = await sujetAuSommet();
  expect(avantIndex).toBeGreaterThan(150);
  expect(avantSujet).toBeTruthy();

  await poserCran('high');

  expect((await geometrie()).h1).toBe(112);
  // L'assertion qui compte : le MÊME message est en haut de l'écran.
  expect(await sujetAuSommet()).toBe(avantSujet);
  expect(Math.abs((await geometrie()).first - avantIndex)).toBeLessThanOrEqual(1);

  await poserCran('low');
});

test('bascule à chaud AVEC une conversation épinglée — le chemin du défaut', async () => {
  // Le défaut que la revue a mesuré ne se manifeste QUE là : les
  // épinglées sont des rangées, elles grandissent avec le cran, leur
  // ResizeObserver réveille l'effet qui recalcule la position — et il
  // est ordonnancé AVANT le ré-ancrage. Sans épingle, ce chemin dort.
  await poserCran('low');
  await page.locator('[data-testid="row"]').first().click();
  const epingler = page.locator('[data-testid="reading-pane"] [data-testid="pin"]');
  await epingler.click();
  await expect(page.locator('[data-testid="pins"]')).toBeVisible();

  await page.evaluate(() => window.__mesure.page(200));
  await page.waitForTimeout(200);
  const avantIndex = (await geometrie()).first;
  const avantSujet = await sujetAuSommet();
  expect(avantIndex).toBeGreaterThan(150);

  await poserCran('high');

  expect(await sujetAuSommet()).toBe(avantSujet);
  expect(Math.abs((await geometrie()).first - avantIndex)).toBeLessThanOrEqual(1);

  // Désépingler pour rendre le décor à la suite.
  await poserCran('low');
  await page.locator('[data-testid="pins"] [data-testid="row"]').first().click();
  await epingler.click();
  await expect(page.locator('[data-testid="pins"]')).toHaveCount(0);
});

test('hors du flot fenêtré, changer de cran ne déplace pas la liste', async () => {
  // Le dossier Brouillons et les résultats de recherche ne sont pas
  // fenêtrés : leur position n'a rien à voir avec la géométrie du flot.
  // Y appliquer le ré-ancrage remonterait la liste en haut à chaque
  // changement de cran (dans Brouillons, `total` vaut 0, donc aller(0)).
  await page.locator('[data-testid="search-field"]').fill('message');
  await expect(page.locator('[data-testid="results"]')).toBeVisible();
  const resultats = page.locator('[data-testid="results"] [data-testid="row"]');
  await expect(resultats.first()).toBeVisible();

  await page.evaluate(() => { document.querySelector('.frame').scrollTop = 300; });
  await page.waitForTimeout(100);
  const avant = await page.evaluate(() => document.querySelector('.frame').scrollTop);
  expect(avant).toBeGreaterThan(0);

  await poserCran('medium');
  const apres = await page.evaluate(() => document.querySelector('.frame').scrollTop);
  // La liste a pu bouger de quelques pixels (les rangées ont grandi),
  // mais elle n'a pas été RENVOYÉE en haut ni jetée en butée.
  expect(apres).toBeGreaterThan(0);

  await poserCran('low');
  await page.locator('[data-testid="search-field"]').press('Escape');
  await expect(page.locator('[data-testid="results"]')).toHaveCount(0);
});

test('le cran survit au relancement ; une valeur tordue retombe au défaut', async () => {
  await poserCran('medium');
  await page.reload();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  expect((await geometrie()).h1).toBe(100);

  // Une préférence corrompue ne casse pas la liste — y compris une clé
  // du PROTOTYPE : `'toString' in CRANS` vaut vrai, et la garde doit
  // passer par la liste des niveaux, pas par l'opérateur `in`.
  for (const tordue of ['gigantesque', 'toString', 'constructor']) {
    await page.evaluate((v) => localStorage.setItem('wind-espacement', v), tordue);
    await page.reload();
    await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
    expect((await geometrie()).h1).toBe(88);
    const pad = await page.evaluate(() => {
      const l = document.querySelector('[data-testid="row"]');
      return getComputedStyle(l).paddingTop;
    });
    expect(pad).toBe('13px');
  }
});

test('fenêtre COURTE : les sondes ne laissent aucune barre fantôme', async () => {
  // La pile des sondes mesure ~203 px. Le fantôme ne peut se voir que
  // si le cadre est plus court — à la taille du décor il resterait
  // invisible, et retirer `position:relative` de la cage passerait.
  // On rétrécit donc le cadre au-dessous de la pile, dans un dossier
  // vide où tout excédent est forcément fantôme.
  await page.locator('[data-testid="nav-folder"][data-category="drafts"]').click();
  await expect(page.locator('[data-testid="folder-drafts"]')).toBeVisible();
  // `.cadre` est en `flex:1` dans une colonne : poser `height` ne le
  // contraint pas, il faut lui retirer sa croissance.
  const mesure = await page.evaluate(() => {
    const cadre = document.querySelector('.frame');
    const avant = cadre.style.flex;
    cadre.style.flex = '0 0 150px';
    void cadre.offsetHeight;
    const r = { scrollHeight: cadre.scrollHeight, clientHeight: cadre.clientHeight };
    cadre.style.flex = avant;
    return r;
  });
  expect(mesure.clientHeight).toBeLessThanOrEqual(160);
  expect(mesure.scrollHeight).toBeLessThanOrEqual(mesure.clientHeight);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
});

test('la fenêtre se recalcule quand le cadre grandit (décision CE D3)', async () => {
  // Défaut PRÉEXISTANT corrigé par ce chantier : la hauteur du cadre se
  // lisait par `clientHeight`, qui n'est pas un signal — agrandir la
  // fenêtre laissait une bande vide en bas jusqu'au prochain
  // défilement. On change la hauteur du cadre, ce qui déclenche le même
  // ResizeObserver qu'un redimensionnement de fenêtre.
  const compter = () => page.locator('[data-testid="row"]').count();

  await page.evaluate(() => { document.querySelector('.frame').style.flex = '0 0 300px'; });
  await page.waitForTimeout(200);
  const court = await compter();

  await page.evaluate(() => { document.querySelector('.frame').style.flex = '0 0 1400px'; });
  await page.waitForTimeout(300);
  const long = await compter();

  // Sans la correction, le nombre de rangées rendues resterait celui du
  // cadre court : la bande du bas serait vide.
  expect(long).toBeGreaterThan(court);

  await page.evaluate(() => { document.querySelector('.frame').style.flex = ''; });
  await page.waitForTimeout(200);
});
