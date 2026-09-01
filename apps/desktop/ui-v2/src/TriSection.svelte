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
  import { t } from './lib/texte.svelte.js';

  let { valeur = 'date-desc', onchanger = () => {} } = $props();

  const TRIS = [
    { id: 'date-desc', libelle: 'tri.dateDesc', icone: 'tri_recent' },
    { id: 'date-asc', libelle: 'tri.dateAsc', icone: 'tri_ancien' },
    { id: 'alpha-az', libelle: 'tri.alphaAZ', icone: 'tri_az' },
    { id: 'alpha-za', libelle: 'tri.alphaZA', icone: 'tri_za' },
  ];
  const courant = $derived(TRIS.find((x) => x.id === valeur) ?? TRIS[0]);

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
    x = Math.min(r.right - 230, window.innerWidth - 240);
    y = Math.min(r.bottom + 4, window.innerHeight - 170);
    ouvert = true;
  }
  function choisir(id) {
    ouvert = false;
    onchanger(id);
  }
  $effect(() => {
    if (!ouvert) return;
    const fermer = () => (ouvert = false);
    window.addEventListener('click', fermer);
    window.addEventListener('keydown', fermer);
    return () => {
      window.removeEventListener('click', fermer);
      window.removeEventListener('keydown', fermer);
    };
  });
</script>

<button type="button" class="nu" data-testid="tri-section"
        title={t('tri.aria')} aria-label={t('tri.aria')}
        aria-haspopup="menu" aria-expanded={ouvert}
        onclick={basculer}>
  <Icone nom={courant.icone} />{t(courant.libelle)}</button>

{#if ouvert}
  <div class="menu-tri" role="menu" data-testid="tri-menu"
       style="left:{x}px; top:{y}px">
    {#each TRIS as choix (choix.id)}
      <button type="button" role="menuitemradio" data-testid={`tri-${choix.id}`}
              aria-checked={choix.id === valeur}
              onclick={() => choisir(choix.id)}>
        <Icone nom={choix.icone} />{t(choix.libelle)}</button>
    {/each}
  </div>
{/if}

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
  /* Le menu — le dessin des menus du produit (famille D-47). */
  .menu-tri {
    position:fixed; z-index:6; min-width:220px; display:flex;
    flex-direction:column; background:var(--surface);
    border:1px solid var(--border); box-shadow:var(--shadow); padding:4px;
  }
  .menu-tri button {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:10px; font-size:13px; color:var(--ink); background:none;
    border:none; cursor:pointer; text-align:left;
  }
  .menu-tri button:hover { background:var(--sel); }
  .menu-tri button[aria-checked="true"] { font-weight:600; }
</style>
