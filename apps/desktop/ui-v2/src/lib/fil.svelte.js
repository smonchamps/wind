// L'état du FIL ouvert — UN objet, DEUX cadres (UI v3, décision D4 du
// 2026-08-16 : « une coexistence qui n'est qu'un changement de taille
// des mêmes objets »). Le volet de lecture et l'écran 03 montent tous
// deux Fil.svelte sur CET état ; `cadre` dit lequel tient l'objet —
// c'est le SEUL interrupteur, l'exclusivité est structurelle (revue
// v3 : trois booléens réconciliés à la main se désynchronisaient au
// premier chemin oublié — archiver au clavier, bascule de disposition).
//
// Invariant S1 intact : chaque corps vient de message_body/echo_body
// (assaini côté cœur), chargé au dépliage seulement, affiché dans une
// iframe sandbox par message — jamais innerHTML.
import { appel } from './transport.js';

const VIDE = () => ({
  messages: [],
  deplies: {},
  corps: {},
  pieces: {},
  // Le compte de pièces d'APRÈS-SCAN par message (vue.attachment_count
  // de message_body) : la ligne de liste porte celui d'AVANT
  // l'ouverture — s'y fier ouvrait les pièces fraîchement reçues sur
  // une rangée vide (terrain CE, 2026-08-14 ; régression attrapée à la
  // revue v3).
  nbPieces: {},
  imagesBloquees: {},
  // La carte d'invitation par message (PLAN-INVITATIONS) : la vue
  // arrive AVEC le corps (BodyView.invitation) — aucun aller-retour
  // dédié. `undefined`/`null` = pas de carte ; objet = la carte.
  invitations: {},
  // R4 (PLAN-RETOURS-7) : la conversation ouverte est-elle épinglée ?
  // Lu par le FIL côté cœur (pin_state) à l'ouverture, tenu à jour par
  // le geste (App.epinglerFil). Faux par défaut — le bouton dit
  // « Épingler » tant que le cœur n'a pas répondu.
  epingle: false,
});

export const fil = $state({
  // Quel cadre tient l'objet : null (aucun), 'volet', 'plein'.
  cadre: null,
  // La ligne d'origine (sélection de liste) et l'état par message.
  ligne: null,
  ...VIDE(),
  // Le chrono d'ouverture (banc P1, e2e) : sélection → corps du
  // dernier message posé. Les pièces restent HORS chrono, comme au
  // volet d'avant v3.
  derniereOuvertureMs: null,
});

let jeton = 0;

// R5 (PLAN-RETOURS-12) : le dernier bilan de `noms_adresses`, clé =
// adresses jointes. Il vit ICI et non dans le composant : Fil est
// démonté/remonté à chaque bascule de cadre (volet ↔ écran 03), et sans
// ce cache chaque bascule repartait en RPC pour les MÊMES adresses
// (revue). Un objet nu suffit — l'effet du composant le consulte et le
// repose.
export const cacheNoms = { cle: '', noms: {} };

export const cleMsg = (m) => `${m.account_id}/${m.mailbox}/${m.uid}`;

// Un écho local (PLAN-REACTIVITE E3) se reconnaît à sa boîte
// synthétique — son corps est local (echo_body), jamais de fil.
export const estEcho = (m) =>
  typeof m?.mailbox === 'string' && m.mailbox.startsWith('echo:');

