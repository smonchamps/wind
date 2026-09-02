// PLAN-RETOURS-14 R2 (D2/D3) : la Réception organisée perd le bandeau
// générique et les onglets, prend l'entête normalisé des vues du mode
// (patron Kiosque/Portier, classes .entete-vue), et le nom de la
// section courante reste visible au défilement (bande collée).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injecterArrivee } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'principal@exemple.fr', messages: 40 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("la Réception organisée : entête normalisé, ni bandeau générique ni onglets", async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Au classique : bandeau générique ET onglets — la garde de départ.
  await expect(page.locator('[data-testid="onglets"]')).toBeVisible();
  await expect(page.locator('[data-testid="reception-titre"]')).toHaveCount(0);

  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');

  // L'entête au format des vues du mode : glyphe + « Réception » en
  // display, PAS le h1 de bandeau ; le pied disparaît (D3).
  const titre = page.locator('[data-testid="reception-titre"]');
  await expect(titre).toBeVisible();
  await expect(titre).toContainText('Réception');
  await expect(titre.locator('svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="onglets"]')).toHaveCount(0);

  // Les autres vues gardent leur forme : les Archives restent au
  // bandeau classique avec onglets.
  await page.locator('[data-testid="nav-dossier"][data-categorie="archives"]').click();
  await expect(page.locator('[data-testid="onglets"]')).toBeVisible();
  await expect(page.locator('[data-testid="reception-titre"]')).toHaveCount(0);
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
});

test('le nom de la section reste visible au défilement, et repart en tête', async () => {
  const cadre = page.locator('[data-testid="liste"] .cadre');
  await expect(page.locator('[data-testid="section"]').first()).toBeVisible();
  // En tête de liste : pas de bande collée — la bande réelle suffit.
  await expect(page.locator('[data-testid="section-collee"]')).toHaveCount(0);

  // Défiler dans le flot : la bande réelle part, la collée la relaie.
  // Le conteneur collé fait 0 px de haut (hors géométrie du
  // fenêtrage) : c'est l'étiquette intérieure qui se voit.
  await cadre.evaluate((el) => { el.scrollTop = 800; });
  const etiquette = page.locator('[data-testid="section-collee"] .cadre-entete');
  await expect(etiquette).toBeVisible();
  await expect(etiquette).toContainText('Nouveau pour vous');

  // Et elle colle VRAIMENT : en tête du cadre, à la géométrie près.
  const boiteCadre = await cadre.boundingBox();
  const bande = await page.locator('[data-testid="section-collee"] .cadre-entete').boundingBox();
  expect(bande.y - boiteCadre.y).toBeGreaterThanOrEqual(0);
  expect(bande.y - boiteCadre.y).toBeLessThan(8);

  // Retour en tête : la bande collée se retire.
  await cadre.evaluate((el) => { el.scrollTop = 0; });
  await expect(page.locator('[data-testid="section-collee"]')).toHaveCount(0);
});

// RETOURS-14 R7 (D8) : les pastilles nav du Kiosque (cartes jamais
// ouvertes — la sémantique exacte est prouvée côté cœur, test
// mail-core `la_pastille_du_kiosque_compte_les_cartes_jamais_ouvertes`)
// et du Registre (non-lu IMAP). Ici : le chemin d'affichage.
test('les pastilles nav du Kiosque et du Registre disent le travail restant', async () => {
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 4; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`, destination: 'kiosque', regle: null,
      });
    }
    for (let n = 4; n < 10; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`, destination: 'registre', regle: null,
      });
    }
  });
  await page.reload();
  const pastille = (cat) =>
    page.locator(`[data-testid="nav-dossier"][data-categorie="${cat}"] .pastille`);
  await expect(pastille('kiosque')).toBeVisible();
  await expect(pastille('kiosque')).toHaveText(/^[1-9]\d*$/);
  await expect(pastille('registre')).toBeVisible();
  await expect(pastille('registre')).toHaveText(/^[1-9]\d*$/);
  // Le Portier garde la sienne (préexistante) — rien n'a été cassé.
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="portier"]')).toBeVisible();
});

