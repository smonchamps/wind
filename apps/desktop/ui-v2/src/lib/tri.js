// RETOURS-14 R9 : LA comparaison des quatre tris de section — un seul
// code pour les quatre surfaces (Kiosque, Registre, historique du
// Portier, Nettoyage) ; `epochDe` et `quiDe` disent où lire chaque
// rangée. L'alphabet suit la langue de l'UI (localeCompare, base) —
// le tri du Kiosque « Lus précédemment » faisait déjà ainsi.
import { langueActuelle } from './texte.svelte.js';

export function comparateurTri(tri, epochDe, quiDe) {
  const alpha = (a, b) =>
    (quiDe(a) ?? '').localeCompare(quiDe(b) ?? '', langueActuelle(), { sensitivity: 'base' });
  switch (tri) {
    case 'date-asc':
      return (a, b) => epochDe(a) - epochDe(b);
    case 'alpha-az':
      return alpha;
    case 'alpha-za':
      return (a, b) => alpha(b, a);
    default:
      return (a, b) => epochDe(b) - epochDe(a);
  }
}
