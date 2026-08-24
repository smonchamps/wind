// Gate de cohérence du Système (PLAN-DC E3, décision DC-D6) : le
// document normatif (docs/design/systeme.dc.html) ne doit jamais
// dériver des valeurs livrées (apps/desktop/ui-v2/src/systeme.css).
//
//   node coherence-systeme.mjs   -> écarts nommés + verdict
//
// Vérifications :
//   1. La table du contrat des jetons (cellules data-theme/data-jeton)
//      égale les :root de systeme.css, VALEUR POUR VALEUR, dans les
//      deux sens — un jeton du CSS absent du doc est un échec autant
//      qu'une valeur fausse, et une cellule orpheline (jeton mort au
//      CSS) autant qu'une cellule manquante.
//   2. Le journal des amendements est présent. (Les anciens contrôles
//      « aucun compteur de glyphes hors journal » et « renvoi à
//      assets/icones/README.md » sont morts avec la fonte : depuis V8
//      le relevé de la section Icônes EST l'inventaire — le doc dit
//      « 78 glyphes » et il a raison de le dire.)
//   3. Les pastilles de FICHES (lib/theme.js) égalent les jetons
//      [accent, bg, border, surface, ink] du CSS — les vignettes du
//      sélecteur Réglages montrent chaque thème sans l'appliquer, la
//      copie ne doit jamais dériver (revue A42). `panel` est mort (V3).
//
// Le remède est toujours le même : amender le Système dans le commit
// fautif (DC-D2) — jamais tordre la gate.
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { lireThemes, NOMBRE_ATTENDU } from './jetons.mjs';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'systeme.css'),
  'utf8',
);
const doc = readFileSync(
  path.join(root, 'docs', 'design', 'systeme.dc.html'),
  'utf8',
);

let echecs = 0;
const echec = (message) => {
  echecs += 1;
  console.log(`ECHEC ${message}`);
};

// --- 1. Les jetons : systeme.css d'un côté… --------------------------
// Le parseur partagé des deux gates (jetons.mjs — né à la revue d'A42,
// après deux corrections du même bogue en deux exemplaires) ; le
// plancher NOMBRE_ATTENDU ferme le silence d'un bloc non reconnu.
const themesCss = lireThemes(css);
if (Object.keys(themesCss).length !== NOMBRE_ATTENDU) {
  echec(`${Object.keys(themesCss).length} thème(s) extraits de systeme.css — ${NOMBRE_ATTENDU} attendus (jetons.mjs) : un bloc échappe au motif, ou la table a changé sans amender le plancher`);
}

// --- …la table du contrat du doc de l'autre --------------------------
const themesDoc = {};
for (const [, theme, jeton, contenu] of doc.matchAll(
  /<td data-theme="([a-z-]+)" data-jeton="([a-zA-Z0-9]+)"[^>]*>([^<]*)<\/td>/g,
)) {
  (themesDoc[theme] ??= {})[jeton] = contenu.replace(/\s+/g, ' ').trim();
}

if (Object.keys(themesDoc).length === 0) {
  echec('la table du contrat des jetons est introuvable dans le doc (cellules data-theme/data-jeton)');
}

for (const [nom, jetonsCss] of Object.entries(themesCss)) {
  const jetonsDoc = themesDoc[nom] ?? {};
  for (const [jeton, valeurCss] of Object.entries(jetonsCss)) {
    if (!(jeton in jetonsDoc)) {
      echec(`${nom} · --${jeton} : livré « ${valeurCss} », absent du doc`);
    } else if (jetonsDoc[jeton] !== valeurCss) {
      echec(`${nom} · --${jeton} : doc « ${jetonsDoc[jeton]} », livré « ${valeurCss} »`);
    }
  }
  for (const jeton of Object.keys(jetonsDoc)) {
    if (!(jeton in jetonsCss)) {
      echec(`${nom} · --${jeton} : au doc mais mort dans systeme.css`);
    }
  }
}
for (const nom of Object.keys(themesDoc)) {
  if (!(nom in themesCss)) {
    echec(`thème « ${nom} » : au doc mais absent de systeme.css (thème fantôme — DC-D5)`);
  }
}

