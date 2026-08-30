<script>
  // Le Portier (PLAN-MODE-ORGANISE E2) — la forme arrêtée au prototype
  // en six passes CE (spikes/mode-organise) : titre et sous-titre
  // centrés, la règle-libellé « Voulez-vous recevoir leurs messages ? »
  // au dessin de « Historique du Portier », puis UN rang par expéditeur
  // en attente AU FORMAT des rangées du volet central (disque non-lu,
  // expéditeur, heure qui ne cède jamais, objet, aperçu) plus l'adresse
  // en clair — la seule donnée que le Portier ajoute. Boutons Oui / Non
  // 44 px à droite, chacun coiffé d'un mini ⋯ : sur Oui il ORIENTE
  // (Réception / Kiosque / Registre), sur Non il pose la RÈGLE
  // (indésirables / archivage / suppression — `corbeille` au cœur, D4 :
  // jamais une suppression définitive). Le clic nu : Oui → Réception,
  // Non → écarté sans règle. **Un oui/non, rien d'autre** — ni tri ni
  // traitement du message au guichet (verdict CE, passe 1) : le rang ne
  // s'ouvre pas. L'expéditeur n'est jamais prévenu ; l'historique dit
  // la règle choisie et « Réintégrer » la défait.
  import Icone from './Icone.svelte';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { t } from './lib/texte.svelte.js';

  let { onchange = () => {}, onflash = () => {} } = $props();

  let rangs = $state([]);
  let ecartes = $state([]);
  // Le mini ⋯ ouvert : { address, qui, type: 'oui'|'non', x, y }.
  let menu = $state(null);

  export async function recharger() {
    try {
      const [attente, historique] = await Promise.all([
        appel('portier_attente'),
        appel('routages'),
      ]);
      rangs = attente;
      // L'historique du guichet ne montre que les ÉCARTÉS (prototype) :
      // un Oui se voit dans sa vue, un Non ne se voit qu'ici.
      ecartes = historique.filter((r) => r.destination === 'ecarte');
    } catch (err) {
      console.error('portier :', err);
    }
  }
  $effect(() => {
    recharger();
  });

  const LIBELLE_ECARTE = {
    spam: 'portier.ecarteSpam',
    archive: 'portier.ecarteArchive',
    corbeille: 'portier.ecarteCorbeille',
  };
  const TOAST_NON = {
    spam: 'toast.portierNonSpam',
    archive: 'toast.portierNonArchive',
    corbeille: 'toast.portierNonCorbeille',
  };
  const BOITE_DE = {
    reception: 'portier.laReception',
    kiosque: 'portier.leKiosque',
    registre: 'portier.leRegistre',
  };

  // Le verdict — la commande E1, LA porte unique du routage. `qui` :
  // le nom d'affichage du rang, pour le toast.
  async function decider(address, qui, destination, regle = null) {
    menu = null;
    try {
      await appel('router_expediteur', { address, destination, regle });
      if (destination === 'ecarte') {
        onflash(t(regle ? TOAST_NON[regle] : 'toast.portierNonNu', { qui }));
      } else if (destination === 'reception') {
        onflash(t('toast.portierOuiNu', { qui }));
      } else {
        onflash(t('toast.portierOuiVers', { qui, boite: t(BOITE_DE[destination]) }));
      }
      await recharger();
      onchange();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }

  async function reintegrer(routage) {
    try {
      await appel('retirer_routage', { address: routage.address });
      onflash(t('toast.portierReintegre', { qui: routage.address }));
      await recharger();
      onchange();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }

  // Le mini ⋯ se pose au coin du bouton ; le menu s'ancre au point du
  // clic, borné à la fenêtre (patron du prototype).
  function ouvrirMini(e, rang, type) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: rang.address,
      qui: rang.row.sender,
      type,
      x: Math.min(r.left, window.innerWidth - 250),
      y: Math.min(r.bottom + 4, window.innerHeight - 170),
    };
  }
</script>

<svelte:window
  onclick={() => (menu = null)}
  onkeydown={(e) => {
    if (e.key === 'Escape') menu = null;
  }} />

