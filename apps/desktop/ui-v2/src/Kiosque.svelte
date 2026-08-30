<script>
  // Le Kiosque en CARTES (PLAN-MODE-ORGANISE E5bis, décision CE du
  // 2026-08-30 : « avant la release ») — la forme du prototype : les
  // lettres d'information arrivent DÉJÀ OUVERTES, la plus récente en
  // tête, colonne centrée ~720 px, une carte = expéditeur + heure,
  // objet en display, le CORPS entier (le même document auto-CSP que
  // l'écran de lecture, iframe sandbox S1, liens vers le navigateur
  // système), et le ⋯ de gestes. Rien n'est « à lire » : le Kiosque se
  // parcourt, il ne se traite pas — aucun marquage lu, aucun clic
  // d'ouverture. Les corps viennent du CACHE par page servie (D5/S3 —
  // jamais un réseau par carte) ; pas de fenêtrage : les cartes
  // s'ajoutent page à page au défilement (limite dite au PLAN — un
  // Kiosque se compte en dizaines, pas en milliers).
  import Icone from './Icone.svelte';
  import { appel } from './lib/transport.js';
  import { corpsAuto } from './lib/corps.js';
  import { brancherLiens } from './lib/liens.js';
  import { quand } from './lib/quand.js';
  import { t } from './lib/texte.svelte.js';

  let {
    compte = null,
    ondeplacer = () => {},
    oncote = () => {},
    ontotal = () => {},
  } = $props();

  const PAGE = 20;
  let cartes = $state([]);
  let epuise = $state(false);
  let enVol = $state(false);
  // Le vide ne s'affirme jamais sans preuve (leçon E2 de la Liste) :
  // `servi` ne passe à vrai qu'à une réponse REÇUE — un échec d'IPC ne
  // peint pas « Rien au Kiosque », et l'entrée ne le flashe pas.
  let servi = $state(false);
  let generation = 0;

  const cleCarte = (r) => `${r.account_id}:${r.mailbox}:${r.uid}`;

  async function charger(depuis) {
    const nee = ++generation;
    enVol = true;
    try {
      const page = await appel('kiosque_cartes', {
        accountId: compte,
        offset: depuis,
        limit: PAGE,
      });
      if (nee !== generation) return;
      if (depuis === 0) {
        cartes = page;
      } else {
        // Dédoublonnage à l'append (revue E5bis) : une arrivée entre
        // deux pages décale les offsets — la même carte re-servie
        // ferait une collision de clés (le crash du keyed each).
        const vues = new Set(cartes.map((c) => cleCarte(c.row)));
        cartes = [...cartes, ...page.filter((c) => !vues.has(cleCarte(c.row)))];
      }
      epuise = page.length < PAGE;
      servi = true;
      // Le total suit chaque recharge (revue E5bis : un ⋯ qui draine
      // des cartes laissait la barre de statut au compte d'avant).
      if (depuis === 0) {
        appel('category_total', { category: 'kiosque', accountId: compte, nonLus: false })
          .then((n) => {
            if (nee === generation) ontotal(n);
          })
          .catch(() => {});
      }
    } catch (err) {
      console.error('kiosque_cartes :', err);
    } finally {
      if (nee === generation) enVol = false;
    }
  }

  export function recharger() {
    charger(0);
  }

  // Nouvelle portée (compte) → repartir du haut.
  $effect(() => {
    void compte;
    charger(0);
  });

  // La page suivante quand le bas approche — un seul vol à la fois.
  function surDefilement(e) {
    if (epuise || enVol) return;
    const el = e.currentTarget;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 600) {
      charger(cartes.length);
    }
  }

  // R1 : accorder les images — LES commandes du produit (message ou
  // expéditeur), puis la page se ressert : la carte revient avec ses
  // images, la garde disparaît.
  async function accorderImages(carte, toujours) {
    try {
      await appel(toujours ? 'allow_images_sender' : 'allow_images_message', {
        accountId: carte.row.account_id,
        mailbox: carte.row.mailbox,
        uid: carte.row.uid,
      });
      charger(0);
    } catch (err) {
      console.error('images kiosque :', err);
    }
  }

  // Le pli d'une carte (constat CE au STOP visuel E5bis) : chaque
  // carte se replie/déplie à droite, comme les messages du volet de
  // lecture — DÉPLIÉE par défaut (les lettres arrivent ouvertes).
  let replies = $state({});
  function basculerPli(carte) {
    const k = cleCarte(carte.row);
    replies[k] = !replies[k];
  }

  // Le menu de gestes d'une carte (le patron du ⋯ des rangées).
  let menu = $state(null);
  function ouvrirMenu(e, carte) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      row: carte.row,
      cle: cleCarte(carte.row),
      x: Math.min(r.left, window.innerWidth - 260),
      y: Math.min(r.bottom + 4, window.innerHeight - 210),
    };
  }
  function geste(fn, ...args) {
    const { row } = menu;
    menu = null;
    fn(row, ...args);
  }