// RETOURS-14 R5 (D6) : Réglages > Portier — la liste EXHAUSTIVE des
// décisions (l'historique de la page Portier ne montre que les
// écartés), à l'alphabet, filtrable, avec « Réintégrer ».
test('Réglages > Portier : toutes les décisions, à l’alphabet, recherche et réintégration', async () => {
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('route_sender', {
      address: 'zeta@exemple.fr', destination: 'ecarte', regle: 'spam',
    });
  });
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="portier"]').click();

  const lignes = page.locator('[data-testid="portier-decision"]');
  // 4 Kiosque + 6 Registre (test précédent) + 1 écarté = 11, TOUTES
  // destinations confondues.
  await expect(lignes).toHaveCount(11);
  // L'alphabet, pas la chronologie : expediteur0 d'abord, zeta en queue.
  await expect(lignes.first()).toContainText('expediteur0@exemple.fr');
  await expect(lignes.first()).toContainText('Le Kiosque');
  await expect(lignes.last()).toContainText('zeta@exemple.fr');
  await expect(lignes.last()).toContainText('signalé indésirable');

  // La recherche filtre, et le « rien » est dit.
  await page.locator('[data-testid="portier-recherche"]').fill('zeta');
  await expect(lignes).toHaveCount(1);
  await page.locator('[data-testid="portier-recherche"]').fill('introuvable');
  await expect(lignes).toHaveCount(0);
  await expect(page.locator('[data-testid="portier-decisions-vide"]')).toBeVisible();

  // R10 (terrain) : « Modifier » repropose TOUTES les règles — un Oui
  // remplace l'écarté, le verdict affiché suit.
  await page.locator('[data-testid="portier-recherche"]').fill('zeta');
  await page.locator('[data-testid="decision-modifier"]').click();
  await expect(page.locator('[data-testid="decision-menu"]')).toBeVisible();
  await page.locator('[data-testid="decision-vers-kiosque"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('vont vers le Kiosque');
  await expect(lignes.first()).toContainText('Le Kiosque');
  // « Renvoyer au portier » — l'ancien Réintégrer : le verdict meurt.
  await page.locator('[data-testid="decision-modifier"]').click();
  await page.locator('[data-testid="decision-renvoyer"]').click();
  await expect(lignes).toHaveCount(0);
  await page.locator('[data-testid="portier-recherche"]').fill('');
  await expect(lignes).toHaveCount(10);
  await page.locator('[data-testid="reglages-termine"]').click();
});

// RETOURS-14 R6 (D7) : le Registre groupé par expéditeur — récence en
// tête (l'ordre exact est prouvé côté cœur, test mail-core
// `le_registre_se_groupe_par_expediteur_a_la_recence`). Ici : la vue,
// le dépli, l'ouverture du fil.
test('le Registre groupé : un rang par expéditeur, le fil s’ouvre depuis le groupe', async () => {
  await page.locator('[data-testid="nav-dossier"][data-categorie="registre"]').click();
  await expect(page.locator('[data-testid="registre-titre"]')).toContainText('Registre');
  const groupes = page.locator('[data-testid="registre-groupe"]');
  // Six adresses routées au Registre (test des pastilles) mais le jeu
  // d'essai n'a que 8 expéditeurs (4 à 7 réels ici), et la clé de
  // groupe est l'expéditeur de TÊTE du fil — un fil mêlé (le décor
  // fait répondre un message sur cinq au précédent) donne sa tête à un
  // autre expéditeur : 5 rangs, jamais une liste plate.
  await expect(groupes).toHaveCount(5);
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(0);

  // Déplier : les fils du seul expéditeur du groupe.
  await groupes.first().click();
  const messages = page.locator('[data-testid="registre-message"]');
  await expect(messages.first()).toBeVisible();

  // Ouvrir : le volet de lecture reste le lecteur du Registre.
  await messages.first().click();
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).toBeVisible();

  // Replier : les rangées se retirent.
  await groupes.first().click();
  await expect(messages).toHaveCount(0);

  // R9 (terrain, 2e passe) : le bouton ouvre un MENU des quatre tris,
  // chaque entrée avec son glyphe ; l'ordre des rangs suit le choix.
  const tri = page.locator('[data-testid="registre"] [data-testid="tri-section"]');
  await expect(tri).toContainText('Plus récents');
  const parDate = await groupes.evaluateAll((els) => els.map((e) => e.dataset.adresse));
  await tri.click();
  const menuTri = page.locator('[data-testid="tri-menu"]');
  await expect(menuTri).toBeVisible();
  // Quatre entrées, chacune son glyphe (tri_*, A104).
  await expect(menuTri.locator('[role="menuitemradio"]')).toHaveCount(4);
  await expect(menuTri.locator('svg[data-nom^="tri_"]')).toHaveCount(4);
  await menuTri.locator('[data-testid="tri-date-asc"]').click();
  await expect(tri).toContainText('Plus anciens');
  await expect
    .poll(async () => groupes.evaluateAll((els) => els.map((e) => e.dataset.adresse)))
    .toEqual([...parDate].reverse());
  // L'alphabet porte sur le NOM AFFICHÉ de l'expéditeur (ce que le
  // rang montre), pas l'adresse.
  await tri.click();
  await page.locator('[data-testid="tri-alpha-az"]').click();
  await expect(tri).toContainText('A → Z');
  const noms = await groupes.evaluateAll((els) => els.map((e) => e.querySelector('.exp').textContent));
  expect(noms).toEqual([...noms].sort((a, b) => a.localeCompare(b, 'fr', { sensitivity: 'base' })));
  await tri.click();
  await page.locator('[data-testid="tri-alpha-za"]').click();
  await expect(tri).toContainText('Z → A');
  await expect
    .poll(async () => groupes.evaluateAll((els) => els.map((e) => e.querySelector('.exp').textContent)))
    .toEqual([...noms].reverse());
  await tri.click();
  await page.locator('[data-testid="tri-date-desc"]').click();
  await expect(tri).toContainText('Plus récents');

  // Revue : les gestes d'expéditeur survivent à la vue groupée — le ⋯
  // d'un groupe route l'expéditeur ENTIER (Déplacer vers…, Écarter).
  await groupes.first().hover();
  await groupes.first().locator('[data-testid="registre-gestes"]').click();
  await expect(page.locator('[data-testid="registre-menu"]')).toBeVisible();
  await expect(page.locator('[data-testid="registre-ecarter"]')).toBeVisible();
  const adresse = await groupes.first().getAttribute('data-adresse');
  await page.locator('[data-testid="registre-vers-reception"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Expéditeur déplacé');
  // Le verdict est POSÉ (la porte du cœur) — le nombre de rangs, lui,
  // peut ne pas bouger : un fil mêlé routé par un AUTRE expéditeur
  // reste au Registre avec la même tête (règle d'or).
  await expect
    .poll(async () => page.evaluate(async (a) => {
      const routings = await window.__TAURI__.core.invoke('routings');
      return routings.find((r) => r.address === a)?.destination;
    }, adresse))
    .toBe('reception');
});