// --- 2. Le journal des amendements est présent -----------------------
if (!doc.includes('Journal des amendements')) {
  echec('la section « Journal des amendements » est introuvable');
}

// --- 3. Les pastilles de FICHES disent les jetons livrés -------------
const themeJs = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'theme.js'),
  'utf8',
);
const ROLES_PASTILLES = ['accent', 'bg', 'border', 'surface', 'ink'];
const fiches = [...themeJs.matchAll(/\{ id: '([a-z-]+)', pastilles: \[([^\]]*)\] \}/g)];
if (fiches.length !== NOMBRE_ATTENDU) {
  echec(`${fiches.length} fiche(s) lues dans lib/theme.js — ${NOMBRE_ATTENDU} attendues : FICHES a changé de forme, ou une fiche manque`);
}
for (const [, id, brut] of fiches) {
  const pastilles = [...brut.matchAll(/'([^']+)'/g)].map(([, v]) => v);
  const jetons = themesCss[id];
  if (!jetons) {
    echec(`fiche « ${id} » : au sélecteur mais absente de systeme.css`);
    continue;
  }
  ROLES_PASTILLES.forEach((role, i) => {
    if (pastilles[i] !== jetons[role]) {
      echec(`fiche « ${id} » · pastille ${role} : theme.js « ${pastilles[i]} », livré « ${jetons[role]} »`);
    }
  });
}

// --- 4. Les catalogues nomment chaque thème livré --------------------
// Revue A42 : la parité fr↔en (refonte-langue.spec) ne dit pas qu'un
// thème LIVRÉ a son libellé — un id renommé sans les catalogues rendait
// la clé brute `theme.<id>.nom` sur la carte, en vert partout.
for (const langue of ['fr', 'en']) {
  const cat = readFileSync(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', `catalogue.${langue}.js`),
    'utf8',
  );
  const ids = new Set(
    [...cat.matchAll(/'theme\.([a-z-]+)\.nom'/g)].map(([, id]) => id),
  );
  for (const id of Object.keys(themesCss)) {
    if (!ids.has(id)) {
      echec(`catalogue.${langue} : theme.${id}.nom manquant — la carte du sélecteur afficherait la clé brute`);
    }
  }
  for (const id of ids) {
    if (!themesCss[id]) {
      echec(`catalogue.${langue} : theme.${id}.nom sans thème livré dans systeme.css`);
    }
  }
}

// --- 5. Aucune règle de barre de défilement (A44) — (numérotation
//     remise d'équerre à E1 de PLAN-ELEMENTS : deux « 5 » cohabitaient)
// Les barres sont NATIVES en surimpression (OverlayScrollbar) : UNE
// seule règle `::-webkit-scrollbar` / `scrollbar-width` /
// `scrollbar-color` fait retomber l'élément sur le chemin classique et
// lui rend ~15 px de gouttière — la régression est silencieuse à
// l'œil des tests. Les commentaires sont retirés avant l'examen (le
// commentaire d'A44 dans systeme.css nomme précisément ces règles).
const srcUi = path.join(root, 'apps', 'desktop', 'ui-v2', 'src');
const fichiersUi = (dossier) =>
  readdirSync(dossier, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory()
      ? fichiersUi(path.join(dossier, e.name))
      : /\.(css|svelte)$/.test(e.name)
        ? [path.join(dossier, e.name)]
        : [],
  );
for (const fichier of fichiersUi(srcUi)) {
  const sansCommentaires = readFileSync(fichier, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/^\s*\/\/.*$/gm, '');
  const interdit = sansCommentaires.match(
    /::-webkit-scrollbar|scrollbar-width\s*:|scrollbar-color\s*:/,
  );
  if (interdit) {
    echec(`${path.relative(root, fichier)} : règle « ${interdit[0]} » — les barres sont natives en surimpression (A44), cette règle rend l'élément au chemin classique (~15 px de gouttière)`);
  }
}

