<script>
  // La pile « Mis de côté » (PLAN-MODE-ORGANISE E5) — la forme du
  // prototype : un bouton-pile en bas à droite de la Réception
  // organisée (visuel de trois feuilles, libellé + compte à l'accent),
  // l'ÉVENTAIL des mini-cartes au clic (une carte = objet + expéditeur
  // · heure, au sol de tuile), « Voir le tableau » = les aperçus en
  // grille sur un écran, « Terminé » renvoie le message d'où il vient.
  // Les données viennent du cœur (`pile_mis_de_cote`, les têtes des
  // fils au squelette unifié) ; les gestes remontent à l'App via
  // `onchange` — le composant possède la pile, jamais les listes.
  import Icone from './Icone.svelte';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { t } from './lib/texte.svelte.js';

  let { onouvrir = () => {}, onchange = () => {}, onflash = () => {} } = $props();

  let cartes = $state([]);
  let eventail = $state(false);
  let tableau = $state(false);

  export async function recharger() {
    try {
      cartes = await appel('pile_mis_de_cote');
    } catch (err) {
      console.error('pile :', err);
    }
  }
  $effect(() => {
    recharger();
  });

  function ouvrir(ligne) {
    eventail = false;
    tableau = false;
    onouvrir(ligne);
  }

  // « Terminé » : le fil quitte la pile et revient d'où il vient —
  // LA commande du produit, puis la pile ET les listes se resservent.
  async function terminer(ligne) {
    try {
      await appel('toggle_mis_de_cote', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      onflash(t('toast.reprisPile'));
      await recharger();
      if (cartes.length === 0) tableau = false;
      onchange();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === 'Escape') {
      eventail = false;
      tableau = false;
    }
  }} />

