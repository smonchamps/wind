// Gate du thread principal (PLAN-GELS E1, décision D1) : dans Tauri 2,
// une commande déclarée SANS `async` s'exécute sur le thread principal —
// celui de la pompe de messages Windows. Toute commande synchrone qui
// ouvre la base, touche un fichier ou le keyring gèle donc la fenêtre
// pour toute sa durée (constat du 2026-08-15 : gels de 2 à 4,6 s au
// démarrage, 25,2 s cumulés sur 40 s — la fenêtre « ne répond pas »).
// Et `async` seul ne suffit pas : le corps bloquant passe par
// `off_pump` (spawn_blocking + verrou des commandes), sinon il épingle
// un worker tokio — le gel quitte la fenêtre pour la file IPC.
//
//   node garde-thread-principal.mjs   -> commandes fautives + verdict
//
// La règle est INVERSÉE à dessein : toute commande `#[tauri::command]`
// est `async fn`, SAUF les commandes pures d'état nommées ci-dessous.
// Une liste de marqueurs bloquants ne tiendrait pas : elle raterait la
// commande qui bloque au travers d'une aide (`queue_removal` ouvre la
// base pour `archive_message`). L'exemption, elle, se voit et se
// justifie.
//
// Deux gardes de l'instrument lui-même (le bogue « ink2 », payé deux
// fois — contraste.mjs:24, coherence-systeme.mjs:41) : la regex accepte
// les attributs paramétrés, `pub(crate)` et les chiffres ; et chaque
// occurrence textuelle de `#[tauri::command` doit correspondre à une
// prise — zéro prise ou un écart de compte est un ÉCHEC, jamais un vert
// silencieux.
//
// Le remède est toujours le même : passer la commande en `async fn` et
// son corps par `off_pump` — jamais allonger l'exemption sans la même
// preuve de pureté.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const sources = path.join(root, 'apps', 'desktop', 'src');

// Les exemptées, chacune avec sa raison d'être synchrone :
// - sync_activity : verrous TRÈS courts (trois Mutex de texte écrits une
//   fois par compte/boîte par le cycle) — pas des atomiques, mais des
//   fenêtres de quelques microsecondes ; le budget de la sonde
//   (`sonde-gel.py`, 150 ms) attraperait toute dérive ;
// - migration_progress : lecture d'atomiques partagés ;
// - migration_cancel : écriture d'un atomique ;
// - network_state : un atomique, plus UN `sync_reculs.lock()` court
//   (clear d'une petite map au retour du réseau) — même garde-fou que
//   sync_activity : la sonde mesure, le budget tranche ;
// - app_version : lecture du manifeste en mémoire ;
// - open_link : ShellExecuteW DÉTACHÉ (open::that_detached) — vrai
//   SEULEMENT avec la feature `shellexecute-on-windows` de la crate
//   `open` (apps/desktop/Cargo.toml) ; sans elle, powershell.exe
//   synchrone sur la pompe (audit 2026-09-01) ;
// - telemetry_selftest_panic : ne bloque pas, elle PANIQUE — et
//   l'ADR 0014 a validé le double-panic du THREAD PRINCIPAL (frontière
//   FFI WebView2) : la déplacer changerait ce que l'auto-test exerce.
const PURES = new Set([
  'sync_activity',
  'migration_progress',
  'migration_cancel',
  'network_state',
  'app_version',
  'open_link',
  'telemetry_selftest_panic',
]);

// Ce qui BLOQUE à coup sûr : base (chaque commande ouvre sa connexion),
// fichiers, coffre de l'OS. Si une exemptée s'en approche, elle perd
// l'exemption. (Détection best-effort — l'aide indirecte lui échappe,
// c'est pour cela que l'exemption est une LISTE et pas une heuristique.)
const MARQUEURS = ['Store::', 'db_path(', 'std::fs', 'File::', 'keyring', 'read_to_string'];

// PLAN-AUDIT-V1 E5 (audit 2026-09-01 S1-2) : la garde s'arrêtait au
// mot-clé `async` — dix-sept commandes ouvraient la base, lisaient le
// coffre ou écrivaient un fichier DANS le corps async, hors
// `off_pump` : le blocage quittait la pompe pour un worker tokio
// (workers = cœurs) et échappait au verrou des commandes (ADR 0019).
// Règle : le corps d'une commande async, une fois RETIRÉS les appels
// `off_pump(...)` et `spawn_blocking(...)` (parenthèses équilibrées),
// n'est que de la glu — aucun de ces marqueurs n'y a sa place.
// `db_path(` n'y est PAS : depuis E5 c'est une lecture pure (OnceLock,
// le dossier est créé au premier appel) — il reste dans MARQUEURS pour
// les exemptées, qui ne doivent même pas nommer la base. `lock_accounts`
// et `veilleur::reconcilier` sont des verrous mémoire de quelques
// microsecondes, pas de l'I/O : la sonde (`sonde-gel.py`) tranche.
const MARQUEURS_GLU = [
  ...MARQUEURS.filter((m) => m !== 'db_path('),
  'auth_for(',
  'connected_jobs(',
  'account_email(',
  'mail_render::sanitize',
  'connect_imap(',
  'trace_maj(',
];
const HORS_POMPE = ['off_pump(', 'spawn_blocking('];

