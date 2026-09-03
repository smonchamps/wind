<script>
  // Surimpression Réglages en deux volets (A13) : à gauche le rail des
  // GROUPES (grammaire de la nav de l'écran 02 — rangées 36 px, état
  // actif = surface + bordure accent + ombre), à droite le contenu du
  // groupe choisi. Carte signature élargie à 800 px, en-tête 48 px et
  // pied « Terminé » inchangés. Le prototype est muet sur cette
  // surface : le Système complète (A6), l'écart s'inscrit au journal.
  //
  // Règle : un groupe ne s'expédie qu'avec du contenu RÉEL — aucun
  // réglage inventé pour meubler, aucun groupe vide.
  import Icone from './Icone.svelte';
  import Menu from './Menu.svelte';
  import Marque from './Marque.svelte';
  import DrapeauUE from './DrapeauUE.svelte';
  import { tick } from 'svelte';
  import {
    FICHES, appliquerTheme, themeAffiche, suiviOs, appliquerSuiviOs,
  } from './lib/theme.js';
  import { t, LANGUES, langueActuelle, appliquerLangue } from './lib/texte.svelte.js';
  import { voletsActuels, appliquerVolets } from './lib/volets.svelte.js';
  import {
    espacementActuel, appliquerEspacement, NIVEAUX,
  } from './lib/espacement.svelte.js';
  import { activation } from './lib/clavier.js';
  import { appel } from './lib/transport.js';
  import { LIBELLE_ECARTE, LIBELLE_DESTINATION } from './lib/portier.js';
  import { REPERE_ICONES, REPERE_TEINTES } from './lib/reperes.js';
  import { HORIZONS_IMPORT as HORIZONS } from './lib/vocabulaires.js';
  import GuichetCompte from './GuichetCompte.svelte';

  // A11 — la section « Comptes » : v1 offrait l'ajout à tout moment,
  // l'écran 01 ne vient qu'à zéro compte ; la porte permanente vit ici.
  // Le retrait vit sur la même rangée : `onsupprime(id)` remonte à
  // l'App, qui recharge nav et liste.
  let {
    comptes = [],
    // Les adresses qui tiennent une session (App.connecter) : un compte
    // du registre absent d'ici a un jeton mort — il se répare sur place.
    connectes = [],
    // R1 (PLAN-RETOURS-8) : les repères posés (App les charge) ; poser
    // ou retirer remonte par `onrepere(id, repere|null)` — l'App patche
    // sa table sur place (revue : jamais un rechargement complet par
    // clic de teinte).
    reperes = {},
    onrepere = () => {},
    // PLAN-RETOURS-9 (D3/D4) : les noms personnalisés (App les charge) ;
    // poser ou vider remonte par `onnom(id, nom|null)` — même régime
    // que le repère.
    noms = {},
    onnom = () => {},
    onajoute = () => {},
    onsupprime = () => {},
    onreconnecte = () => {},
    // RETOURS-14 R5 (revue) : la réintégration parle et se propage —
    // le contrat du même geste à la page Portier (toast + resservie
    // des vues par l'App).
    onflash = () => {},
    onroutage = () => {},
  } = $props();

  const GROUPES = [
    { id: 'comptes', icon: 'person', libelle: 'groupe.comptes' },
    { id: 'themes', icon: 'bookmark', libelle: 'groupe.themes' },
    { id: 'affichage', icon: 'display_settings', libelle: 'groupe.affichage' },
    { id: 'notifications', icon: 'notifications', libelle: 'groupe.notifications' },
    // RETOURS-13 R9, terrain C4 : les défauts du Portier — le groupe
    // reste visible QUEL QUE SOIT le mode (verdict CE du terrain, qui
    // renverse le choix « organisé seul » de la première passe).
    { id: 'screener', icon: 'screener', libelle: 'groupe.portier' },
    // R1 (PLAN-RETOURS-6) : le gestionnaire de signature — du contenu
    // réel (un éditeur par compte), la règle des groupes est tenue.
    { id: 'signature', icon: 'signature', libelle: 'groupe.signature' },
    { id: 'raccourcis', icon: 'keyboard', libelle: 'groupe.raccourcis' },
    { id: 'apropos', icon: 'info', libelle: 'groupe.apropos' },
  ];

  // La table D3, en RÉFÉRENCE seulement — pas de re-mappage. Touches et
  // gestes au catalogue (`raccourci.touche.*` / `raccourci.geste.*`) :
  // « Suppr » / « Échap » deviennent "Del" / "Esc", les GESTES seuls se
  // traduisent — les touches c/r/f/e ne bougent pas d'une langue à
  // l'autre (A15).
  const RACCOURCIS = ['c', 'r', 'f', 'e', 'suppr', 'slash', 'echap'];

  let visible = $state(false);
  let panneau = $state(null);
  let groupe = $state('comptes');
  // La coche suit la fiche AFFICHÉE, pas le choix persisté (revue
  // A42) : sous suivi OS + OS sombre, l'écran est en -nuit — la coche
  // aussi, sinon l'utilisateur « corrige » en cliquant la fiche -nuit
  // et s'enferme dans le sombre permanent. Le signal
  // `wind:theme-affiche` la garde alignée, y compris quand l'OS
  // bascule pendant que le dialogue est ouvert.
  let active = $state(themeAffiche());
  let ajoutOuvert = $state(false);
  $effect(() => {
    if (!visible) return;
    const suivre = () => (active = themeAffiche());
    document.addEventListener('wind:theme-affiche', suivre);
    return () => document.removeEventListener('wind:theme-affiche', suivre);
  });

  // Retrait d'un compte : le geste est DESTRUCTEUR localement (courrier
  // local effacé, connexion oubliée — le serveur, lui, n'est jamais
  // touché), donc il se confirme sur place, dans une carte sous la
  // rangée. `retrait` porte l'account_id en attente de confirmation.
  let retrait = $state(null);
  let retraitOccupe = $state(false);
  let retraitErreur = $state(null);

  // À propos : la version se lit UNE fois (elle ne change pas en cours
  // de session) ; hors Tauri le rejet laisse le tiret — jamais un vide
  // silencieux qui ressemblerait à un oubli.
  let version = $state('');
  // null (repos) | 'controle' | 'ajour' | {version} | {erreur}
  let maj = $state(null);

  // Affichage (D6) : le suivi de l'OS sombre, un booléen localStorage
  // comme le thème. Notifications (R-D2) : les bulles d'arrivée, une
  // préférence EN BASE — c'est le shell Rust qui émet. Langue (A15) :
  // en base aussi, même raison — le shell compose les bulles dans
  // cette langue.
  let auto = $state(suiviOs());
  let bulles = $state(true);
  let langue = $state(langueActuelle());
  // Disposition (PLAN-VOLETS, V-D4) : le nombre de volets, un
  // localStorage comme le thème — application immédiate, le geste du
  // thème ; rien à faire échouer, donc rien à faire revenir.
  let volets = $state(voletsActuels());
  let espacement = $state(espacementActuel());
  // R1 (RETOURS-11, D4) : les règles « toujours afficher les images de
  // cet expéditeur » — lues du cœur à chaque ouverture, retirées sur
  // place. La porte de sortie du « toujours ».
  let expediteursImages = $state([]);
  // RETOURS-13 R9 : les défauts des boutons du Portier — lus du cœur
  // quand le groupe s'ouvre. `null` tant que la base n'a pas répondu :
  // les sélecteurs ne se peignent qu'avec l'état PERSISTÉ (revue — un
  // clic avant la réponse aurait réécrit l'autre défaut avec la valeur
  // livrée, pas la sienne). Sur échec d'écriture, l'interface ne ment
  // pas : elle revient à l'état réellement persisté.
  let portierDefauts = $state(null);
  // RETOURS-14 R5 (D6) : la liste EXHAUSTIVE des décisions du Portier
  // — toutes les destinations (l'historique de la page Portier ne
  // montre que les écartés), à l'alphabet, filtrable, réintégrable.
  // `null` tant que la base n'a pas répondu : le vide ne s'affirme
  // jamais sans preuve.
  let routagesListe = $state(null);
  let filtreRoutages = $state('');
  $effect(() => {
    if (visible && groupe === 'screener') {
      appel('screener_defaults_get')
        .then((d) => (portierDefauts = d))
        .catch((err) => console.error('screener_defaults_get :', err));
      filtreRoutages = '';
      appel('routings')
        .then((r) => (routagesListe = r))
        .catch((err) => console.error('routings :', err));
    }
  });
  const routagesVisibles = $derived.by(() => {
    if (!routagesListe) return null;
    const filtre = filtreRoutages.trim().toLowerCase();
    return routagesListe
      .filter((r) => !filtre || r.address.toLowerCase().includes(filtre))
      .slice()
      .sort((a, b) => a.address.localeCompare(b.address, langueActuelle(), { sensitivity: 'base' }));
  });
  // Le vocabulaire des verdicts : UNE copie (lib/portier.js, partagée
  // avec la page Portier), jamais des textes recopiés.
  const libelleRoutage = (r) =>
    r.destination === 'screened_out'
      ? t(LIBELLE_ECARTE[r.rule] ?? 'portier.ecarte')
      : t(LIBELLE_DESTINATION[r.destination] ?? r.destination);
  // RETOURS-14 R10 (terrain 2026-08-31) : « Réintégrer » devient
  // « Modifier » — le menu repropose TOUTES les règles du Portier
  // (les Oui, les règles du Non) plus « Renvoyer au portier »
  // (l'ancien Réintégrer). Même contrat que la page Portier : le
  // toast dit ce qui vient d'arriver, `onroutage` fait resservir les
  // vues et la nav par l'App, l'échec se DIT (jamais un silence).
  let menuDecision = $state(null);
  function ouvrirModifier(e, r) {
    e.stopPropagation();
    const rect = e.currentTarget.getBoundingClientRect();
    menuDecision = {
      address: r.address,
      x: rect.left,
      y: rect.bottom + 4,
    };
  }
  const TOAST_NON = {
    spam: 'toast.portierNonSpam',
    archive: 'toast.portierNonArchive',
    trash: 'toast.portierNonCorbeille',
  };
  const BOITE_DE = {
    inbox: 'portier.laReception',
    feed: 'portier.leKiosque',
    paper_trail: 'portier.leRegistre',
  };
  async function modifierRoutage(destination, rule = null) {
    const { address } = menuDecision;
    menuDecision = null;
    try {
      await appel('route_sender', { address, destination, rule });
      routagesListe = routagesListe.map((r) =>
        r.address === address ? { ...r, destination, rule } : r);
      if (destination === 'screened_out') {
        onflash(t(rule ? TOAST_NON[rule] : 'toast.portierNonNu', { qui: address }));
      } else if (destination === 'inbox') {
        onflash(t('toast.portierOuiNu', { qui: address }));
      } else {
        onflash(t('toast.portierOuiVers', { qui: address, mailbox: t(BOITE_DE[destination]) }));
      }
      onroutage();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }
  async function renvoyerAuPortier() {
    const { address } = menuDecision;
    menuDecision = null;
    try {
      await appel('remove_routing', { address });
      routagesListe = routagesListe.filter((r) => r.address !== address);
      onflash(t('toast.portierReintegre', { qui: address }));
      onroutage();
    } catch (err) {
      onflash(t('erreur.preference', { err }));
    }
  }
  function changerPortier(champ, value) {
    if (!portierDefauts) return;
    const avant = { ...portierDefauts };
    portierDefauts = { ...portierDefauts, [champ]: value };
    appel('screener_defaults_set', {
      yes: portierDefauts.yes,
      no: portierDefauts.no,
    }).catch(() => {
      portierDefauts = avant;
    });
  }

  // ADR 0029 (D3) : l'horizon d'import par compte — lu du cœur à
  // L'OUVERTURE (revue 2026-08-30 : un $effect sur `comptes` re-tirait
  // toutes les 10 s au rythme de chargerNav, et une lecture tardive
  // pouvait écraser un choix optimiste en vol) ; le sélecteur ne se
  // peint qu'avec l'état PERSISTÉ (patron portierDefauts). Sur échec
  // d'écriture, retour à l'état réellement persisté — l'interface ne
  // ment pas.
  let horizons = $state({});
  let horizonOuvert = $state(null);
  let horizonErreur = $state(null);
  function chargerHorizons() {
    for (const c of comptes) {
      appel('horizon_import_get', { accountId: c.account_id })
        .then((v) => (horizons[c.account_id] = v))
        .catch((err) => console.error('horizon_import_get :', err));
    }
  }
  function ouvrirHorizon(id) {
    const rouvre = horizonOuvert !== id;
    fermerCartes();
    if (rouvre) horizonOuvert = id;
  }
  function changerHorizon(id, value) {
    const avant = horizons[id];
    horizons[id] = value;
    horizonErreur = null;
    appel('horizon_import_set', { accountId: id, value }).catch((err) => {
      horizons[id] = avant;
      horizonErreur = t('reglages.horizonImpossible', { err });
    });
  }

  // « Jamais deux cartes sous la même rangée » (revue 2026-08-22) : LE
  // point unique — la prochaine carte s'ajoute ici, pas dans N sites
  // (revue 2026-08-23 : l'invariant vivait copié en cinq endroits).
  function fermerCartes() {
    retrait = null;
    retraitErreur = null;
    repereOuvert = null;
    repereErreur = null;
    nomOuvert = null;
    nomErreur = null;
    horizonOuvert = null;
    horizonErreur = null;
  }

  export function ouvrir() {
    active = themeAffiche();
    auto = suiviOs();
    langue = langueActuelle();
    volets = voletsActuels();
    espacement = espacementActuel();
    ajoutOuvert = false;
    fermerCartes();
    // Remise à zéro AVANT le rechargement (patron expediteursImages) :
    // les portes se peignent depuis la BASE, jamais depuis la mémoire
    // de l'ouverture précédente — un choix qui n'a pas persisté ne doit
    // pas se montrer persisté (filet prouvé vacant sans cette ligne).
    horizons = {};
    chargerHorizons();
    reconnexion = null;
    reconnexionErreur = null;
    groupe = 'comptes';
    maj = null;
    visible = true;
    if (!version) {
      appel('app_version')
        .then((v) => (version = v))
        .catch(() => (version = '—'));
    }
    appel('notif_pref_get')
      .then((v) => (bulles = v))
      .catch(() => { /* hors Tauri : le défaut (activées) reste affiché */ });
    // Remise à zéro AVANT le rechargement : sur échec, montrer la liste
    // de l'ouverture précédente serait un mensonge (une règle révoquée
    // ailleurs paraîtrait encore vivante) ; et l'échec se dit (§9 —
    // jamais avalé).
    expediteursImages = [];
    appel('images_senders')
      .then((liste) => (expediteursImages = liste))
      .catch((err) => console.error('images_senders :', err));
    // PLAN-AUDIT-V2 E11 (entrée de D-4) : le focus entre AVEC le
    // panneau — le premier contrôle du rail, comme `Retour.svelte`.
    queueMicrotask(() => panneau?.querySelector('button, input, select, [tabindex]')?.focus());
  }

  async function retirerExpediteurImages(adresse) {
    try {
      await appel('revoke_images_sender', { address: adresse });
      expediteursImages = expediteursImages.filter((a) => a !== adresse);
    } catch (err) {
      console.error('revoke_images_sender :', err);
    }
  }
  export function fermer() {
    visible = false;
  }
  export function estOuverte() {
    return visible;
  }
  function choisirGroupe(id) {
    groupe = id;
    ajoutOuvert = false;
    fermerCartes();
  }
  function demanderRetrait(id) {
    const rouvre = retrait !== id;
    fermerCartes();
    if (rouvre) retrait = id;
  }
  // R1 — le repère : la carte de choix s'ouvre sous la rangée (le
  // patron du retrait). Un repère n'existe qu'ENTIER (icône + teinte,
  // l'allowlist Rust fait foi) : le premier choix attend son jumeau,
  // ensuite chaque clic applique immédiatement — le geste du thème.
  let repereOuvert = $state(null);
  let repereChoix = $state({ icon: null, hue: null });
  let repereErreur = $state(null);
  function ouvrirRepere(id) {
    const rouvre = repereOuvert !== id;
    fermerCartes();
    if (!rouvre) return;
    repereOuvert = id;
    const r = reperes[id];
    repereChoix = { icon: r?.icon ?? null, hue: r?.hue ?? null };
  }
  async function choisirRepere(id, champ, value) {
    repereChoix = { ...repereChoix, [champ]: value };
    if (!repereChoix.icon || !repereChoix.hue) return;
    repereErreur = null;
    try {
      await appel('marker_set', {
        accountId: id,
        icon: repereChoix.icon,
        hue: repereChoix.hue,
      });
      onrepere(id, { icon: repereChoix.icon, hue: repereChoix.hue });
    } catch (err) {
      // La base n'a pas pris le choix : l'erreur se dit sur place et le
      // geste se rejoue — la pastille de la rangée, elle, ne ment pas
      // (elle suit `reperes`, l'état réellement persisté).
      repereErreur = t('reglages.repereImpossible', { err });
    }
  }
  async function retirerRepere(id) {
    repereErreur = null;
    try {
      await appel('marker_set', { accountId: id, icon: null, hue: null });
      repereChoix = { icon: null, hue: null };
      onrepere(id, null);
    } catch (err) {
      repereErreur = t('reglages.repereImpossible', { err });
    }
  }
  // PLAN-RETOURS-9 (D3) : le nom personnalisé — la carte s'ouvre par le
  // LIBELLÉ de la rangée (l'identité est la porte de son nom ; pas de
  // glyphe neuf : le jeu n'a pas de crayon, A3 interdit d'en réemployer
  // un). Vider le champ retire le nom ; le shell normalise et fait foi.
  let nomOuvert = $state(null);
  let nomBrouillon = $state('');
  let nomErreur = $state(null);
  let nomOccupe = $state(false);
  function ouvrirNom(id) {
    const rouvre = nomOuvert !== id;
    fermerCartes();
    if (!rouvre) return;
    nomOuvert = id;
    nomBrouillon = noms[id] ?? '';
  }
  async function enregistrerNom(id) {
    // Un seul vol à la fois : Entrée et le bouton passent par la même
    // porte (revue 2026-08-23 — le disabled du bouton ne gardait pas
    // le chemin clavier).
    if (nomOccupe) return;
    nomOccupe = true;
    nomErreur = null;
    try {
      const name = await appel('name_set', { accountId: id, name: nomBrouillon });
      onnom(id, name ?? null);
      // Ne fermer QUE sa propre carte : une réponse tardive ne doit
      // jamais claquer celle qu'un autre compte vient d'ouvrir.
      if (nomOuvert === id) nomOuvert = null;
    } catch (err) {
      // La base n'a pas pris le nom : l'erreur se dit sur place et le
      // geste se rejoue — le libellé de la rangée suit `noms`, l'état
      // réellement persisté.
      nomErreur = t('reglages.nomImpossible', { err });
    } finally {
      nomOccupe = false;
    }
  }
  // Reconnexion d'un compte au jeton mort (constat terrain 2026-08-20) :
  // le consentement navigateur se rejoue depuis la rangée — l'échec se
  // dit SUR PLACE et le geste se rejoue, comme le retrait.
  const estDeconnecte = (c) => !connectes.includes(c.email);
  let reconnexion = $state(null);
  let reconnexionErreur = $state(null);
  async function reconnecter(c) {
    reconnexion = c.account_id;
    reconnexionErreur = null;
    try {
      await appel('reconnect_account', { accountId: c.account_id });
      onreconnecte();
    } catch (err) {
      reconnexionErreur = { id: c.account_id, texte: t('reglages.reconnexionImpossible', { err }) };
    } finally {
      reconnexion = null;
    }
  }
  async function confirmerRetrait() {
    const id = retrait;
    retraitOccupe = true;
    retraitErreur = null;
    try {
      await appel('remove_account', { accountId: id });
      retrait = null;
      onsupprime(id);
    } catch (err) {
      // Le compte est toujours listé : l'erreur se dit sur place et le
      // geste se rejoue — le retrait est répétable côté shell.
      retraitErreur = t('reglages.retraitImpossible', { err });
    } finally {
      retraitOccupe = false;
    }
  }
  function choisir(id) {
    appliquerTheme(id);
    // Jamais `actif = id` : appliquerTheme refuse en silence un id
    // inconnu, et sous suivi OS le thème posé peut être `id-nuit` —
    // la fiche affichée fait foi dans les deux cas.
    active = themeAffiche();
  }
  function basculerAuto() {
    auto = !auto;
    appliquerSuiviOs(auto);
  }
  function basculerBulles() {
    bulles = !bulles;
    const voulu = bulles;
    appel('notif_pref_set', { enabled: voulu }).catch(() => {
      // La base n'a pas pris le choix : l'interrupteur ne doit pas
      // mentir — il revient à l'état réellement persisté.
      if (bulles === voulu) bulles = !voulu;
    });
  }
  function changerVolets(n) {
    appliquerVolets(n);
    volets = voletsActuels();
  }
  function changerEspacement(niveau) {
    appliquerEspacement(niveau);
    espacement = espacementActuel();
  }
  function changerLangue(code) {
    const avant = langueActuelle();
    if (code === avant) return;
    // Application immédiate (le geste du thème), persistance en base ;
    // si la base n'a pas pris le choix, l'interface ne ment pas — elle
    // revient à la langue réellement persistée.
    appliquerLangue(code);
    langue = code;
    appel('lang_set', { lang: code }).catch(() => {
      appliquerLangue(avant);
      langue = avant;
    });
  }

  // R1 (PLAN-RETOURS-6, D3/D4) : la signature par compte. Un éditeur
  // riche RÉDUIT (gras/italique/souligné — le vocabulaire passe la même
  // frontière ammonia que le composeur, à l'enregistrement côté Rust)
  // et la PORTÉE (aussi dans les réponses/transferts ?) — un choix par
  // compte, applicable à tous d'un geste (D4, mot pour mot).
  let signatures = $state({}); // account_id -> { replies, etat }
  let champsSignature = {}; // account_id -> contenteditable (hors réactivité)
  async function chargerSignatures() {
    for (const c of comptes) {
      try {
        const lue = await appel('signature_get', { accountId: c.account_id });
        signatures[c.account_id] = { replies: lue.replies, etat: null };
        // Le nœud n'existe qu'une fois le groupe rendu — poser avant
        // serait perdu (même raison que `poserCorps` au composeur).
        await tick();
        const champ = champsSignature[c.account_id];
        if (champ) champ.innerHTML = lue.html ?? '';
      } catch (err) {
        console.error('signature_get :', err);
      }
    }
  }
  $effect(() => {
    if (visible && groupe === 'signature') chargerSignatures();
  });
  // `styleWithCSS` éteint, comme au composeur : la sortie reste le
  // vocabulaire exact de l'allowlist (b/i/u), jamais du style généré.
  function commandeSignature(name) {
    document.execCommand('styleWithCSS', false, false);
    document.execCommand(name, false, null);
  }
  async function enregistrerSignature(c, { replies = null, etat = 'ok' } = {}) {
    const sig = signatures[c.account_id] ?? { replies: false };
    const voulu = replies ?? sig.replies;
    try {
      await appel('signature_set', {
        accountId: c.account_id,
        html: champsSignature[c.account_id]?.innerHTML ?? '',
        replies: voulu,
      });
      signatures[c.account_id] = { replies: voulu, etat };
      return true;
    } catch (err) {
      signatures[c.account_id] = {
        ...sig,
        etat: { erreur: t('erreur.signature', { err }) },
      };
      return false;
    }
  }
  function effacerSignature(c) {
    const champ = champsSignature[c.account_id];
    if (champ) champ.innerHTML = '';
    enregistrerSignature(c);
  }
  // La bascule ENREGISTRE (le choix s'applique tout de suite, comme les
  // autres interrupteurs des Réglages) — et emporte le texte courant de
  // l'éditeur : un seul chemin d'écriture.
  function basculerRepliques(c) {
    const sig = signatures[c.account_id] ?? { replies: false };
    enregistrerSignature(c, { replies: !sig.replies });
  }
  // D4, précisé au terrain (2026-08-21) : « appliquer à tous les
  // comptes » copie la SIGNATURE ET la PORTÉE de ce compte chez tous
  // les autres — et ça se VOIT : leurs éditeurs et leurs interrupteurs
  // se mettent à jour à l'écran, pas seulement en base.
  async function appliquerATous(c) {
    const html = champsSignature[c.account_id]?.innerHTML ?? '';
    const voulu = signatures[c.account_id]?.replies ?? false;
    await enregistrerSignature(c, { replies: voulu, etat: 'tous' });
    for (const autre of comptes) {
      if (autre.account_id === c.account_id) continue;
      try {
        await appel('signature_set', {
          accountId: autre.account_id,
          html,
          replies: voulu,
        });
        const champ = champsSignature[autre.account_id];
        if (champ) champ.innerHTML = html;
        signatures[autre.account_id] = { replies: voulu, etat: null };
      } catch (err) {
        signatures[autre.account_id] = {
          ...(signatures[autre.account_id] ?? { replies: false }),
          etat: { erreur: t('erreur.signature', { err }) },
        };
      }
    }
  }

  // Le même flux que la fente d'avis (ADR 0013) : update_check en
  // silence, update_install ne rend pas la main en cas de succès.
  async function verifierMaj() {
    maj = 'controle';
    try {
      const info = await appel('update_check');
      maj = info ? { version: info.version } : 'ajour';
    } catch (err) {
      maj = { erreur: String(err) };
    }
  }
  async function installerMaj() {
    const version = maj.version;
    maj = 'installation';
    try {
      // Ne rend pas la main en cas de succès ; la version part avec —
      // on n'installe que ce qui a été annoncé.
      await appel('update_install', { version });
    } catch (err) {
      // Le lancement a échoué : la mise à jour reste disponible — on la
      // repropose avec l'erreur dite, jamais un cul-de-sac (revue
      // PLAN-SIGNATURE). { erreur } seul reste l'échec du CONTRÔLE.
      maj = { version, erreur: String(err) };
    }
  }
</script>

{#if visible}
  <div class="scrim" data-testid="reglages-modal" bind:this={panneau}>
    <div class="carte" role="dialog" aria-modal="true" aria-label={t('entete.reglages')}>
      <div class="tete">
        <span class="titre">{t('entete.reglages')}</span>
        <button type="button" class="fermer" aria-label={t('action.fermer')} onclick={fermer}>
          <Icone name="close" /></button>
      </div>
      <div class="milieu">
        <div class="rail" role="group" aria-label={t('reglages.groupesAria')}>
          {#each GROUPES as g (g.id)}
            <div class="rang" class:actif={groupe === g.id}
                 data-testid="reglages-groupe" data-groupe={g.id}
                 role="button" tabindex="0" aria-current={groupe === g.id}
                 onclick={() => choisirGroupe(g.id)}
                 onkeydown={activation(() => choisirGroupe(g.id))}>
              <span class="icone" aria-hidden="true"><Icone name={g.icon} /></span>
              <span class="libelle">{t(g.libelle)}</span>
            </div>
          {/each}
        </div>
        <div class="volet" data-testid="reglages-volet">
          {#if groupe === 'comptes'}
            <p class="section">{t('groupe.comptes')}</p>
            <div class="rangees" data-testid="reglages-comptes">
              {#each comptes as c (c.account_id)}
                <div class="compte">
                  <!-- A74 : l'icône de la rangée devient LA porte du
                       repère — elle montre l'état persisté (pastille ou
                       `person` neutre) et ouvre la carte de choix. -->
                  <button type="button" class="btn-repere" data-testid="compte-repere"
                          aria-expanded={repereOuvert === c.account_id}
                          aria-label={t('reglages.repereCompte', { email: c.email })}
                          onclick={() => ouvrirRepere(c.account_id)}>
                    {#if reperes[c.account_id]}
                      <span class="repere p20"
                            data-teinte={reperes[c.account_id].hue}
                            aria-hidden="true"><Icone name={reperes[c.account_id].icon} /></span>
                    {:else}
                      <Icone name="person" />
                    {/if}
                  </button>
                  <!-- PLAN-RETOURS-9 (D3/D4) : le libellé est la PORTE
                       du name personnalisé — en Réglages le name s'affiche
                       AVEC l'adresse (elle reste la vérité de connexion). -->
                  <button type="button" class="identite" data-testid="compte-nommer"
                          aria-expanded={nomOuvert === c.account_id}
                          aria-label={t('reglages.nommerCompte', { email: c.email })}
                          onclick={() => ouvrirNom(c.account_id)}>
                    {#if noms[c.account_id]}
                      <span class="nom-compte" data-testid="compte-nom">{noms[c.account_id]}</span>
                    {/if}
                    <span class="adresse" class:sous-nom={noms[c.account_id]}>{c.email}</span>
                  </button>
                  <!-- ADR 0029 (D3) : la porte de l'horizon d'import —
                       la VALEUR est la porte (pas de glyphe neuf, A3),
                       la carte s'ouvre sous la rangée. -->
                  <button type="button" class="btn-horizon" data-testid="compte-horizon"
                          aria-expanded={horizonOuvert === c.account_id}
                          aria-label={t('reglages.horizonCompte', { email: c.email })}
                          onclick={() => ouvrirHorizon(c.account_id)}>
                    {horizons[c.account_id] ? t(`horizon.${horizons[c.account_id]}`) : '…'}</button>
                  {#if estDeconnecte(c)}
                    <!-- Jeton mort : l'état se DIT (link_off, le glyphe de
                         la reconnexion — même sens qu'à la fente d'avis)
                         et se répare sur place. -->
                    <span class="deconnecte" data-testid="compte-deconnecte">
                      <Icone name="link_off" />{t('reglages.deconnecte')}</span>
                    <button type="button" class="reconnecter" data-testid="compte-reconnecter"
                            disabled={reconnexion === c.account_id}
                            aria-label={t('reglages.reconnecterCompte', { email: c.email })}
                            onclick={() => reconnecter(c)}>
                      {reconnexion === c.account_id
                        ? t('reglages.reconnexionEnCours')
                        : t('reglages.reconnecter')}</button>
                  {/if}
                  <!-- PLAN-RETOURS-9 (D2) : le geste se DIT — icône +
                       texte, dans le vocabulaire du produit (« retirer »,
                       rien n'est supprimé du serveur). -->
                  <button type="button" class="retirer" data-testid="compte-retirer"
                          aria-label={t('reglages.retirerCompte', { email: c.email })}
                          onclick={() => demanderRetrait(c.account_id)}>
                    <Icone name="delete" />{t('reglages.retirer')}</button>
                </div>
                {#if reconnexionErreur?.id === c.account_id}
                  <p class="erreur-reconnexion" data-testid="reconnexion-erreur">
                    {reconnexionErreur.texte}</p>
                {/if}
                {#if repereOuvert === c.account_id}
                  <!-- A74 : la carte du repère, sous la rangée (le
                       patron du retrait). Icônes puis teintes ; le
                       premier choix attend son jumeau, ensuite chaque
                       clic applique immédiatement (le geste du thème). -->
                  <div class="carte-repere" data-testid="reglages-repere">
                    <p class="titre-repere">{t('reglages.repereTitre')}</p>
                    <div class="choix-repere" role="group" aria-label={t('reglages.repereIcones')}>
                      {#each REPERE_ICONES as ic (ic)}
                        <button type="button" class="choix" class:choisi={repereChoix.icon === ic}
                                data-testid="repere-icone" data-icone={ic}
                                aria-pressed={repereChoix.icon === ic}
                                title={t(`repere.icone.${ic}`)}
                                aria-label={t(`repere.icone.${ic}`)}
                                onclick={() => choisirRepere(c.account_id, 'icon', ic)}>
                          <Icone name={ic} /></button>
                      {/each}
                    </div>
                    <div class="choix-repere" role="group" aria-label={t('reglages.repereTeintes')}>
                      {#each REPERE_TEINTES as te (te)}
                        <button type="button" class="choix" class:choisi={repereChoix.hue === te}
                                data-testid="repere-teinte" data-couleur={te}
                                aria-pressed={repereChoix.hue === te}
                                title={t(`repere.teinte.${te}`)}
                                aria-label={t(`repere.teinte.${te}`)}
                                onclick={() => choisirRepere(c.account_id, 'hue', te)}>
                          <span class="repere pastille-teinte" data-teinte={te}
                                aria-hidden="true"></span></button>
                      {/each}
                    </div>
                    {#if repereErreur}
                      <p class="erreur-repere" data-testid="repere-erreur">{repereErreur}</p>
                    {/if}
                    {#if reperes[c.account_id]}
                      <button type="button" class="ajouter" data-testid="repere-retirer"
                              onclick={() => retirerRepere(c.account_id)}>
                        {t('reglages.repereRetirer')}</button>
                    {/if}
                  </div>
                {/if}
                {#if nomOuvert === c.account_id}
                  <!-- La carte du nom, sous la rangée (le patron du
                       retrait). Vider le champ retire le name ; Entrée
                       enregistre. -->
                  <div class="carte-nom" data-testid="reglages-nom">
                    <p class="titre-repere">{t('reglages.nomTitre')}</p>
                    <!-- Pas de maxlength : « jamais tronqué en silence »
                         (contrat D3) — un name trop long se REFUSE avec
                         son erreur, par le shell. -->
                    <input type="text" class="champ-nom"
                           data-testid="nom-champ" bind:value={nomBrouillon}
                           placeholder={c.email}
                           aria-label={t('reglages.nomTitre')}
                           onkeydown={(e) => { if (e.key === 'Enter') enregistrerNom(c.account_id); }}>
                    {#if nomErreur}
                      <p class="erreur-repere" data-testid="nom-erreur">{nomErreur}</p>
                    {/if}
                    <div class="boutons-retrait">
                      <button type="button" class="ajouter" data-testid="nom-enregistrer"
                              disabled={nomOccupe} onclick={() => enregistrerNom(c.account_id)}>
                        {t('action.enregistrer')}</button>
                      <button type="button" class="ajouter" data-testid="nom-annuler"
                              onclick={() => (nomOuvert = null)}>
                        {t('action.annuler')}</button>
                    </div>
                  </div>
                {/if}
                {#if horizonOuvert === c.account_id}
                  <!-- La carte de l'horizon, sous la rangée (patron du
                       name). Application immédiate — le geste du thème ;
                       la note dit ce qu'étendre et réduire FONT. -->
                  <div class="carte-nom" data-testid="reglages-horizon">
                    <p class="titre-repere">{t('reglages.horizonTitre')}</p>
                    {#if horizons[c.account_id]}
                      <select class="select-horizon" data-testid="horizon-select"
                              value={horizons[c.account_id]}
                              onchange={(e) => changerHorizon(c.account_id, e.currentTarget.value)}>
                        {#each HORIZONS as h (h)}
                          <option value={h}>{t(`horizon.${h}`)}</option>
                        {/each}
                      </select>
                    {/if}
                    <p class="note-horizon">{t('reglages.horizonNote')}</p>
                    {#if horizonErreur}
                      <p class="erreur-repere" data-testid="horizon-erreur">{horizonErreur}</p>
                    {/if}
                  </div>
                {/if}
                {#if retrait === c.account_id}
                  <!-- La confirmation vit SOUS la rangée, dans la carte
                       signature : un geste destructeur ne part jamais du
                       premier clic, et il dit ce qu'il efface — et ce
                       qu'il n'efface pas (le serveur). -->
                  <div class="carte-retrait" data-testid="reglages-retrait">
                    <p class="avertissement">{t('reglages.retirerConfirme', { email: c.email })}</p>
                    {#if retraitErreur}
                      <p class="erreur-retrait" data-testid="retrait-erreur">{retraitErreur}</p>
                    {/if}
                    <div class="boutons-retrait">
                      <button type="button" class="danger" data-testid="retrait-confirmer"
                              disabled={retraitOccupe} onclick={confirmerRetrait}>
                        {retraitOccupe ? t('reglages.retraitEnCours') : t('action.retirer')}</button>
                      <button type="button" class="ajouter" data-testid="retrait-annuler"
                              onclick={() => demanderRetrait(c.account_id)}>
                        {t('action.annuler')}</button>
                    </div>
                  </div>
                {/if}
              {/each}
              {#if ajoutOuvert}
                <!-- Carte signature : le guichet est un BLOC voulu, pas un
                     formulaire qui flotte (verdict terrain). Démonté au repli
                     ou au succès : il repart toujours propre. -->
                <div class="carte-ajout" data-testid="reglages-guichet">
                  <div class="tete-ajout">
                    <span class="titre-ajout">{t('reglages.ajouterCompte')}</span>
                    <button type="button" class="fermer" aria-label={t('action.replier')}
                            onclick={() => (ajoutOuvert = false)}>
                      <Icone name="close" /></button>
                  </div>
                  <GuichetCompte compact onajoute={() => { ajoutOuvert = false; onajoute(); }} />
                </div>
              {:else}
                <button type="button" class="ajouter" data-testid="reglages-ajouter"
                        onclick={() => (ajoutOuvert = true)}>
                  <Icone name="person_add" />{t('reglages.ajouterCompte')}</button>
              {/if}
            </div>
          {:else if groupe === 'themes'}
            <p class="section">{t('reglages.sectionThemes')}</p>
            <div class="rangees">
              <!-- R1 (PLAN-RETOURS-13) : le suivi de l'OS sombre vit en
                   TÊTE des Thèmes — il gouverne le thème affiché, pas
                   l'affichage. Le testid historique reste (deux specs
                   et la doc le portent). -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.sombreAuto')}</span>
                  <span class="desc">{t('reglages.sombreAutoDesc')}</span>
                </span>
                <button type="button" class="bascule" role="switch"
                        aria-checked={auto} aria-label={t('reglages.sombreAuto')}
                        data-testid="affichage-auto" onclick={basculerAuto}>
                  <span class="bille"></span>
                </button>
              </div>
              {#each FICHES as fiche (fiche.id)}
                <div class="rangee" class:active={active === fiche.id}
                     data-testid="theme" data-theme-id={fiche.id}
                     role="button" tabindex="0" aria-pressed={active === fiche.id}
                     onclick={() => choisir(fiche.id)}
                     onkeydown={activation(() => choisir(fiche.id))}>
                  <span class="pastilles">
                    {#each fiche.pastilles as couleur (couleur)}
                      <span class="pastille" style="background:{couleur}"></span>
                    {/each}
                  </span>
                  <span class="libelles">
                    <span class="nom">{t(`theme.${fiche.id}.nom`)}</span>
                    <span class="desc">{t(`theme.${fiche.id}.desc`)}</span>
                  </span>
                  {#if active === fiche.id}
                    <span class="coche" aria-hidden="true"><Icone name="check_circle" /></span>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if groupe === 'affichage'}
            <p class="section">{t('groupe.affichage')}</p>
            <div class="rangees" data-testid="reglages-affichage">
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.langue')}</span>
                  <span class="desc">{t('reglages.langueDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-langue"
                        aria-label={t('reglages.langue')} value={langue}
                        onchange={(e) => changerLangue(e.target.value)}>
                  {#each LANGUES as code (code)}
                    <option value={code}>{t(`langue.${code}`)}</option>
                  {/each}
                </select>
              </div>
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.volets')}</span>
                  <span class="desc">{t('reglages.voletsDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-volets"
                        aria-label={t('reglages.volets')} value={String(volets)}
                        onchange={(e) => changerVolets(Number(e.target.value))}>
                  {#each [3, 2, 1] as n (n)}
                    <option value={String(n)}>{t(`volets.${n}`)}</option>
                  {/each}
                </select>
              </div>
              <!-- A83 : l'espacement des rangées, au patron EXACT de la
                   Disposition (A26) — sélecteur natif habillé aux jetons
                   de la ligne, aucun dessin neuf (A15 : pas de groupe
                   neuf pour une rangée). -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.espacement')}</span>
                  <span class="desc">{t('reglages.espacementDesc')}</span>
                </span>
                <select class="langue" data-testid="affichage-espacement"
                        aria-label={t('reglages.espacement')} value={espacement}
                        onchange={(e) => changerEspacement(e.target.value)}>
                  {#each NIVEAUX as n (n)}
                    <option value={n}>{t(`espacement.${n}`)}</option>
                  {/each}
                </select>
              </div>
              <!-- R1 (RETOURS-11, D4) : les règles « toujours afficher
                   les images de cet expéditeur », retirables ici. Pas
                   de groupe neuf pour une liste (A15) ; rien ne
                   s'affiche tant qu'aucune règle n'existe — jamais une
                   section vide. -->
              {#if expediteursImages.length > 0}
                <div class="reglage">
                  <span class="libelles">
                    <span class="nom">{t('reglages.imagesExpediteurs')}</span>
                    <span class="desc">{t('reglages.imagesExpediteursDesc')}</span>
                  </span>
                </div>
                {#each expediteursImages as adresse (adresse)}
                  <div class="regle-images" data-testid="expediteur-images">
                    <span class="adresse-regle">{adresse}</span>
                    <button type="button" class="ajouter"
                            data-testid="retirer-expediteur-images"
                            onclick={() => retirerExpediteurImages(adresse)}>
                      {t('reglages.retirerExpediteur')}</button>
                  </div>
                {/each}
              {/if}
            </div>
          {:else if groupe === 'screener'}
            <p class="section">{t('groupe.portier')}</p>
            <div class="rangees" data-testid="reglages-portier">
              <p class="desc-groupe">{t('reglages.portierDesc')}</p>
              {#if portierDefauts}
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.portierOui')}</span>
                  <span class="desc">{t('reglages.portierOuiDesc')}</span>
                </span>
                <select class="langue" data-testid="portier-defaut-oui"
                        aria-label={t('reglages.portierOui')} value={portierDefauts.yes}
                        onchange={(e) => changerPortier('yes', e.target.value)}>
                  <option value="inbox">{t('portier.versReception')}</option>
                  <option value="feed">{t('portier.versKiosque')}</option>
                  <option value="paper_trail">{t('portier.versRegistre')}</option>
                </select>
              </div>
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.portierNon')}</span>
                  <span class="desc">{t('reglages.portierNonDesc')}</span>
                </span>
                <select class="langue" data-testid="portier-defaut-non"
                        aria-label={t('reglages.portierNon')} value={portierDefauts.no}
                        onchange={(e) => changerPortier('no', e.target.value)}>
                  <option value="trash">{t('portier.regleCorbeille')}</option>
                  <option value="archive">{t('portier.regleArchive')}</option>
                  <option value="spam">{t('portier.regleSpam')}</option>
                  <option value="screened_out">{t('portier.regleEcarte')}</option>
                </select>
              </div>
              {/if}
              <!-- RETOURS-14 R5 (D6) : toutes les décisions, à
                   l'alphabet, recherche cliente (une liste de
                   verdicts, pas un corpus — refus §2.6), le geste
                   « Réintégrer » de la page Portier. -->
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.portierDecisions')}</span>
                  <span class="desc">{t('reglages.portierDecisionsDesc')}</span>
                </span>
              </div>
              {#if routagesListe?.length}
                <div class="recherche-decisions">
                  <input type="search" data-testid="portier-recherche"
                         placeholder={t('reglages.portierRecherche')}
                         aria-label={t('reglages.portierRecherche')}
                         bind:value={filtreRoutages} />
                </div>
              {/if}
              {#if routagesVisibles}
                {#if routagesListe.length === 0}
                  <p class="decisions-vide" data-testid="portier-decisions-vide">{t('reglages.portierAucuneDecision')}</p>
                {:else if routagesVisibles.length === 0}
                  <p class="decisions-vide" data-testid="portier-decisions-vide">{t('reglages.portierAucunResultat')}</p>
                {:else}
                  <div class="decisions" data-testid="portier-decisions">
                    {#each routagesVisibles as r (r.address)}
                      <div class="regle-images decision" data-testid="portier-decision">
                        <span class="adresse-regle"><b>{r.address}</b>
                          <span class="verdict">{libelleRoutage(r)}</span></span>
                        <button type="button" class="ajouter"
                                data-testid="decision-modifier"
                                aria-haspopup="menu"
                                aria-expanded={menuDecision?.address === r.address}
                                onclick={(e) => ouvrirModifier(e, r)}>
                          {t('reglages.portierModifier')}</button>
                      </div>
                    {/each}
                  </div>
                {/if}
              {/if}
            </div>
          {:else if groupe === 'notifications'}
            <p class="section">{t('groupe.notifications')}</p>
            <div class="rangees" data-testid="reglages-notifications">
              <div class="reglage">
                <span class="libelles">
                  <span class="nom">{t('reglages.bulles')}</span>
                  <span class="desc">{t('reglages.bullesDesc')}</span>
                </span>
                <button type="button" class="bascule" role="switch"
                        aria-checked={bulles} aria-label={t('reglages.bulles')}
                        data-testid="notif-bulles" onclick={basculerBulles}>
                  <span class="bille"></span>
                </button>
              </div>
            </div>
          {:else if groupe === 'signature'}
            <p class="section">{t('groupe.signature')}</p>
            <div class="rangees" data-testid="reglages-signature">
              <p class="desc-groupe">{t('reglages.signatureDesc')}</p>
              {#each comptes as c (c.account_id)}
                <div class="bloc-signature" data-testid="signature-compte">
                  <!-- D4 (PLAN-RETOURS-9) : en Réglages le nom s'affiche
                       AVEC l'adresse — ici aussi : c'est la surface où
                       éditer le mauvais account coûte (contenu envoyé). -->
                  <span class="adresse-signature">
                    <Icone name="person" />{#if noms[c.account_id]}{noms[c.account_id]}<span class="adresse-sous">{c.email}</span>{:else}{c.email}{/if}</span>
                  <!-- La barre réduite (D3) : gras/italique/souligné —
                       onmousedown neutralisé, un bouton de format ne vole
                       jamais la sélection de l'éditeur (idiome A62). -->
                  <div class="barre-signature">
                    <button type="button" class="bouton-format" aria-label={t('compo.gras')}
                            title={t('compo.gras')} data-testid="signature-gras"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => commandeSignature('bold')}>
                      <Icone name="format_bold" /></button>
                    <button type="button" class="bouton-format" aria-label={t('compo.italique')}
                            title={t('compo.italique')} data-testid="signature-italique"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => commandeSignature('italic')}>
                      <Icone name="format_italic" /></button>
                    <button type="button" class="bouton-format" aria-label={t('compo.souligne')}
                            title={t('compo.souligne')} data-testid="signature-souligne"
                            onmousedown={(e) => e.preventDefault()}
                            onclick={() => commandeSignature('underline')}>
                      <Icone name="format_underlined" /></button>
                  </div>
                  <div class="editeur-signature" contenteditable="true" role="textbox"
                       aria-multiline="true" tabindex="0"
                       data-placeholder={t('reglages.signaturePlaceholder')}
                       aria-label={t('reglages.signaturePlaceholder')}
                       data-testid="signature-editeur"
                       bind:this={champsSignature[c.account_id]}
                       oninput={() => {
                         const sig = signatures[c.account_id];
                         if (sig?.etat) signatures[c.account_id] = { ...sig, etat: null };
                       }}></div>
                  <div class="reglage">
                    <span class="libelles">
                      <span class="nom">{t('reglages.signatureRepliques')}</span>
                      <span class="desc">{t('reglages.signatureRepliquesDesc')}</span>
                    </span>
                    <button type="button" class="bascule" role="switch"
                            aria-checked={signatures[c.account_id]?.replies ?? false}
                            aria-label={t('reglages.signatureRepliques')}
                            data-testid="signature-repliques"
                            onclick={() => basculerRepliques(c)}>
                      <span class="bille"></span>
                    </button>
                  </div>
                  <div class="boutons-signature">
                    <button type="button" class="ajouter" data-testid="signature-enregistrer"
                            onclick={() => enregistrerSignature(c)}>
                      <Icone name="signature" />{t('action.enregistrer')}</button>
                    <button type="button" class="ajouter" data-testid="signature-effacer"
                            onclick={() => effacerSignature(c)}>{t('action.effacer')}</button>
                    {#if comptes.length > 1}
                      <button type="button" class="ajouter" data-testid="signature-tous"
                              onclick={() => appliquerATous(c)}>
                        {t('reglages.signatureTous')}</button>
                    {/if}
                  </div>
                  {#if signatures[c.account_id]?.etat === 'ok'}
                    <p class="etat-signature" data-testid="signature-etat">{t('toast.signature')}</p>
                  {:else if signatures[c.account_id]?.etat === 'tous'}
                    <p class="etat-signature" data-testid="signature-etat">{t('toast.signatureTous')}</p>
                  {:else if signatures[c.account_id]?.etat?.erreur}
                    <p class="erreur-retrait" data-testid="signature-erreur">
                      {signatures[c.account_id].etat.erreur}</p>
                  {/if}
                </div>
              {/each}
            </div>
          {:else if groupe === 'raccourcis'}
            <p class="section">{t('reglages.sectionRaccourcis')}</p>
            <div class="rangees" data-testid="reglages-raccourcis">
              {#each RACCOURCIS as r (r)}
                <div class="raccourci">
                  <kbd>{t(`raccourci.touche.${r}`)}</kbd>
                  <span class="geste">{t(`raccourci.geste.${r}`)}</span>
                </div>
              {/each}
              <p class="note">{t('reglages.noteRaccourcis')}</p>
            </div>
          {:else if groupe === 'apropos'}
            <p class="section">{t('groupe.apropos')}</p>
            <div class="rangees" data-testid="reglages-apropos">
              <!-- V11 : la marque EN TUILE — « À propos » est un des
                   quatre emplacements du régime figé (W-D3). -->
              <span class="marque-bande apropos-bande"><Marque tuile taille={40} /><b>Wind</b></span>
              <div class="ligne-apropos">
                <span class="cle">{t('reglages.version')}</span>
                <span class="valeur" data-testid="apropos-version">{version || '…'}</span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">{t('reglages.maj')}</span>
                <span class="valeur">
                  {#if maj === null}
                    <button type="button" class="ajouter" data-testid="apropos-verifier"
                            onclick={verifierMaj}>{t('reglages.verifierMaj')}</button>
                  {:else if maj === 'controle'}
                    {t('reglages.verification')}
                  {:else if maj === 'ajour'}
                    {t('reglages.ajour')}
                  {:else if maj === 'installation'}
                    {t('reglages.installation')}
                  {:else if maj.version}
                    <!-- L'échec d'INSTALLATION se dit sous son vrai nom
                         (erreur.maj), et l'action reste offerte. -->
                    {#if maj.erreur}{t('erreur.maj', { err: maj.erreur })}. {/if}
                    {t('reglages.majDisponible', { version: maj.version })}
                    <button type="button" class="ajouter" onclick={installerMaj}>
                      {t('action.installer')}</button>
                  {:else}
                    {t('reglages.majImpossible', { err: maj.erreur })}
                  {/if}
                </span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">{t('reglages.icones')}</span>
                <span class="valeur">{t('reglages.iconesValeur')}</span>
              </div>
              <!-- R2 (PLAN-RETOURS-11, verdict CE du STOP visuel) : la
                   mention d'origine est SANS clé — un label posé seul,
                   dégagé du bloc clé/value qui la précède. -->
              <div class="origine" data-testid="apropos-origine">
                <DrapeauUE />{t('reglages.origineValeur')}
              </div>
            </div>
          {/if}
        </div>
      </div>
      <div class="pied">
        <button type="button" class="principal" data-testid="reglages-termine" onclick={fermer}>
          {t('action.termine')}</button>
      </div>
    </div>
  </div>
{/if}

<Menu ouvert={menuDecision !== null} x={menuDecision?.x ?? 0} y={menuDecision?.y ?? 0}
      testid="decision-menu" onfermer={() => (menuDecision = null)}>
    <p class="titre-menu">{t('portier.ouiVers')}</p>
    <button type="button" role="menuitem" data-testid="decision-vers-reception"
            onclick={() => modifierRoutage('inbox')}>
      <Icone name="inbox" />{t('portier.versReception')}</button>
    <button type="button" role="menuitem" data-testid="decision-vers-kiosque"
            onclick={() => modifierRoutage('feed')}>
      <Icone name="feed" />{t('portier.versKiosque')}</button>
    <button type="button" role="menuitem" data-testid="decision-vers-registre"
            onclick={() => modifierRoutage('paper_trail')}>
      <Icone name="paper_trail" />{t('portier.versRegistre')}</button>
    <div class="filet-menu"></div>
    <p class="titre-menu">{t('portier.nonSeront')}</p>
    <button type="button" role="menuitem" data-testid="decision-regle-spam"
            onclick={() => modifierRoutage('screened_out', 'spam')}>
      <Icone name="report" />{t('portier.regleSpam')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-archive"
            onclick={() => modifierRoutage('screened_out', 'archive')}>
      <Icone name="inventory_2" />{t('portier.regleArchive')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-corbeille"
            onclick={() => modifierRoutage('screened_out', 'trash')}>
      <Icone name="delete" />{t('portier.regleCorbeille')}</button>
    <button type="button" role="menuitem" data-testid="decision-regle-ecarte"
            onclick={() => modifierRoutage('screened_out')}>
      <Icone name="visibility_off" />{t('portier.regleEcarte')}</button>
    <div class="filet-menu"></div>
    <button type="button" role="menuitem" data-testid="decision-renvoyer"
            onclick={renvoyerAuPortier}>
      <Icone name="screener" />{t('reglages.renvoyerPortier')}</button>
  </Menu>

<style>
  /* Carte signature du prototype, élargie à 800 px (A13). La hauteur est
     POSÉE (640 px, bornée à l'écran) : le rail ne doit pas respirer au
     gré du groupe affiché. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:800px; height:min(640px, 100%); background:var(--surface);
    border:1px solid var(--border);
    border-radius:var(--r-surface); box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  .tete {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
  }
  .titre { font-size:15px; font-weight:600; flex:1; color:var(--ink); }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }

  .milieu { flex:1; display:flex; min-height:0; }

  /* Le rail : sa grammaire propre depuis A29 (la nav de l'écran 02 vit
     au dessin des pistes) — rangées 36 px, icône + libellé, actif en
     surface blanche avec l'ombre unique, sans filet gauche. */
  .rail {
    width:220px; flex:none; background:var(--bg);
    border-right:1px solid var(--border); padding:20px 16px;
    display:flex; flex-direction:column; gap:4px; overflow:auto;
  }
  /* R2 (PLAN-RETOURS-13) : le glyphe du rail se cale comme celui des
     dossiers de la nav — baseline du libellé + 2 px (le calage optique
     CE, variante C, terrain 2026-08-27) ; le centrage flex posait le
     SVG plus bas que dans la nav. Même mécanique que Nav.svelte : la
     descente est un transform, hors géométrie. La rangée garde ses
     36 px (grammaire du rail, A13/A29) : le libellé porte la baseline
     au centre par sa line-height, l'icône s'y accroche. */
  .rang {
    display:flex; align-items:baseline; gap:10px; height:36px; flex:none;
    padding:0 12px; border-radius:var(--r-controle); cursor:pointer;
    border:1px solid transparent;
  }
  .rang:hover { background:var(--sel); border-color:var(--border); }
  .rang.actif {
    background:var(--surface); border-color:var(--border);
    box-shadow:var(--shadow);
  }
  .icone { color:var(--muted); }
  .icone :global(.ic) { vertical-align:baseline; transform:translateY(2px); }
  .actif .icone { color:var(--accent); }
  .libelle {
    font-size:13px; line-height:36px; color:var(--ink2); flex:1;
    min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .actif .libelle { font-weight:600; color:var(--ink); }

  .volet {
    flex:1; padding:22px; display:flex; flex-direction:column; gap:14px;
    overflow:auto; min-width:0;
  }
  .section {
    margin:0; font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .rangees { display:flex; flex-direction:column; gap:6px; }
  .rangee {
    display:flex; align-items:center; gap:16px; padding:14px 16px;
    border-radius:var(--r-surface); cursor:pointer; border:1px solid transparent;
  }
  .rangee:hover { background:var(--sel); }
  .rangee.active {
    background:var(--surface); border:1px solid var(--border);
    box-shadow:var(--shadow);
  }
  .rangee.active:hover { background:var(--surface); }
  .pastilles { display:flex; gap:5px; flex:none; }
  .pastille {
    width:22px; height:22px; border-radius:var(--r-controle);
    border:1px solid var(--border);
  }
  .libelles {
    display:flex; flex-direction:column; gap:2px; flex:1; min-width:0;
  }
  .nom { font-size:14px; font-weight:600; color:var(--ink); }
  .desc { font-size:12px; line-height:1.4; color:var(--muted); }
  .coche { color:var(--accent); }
  .compte {
    display:flex; align-items:center; gap:12px; padding:10px 16px;
    font-size:13px; color:var(--ink2);
  }
  /* A74 : la pastille du repère garde son encre propre (nuancier
     mesuré) — seuls les glyphes neutres de la rangée sont en muted.
     `:where(…)` : spécificité NULLE pour l'exclusion — sinon la règle
     passerait devant `.deconnecte :global(.ic)` et éteindrait le glyphe
     d'alerte link_off (revue 2026-08-22). */
  .compte :global(:where(:not(.repere)) > .ic) { color:var(--muted); }
  .adresse {
    color:var(--ink); overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* PLAN-RETOURS-9 : le libellé-porte du nom — bouton discret, la
     rangée reste une rangée ; le survol dit qu'il s'ouvre. min-width:0
     + overflow : le bouton rétrécit et ses textes se tronquent — une
     adresse longue ne recouvre jamais les gestes de droite (l'ellipsis
     que la rangée d'avant tenait, revue 2026-08-23). */
  .identite {
    display:flex; flex-direction:column; align-items:flex-start; gap:1px;
    min-width:0; overflow:hidden; padding:2px 6px; margin:0 -6px;
    font-size:13px; text-align:left; color:var(--ink);
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-controle); cursor:pointer;
  }
  .identite:hover { background:var(--sel); border-color:var(--border); }
  .identite .nom-compte, .identite .adresse {
    max-width:100%; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* `.nom` appartient aux fiches des groupes (14px) — le nom de compte
     a SA classe, jamais un réemploi (collision relevée en revue). */
  .nom-compte { color:var(--ink); font-weight:600; }
  .sous-nom { font-size:12px; color:var(--muted); }
  .champ-nom {
    height:32px; padding:0 10px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle);
  }
  .champ-nom:focus { border-color:var(--accent); outline:none; }
  .ajouter {
    height:32px; padding:0 16px; align-self:flex-start; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .ajouter:hover { background:var(--sel); }

  /* Jeton mort : l'état se dit en alerte (link_off + « Déconnecté »),
     poussé à droite avec le geste de réparation — la rangée d'un compte
     sain, elle, ne change pas. */
  .deconnecte {
    margin-left:auto; flex:none; display:inline-flex; align-items:center;
    gap:6px; font-size:12.5px; font-weight:600; color:var(--alert);
    white-space:nowrap;
  }
  .deconnecte :global(.ic) { color:var(--alert); width:15px; height:15px; }
  .reconnecter {
    height:28px; padding:0 12px; flex:none; display:inline-flex;
    align-items:center; font-size:12.5px; font-weight:600;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
    white-space:nowrap;
  }
  .reconnecter:hover:not(:disabled) { background:var(--sel); }
  .reconnecter:disabled { opacity:.6; cursor:default; }
  /* Un compte déconnecté a déjà son état à droite : la corbeille du
     retrait perd son ressort automatique. */
  .compte:has(.deconnecte) .retirer, .compte:has(.btn-horizon) .retirer { margin-left:0; }
  /* La porte de l'horizon : la valeur en texte, discrète au repos —
     le dessin du retrait, sans l'alerte. */
  .btn-horizon {
    height:28px; padding:0 10px; margin-left:auto; flex:none;
    display:inline-flex; align-items:center; font-size:12.5px;
    white-space:nowrap; color:var(--muted); background:transparent;
    border:1px solid transparent; border-radius:var(--r-controle); cursor:pointer;
  }
  .btn-horizon:hover { color:var(--ink); background:var(--sel); border-color:var(--border); }
  .compte:has(.deconnecte) .btn-horizon { margin-left:0; }
  .select-horizon {
    height:32px; font-size:13px; padding:0 10px; align-self:flex-start;
    min-width:200px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
    outline:none; cursor:pointer;
  }
  .note-horizon { margin:0; font-size:12px; line-height:1.5; color:var(--muted); }
  .erreur-reconnexion {
    margin:0; padding:0 16px 6px; font-size:12px; line-height:1.4;
    color:var(--alert);
  }

  /* Le retrait : discret au repos (la rangée reste une rangée), l'alerte
     ne se montre qu'au survol — le rouge permanent crierait sur chaque
     compte sain. */
  .retirer {
    height:28px; padding:0 10px; margin-left:auto; flex:none;
    display:inline-flex; align-items:center; justify-content:center;
    gap:6px; font-size:12.5px; white-space:nowrap;
    color:var(--muted); background:transparent;
    border:1px solid transparent; border-radius:var(--r-controle); cursor:pointer;
  }
  .retirer:hover {
    color:var(--alert); background:var(--sel); border-color:var(--border);
  }
  /* La « carte sous la rangée » — UNE règle pour retrait, repère et
     ajout (revue 2026-08-22 : trois copies identiques dérivaient). */
  .carte-retrait, .carte-repere, .carte-ajout, .carte-nom {
    border:1px solid var(--border);
    border-radius:var(--r-surface); padding:14px 16px 16px;
    display:flex; flex-direction:column; gap:12px;
  }
  /* A74 — le repère : la porte est l'icône de la rangée (bouton
     discret, le dessin du retrait), la carte de choix suit le patron
     de la carte de retrait — le MÊME bloc de règles, pas une copie. */
  .btn-repere {
    height:28px; width:28px; padding:0; flex:none;
    display:inline-flex; align-items:center; justify-content:center;
    background:transparent; border:1px solid transparent;
    border-radius:var(--r-controle); cursor:pointer;
  }
  .btn-repere:hover { background:var(--sel); border-color:var(--border); }
  .titre-repere { margin:0; font-size:13px; font-weight:600; color:var(--ink); }
  .choix-repere { display:flex; flex-wrap:wrap; gap:6px; }
  .choix {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .choix:hover { background:var(--sel); }
  .choix.choisi { border-color:var(--accent); background:var(--sel); }
  .pastille-teinte { width:18px; height:18px; }
  .erreur-repere { margin:0; font-size:12px; line-height:1.4; color:var(--alert); }
  .avertissement { margin:0; font-size:13px; line-height:1.5; color:var(--ink2); }
  .erreur-retrait { margin:0; font-size:12px; line-height:1.4; color:var(--alert); }
  .boutons-retrait { display:flex; align-items:center; gap:10px; }
  .danger {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; font-weight:600; color:var(--onAccent);
    background:var(--alert); border:1px solid var(--alert);
    border-radius:var(--r-controle); cursor:pointer;
  }
  .danger:disabled { opacity:.6; cursor:default; }
  .tete-ajout { display:flex; align-items:center; gap:14px; }
  .titre-ajout { flex:1; font-size:14px; font-weight:600; color:var(--ink); }

  /* Une rangée de réglage : libellé + description, interrupteur à
     droite. L'interrupteur reste aux jetons — piste `--bg`/filet au
     repos (V3), accent quand il est armé ; focus visible hérité (A8). */
  .reglage {
    display:flex; align-items:center; gap:16px; padding:14px 16px;
    border-radius:var(--r-surface);
  }
  .bascule {
    width:38px; height:22px; flex:none; padding:2px; cursor:pointer;
    display:inline-flex; align-items:center;
    background:var(--bg); border:1px solid var(--border);
    border-radius:999px; transition:background .12s ease;
  }
  .bille {
    width:16px; height:16px; border-radius:50%;
    background:var(--surface); border:1px solid var(--border);
    transition:transform .12s ease;
  }
  .bascule[aria-checked="true"] {
    background:var(--accent); border-color:var(--accent);
  }
  .bascule[aria-checked="true"] .bille {
    transform:translateX(16px); border-color:var(--accent);
  }

  /* Les sélecteurs (Langue, Disposition) : la grammaire des boutons
     (32 px, jetons) — un <select> natif, clavier et lecteur d'écran
     compris. */
  .langue {
    height:32px; padding:0 10px; flex:none; font:inherit; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); cursor:pointer;
  }
  .langue option { background:var(--surface); color:var(--ink); }

  /* R1 (RETOURS-11, D4) : une règle d'images par rangée — l'adresse et
     sa porte de sortie, aux jetons de la carte. */
  .regle-images {
    display:flex; align-items:center; gap:12px; padding:6px 16px;
    font-size:13px; color:var(--ink);
  }
  .adresse-regle {
    flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  /* RETOURS-14 R5 : la liste des décisions du Portier — rangée au
     dessin de .regle-images, verdict à l'encre atténuée derrière
     l'adresse ; champ de recherche au gabarit des contrôles (32 px). */
  .decision .verdict { margin-left:8px; color:var(--muted); }
  .recherche-decisions { padding:2px 16px 8px; }
  .recherche-decisions input {
    width:100%; height:32px; padding:0 12px; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .recherche-decisions input:focus-visible {
    outline:2px solid var(--accent); outline-offset:-1px;
  }
  .decisions-vide {
    margin:0; padding:6px 16px 10px; font-size:13px; color:var(--muted);
  }
  /* R10 : le menu « Modifier » — le dessin des menus du produit
     (famille D-47, consignée). Au-dessus de la surimpression des
     Réglages (z-index 2). */

  /* Raccourcis : référence en lecture seule, aux jetons. */
  .raccourci {
    display:flex; align-items:center; gap:14px; padding:8px 16px;
    font-size:13px; color:var(--ink2);
  }
  kbd {
    min-width:44px; padding:3px 8px; text-align:center; flex:none;
    font-family:inherit; font-size:12px; font-weight:600; color:var(--ink);
    background:var(--bg); border:1px solid var(--border);
    border-bottom-width:2px; border-radius:var(--r-controle);
  }
  .geste { color:var(--ink2); }
  .note {
    margin:6px 0 0; padding:0 16px; font-size:12px; line-height:1.4;
    color:var(--muted);
  }

  /* À propos : clé / valeur, sans invention de forme. */
  /* La bande partagée (.marque-bande, systeme.css) — ici avec son
     dégagement propre. */
  .apropos-bande { padding:2px 0 6px; }
  .ligne-apropos {
    display:flex; align-items:baseline; gap:14px; padding:10px 16px;
    font-size:13px;
  }
  .cle { width:110px; flex:none; color:var(--muted); }
  .valeur {
    color:var(--ink); display:inline-flex; flex-wrap:wrap;
    align-items:center; gap:10px; min-width:0;
  }
  /* La mention d'origine (R2, RETOURS-11) : sans clé, dégagée du bloc
     clé/valeur par une marge haute, et ALIGNÉE sur la colonne des
     valeurs (verdicts CE du STOP visuel) : 16 px du bord + 110 px de
     clé + 14 px de gouttière = 140 px. */
  .origine {
    display:flex; align-items:center; gap:10px; margin-top:18px;
    padding:10px 16px 10px calc(16px + 110px + 14px);
    font-size:13px; color:var(--ink);
  }

  /* R1 (PLAN-RETOURS-6) : le groupe Signature — un bloc par compte,
     éditeur riche réduit aux jetons de la carte. */
  .desc-groupe {
    margin:0; padding:0 16px 4px; font-size:12px; line-height:1.5;
    color:var(--muted);
  }
  .bloc-signature {
    display:flex; flex-direction:column; gap:10px; padding:12px 16px;
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .adresse-signature {
    display:flex; align-items:center; gap:8px;
    font-size:13px; font-weight:600; color:var(--ink);
  }
  .adresse-sous { font-weight:400; color:var(--muted); }
  .barre-signature { display:flex; align-items:center; gap:6px; }
  .bouton-format {
    height:32px; min-width:32px; padding:0 6px; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:var(--r-controle);
  }
  .bouton-format:hover { background:var(--sel); color:var(--ink); }
  .editeur-signature {
    min-height:72px; padding:10px 12px; font-size:13px; line-height:1.6;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:var(--r-controle); outline:none;
    overflow-wrap:break-word;
  }
  .editeur-signature:focus { border-color:var(--accent); }
  .editeur-signature:empty::before {
    content:attr(data-placeholder); color:var(--muted); pointer-events:none;
  }
  .boutons-signature { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .etat-signature { margin:0; font-size:12px; line-height:1.4; color:var(--accent); }

  .pied {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center;
  }
  .principal {
    height:32px; padding:0 16px; margin-left:auto; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; font-weight:600;
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:var(--r-controle); cursor:pointer;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
