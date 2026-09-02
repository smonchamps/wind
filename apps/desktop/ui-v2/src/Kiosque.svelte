<script>
  // Le Kiosque en CARTES (PLAN-MODE-ORGANISE E5bis, puis RETOURS-13
  // R10/R11) — les lettres d'information arrivent DÉJÀ OUVERTES,
  // colonne centrée ~720 px, une carte = expéditeur + heure, objet en
  // display, le CORPS entier (document auto-CSP, iframe sandbox S1),
  // le ⋯ de gestes. R10 renverse le « rien n'est marqué lu » d'A100 :
  // une carte dont le BAS de l'élévation a été affiché est LUE (témoin
  // IntersectionObserver → `kiosque_marquer_lu`, patron pins) — la
  // scène se coupe en « Non lus » (dépliées, chronologique) et « Lus
  // précédemment » (groupes par expéditeur à l'alphabet, repliés en
  // pile — D5). Le sectionnement se calcule AU SERVICE de la page :
  // une carte ne saute jamais pendant la lecture. Les corps viennent
  // du CACHE par page servie (D5/S3) ; pas de fenêtrage : les cartes
  // s'ajoutent page à page au défilement (limite dite au PLAN).
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import TriSection from './TriSection.svelte';
  import { comparateurTri } from './lib/tri.js';
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
        // Fusion par cle (PLAN-AUDIT-V2 E10) : une resservie REMPLACAIT
        // tout — la carte lue sautait de section pendant la lecture et
        // les pages 2..n disparaissaient. `lu` reste FIGE au premier
        // service (les sections se calculent de l'etat servi, R10) ; les
        // cartes deja servies au-dela de la page restent derriere.
        const anciennes = new Map(cartes.map((c) => [cleCarte(c.row), c]));
        const fraiches = page.map((c) => {
          const ancienne = anciennes.get(cleCarte(c.row));
          return ancienne ? { ...c, lu: ancienne.lu } : c;
        });
        const vues = new Set(fraiches.map((c) => cleCarte(c.row)));
        cartes = [...fraiches, ...cartes.slice(PAGE).filter((c) => !vues.has(cleCarte(c.row)))];
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

  // Fenetrage (PLAN-AUDIT-V2 E10) : une iframe vivante + un
  // ResizeObserver PAR carte — dix pages = deux cents documents. Seules
  // les cartes a moins de FENETRE rangs de la premiere visible portent
  // leur iframe ; une carte qui sort de la fenetre laisse un bloc de sa
  // hauteur mesuree (le defilement ne saute pas), et la retrouve en
  // revenant.
  const FENETRE = 12;
  let indexVisible = $state(0);
  let hauteurs = $state({});
  const horsFenetre = (i) => Math.abs(i - indexVisible) > FENETRE;
  let mesureDemandee = false;
  function mesurerFenetre(scene) {
    if (mesureDemandee) return;
    mesureDemandee = true;
    requestAnimationFrame(() => {
      mesureDemandee = false;
      const haut = scene.getBoundingClientRect().top;
      const articles = scene.querySelectorAll('article.carte');
      let premiere = 0;
      for (let i = 0; i < articles.length; i += 1) {
        if (articles[i].getBoundingClientRect().bottom > haut) { premiere = i; break; }
      }
      // La hauteur des corps qui vont sortir de la fenetre, relevee AVANT
      // qu'ils ne se demontent.
      const neuves = { ...hauteurs };
      articles.forEach((article, i) => {
        if (Math.abs(i - premiere) > FENETRE) {
          const corps = article.querySelector('iframe.corps');
          if (corps) neuves[article.dataset.cle] = corps.offsetHeight;
        }
      });
      hauteurs = neuves;
      indexVisible = premiere;
    });
  }

  // La page suivante quand le bas approche — un seul vol à la fois.
  function surDefilement(e) {
    const el = e.currentTarget;
    mesurerFenetre(el);
    if (epuise || enVol) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 600) {
      charger(cartes.length);
    }
  }

  // R1 : accorder les images — LES commandes du produit (message ou
  // expéditeur), puis LA carte se ressert : elle revient avec ses
  // images, la garde disparaît. Terrain STOP 2 PLAN-AUDIT-V2
  // (2026-09-02) : `charger(0)` ne re-servait que la page 0 — la fusion
  // par clé (E10) gardait telle quelle une carte au-delà, et sa garde
  // restait après dix pages défilées. La carte se re-sert où qu'elle
  // soit, par `message_body` (le même document que le volet) ; pour la
  // règle d'expéditeur, toutes les cartes servies encore gardées — le
  // cœur départage, une carte d'un tiers re-rend à l'identique.
  async function accorderImages(carte, toujours) {
    try {
      await appel(toujours ? 'allow_images_sender' : 'allow_images_message', {
        accountId: carte.row.account_id,
        mailbox: carte.row.mailbox,
        uid: carte.row.uid,
      });
      const cibles = toujours ? cartes.filter((c) => c.remote_images_blocked > 0) : [carte];
      await Promise.all(cibles.map(resservir));
    } catch (err) {
      console.error('images kiosque :', err);
    }
  }
  async function resservir(carte) {
    const vue = await appel('message_body', {
      accountId: carte.row.account_id,
      mailbox: carte.row.mailbox,
      uid: carte.row.uid,
      showImages: false,
    });
    const cle = cleCarte(carte.row);
    cartes = cartes.map((c) =>
      cleCarte(c.row) === cle
        ? { ...c, document: vue.document, remote_images_blocked: vue.remote_images_blocked }
        : c,
    );
  }

  // Le pli d'une carte (constat CE au STOP visuel E5bis) : chaque
  // carte se replie/déplie à droite, comme les messages du volet de
  // lecture. R10 : une carte NON LUE arrive dépliée, une carte LUE
  // (dans son groupe) arrive repliée sur la ligne de l'objet.
  let replies = $state({});
  const estRepliee = (carte) => replies[cleCarte(carte.row)] ?? carte.lu;
  function basculerPli(carte) {
    replies[cleCarte(carte.row)] = !estRepliee(carte);
  }

  // R10 — les deux sections, calculées de l'état SERVI (carte.lu) :
  // les marques posées en vol n'y touchent pas.
  // R9 (terrain 2026-08-31) : chaque section porte SON tri — les
  // défauts restent l'ordre d'avant (non-lus par récence servie,
  // groupes à l'alphabet A → Z) ; le bouton cycle, présentation seule
  // (comparateurTri, la collation suit la langue de l'UI).
  let triNonLus = $state('date-desc');
  let triLus = $state('alpha-az');
  const nonLues = $derived(
    cartes
      .filter((c) => !c.lu)
      .sort(comparateurTri(triNonLus, (c) => c.row.epoch, (c) => c.row.sender ?? '')),
  );
  const groupes = $derived.by(() => {
    const parQui = new Map();
    for (const c of cartes) {
      if (!c.lu) continue;
      const qui = c.row.sender ?? '';
      if (!parQui.has(qui)) parQui.set(qui, []);
      parQui.get(qui).push(c);
    }
    return [...parQui.entries()]
      .map(([qui, siennes]) => ({ qui, cartes: siennes }))
      .sort(comparateurTri(
        triLus,
        (g) => Math.max(...g.cartes.map((c) => c.row.epoch)),
        (g) => g.qui,
      ));
  });
  // Le rang DOM de chaque carte, sections et groupes confondus : c'est
  // lui que la fenetre compare a la premiere carte visible (E10).
  const rangs = $derived(
    new Map([...nonLues, ...groupes.flatMap((g) => g.cartes)].map((c, i) => [cleCarte(c.row), i])),
  );
  let groupesOuverts = $state({});

  // R10 — le témoin de lecture : un nœud au PIED de chaque carte non
  // lue ; quand il entre dans la scène, le bas de l'élévation a été
  // affiché — la carte se marque (idempotent, une écriture par carte).
  let scene = $state(null);
  const temoins = new Map();
  let observateur = null;
  $effect(() => {
    if (!scene) return;
    observateur = new IntersectionObserver((entrees) => {
      for (const e of entrees) {
        if (!e.isIntersecting) continue;
        const carte = temoins.get(e.target);
        if (!carte) continue;
        observateur?.unobserve(e.target);
        marquerLue(carte, e.target);
      }
    }, { root: scene });
    // Les témoins montés avant l'effet (le premier rendu) s'observent
    // ici — l'action court avant l'observateur.
    for (const nœud of temoins.keys()) observateur.observe(nœud);
    return () => {
      observateur?.disconnect();
      observateur = null;
    };
  });
  function temoinLu(nœud, carte) {
    temoins.set(nœud, carte);
    observateur?.observe(nœud);
    return {
      destroy() {
        temoins.delete(nœud);
        observateur?.unobserve(nœud);
      },
    };
  }
  const marquees = new Set();
  async function marquerLue(carte, temoin) {
    const k = cleCarte(carte.row);
    if (marquees.has(k)) return;
    marquees.add(k);
    try {
      await appel('kiosque_marquer_lu', {
        accountId: carte.row.account_id,
        mailbox: carte.row.mailbox,
        uid: carte.row.uid,
      });
    } catch (err) {
      // L'écriture a manqué : le témoin se RÉARME (revue — sans le
      // re-observe, « au prochain passage » était un mensonge : un
      // nœud désobservé ne repasse jamais) et la marque se rejouera.
      marquees.delete(k);
      if (temoins.has(temoin)) observateur?.observe(temoin);
      console.error('kiosque_marquer_lu :', err);
    }
  }

  // Le menu de gestes d'une carte (le patron du ⋯ des rangées).
  let menu = $state(null);
  function ouvrirMenu(e, carte) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menu = {
      row: carte.row,
      cle: cleCarte(carte.row),
      x: r.left,
      y: r.bottom + 4,
    };
  }
  function geste(fn, ...args) {
    const { row } = menu;
    menu = null;
    fn(row, ...args);
  }
