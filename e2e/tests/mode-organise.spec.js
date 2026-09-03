// PLAN-MODE-ORGANISE E1 — le socle du Mode organisé (2026-08-29).
//
// Le va-et-vient « Organisé » vit à droite de la recherche (forme
// arrêtée au prototype, six passes CE) ; l'état vit en prefs SQLite
// (D2 amendée : le cœur doit le lire — les règles du Non de E3
// s'éteindront avec lui), donc il survit au rechargement SANS
// localStorage. En mode organisé, la nav gagne Kiosque et Registre —
// des vues du flot unifié filtrées par le routage d'expéditeur
// (routage_unified_scoped, sonde PK prouvée au spike S2). Le mode
// classique reste l'app d'aujourd'hui : la garde « zéro diff » est le
// premier test.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp, injecterArrivee } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {

  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'principal@exemple.fr', messages: 6 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('le mode classique est intact : va-et-vient éteint, nav aux six dossiers', async () => {
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  const bascule = page.locator('[data-testid="organized-mode"]');
  await expect(bascule).toBeVisible();
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  // La garde « classique inchangé » : exactement les six dossiers
  // canoniques, ni Kiosque ni Registre.
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  await expect(page.locator('[data-testid="nav-folder"][data-category="feed"]')).toHaveCount(0);
  // R3/R12 (RETOURS-13) : au classique, le libellé long et pas de filet.
  await expect(page.locator('[data-testid="nav-folder"][data-category="inbox"]'))
    .toContainText('Boîte de réception');
  await expect(page.locator('[data-testid="nav-separator"]')).toHaveCount(0);
});

test('la bascule recompose la nav, le Kiosque sert les expéditeurs routés, et le mode PERSISTE', async () => {
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(10);
  // R3/R12 (RETOURS-13) : en mode organisé la Réception se dit
  // « Réception », et un filet sépare les 5 dossiers organisés du reste.
  const rangReception = page.locator('[data-testid="nav-folder"][data-category="inbox"]');
  await expect(rangReception).toContainText('Réception');
  await expect(rangReception).not.toContainText('Boîte de réception');
  await expect(page.locator('[data-testid="nav-separator"]')).toHaveCount(1);

  // Le Kiosque avant tout routage : rien — le filtre est réel, pas un
  // décor (le Registre le reprouve plus bas après routage). E5bis :
  // le Kiosque est une scène de CARTES, plus une liste.
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-empty"]')).toBeVisible();
  await expect(page.locator('[data-testid="feed-card"]')).toHaveCount(0);

  // Route les expéditeurs du jeu d'essai vers le Kiosque, par LA
  // commande du produit (le geste « Déplacer vers… » arrive plus tard
  // dans E1 — le service, lui, est déjà le vrai).
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 12; n += 1) {
      await invoke('route_sender', {
        address: `expediteur${n}@exemple.fr`,
        destination: 'feed',
        rule: null,
      });
    }
  });

  // La persistance est en BASE (prefs SQLite) : un rechargement complet
  // relit le mode du cœur — jamais du localStorage.
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(10);

  // Le Kiosque montre désormais le courrier des expéditeurs routés,
  // en cartes DÉJÀ OUVERTES : le corps se lit sans un clic (E5bis —
  // la preuve du préchargement D5/S3, dans l'iframe assainie S1).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]').first()).toBeVisible();
  // R11 (RETOURS-13) : l'entête au format du Portier — glyphe + titre
  // + deux phrases CE, à gauche ; tout est neuf : la section « Non lus ».
  await expect(page.locator('[data-testid="feed-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="feed"]'))
    .toContainText("Tous vos emails d'information sont regroupés ici.");
  await expect(page.locator('[data-testid="feed"]'))
    .toContainText('Il vous suffit de les faire défiler pour les lire.');
  await expect(page.locator('[data-testid="feed-section-unread"]')).toBeVisible();
  await expect(
    page.frameLocator('[data-testid="feed-card"] iframe').first().locator('body'),
  ).toContainText('contenu de démonstration');
  // Le pli (constat CE) : replier remplace le corps par l'aperçu,
  // déplier le rend.
  const premiere = page.locator('[data-testid="feed-card"]').first();
  await premiere.locator('[data-testid="feed-fold"]').click();
  await expect(premiere.locator('iframe')).toHaveCount(0);
  await premiere.locator('[data-testid="feed-fold"]').click();
  await expect(premiere.locator('iframe')).toHaveCount(1);
  // …le Registre reste vide (la destination filtre vraiment)…
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  await expect(page.locator('[data-testid="status"]')).toContainText('Registre');
  await expect(page.locator('[data-testid="row"]')).toHaveCount(0);
  // …et la Réception ORGANISÉE ne les montre plus (E2 : un fil routé
  // ailleurs vit dans SA vue — l'exclusion partagée du flot ; tout est
  // au Kiosque ici, la Réception organisée est donc vide).
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  // Le vide est AFFIRMÉ (« Aucun message ici. ») — jamais un décompte
  // à zéro pendant que la page charge encore.
  await expect(page.locator('[data-testid="list"]')).toContainText('Aucun message ici.');
  await expect(page.locator('[data-testid="row"]')).toHaveCount(0);
});