// Retire du texte chaque appel `nom(...)` avec ses parenthèses
// équilibrées — ce qui reste est la glu que la commande exécute
// elle-même, sur le worker async.
function sansLesAppels(texte, noms) {
  let reste = texte;
  for (const nom of noms) {
    let depart = reste.indexOf(nom);
    while (depart !== -1) {
      const ouvrante = depart + nom.length - 1;
      let profondeur = 0;
      let fin = ouvrante;
      for (; fin < reste.length; fin += 1) {
        if (reste[fin] === '(') profondeur += 1;
        else if (reste[fin] === ')') {
          profondeur -= 1;
          if (profondeur === 0) break;
        }
      }
      reste = reste.slice(0, depart) + reste.slice(fin + 1);
      depart = reste.indexOf(nom);
    }
  }
  return reste;
}

let echecs = 0;
const echec = (message) => {
  echecs += 1;
  console.log(`ECHEC ${message}`);
};

// Extrait le corps d'une fonction par équilibrage d'accolades, à partir
// de l'offset de sa première `{`. Heuristique assumée : une accolade
// non appariée dans une chaîne fausserait la borne — les corps exemptés
// sont courts et relus à chaque ajout à PURES.
function corps(texte, debut) {
  let profondeur = 0;
  for (let i = debut; i < texte.length; i += 1) {
    if (texte[i] === '{') profondeur += 1;
    else if (texte[i] === '}') {
      profondeur -= 1;
      if (profondeur === 0) return texte.slice(debut, i + 1);
    }
  }
  return texte.slice(debut);
}

let attributs = 0;
let prises = 0;
for (const fichier of readdirSync(sources).filter((f) => f.endsWith('.rs'))) {
  const texte = readFileSync(path.join(sources, fichier), 'utf8');
  attributs += (texte.match(/#\[tauri::command/g) ?? []).length;
  const commandes = texte.matchAll(
    /#\[tauri::command[^\]]*\]\s*(?:#\[[^\]]*\]\s*|\/\/[^\n]*\n\s*)*pub(?:\([^)]*\))?\s+(async\s+)?fn\s+([A-Za-z0-9_]+)/g,
  );
  for (const prise of commandes) {
    prises += 1;
    const [, estAsync, nom] = prise;
    if (estAsync) {
      if (PURES.has(nom)) {
        echec(
          `${fichier} : \`${nom}\` est async mais figure dans l'exemption des pures — retirer l'un ou l'autre`,
        );
        continue;
      }
      // E5 : async ne suffit pas — le bloquant passe par off_pump.
      const ouvranteAsync = texte.indexOf('{', prise.index + prise[0].length);
      if (ouvranteAsync === -1) continue;
      const glu = sansLesAppels(corps(texte, ouvranteAsync), HORS_POMPE);
      const dansLaGlu = MARQUEURS_GLU.filter((m) => glu.includes(m));
      if (dansLaGlu.length > 0) {
        echec(
          `${fichier} : la commande async \`${nom}\` touche ${dansLaGlu.join(', ')} HORS de off_pump/spawn_blocking — bloque un worker tokio sans le verrou des commandes (ADR 0019)`,
        );
      }
      continue;
    }
    if (!PURES.has(nom)) {
      echec(
        `${fichier} : la commande synchrone \`${nom}\` s'exécute sur le thread principal — la passer en \`async fn\` + \`off_pump\` (ou prouver sa pureté et l'exempter)`,
      );
      continue;
    }
    const ouvrante = texte.indexOf('{', prise.index + prise[0].length);
    if (ouvrante === -1) continue;
    const interieur = corps(texte, ouvrante);
    const trouves = MARQUEURS.filter((m) => interieur.includes(m));
    if (trouves.length > 0) {
      echec(
        `${fichier} : \`${nom}\` est exemptée comme pure mais touche ${trouves.join(', ')} — la passer en \`async fn\` + \`off_pump\``,
      );
    }
  }
}

// L'instrument se vérifie comme le reste (PASSATION §9) : chaque
// attribut doit avoir sa prise, et zéro commande = la gate ne regarde
// plus rien (déplacement de dossier, changement de forme).
if (prises === 0) {
  echec('aucune commande trouvée — la gate ne vérifie plus rien (dossier déplacé ? forme changée ?)');
} else if (prises !== attributs) {
  echec(
    `${attributs} attributs #[tauri::command] mais ${prises} prises — la regex rate des commandes`,
  );
}

if (echecs > 0) {
  console.log(`\n${echecs} défaut(s) sur le thread principal.`);
  process.exitCode = 1;
} else {
  console.log(
    `OK : ${prises} commandes vérifiées, aucune bloquante sur le thread principal.`,
  );
}