// --- 6. Le jeu dédié des repères : UNE liste, quatre porteurs --------
// (PLAN-RETOURS-8, revue 2026-08-22) : l'allowlist Rust (commands.rs,
// elle fait foi à l'écriture), lib/reperes.js (ce que l'UI propose),
// les teintes de systeme.css (ce qui se dessine) et les catalogues
// (libellés). Une dérive = un repère proposé mais refusé, ou stocké
// mais rendu sans couleur — toujours en silence.
const commandsRs = readFileSync(
  path.join(root, 'apps', 'desktop', 'src', 'commands.rs'),
  'utf8',
);
const reperesJs = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'reperes.js'),
  'utf8',
);
const listeRust = (nom) => [
  ...(commandsRs.match(new RegExp(`const ${nom}[^=]*= \\[([^;]*)\\];`))?.[1] ?? '')
    .matchAll(/"([a-z_]+)"/g),
].map(([, v]) => v);
const listeJs = (nom) => [
  ...(reperesJs.match(new RegExp(`export const ${nom} = \\[([^\\]]*)\\]`))?.[1] ?? '')
    .matchAll(/'([a-z_]+)'/g),
].map(([, v]) => v);
const compareListes = (quoi, a, aNom, b, bNom) => {
  if (a.length === 0) echec(`repères : liste ${quoi} introuvable dans ${aNom}`);
  for (const v of a) {
    if (!b.includes(v)) echec(`repères : ${quoi} « ${v} » dans ${aNom} mais pas dans ${bNom}`);
  }
  for (const v of b) {
    if (!a.includes(v)) echec(`repères : ${quoi} « ${v} » dans ${bNom} mais pas dans ${aNom}`);
  }
};
const iconesRust = listeRust('REPERE_ICONES');
const teintesRust = listeRust('REPERE_TEINTES');
compareListes('icône', iconesRust, 'commands.rs', listeJs('REPERE_ICONES'), 'lib/reperes.js');
compareListes('teinte', teintesRust, 'commands.rs', listeJs('REPERE_TEINTES'), 'lib/reperes.js');
const teintesCss = [
  ...new Set([...css.matchAll(/\.repere\[data-teinte="([a-z]+)"\]/g)].map(([, v]) => v)),
];
compareListes('teinte', teintesRust, 'commands.rs', teintesCss, 'systeme.css');
for (const langue of ['fr', 'en']) {
  const cat = readFileSync(
    path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', `catalogue.${langue}.js`),
    'utf8',
  );
  for (const icone of iconesRust) {
    if (!cat.includes(`'repere.icone.${icone}'`)) {
      echec(`catalogue.${langue} : repere.icone.${icone} manquant — la carte du choix afficherait la clé brute en infobulle`);
    }
  }
  for (const teinte of teintesRust) {
    if (!cat.includes(`'repere.teinte.${teinte}'`)) {
      echec(`catalogue.${langue} : repere.teinte.${teinte} manquant`);
    }
  }
}

