// Banc de contraste du spike « direction Elements » — MÊME banc que la
// gate expédiée (e2e/contraste.mjs) : mêmes formules WCAG, même table de
// paires (les paires RÉELLEMENT posées par ui-v2), appliquée aux jetons
// de la direction. Un jeton qui ne passe pas ici ne passerait pas la
// gate : la direction se mesure avant de se dessiner.
//
//   node contraste.mjs
//
// Ajouts propres au spike : le rôle --marque (le teal #1F8A8A du jeu
// d'icônes, COMPOSANT seulement) et les paires sur --tuile, qui devient
// un fond de rangée (épinglés) et non plus la seule tuile de la nav.

function lum(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
const rapport = (a, b) => {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

const THEMES = {
  'elements': {
    bg:'#F3F2EE', panel:'#F3F2EE', surface:'#FFFFFF',
    ink:'#191D1E', ink2:'#565C5E', muted:'#606668',
    border:'#CBC8BB', accent:'#1A7A7A', accentH:'#14625E',
    marque:'#1F8A8A', sel:'#DDE9E6', hover:'#EAE8E1',
    alert:'#C42D24', onAccent:'#FFFFFF',
    tuile:'#F2EDE3', tuileInk:'#4A4436',
  },
  'elements-nuit': {
    bg:'#0D100F', panel:'#0D100F', surface:'#171B1A',
    ink:'#ECEDEA', ink2:'#B4BAB8', muted:'#98A0A1',
    border:'#333B3A', accent:'#3FA39C', accentH:'#55B7B0',
    marque:'#3FA39C', sel:'#1E322F', hover:'#141817',
    alert:'#EA9A90', onAccent:'#06211F',
    tuile:'#241F17', tuileInk:'#DFCFAE',
  },
};

// [encre, fond, seuil, où] — la table de la gate, plus les rôles du spike.
const PAIRES = [
  ['ink', 'bg', 4.5, 'titres, objets de liste'],
  ['ink', 'surface', 4.5, 'cartes de message, champs'],
  ['ink', 'panel', 4.5, 'nav, entete, barre de statut'],
  ['ink', 'sel', 4.5, 'rangee selectionnee'],
  ['ink', 'hover', 4.5, 'rangee survolee'],
  ['ink2', 'bg', 4.5, 'expediteurs, corps de texte'],
  ['ink2', 'surface', 4.5, 'puces, boutons secondaires'],
  ['ink2', 'panel', 4.5, 'nav (libelles)'],
  ['ink2', 'sel', 4.5, 'apercus (rangee selectionnee)'],
  ['ink2', 'hover', 4.5, 'apercus (rangee survolee)'],
  ['muted', 'bg', 4.5, 'heures, apercus, statut'],
  ['muted', 'surface', 4.5, 'kickers, descriptions, placeholder'],
  ['muted', 'panel', 4.5, 'statut, sections nav'],
  ['muted', 'sel', 4.5, 'heures (rangee selectionnee)'],
  ['muted', 'hover', 4.5, 'heures (rangee survolee)'],
  ['muted', 'tuile', 4.5, 'heures (rangee epinglee)'],
  ['onAccent', 'accent', 4.5, 'bouton Ecrire, pastilles pleines'],
  ['onAccent', 'accentH', 4.5, 'bouton Ecrire (survol)'],
  ['alert', 'bg', 4.5, "texte d'erreur, mention Brouillon"],
  ['alert', 'sel', 4.5, 'mention Brouillon (rangee choisie)'],
  ['alert', 'hover', 4.5, 'mention Brouillon (rangee survolee)'],
  ['alert', 'surface', 4.5, 'mention Brouillon (cartes)'],
  ['alert', 'surface', 3, "icone d'alerte"],
  ['accent', 'surface', 3, 'icones, coche, anneau de focus'],
  ['accent', 'bg', 3, 'anneau de focus, filets porteurs'],
  ['accent', 'panel', 3, 'indicateur de la barre de statut'],
  ['accent', 'sel', 3, 'lisere de la ligne choisie'],
  ['accent', 'surface', 4.5, 'liens et libelles en accent (TEXTE)'],
  ['accent', 'bg', 4.5, 'libelles en accent sur le fond (TEXTE)'],
  ['tuileInk', 'tuile', 4.5, 'la rangee epinglee / la boite en cours'],
  // --- rôles propres à la direction Elements -------------------------
  ['marque', 'bg', 3, 'DISQUE de non-lu (composant) sur le fond'],
  ['marque', 'surface', 3, 'DISQUE sur une carte'],
  ['marque', 'sel', 3, 'DISQUE sur la rangee choisie'],
  ['marque', 'hover', 3, 'DISQUE sur la rangee survolee'],
  ['marque', 'tuile', 3, 'DISQUE sur la rangee epinglee'],
  ['border', 'bg', 1.49, 'filet sur le fond — seuil = le filet SHIPPE de Clarity (1,49 clair / 1,59 nuit)'],
  ['border', 'surface', 1.26, 'filet sur une carte — seuil = le filet SHIPPE (1,70 clair / 1,27 nuit)'],
];

let echecs = 0, mesures = 0;
for (const [nom, t] of Object.entries(THEMES)) {
  for (const [encre, fond, seuil, ou] of PAIRES) {
    const r = rapport(t[encre], t[fond]);
    mesures += 1;
    const ok = r >= seuil;
    if (!ok) echecs += 1;
    if (!ok || process.argv.includes('--tout')) {
      console.log(
        `${ok ? 'ok   ' : 'ECHEC'} ${nom.padEnd(15)} ${encre}/${fond}`.padEnd(46) +
        `${r.toFixed(2)}:1 (seuil ${seuil}) — ${ou}`,
      );
    }
  }
}
console.log(`\n${mesures} mesures, ${echecs} echec(s)`);

// --- La palette de la SUITE comme marqueur de compte -------------------
// Le jeu d'icônes donne six teintes. Un disque de compte est un COMPOSANT
// (seuil 3:1) posé sur les cinq fonds de rangée. Mesure : cinq tiennent,
// Helios ne tient pas — le jaune ne survit pas hors de sa tuile, où le
// #141414 qui l'entoure lui fabrique son contraste.
const SUITE = {
  Wind:'#1F8A8A', Stone:'#B0703C', River:'#2153A0',
  Flame:'#D8332A', Helios:'#E0AE1C', Moon:'#6C4E9C',
};
console.log('\n--- palette de la suite en DISQUE de compte (composant, 3:1) ---');
for (const [nom, hex] of Object.entries(SUITE)) {
  for (const [theme, t] of Object.entries(THEMES)) {
    const fonds = ['bg', 'surface', 'sel', 'hover', 'tuile'];
    const min = Math.min(...fonds.map((f) => rapport(hex, t[f])));
    console.log(`  ${nom.padEnd(7)} ${hex} ${theme.padEnd(15)} pire fond ${min.toFixed(2)}:1 ${min >= 3 ? 'ok' : 'ECHEC — a exclure du nuancier de compte'}`);
  }
}

process.exit(echecs ? 1 : 0);
