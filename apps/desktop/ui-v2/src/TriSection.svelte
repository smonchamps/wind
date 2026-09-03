<script>
  // RETOURS-14 R9 (terrain 2026-08-31, 2e passe) : LE bouton de tri
  // d'une section — il ouvre un MENU déroulant des quatre tris (plus
  // récents / plus anciens / expéditeur A → Z / Z → A), chaque entrée
  // avec son glyphe (`tri_*`, A104 — jeu 87 → 91). Le bouton est au
  // dessin des boutons nus « Tout déplier » (Fil) et se justifie à
  // droite par la LIGNE qui l'accueille ; l'état vit chez l'appelant —
  // quatre surfaces, UN composant (leçon D-47). Le menu est le dessin
  // des menus du produit.
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import { t } from './lib/texte.svelte.js';

  let { value = 'date-desc', onchanger = () => {} } = $props();

  const TRIS = [
    { id: 'date-desc', libelle: 'tri.dateDesc', icon: 'tri_recent' },
    { id: 'date-asc', libelle: 'tri.dateAsc', icon: 'tri_ancien' },
    { id: 'alpha-az', libelle: 'tri.alphaAZ', icon: 'tri_az' },
    { id: 'alpha-za', libelle: 'tri.alphaZA', icon: 'tri_za' },
  ];
  const courant = $derived(TRIS.find((x) => x.id === value) ?? TRIS[0]);

  let ouvert = $state(false);
  let x = $state(0);
  let y = $state(0);
  function basculer(e) {
    e.stopPropagation();
    if (ouvert) {
      ouvert = false;
      return;
    }
    const r = e.currentTarget.getBoundingClientRect();
    x = r.right - 230;
    y = r.bottom + 4;
    ouvert = true;
  }
  function choisir(id) {
    ouvert = false;
    onchanger(id);
  }
</script>

<button type="button" class="nu" data-testid="tri-section"
        title={t('tri.aria')} aria-label={t('tri.aria')}
        aria-haspopup="menu" aria-expanded={ouvert}
        onclick={basculer}>
  <Icone name={courant.icon} />{t(courant.libelle)}</button>

<Menu ouvert={ouvert} x={x} y={y} testid="tri-menu" largeur={220}
      onfermer={() => (ouvert = false)}>
    {#each TRIS as choix (choix.id)}
      <button type="button" role="menuitemradio" data-testid={`tri-${choix.id}`}
              aria-checked={choix.id === value}
              onclick={() => choisir(choix.id)}>
        <Icone name={choix.icon} />{t(choix.libelle)}</button>
    {/each}
  </Menu>

<style>
  /* Le dessin exact du bouton nu du fil (« Tout déplier ») — copié
     ici faute de classe globale ; si une troisième copie apparaît,
     la promouvoir à systeme.css (patron .entete-vue). */
  .nu {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-controle); cursor:pointer;
    white-space:nowrap; flex:none;
  }
  .nu:hover, .nu[aria-expanded="true"] { background:var(--sel); }
</style>
