<script>
  // Le Nettoyage de printemps (PLAN-HORIZON-NETTOYAGE, volet B) — la 5e
  // section du Mode organisé. Deux écrans dans UNE scène :
  // — l'INTRO (EB1) : entête au patron du Portier, sous-texte CE mot
  //   pour mot, plage et périmètre (D6), « Démarrer le nettoyage » ;
  // — le TRI (EB3) : l'organisation du Portier, mais les rangs sont des
  //   GROUPES par expéditeur — le Oui/Non vaut pour le groupe entier et
  //   s'applique au stock de la plage ET à l'avenir (D5) ; on entre
  //   dans un groupe pour VOIR ses messages (jamais trier au message —
  //   refus de périmètre du PLAN) ; la barre de progression en haut dit
  //   le pourcentage de groupes traités. La session est PERSISTÉE
  //   (D8) : rouvrir la section reprend le tri où il en était. Le clic
  //   nu suit les défauts du Portier (D9), le mini ⋯ déroge.
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import TriSection from './TriSection.svelte';
  import { comparateurTri } from './lib/tri.js';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { t } from './lib/texte.svelte.js';
  import {
    PLAGES_NETTOYAGE as PLAGES,
    PERIMETRES_NETTOYAGE as PERIMETRES,
  } from './lib/vocabulaires.js';

  let { onchange = () => {}, onflash = () => {} } = $props();

  let plage = $state('1a');
  let perimetre = $state('reception');
  // null = intro ; sinon { plage, perimetre, total, traites }.
  let session = $state(null);
  let groupes = $state([]);
  // R9 (terrain 2026-08-31) : le tri de la section — récence par
  // défaut (l'ordre servi), le bouton cycle ; présentation seule.
  let tri = $state('date-desc');
  const groupesTries = $derived(
    [...groupes].sort(comparateurTri(tri, (g) => g.dernierEpoch, (g) => g.qui ?? g.address)),
  );
  let defauts = $state({ oui: 'reception', non: 'corbeille' });
  // Le groupe déplié (address) et ses messages — VOIR, rien d'autre.
  let ouvert = $state(null);
  let messagesOuverts = $state([]);
  // Le mini ⋯ ouvert : { address, qui, type: 'oui'|'non', x, y }.
  let menu = $state(null);
  // Un seul verdict en vol (revue 2026-08-30) : double-clic sur Oui =
  // deux verdicts pour un groupe, et `traites` dépasse le total.
  let occupe = $state(false);

  $effect(() => {
    (async () => {
      // D9 : les défauts du Portier, lus UNE fois — le premier rang ne
      // se peint qu'avec les défauts connus (patron Portier).
      try {
        defauts = await appel('portier_defauts_get');
      } catch (err) {
        console.error('portier_defauts_get :', err);
      }
      // D8 : une session entamée reprend — l'intro ne se montre pas.
      try {
        session = await appel('nettoyage_etat');
        if (session) await chargerGroupes();
      } catch (err) {
        console.error('nettoyage_etat :', err);
      }
    })();
  });

  async function chargerGroupes() {
    groupes = await appel('nettoyage_groupes');
  }

  async function demarrer() {
    try {
      session = await appel('nettoyage_demarrer', { plage, perimetre });
      ouvert = null;
      await chargerGroupes();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }

  async function terminer() {
    try {
      await appel('nettoyage_terminer');
      session = null;
      groupes = [];
      ouvert = null;
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }

  const BOITE_DE = {
    reception: 'portier.laReception',
    kiosque: 'portier.leKiosque',
    registre: 'portier.leRegistre',
  };
  const TOAST_NON = {
    spam: 'toast.portierNonSpam',
    archive: 'toast.portierNonArchive',
    corbeille: 'toast.portierNonCorbeille',
  };

  // Le verdict de GROUPE — même vocabulaire que le Portier, la porte
  // `nettoyage_verdict` applique aussi la règle au stock de la plage.
  async function decider(address, qui, destination, regle = null) {
    if (occupe) return;
    occupe = true;
    menu = null;
    try {
      session = await appel('nettoyage_verdict', { address, destination, regle });
      if (destination === 'ecarte') {
        onflash(t(regle ? TOAST_NON[regle] : 'toast.portierNonNu', { qui }));
      } else if (destination === 'reception') {
        onflash(t('toast.portierOuiNu', { qui }));
      } else {
        onflash(t('toast.portierOuiVers', { qui, boite: t(BOITE_DE[destination]) }));
      }
      if (ouvert === address) ouvert = null;
      // Le groupe décidé quitte la liste SUR PLACE (revue 2026-08-30 :
      // re-agréger toute la base à chaque clic payait la requête des
      // groupes par verdict) ; la base fait foi au prochain passage.
      groupes = groupes.filter((g) => g.address !== address);
      onchange();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    } finally {
      occupe = false;
    }
  }

  async function basculerGroupe(address) {
    if (ouvert === address) {
      ouvert = null;
      return;
    }
    try {
      messagesOuverts = await appel('nettoyage_messages', { address });
      ouvert = address;
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }

  function ouvrirMini(e, groupe, type) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      address: groupe.address,
      qui: groupe.qui ?? groupe.address,
      type,
      x: r.left,
      y: r.bottom + 4,
    };
  }

  // La jauge suit ce qui RESTE, pas le compteur de verdicts (revue
  // 2026-08-30) : un verdict posé ailleurs (Portier, « Écarter cet
  // expéditeur » d'une liste) fait disparaître un groupe sans passer
  // ici — compté sur `traites`, l'écran « fini » aurait montré une
  // barre coincée sous 100 %.
  const pourcent = $derived(
    session && session.total > 0
      ? Math.min(
          100,
          Math.max(0, Math.round(((session.total - groupes.length) * 100) / session.total)),
        )
      : 100,
  );
</script>


<div class="scene" data-testid="nettoyage">
  <div class="colonne">
    {#if !session}
      <h2 class="display entete-vue" data-testid="nettoyage-titre">
        <span class="glyphe-titre" aria-hidden="true"><Icone nom="nettoyage" taille={26} /></span>{t('boite.nettoyage')}</h2>
      <p class="sous-titre-vue">{t('nettoyage.sousTitre')}</p>

      <p class="regle-libelle">{t('nettoyage.plage')}</p>
      <div class="choix-plage" role="radiogroup" aria-label={t('nettoyage.plage')}>
        {#each PLAGES as p (p)}
          <button type="button" class="pastille-plage" class:choisie={plage === p}
                  role="radio" aria-checked={plage === p}
                  data-testid="nettoyage-plage" data-plage={p}
                  onclick={() => (plage = p)}>{t(`horizon.${p}`)}</button>
        {/each}
      </div>

      <p class="regle-libelle">{t('nettoyage.perimetre')}</p>
      <div class="choix-plage" role="radiogroup" aria-label={t('nettoyage.perimetre')}>
        {#each PERIMETRES as pe (pe)}
          <button type="button" class="pastille-plage" class:choisie={perimetre === pe}
                  role="radio" aria-checked={perimetre === pe}
                  data-testid="nettoyage-perimetre" data-perimetre={pe}
                  onclick={() => (perimetre = pe)}>{t(`nettoyage.perimetre.${pe}`)}</button>
        {/each}
      </div>

      <button type="button" class="demarrer" data-testid="nettoyage-demarrer"
              onclick={demarrer}>{t('nettoyage.demarrer')}</button>
    {:else}
      <!-- La barre de progression EN HAUT (énoncé CE) : % de groupes
           traités — le dessin de la jauge de migration. -->
      <div class="progression" data-testid="nettoyage-progression"
           role="progressbar" aria-valuemin="0" aria-valuemax="100"
           aria-valuenow={pourcent} aria-label={t('nettoyage.progressionAria')}>
        <div class="jauge"><div class="remplie" style="width:{pourcent}%"></div></div>
        <span class="pct">{t('nettoyage.progression', { p: pourcent })}</span>
      </div>

      <h2 class="display entete-vue">
        <span class="glyphe-titre" aria-hidden="true"><Icone nom="nettoyage" taille={26} /></span>{t('boite.nettoyage')}</h2>

      <div class="ligne-section">
        <p class="regle-libelle">{t('portier.question')}</p>
        {#if groupes.length}<TriSection valeur={tri} onchanger={(v) => (tri = v)} />{/if}
      </div>
      {#if groupes.length}
        {#each groupesTries as g (g.address)}
          <div class="rang-groupe" data-testid="nettoyage-groupe" data-adresse={g.address}>
            <!-- Le corps du rang est la PORTE du groupe : on entre pour
                 voir — le verdict, lui, reste aux boutons. -->
            <button type="button" class="msg" data-testid="nettoyage-ouvrir"
                    aria-expanded={ouvert === g.address}
                    onclick={() => basculerGroupe(g.address)}>
              <span class="l1">
                <span class="exp">{g.qui ?? g.address}</span>
                <span class="adr">&lt;{g.address}&gt;</span>
                <span class="essor"></span>
                <span class="heure">{quand(g.dernierEpoch)}</span>
              </span>
              <span class="l2">
                <span class="nombre">{t(g.messages > 1 ? 'nettoyage.messages' : 'nettoyage.message', { n: g.messages })}</span>
                {#if g.dernierObjet}<span class="objet">{g.dernierObjet}</span>{/if}
              </span>
            </button>
            <div class="choix">
              <span class="btn-portier">
                <button type="button" class="gros" data-testid="nettoyage-oui"
                        onclick={() => decider(g.address, g.qui ?? g.address, defauts.oui)}>
                  <span class="ic-oui"><Icone nom="check_circle" /></span>{t('portier.oui')}</button>
                <button type="button" class="mini" data-testid="nettoyage-mini-oui"
                        aria-label={t('portier.ouiChoix')} aria-haspopup="menu"
                        aria-expanded={menu?.address === g.address && menu?.type === 'oui'}
                        onclick={(e) => ouvrirMini(e, g, 'oui')}>
                  <Icone nom="more_horiz" taille={12} /></button>
              </span>
              <span class="btn-portier">
                <button type="button" class="gros" data-testid="nettoyage-non"
                        onclick={() => decider(g.address, g.qui ?? g.address, 'ecarte',
                          defauts.non === 'ecarte' ? null : defauts.non)}>
                  <span class="ic-non"><Icone nom="cancel" /></span>{t('portier.non')}</button>
                <button type="button" class="mini" data-testid="nettoyage-mini-non"
                        aria-label={t('portier.nonChoix')} aria-haspopup="menu"
                        aria-expanded={menu?.address === g.address && menu?.type === 'non'}
                        onclick={(e) => ouvrirMini(e, g, 'non')}>
                  <Icone nom="more_horiz" taille={12} /></button>
              </span>
            </div>
          </div>
          {#if ouvert === g.address}
            <div class="dedans" data-testid="nettoyage-messages">
              <!-- La clé porte le COMPTE : les UID repartent de 1 par
                   boîte — « INBOX/42 » existerait deux fois dès que la
                   même lettre touche deux comptes (patron cleMsg). -->
              {#each messagesOuverts as m (m.account_id + '/' + m.mailbox + '/' + m.uid)}
                <div class="rang-message">
                  <span class="objet-m">{m.subject}</span>
                  <span class="essor"></span>
                  <span class="heure">{quand(m.epoch)}</span>
                </div>
              {/each}
            </div>
          {/if}
        {/each}
      {:else}
        <!-- Plus un groupe : le nettoyage est fait — la coche du
             Portier, et la sortie. -->
        <div class="vide" data-testid="nettoyage-vide">
          <span class="ic-oui"><Icone nom="check_circle" /></span>{t('nettoyage.fini')}
        </div>
      {/if}

      <button type="button" class="terminer" data-testid="nettoyage-terminer"
              onclick={terminer}>{t('nettoyage.terminer')}</button>
    {/if}
  </div>
</div>

<Menu ouvert={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="nettoyage-menu" onfermer={() => (menu = null)}>
    {#if menu.type === 'oui'}
      <p class="titre-menu">{t('portier.ouiVers')}</p>
      <button type="button" role="menuitem" data-testid="nettoyage-vers-reception"
              onclick={() => decider(menu.address, menu.qui, 'reception')}>
        <Icone nom="inbox" />{t('portier.versReception')}</button>
      <button type="button" role="menuitem" data-testid="nettoyage-vers-kiosque"
              onclick={() => decider(menu.address, menu.qui, 'kiosque')}>
        <Icone nom="kiosque" />{t('portier.versKiosque')}</button>
      <button type="button" role="menuitem" data-testid="nettoyage-vers-registre"
              onclick={() => decider(menu.address, menu.qui, 'registre')}>
        <Icone nom="registre" />{t('portier.versRegistre')}</button>
    {:else}
      <p class="titre-menu">{t('portier.nonSeront')}</p>
      <button type="button" role="menuitem" data-testid="nettoyage-regle-spam"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'spam')}>
        <Icone nom="report" />{t('portier.regleSpam')}</button>
      <button type="button" role="menuitem" data-testid="nettoyage-regle-archive"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'archive')}>
        <Icone nom="inventory_2" />{t('portier.regleArchive')}</button>
      <button type="button" role="menuitem" data-testid="nettoyage-regle-corbeille"
              onclick={() => decider(menu.address, menu.qui, 'ecarte', 'corbeille')}>
        <Icone nom="delete" />{t('portier.regleCorbeille')}</button>
    {/if}
  </Menu>

<style>
  /* R9 : la ligne de section porte le tri à droite. */
  .ligne-section { display:flex; align-items:center; gap:10px; }
  .ligne-section .regle-libelle { flex:1; min-width:0; }
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .colonne { max-width:820px; margin:0 auto; }
  /* --- Intro --------------------------------------------------------- */
  .choix-plage { display:flex; flex-wrap:wrap; gap:8px; padding:12px 0 4px; }
  .pastille-plage {
    height:32px; padding:0 14px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .pastille-plage:hover { background:var(--sel); }
  .pastille-plage.choisie {
    border-color:var(--accent); color:var(--accent); font-weight:600;
    background:var(--sel);
  }
  .demarrer {
    margin-top:26px; height:40px; padding:0 22px; font-size:14px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-controle);
    cursor:pointer;
  }
  .demarrer:hover { background:var(--accentH); border-color:var(--accentH); }
  /* --- Tri ----------------------------------------------------------- */
  /* La jauge : le dessin de la modale de migration (6 px, remplie à
     l'accent), le % dans le TEXTE (A52). */
  .progression { display:flex; align-items:center; gap:12px; padding:2px 0 18px; }
  .jauge {
    flex:1; height:6px; background:var(--sel);
    border-radius:999px; overflow:hidden;
  }
  .remplie { height:100%; background:var(--accent); transition:width .25s ease; }
  .pct { flex:none; font-size:12.5px; color:var(--ink2); }
  .rang-groupe {
    display:flex; align-items:center; gap:18px; padding:16px 0;
    border-top:1px solid var(--border);
  }
  .msg {
    flex:1; min-width:0; display:flex; flex-direction:column; gap:3px;
    padding:4px 6px; margin:0 -6px; text-align:left;
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-controle); cursor:pointer;
  }
  .msg:hover { background:var(--sel); border-color:var(--border); }
  .l1, .l2 { display:flex; align-items:baseline; gap:8px; min-width:0; }
  .exp { font-size:14px; font-weight:600; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .adr { font-size:13px; color:var(--muted); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .essor { flex:1; }
  .heure { font-size:12px; color:var(--muted); flex:none; }
  .nombre { font-size:12.5px; color:var(--accent); font-weight:600; flex:none; }
  .objet { font-size:13px; color:var(--ink2); min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
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
  .dedans {
    margin:0 0 8px; padding:4px 12px 10px 24px;
    border-left:2px solid var(--border);
  }
  .rang-message { display:flex; align-items:baseline; gap:8px; padding:5px 0; min-width:0; }
  .objet-m { font-size:13px; color:var(--ink2); min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .vide {
    display:flex; align-items:center; gap:8px;
    padding:12px 0; font-size:13px; color:var(--ink2);
    border-top:1px solid var(--border);
  }
  .terminer {
    margin-top:22px; height:32px; padding:0 16px; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
    cursor:pointer;
  }
  .terminer:hover { background:var(--sel); }
  /* Le menu du mini ⋯ — le dessin des menus du produit (Portier). */
  .titre-menu {
    margin:4px 8px 6px; font-size:11px; letter-spacing:.06em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
</style>