</script>


{#snippet blocCarte(carte)}
  <article class="carte" data-testid="kiosque-carte" data-cle={cleCarte(carte.row)}>
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
              aria-expanded={!estRepliee(carte)}
              onclick={() => basculerPli(carte)}>
        <Icone nom={estRepliee(carte) ? 'unfold_more' : 'unfold_less'} />
        {estRepliee(carte) ? t('action.deplier') : t('action.replier')}</button>
    </div>
    {#if estRepliee(carte)}
      <p class="apercu">{carte.row.preview ?? ''}</p>
    {:else if horsFenetre(rangs.get(cleCarte(carte.row)) ?? 0) && carte.document !== null}
      <!-- Hors fenetre : le bloc garde la hauteur du corps demonte. -->
      <div class="corps-dormant" style={`height:${hauteurs[cleCarte(carte.row)] ?? 0}px`}
           data-testid="kiosque-corps-dormant"></div>
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
    {#if !carte.lu && !estRepliee(carte)}
      <!-- R10 : le témoin de lecture — le PIED de l'élévation ; le
           voir passer, c'est avoir lu la carte jusqu'en bas. -->
      <div class="temoin-lu" use:temoinLu={carte} aria-hidden="true"></div>
    {/if}
  </article>
{/snippet}

<div class="scene" data-testid="kiosque" onscroll={surDefilement} bind:this={scene}>
  <div class="colonne">
    <!-- R11 (RETOURS-13) : l'entête au format du Portier — glyphe +
         titre + deux phrases CE, justifiés à gauche sur la colonne. -->
    <h2 class="display entete-vue" data-testid="kiosque-titre">
      <span class="glyphe-titre" aria-hidden="true"><Icone nom="kiosque" taille={26} /></span>{t('boite.kiosque')}</h2>
    <p class="sous-titre-vue">{t('kiosque.sousTitre1')}<br />{t('kiosque.sousTitre2')}</p>
    {#if cartes.length}
      <!-- Terrain RETOURS-13 (C5) : le titre de section reste visible
           quand tout est lu — la coche du Portier dit le travail fait. -->
      <div class="ligne-section">
        <p class="regle-libelle" data-testid="kiosque-section-nonlus">{t('kiosque.sectionNonLus')}</p>
        {#if nonLues.length}<TriSection valeur={triNonLus} onchanger={(v) => (triNonLus = v)} />{/if}
      </div>
      {#if nonLues.length}
        {#each nonLues as carte (cleCarte(carte.row))}
          {@render blocCarte(carte)}
        {/each}
      {:else}
        <div class="tout-lu" data-testid="kiosque-tout-lu">
          <span class="ic-oui" aria-hidden="true"><Icone nom="check_circle" /></span>{t('kiosque.toutLu')}
        </div>
      {/if}
    {/if}
    {#if groupes.length}
      <div class="ligne-section">
        <p class="regle-libelle" data-testid="kiosque-section-lus">{t('kiosque.sectionLus')}</p>
        <TriSection valeur={triLus} onchanger={(v) => (triLus = v)} />
      </div>
      {#each groupes as g (g.qui)}
        <!-- D5 : la rangée d'un groupe replié montre une PILE
             d'élévations (le visuel des mis de côté) ; le clic déplie
             ses cartes, repliées sur la ligne de l'objet. -->
        <button type="button" class="rang-groupe" data-testid="kiosque-groupe"
                aria-expanded={!!groupesOuverts[g.qui]}
                onclick={() => (groupesOuverts[g.qui] = !groupesOuverts[g.qui])}>
          <span class="empile" aria-hidden="true"><span></span><span></span><span></span></span>
          <span class="qui" data-testid="kiosque-groupe-nom">{g.qui}</span>
          <span class="nb">{g.cartes.length}</span>
        </button>
        {#if groupesOuverts[g.qui]}
          {#each g.cartes as carte (cleCarte(carte.row))}
            {@render blocCarte(carte)}
          {/each}
        {/if}
      {/each}
    {/if}
    {#if servi && cartes.length === 0 && !enVol}
      <p class="vide" data-testid="kiosque-vide">{t('kiosque.vide')}</p>
    {/if}
  </div>
</div>

<Menu ouvert={menu !== null} x={menu?.x ?? 0} y={menu?.y ?? 0}
      testid="kiosque-menu" onfermer={() => (menu = null)}>
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
  </Menu>

<style>
  .scene { flex:1; overflow:auto; padding:28px 36px 60px; min-width:0; }
  .colonne { max-width:720px; margin:0 auto; }
  /* R11 : l'entête et la règle-libellé sont les classes PARTAGÉES de
     systeme.css (.entete-vue / .sous-titre-vue / .regle-libelle —
     une copie, Portier et Kiosque). */
  .carte { padding:26px 0 10px; border-top:1px solid var(--border); }
  /* R9 : la ligne de section porte le tri à droite. */
  .ligne-section { display:flex; align-items:center; gap:10px; }
  .ligne-section .regle-libelle { flex:1; min-width:0; }
  /* R10 — la rangée d'un groupe replié : pile d'élévations + nom +
     nombre, le dessin d'une rangée (jamais un bouton plein). */
  .rang-groupe {
    width:100%; display:flex; align-items:center; gap:12px;
    padding:12px 10px; font-size:13px; color:var(--ink); text-align:left;
    background:none; border:none; border-top:1px solid var(--border);
    cursor:pointer;
  }
  .rang-groupe:hover { background:var(--hover); }
  .rang-groupe .qui {
    flex:1; min-width:0; font-weight:600;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .rang-groupe .nb {
    flex:none; font-size:12px; font-weight:600; color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  /* La pile (D5) : trois élévations décalées, le visuel de l'éventail
     des mis de côté en miniature. */
  .empile { position:relative; width:20px; height:16px; flex:none; }
  /* V14 : zéro rayon — les feuilles de la pile sont des rectangles
     nus, comme le visuel de la pile des mis de côté. */
  .empile span {
    position:absolute; inset:0; background:var(--surface);
    border:1px solid var(--border);
  }
  .empile span:nth-child(1) { transform:translate(4px, -4px); }
  .empile span:nth-child(2) { transform:translate(2px, -2px); }
  /* Le témoin de lecture : un nœud sans géométrie — il ne déplace
     rien, il n'existe que pour l'observateur. */
  .temoin-lu { height:1px; }
  /* C5 : « tout lu » — la coche du Portier (accent), le dessin de son
     vide (filet supérieur par la section, texte atténué). */
  .tout-lu {
    display:flex; align-items:center; gap:8px; padding:12px 0;
    font-size:13px; color:var(--ink2); border-top:1px solid var(--border);
  }
  .ic-oui :global(.ic) { color:var(--accent); }
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
  .filet { border-top:1px solid var(--border); margin:4px 0; }
</style>
