// Reconstruire l'application — le point UNIQUE qui connaît les trois
// pièges MESURÉS du banc de la refonte. Chaque piège a coûté une
// session de fantômes ; ils vivent ici, une fois.
//
// Depuis B2 (PLAN-RETRAIT-V1), ui-v2 est la SEULE interface : plus
// aucun échange de dist — il ne reste que la taille de fenêtre du banc
// de parité.
import { execSync } from 'node:child_process';
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';

// Empreinte du dist embarqué + de la conf Tauri : chemins relatifs ET
// contenus (les assets de Vite sont hashés dans leur NOM — un rename
// seul doit suffire à invalider). Déterministe : parcours trié.
export function empreinteDist(distDir, conf) {
  const hash = createHash('sha1');
  const marcher = (dir) => {
    for (const entree of readdirSync(dir, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const abs = path.join(dir, entree.name);
      if (entree.isDirectory()) {
        marcher(abs);
      } else {
        hash.update(path.relative(distDir, abs).replaceAll('\\', '/'));
        hash.update('\0');
        hash.update(readFileSync(abs));
        hash.update('\0');
      }
    }
  };
  marcher(distDir);
  hash.update(conf);
  return hash.digest('hex');
}

// Tuer les instances de banc survivantes — QUE celles issues de CE
// target/ : jamais l'application installée de l'utilisateur, et jamais
// la suite d'un AUTRE worktree (le motif '*\target\*' d'origine
// abattait l'application de l'autre suite en plein vol — Stop-Process
// -Force = code 0xFFFFFFFF sans sortie, constat du 2026-08-15,
// PLAN-ISOLATION-E2E). Exporté : depuis que le build est mémoïsé, le
// lanceur doit balayer lui-même avant de reprendre la base d'une spec
// précédente — un zombie qui tient wind.db ferait un EBUSY illisible
// à la place d'un flake propre.
export function balayerZombies(root) {
  const balayage =
    'Get-Process wind-desktop -ErrorAction SilentlyContinue | '
    + `Where-Object { $_.Path -like '${path.join(root, 'target')}\\*' } | Stop-Process -Force`;
  try {
    execSync(`powershell -NoProfile -Command "${balayage}"`, { stdio: 'ignore' });
  } catch {
    /* rien à tuer */
  }
}

function construire(root, { release, fenetre }) {
  // 1. `generate_context!` n'embarque le dist qu'à la COMPILATION de
  //    main.rs : un changement d'assets SEULS ne recompile rien, et le
  //    binaire garderait un dist périmé (constaté : règle CSS présente
  //    sur disque, absente des feuilles chargées). Le bump de mtime
  //    force la ré-expansion — mais seulement si le dist ou la conf ont
  //    RÉELLEMENT changé depuis le dernier build : bumper à chaque
  //    lancement payait un link complet par spec, même à vide
  //    (PLAN-KAIZEN-CLAUDE vague 2, E1 — ~74 s la spec, dominés par le
  //    rebuild).
  const empreinte = empreinteDist(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'dist'),
    readFileSync(path.join(root, 'apps', 'desktop', 'tauri.conf.json'), 'utf8') +
      JSON.stringify(fenetre),
  );
  const memoire = path.join(
    root,
    'target',
    'e2e',
    `empreinte-rebuild-${release ? 'release' : 'debug'}.txt`,
  );
  let stockee = null;
  try {
    stockee = readFileSync(memoire, 'utf8');
  } catch {
    /* premier build : pas d'empreinte */
  }
  if (stockee !== empreinte) {
    utimesSync(path.join(root, 'apps', 'desktop', 'src', 'main.rs'), new Date(), new Date());
  }
  // 2. Un zombie de banc verrouille l'exe : le LINK échoue (« accès
  //    refusé ») et le vieux binaire serait rejoué en silence
  //    (constaté).
  balayerZombies(root);
  // 3. Échange de conf éventuel (taille de fenêtre du banc de parité) :
  //    RESTAURÉ même sur échec — le dépôt ne reste jamais sale.
  const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
  const commande = `cargo build -p wind-desktop${release ? ' --release' : ''}`;
  if (!fenetre) {
    execSync(commande, { cwd: root, stdio: 'inherit' });
  } else {
    const origine = readFileSync(conf, 'utf8');
    const modifiee = JSON.parse(origine);
    Object.assign(modifiee.app.windows[0], fenetre);
    try {
      writeFileSync(conf, JSON.stringify(modifiee, null, 2));
      execSync(commande, { cwd: root, stdio: 'inherit' });
    } finally {
      writeFileSync(conf, origine);
    }
  }
  // L'empreinte ne s'écrit qu'APRÈS un build réussi : un build interrompu
  // ne doit pas faire croire au prochain lancement que le binaire est bon.
  mkdirSync(path.dirname(memoire), { recursive: true });
  writeFileSync(memoire, empreinte);
}

// Mémo par PROCESSUS de suite : Playwright réutilise son worker d'un
// fichier de spec à l'autre (workers: 1) — la première spec paie le
// build, les suivantes ne paient rien du tout, pas même le `npm run
// build` de Vite (~3 s) ni le no-op cargo. Un worker relancé (après un
// échec) repasse ici : le build Vite se rejoue, et l'empreinte sur
// disque évite alors le bump — cargo ne fait que vérifier.
const dejaConstruits = new Set();

export function construireV2(root, { release = true, fenetre = null } = {}) {
  const cle = `${root}|${release}|${JSON.stringify(fenetre)}`;
  if (dejaConstruits.has(cle)) return;
  execSync('npm run build', {
    cwd: path.join(root, 'apps', 'desktop', 'ui-v2'),
    stdio: 'inherit',
  });
  construire(root, { release, fenetre });
  dejaConstruits.add(cle);
}

// Le cache HTTP du profil WebView2 survit aux rebuilds et peut servir un
// index.html périmé avec ses vieux assets hashés — styles fantômes, CSP
// d'une autre époque (constaté). On purge le CACHE, pas le profil : le
// cache GPU qui rend les démarrages tièdes reste.
export function purgerCacheHttp(profile) {
  for (const dossier of ['Cache', 'Code Cache']) {
    rmSync(path.join(profile, 'EBWebView', 'Default', dossier), {
      recursive: true,
      force: true,
    });
  }
}