// --- 7. Les icônes : le relevé du Système EST l'inventaire (A18, V8) --
// Depuis V8 la fonte est morte : les 78 glyphes vivent en SVG dans
// lib/icones.js, et le Système les dessine. Trois assertions, qui
// reprennent la garde du générateur du v2 :
//   a. les noms du catalogue == les figcaptions de la grille du doc,
//      dans les DEUX sens (l'écart des dix glyphes d'avant V8 ne peut
//      pas renaître) ;
//   b. chaque tracé (`d`) du catalogue apparaît dans le doc — un
//      dessin changé au code sans amender le Système crie ;
//   c. chaque nom d'icône posé en dur dans un composant existe au
//      catalogue et n'est pas réservé (A53/A60/A62 : un réservé ne se
//      pose pas).
const iconesJs = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'lib', 'icones.js'),
  'utf8',
);
const nomsCatalogue = [...iconesJs.matchAll(/^ {2}([a-z_0-9]+): *\{/gm)].map(([, n]) => n);
const reservesCatalogue = new Set(
  [...iconesJs.matchAll(/^ {2}([a-z_0-9]+): *\{[^\n]*\br:true/gm)].map(([, n]) => n),
);
const nomsDoc = [...new Set(
  [...doc.matchAll(/<figcaption>([a-z_0-9]+)<\/figcaption>/g)].map(([, n]) => n),
)];
if (nomsCatalogue.length === 0) echec('lib/icones.js : aucun glyphe lu — le catalogue a changé de forme');
if (nomsDoc.length === 0) echec('le doc ne porte aucune grille de glyphes (figcaption) — la section Icônes a changé de forme');
for (const n of nomsCatalogue) {
  if (!nomsDoc.includes(n)) echec(`icône « ${n} » : au catalogue (lib/icones.js) mais absente de la grille du Système`);
}
for (const n of nomsDoc) {
  if (!nomsCatalogue.includes(n)) echec(`icône « ${n} » : dessinée au Système mais absente du catalogue livré`);
}
for (const [, nom, brut] of iconesJs.matchAll(/^ {2}([a-z_0-9]+): *\{ *d:\[([^\]]*)\]/gm)) {
  for (const [, d] of brut.matchAll(/'([^']+)'/g)) {
    if (!doc.includes(d)) {
      echec(`icône « ${nom} » : le tracé « ${d} » du catalogue n'apparaît pas dans le Système — dessin changé sans amender le doc (DC-D2)`);
    }
  }
}
for (const fichier of fichiersUi(srcUi).filter((f) => f.endsWith('.svelte'))) {
  const source = readFileSync(fichier, 'utf8');
  for (const [, nom] of source.matchAll(/<Icone[^>]*\bnom="([a-z_0-9]+)"/g)) {
    if (!nomsCatalogue.includes(nom)) {
      echec(`${path.relative(root, fichier)} : icône « ${nom} » posée mais absente du catalogue`);
    } else if (reservesCatalogue.has(nom)) {
      echec(`${path.relative(root, fichier)} : icône « ${nom} » posée mais RÉSERVÉE (A53/A60/A62)`);
    }
  }
}

// --- 8. Zéro rayon : plus un littéral de border-radius (V14) ---------
// Les trois jetons de forme (--r-surface, --r-controle, --r-tuile)
// valent 0 et vivent sur `html` (pas :root — le contrat des jetons de
// couleur ne s'en gonfle pas). Restent DEUX formes rondes qui disent
// quelque chose : le disque (50 % — l'état, l'identité, la poignée
// d'interrupteur) et la pilule de la piste d'interrupteur (999px).
// Tout autre littéral est un écart — le rembobinage de V14 tient en
// une ligne PARCE QUE tout passe par les jetons.
for (const jeton of ['--r-surface', '--r-controle', '--r-tuile']) {
  if (!new RegExp(`html\\s*\\{[^}]*${jeton}:0`).test(css)) {
    echec(`systeme.css : le jeton de forme ${jeton}:0 manque sur html (V14)`);
  }
}
for (const fichier of fichiersUi(srcUi)) {
  const source = readFileSync(fichier, 'utf8')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/^\s*\/\/.*$/gm, '');
  for (const [brut, valeur] of source.matchAll(/border-radius:\s*([^;}]+)/g)) {
    const v = valeur.trim();
    if (v === 'var(--r-surface)' || v === 'var(--r-controle)' || v === 'var(--r-tuile)'
      || v === '50%' || v === '999px') continue;
    echec(`${path.relative(root, fichier)} : « ${brut.trim()} » — V14 : zéro rayon, tout littéral passe par un jeton de forme (ou 50 % / 999px pour le disque et la piste)`);
  }
}

const nbThemes = Object.keys(themesCss).length;
const nbJetons = Object.values(themesCss).reduce((n, t) => n + Object.keys(t).length, 0);
console.log(
  echecs === 0
    ? `Tout concorde — ${nbThemes} thèmes, ${nbJetons} valeurs de jetons, le doc dit le livré.`
    : `${echecs} écart(s) entre le Système et le livré. Le remède : amender le Système dans le commit fautif (DC-D2).`,
);
process.exitCode = echecs === 0 ? 0 : 1;
