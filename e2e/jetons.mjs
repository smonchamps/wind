// Lecture des jetons de systeme.css — le parseur UNIQUE des deux gates
// (contraste.mjs, coherence-systeme.mjs). Né à la revue d'A42 : chacune
// portait sa copie de la même regex, et le même bogue s'y est corrigé
// deux fois de suite (la classe [a-zA-Z] qui perdait « ink2 » à
// PLAN-DC E3, puis [a-z]+ qui laissait les 14 thèmes -nuit sans AUCUNE
// mesure). Une seule implémentation, un seul endroit à corriger.
//
// NOMBRE_ATTENDU est le plancher de complétude : un bloc que le motif
// ne reconnaît pas n'est pas une paire à sauter, c'est la gate qui
// devient aveugle en silence. À mettre à jour avec la table, dans le
// même commit (DC-D2) — l'écart est bruyant, c'est le but.
export const NOMBRE_ATTENDU = 4;

// [a-z-]+ : la déclinaison sombre porte un trait d'union
// (« elements-nuit », V7). [a-zA-Z][a-zA-Z0-9]* côté jeton : « ink2 »
// porte un chiffre. `motifValeur` : contraste.mjs n'extrait que les
// hex #RRGGBB (seuls mesurables), coherence-systeme.mjs prend toute
// valeur (ombres, scrims) — le défaut.
export function lireThemes(css, { motifValeur = /[^;]+/ } = {}) {
  const themes = {};
  for (const [, nom, corps] of css.matchAll(
    /:root(?:\[data-theme="([a-z-]+)"\])?\s*\{([^}]+)\}/g,
  )) {
    const jetons = {};
    for (const [, cle, valeur] of corps.matchAll(
      new RegExp(`--([a-zA-Z][a-zA-Z0-9]*)\\s*:\\s*(${motifValeur.source})`, 'g'),
    )) {
      jetons[cle] = valeur.replace(/\s+/g, ' ').trim();
    }
    if (Object.keys(jetons).length > 0) themes[nom ?? 'elements'] = jetons;
  }
  return themes;
}

// A94 (revue PLAN-MONA) — le second motif partagé : les blocs
// --rep-<teinte> par polarité. La table claire des repères vit sous
// `:root[data-theme$="-nuit"]` (servie à TOUT thème sombre, comme le
// color-scheme d'A44), la sombre sous les :root nus. Chaque gate
// portait sa copie de cette regex et le diff A94 a dû les éditer en
// lockstep — le bogue de naissance de ce fichier, rejoué ; désormais
// UNE implémentation. Retourne {teinte: hex} de la polarité demandée
// (contraste.mjs mesure les hex, coherence-systeme.mjs ne prend que
// les noms).
export function lireReperes(css, { nuit }) {
  const reperes = {};
  for (const [, theme, corps] of css.matchAll(
    /:root(\[data-theme\$="-nuit"\])?\s*\{([^}]+)\}/g,
  )) {
    if (Boolean(theme) !== nuit) continue;
    for (const [, teinte, hex] of corps.matchAll(
      /--rep-([a-z]+)\s*:\s*(#[0-9a-fA-F]{6})/g,
    )) {
      reperes[teinte] = hex;
    }
  }
  return reperes;
}