</script>

<svelte:window
  onclick={() => (menu = null)}
  onkeydown={(e) => {
    if (e.key === 'Escape') menu = null;
  }} />

<div class="scene" data-testid="kiosque" onscroll={surDefilement}>
  <div class="colonne">
    <p class="note"><Icone nom="info" />{t('kiosque.note')}</p>
    {#each cartes as carte (cleCarte(carte.row))}
      <article class="carte" data-testid="kiosque-carte">
        <div class="de">
          <span class="nom">{carte.row.sender}</span>
          <button type="button" class="gestes" data-testid="kiosque-gestes"
                  aria-label={t('liste.gestes')} aria-haspopup="menu"
                  aria-expanded={menu?.cle === cleCarte(carte.row)}
                  onclick={(e) => ouvrirMenu(e, carte)}>
            <Icone nom="more_horiz" taille={14} /></button>
          <span class="heure">{quand(carte.row.epoch)}</span>
        </div>
        <!-- Le pli (constat CE, 3 passes) : le bouton exact du volet
             de lecture — glyphe + texte, bouton nu —, SUR LA LIGNE DE
             L'OBJET, aligné à droite. -->
        <div class="rang-objet">
          <h3 class="display">{carte.row.subject}</h3>
          <button type="button" class="nu" data-testid="kiosque-pli"
                  aria-expanded={!replies[cleCarte(carte.row)]}
                  onclick={() => basculerPli(carte)}>
            <Icone nom={replies[cleCarte(carte.row)] ? 'unfold_more' : 'unfold_less'} />
            {replies[cleCarte(carte.row)] ? t('action.deplier') : t('action.replier')}</button>
        </div>
        {#if replies[cleCarte(carte.row)]}
          <p class="apercu">{carte.row.preview ?? ''}</p>
        {:else if carte.document !== null}
          {#if carte.remote_images_blocked > 0}
            <!-- R1 : la garde d'images, comme au volet de lecture —
                 sans elle, une lettre toute en images distantes serait
                 une dalle vide sans recours (revue E5bis). -->
            <div class="garde-images" data-testid="kiosque-garde-images">
              <span>{t('lecture.imagesBloquees', { n: carte.remote_images_blocked })}</span>
              <button type="button" onclick={() => accorderImages(carte, false)}>
                {t('lecture.afficherImages')}</button>
              <button type="button" onclick={() => accorderImages(carte, true)}>
                {t('lecture.toujoursAfficherImages')}</button>
            </div>
          {/if}
          <iframe class="corps" sandbox="allow-same-origin" srcdoc={carte.document}
                  title={carte.row.subject} use:corpsAuto
                  onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
        {:else}
          <!-- Corps pas encore en cache : l'aperçu dit l'essentiel, le
               rattrapage normal remplira la carte. -->
          <p class="apercu">{carte.row.preview ?? ''}</p>
        {/if}
      </article>
    {/each}
    {#if servi && cartes.length === 0 && !enVol}
      <p class="vide" data-testid="kiosque-vide">{t('kiosque.vide')}</p>
    {/if}
  </div>
</div>

{#if menu}
  <div class="menu-carte" role="menu" data-testid="kiosque-menu"
       style="left:{menu.x}px; top:{menu.y}px">
    {#each ['reception', 'registre'] as dest (dest)}
      <button type="button" role="menuitem" data-testid={`kiosque-vers-${dest}`}
              onclick={() => geste(ondeplacer, dest)}>
        <Icone nom={dest === 'reception' ? 'inbox' : 'registre'} />{t('liste.deplacerVers', { boite: t(`boite.${dest}`) })}</button>
    {/each}
    <div class="filet"></div>
    <button type="button" role="menuitem" data-testid="kiosque-cote"
            onclick={() => geste(oncote)}>
      <Icone nom="pile" />{t('pile.mettre')}</button>
    <div class="filet"></div>
    <button type="button" role="menuitem" data-testid="kiosque-ecarter"
            onclick={() => geste(ondeplacer, 'ecarte')}>
      <Icone nom="visibility_off" />{t('liste.ecarter')}</button>
  </div>
{/if}

<style>
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .colonne { max-width:720px; margin:0 auto; }
  .note {
    display:flex; align-items:baseline; gap:8px; margin:0 0 22px;
    font-size:13px; line-height:1.5; color:var(--ink2); max-width:70ch;
  }
  .note :global(.ic) { color:var(--muted); align-self:center; flex:none; }
  .carte { padding:26px 0 10px; border-top:1px solid var(--border); }
  .carte:first-of-type { border-top:none; padding-top:4px; }
  .de { display:flex; align-items:baseline; gap:8px; margin-bottom:10px; }
  .de .nom { font-size:13px; font-weight:600; color:var(--ink2); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .de .heure { font-size:12px; color:var(--muted); flex:none; }
  /* Le ⋯ : place réservée, opacité seule (la géométrie ne bouge pas). */
  .gestes {
    flex:none; width:24px; height:24px; padding:0; align-self:center;
    display:inline-flex; align-items:center; justify-content:center;
    opacity:0; color:var(--muted); background:none;
    border:1px solid transparent;
  }
  .carte:hover .gestes, .gestes:focus-visible, .gestes[aria-expanded="true"] { opacity:1; }
  .gestes:hover, .gestes[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  /* Le pli : le bouton NU du volet de lecture (glyphe + texte), sur
     la LIGNE DE L'OBJET, à droite (constat CE, 3 passes). */
  .rang-objet {
    display:flex; align-items:center; gap:12px; margin:0 0 12px;
  }
  .rang-objet h3 { margin:0; flex:1; min-width:0; }
  .rang-objet .nu { flex:none; }
  .nu {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:var(--r-controle); cursor:pointer;
    white-space:nowrap;
  }
  .nu:hover { background:var(--sel); }
  h3 { margin:0; font-size:24px; line-height:1.25; color:var(--ink); }
  .corps { width:100%; border:none; display:block; background:#fff; }
  .apercu { margin:0 0 8px; font-size:13px; line-height:1.5; color:var(--ink2); }
  .garde-images {
    display:flex; align-items:center; gap:10px; flex-wrap:wrap;
    padding:8px 12px; margin:0 0 8px; font-size:12px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
  }
  .garde-images button { height:26px; padding:0 10px; font-size:12px; }
  .vide { margin:8px 0 0; font-size:13px; line-height:1.5; color:var(--ink2); max-width:66ch; }
  .menu-carte {
    position:fixed; z-index:30; min-width:240px; padding:6px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:0 8px 24px rgba(0,0,0,.14);
    display:flex; flex-direction:column; gap:2px;
  }
  .menu-carte button {
    display:flex; align-items:center; gap:8px; text-align:left;
    border:1px solid transparent; background:none; height:32px; padding:0 8px;
  }
  .menu-carte button:hover { background:var(--hover); }
  .filet { border-top:1px solid var(--border); margin:4px 0; }
</style>
