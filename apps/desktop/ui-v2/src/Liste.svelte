<script>
  // Liste fenêtrée de l'écran 02 — lignes continues séparées au filet,
  // le dessin des pistes (A29/A30), servie par `list_category` : la
  // source est (catégorie, compte, non-lus), les onglets vivent dans le
  // pied de cette colonne. Depuis A44 (PLAN-RETOURS-V3, renverse la
  // « ligne nue » d'A29/A2 ; terrain du 2026-08-16 : hauteur AU
  // CONTENU, pas de rang réservé) une ligne qui a quelque chose à dire
  // porte le rang de puces du prototype et s'en agrandit : DEUX
  // gabarits (h1 nue, h2 porteuse), la mécanique de fenêtrage d'avant
  // A29 — chipsParPage, extraPuce, correction itérative — reprend du
  // service, à l'identique de ce qu'elle était (848f286~1).
  //
  // Changement de source = nouvelle génération : les pages en vol de la
  // source précédente sont jetées à l'arrivée, jamais mélangées.
  import Icone from './Icone.svelte';
  import { tick, untrack } from 'svelte';
  import { SvelteMap } from 'svelte/reactivity';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { puceInvitation } from './lib/invitation.js';
  import { blocBoite, vueMelange } from './lib/boite.js';
  import { padRangee } from './lib/espacement.svelte.js';
  import { initiales } from './lib/initiales.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';
  import { cleLibelleBoite } from './lib/organise.svelte.js';

  let {
    categorie = 'reception',
    compte = null,
    // A80/D7 : le bloc de boîte ne se dit que si les comptes se
    // mélangent VRAIMENT — il faut donc savoir combien il y en a.
    comptes = [],
    // A80 : les repères de compte nourrissent le TRACÉ du bloc de
    // boîte, qui ne vit qu'en boîte unifiée et en recherche (D3/D7 :
    // là où identifier le compte a un sens) — sur toutes les rangées,
    // repère ou non (D8).
    reperes = {},
    // PLAN-RETOURS-9 (D4) : le nom personnalisé est le libellé du
    // bloc — l'adresse reste le repli d'un compte sans nom, et la
    // vérité de l'infobulle.
    noms = {},
    onglet = 'tous',
    recherche = '',
    // PLAN-BROUILLONS : les brouillons locaux (rows de `list_drafts`),
    // sondés par l'App — le dossier Brouillons les affiche, la
    // Réception mentionne les fils qui en portent un.
    brouillons = [],
    onreprendre = () => {},
    onselect = () => {},
    ononglet = () => {},
    ontotal = () => {},
    onresultats = () => {},
    onflash = () => {},
    // PLAN-RETOURS-10 R1 : les gestes de MASSE remontent à l'App, qui
    // possède les commandes (archiver, supprimer, spam, lu/non-lu) —
    // la Liste possède la sélection, jamais l'action.
    ongroupe = async () => {},
    // PLAN-MODE-ORGANISE E4 : en mode organisé, la Réception se
    // présente en colonne centrée à SECTIONS et chaque rangée porte le
    // ⋯ de gestes (Déplacer vers…, Écarter) — remontés à l'App, qui
    // possède les commandes (même règle que la sélection de masse).
    organise = false,
    ondeplacer = () => {},
    oncote = () => {},
  } = $props();

  const PAGE = 200;

  // E4 — les sections de la Réception organisée (verdict S1/A2) : le
  // service rend UN flot ordonné « non-lus d'abord », la couture est
  // le COUNT des non-lus. Les entêtes vivent HORS des rangées (la
  // géométrie du fenêtrage gagne un décrochement, patron des puces
  // d'invitation) — jamais une rangée à hauteur d'exception.
  const sections = $derived(
    organise && categorie === 'reception' && onglet === 'tous'
      && resultats === null && lignesBrouillons === null,
  );
  // La colonne centrée de la Réception organisée (~760 px, prototype).
  const centre = $derived(
    organise && categorie === 'reception'
      && resultats === null && lignesBrouillons === null,
  );
  // Le ⋯ de gestes par rangée — les vues organisées seulement.
  const gestesOrganise = $derived(
    organise && ['reception', 'kiosque', 'registre'].includes(categorie)
      && resultats === null && lignesBrouillons === null,
  );
  let couture = $state(0);
  // 52 px : l'air AU-DESSUS du libellé (constat CE au STOP visuel E4 —
  // le dernier mail d'une section et le titre de la suivante doivent
  // respirer) ; le libellé reste calé en bas de sa bande.
  const H_ENTETE = 52;
  const entetes = $derived.by(() => {
    if (!sections || total === 0) return [];
    const liste = [];
    if (couture > 0) liste.push({ index: 0, libelle: t('liste.sectionNouveau', { n: couture }) });
    if (couture < total) liste.push({ index: couture, libelle: t('liste.sectionConsulte') });
    return liste;
  });
  // Les entêtes POSITIONNÉES — dérivé à part : `decalage` lit des
  // Maps non réactives (pages/chips), seul `version` signale leurs
  // mouvements (le canal de `hauteurEspace`).
  const positionsEntetes = $derived.by(() => {
    void version;
    void h1;
    return entetes.map((e) => ({ ...e, top: decalage(e.index) - H_ENTETE }));
  });
  function entetesAvant(i) {
    let n = 0;
    for (const e of entetes) if (e.index <= i) n += 1;
    return n;
  }
  // RETOURS-14 R2 : le nom de la section courante reste VISIBLE au
  // défilement — une bande collée en tête du cadre, servie dès que la
  // bande réelle est partie au-dessus. `premier` est déjà la vérité
  // réactive du scroll (fenêtrage) : à `premier > 0`, la bande de la
  // section en cours n'est plus à l'écran.
  const sectionCollee = $derived.by(() => {
    if (!sections || premier <= 0) return null;
    let courante = null;
    for (const e of entetes) {
      if (e.index <= premier) courante = e;
      else break;
    }
    return courante;
  });
  // Le menu du ⋯ — le patron du Portier : ancré au clic, borné à la
  // fenêtre, refermé au clic dehors et à Échap.
  let menuGestes = $state(null);
  const cleLigne = (l) => `${l.account_id}:${l.mailbox}:${l.uid}`;
  function ouvrirGestes(e, ligne) {
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    menuGestes = {
      ligne,
      cle: cleLigne(ligne),
      x: Math.min(r.left, window.innerWidth - 260),
      y: Math.min(r.bottom + 4, window.innerHeight - 210),
    };
  }
  function geste(destination) {
    const { ligne } = menuGestes;
    menuGestes = null;
    ondeplacer(ligne, destination);
  }
  const OVER = 8;

  // R4 (PLAN-RETOURS-MAIL) : dans le dossier d'envois, l'expéditeur est
  // SOI — répéter son nom sur chaque ligne n'apprend rien. La colonne
  // dit le DESTINATAIRE (« À : X »), tiré de `to_addrs` stocké à la
  // synchro. À défaut (ancien envoi non encore rattrapé) on garde le nom
  // d'expéditeur d'avant — jamais de ligne muette.
  const versEnvoi = (ligne) => categorie === 'envoyes' && (ligne.to_addrs?.length ?? 0) > 0;
  const correspondant = (ligne) =>
    versEnvoi(ligne) ? ligne.to_addrs.join(', ') : ligne.sender;

  // A80 : le bloc de boîte vit là où les comptes se MÉLANGENT — boîte
  // unifiée (D3 d'A74) et recherche, D7. À la différence du badge, il
  // ne demande PAS de repère : le mot suffit, et le libellé retombe
  // sur l'adresse quand aucun nom n'est posé (D8).
  // La règle ENTIÈRE vit dans lib/boite.js — garde de vue comprise
  // depuis le verdict terrain du 2026-08-25 (point 12) : le volet de
  // lecture applique la même, et deux expressions divergeraient.
  const boiteDe = (ligne) =>
    !vueMelange(compte, resultats !== null)
      ? null
      : blocBoite({
        accountId: ligne.account_id,
        adresse: ligne.account_email,
        reperes,
        noms,
        comptes,
      });

  let cadre = $state(null);
  let total = $state(0);
  let premier = $state(0);
  let version = $state(0);
  // Amorces = la géométrie réelle du cran par défaut (mesurée : 88 nue,
  // 115 porteuse). Elles ne servent que le premier frame, avant que les
  // sondes ne se lient — les tenir justes évite un saut inutile.
  let h1 = $state(88);
  let h2 = $state(115);
  let selection = $state(null);
  let premierePageMs = $state(null);
  // PLAN-DEFILEMENT-PROFOND E2 + terrain du 2026-08-20 :
  // `sourceRepondue` — une page de la SOURCE courante est arrivée ;
  // avant cette preuve l'écran montre l'attente, jamais « Aucun
  // message ici. ». `totalPrecis` — le total affiché est exact : soit
  // une page plus courte que sa limite l'a dit d'elle-même (fin de
  // liste — les petits dossiers ne paient JAMAIS de comptage), soit
  // `category_total` a répondu. Entre les deux, `total` est un
  // PLANCHER tiré des lignes servies : les lignes s'affichent sans
  // attendre le comptage (~240 ms par intégrale, plus à froid), la
  // barre de défilement s'ajuste à l'arrivée du vrai total.
  let sourceRepondue = $state(false);
  let totalPrecis = $state(false);

  // Deux compteurs (revue 2026-08-20) : `source` ne bouge qu'au
  // changement de (catégorie, compte, onglet) — un vol né d'une AUTRE
  // source est jeté à l'arrivée ; `generation` bouge aussi à chaque
  // recharge — un vol de la MÊME source à génération antérieure reste
  // bon à AFFICHER (stale-while-revalidate), sa page reste dépareillée
  // donc resservie. Sans cette distinction, des recharges plus
  // rapprochées que le règlement d'une page profonde (rattrapage des
  // corps : une recharge PAR LOT, des jours durant) condamneraient
  // chaque résultat à l'arrivée — squelette à demeure.
  let source = 0;
  let generation = 0;
  let pages = new Map();
  let chipsParPage = new Map();
  let pending = new Map();
  // Stale-while-revalidate (PLAN-REACTIVITE E1) : la génération à
  // laquelle chaque page a été servie. Une recharge bump `generation`
  // SANS jeter `pages` — les lignes affichées restent le fond, et une
  // page ne se resert que si sa génération est dépareillée.
  let servieA = new Map();

  // Deux gabarits (A44, terrain : hauteur au contenu) : h1 la ligne
  // nue, h2 la porteuse — la géométrie corrige la multiplication par
  // le compte de porteuses AVANT l'index, tenu par page. `pinned` ne
  // vient que de la section épinglée (hors fenêtrage) : il rend la
  // ligne porteuse pour montrer sa marque, sans toucher les pages.
  // Terrain R3'c (2026-08-23) : les GESTES d'une invitation occupent
  // un rang À EUX — les autres puces (messages, fichiers, épingle)
  // descendent au rang du dessous et ne remontent que quand la puce de
  // réponse les rejoint. Le fenêtrage compte donc des RANGS (0, 1 ou
  // 2) : le coût marginal d'un rang est constant (extraPuce = h2 − h1,
  // la grille espace chaque rang du même row-gap) — la correction
  // d'A44 se généralise, toujours pas de gabarit mesuré en plus.
  const autresPuces = (l) => l.thread_size > 1 || l.attachment_count > 0 || l.pinned;
  const gestesInvitation = (l) =>
    l.invitation != null && !puceInvitation(l.invitation) && l.invitation.peut_repondre;
  const rangsPuces = (l) =>
    (gestesInvitation(l) ? 1 : 0) +
    (autresPuces(l) || (l.invitation != null && puceInvitation(l.invitation) != null) ? 1 : 0);
  const aPuces = (l) => rangsPuces(l) > 0;

  // R10 : répondre à une invitation SANS l'ouvrir — le même chemin que
  // la carte (repondre_invitation : journal + réponse en une
  // transaction), le sujet dans la langue du produit, la puce suit
  // localement. stopPropagation : le clic ne choisit pas la ligne.
  let reponsesInvitation = $state({});
  async function repondreInvitation(e, ligne, reponse) {
    e.stopPropagation();
    const cle = `${ligne.account_id}/${ligne.invitation.mailbox}/${ligne.invitation.uid}`;
    if (reponsesInvitation[cle]) return;
    reponsesInvitation[cle] = true;
    // OPTIMISTE (terrain R3'a, corrigé 3e passe) : la puce remplace
    // les boutons À L'INSTANT du clic — le journal suit derrière ; un
    // échec rend l'état d'avant et le dit. Les lignes vivent dans des
    // pages NON réactives (le fenêtrage) : c'est `version` — le canal
    // d'invalidation maison — qui redessine la fenêtre, sinon la puce
    // n'apparaissait qu'à la prochaine invalidation venue d'ailleurs
    // (la sélection, une sonde…).
    const avant = ligne.invitation.reponse;
    ligne.invitation.reponse = reponse;
    version += 1;
    try {
      const sujet = t(`inv.sujet_${reponse}`, { titre: ligne.invitation.titre });
      await appel('repondre_invitation', {
        accountId: ligne.account_id,
        mailbox: ligne.invitation.mailbox,
        uid: ligne.invitation.uid,
        reponse,
        sujet,
        corps: sujet,
      });
      appel('flush_outbox').catch(() => {});
    } catch (err) {
      ligne.invitation.reponse = avant;
      version += 1;
      onflash(t('erreur.invitation', { err }));
    } finally {
      reponsesInvitation[cle] = false;
    }
  }

  // R4 (PLAN-RETOURS-7, D4/D5) : les conversations ÉPINGLÉES de la
  // Réception — servies À PART (`pinned_rows`), préposées au flot dans
  // le MÊME cadre de défilement ; le flot paginé les exclut côté cœur
  // (jamais deux fois la même ligne). Leur hauteur MESURÉE recale le
  // fenêtrage : le flot commence sous la section.
  let epingles = $state([]);
  let hautEpinglesMesure = $state(0);
  // La mesure survit au démontage du bloc : zéro dès qu'il n'y a plus
  // d'épingle, sans attendre une mesure qui ne viendra pas.
  const hautEpingles = $derived(epingles.length > 0 ? hautEpinglesMesure : 0);
  // Le vide ne s'affirme jamais sans preuve (E2) — preuve des DEUX
  // sources : la page 0 du flot ET la réponse des épingles. Sans ce
  // drapeau, une boîte entièrement épinglée dirait « Aucun message
  // ici. » pendant (ou après, sur échec) le vol de `pinned_rows`
  // (revue 2026-08-21).
  let epinglesRepondues = $state(false);
  function lancerEpingles() {
    if (categorie !== 'reception') {
      epingles = [];
      epinglesRepondues = true;
      return;
    }
    const neeSource = source;
    appel('pinned_rows', { accountId: compte, nonLus: onglet === 'nonlus' })
      .then((rows) => {
        if (neeSource === source) epingles = rows;
      })
      .catch((err) => console.error('pinned_rows :', err))
      .finally(() => {
        if (neeSource === source) epinglesRepondues = true;
      });
  }
  const extraPuce = $derived(h2 - h1);

  function chipsAvant(i) {
    let extra = 0;
    const pleine = Math.floor(i / PAGE);
    for (const [p, n] of chipsParPage) {
      if (p < pleine) extra += n;
    }
    const page = pages.get(pleine);
    if (page) {
      const borne = i - pleine * PAGE;
      for (let k = 0; k < borne && k < page.length; k++) {
        extra += rangsPuces(page[k]);
      }
    }
    return extra;
  }
  function decalage(i) {
    return i * h1 + chipsAvant(i) * extraPuce + entetesAvant(i) * H_ENTETE;
  }

  const hauteurEspace = $derived.by(() => {
    void version;
    if (total === 0) return 0;
    let extra = 0;
    for (const n of chipsParPage.values()) extra += n;
    return total * h1 + extra * extraPuce + entetes.length * H_ENTETE;
  });

  function indexPour(scrollTop) {
    let i = Math.max(0, Math.floor(scrollTop / h1));
    for (let tour = 0; tour < 4; tour++) {
      const corrige = Math.max(
        0,
        Math.floor(
          (scrollTop - chipsAvant(i) * extraPuce - entetesAvant(i) * H_ENTETE) / h1,
        ),
      );
      if (corrige === i) break;
      i = corrige;
    }
    return Math.min(i, Math.max(0, total - 1));
  }

  // PLAN-DEFILEMENT-PROFOND E1 : au plus VOL_MAX pages en vol
  // (`pending` est la jauge — une seule vérité), et à chaque vol libre
  // on lance la page la plus utile de la fenêtre COURANTE — jamais
  // celles d'une position dépassée. Avant : l'effet servait chaque page
  // traversée par chaque position d'un drag tenu (~161 appels pour 2 s
  // de barre, mesurés au banc) ; la file sérialisée de `hors_pompe`
  // (ADR 0019) se drainait en minutes sur la vraie base et TOUTES les
  // commandes attendaient derrière.
  //
  // UN seul vol (terrain 2026-08-20) : le cœur sérialise de toute
  // façon (verrou global) — deux vols ne paralléliseraient rien, ils
  // ne feraient qu'allonger d'une page dépassée l'attente de la page
  // utile à l'arrêt du geste. La fenêtre à cheval se sert en deux
  // allers successifs, exactement ce que le cœur aurait fait.
  const VOL_MAX = 1;

  // La page la plus utile : celles de la fenêtre visible, la plus
  // proche de `premier` d'abord ; en dernier recours la page 0
  // dépareillée (elle porte le total frais d'une recharge). Null si
  // tout ce qui compte est servi ou déjà en vol.
  // Les vols en cours sont clés par (source, page) : un vol d'une
  // AUTRE source n'occulte jamais la même page de la source neuve — la
  // jauge (`pending.size`) les compte tous, les recherches par clé ne
  // voient que la source courante.
  const cleVol = (p) => `${source}:${p}`;

  function pageUtile() {
    const de = Math.floor(debut / PAGE);
    const a = Math.floor(Math.max(0, fin - 1) / PAGE);
    const pivot = Math.floor(premier / PAGE);
    const candidats = [];
    for (let p = de; p <= a; p++) candidats.push(p);
    candidats.sort((x, y) => Math.abs(x - pivot) - Math.abs(y - pivot));
    candidats.push(0);
    for (const p of candidats) {
      if (servieA.get(p) !== generation && !pending.has(cleVol(p))) return p;
    }
    return null;
  }

  function pomper() {
    if (categorie === 'brouillons') return;
    // La page 0 d'une source qui n'a pas encore répondu passe DEVANT la
    // jauge (revue 2026-08-20) : une bascule de dossier part tout de
    // suite, même si une page profonde de l'ancienne source vole
    // encore — le débord est borné (une seule, `pending` la retient).
    if (!sourceRepondue && servieA.get(0) !== generation && !pending.has(cleVol(0))) {
      lancer(0);
    }
    while (pending.size < VOL_MAX) {
      const p = pageUtile();
      if (p === null) break;
      lancer(p);
    }
    // Le comptage — jamais devant des lignes : seulement quand la pompe
    // est au repos, et jamais si une page courte a déjà dit le total.
    if (
      pending.size === 0 &&
      sourceRepondue &&
      totalServiA !== generation &&
      !totalEnVol
    ) {
      lancerTotal();
    }
    if (
      sections &&
      pending.size === 0 &&
      sourceRepondue &&
      coutureServieA !== generation &&
      !coutureEnVol
    ) {
      lancerCouture();
    }
  }

  // Le total de la source, à part des pages (terrain 2026-08-20) : le
  // comptage d'une intégrale coûte plus que la page — il suit le
  // premier rendu, il ne le précède jamais.
  let totalEnVol = false;
  let totalServiA = -1;
  function lancerTotal() {
    const neeSource = source;
    const nee = generation;
    totalEnVol = true;
    appel('category_total', {
      category: categorie,
      accountId: compte,
      nonLus: onglet === 'nonlus',
    })
      .then((n) => {
        if (neeSource !== source) return;
        total = n;
        totalPrecis = true;
        totalServiA = nee;
      })
      .catch((err) => {
        console.error(`category_total ${categorie} :`, err);
      })
      .finally(() => {
        totalEnVol = false;
      });
    // E4 : la couture des sections — le COUNT des non-lus, même
    // cadence que le total, jamais devant des lignes.
  }

  // E4 : la couture des sections — le COUNT des non-lus, sa PROPRE
  // pompe : le total peut venir d'une page courte sans que
  // `lancerTotal` ne parte jamais (petite boîte) — greffée dessus, la
  // couture ne partait pas non plus (prouvé au décor e2e).
  let coutureEnVol = false;
  let coutureServieA = -1;
  function lancerCouture() {
    const neeSource = source;
    const nee = generation;
    coutureEnVol = true;
    appel('category_total', {
      category: categorie,
      accountId: compte,
      nonLus: true,
    })
      .then((n) => {
        if (neeSource !== source) return;
        couture = n;
        coutureServieA = nee;
      })
      .catch(() => {})
      .finally(() => {
        coutureEnVol = false;
      });
  }

  function lancer(p) {
    const neeSource = source;
    const nee = generation;
    const cle = cleVol(p);
    const t0 = performance.now();
    // Un échec ne repompe pas (revue 2026-08-20) : la même page serait
    // resélectionnée à l'instant — tempête de relances à vitesse
    // microtâche sur toute erreur persistante. L'essai suivant attend
    // un geste ou un effet, comme avant.
    let echoue = false;
    const promesse = appel('list_category', {
      category: categorie,
      accountId: compte,
      nonLus: onglet === 'nonlus',
      offset: p * PAGE,
      limit: PAGE,
    })
      .then(async (page) => {
        // Autre source : résultat jeté. Même source, génération
        // antérieure (recharge pendant le vol) : les lignes restent
        // bonnes à afficher — consignées à LEUR génération, la page
        // demeure dépareillée et se ressert (stale-while-revalidate).
        if (neeSource !== source) return;
        sourceRepondue = true;
        // La page ne porte plus de total (terrain 2026-08-20) : les
        // lignes elles-mêmes le disent — une page COURTE marque la fin
        // exacte de la liste, une page pleine pose un PLANCHER que la
        // barre de défilement suit en attendant `category_total`.
        if (page.rows.length < PAGE) {
          total = p * PAGE + page.rows.length;
          totalPrecis = true;
          totalServiA = nee;
        } else {
          total = Math.max(total, (p + 1) * PAGE);
        }
        // Le delta de puces, pas le compte brut : une page REMPLACÉE
        // affichait déjà les siennes — l'ancrage du défilement ne doit
        // bouger que de la différence (première servie : avant = 0).
        const avant = chipsParPage.get(p) ?? 0;
        pages.set(p, page.rows);
        servieA.set(p, nee);
        let n = 0;
        for (const l of page.rows) n += rangsPuces(l);
        chipsParPage.set(p, n);
        if (premierePageMs === null) premierePageMs = performance.now() - t0;
        const delta = n - avant;
        if (delta !== 0 && (p + 1) * PAGE <= premier && cadre) {
          version += 1;
          await tick();
          cadre.scrollTop += delta * extraPuce;
        } else {
          version += 1;
        }
      })
      .catch((err) => {
        echoue = true;
        console.error(`list_category ${categorie} page ${p} :`, err);
      })
      .finally(() => {
        pending.delete(cle);
        // Un vol s'est libéré : la fenêtre COURANTE choisit la suite.
        if (!echoue) pomper();
      });
    pending.set(cle, promesse);
    return promesse;
  }

  function servirPage(p) {
    // Le dossier Brouillons ne se sert pas ici : sa page vient de
    // `list_drafts` (PLAN-BROUILLONS, B-D1), pas de `list_category`.
    // Conservée pour `allerEtServir` (banc P1, e2e) : un saut délibéré
    // vise exactement ses pages — il sert sans attendre la jauge (le
    // débord d'un saut est assumé, une fenêtre au plus).
    if (categorie === 'brouillons') return Promise.resolve();
    if (servieA.get(p) === generation) return Promise.resolve();
    // La clé qualifiée ne voit que la source courante : jamais une
    // promesse d'une autre source, qui se réglerait sans rien écrire.
    return pending.get(cleVol(p)) ?? lancer(p);
  }

  // Nouvelle source -> repartir du haut, tout jeter, resservir. SEULE la
  // clé de source est une dépendance : tout le reste est sous `untrack`,
  // sans quoi l'effet dépendrait de ce qu'il modifie (boucle).
  $effect(() => {
    void categorie;
    void compte;
    void onglet;
    untrack(() => {
      source += 1;
      generation += 1;
      pages = new Map();
      chipsParPage = new Map();
      servieA = new Map();
      // `pending` reste : les vols ouverts occupent la jauge jusqu'à
      // leur règlement — leur résultat d'une autre source est jeté, et
      // leur clôture repompe la fenêtre de la source neuve (la page 0
      // de celle-ci passe devant la jauge, voir pomper).
      total = 0;
      // E4 (revue) : la couture est un état DE SOURCE — gardée, elle
      // peindrait le N de l'ancienne boîte sur la nouvelle (et
      // masquerait « Déjà consulté » tant que couture >= total).
      couture = 0;
      coutureServieA = -1;
      sourceRepondue = false;
      totalPrecis = false;
      premier = 0;
      selection = null;
      // R1 : la sélection multiple ne survit jamais à sa source — un
      // geste de masse sur des rangées qu'on ne voit plus serait un
      // piège (D4 : la sélection est toujours sous les yeux).
      viderSelection();
      // La première page se re-mesure PAR source (revue 2026-08-20) :
      // figée à la toute première, `etat().premierePageMs` aurait menti
      // aux bancs — le statut de démarrage, lui, est déjà capturé.
      premierePageMs = null;
      epingles = [];
      epinglesRepondues = false;
      if (cadre) cadre.scrollTop = 0;
      version += 1;
      pomper();
      lancerEpingles();
    });
  });

  // La section épinglée change la hauteur AU-DESSUS du flot hors de
  // tout événement de défilement (mesure asynchrone, pin/désépingle) :
  // `premier` se recale à chaque mouvement de la mesure, sinon la
  // fenêtre resterait calée sur l'ancienne origine jusqu'au prochain
  // pixel de scroll (revue 2026-08-21).
  $effect(() => {
    void hautEpingles;
    untrack(() => {
      if (cadre) surDefilement();
    });
  });

  // E4 : l'arrivée de la couture (0 → n) déplace la hauteur des
  // entêtes de section hors de tout événement de défilement — même
  // recalage que la bande épinglée.
  $effect(() => {
    void entetes;
    untrack(() => {
      if (cadre) surDefilement();
    });
  });

  // A83 — le ré-ancrage au changement de cran, et c'est l'INVERSE de
  // l'effet ci-dessus : quand la hauteur au-dessus du flot bouge, les
  // pixels gardent leur sens et c'est l'index qu'on recalcule ; quand
  // c'est la hauteur d'une RANGÉE qui bouge, la conversion
  // index <-> pixel change et il faut garder la LIGNE DU HAUT en
  // déplaçant le défilement. Sans cela, changer d'espacement ferait
  // sauter la liste ailleurs — d'autant plus loin qu'on a défilé.
  //
  // Il faut DEUX temps, et la revue a montré pourquoi : on lit la
  // position avec l'ANCIENNE géométrie et on l'écrit avec la NOUVELLE.
  //
  // 1) La CAPTURE se déclenche sur le cran lui-même — `padRangee()`, un
  //    état AMONT, qui bouge avant que le style ne soit relayouté et
  //    donc avant que les sondes ne re-mesurent. Capturer depuis
  //    l'effet de `h1` serait TROP TARD : l'effet des épinglées, créé
  //    plus haut donc joué avant lui dans le même flush (les épinglées
  //    sont des `.ligne`, elles grandissent aussi), a déjà réécrit
  //    `premier` avec la nouvelle hauteur contre l'ancien scrollTop —
  //    44 rangées de dérive mesurées avec deux conversations épinglées.
  let ancreCran = null;
  $effect(() => {
    void padRangee();
    untrack(() => {
      ancreCran = cadre
        ? { ligne: premier, dansEpingles: cadre.scrollTop < hautEpingles }
        : null;
    });
  });

  // 2) L'APPLICATION attend que les sondes aient rendu la nouvelle
  //    hauteur, puis rend sa ligne à l'utilisateur. Deux gardes, chacune
  //    payée par un défaut réel :
  //    — le FLOT seulement : `aller()` parle la géométrie du flot
  //      fenêtré ; l'appliquer au dossier Brouillons (où `total` reste 0,
  //      donc `aller(0)`) ou à une recherche remonterait la liste en
  //      haut à chaque changement de cran ;
  //    — pas depuis la BANDE ÉPINGLÉE : `aller(0)` pose le défilement
  //      SOUS elle, c'est-à-dire qu'il la ferait sortir de l'écran de
  //      quelqu'un qui était précisément en train de la regarder.
  $effect(() => {
    void h1;
    const ancre = ancreCran;
    if (ancre === null) return;
    ancreCran = null;
    if (ancre.dansEpingles) return;
    untrack(() => {
      if (cadre && resultats === null && lignesBrouillons === null) {
        aller(ancre.ligne);
      }
    });
  });

  // Décision CE D3 (2026-08-25) — défaut PRÉEXISTANT corrigé ici : la
  // hauteur du cadre se lisait par `cadre.clientHeight`, qui n'est pas
  // un signal. Le dérivé ne se recalculait donc qu'au changement de
  // `cadre` ou de `h1`, et agrandir la fenêtre de plus de OVER rangées
  // laissait une bande vide en bas jusqu'au prochain défilement.
  // `bind:clientHeight` compile vers un ResizeObserver (le patron de
  // `hautEpingles`) : la fenêtre suit la fenêtre.
  // Corrigé DANS ce chantier et pas en dette, parce que le cran
  // d'espacement l'aurait masqué par intermittence — chaque changement
  // de cran recalcule `visibles` — et l'aurait rendu irreproductible.
  let hCadre = $state(0);
  const visibles = $derived(
    hCadre > 0 ? Math.ceil(hCadre / h1) + 1 : 12,
  );
  const debut = $derived(Math.max(0, premier - OVER));
  const fin = $derived(Math.min(total, premier + visibles + OVER));

  const fenetre = $derived.by(() => {
    void version;
    const arr = [];
    for (let i = debut; i < fin; i++) {
      const page = pages.get(Math.floor(i / PAGE));
      arr.push({ i, ligne: page ? page[i % PAGE] : null });
    }
    return arr;
  });

  $effect(() => {
    void debut;
    void fin;
    untrack(pomper);
  });

  function surDefilement() {
    // La section épinglée vit AU-DESSUS du flot dans le même cadre :
    // le fenêtrage du flot se calcule sous elle.
    premier = indexPour(Math.max(0, cadre.scrollTop - hautEpingles));
  }

  // D1 — recherche (FTS5, `search_messages`) : les résultats prennent la
  // place de la liste, AUX LIGNES MÊMES du prototype — aucune UI neuve.
  // Bornés côté coeur : pas de fenêtrage. En deçà de 3 caractères, la
  // boîte revient telle quelle.
  let resultats = $state(null);
  let totalResultats = $state(0);
  let chargementPlus = $state(false);
  let minuterieRecherche;
  let jetonRecherche = 0;
  // Le lot, miroir du `SEARCH_LIMIT` de la commande : la taille d'un « charger
  // plus » (le dernier lot rend le reste, moins de 100).
  const LOT = 100;
  // Borne douce (D1) : au-delà, le bouton « charger plus » s'efface au profit
  // d'une invite à affiner — la liste n'est pas fenêtrée, empiler sans fin
  // finirait par alourdir le DOM. Dix lots.
  const MAX_RESULTATS = 10 * LOT;
  async function executerRecherche(q) {
    const mien = ++jetonRecherche;
    try {
      const res = await appel('search_messages', { query: q, offset: 0 });
      if (mien !== jetonRecherche) return; // frappe plus récente
      resultats = res.rows;
      totalResultats = res.total;
      // Le nombre rendu (plafonné) ET le total : la barre dit « N sur M ».
      onresultats(res.rows.length, res.total);
    } catch (err) {
      console.error('search_messages :', err);
    }
  }
  // « Charger plus » : le lot suivant, APPENDÉ. On ne touche pas au jeton
  // (rien à annuler), mais on le CAPTURE : si une frappe survient pendant le
  // chargement, elle l'incrémente et on jette ce lot devenu caduc.
  async function chargerPlus() {
    const q = recherche.trim();
    // Garde SYNCHRONE : un seul lot en vol à la fois. Le bouton `disabled`
    // peut retarder d'un tick — ce test bloque un double-déclenchement avant
    // qu'il ne lise deux fois le même offset (donc n'append deux fois le
    // même lot).
    if (q.length < 3 || resultats === null || chargementPlus) return;
    const mien = jetonRecherche;
    chargementPlus = true;
    try {
      const res = await appel('search_messages', { query: q, offset: resultats.length });
      if (mien !== jetonRecherche) return; // une recherche plus récente a pris la main
      resultats = [...resultats, ...res.rows];
      totalResultats = res.total;
      onresultats(resultats.length, res.total);
    } catch (err) {
      console.error('search_messages (plus) :', err);
    } finally {
      // Ce lot n'est plus en vol, QUOI QU'IL ARRIVE (superseded compris) :
      // sans reset inconditionnel, une frappe pendant le chargement laisserait
      // le drapeau à true et condamnerait le bouton des recherches suivantes.
      chargementPlus = false;
    }
  }
  $effect(() => {
    const q = recherche.trim();
    untrack(() => {
      clearTimeout(minuterieRecherche);
      // R1 : une frappe change les rangées affichées — la sélection
      // multiple se vide dans les deux sens (entrer, affiner, sortir).
      viderSelection();
      if (q.length < 3) {
        jetonRecherche += 1;
        resultats = null;
        totalResultats = 0;
        onresultats(null, null);
        return;
      }
      minuterieRecherche = setTimeout(() => executerRecherche(q), 150);
    });
  });

  $effect(() => {
    // Le dossier Brouillons compte ses lignes à lui — la barre de
    // statut dit « Brouillons · N éléments » sur la même mécanique.
    // Ailleurs, le compte ne se dit qu'une fois EXACT (E2 + terrain
    // 2026-08-20) : null tant que le vrai total n'est pas là — le
    // statut ne dira jamais « 0 éléments » sur une boîte qui n'a pas
    // parlé, ni un plancher provisoire comme s'il était le compte.
    // Les épinglées comptent : elles sont À L'ÉCRAN — sans elles, la
    // barre dirait « 8 éléments » devant 10 lignes (revue 2026-08-21).
    const n =
      lignesBrouillons !== null
        ? lignesBrouillons.length
        : totalPrecis
          ? total + epingles.length
          : null;
    untrack(() => ontotal(n));
  });

  // A83 : `sonder()` et `sondees` sont morts — les sondes sont montées
  // en permanence et se lient par `bind:offsetHeight`. Une mesure
  // ponctuelle ne pouvait pas suivre un cran d'espacement réglable.

  const cle = (l) => `${l.account_id}/${l.mailbox}/${l.uid}`;
  function choisir(l) {
    selection = cle(l);
    onselect(l);
  }
  const estChoisie = (l) => selection === cle(l);

  // --- Sélection multiple (PLAN-RETOURS-10 R1, D1-D4) -----------------
  // Un ensemble clé -> ligne (SvelteMap : les rangées vivent dans des
  // pages NON réactives, la carte réactive suffit à redessiner cases et
  // barre). `ancre` = la dernière rangée basculée — le Shift-clic étend
  // depuis elle, sur l'ORDRE AFFICHÉ des rangées chargées (refus §2.6 :
  // jamais « tout le dossier », la sélection vit dans le chargé).
  let cochees = $state(new SvelteMap());
  let ancre = null;
  const estCochee = (l) => cochees.has(cle(l));
  function basculer(l) {
    // Un lot en vol fige la sélection : cocher pendant l'exécution
    // fabriquerait des rangées jamais servies (revue).
    if (gesteEnCours) return;
    const k = cle(l);
    if (cochees.has(k)) cochees.delete(k);
    else cochees.set(k, l);
    ancre = k;
  }
  function lignesOrdonnees() {
    if (resultats !== null) return resultats;
    const flot = [];
    for (const p of [...pages.keys()].sort((a, b) => a - b)) flot.push(...pages.get(p));
    return [...epingles, ...flot];
  }
  function etendre(l) {
    if (gesteEnCours) return;
    const ordre = lignesOrdonnees();
    // Terrain 2026-08-27 (R1-2) : sans ancre de coche, l'ancre est la
    // rangée SÉLECTIONNÉE (le message choisi, ex. le premier au
    // démarrage) — la plage va de la sélection à la cible, incluses.
    const depart = ancre ?? selection;
    const ia = depart === null ? -1 : ordre.findIndex((x) => cle(x) === depart);
    const ib = ordre.findIndex((x) => cle(x) === cle(l));
    // Sans ancre visible (jamais cochée ni choisie, ou hors des pages
    // chargées), le Shift-clic vaut une bascule simple — jamais un
    // silence.
    if (ia < 0 || ib < 0) return basculer(l);
    for (let i = Math.min(ia, ib); i <= Math.max(ia, ib); i++) {
      cochees.set(cle(ordre[i]), ordre[i]);
    }
  }
  function viderSelection() {
    cochees.clear();
    ancre = null;
  }
  // L'App décoche la cible d'un geste UNITAIRE abouti (e/Suppr, boutons
  // du fil) : la barre ne compte jamais une rangée qui n'est plus — un
  // lot rejoué sur un uid parti rapporterait un faux échec (revue).
  export function decocher(l) {
    cochees.delete(cle(l));
    if (ancre === cle(l)) ancre = null;
  }
  // Le clic d'une rangée, trois régimes : Shift étend (Ctrl+Shift
  // aussi), Ctrl/Cmd bascule ET choisit, nu = choisir (l'existant,
  // inchangé). Terrain 2026-08-27 (R1-1) : le focus de lecture SUIT le
  // Ctrl-clic — laisser le liseré (et le volet) sur une autre rangée
  // que celle qu'on vient de cocher déroutait.
  // Constat terrain (2026-08-15, suite d'A38), valable pour les TROIS
  // régimes : choisie ou cochée à la souris (detail > 0), la rangée
  // rend le focus — sinon l'anneau :focus-visible surgirait plus tard
  // sur un nœud recyclé par index.
  function clicRangee(e, l) {
    if (e.detail > 0) e.currentTarget.blur();
    if (e.shiftKey) return etendre(l);
    if (e.ctrlKey || e.metaKey) basculer(l);
    choisir(l);
  }
  // Le geste de masse : l'App agit sur l'INSTANTANÉ du lot ; au retour,
  // seul ce lot se décoche — une rangée cochée pendant le vol (bloquée
  // aujourd'hui, mais la garde ne repose pas dessus) survivrait.
  // Exporté : les raccourcis clavier de l'App (e/Suppr) s'appliquent au
  // lot coché quand il existe (terrain 2026-08-27, R1-8).
  export const enSelection = () => cochees.size > 0;
  let gesteEnCours = $state(false);
  export async function agir(action) {
    if (gesteEnCours) return;
    gesteEnCours = true;
    const lot = [...cochees.values()];
    try {
      await ongroupe(action, lot);
    } finally {
      for (const l of lot) cochees.delete(cle(l));
      ancre = null;
      gesteEnCours = false;
    }
  }

  // --- Brouillons (PLAN-BROUILLONS) -----------------------------------
  // Le fil -> son brouillon le plus récent : la mention de la Réception
  // (variante B validée) montre le préfixe et le CORPS du brouillon en
  // aperçu — première ligne et heure intactes (B-D3). Jamais sur une
  // recherche : un résultat est un message, pas une conversation.
  const brouillonsParFil = $derived.by(() => {
    const carte = new Map();
    for (const b of brouillons) {
      if (b.thread_id == null) continue;
      const connu = carte.get(b.thread_id);
      if (!connu || b.updated_epoch > connu.updated_epoch) carte.set(b.thread_id, b);
    }
    return carte;
  });
  const brouillonDe = (l) =>
    categorie === 'reception' && resultats === null
      ? (brouillonsParFil.get(l.thread_id) ?? null)
      : null;
  // Le dossier : les brouillons du compte borné par la nav, déjà du
  // plus récent au plus ancien (`list_drafts`). Peu nombreux par
  // construction : le chemin non fenêtré des résultats suffit.
  const lignesBrouillons = $derived(
    categorie === 'brouillons'
      ? brouillons.filter((b) => compte === null || b.account_id === compte)
      : null,
  );

  // R1 (RETOURS-10, D1) : les gestes de la barre de sélection — une
  // table, comme ONGLETS ; en Indésirables, « Signaler » cède à « Ce
  // n'est pas un spam », le miroir du volet de lecture.
  const GESTES_BARRE = $derived([
    { action: 'lu', icone: 'drafts', libelle: 'action.marquerLu' },
    { action: 'nonlu', icone: 'mark_email_unread', libelle: 'action.marquerNonLu' },
    { action: 'archiver', icone: 'archive', libelle: 'action.archiver' },
    categorie === 'indesirables'
      ? { action: 'nonspam', icone: 'report', libelle: 'action.pasSpam' }
      : { action: 'spam', icone: 'report', libelle: 'action.signalerSpam' },
    { action: 'supprimer', icone: 'delete', libelle: 'action.supprimer' },
  ]);

  const ONGLETS = [
    { id: 'tous', icone: 'inbox', libelle: 'onglet.tous' },
    { id: 'nonlus', icone: 'mark_email_unread', libelle: 'onglet.nonlus' },
    { id: 'brouillons', icone: 'edit_note', libelle: 'boite.brouillons' },
  ];
  const ongletActif = $derived(categorie === 'brouillons' ? 'brouillons' : onglet);

  // --- API (App, banc P1, e2e) ---------------------------------------
  export function aller(index) {
    cadre.scrollTop = decalage(index) + hautEpingles;
    surDefilement();
  }
  export async function allerEtServir(index) {
    const t0 = performance.now();
    aller(index);
    const de = Math.floor(Math.max(0, index - OVER) / PAGE);
    const a = Math.floor(Math.min(Math.max(0, total - 1), index + visibles + OVER) / PAGE);
    const attentes = [];
    for (let p = de; p <= a; p++) attentes.push(servirPage(p));
    await Promise.all(attentes);
    await tick();
    void cadre.offsetHeight;
    return performance.now() - t0;
  }
  export function etat() {
    // `totalPrecis` : les bancs qui sautent « sur toute la profondeur »
    // (mesure-v2) doivent attendre le VRAI total — le plancher tiré des
    // premières lignes ne couvre que l'écran.
    return { total, totalPrecis, premier, h1, h2, premierePageMs };
  }
  export function ligneA(index) {
    const page = pages.get(Math.floor(index / PAGE));
    return page ? page[index % PAGE] : null;
  }
  // Le triage clavier (App) : la sélection se pose sans passer par le
  // clic — même clé, même liseré, aucun rappel onselect.
  export function selectionner(ligne) {
    selection = cle(ligne);
  }
  // La ligne SOUS celle-ci. Recherche active : la suivante des
  // résultats ; sinon l'index absolu dans les pages fenêtrées — une
  // page voisine non servie rend null (rare : la fenêtre sert large).
  export function suivante(ligne) {
    const id = cle(ligne);
    if (resultats !== null) {
      const i = resultats.findIndex((l) => cle(l) === id);
      return i >= 0 && i + 1 < resultats.length ? resultats[i + 1] : null;
    }
    // Depuis la section épinglée : la suivante y vit, ou la première
    // ligne du flot en sortant par le bas.
    const e = epingles.findIndex((l) => cle(l) === id);
    if (e >= 0) {
      return e + 1 < epingles.length ? epingles[e + 1] : (ligneA(0) ?? null);
    }
    for (const [p, rows] of pages) {
      const i = rows.findIndex((l) => cle(l) === id);
      if (i >= 0) return ligneA(p * PAGE + i + 1) ?? null;
    }
    return null;
  }
  export function marquerLue(ligne) {
    const id = cle(ligne);
    for (const page of pages.values()) {
      for (const l of page) {
        if (cle(l) === id) l.thread_unseen = 0;
      }
    }
    for (const l of epingles) {
      if (cle(l) === id) l.thread_unseen = 0;
    }
    version += 1;
  }
  export function recharger() {
    // Stale-while-revalidate (PLAN-REACTIVITE E1) : les lignes servies
    // RESTENT affichées — chaque page est remplacée à l'arrivée de sa
    // version fraîche, jamais jetée avant. Le squelette n'existe qu'au
    // premier chargement d'une source et au défilement vers l'inconnu ;
    // les pages hors écran, gardées telles quelles, se resservent au
    // défilement (génération dépareillée).
    generation += 1;
    // La pompe ressert l'écran — tout le rang visible page à page,
    // puis la page 0 dépareillée (le total frais) ; les vols ouverts
    // gardent leurs places et repompent en se réglant (E1).
    pomper();
    // R4 : la section épinglée suit chaque recharge — un épinglage
    // déplace une ligne entre la section et le flot, jamais un doublon.
    lancerEpingles();
    // Une recherche ACTIVE se resert aussi : archiver un résultat doit
    // le retirer des résultats — la régression #4 de v1, même trou.
    if (resultats !== null) {
      const q = recherche.trim();
      if (q.length >= 3) executerRecherche(q);
    }
  }