<div class="scene" data-testid="portier">
  <div class="colonne">
    <h2 class="display">{t('boite.portier')}</h2>
    <p class="sous-titre">{t('portier.sousTitre1')}<br />{t('portier.sousTitre2')}</p>

    {#if rangs.length}
      <p class="regle-libelle">{t('portier.question')}</p>
      {#each rangs as rang (rang.address)}
        <div class="rang-portier" data-testid="portier-rang" data-adresse={rang.address}>
          <div class="msg" class:nonlu={rang.row.thread_unseen > 0}>
            <div class="l1">
              {#if rang.row.thread_unseen > 0}<span class="disque"></span>{/if}
              <span class="exp">{rang.row.sender}</span>
              <span class="adr">&lt;{rang.address}&gt;</span>
              <span class="essor"></span>
              <span class="heure">{quand(rang.row.epoch)}</span>
            </div>
            <p class="objet">{rang.row.subject}</p>
            <p class="apercu">{rang.row.preview ?? ''}</p>
          </div>
          <div class="choix">
            <span class="btn-portier">
              <button type="button" class="gros" data-testid="portier-oui"
                      onclick={() => decider(rang.address, rang.row.sender, 'reception')}>
                <span class="ic-oui"><Icone nom="check_circle" /></span>{t('portier.oui')}</button>
              <button type="button" class="mini" data-testid="portier-mini-oui"
                      aria-label={t('portier.ouiChoix')} aria-haspopup="menu"
                      aria-expanded={menu?.address === rang.address && menu?.type === 'oui'}
                      onclick={(e) => ouvrirMini(e, rang, 'oui')}>
                <Icone nom="more_horiz" taille={12} /></button>
            </span>
            <span class="btn-portier">
              <button type="button" class="gros" data-testid="portier-non"
                      onclick={() => decider(rang.address, rang.row.sender, 'ecarte')}>
                <span class="ic-non"><Icone nom="cancel" /></span>{t('portier.non')}</button>
              <button type="button" class="mini" data-testid="portier-mini-non"
                      aria-label={t('portier.nonChoix')} aria-haspopup="menu"
                      aria-expanded={menu?.address === rang.address && menu?.type === 'non'}
                      onclick={(e) => ouvrirMini(e, rang, 'non')}>
                <Icone nom="more_horiz" taille={12} /></button>
            </span>
          </div>
        </div>
      {/each}
    {:else}
      <div class="vide" data-testid="portier-vide">
        <span class="ic-oui"><Icone nom="check_circle" /></span>{t('portier.vide')}
      </div>
    {/if}

    <p class="regle-libelle historique">{t('portier.historique')}</p>
    {#if ecartes.length}
      {#each ecartes as routage (routage.address)}
        <div class="rang-historique" data-testid="portier-historique">
          <span class="ic-hist" aria-hidden="true"><Icone nom="visibility_off" /></span>
          <span class="qui"><b>{routage.address}</b> — {t(LIBELLE_ECARTE[routage.regle] ?? 'portier.ecarte')}</span>
          <button type="button" data-testid="portier-reintegrer"
                  onclick={() => reintegrer(routage)}>{t('portier.reintegrer')}</button>
        </div>
      {/each}
    {:else}
      <p class="historique-vide">{t('portier.historiqueVide')}</p>
    {/if}
  </div>
</div>

{#if menu}
  <div class="menu" role="menu" data-testid="portier-menu"
       style="left:{menu.x}px; top:{menu.y}px">
    {#if menu.type === 'oui'}
      <p class="titre-menu">{t('portier.ouiVers')}</p>
      <button type="button" role="menuitem" data-testid="portier-vers-reception"
              onclick={() => decider(menu.address, menu.qui, 'reception')}>
        <Icone nom="inbox" />{t('portier.versReception')}</button>
      <button type="button" role="menuitem" data-testid="portier-vers-kiosque"
              onclick={() => decider(menu.address, menu.qui, 'kiosque')}>
        <Icone nom="kiosque" />{t('portier.versKiosque')}</button>
      <button type="button" role="menuitem" data-testid="portier-vers-registre"
              onclick={() => decider(menu.address, menu.qui, 'registre')}>
        <Icone nom="registre" />{t('portier.versRegistre')}</button>
    {:else}
      <p class="titre-menu">{t('portier.nonSeront')}</p>
      <button type="button" role="menuitem" data-testid="portier-regle-spam"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'spam')}>
        <Icone nom="report" />{t('portier.regleSpam')}</button>
      <button type="button" role="menuitem" data-testid="portier-regle-archive"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'archive')}>
        <Icone nom="inventory_2" />{t('portier.regleArchive')}</button>
      <button type="button" role="menuitem" data-testid="portier-regle-corbeille"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'corbeille')}>
        <Icone nom="delete" />{t('portier.regleCorbeille')}</button>
    {/if}
  </div>
{/if}

<style>
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .colonne { max-width:820px; margin:0 auto; }
  h2 { margin:0 0 6px; font-size:24px; line-height:1.25; color:var(--ink); text-align:center; }
  .sous-titre {
    margin:0 auto 24px; font-size:13px; line-height:1.5; color:var(--ink2);
    max-width:66ch; text-align:center;
  }
  /* La règle-libellé : le dessin de « Historique du Portier » — libellé
     nu, 8 px d'écart, le filet supérieur du premier rang fait
     séparateur (verdict CE, passe finale du prototype). */
  .regle-libelle {
    margin:30px 0 8px; font-size:11px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .regle-libelle.historique { margin-top:34px; }
  .rang-portier {
    display:flex; align-items:center; gap:18px; padding:20px 0;
    border-top:1px solid var(--border);
  }
  /* Le message : LE format des rangées du volet central (l1 / objet /
     aperçu), plus l'adresse en clair. L'heure ne cède jamais. */
  .msg { flex:1; min-width:0; display:grid; grid-template-columns:1fr; row-gap:3px; }
  .l1 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .l1 :global(.disque) { align-self:center; }
  .exp { font-size:14px; font-weight:400; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .nonlu .exp { font-weight:700; }
  .adr { font-size:13px; color:var(--muted); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .essor { flex:1; }
  .heure { font-size:12px; color:var(--muted); flex:none; }
  .objet { margin:0; font-size:14px; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .nonlu .objet { font-weight:700; }
  .apercu { margin:0; font-size:13px; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* Les choix, à DROITE (verdict CE, passe 3) : Oui / Non 44 px, mini ⋯
     au coin haut-droit de chacun. */
  .choix { display:flex; gap:12px; flex:none; }
  .choix .gros { height:44px; padding:0 18px; font-weight:600; }
  .ic-oui :global(.ic) { color:var(--accent); }
  .ic-non :global(.ic) { color:var(--alert); }
  .btn-portier { position:relative; display:inline-flex; }
  .mini {
    position:absolute; top:-8px; right:-8px; width:19px; height:19px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); color:var(--muted); cursor:pointer;
  }
  .mini:hover, .mini[aria-expanded="true"] { background:var(--sel); color:var(--ink); }
  .vide {
    display:flex; align-items:center; justify-content:center; gap:8px;
    padding:12px 0; font-size:13px; color:var(--ink2);
  }
  .rang-historique {
    display:flex; align-items:center; gap:10px; padding:10px 2px;
    border-top:1px solid var(--border); font-size:13px; color:var(--ink2);
  }
  .ic-hist :global(.ic) { color:var(--alert); }
  .qui { flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .rang-historique button { height:28px; padding:0 12px; font-size:12px; }
  .historique-vide { margin:8px 0 0; font-size:13px; line-height:1.5; color:var(--ink2); max-width:66ch; }
  /* Le menu du mini ⋯ — le dessin des menus du produit. */
  .menu {
    position:fixed; z-index:30; min-width:230px; padding:6px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:var(--ombre, 0 8px 24px rgba(0,0,0,.14));
    display:flex; flex-direction:column; gap:2px;
  }
  .titre-menu {
    margin:4px 8px 6px; font-size:11px; letter-spacing:.06em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
  .menu button {
    display:flex; align-items:center; gap:8px; text-align:left;
    border:1px solid transparent; background:none; height:32px; padding:0 8px;
  }
  .menu button:hover { background:var(--hover); }
</style>
