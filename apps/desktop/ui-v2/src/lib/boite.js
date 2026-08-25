// A80 — la règle du bloc de boîte (« Camille Roux sur ▣ Travail »), en
// UN endroit. Née de la revue du 2026-08-25 : elle vivait en deux
// exemplaires — Liste.svelte et Fil.svelte — et la règle du titre y
// avait déjà divergé le temps d'un incrément. Le CSS est partagé
// (systeme.css, .boite) ; la DÉCISION l'est ici.
//
// Fonction pure, sans I/O ni état : la décision ici, l'affichage chez
// l'appelant (motif STANDARD §4). Le markup reste posé par chaque
// composant, comme la pastille de repère — trois spans qui se lisent
// sur place valent mieux qu'un composant pour si peu.
//
// D7 (« le bloc ne vit QUE là où les comptes se mélangent ») se lit ici
// au pied de la lettre : un poste qui n'a qu'UN compte ne mélange rien,
// et « sur <sa propre adresse> » sur chaque rangée est le refrain que
// D7 refuse.
//
// La garde de VUE vit dans `vueMelange` ci-dessous — une seule
// expression pour deux appelants (la liste et le volet de lecture,
// verdict terrain du 2026-08-25, point 12).
// La VUE courante mélange-t-elle les comptes ? Boîte unifiée : oui, par
// définition. Vue bornée à un compte : non — sauf en recherche, qui
// traverse les comptes et les dossiers (D3 d'A74).
//
// Verdict terrain du 2026-08-25 (point 12) : le volet de lecture suit
// LA MÊME règle que la liste. D5 disait « le même schéma au volet », et
// le terrain a montré l'asymétrie : la liste se taisait, le volet
// parlait encore. Une seule règle, deux appelants.
export const vueMelange = (compte, enRecherche) => compte === null || enRecherche;

export function blocBoite({ accountId, adresse, reperes = {}, noms = {}, comptes = [] }) {
  if (comptes.length < 2) return null;
  // Le libellé est le nom personnalisé (A78) s'il existe, sinon
  // l'adresse : c'est ce qui rend le repère facultatif (D8).
  const libelle = noms[accountId] ?? adresse;
  return {
    repere: reperes[accountId] ?? null,
    libelle,
    // L'adresse reste la vérité technique de l'infobulle — mais sans
    // nom personnalisé les deux chaînes sont identiques, et « adresse
    // — adresse » serait un bégaiement.
    titre: libelle === adresse ? libelle : `${libelle} — ${adresse}`,
  };
}
