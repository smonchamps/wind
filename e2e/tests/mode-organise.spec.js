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
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  const bascule = page.locator('[data-testid="mode-organise"]');
  await expect(bascule).toBeVisible();
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  // La garde « classique inchangé » : exactement les six dossiers
  // canoniques, ni Kiosque ni Registre.
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(6);
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]')).toHaveCount(0);
});

test('la bascule recompose la nav, le Kiosque sert les expéditeurs routés, et le mode PERSISTE', async () => {
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(9);

  // Le Kiosque avant tout routage : rien — le filtre est réel, pas un
  // décor (le Registre le reprouve plus bas après routage).
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText('Kiosque');
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(0);

  // Route les expéditeurs du jeu d'essai vers le Kiosque, par LA
  // commande du produit (le geste « Déplacer vers… » arrive plus tard
  // dans E1 — le service, lui, est déjà le vrai).
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    for (let n = 0; n < 12; n += 1) {
      await invoke('router_expediteur', {
        address: `expediteur${n}@exemple.fr`,
        destination: 'kiosque',
        regle: null,
      });
    }
  });

  // La persistance est en BASE (prefs SQLite) : un rechargement complet
  // relit le mode du cœur — jamais du localStorage.
  await page.reload();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(9);

  // Le Kiosque montre désormais le courrier des expéditeurs routés…
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // …le Registre reste vide (la destination filtre vraiment)…
  await page.locator('[data-testid="nav-dossier"][data-categorie="registre"]').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText('Registre');
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(0);
  // …et la Réception ORGANISÉE ne les montre plus (E2 : un fil routé
  // ailleurs vit dans SA vue — l'exclusion partagée du flot ; tout est
  // au Kiosque ici, la Réception organisée est donc vide).
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  // Le vide est AFFIRMÉ (« Aucun message ici. ») — jamais un décompte
  // à zéro pendant que la page charge encore.
  await expect(page.locator('[data-testid="liste"]')).toContainText('Aucun message ici.');
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(0);
});

test("« Déplacer vers… » route l'expéditeur ENTIER depuis la barre du fil", async () => {
  // Tout est au Kiosque (test précédent) ; on ouvre un fil et on
  // déplace son expéditeur au Registre — ce que l'utilisateur VOIT :
  // le menu, le toast, puis le courrier de l'expéditeur au Registre.
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="deplacer-vers"]').click();
  await page.locator('[data-testid="deplacer-registre"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Registre');
  await page.locator('[data-testid="nav-dossier"][data-categorie="registre"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Le geste n'existe qu'en mode organisé : la garde du classique.
  await page.locator('[data-testid="mode-organise"]').click();
  await page.locator('[data-testid="ligne"]').first().click();
  await expect(page.locator('[data-testid="deplacer-vers"]')).toHaveCount(0);
  await page.locator('[data-testid="mode-organise"]').click();
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
    const routages = await invoke('routages');
    for (const r of routages) await invoke('retirer_routage', { address: r.address });
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
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  // Le connu arrive en Réception ; l'inconnue n'y est PAS.
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Suite du dossier' })).toHaveCount(1);
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Premiere fois' })).toHaveCount(0);
  // La pastille du Portier compte SON message.
  const rangPortier = page.locator('[data-testid="nav-dossier"][data-categorie="portier"]');
  await expect(rangPortier).toContainText('Portier');
  await expect(rangPortier).toContainText('1');
  // Le guichet : un rang au format des rangées, l'adresse en clair.
  await rangPortier.click();
  await expect(page.locator('[data-testid="portier"]')).toContainText('Voulez-vous recevoir leurs messages ?');
  const rang = page.locator('[data-testid="portier-rang"]');
  await expect(rang).toHaveCount(1);
  await expect(rang).toContainText('Nouvelle Venue');
  await expect(rang).toContainText('<inconnue@exemple.fr>');
  await expect(rang).toContainText('Premiere fois');
});

test("le Oui nu rend l'expéditeur à la Réception, le guichet se vide", async () => {
  await page.locator('[data-testid="portier-oui"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('peut vous écrire');
  await expect(page.locator('[data-testid="portier-vide"]')).toBeVisible();
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

test("le Non avec règle écarte, l'historique le dit, « Réintégrer » rend au Portier", async () => {
  injecterArrivee({
    email: 'principal@exemple.fr', expediteur: 'promo@exemple.fr',
    nom: 'Promo Eclair', sujet: 'Offre eclair', n: 2,
  });
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await page.locator('[data-testid="nav-dossier"][data-categorie="portier"]').click();
  await expect(page.locator('[data-testid="portier-rang"]')).toHaveCount(1);
  // Le mini ⋯ du Non pose la règle — « Archivés automatiquement ».
  await page.locator('[data-testid="portier-mini-non"]').click();
  await page.locator('[data-testid="portier-regle-archive"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('archiveront automatiquement');
  await expect(page.locator('[data-testid="portier-vide"]')).toBeVisible();
  const historique = page.locator('[data-testid="portier-historique"]');
  await expect(historique).toHaveCount(1);
  await expect(historique).toContainText('promo@exemple.fr');
  await expect(historique).toContainText('archivage automatique');
  // « Réintégrer » défait le verdict : l'inconnu RE-attend au guichet.
  await page.locator('[data-testid="portier-reintegrer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('réintégré');
  await expect(page.locator('[data-testid="portier-rang"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="portier-rang"]')).toContainText('promo@exemple.fr');
});

test('le mode classique montre TOUJOURS tout — la rétention est une affaire du mode organisé', async () => {
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'false');
  // L'inconnu encore en attente (promo) est VISIBLE au classique.
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Premiere fois' })).toHaveCount(1);
  // Le retour au mode organisé, SANS naviguer : la liste affichée se
  // ressert d'elle-même (revue E2 — la bascule rechargeait la nav mais
  // pas la Réception, l'écran gardait la page de l'autre mode).
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Offre eclair' })).toHaveCount(0);
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Premiere fois' })).toHaveCount(1);
});

// ------------------- E3 — les règles du Non exécutées -------------------
test("la règle du Non s'exécute à l'arrivée — et ne touche jamais l'antérieur au verdict", async () => {
  // promo@ re-attend au guichet (test précédent) : le Non avec règle
  // « Supprimés automatiquement » (corbeille au cœur, D4).
  await page.locator('[data-testid="nav-dossier"][data-categorie="portier"]').click();
  await page.locator('[data-testid="portier-mini-non"]').click();
  await page.locator('[data-testid="portier-regle-corbeille"]').click();
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
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="nav-dossier"][data-categorie="portier"]').click();
  await expect(page.locator('[data-testid="portier-rang"]')).toContainText('temoin@exemple.fr');
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Temoin de synchro' })).toHaveCount(1);
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Offre eclair' }).first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne"]', { hasText: 'Relance finale' })).toHaveCount(0);
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'true');
});

test('quitter le mode depuis une vue organisée rend la Réception et la nav classique', async () => {
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  await page.locator('[data-testid="mode-organise"]').click();
  await expect(page.locator('[data-testid="mode-organise"]')).toHaveAttribute('aria-checked', 'false');
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(6);
  // Jamais une vue orpheline : la catégorie revient à la Réception.
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Le nettoyage rend le poste au classique pour les autres specs.
  await page.evaluate(async () => {
    const invoke = window.__TAURI__.core.invoke;
    const routages = await invoke('routages');
    for (const r of routages) await invoke('retirer_routage', { address: r.address });
  });
});