// RETOURS-14 R4 (D5) : le « fil mêlé » — un INCONNU répond dans le fil
// d'un connu. La règle d'or laisse le fil entier en Réception (jamais
// perdre de courrier) ; l'inconnu attend au Portier pendant que son
// message se lit — et le fil le DIT (badge « En attente au Portier »).
test('fil mêlé : l’inconnu qui répond dans un fil connu est signalé, et attend au Portier', async () => {
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  // Le jeu d'essai n'a que 8 expéditeurs et les tests précédents les
  // ont TOUS routés — on en réintègre un : expediteur0 redevient un
  // connu NON routé, son fil vit en Réception.
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'expediteur0@exemple.fr' });
  });
  // L'intrus répond au fil du connu expediteur0 (uid 16, fil d'un seul
  // message) — par LE chemin de production (upsert_envelopes).
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'intrus@exemple.fr', n: 1,
    nom: 'Un Intrus', sujet: 'Je rejoins le fil',
    reponseA: '<seed-INBOX-16@exemple.fr>',
  });
  await page.reload();

  // Le fil mêlé RESTE en Réception, tête au message de l'intrus.
  const ligne = page.locator('[data-testid="ligne"]', { hasText: 'Je rejoins le fil' }).first();
  await expect(ligne).toBeVisible();
  await ligne.click();

  // La Réception organisée est une scène sans volet : le fil s'ouvre à
  // l'écran 03. Le badge dit l'attente — sur le message de l'intrus.
  await expect(page.locator('[data-testid="conversation"] [data-testid="attente-portier"]').first())
    .toContainText('En attente au Portier');
  await page.locator('[data-testid="retour-boite"]').click();

  // Et l'intrus attend RÉELLEMENT au guichet.
  await page.locator('[data-testid="nav-dossier"][data-categorie="portier"]').click();
  await expect(page.locator('[data-testid="portier-rang"]', { hasText: 'intrus@exemple.fr' }))
    .toBeVisible();
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
});

// RETOURS-14 R8 (terrain 2026-08-31) : un OUI au Portier vaut
// confiance — le verdict pose AUSSI la règle « toujours afficher les
// images de cet expéditeur », révocable aux Réglages > Affichage.
// (La sémantique exacte est prouvée côté cœur, test mail-core
// `un_oui_au_portier_autorise_les_images_de_l_expediteur`.)
test('valider un expéditeur au Portier autorise ses images — règle visible et révocable', async () => {
  // L'intrus du test précédent attend au guichet : Oui.
  await page.locator('[data-testid="nav-dossier"][data-categorie="portier"]').click();
  await page.locator('[data-testid="portier-rang"]', { hasText: 'intrus@exemple.fr' })
    .locator('[data-testid="portier-oui"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('peut vous écrire');

  // La règle d'images est posée — Réglages > Affichage la montre, et
  // sa porte de sortie existante la retire.
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  const regle = page.locator('[data-testid="expediteur-images"]', { hasText: 'intrus@exemple.fr' });
  await expect(regle).toBeVisible();
  await regle.locator('[data-testid="retirer-expediteur-images"]').click();
  await expect(regle).toHaveCount(0);
  await page.locator('[data-testid="reglages-termine"]').click();
});