{#if cartes.length > 0}
  <div class="pile-zone">
    {#if eventail}
      <div class="eventail" role="dialog" aria-label={t('pile.aria')} data-testid="pile-eventail">
        <p class="tete-e">{t('pile.misDeCote')}</p>
        {#each cartes as ligne (`${ligne.account_id}:${ligne.mailbox}:${ligne.uid}`)}
          <button type="button" class="carte-e" data-testid="pile-carte"
                  onclick={() => ouvrir(ligne)}>
            <span class="o">{ligne.subject}</span>
            <span class="e">{ligne.sender} · {quand(ligne.epoch)}</span>
          </button>
        {/each}
        <div class="pied-e">
          <button type="button" data-testid="pile-voir-tableau"
                  onclick={() => { eventail = false; tableau = true; }}>
            <Icone nom="pile" />{t('pile.voirTableau')}</button>
        </div>
      </div>
    {/if}
    <button type="button" class="pile-bouton" data-testid="pile-bouton"
            aria-expanded={eventail}
            onclick={() => (eventail = !eventail)}>
      <span class="pile-visuel" aria-hidden="true"><span></span><span></span><span></span></span>
      <span class="pile-libelle">{t('pile.misDeCote')} <span class="n">{cartes.length}</span></span>
    </button>
  </div>
{/if}

{#if tableau}
  <!-- L'écran du tableau : les aperçus en grille, plein écran — la
       surimpression du prototype (retour, titre, note, cartes). -->
  <div class="tableau" data-testid="pile-tableau">
    <div class="tableau-int">
      <div class="tete-t">
        <button type="button" class="retour-t" data-testid="pile-tableau-retour"
                aria-label={t('action.fermer')}
                onclick={() => (tableau = false)}>
          <Icone nom="arrow_back" /></button>
        <h2 class="display">{t('pile.tableauTitre')}</h2>
      </div>
      <p class="note-t"><Icone nom="info" />{t('pile.tableauNote')}</p>
      <div class="grille">
        {#each cartes as ligne (`${ligne.account_id}:${ligne.mailbox}:${ligne.uid}`)}
          <div class="carte-t" data-testid="pile-tableau-carte">
            <span class="e">{ligne.sender} · {quand(ligne.epoch)}</span>
            <span class="o">{ligne.subject}</span>
            <span class="x">{ligne.preview ?? ''}</span>
            <div class="actions-t">
              <button type="button" onclick={() => ouvrir(ligne)}>{t('pile.ouvrir')}</button>
              <button type="button" data-testid="pile-terminer"
                      onclick={() => terminer(ligne)}>
                <Icone nom="check" />{t('action.termine')}</button>
            </div>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .pile-zone {
    position:absolute; right:28px; bottom:64px; z-index:20;
    display:flex; flex-direction:column; align-items:flex-end; gap:10px;
  }
  .pile-bouton {
    height:auto; padding:10px 14px 12px; display:flex; flex-direction:column;
    align-items:center; gap:8px; background:var(--surface);
    border:1px solid var(--border); box-shadow:0 8px 24px rgba(0,0,0,.14);
    cursor:pointer;
  }
  .pile-bouton:hover { background:var(--sel); }
  .pile-visuel { position:relative; width:52px; height:38px; }
  .pile-visuel span {
    position:absolute; left:0; right:0; height:30px;
    background:var(--tuile); border:1px solid var(--border);
  }
  .pile-visuel span:nth-child(1) { top:8px; transform:rotate(-3deg); }
  .pile-visuel span:nth-child(2) { top:4px; transform:rotate(2deg); }
  .pile-visuel span:nth-child(3) { top:0; background:var(--surface); }
  .pile-libelle {
    font-size:12px; font-weight:600; color:var(--ink2); display:flex; gap:5px;
  }
  .pile-libelle .n { color:var(--accent); font-variant-numeric:tabular-nums; }
  .eventail {
    width:330px; max-height:420px; overflow:auto; display:flex;
    flex-direction:column; background:var(--surface);
    border:1px solid var(--border); box-shadow:0 8px 24px rgba(0,0,0,.14);
  }
  .tete-e {
    margin:0; padding:12px 14px 8px; font-size:11px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .carte-e {
    display:flex; flex-direction:column; gap:2px; padding:10px 14px;
    border:none; border-top:1px solid var(--border); cursor:pointer;
    text-align:left; background:var(--tuile); color:var(--tuileInk);
  }
  .carte-e:hover { filter:brightness(0.97); }
  .carte-e .o {
    font-size:13px; font-weight:600; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap;
  }
  .carte-e .e {
    font-size:12px; opacity:.85; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap;
  }
  .pied-e { padding:10px 14px; border-top:1px solid var(--border); }
  .pied-e button { width:100%; justify-content:center; }
  /* Le tableau — la surimpression plein écran (z au-dessus des volets,
     sous les modales). */
  .tableau {
    position:fixed; inset:0; z-index:25; background:var(--bg);
    overflow:auto; padding:18px 28px 40px;
  }
  .tableau-int { max-width:1080px; margin:0 auto; }
  .tete-t { display:flex; align-items:center; gap:10px; }
  .tete-t h2 { margin:0; font-size:24px; line-height:1.25; color:var(--ink); }
  .retour-t {
    width:32px; height:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; flex:none;
  }
  .note-t {
    display:flex; align-items:baseline; gap:8px; margin:10px 0 22px;
    font-size:13px; line-height:1.5; color:var(--ink2); max-width:70ch;
  }
  .note-t :global(.ic) { color:var(--muted); align-self:center; flex:none; }
  .grille {
    display:grid; grid-template-columns:repeat(auto-fill, minmax(300px, 1fr));
    gap:14px;
  }
  .carte-t {
    display:flex; flex-direction:column; gap:6px; padding:14px;
    background:var(--surface); border:1px solid var(--border);
  }
  .carte-t .e { font-size:12px; color:var(--muted); }
  .carte-t .o { font-size:14px; font-weight:600; color:var(--ink); }
  .carte-t .x {
    font-size:13px; color:var(--ink2); line-height:1.5;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical;
    overflow:hidden;
  }
  .actions-t { display:flex; gap:8px; margin-top:6px; }
</style>
