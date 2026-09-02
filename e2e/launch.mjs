// Lanceur E2E : construit l'application, seed une base ISOLÉE, démarre la
// fenêtre Tauri avec les crochets de test, s'y attache via CDP.
//
// Déterminisme par construction :
// - base de test jetable (WIND_DB_PATH) — jamais la vraie ;
// - compte factice au jeton invalide (WIND_E2E_ACCOUNT) — hors ligne
//   garanti, la boîte d'envoi journalise sans jamais rien envoyer ;
// - configuration OAuth retirée de l'environnement — aucun test ne peut
//   toucher au vrai compte, même par accident.
//
// Deux leçons du premier passage en CI :
// - **diagnosticabilité** : la sortie de l'application est CAPTURÉE et
//   recrachée en cas d'échec. Sans cela, une panique au démarrage ou un
//   WebView2 absent se présentent comme un timeout muet, indiagnosticable
//   à distance ;
// - **on attend la PAGE, pas le port** : le CDP répond avant que la fenêtre
//   n'ait créé son document. Se contenter du port ouvert crée une course
//   qui se voit dès que le démarrage est froid.
import { spawn, execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { balayerZombies, construireV2, purgerCacheHttp } from './rebuild-v2.mjs';
import { purgerOAuth } from './isolation.mjs';
import { allouerPortCdp } from './port-cdp.mjs';
import { argsNavigateur } from './args-navigateur.mjs';

const root = path.resolve(import.meta.dirname, '..');
// Un premier démarrage WebView2 sur machine froide (CI : pas de cache,
// antivirus actif) dépasse largement les 30 s d'origine.
const READY_TIMEOUT_MS = 90_000;
const POLL_MS = 500;
// Mémo du port de la suite (alloué au premier lancement, voir attacher).
let portSuite = null;

// L'app embarque ui-v2 (la seule interface depuis B2, PLAN-RETRAIT-V1) ;
// le décor par défaut est le jeu d'essai Clarity (seed_clarity). Les
// pièges du rebuild (dist périmé, zombie, cache) vivent dans
// `rebuild-v2.mjs`, une fois.
// Graines par GABARIT (PLAN-KAIZEN-CLAUDE vague 2, E6) : la même
// recette de seed était rejouée par `cargo run --example` à CHAQUE spec
// (~14 exécutions par suite). Le gabarit se construit une fois — clé =
// recette + empreinte de l'exe du seeder (un seeder modifié invalide) —
// puis chaque spec reçoit une COPIE de fichier (la base reste jetable
// et isolée, STANDARD §7.1 tenu). Les exemples sont compilés une fois
// par processus de suite ; on exécute ensuite l'exe directement.
let exemplesConstruits = false;

function seeder(db, etapes) {
  if (!exemplesConstruits) {
    execSync('cargo build -p mail-core --examples', { cwd: root, stdio: 'inherit' });
    exemplesConstruits = true;
  }
  const exe = (exemple) => path.join(root, 'target', 'debug', 'examples', `${exemple}.exe`);
  const hash = createHash('sha1');
  for (const nom of [...new Set(etapes.map((etape) => etape.exemple))].sort()) {
    const stat = statSync(exe(nom));
    hash.update(`${nom}|${stat.size}|${stat.mtimeMs}\0`);
  }
  hash.update(JSON.stringify(etapes));
  const gabarit = path.join(root, 'target', 'e2e', 'gabarits', `${hash.digest('hex')}.db`);
  // Les seeders figent l'horloge À LA CONSTRUCTION — les jours relatifs
  // (« aujourd'hui », « hier ») mais aussi `derniere_synchro` posée « il
  // y a 2 min » : un gabarit d'il y a une heure fait dire « il y a
  // 1 heure » à la barre d'état (rouge PAYÉ à la gate du push,
  // 2026-08-23 — une clé à la journée ne suffisait pas). Fraîcheur par
  // TTL : au-delà de 30 min, on reconstruit (~1-4 s), le décor reste
  // dans la minute de son vocabulaire. ET par jour calendaire (rouge
  // payé au pre-push du 2026-08-28→29) : un gabarit bâti à 23 h 50
  // reste « frais » à 00 h 15, mais son « Aujourd'hui, 09:12 » est
  // devenu « Hier » — minuit périme, quel que soit le TTL.
  let frais = false;
  try {
    const bati = statSync(gabarit).mtimeMs;
    frais = Date.now() - bati < 30 * 60 * 1000
      && new Date(bati).toDateString() === new Date().toDateString();
  } catch {
    /* pas de gabarit : à construire */
  }
  if (!frais) {
    mkdirSync(path.dirname(gabarit), { recursive: true });
    // Construction à côté puis rename : un seed interrompu ne laisse
    // jamais un gabarit à moitié plein sous la clé finale — et ses
    // sidecars WAL non plus (des frames orphelines rejouées sur la base
    // reconstruite empoisonneraient le cache sans changer la clé).
    const chantier = `${gabarit}.chantier`;
    for (const suffixe of ['', '-wal', '-shm']) rmSync(`${chantier}${suffixe}`, { force: true });
    for (const etape of etapes) {
      execSync(`"${exe(etape.exemple)}" "${chantier}" ${etape.args}`.trim(), {
        cwd: root,
        stdio: 'inherit',
      });
    }
    renameSync(chantier, gabarit);
  }
  copyFileSync(gabarit, db);
}

// `vierge: true` : base NEUVE et aucun compte factice — l'état « zéro
// compte » qui doit montrer l'écran 01 (onboarding).
// `comptes: [{email, messages}]` : le décor seed_inbox — les parcours
// portés de v1 (R2) rejouent les graines EXACTES des specs d'origine.
// Les clés locales que les suites touchent — le profil WebView2 est
// PARTAGÉ entre suites, un run interrompu laisse son état : on purge
// avant ET après (PLAN-AUDIT-V2 E9 : cinq specs recopiaient chacune sa
// liste). Une fenêtre déjà morte n'est pas une erreur.
export const CLES_LOCALES = [
  'wind-accueil-fait',
  'wind-accueil-commence',
  'wind-volets',
  'wind-largeurs',
  'wind-theme',
  'wind-theme-auto',
  'wind-espacement',
];

export async function purgerLocales(page, cles = CLES_LOCALES) {
  await page
    .evaluate((liste) => {
      for (const cle of liste) localStorage.removeItem(cle);
    }, cles)
    .catch(() => { /* fenêtre déjà morte */ });
}

export async function launchAppV2({ vierge = false, comptes = null } = {}) {
  construireV2(root, { release: false });

  const db = path.join(
    root,
    'target',
    'e2e',
    vierge ? 'parcours-v2-vierge.db' : comptes ? 'parcours-v2-inbox.db' : 'parcours-v2.db',
  );
  // Un zombie d'une spec précédente qui tiendrait encore cette base
  // ferait un EBUSY illisible au rmSync — depuis que le build (et son
  // balayage) est mémoïsé, le lanceur balaie lui-même.
  balayerZombies(root);
  // La base ET ses sidecars : un -wal orphelin d'un run précédent collé
  // à une base fraîchement copiée serait un mensonge d'état.
  for (const suffixe of ['', '-wal', '-shm']) rmSync(`${db}${suffixe}`, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  if (vierge) {
    return attacher(db, []);
  }
  if (comptes) {
    const etapes = [];
    for (const compte of comptes) {
      // `ko: N` : chaque message porte un corps synthétique de N Ko —
      // le décor des mesures de RAM du Kiosque (terrain STOP 2
      // PLAN-AUDIT-V2 : 249 Mo après dix pages de vraies lettres).
      const lourd = compte.ko ? ` ${compte.messages} ${compte.ko}` : '';
      etapes.push({ exemple: 'seed_inbox', args: `${compte.messages} ${compte.email}${lourd}` });
      // `archives: N` : une boîte Archives de N messages, sans corps —
      // le décor du défilement profond (PLAN-DEFILEMENT-PROFOND). Le
      // seeder inscrit la boîte au cache `folders` (la canonique
      // archives résout), les dossiers Archivés/Factures des autres
      // comptes restent intacts.
      if (compte.archives) {
        etapes.push({ exemple: 'seed_inbox', args: `${compte.archives} ${compte.email} 0 0 Archives` });
      }
    }
    seeder(db, etapes);
    // `deconnecte: true` : le compte vit au REGISTRE (seedé ci-dessus)
    // mais ne reçoit pas de session — l'état « jeton mort » du réel,
    // celui que Réglages sait désormais réparer (terrain 2026-08-20).
    return attacher(
      db,
      comptes.filter((compte) => !compte.deconnecte).map((compte) => compte.email),
    );
  }
  seeder(db, [{ exemple: 'seed_clarity', args: '' }]);
  return attacher(db, ['paul.merand@atelier-nord.fr', 'paul@merand.fr']);
}

// L'ARRIVÉE de courrier en cours de spec (PLAN-MODE-ORGANISE E2) : des
// enveloppes datées de MAINTENANT entrent par le chemin de production
// (`upsert_envelopes` — la décision d'arrivée du Portier y vit), dans
// la base VIVANTE de la spec (WAL : l'app tourne, comme une synchro).
// Les exemples sont déjà compilés par `seeder` ; l'appelant recharge la
// page pour voir l'état neuf.
// ⚠️ `db` par défaut = la base du décor `comptes` (parcours-v2-inbox) —
// une spec lancée sous un AUTRE décor (Clarity, vierge) doit passer sa
// base, sinon l'arrivée part dans un fichier que l'app ne lit pas (le
// seeder sortirait vert, l'assertion rougirait sans indice).
export function injecterArrivee({ email, expediteur, n = 1, nom = null, sujet = null, reponseA = null, corps = null, db = null }) {
  db ??= path.join(root, 'target', 'e2e', 'parcours-v2-inbox.db');
  statSync(db); // la base doit EXISTER — jamais une arrivée dans le vide
  const exe = path.join(root, 'target', 'debug', 'examples', 'seed_arrivee.exe');
  const args = [`"${db}"`, email, expediteur, String(n)];
  // Les arguments sont POSITIONNELS : `reponseA` (RETOURS-14 R4, le
  // décor du fil mêlé) exige nom et sujet devant lui.
  if (nom || sujet || reponseA) args.push(`"${nom ?? expediteur}"`);
  if (sujet || reponseA) args.push(`"${sujet ?? 'Premier contact'}"`);
  if (reponseA || corps) args.push(`"${reponseA ?? '-'}"`);
  // `corps: 'images'` (terrain STOP 2 PLAN-AUDIT-V2) : un corps à image
  // distante par arrivée — le décor de la garde d'images du Kiosque.
  if (corps) args.push(corps);
  execSync(`"${exe}" ${args.join(' ')}`, { cwd: root, stdio: 'inherit' });
}

async function attacher(db, emails) {
  // Profil WebView2 explicite et inscriptible : sur un runner CI,
  // l'emplacement par défaut peut être refusé. Stable d'un lancement à
  // l'autre — un profil neuf à chaque fois rendrait chaque démarrage
  // froid, donc lent, pour rien.
  const profile = path.join(root, 'target', 'e2e', 'webview2');
  mkdirSync(profile, { recursive: true });
  purgerCacheHttp(profile);

  // Port CDP libre, choisi par l'OS — un port par SUITE, pas par
  // lancement : WebView2 partage son processus navigateur par profil, et
  // deux lancements de la même gate (même profil) doivent porter des
  // arguments navigateur IDENTIQUES — un processus attardé aux options
  // différentes ferait échouer la création d'environnement. Entre
  // worktrees, chaque suite est un processus Node distinct : ports
  // distincts, plus aucun état partagé (constat 2026-08-15, port-cdp.mjs).
  const port = (portSuite ??= await allouerPortCdp());

  const env = {
    ...process.env,
    WIND_DB_PATH: db,
    // `--lang=fr` : la détection de langue au premier lancement
    // (navigator.language, PLAN-LANGUES) lit la locale du WebView — sans
    // cette épingle, la suite dépendrait de la langue de la machine.
    // Le français reste la langue canonique des parcours (L-6).
    // Les arguments de PRODUCTION (tauri.conf.json) + le port CDP + la
    // langue épinglée, composés par args-navigateur.mjs — la variable
    // ÉCRASE la conf au niveau du loader WebView2, elle doit donc la
    // reprendre pour que l'e2e voie le navigateur livré (revue
    // 2026-08-16). WIND_E2E_ARGS_EXTRA : passe-plat des bancs de mesure
    // (E4, mesure-scrollbar.mjs) — un flag posé dans l'environnement du
    // parent n'atteindrait jamais le WebView2 sans lui.
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: argsNavigateur(
      root,
      port,
      process.env.WIND_E2E_ARGS_EXTRA ?? '',
    ),
    WEBVIEW2_USER_DATA_FOLDER: profile,
  };
  if (emails.length > 0) env.WIND_E2E_ACCOUNT = emails.join(',');
  else delete env.WIND_E2E_ACCOUNT;
  purgerOAuth(env);

  const app = spawn(path.join(root, 'target', 'debug', 'wind-desktop.exe'), [], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  // Le journal de l'application est notre seule fenêtre sur un échec de
  // démarrage : on le collecte dès la première ligne.
  let log = '';
  app.stdout.on('data', (chunk) => {
    log += chunk;
  });
  app.stderr.on('data', (chunk) => {
    log += chunk;
  });
  let exited = null;
  app.on('exit', (code, signal) => {
    exited = { code, signal };
  });

  // On attend que la PAGE de l'application soit là. On s'arrête net si le
  // processus meurt : inutile d'attendre 90 s un CDP qui ne viendra jamais.
  let browser = null;
  let page = null;
  const deadline = Date.now() + READY_TIMEOUT_MS;
  while (!page && !exited && Date.now() < deadline) {
    try {
      browser ??= await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      page =
        browser
          .contexts()
          .flatMap((context) => context.pages())
          .find((candidate) => candidate.url().includes('tauri.localhost')) ?? null;
    } catch {
      // Ni le port ni la page ne sont prêts : on repasse.
    }
    if (!page) await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }

  if (!page) {
    if (browser) await browser.close().catch(() => {});
    app.kill();
    throw new Error(startupFailure(exited, browser !== null, log, port));
  }
  return { app, browser, page };
}

/// Message d'échec qui DIT pourquoi : processus mort (avec son code), port
/// muet, ou page jamais créée — et dans tous les cas la sortie réelle de
/// l'application.
function startupFailure(exited, connected, log, port) {
  let cause;
  if (exited) {
    cause = `l'application s'est arrêtée au démarrage (code ${exited.code}, signal ${exited.signal})`;
  } else if (connected) {
    cause = `CDP joignable sur le port ${port}, mais aucune page « tauri.localhost » après ${READY_TIMEOUT_MS / 1000} s`;
  } else {
    cause = `CDP injoignable sur le port ${port} après ${READY_TIMEOUT_MS / 1000} s`;
  }
  const output = log.trim();
  return output
    ? `${cause}\n--- sortie de l'application ---\n${output}\n--- fin ---`
    : `${cause}\n(l'application n'a rien écrit sur sa sortie)`;
}

export async function closeApp({ app, browser }) {
  if (browser) await browser.close().catch(() => {});
  if (app) {
    // Attendre la sortie RÉELLE : un second lancement dans la même gate
    // (écran 01 sur base vierge) réutilise le port de la suite et le
    // profil WebView2 — les reprendre à un processus encore vivant est
    // une course.
    const fini = new Promise((resolve) => {
      if (app.exitCode !== null) resolve();
      else app.once('exit', resolve);
    });
    app.kill();
    await fini;
  }
}