test("« Déplacer vers… » route l'expéditeur ENTIER — le ⋯ des cartes et la barre du fil", async () => {
  // Tout est au Kiosque (test précédent) ; le ⋯ d'une carte envoie
  // son expéditeur au Registre — ce que l'utilisateur VOIT : le menu,
  // le toast, puis le courrier au Registre (une liste, elle).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  // RETOURS-13 R10 : des cartes déjà lues peuvent s'être groupées par
  // expéditeur — on déplie tout pour attraper la première carte.
  for (const g of await page.locator('[data-testid="feed-group"]').all()) await g.click();
  const carte = page.locator('[data-testid="feed-card"]').first();
  await carte.hover();
  await carte.locator('[data-testid="feed-gestures"]').click();
  await page.locator('[data-testid="feed-to-paper_trail"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Registre');
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  // RETOURS-14 R6 (D7) : le Registre est GROUPÉ par expéditeur — le
  // groupe se déplie, le fil s'ouvre depuis ses rangées.
  await expect(page.locator('[data-testid="paper-trail-group"]').first()).toBeVisible();
  await page.locator('[data-testid="paper-trail-group"]').first().click();
  await expect(page.locator('[data-testid="paper-trail-message"]').first()).toBeVisible();
  // La barre du fil, depuis le Registre : le menu Déplacer vers…
  await page.locator('[data-testid="paper-trail-message"]').first().click();
  await page.locator('[data-testid="move-to"]').click();
  await expect(page.locator('[data-testid="move-feed"]')).toBeVisible();
  await page.keyboard.press('Escape');
  // Le geste n'existe qu'en mode organisé : la garde du classique.
  await page.locator('[data-testid="organized-mode"]').click();
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="move-to"]')).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
});

// ------- RETOURS-13 R10 — le Kiosque en sections Non lus / Lus -------
test('les cartes lues jusqu’en bas se groupent par expéditeur — « Lus précédemment »', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  const scene = page.locator('[data-testid="feed"]');
  // Déplier les groupes déjà lus, puis parcourir TOUTE la scène : le
  // bas de chaque élévation passe à l'écran — la définition du « lu ».
  for (const g of await page.locator('[data-testid="feed-group"]').all()) await g.click();
  await scene.evaluate(async (el) => {
    for (let y = 0; y <= el.scrollHeight; y += 150) {
      el.scrollTop = y;
      await new Promise((r) => setTimeout(r, 40));
    }
  });
  // Le temps aux marques de s'écrire (observer + IPC).
  await page.waitForTimeout(600);
  // Le sectionnement se fait AU SERVICE de la page (une carte ne saute
  // jamais en pleine lecture) : aller-retour de dossier.
  await page.locator('[data-testid="nav-folder"][data-category="paper_trail"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-section-read"]')).toBeVisible();
  // Terrain C5 : le titre « Non lus » RESTE, la coche dit tout lu.
  await expect(page.locator('[data-testid="feed-section-unread"]')).toBeVisible();
  await expect(page.locator('[data-testid="feed-all-read"]'))
    .toContainText('Vous avez lu toutes les nouvelles actualités de votre Kiosque.');
  // Repliés par défaut : aucune carte à l'écran, des groupes par
  // expéditeur triés à l'alphabet.
  await expect(page.locator('[data-testid="feed-card"]')).toHaveCount(0);
  const noms = await page.locator('[data-testid="feed-group-name"]').allTextContents();
  expect(noms.length).toBeGreaterThan(1);
  expect(noms).toEqual(
    [...noms].sort((a, b) => a.localeCompare(b, 'fr', { sensitivity: 'base' })),
  );
  // Le clic déplie le groupe : ses cartes, repliées sur la ligne de
  // l'objet — dépliables une à une.
  await page.locator('[data-testid="feed-group"]').first().click();
  const carte = page.locator('[data-testid="feed-card"]').first();
  await expect(carte).toBeVisible();
  await expect(carte.locator('iframe')).toHaveCount(0);
  await carte.locator('[data-testid="feed-fold"]').click();
  await expect(carte.locator('iframe')).toHaveCount(1);
});