// Ouvre le fil de `nouvelle` dans `cadre` — TOUJOURS rechargé : la
// mémoïsation de la première v3 rendait un fil périmé (sa propre
// réponse absente après un envoi) et figeait un échec de chargement.
// L'agrandissement ne passe PAS ici : c'est `agrandirFil()`, zéro
// rechargement — le cadre change, pas l'objet (D4).
export async function ouvrirFil(nouvelle, cadre = 'volet') {
  const t0 = performance.now();
  const mien = ++jeton;
  fil.cadre = cadre;
  fil.ligne = nouvelle;
  Object.assign(fil, VIDE());
  // R4 : l'état d'épingle vient de la LIGNE servie — exact par
  // construction dans la Réception, la seule à offrir le geste (D4) :
  // une ligne du flot n'est JAMAIS épinglée (D5, le cœur l'exclut),
  // une ligne de la section l'est toujours. Aucun aller-retour au cœur
  // sur le chemin d'ouverture (revue 2026-08-21 : un pin_state par
  // ouverture, dans la file sérialisée, payait pour un bouton le plus
  // souvent absent — et l'état mentait pendant l'aller-retour).
  fil.epingle = nouvelle.pinned ?? false;
  // V-D2 : sans fil — écho compris — le MESSAGE SEUL est le fil.
  if (nouvelle.thread_id == null) {
    fil.messages = [nouvelle];
    await basculerMessage(nouvelle, true);
    if (mien === jeton) fil.derniereOuvertureMs = performance.now() - t0;
    return fil.derniereOuvertureMs;
  }
  try {
    const messages = await appel('thread_messages', { threadId: nouvelle.thread_id });
    if (mien !== jeton) return fil.derniereOuvertureMs;
    fil.messages = messages;
    const dernier = messages[messages.length - 1];
    if (dernier) await basculerMessage(dernier, true);
  } catch (err) {
    console.error('thread_messages :', err);
  }
  if (mien === jeton) fil.derniereOuvertureMs = performance.now() - t0;
  return fil.derniereOuvertureMs;
}

// Le changement de taille (D4) : aucun rechargement, aucun jeton.
export function agrandirFil() {
  if (fil.ligne) fil.cadre = 'plein';
}
// Le retour : au volet si le mode en a un, sinon fermeture.
export function reduireFil(versVolet) {
  if (versVolet && fil.ligne) fil.cadre = 'volet';
  else fermerFil();
}

// Terrain R8' (2026-08-23) : « Supprimer » vise UN message — le fil
// ouvert le retire et reste en place s'il lui en reste. Rend le nombre
// restant ; 0 = plus rien à montrer, l'appelant ferme. Un message qui
// n'appartient pas au fil ouvert rend -1 (rien n'est touché).
export function retirerMessage(m) {
  const k = cleMsg(m);
  if (!fil.messages.some((x) => cleMsg(x) === k)) return -1;
  fil.messages = fil.messages.filter((x) => cleMsg(x) !== k);
  delete fil.deplies[k];
  delete fil.corps[k];
  delete fil.pieces[k];
  delete fil.nbPieces[k];
  delete fil.imagesBloquees[k];
  delete fil.invitations[k];
  return fil.messages.length;
}

export function fermerFil() {
  jeton += 1;
  fil.cadre = null;
  fil.ligne = null;
  Object.assign(fil, VIDE());
  fil.derniereOuvertureMs = null;
}

async function chargerMessage(m, avecImages = false) {
  const k = cleMsg(m);
  const mien = jeton;
  if (fil.corps[k] === undefined || avecImages) {
    if (fil.corps[k] === undefined) fil.corps[k] = '';
    try {
      const vue = estEcho(m)
        ? await appel('echo_body', {
            id: Number(m.mailbox.slice(5)),
            showImages: avecImages,
          })
        : await appel('message_body', {
            accountId: m.account_id,
            mailbox: m.mailbox,
            uid: m.uid,
            showImages: avecImages,
          });
      // Le jeton d'ouverture garde chaque écriture : une réponse
      // tardive (images accordées puis sélection changée) n'écrase
      // jamais l'état d'un fil plus récent.
      if (mien !== jeton) return;
      fil.corps[k] = vue.document;
      fil.imagesBloquees[k] = avecImages ? 0 : vue.remote_images_blocked;
      fil.nbPieces[k] = vue.attachment_count;
      // La carte d'invitation voyage avec le corps — même fraîcheur que
      // le compte de pièces, aucun aller-retour de plus (revue).
      fil.invitations[k] = vue.invitation ?? null;
    } catch (err) {
      console.error('message_body :', err);
    }
  }
  // Les métadonnées de pièces : HORS du chemin mesuré (elles arrivent
  // après le corps, jamais avant), gatées sur le compte d'après-scan.
  // Un écho les tire du journal d'envoi (echo_attachments — nom et
  // taille seuls, les octets sont purgés) : jamais un titre « Fichiers
  // joints » sans rien dessous (PLAN-RETOURS-5, D2).
  const nb = fil.nbPieces[k] ?? m.attachment_count;
  if (nb > 0 && fil.pieces[k] === undefined) {
    fil.pieces[k] = [];
    const lecture = estEcho(m)
      ? appel('echo_attachments', { id: Number(m.mailbox.slice(5)) })
      : appel('message_attachments', {
          accountId: m.account_id,
          mailbox: m.mailbox,
          uid: m.uid,
        });
    lecture
      .then((lues) => {
        if (mien === jeton) fil.pieces[k] = lues;
      })
      .catch((err) => console.error('message_attachments :', err));
  }
}

