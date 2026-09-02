<script>
  // RETOURS-14 R6 (D7) : le Registre GROUPÉ par expéditeur — un rang
  // par expéditeur routé, trié par récence du dernier message (le
  // patron du Nettoyage, jamais l'alphabet — D7), déplié sur ses fils.
  // La donnée reste le flot routé (`registre_groupes` /
  // `registre_groupe_page`, mêmes bornes que la vue plate) ; ouvrir un
  // fil passe par le chemin de la liste (`onouvrir` → App), le volet
  // de lecture reste le lecteur.
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import TriSection from './TriSection.svelte';
  import { appel } from './lib/transport.js';
  import { comparateurTri } from './lib/tri.js';
  import { quand } from './lib/quand.js';
  import { t } from './lib/texte.svelte.js';
  import { activation } from './lib/clavier.js';

  let {
    compte = null,
    onouvrir = () => {},
    ontotal = () => {},
    // Revue : les gestes d'expéditeur ne meurent pas avec la vue plate
    // — le ⋯ d'un groupe route l'expéditeur ENTIER (Déplacer vers…,
    // Écarter) ; l'App possède la commande et les toasts.
    onrouter = () => {},
  } = $props();

  const PAGE = 200;

  let groupes = $state([]);
  let servi = $state(false);
  let ouvert = $state(null);
  let messagesOuverts = $state([]);
  // Revue : la page d'un groupe est PAGINÉE — « Voir plus » charge la
  // suite, jamais une troncature silencieuse à 200 pendant que le rang
  // annonce le vrai compte.
  let plusPossible = $state(false);
  let chargementPlus = $state(false);
  // Le ⋯ d'un groupe : { address, qui, x, y } (patron des menus du
  // produit — famille D-47, consignée).
  let menu = $state(null);
  // R9 : le tri de la section — récence par défaut (D7), le bouton
  // cycle ; l'ordre servi par le cœur reste la récence, le tri est
  // une PRÉSENTATION (les groupes sont déjà tous là).
  let tri = $state('date-desc');
  let jeton = 0;
  const groupesTries = $derived(
    [...groupes].sort(comparateurTri(tri, (g) => g.dernierEpoch, (g) => g.qui ?? g.address)),
  );

  async function charger() {
    const j = (jeton += 1);
    try {
      const g = await appel('registre_groupes', { accountId: compte });
      if (j !== jeton) return;
      groupes = g;
      servi = true;
      ontotal(g.reduce((n, x) => n + x.fils, 0));
      // Le groupe déplié a pu disparaître (verdict retiré ailleurs).
      if (ouvert && !g.some((x) => x.address === ouvert)) {
        ouvert = null;
        messagesOuverts = [];
      }
    } catch (err) {
      console.error('registre_groupes :', err);
    }
  }

  export function recharger() {
    charger();
  }

  $effect(() => {
    void compte;
    servi = false;
    ouvert = null;
    messagesOuverts = [];
    charger();
  });

  async function basculerGroupe(address) {
    if (ouvert === address) {
      ouvert = null;
      messagesOuverts = [];
      plusPossible = false;
      return;
    }
    ouvert = address;
    messagesOuverts = [];
    plusPossible = false;
    try {
      const rows = await appel('registre_groupe_page', {
        address, accountId: compte, offset: 0, limit: PAGE,
      });
      if (ouvert === address) {
        messagesOuverts = rows;
        plusPossible = rows.length === PAGE;
      }
    } catch (err) {
      console.error('registre_groupe_page :', err);
    }
  }
  async function chargerPlus() {
    if (!ouvert || chargementPlus) return;
    chargementPlus = true;
    const address = ouvert;
    try {
      const rows = await appel('registre_groupe_page', {
        address, accountId: compte, offset: messagesOuverts.length, limit: PAGE,
      });
      if (ouvert === address) {
        messagesOuverts = [...messagesOuverts, ...rows];
        plusPossible = rows.length === PAGE;
      }
    } catch (err) {
      console.error('registre_groupe_page :', err);
    } finally {
      chargementPlus = false;
    }
  }
  function ouvrirMenu(e, g) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: g.address, qui: g.qui ?? g.address,
      x: r.left,
      y: r.bottom + 4,
    };
  }
  function geste(destination) {
    const { address, qui } = menu;
    menu = null;
    onrouter(address, qui, destination);
  }
</script>