// ------------------------- E2 — le Portier -------------------------
// La rétention (D3 « arrivées seules ») se prouve par ce que
// l'utilisateur VOIT : un inconnu qui écrit APRÈS l'activation
// n'apparaît PAS en Réception organisée — il attend au Portier, avec
// sa pastille ; un connu (du courrier d'avant l'époque) arrive
// normalement. L'arrivée passe par le chemin de production
// (`injecterArrivee` → upsert_envelopes), jamais un décor.
test("un inconnu qui écrit attend au Portier — la Réception organisée ne le montre pas", async () => {
  // Décor neutre : les verdicts posés par les tests E1 se retirent —
  // le guichet se prouve sur un poste sans routage préalable.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    const routings = await invoke('routings');
    for (const r of routings) await invoke('remove_routing', { address: r.address });
  });
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'inconnue@exemple.fr',
    nom: 'Nouvelle Venue', sujet: 'Premiere fois',
  });
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'expediteur2@exemple.fr',
    nom: 'Alice Martin', sujet: 'Suite du dossier',
  });
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  // Le connu arrive en Réception ; l'inconnue n'y est PAS.
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(1);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // La pastille du Portier compte SON message.
  const rangPortier = page.locator('[data-testid="nav-folder"][data-category="screener"]');
  await expect(rangPortier).toContainText('Portier');
  await expect(rangPortier).toContainText('1');
  // Le guichet : un rang au format des rangées, l'adresse en clair.
  await rangPortier.click();
  await expect(page.locator('[data-testid="screener"]')).toContainText('Voulez-vous recevoir leurs messages ?');
  // R4/R7 (RETOURS-13) : le glyphe portier coiffe le titre, le
  // sous-titre porte les trois phrases CE mot pour mot.
  await expect(page.locator('[data-testid="screener-title"] svg')).toHaveCount(1);
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText('Les autorisez-vous à vous contacter ?');
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText('Les expéditeurs ne seront jamais informés de votre décision.');
  const rang = page.locator('[data-testid="screener-rank"]');
  await expect(rang).toHaveCount(1);
  await expect(rang).toContainText('Nouvelle Venue');
  await expect(rang).toContainText('<inconnue@exemple.fr>');
  await expect(rang).toContainText('Premiere fois');
});

