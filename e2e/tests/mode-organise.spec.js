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
import { launchAppV2, closeApp } from '../launch.mjs';

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
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(8);

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
  await expect(page.locator('[data-testid="nav-dossier"]')).toHaveCount(8);

  // Le Kiosque montre désormais le courrier des expéditeurs routés…
  await page.locator('[data-testid="nav-dossier"][data-categorie="kiosque"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // …le Registre reste vide (la destination filtre vraiment)…
  await page.locator('[data-testid="nav-dossier"][data-categorie="registre"]').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText('Registre');
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(0);
  // …et la Réception montre TOUJOURS tout (E1 : la rétention du
  // Portier est l'affaire d'E2 — rien ne quitte le flot aujourd'hui).
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
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
