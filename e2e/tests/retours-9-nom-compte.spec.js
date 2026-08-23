// PLAN-RETOURS-9 (D3/D4) : le nom personnalisé d'un compte. Posé
// depuis Réglages > Comptes (carte sous la rangée, patron repère), il
// REMPLACE l'adresse dans la nav ; en Réglages il s'affiche AVEC
// l'adresse ; au composeur le sélecteur dit « Nom — adresse »
// (l'adresse reste la donnée fonctionnelle d'envoi). Vidé, l'adresse
// revient partout. Décor : deux comptes — le nom n'a de sens que
// lorsqu'il distingue.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

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
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const boiteNav = (libelle) =>
  page.locator('[data-testid="nav-boite"]', { hasText: libelle });

test('nommer un compte depuis Réglages : la nav prend le nom, la rangée garde l’adresse', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(boiteNav('un@exemple.fr')).toHaveCount(1);

  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="compte-nommer"]').first().click();
  await expect(page.locator('[data-testid="reglages-nom"]')).toBeVisible();
  await page.locator('[data-testid="nom-champ"]').fill('Boulot');
  await page.locator('[data-testid="nom-enregistrer"]').click();
  await expect(page.locator('[data-testid="reglages-nom"]')).toHaveCount(0);

  // En Réglages, le nom s'affiche AVEC l'adresse (D4) — l'adresse
  // reste la vérité de connexion.
  const rangee = page.locator('[data-testid="reglages-comptes"] .compte').first();
  await expect(rangee).toContainText('Boulot');
  await expect(rangee).toContainText('un@exemple.fr');
  await page.locator('[data-testid="reglages-termine"]').click();

  // La nav : le nom REMPLACE l'adresse ; l'autre compte ne bouge pas.
  await expect(boiteNav('Boulot')).toHaveCount(1);
  await expect(boiteNav('un@exemple.fr')).toHaveCount(0);
  await expect(boiteNav('deux@exemple.fr')).toHaveCount(1);
});

test('au composeur, le sélecteur d’expéditeur dit « Nom — adresse »', async () => {
  await page.locator('[data-testid="ecrire"]').click();
  const de = page.locator('select[data-testid="composition-de"]');
  await expect(de.locator('option').first()).toHaveText('Boulot — un@exemple.fr');
  await expect(de.locator('option').nth(1)).toHaveText('deux@exemple.fr');
  // Fermer le composeur (vide : rien à conserver) — son voile
  // intercepterait sinon les clics du test suivant.
  await page.locator('[data-testid="composition"] button[aria-label="Fermer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
});

test('vider le nom rend l’adresse à la nav', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="compte-nommer"]').first().click();
  await page.locator('[data-testid="nom-champ"]').fill('');
  await page.locator('[data-testid="nom-enregistrer"]').click();
  // Attendre la clôture de la carte (l'écriture est passée) avant de
  // fermer la surimpression — sinon un nom_set lent meurt en timeout
  // opaque sur la nav.
  await expect(page.locator('[data-testid="reglages-nom"]')).toHaveCount(0);
  await page.locator('[data-testid="reglages-termine"]').click();

  await expect(boiteNav('un@exemple.fr')).toHaveCount(1);
  await expect(boiteNav('Boulot')).toHaveCount(0);
});