test("le Oui nu rend l'expéditeur à la Réception, le guichet se vide", async () => {
  await page.locator('[data-testid="screener-yes"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('peut vous écrire');
  await expect(page.locator('[data-testid="screener-empty"]')).toBeVisible();
  // R6 (RETOURS-13) : l'historique vide, le texte CE mot pour mot.
  await expect(page.locator('[data-testid="screener"]'))
    .toContainText("Vous n'avez écarté aucun expéditeur pour le moment.");
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

test("le Non avec règle écarte, l'historique le dit, « Réintégrer » rend au Portier", async () => {
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'promo@exemple.fr',
    nom: 'Promo Eclair', sujet: 'Offre eclair', n: 2,
  });
  await page.reload();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toHaveCount(1);
  // Le mini ⋯ du Non pose la règle — « Archivés automatiquement ».
  await page.locator('[data-testid="screener-mini-no"]').click();
  await page.locator('[data-testid="screener-rule-archive"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('archiveront automatiquement');
  await expect(page.locator('[data-testid="screener-empty"]')).toBeVisible();
  const historique = page.locator('[data-testid="screener-history"]');
  await expect(historique).toHaveCount(1);
  await expect(historique).toContainText('promo@exemple.fr');
  await expect(historique).toContainText('archivage automatique');
  // « Réintégrer » défait le verdict : l'inconnu RE-attend au guichet.
  await page.locator('[data-testid="screener-reinstate"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('réintégré');
  await expect(page.locator('[data-testid="screener-rank"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('promo@exemple.fr');
});

test('le mode classique montre TOUJOURS tout — la rétention est une affaire du mode organisé', async () => {
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  // L'inconnu encore en attente (promo) est VISIBLE au classique.
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
  // Le retour au mode organisé, SANS naviguer : la liste affichée se
  // ressert d'elle-même (revue E2 — la bascule rechargeait la nav mais
  // pas la Réception, l'écran gardait la page de l'autre mode).
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

// ------------------- E3 — les règles du Non exécutées -------------------
test("la règle du Non s'exécute à l'arrivée — et ne touche jamais l'antérieur au verdict", async () => {
  // promo@ re-attend au guichet (test précédent) : le Non avec règle
  // « Déplacés automatiquement dans la corbeille » (corbeille au cœur, D4).
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-mini-no"]').click();
  await page.locator('[data-testid="screener-rule-trash"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('iront à la Corbeille');
  // Le verdict est daté à la SECONDE : une arrivée dans la même
  // seconde compte comme antérieure (« > verdict », limite assumée) —
  // on laisse la borne passer avant d'injecter.
  await page.waitForTimeout(1500);
  // Son PROCHAIN message arrive : la règle le traite — journal d'action
  // + disparition locale, il n'apparaît NULLE PART, pas même au
  // classique. Son courrier d'AVANT le verdict, lui, ne bouge pas.
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'promo@exemple.fr',
    nom: 'Promo Eclair', sujet: 'Relance finale',
  });
  // Le TÉMOIN (revue E3) : une seconde arrivée, d'un inconnu — sa
  // présence prouve que l'injection et son traitement ont bien eu
  // lieu ; sans lui, « Relance finale absente » serait aussi vraie si
  // rien n'était arrivé du tout (filet vacant).
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'temoin@exemple.fr',
    nom: 'Temoin', sujet: 'Temoin de synchro',
  });
  await page.reload();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Temoin de synchro' })).toHaveCount(1);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'true');
});

// ---------- RETOURS-13 R5/R9 — les défauts des boutons du Portier ----------
test("le Non nu envoie à la Corbeille — le défaut livré, dit par le toast et l'historique", async () => {
  // temoin@ attend au Portier (fin du test E3).
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="screener-no"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('iront à la Corbeille');
  await expect(page.locator('[data-testid="screener-history"]', { hasText: 'temoin@exemple.fr' }))
    .toContainText('suppression automatique');
  // On défait : temoin re-attend, l'état de la chaîne sérielle est rendu.
  await page.locator('[data-testid="screener-history"]', { hasText: 'temoin@exemple.fr' })
    .locator('[data-testid="screener-reinstate"]').click();
  await expect(page.locator('[data-testid="screener-rank"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

test('Réglages > Portier règle les défauts — le clic nu obéit, la persistance est en base', async () => {
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  const oui = page.locator('[data-testid="screener-default-yes"]');
  const non = page.locator('[data-testid="screener-default-no"]');
  await expect(oui).toHaveValue('inbox');
  await expect(non).toHaveValue('trash');
  await oui.selectOption('feed');
  await non.selectOption('archive');
  await page.locator('[data-testid="settings-done"]').click();
  // Le clic nu Oui suit le défaut réglé : temoin part au Kiosque.
  await page.locator('[data-testid="nav-folder"][data-category="screener"]').click();
  await page.locator('[data-testid="screener-yes"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('vont vers le Kiosque');
  // La persistance est en BASE : un rechargement complet relit les
  // défauts du cœur.
  await page.reload();
  await page.locator('[data-testid="settings"]').click();
  await page.locator('[data-testid="settings-group"][data-group="screener"]').click();
  await expect(page.locator('[data-testid="screener-default-yes"]')).toHaveValue('feed');
  await expect(page.locator('[data-testid="screener-default-no"]')).toHaveValue('archive');
  // Retour aux défauts livrés, temoin re-attend : l'état est rendu.
  await page.locator('[data-testid="screener-default-yes"]').selectOption('inbox');
  await page.locator('[data-testid="screener-default-no"]').selectOption('trash');
  await page.locator('[data-testid="settings-done"]').click();
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'temoin@exemple.fr' });
  });
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
});

// ---------------- E4 — la Réception organisée (sections) ----------------
test("la Réception organisée a ses sections, s'ouvre en écran 03, et un fil lu quitte « Nouveau pour vous »", async () => {
  // Mode ON, sur la Réception (fin du test E3). Les deux sections du
  // prototype encadrent UN flot : non-lus d'abord, la couture est le
  // COUNT — et le volet de lecture n'existe pas ici.
  const sections = page.locator('[data-testid="section"]');
  await expect(sections).toHaveCount(2);
  await expect(sections.first()).toContainText('Nouveau pour vous ·');
  await expect(sections.last()).toContainText('Déjà consulté');
  // `reading-pane` : le VRAI testid du volet (revue E5 — « lecture »
  // n'existe pas, l'assertion était vacante par construction).
  await expect(page.locator('[data-testid="reading-pane"]')).toHaveCount(0);

  const libelle = await sections.first().textContent();
  const n = Number(libelle.match(/(\d+)/)[1]);
  // Le clic ouvre l'ÉCRAN 03 (jamais un volet), le retour ressert la
  // liste : le fil LU a quitté « Nouveau pour vous ».
  await page.locator('[data-testid="row"]').first().click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(sections.first()).toContainText(`Nouveau pour vous · ${n - 1}`);
});

test("le ⋯ d'une rangée déplace l'expéditeur — à gauche de l'heure, sans bouger la géométrie", async () => {
  const rang = page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' });
  await expect(rang).toHaveCount(1);
  await rang.locator('[data-testid="row-gestures"]').click();
  await page.locator('[data-testid="gestures-feed"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Kiosque');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(0);
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await expect(page.locator('[data-testid="feed-card"]', { hasText: 'Suite du dossier' })).toHaveCount(1);
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  // Décor rendu (revue E5) : le verdict posé par CE test se retire —
  // les tests suivants héritent d'une Réception complète, jamais d'un
  // Kiosque peuplé par accident.
  await page.evaluate(async () => {
    await window.__TAURI__.core.invoke('remove_routing', { address: 'expediteur2@exemple.fr' });
  });
  // La liste ne suit pas une écriture externe (elle ne se recharge
  // qu'au battement d'une génération de relève) : on la ressert par le
  // geste produit — l'aller-retour de dossier. Avant RETOURS-13 ce pas
  // passait par une recharge FORTUITE de la sonde (filet chanceux).
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await page.locator('[data-testid="nav-folder"][data-category="inbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Suite du dossier' })).toHaveCount(1);
});

// ------------------------- E5 — Mis de côté -------------------------
test('mis de côté : le fil quitte la liste, vit dans la pile, et « Terminé » le rend', async () => {
  const rang = page.locator('[data-testid="row"]', { hasText: 'Premiere fois' });
  await expect(rang).toHaveCount(1);
  await rang.locator('[data-testid="row-gestures"]').click();
  await page.locator('[data-testid="gestures-aside"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Mis de côté');
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // La pile, en bas à droite : le compte, l'éventail, le tableau.
  const pile = page.locator('[data-testid="pile-button"]');
  await expect(pile).toContainText('1');
  await pile.click();
  const carte = page.locator('[data-testid="pile-card"]');
  await expect(carte).toHaveCount(1);
  await expect(carte).toContainText('Premiere fois');
  await page.locator('[data-testid="pile-see-board"]').click();
  await expect(page.locator('[data-testid="pile-board"]')).toBeVisible();
  await expect(page.locator('[data-testid="pile-board-card"]')).toContainText('Premiere fois');
  // « Terminé » renvoie le message d'où il vient — la pile se vide.
  await page.locator('[data-testid="pile-finish"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Repris');
  await expect(page.locator('[data-testid="pile-board"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
  await expect(page.locator('[data-testid="pile-button"]')).toHaveCount(0);
});

test("la barre du fil bascule « Mettre de côté » / « Reprendre »", async () => {
  await page.locator('[data-testid="row"]', { hasText: 'Premiere fois' }).click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  const bascule = page.locator('[data-testid="put-aside"]');
  await expect(bascule).toContainText('Mettre de côté');
  await bascule.click();
  // Le fil vient de quitter sa vue : l'écran retourne à la boîte.
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // Reprendre depuis l'éventail : la carte ouvre l'écran 03, la barre
  // dit « Reprendre », le geste rend le fil à la Réception.
  await page.locator('[data-testid="pile-button"]').click();
  await page.locator('[data-testid="pile-card"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await expect(page.locator('[data-testid="put-aside"]')).toContainText('Reprendre');
  await page.locator('[data-testid="put-aside"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Repris');
  await page.locator('[data-testid="back-to-mailbox"]').click();
  await expect(page.locator('[data-testid="row"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

test('quitter le mode depuis une vue organisée rend la Réception et la nav classique', async () => {
  await page.locator('[data-testid="nav-folder"][data-category="feed"]').click();
  await page.locator('[data-testid="organized-mode"]').click();
  await expect(page.locator('[data-testid="organized-mode"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="nav-folder"]')).toHaveCount(6);
  // Jamais une vue orpheline : la catégorie revient à la Réception.
  await expect(page.locator('[data-testid="row"]').first()).toBeVisible();
  // Le nettoyage rend le poste au classique pour les autres specs.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    const routings = await invoke('routings');
    for (const r of routings) await invoke('remove_routing', { address: r.address });
  });
});
