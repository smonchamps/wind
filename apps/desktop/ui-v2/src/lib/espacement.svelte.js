// Espacement des rangées de la liste (PLAN-ESPACEMENT, décision CE D1
// du 2026-08-25) : trois crans d'air entre les messages — « Faible »
// (l'existant AU PIXEL PRÈS), « Moyen », « Élevé ». Préférence pure UI,
// le patron de volets.svelte.js : localStorage, restauration AVANT le
// premier rendu, valeur inconnue → défaut.
//
// Le shell Rust n'a rien à en lire : c'est un choix d'affichage, il ne
// va donc JAMAIS en base (la règle est écrite en tête de
// volets.svelte.js, et A26 est le précédent exact — la Disposition).
//
// `$state` partagé : la liste qui lit `padRangee()` se re-rend au
// changement, comme pour les largeurs de volets.
//
// LES VALEURS SONT DU PADDING VERTICAL, et ce n'est pas un détail
// d'implémentation : `offsetHeight` — dont TOUT le fenêtrage dépend —
// ne voit que la boîte de bordure. Une marge ou un `row-gap` donneraient
// 12,375 px par rangée INVISIBLES à la sonde : le rendu mentirait alors
// que la sonde dirait vrai. Ni marge, ni gap. Jamais.
//
// Le delta est arithmétique : +6 px de padding = +12 px de rangée.
// Rangées mesurées (banc, msedge) : 88 / 100 / 112 px nues.
const CLE = 'wind-espacement';
export const DEFAUT = 'faible';
export const CRANS = {
  faible: 13,
  moyen: 19,
  eleve: 25,
};
// L'ordre d'affichage du sélecteur (Réglages > Affichage).
export const NIVEAUX = ['faible', 'moyen', 'eleve'];

const etat = $state({ niveau: DEFAUT });

export function espacementActuel() {
  return etat.niveau;
}

// Le padding vertical du cran courant, en px — ce que la liste pose
// dans son jeton `--rangee-pad`.
export function padRangee() {
  // `NIVEAUX.includes` plutôt qu'un `??` sur l'indexation : une clé de
  // prototype rendrait une fonction, que le CSS accepterait comme
  // valeur invalide — padding 0, liste écrasée, et aucune erreur.
  return NIVEAUX.includes(etat.niveau) ? CRANS[etat.niveau] : CRANS[DEFAUT];
}

// La garde passe par la LISTE, jamais par `in` : `'toString' in CRANS`
// vaut vrai (l'opérateur remonte la chaîne de prototypes), et un
// localStorage tripoté rendrait alors une fonction en guise de padding.
// C'est aussi le patron de volets.svelte.js, dont ce module est le
// calque — la garde y est un `includes` (revue du 2026-08-25).
export function appliquerEspacement(niveau) {
  if (!NIVEAUX.includes(niveau)) return;
  etat.niveau = niveau;
  try {
    localStorage.setItem(CLE, niveau);
  } catch { /* stockage indisponible : le choix ne survivra pas, rien d'autre à faire */ }
}

// Restaure AVANT le premier rendu (pas de saut de géométrie, et
// surtout : la PREMIÈRE sonde mesure déjà le bon cran).
export function restaurerEspacement() {
  let lu = null;
  try {
    lu = localStorage.getItem(CLE);
  } catch { /* stockage illisible : défaut */ }
  etat.niveau = NIVEAUX.includes(lu) ? lu : DEFAUT;
}