export function basculerMessage(m, valeur = null) {
  const k = cleMsg(m);
  const nouveau = valeur ?? !fil.deplies[k];
  fil.deplies[k] = nouveau;
  return nouveau ? chargerMessage(m) : Promise.resolve();
}

export function toutDeplier() {
  for (const m of fil.messages) basculerMessage(m, true);
}

// Le geste inverse (terrain A46) : TOUT se replie, le dernier compris.
// La bascule « Tout déplier »/« Tout replier » n'est PAS un drapeau :
// elle se DÉRIVE de l'état réel des dépliages (terrain A47 — un fil
// d'un message s'ouvre déplié, le bouton dit « Tout replier » ; les
// dépliages manuels la font suivre).
export function toutReplier() {
  for (const m of fil.messages) basculerMessage(m, false);
}

// Images distantes : bloquées par DÉFAUT (l'invariant qui reste), avec
// deux exceptions EXPLICITES et PERSISTANTES (RETOURS-11, D1 renverse
// A43 « l'opt-in ne survit pas à la sélection ») : par message ici,
// par expéditeur ci-dessous. L'écriture part en tir-et-oublie et le
// rechargement DANS LE MÊME TOUR : le rendu immédiat n'a pas besoin de
// l'écriture (`showImages: true` suffit à la session), la file
// sérialisée du cœur pose l'écriture avant toute lecture future, et
// `chargerMessage` capture son jeton au clic — un `await` ici rendait
// la garde anti-course vacante (revue 2026-08-28). Si l'écriture
// échoue, les images de la session s'affichent quand même et l'échec
// est dit. Un écho local reste hors mémoire (clé éphémère par nature).
export function afficherImages(m) {
  if (!estEcho(m)) {
    appel('allow_images_message', {
      accountId: m.account_id,
      mailbox: m.mailbox,
      uid: m.uid,
    }).catch((err) => console.error('allow_images_message :', err));
  }
  return chargerMessage(m, true);
}

// D3 : « Toujours afficher les images de cet expéditeur » — l'adresse
// est résolue par le CŒUR depuis l'enveloppe (l'UI ne parse jamais une
// adresse) ; la règle est globale au poste et se révoque aux Réglages
// (D4). Elle n'écrit PAS de choix par message : sa révocation défait
// tout. Le retour du cœur se LIT : `null` = enveloppe sans adresse,
// rien n'a été écrit — le dire, sinon la promesse du bouton se rompt
// en silence (revue 2026-08-28).
export function toujoursAfficherImages(m) {
  // Jamais offert sur un écho (le template le garde déjà — ceinture).
  if (estEcho(m)) return Promise.resolve();
  const mien = jeton;
  appel('allow_images_sender', {
    accountId: m.account_id,
    mailbox: m.mailbox,
    uid: m.uid,
  })
    .then((adresse) => {
      if (adresse == null) {
        console.error(
          'allow_images_sender : enveloppe sans adresse — aucune règle posée',
        );
        return;
      }
      if (mien !== jeton) return;
      // La règle couvre les AUTRES messages du fil dont le bandeau est
      // levé : les recharger sans opt-in — le cœur départage, un
      // message d'un tiers re-rend à l'identique.
      for (const autre of fil.messages) {
        const ka = cleMsg(autre);
        if (ka !== cleMsg(m) && (fil.imagesBloquees[ka] ?? 0) > 0) {
          delete fil.corps[ka];
          chargerMessage(autre);
        }
      }
    })
    .catch((err) => console.error('allow_images_sender :', err));
  return chargerMessage(m, true);
}