<div class="scene" data-testid="registre">
  <div class="colonne">
    <!-- R2/R11 : l'entête normalisé des vues du mode — glyphe + titre,
         classes partagées de systeme.css. Titre seul (patron D2). -->
    <h2 class="display entete-vue" data-testid="registre-titre">
      <span class="glyphe-titre" aria-hidden="true"><Icone nom="registre" taille={26} /></span>{t('boite.registre')}
      <span class="essor-titre"></span>
      <TriSection valeur={tri} onchanger={(v) => (tri = v)} /></h2>
    {#if groupes.length}
      {#each groupesTries as g (g.address)}
        <!-- Le rang est un div-bouton (patron .ligne de la Liste) : le
             ⋯ vit DEDANS — un bouton niché dans un bouton serait
             invalide et inatteignable au clavier. -->
        <div class="rang-groupe" data-testid="registre-groupe" role="button"
             tabindex="0" data-adresse={g.address} aria-expanded={ouvert === g.address}
             onclick={() => basculerGroupe(g.address)}
             onkeydown={activation(() => basculerGroupe(g.address))}>
          <span class="empile" aria-hidden="true"><span></span><span></span><span></span></span>
          <span class="corps">
            <span class="l1">
              <span class="exp">{g.qui ?? g.address}</span>
              <span class="essor"></span>
              <span class="heure">{quand(g.dernierEpoch)}</span>
            </span>
            <span class="l2">
              <span class="nombre">{t(g.fils > 1 ? 'registre.fils' : 'registre.fil', { n: g.fils })}</span>
              {#if g.dernierObjet}<span class="objet">{g.dernierObjet}</span>{/if}
            </span>
          </span>
          <span class="gestes" role="button" tabindex="0"
                data-testid="registre-gestes" aria-haspopup="menu"
                aria-expanded={menu?.address === g.address}
                aria-label={t('liste.gestes')}
                onclick={(e) => ouvrirMenu(e, g)}
                onkeydown={(e) => e.key === 'Enter' && ouvrirMenu(e, g)}>
            <Icone nom="more_horiz" taille={16} /></span>
        </div>
        {#if ouvert === g.address}
          <div class="dedans" data-testid="registre-messages">
            {#each messagesOuverts as m (m.account_id + '/' + m.mailbox + '/' + m.uid)}
              <button type="button" class="rang-message" data-testid="registre-message"
                      class:nonlu={m.thread_unseen > 0}
                      onclick={() => onouvrir(m)}>
                <span class="objet-m">{m.subject}</span>
                <span class="essor"></span>
                <span class="heure">{quand(m.epoch)}</span>
              </button>
            {/each}
            {#if plusPossible}
              <button type="button" class="voir-plus" data-testid="registre-plus"
                      disabled={chargementPlus} onclick={chargerPlus}>
                {t('registre.voirPlus')}</button>
            {/if}
          </div>
        {/if}
      {/each}
    {:else if servi}
      <p class="vide" data-testid="registre-vide">{t('liste.vide')}</p>
    {/if}
  </div>
</div>

<Menu ouvert={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="registre-menu" largeur={220} onfermer={() => (menu = null)}>
    {#each ['reception', 'kiosque'] as dest (dest)}
      <button type="button" role="menuitem" data-testid={`registre-vers-${dest}`}
              onclick={() => geste(dest)}>
        <Icone nom={dest === 'reception' ? 'inbox' : 'kiosque'} />{t('liste.deplacerVers', { boite: t(`boite.${dest}`) })}</button>
    {/each}
    <div class="filet"></div>
    <button type="button" role="menuitem" data-testid="registre-ecarter"
            onclick={() => geste('ecarte')}>
      <Icone nom="visibility_off" />{t('liste.ecarter')}</button>
  </Menu>

<style>
  /* La scène du Registre — la géométrie du Kiosque (colonne centrée,
     la scène défile). */
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .essor-titre { flex:1; }
  .colonne { max-width:720px; margin:0 auto; }
  /* Le rang d'un groupe : la pile du Kiosque + les deux lignes du
     Nettoyage (expéditeur/heure, nombre/objet du dernier). */
  .rang-groupe {
    width:100%; display:flex; align-items:center; gap:12px;
    padding:12px 10px; font-size:13px; color:var(--ink); text-align:left;
    background:none; border:none; border-top:1px solid var(--border);
    cursor:pointer;
  }
  .rang-groupe:hover { background:var(--hover); }
  .empile { position:relative; width:20px; height:16px; flex:none; }
  .empile span {
    position:absolute; inset:0; background:var(--surface);
    border:1px solid var(--border);
  }
  .empile span:nth-child(1) { transform:translate(4px, -4px); }
  .empile span:nth-child(2) { transform:translate(2px, -2px); }
  .corps { flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .l1, .l2 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .exp {
    font-weight:600; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .essor { flex:1; }
  .heure { flex:none; font-size:12px; color:var(--muted); font-variant-numeric:tabular-nums; }
  .nombre { flex:none; font-size:12px; font-weight:600; color:var(--accent); font-variant-numeric:tabular-nums; }
  .l2 .objet {
    min-width:0; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* Les fils d'un groupe déplié — rangées nues, décrochées sous la
     pile ; le non-lu garde son gras (la règle de la liste). */
  .dedans { border-top:1px solid var(--border); }
  .rang-message {
    width:100%; display:flex; align-items:baseline; gap:8px;
    padding:8px 10px 8px 42px; font-size:13px; color:var(--ink);
    text-align:left; background:none; border:none; cursor:pointer;
  }
  .rang-message:hover { background:var(--hover); }
  .rang-message .objet-m {
    min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .rang-message.nonlu .objet-m { font-weight:600; }
  .vide {
    margin:24px 0 0; font-size:13px; line-height:1.5; color:var(--muted);
  }
  /* Le ⋯ : place réservée, opacité seule (patron Liste/Kiosque). */
  .gestes {
    flex:none; width:24px; height:24px; align-self:center;
    display:inline-flex; align-items:center; justify-content:center;
    opacity:0; color:var(--muted); border:1px solid transparent;
  }
  .rang-groupe:hover .gestes, .gestes:focus-visible,
  .gestes[aria-expanded="true"] { opacity:1; }
  .gestes:hover, .gestes[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  .voir-plus {
    margin:6px 0 10px 42px; height:30px; padding:0 14px;
    display:inline-flex; align-items:center; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
    cursor:pointer;
  }
  .voir-plus:hover { background:var(--sel); }
  .voir-plus:disabled { opacity:.6; cursor:default; }
</style>