</script>

<svelte:window
  onclick={() => (menuGestes = null)}
  onkeydown={(e) => {
    if (e.key === 'Escape') menuGestes = null;
  }} />

<section class="colonne" class:centre aria-label={t('liste.aria')} data-testid="liste">
  <!-- UI v3, E1 (verdict CE 2026-08-16) : le bandeau de la maquette
       Classique — le nom de la boîte courante, SEUL (« Tout marquer
       lu » écarté). Les clés boite.* sont celles de la nav. -->
  {#if cochees.size > 0}
    <!-- R1/D3 : la barre de la liste SE TRANSFORME tant que la
         sélection est non vide — le compte, les quatre gestes de masse
         (D1), Annuler. Aucune surface neuve : mêmes 52 px, même filet.
         Boutons-icônes à la grammaire de l'entête (32 px), le libellé
         vit dans aria-label ET title. En Indésirables, « Signaler
         indésirable » cède à « Ce n'est pas un spam » — le miroir du
         volet de lecture. -->
    <header class="bandeau bandeau-selection" data-testid="barre-selection">
      <h1>{t('liste.nSelection', { n: cochees.size })}</h1>
      {#each GESTES_BARRE as g (g.action)}
        <button type="button" class="btn-barre" data-testid="barre-{g.action}"
                disabled={gesteEnCours}
                aria-label={t(g.libelle)} title={t(g.libelle)}
                onclick={() => agir(g.action)}><Icone nom={g.icone} /></button>
      {/each}
      <!-- Annuler gèle aussi pendant le lot : la barre qui se replierait
           pendant que les commandes continuent se lirait comme une
           annulation (revue). -->
      <button type="button" class="btn-barre" data-testid="barre-annuler"
              disabled={gesteEnCours}
              aria-label={t('action.annulerSelection')} title={t('action.annulerSelection')}
              onclick={viderSelection}><Icone nom="close" /></button>
    </header>
  {:else if centre}
    <!-- RETOURS-14 R2 (D2/D3) : la Réception organisée prend l'entête
         normalisé des vues du mode (classes partagées .entete-vue de
         systeme.css, patron Kiosque/Portier R7/R11) — titre seul (D2),
         ni bandeau générique ni onglets (D3, plus bas). -->
    <header class="tete-organisee" data-testid="liste-titre">
      <h2 class="display entete-vue" data-testid="reception-titre">
        <span class="glyphe-titre" aria-hidden="true"><Icone nom="inbox" taille={26} /></span>{t(cleLibelleBoite('reception'))}</h2>
    </header>
  {:else}
    <header class="bandeau" data-testid="liste-titre">
      <!-- RETOURS-13 R3 : le libellé sort de LA règle partagée. -->
      <h1>{t(cleLibelleBoite(categorie))}</h1>
    </header>
  {/if}
  <!-- A83 : le cran d'espacement se pose EN JETON sur le cadre — les
       cinq poses de `.ligne` (sondes, attente, flot, épinglées,
       brouillons) sont dessous et le prennent d'un coup, sondes
       comprises. Le patron est celui des largeurs de volets
       (`--l-nav`) ; le trait d'union le fait échapper au contrat des
       17 jetons de thème, et c'est voulu : c'est une dimension de mise
       en page, pas une couleur. -->
  <div class="cadre" bind:this={cadre} bind:clientHeight={hCadre}
       class:selection-en-cours={cochees.size > 0}
       onscroll={surDefilement}
       style="--rangee-pad:{padRangee()}px">
    <!-- RETOURS-14 R2 : la section courante, collée en tête du cadre.
         `height:0` : la bande vit HORS de la géométrie du fenêtrage
         (decalage/indexPour ne la connaissent pas — le piège E4 des
         espaceurs ne la concerne donc pas). -->
    {#if sectionCollee}
      <div class="section-collee" data-testid="section-collee" aria-hidden="true">
        <span class="cadre-entete"><span class="lab">{sectionCollee.libelle}</span></span>
      </div>
    {/if}
    <!-- A81 : les sondes suivent la rangée réelle — plus de colonne de
         tuile ; une sonde qui rendrait un objet mort mentirait sur la
         géométrie.
         A83 : elles restent MONTÉES et se re-mesurent seules
         (`bind:offsetHeight` compile vers un ResizeObserver, le patron
         de `hautEpingles`). Avant, elles étaient retirées après une
         mesure unique et `sondees` n'était jamais remis à false : un
         changement de cran aurait redessiné les rangées à la nouvelle
         hauteur en laissant les gabarits figés sur l'ancienne — barre
         de défilement fausse de 13,6 % à 27,3 %, et jusqu'à 12 000 px
         d'écart sur un saut (mesuré, PLAN-ESPACEMENT §3). Montées en
         permanence, la classe de bug est IMPOSSIBLE, pas corrigée.
         La cage est POSITIONNÉE, et ce n'est pas décoratif : sans son
         `position:relative` elle n'est pas le bloc conteneur des sondes
         en `position:absolute`, qui se calent alors sur `.cadre` et lui
         ajoutent jusqu'à 85 px de défilement FANTÔME sur une fenêtre
         courte (mesuré au banc, variante C). -->
    <div class="sondes-cage" aria-hidden="true">
      <div class="sondes">
        <article class="ligne" bind:offsetHeight={h1}>
          <div class="l1"><span class="exp">Sonde</span><span class="essor"></span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
        </article>
        <article class="ligne" bind:offsetHeight={h2}>
          <div class="l1"><span class="exp">Sonde</span><span class="essor"></span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
          <div class="puces"><span class="puce"><Icone nom="forum" />2</span></div>
        </article>
      </div>
    </div>
    {#snippet attente()}
      <article class="ligne attente" data-testid="ligne-attente">
        <div class="l1"><span class="exp">…</span><span class="essor"></span><span class="heure"></span></div>
        <p class="objet">…</p>
        <p class="apercu"></p>
      </article>
    {/snippet}
    {#snippet rangee(ligne, epinglee = false)}
      <!-- A80 : le bloc de boîte vit partout où les comptes se
           MÉLANGENT — boîte unifiée (D3/D7) et recherche (toujours
           multi-comptes, même depuis la vue d'un seul compte ; revue
           2026-08-22) — et sur TOUTES les rangées, repère ou non (D8). -->
      {@const boite = boiteDe(ligne)}
      {@const cochee = estCochee(ligne)}
      <!-- R1 : le clic vit en trois régimes (clicRangee) — Ctrl/Cmd
           bascule la coche, Shift étend depuis l'ancre, nu = choisir
           (la note d'A38 sur le focus vit dans clicRangee). Le
           mousedown avale la sélection de TEXTE d'un Shift-clic —
           jamais le geste. -->
      <div class="ligne"
           class:nonlu={ligne.thread_unseen > 0}
           class:choisie={estChoisie(ligne)}
           class:cochee={cochee}
           data-testid="ligne"
           role="button" tabindex="0"
           onmousedown={(e) => { if (e.shiftKey) e.preventDefault(); }}
           onclick={(e) => clicRangee(e, ligne)}
           onkeydown={activation(() => choisir(ligne))}>
        <!-- R1/D4 : la case — absolue dans la gouttière gauche, la
             géométrie de la rangée ne bouge JAMAIS (les sondes h1/h2
             mesurent la rangée sans elle) ; opacité 0 au repos, révélée
             au survol et dès qu'une sélection existe (CSS). tabindex -1 :
             la coche clavier passe par Entrée/Espace sur la rangée
             choisie, la case est une affordance de pointeur. -->
        <button type="button" class="case" data-testid="ligne-case"
                role="checkbox" aria-checked={cochee} tabindex="-1"
                aria-label={t('liste.cocher')}
                onclick={(e) => {
                  e.stopPropagation();
                  // La garde A38 vaut aussi ici : un clic ne laisse pas
                  // le focus sur un bouton d'un nœud recyclé.
                  e.currentTarget.blur();
                  basculer(ligne);
                }}>
          {#if cochee}<Icone nom="check" taille={12} />{/if}
        </button>
        <!-- A81 : la tuile aux initiales a quitté la liste — le nom en
             toutes lettres disait déjà ce qu'elle disait. A80 : la
             ligne d'entête porte à sa place le bloc de boîte, EN
             LIGNE — aucune colonne réservée, son absence ne décale
             rien (D7). Le tracé est aria-hidden : il DOUBLE le mot ;
             l'infobulle donne « libellé — adresse ». -->
        <div class="l1">
          <!-- V4 : le non-lu se dit par le disque de 9 px ET la graisse
               (A8 — jamais la couleur seule) ; l'épinglée porte la
               marque keep sur son sol --tuile (A73). -->
          {#if ligne.thread_unseen > 0}<span class="disque"></span>{/if}
          {#if epinglee}<span class="marque-epingle" aria-hidden="true"><Icone nom="keep" taille={14} /></span>{/if}
          <span class="exp">{#if versEnvoi(ligne)}{t('liste.dest', { a: correspondant(ligne) })}{:else}{ligne.sender}{/if}</span>
          {#if boite}
            <span class="boite" data-testid="ligne-boite" title={boite.titre}>
              <span class="mot">{t('liste.sur')}</span>
              {#if boite.repere}
                <span class="repere-nu" data-teinte={boite.repere.teinte}
                      aria-hidden="true"><Icone nom={boite.repere.icone} taille={14} /></span>
              {/if}
              <span class="lib">{boite.libelle}</span>
            </span>
          {/if}
          <span class="essor"></span>
          {#if gestesOrganise}
            <!-- E4 : le ⋯ à GAUCHE de l'heure, place RÉSERVÉE —
                 opacité seule, la géométrie ne bouge jamais. -->
            <button type="button" class="gestes" data-testid="ligne-gestes"
                    aria-label={t('liste.gestes')} aria-haspopup="menu"
                    aria-expanded={menuGestes?.cle === cleLigne(ligne)}
                    onclick={(e) => ouvrirGestes(e, ligne)}>
              <Icone nom="more_horiz" taille={14} /></button>
          {/if}
          <span class="heure">{quand(ligne.epoch)}</span>
        </div>
        <p class="objet">{ligne.subject}</p>
        {#if brouillonDe(ligne)}
          <!-- Variante B (PLAN-BROUILLONS §3) : l'aperçu dit le
               brouillon — préfixe et corps ; le reste de la ligne ne
               bouge pas. -->
          <p class="apercu"><span class="prefixe" data-testid="mention-brouillon">{t('liste.prefixeBrouillon')}</span>{brouillonDe(ligne).body}</p>
        {:else}
          <p class="apercu">{ligne.preview ?? ''}</p>
        {/if}
        <!-- PLAN-RETOURS-V3 R1 (verdict CE 2026-08-16, D1/D2) : la
             « ligne nue » d'A29 est renversée — le rang de puces du
             prototype revient à la ligne, aux règles de la tête du Fil
             (« N messages » si le fil en a plus d'un, « N fichiers »
             si pièces). Hauteur AU CONTENU (terrain CE 2026-08-16,
             renverse D1) : le rang n'existe que sur les porteurs et
             agrandit leur ligne — deux gabarits, le fenêtrage corrige
             par chipsAvant. En
             RECHERCHE, un résultat est un message, pas une conversation
             (le coeur sert thread_size=1 sans joindre threads) : la
             puce de fil n'y figure pas, par construction. Le compte de
             pièces est celui d'AVANT lecture du corps : 0 tant que le
             corps n'est pas rapatrié — la puce apparaît au fil du
             rattrapage, jamais à tort. -->
        <!-- R10/R3'c (terrain 2026-08-23) : les GESTES d'une invitation
             occupent un rang à eux — icône dite par couleur ET par le
             texte (A8), la puce agit à l'instant du clic (optimiste). -->
        {#if gestesInvitation(ligne)}
          <div class="puces" data-testid="puces-invitation">
            <button type="button" class="puce ton-accepte" data-testid="liste-accepter"
                    disabled={reponsesInvitation[`${ligne.account_id}/${ligne.invitation.mailbox}/${ligne.invitation.uid}`]}
                    onclick={(e) => repondreInvitation(e, ligne, 'accepte')}>
              <Icone nom="check_circle" />{t('action.accepter')}</button>
            <button type="button" class="puce ton-provisoire" data-testid="liste-provisoire"
                    disabled={reponsesInvitation[`${ligne.account_id}/${ligne.invitation.mailbox}/${ligne.invitation.uid}`]}
                    onclick={(e) => repondreInvitation(e, ligne, 'provisoire')}>
              <Icone nom="question_mark" />{t('action.provisoire')}</button>
            <button type="button" class="puce ton-refuse" data-testid="liste-refuser"
                    disabled={reponsesInvitation[`${ligne.account_id}/${ligne.invitation.mailbox}/${ligne.invitation.uid}`]}
                    onclick={(e) => repondreInvitation(e, ligne, 'refuse')}>
              <Icone nom="cancel" />{t('action.refuser')}</button>
          </div>
        {/if}
        {#if autresPuces(ligne) || (ligne.invitation && puceInvitation(ligne.invitation))}
          <div class="puces" data-testid="puces-ligne">
            <!-- R11 : la réponse donnée (ou l'annulation) rejoint le
                 rang commun — les autres puces remontent avec elle. -->
            {#if ligne.invitation && puceInvitation(ligne.invitation)}
              {@const puce = puceInvitation(ligne.invitation)}
              <span class="puce ton-{puce.ton}" data-testid="puce-invitation">
                {#if puce.icone}<Icone nom={puce.icone} />{/if}{puce.texte}</span>
            {/if}
            {#if ligne.pinned}
              <span class="puce"><Icone nom="keep" />{t('puce.epingle')}</span>
            {/if}
            {#if ligne.thread_size > 1}
              <span class="puce"><Icone nom="forum" />{t('puce.messages', { n: ligne.thread_size })}</span>
            {/if}
            {#if ligne.attachment_count > 0}
              <span class="puce"><Icone nom="attach_file" />{t('puce.fichiers', { n: ligne.attachment_count })}</span>
            {/if}
          </div>
        {/if}
      </div>
    {/snippet}
    {#if resultats !== null}
      <div class="fenetre-recherche" data-testid="resultats">
        {#if resultats.length === 0}
          <div class="vide-recherche"><p>{t('liste.aucunResultat')}</p></div>
        {/if}
        {#each resultats as ligne (`${ligne.account_id}/${ligne.mailbox}/${ligne.uid}`)}
          {@render rangee(ligne)}
        {/each}
        {#if resultats.length > 0 && resultats.length < totalResultats}
          {#if resultats.length < MAX_RESULTATS}
            <button type="button" class="charger-plus" data-testid="charger-plus"
                    disabled={chargementPlus} onclick={chargerPlus}>
              {t('liste.chargerPlus', { n: Math.min(LOT, totalResultats - resultats.length) })}
            </button>
          {:else}
            <p class="affiner" data-testid="affiner">{t('liste.affiner')}</p>
          {/if}
        {/if}
      </div>
    {:else if lignesBrouillons !== null}
      <!-- Le dossier Brouillons (B-D1) : les brouillons locaux, du plus
           récent au plus ancien. Le clic REPREND — jamais mark_seen, il
           n'y a rien à lire ici, seulement à finir. -->
      <div class="fenetre-recherche" data-testid="dossier-brouillons">
        {#if lignesBrouillons.length === 0}
          <div class="vide-recherche"><p>{t('liste.vide')}</p></div>
        {/if}
        {#each lignesBrouillons as b (b.id)}
          <!-- A81 : le dossier Brouillons GARDE sa tuile (D9 — elle y
               dit le destinataire) : la classe `tuilee` lui rend la
               colonne de tête que la rangée de liste a perdue. -->
          <div class="ligne tuilee" data-testid="ligne-brouillon"
               role="button" tabindex="0"
               onclick={() => onreprendre(b)}
               onkeydown={activation(() => onreprendre(b))}>
            <span class="avatar" aria-hidden="true">{initiales(b.to)}</span>
            <div class="l1">
              <span class="exp" class:sans={!b.to}>
                {b.to ? t('brouillons.a', { a: b.to }) : t('brouillons.sansDestinataire')}</span>
              <!-- L'essor pousse l'heure au bord droit : depuis A80,
                   .exp ne grandit plus (flex:0 1 auto), c'est lui qui
                   porte le ressort — ici comme dans la rangée du flot. -->
              <span class="essor"></span>
              <span class="heure">{quand(Math.floor(b.updated_epoch / 1000))}</span>
            </div>
            <p class="objet" class:sans={!b.subject}>{b.subject || t('brouillons.sansObjet')}</p>
            <p class="apercu">{b.body}</p>
          </div>
        {/each}
      </div>
    {:else}
      <!-- R4 : la section ÉPINGLÉE — les mêmes lignes, préposées au
           flot dans le même défilement ; le flot les exclut (D5). Sa
           hauteur mesurée recale le fenêtrage dessous. -->
      {#if epingles.length > 0}
        <div class="epingles" data-testid="epingles" bind:offsetHeight={hautEpinglesMesure}>
          {#each epingles as ligne (cle(ligne))}
            {@render rangee(ligne, true)}
          {/each}
        </div>
      {/if}
      {#if total === 0 && sourceRepondue && epinglesRepondues && epingles.length === 0}
        <!-- Les DEUX sources ont répondu zéro : le vide est PROUVÉ,
             sans comptage (une page courte dit le total d'elle-même).
             Tout-épinglé : le flot est vide mais la boîte ne l'est
             pas — la section seule, rien à affirmer dessous. -->
        <div class="vide"><p>{t('liste.vide')}</p></div>
      {:else if total === 0 && !(sourceRepondue && epinglesRepondues)}
        <!-- La source courante n'a pas encore répondu : l'attente se
             montre, le vide ne s'affirme jamais sans preuve
             (PLAN-DEFILEMENT-PROFOND E2). -->
        <div class="fenetre-recherche" data-testid="attente-source">
          {#each Array.from({ length: 6 }) as _, i (i)}
            {@render attente()}
          {/each}
        </div>
      {/if}
      <div class="espace" style="height:{hauteurEspace}px">
        {#each positionsEntetes as e (e.index)}
          <!-- E4 : l'entête de section vit HORS des rangées, absolu
               dans l'espace — la géométrie des lignes reste uniforme,
               le décrochement est porté par decalage/indexPour. Les
               positions viennent d'un dérivé qui ÉCOUTE `version`
               (revue E5) : une puce d'invitation qui pousse les
               rangées re-cale l'entête dans le même flush. -->
          <div class="entete-section" data-testid="section"
               style="top:{e.top}px">
            <span class="cadre-entete"><span class="lab">{e.libelle}</span></span>
          </div>
        {/each}
        <div class="fenetre" style="transform:translateY({decalage(debut)}px)">
          {#each fenetre as { i, ligne } (i)}
            <!-- E4 : la bande d'entête occupe 34 px RÉELS dans le flux
                 (les rangées s'empilent en flex — un décrochement qui
                 ne vivrait que dans decalage/indexPour ferait chevaucher
                 l'entête et dériver la fenêtre, constat de capture).
                 Quand la fenêtre COMMENCE à la borne, la bande est déjà
                 dans le translateY (entetesAvant compte e.index <= i). -->
            {#if entetes.some((e) => e.index === i && e.index > debut)}
              <div class="espace-entete" aria-hidden="true"></div>
            {/if}
            {#if ligne}
              {@render rangee(ligne)}
            {:else}
              {@render attente()}
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <!-- RETOURS-14 R2 (D3) : la Réception organisée n'a pas de pied —
       les onglets (et leur filtre Tous / Non lus) appartiennent au
       classique ; Brouillons reste accessible par la nav. -->
  {#if !centre}
  <div class="onglets" data-testid="onglets">
    {#each ONGLETS as o (o.id)}
      <span class="onglet" class:actif={ongletActif === o.id}
            data-testid="onglet" data-onglet={o.id}
            role="button" tabindex="0" aria-pressed={ongletActif === o.id}
            onclick={() => ononglet(o.id)}
            onkeydown={activation(() => ononglet(o.id))}>
        <Icone nom={o.icone} />{t(o.libelle)}
      </span>
    {/each}
  </div>
  {/if}
</section>

{#if menuGestes}
  <!-- E4 : le menu de gestes d'une rangée organisée — le dessin des
       menus du produit (patron Portier). « Déplacer vers… » sert les
       destinations AUTRES que la vue courante ; « Écarter » pose le
       Non nu (le choix se rejoue à l'historique du Portier). -->
  <div class="menu-gestes" role="menu" data-testid="menu-gestes"
       style="left:{menuGestes.x}px; top:{menuGestes.y}px">
    {#each ['reception', 'kiosque', 'registre'].filter((d) => d !== categorie) as dest (dest)}
      <button type="button" role="menuitem" data-testid={`gestes-${dest}`}
              onclick={() => geste(dest)}>
        <Icone nom={dest === 'reception' ? 'inbox' : dest} />{t('liste.deplacerVers', { boite: t(`boite.${dest}`) })}</button>
    {/each}
    <div class="filet-menu"></div>
    <button type="button" role="menuitem" data-testid="gestes-cote"
            onclick={() => {
              const { ligne } = menuGestes;
              menuGestes = null;
              oncote(ligne);
            }}>
      <Icone nom="pile" />{t('pile.mettre')}</button>
    <div class="filet-menu"></div>
    <button type="button" role="menuitem" data-testid="gestes-ecarter"
            onclick={() => geste('ecarte')}>
      <Icone nom="visibility_off" />{t('liste.ecarter')}</button>
  </div>
{/if}

<style>
  /* Géométrie et états du dessin des pistes (A29/A30) : lignes
     continues séparées au filet, sans carte ni ombre. */
  .colonne {
    display:flex; flex-direction:column; min-height:0;
    background:var(--bg); border-right:1px solid var(--border);
  }
  /* Le bandeau (UI v3, E1 — reformé PLAN-RETOURS-V3 R2) : le MÊME
     format visuel que le bandeau de filtre du bas — 52 px, fond --bg
     (V3 : le filet porte seul la séparation) ; titre 16 px 600. */
  .bandeau {
    flex:none; height:52px; display:flex; align-items:center;
    padding:0 16px; background:var(--bg);
    border-bottom:1px solid var(--border);
  }
  .bandeau h1 {
    margin:0; font-size:16px; font-weight:600; line-height:1.3;
    color:var(--ink); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  /* `isolation` (RETOURS-14 R2, revue) : la bande de section collée
     porte un z-index — confiné ICI, sinon elle passerait au-dessus
     des voiles modaux (z-index 2) du contexte racine. */
  .cadre { flex:1; overflow:auto; position:relative; isolation:isolate; }
  .espace { position:relative; }
  /* RETOURS-14 R2 : l'entête normalisé de la Réception organisée —
     même géométrie de page que Kiosque/Portier (marge 24/28), sans
     filet : la vue est une page du mode, pas un bandeau d'outil. */
  .tete-organisee { flex:none; padding:24px 28px 0; }
  .tete-organisee .entete-vue { max-width:760px; margin-inline:auto; }
  /* La section courante collée : hauteur NULLE (hors géométrie du
     fenêtrage), l'étiquette peinte par-dessus les rangées sur fond
     opaque — le dessin de la bande réelle (.entete-section). */
  .section-collee {
    position:sticky; top:0; z-index:3; height:0; overflow:visible;
  }
  .section-collee .cadre-entete {
    display:block; background:var(--bg);
    padding:10px 16px 6px; border-bottom:1px solid var(--border);
  }
  /* E4 : l'entête de section — le dessin de la règle-libellé du
     Portier (libellé nu, majuscules, encre atténuée), calé au bas de
     sa bande de 34 px, le filet du premier rang fait séparateur. */
  .entete-section {
    position:absolute; left:0; right:0; height:52px;
    display:flex; align-items:flex-end; padding:0 16px 6px;
  }
  /* Le cadre interne porte le centrage : l'auto-marge d'un absolu
     sur-contraint est fragile — un bloc en flux ne l'est pas. */
  .cadre-entete { display:block; width:100%; }
  .espace-entete { flex:none; height:52px; }
  .entete-section .lab, .section-collee .lab {
    font-size:11px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600; white-space:nowrap;
  }
  /* E4 : la colonne centrée de la Réception organisée (~760 px du
     prototype) — rangées et entêtes ensemble, au pixel. */
  /* `width:100%` d'abord : dans une colonne flex, une marge auto en
     travers ÉTEINT le stretch — sans elle, la rangée rétrécit à son
     contenu (constat de capture E4). */
  .centre :global(.ligne) {
    width:100%; max-width:760px; margin-inline:auto; box-sizing:border-box;
  }
  .centre .cadre-entete { max-width:760px; margin-inline:auto; }
  /* E4 : le ⋯ de gestes — place RÉSERVÉE à gauche de l'heure (24 px),
     opacité seule : la géométrie de la rangée ne bouge jamais. */
  .gestes {
    flex:none; width:24px; height:24px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    align-self:center; opacity:0; color:var(--muted);
    background:none; border:1px solid transparent;
  }
  .ligne:hover .gestes, .gestes:focus-visible, .gestes[aria-expanded="true"] {
    opacity:1;
  }
  .gestes:hover, .gestes[aria-expanded="true"] {
    background:var(--hover); border-color:var(--border); color:var(--ink);
  }
  .fenetre {
    position:absolute; top:0; left:0; right:0;
    display:flex; flex-direction:column;
  }
  /* A83 — la cage des sondes. `position:relative` est LA ligne qui
     compte : elle fait de la cage le bloc conteneur des sondes, qui
     sont alors clippées par `height:0; overflow:hidden` et sortent de
     la région défilante du cadre. Sans elle, les sondes se calent sur
     `.cadre` (lui aussi positionné) et lui ajoutent jusqu'à 85 px de
     défilement fantôme sur une fenêtre courte — mesuré au banc
     (spikes/espacement/sondes.mjs, variantes B et C). */
  .sondes-cage { position:relative; height:0; overflow:hidden; }
  .sondes { position:absolute; visibility:hidden; left:0; right:0; }
  .vide {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; padding:40px; text-align:center;
  }
  .vide p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .fenetre-recherche { display:flex; flex-direction:column; }
  .vide-recherche { padding:40px; text-align:center; }
  .vide-recherche p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  /* « Charger plus » : un bouton discret centré sous les résultats (paire
     ink/surface, survol ink/sel — validées par la gate contraste). Au-delà
     de la borne douce, l'invite à affiner le remplace (encre atténuée, comme
     l'état vide). */
  .charger-plus {
    align-self:center; margin:12px 0 20px; height:32px; padding:0 18px;
    display:inline-flex; align-items:center; font-size:13px; font-weight:600;
    color:var(--ink); background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .charger-plus:hover { background:var(--sel); }
  .charger-plus:disabled { opacity:.6; cursor:default; }
  .affiner {
    margin:0; padding:16px 40px 24px; text-align:center;
    font-size:13px; line-height:1.5; color:var(--muted);
  }

  /* Quatre états (A30) : repos transparent, survol en teinte légère,
     sélection en teinte + liseré d'accent de 2 px — jamais d'ombre ni
     de surface blanche (A29). Le liseré est réservé en transparent :
     la sélection ne déplace pas le contenu. */
  /* A81 : la colonne de tête (tuile d'initiales) a quitté la rangée de
     liste — la grille est à UNE colonne, le contenu prend toute la
     largeur. Le rang de puces (A44, terrain : hauteur au contenu)
     n'existe que sur les lignes porteuses ; les DEUX hauteurs sont
     sondées (h1/h2). Le dossier Brouillons garde sa tuile (D9) : la
     classe `tuilee` lui rend la colonne de tête. */
  .ligne {
    /* A83 : l'air vertical vient du cran (--rangee-pad, posé sur le
       cadre) ; 13 px reste le défaut, l'existant au pixel près. Le
       repli couvre les rangées rendues hors du cadre, s'il en naissait
       une. */
    padding:var(--rangee-pad, 13px) 16px; border-top:1px solid var(--border);
    border-left:2px solid transparent;
    display:grid; grid-template-columns:1fr;
    row-gap:3px; align-items:start; cursor:pointer;
    /* R1 : le bloc conteneur de la case (absolue) — sans effet sur la
       géométrie mesurée par les sondes. */
    position:relative;
  }
  .ligne.tuilee { grid-template-columns:auto 1fr; column-gap:10px; }
  .avatar {
    grid-row:1 / span 3; width:28px; height:28px;
    border-radius:var(--r-tuile);
    background:var(--tuile); border:1px solid var(--border);
    display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--tuileInk);
  }
  .l1, .objet, .apercu, .puces { grid-column:1; min-width:0; }
  .tuilee .l1, .tuilee .objet, .tuilee .apercu, .tuilee .puces { grid-column:2; }
  /* Le rang de puces (PLAN-RETOURS-V3 R1) : le gabarit 24 px du
     prototype Classique — présent sur les seules lignes porteuses. */
  .puces {
    height:24px; display:flex; align-items:center; gap:6px;
    overflow:hidden;
  }
  .puce {
    display:inline-flex; align-items:center; gap:5px; height:24px;
    padding:0 9px; font-size:12px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); white-space:nowrap;
  }
  .puce :global(.ic) { width:14px; height:14px; }
  /* R10 : les gestes d'invitation du rang — la puce qui AGIT. */
  button.puce { cursor:pointer; }
  button.puce:hover:not(:disabled) { background:var(--sel); }
  button.puce:disabled { cursor:default; opacity:.55; }
  /* R9 : la couleur dit le sens de la réponse — portée par l'ICÔNE
     (le texte la double, A8), aux jetons du système : accepter en
     accent, refuser en alerte, provisoire neutre. Paires déjà gatées
     (accent/surface 3:1, alert/surface 3:1, et leurs pendants --sel). */
  .puce.ton-accepte :global(.ic) { color:var(--accent); }
  .puce.ton-provisoire :global(.ic) { color:var(--muted); }
  .puce.ton-refuse :global(.ic) { color:var(--alert); }
  .puce.ton-annulee { color:var(--alert); }
  .ligne:hover { background:var(--hover); }
  .ligne.choisie {
    background:var(--sel); border-left-color:var(--accent);
  }
  /* R1 : la rangée COCHÉE prend la teinte de sélection, sans le liseré
     (le liseré reste la position de lecture — deux idées, deux
     dessins). Verdict terrain 2026-08-27 (R1-7) : les ÉPINGLÉES la
     prennent AUSSI — la coche est le seul état qui déloge le sol
     --tuile d'A73, parce qu'elle précède un geste de masse : ce que
     l'œil ne compte pas peut partir par surprise. */
  .ligne.cochee { background:var(--sel); }
  /* R1/D4 — la case : absolue dans la gouttière gauche (padding 16 px
     + liseré réservé 2 px : elle n'entre pas dans la grille, les
     gabarits sondés h1/h2 ne la voient pas). Invisible au repos
     (opacité seule — elle reste cliquable à l'aveugle dans la
     gouttière, et la géométrie ne reflue jamais) ; révélée au survol
     de SA rangée, et sur toutes dès qu'une sélection existe. */
  /* Terrain 2026-08-27 (R1-3) : la case respire — 8 px du bord, 16 px
     de boîte, et le CONTENU s'écarte à 34 px quand la case se montre
     (survol de SA rangée, rangée cochée, ou mode sélection — là,
     toutes les rangées s'écartent d'un bloc, rien ne « saute » pendant
     qu'on coche). Le décalage vit dans le padding : la hauteur des
     rangées ne bouge pas, les sondes h1/h2 restent justes. Le dossier
     Brouillons (.tuilee) n'a pas de case, il ne s'écarte pas. */
  .case {
    position:absolute; left:8px; top:calc(var(--rangee-pad, 13px) + 1px);
    width:16px; height:16px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:var(--bg); border:1px solid var(--border);
    border-radius:var(--r-controle); color:var(--accent);
    cursor:pointer; opacity:0;
  }
  .ligne:hover .case,
  .selection-en-cours .case,
  .case[aria-checked="true"] { opacity:1; }
  .ligne:not(.tuilee):hover,
  .ligne.cochee,
  .selection-en-cours .ligne:not(.tuilee) { padding-left:34px; }
  /* La barre transformée (D3) : mêmes 52 px que le bandeau, boutons
     32 px de la grammaire d'entête. */
  .bandeau-selection { gap:4px; }
  .bandeau-selection h1 { font-size:14px; }
  .btn-barre {
    flex:none; width:32px; height:32px; padding:0;
    display:inline-flex; align-items:center; justify-content:center;
    background:none; border:1px solid transparent;
    border-radius:var(--r-controle); color:var(--ink2); cursor:pointer;
  }
  /* Survol en --sel : le jeton de la grammaire d'entête (.btn-tiroir,
     .btn-statut) — jamais une seconde convention (revue). */
  .btn-barre:hover:not(:disabled) { background:var(--sel); color:var(--ink); }
  .btn-barre:disabled { opacity:.55; cursor:default; }
  /* A73, terrain 2026-08-21 : la ligne ÉPINGLÉE prend le dessin de la
     tuile de la boîte en cours (nav, W2-D5) — fond --tuile, encre
     --tuileInk (paire déjà mesurée par la gate) : elle se distingue au
     premier regard du flot. La teinte tient au survol (la tuile n'a
     pas d'état de survol) ; la sélection garde son liseré d'accent. */
  .epingles .ligne,
  .epingles .ligne:hover,
  .epingles .ligne.choisie { background:var(--tuile); }
  .epingles .ligne.choisie { border-left-color:var(--accent); }
  /* Verdict terrain 2026-08-27 (R1-7) : la COCHE déloge le sol --tuile
     — déclarée APRÈS le bloc ci-dessus pour gagner aussi sur une
     rangée à la fois choisie et cochée (même spécificité, l'ordre
     tranche). */
  .epingles .ligne.cochee { background:var(--sel); }
  .epingles .ligne .exp,
  .epingles .ligne .objet,
  .epingles .ligne .apercu,
  .epingles .ligne .heure { color:var(--tuileInk); }
  /* A73 vaut pour la ligne ENTIÈRE : le bloc de boîte (A80) prend
     l'encre chaude comme ses voisins — sans cette règle il gardait ses
     deux gris froids (--ink2/--muted) sur le sol --tuile, seul îlot
     froid de la rangée (revue). Le tracé, lui, garde la teinte du
     compte : c'est son identité, et sa paire sur --tuile est mesurée. */
  .epingles .ligne :global(.boite),
  .epingles .ligne :global(.boite .mot),
  .epingles .ligne :global(.boite .lib) { color:var(--tuileInk); }
  /* A80 — la ligne d'entête : gap 6 (le bloc de boîte ajoute deux
     gouttières ; à 10 la ligne perdait 12 px pour rien). L'ORDRE DE
     TRONCATURE EST LE DESSIN : l'heure ne cède jamais (flex:none),
     le bloc (.boite, systeme.css) cède trois fois plus vite que
     l'expéditeur, l'essor absorbe le mou. */
  .l1 { display:flex; align-items:baseline; gap:6px; }
  .l1 :global(.disque), .l1 .marque-epingle { align-self:center; }
  .marque-epingle { color:var(--tuileInk); display:inline-flex; }
  .exp {
    font-size:14px; color:var(--ink); flex:0 1 auto; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .essor { flex:1 1 0; min-width:0; }
  .nonlu .exp { font-weight:700; }
  .heure { font-size:12px; color:var(--muted); flex:none; }
  .objet {
    /* 14 px (A29 — amende A9) : le gabarit des pistes. */
    margin:0; font-size:14px; font-weight:400; line-height:1.3;
    color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .nonlu .objet { font-weight:700; }
  .apercu {
    margin:0; font-size:13px; line-height:1.45; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    min-height:1.45em;
  }
  /* La mention « Brouillon : » (variante B, PLAN-BROUILLONS §3) : le
     jeton d'alerte en texte — mesuré par contraste.mjs sur les trois
     fonds de rangée (repos, survol, choisie). */
  .prefixe { color:var(--alert); font-weight:600; }
  /* Champs vides du dossier : l'atténué italique le dit, jamais un
     blanc (« (sans objet) », « (sans destinataire) »). */
  .sans, .objet.sans { font-style:italic; color:var(--muted); font-weight:400; }
  .attente { color:var(--muted); }

  .onglets {
    flex:none; height:52px; padding:0 12px; display:flex;
    align-items:center; gap:10px; border-top:1px solid var(--border);
    background:var(--bg);
  }
  .onglet {
    height:32px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; border-radius:var(--r-controle); cursor:pointer;
    color:var(--ink2); background:var(--surface);
    border:1px solid var(--border);
  }
  .onglet:hover { background:var(--hover); }
  .onglet.actif {
    font-weight:600; color:var(--ink); background:var(--sel);
    border-color:var(--accent);
  }
  .menu-gestes {
    position:fixed; z-index:30; min-width:240px; padding:6px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); box-shadow:0 8px 24px rgba(0,0,0,.14);
    display:flex; flex-direction:column; gap:2px;
  }
  .menu-gestes button {
    display:flex; align-items:center; gap:8px; text-align:left;
    border:1px solid transparent; background:none; height:32px; padding:0 8px;
  }
  .menu-gestes button:hover { background:var(--hover); }
  .filet-menu { border-top:1px solid var(--border); margin:4px 0; }
</style>
