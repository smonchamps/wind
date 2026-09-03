// Le vocabulaire d'affichage des verdicts du Portier — UNE copie
// (RETOURS-14 R5, revue : Portier.svelte et Reglages.svelte portaient
// chacun la leur ; une règle du Non ajoutée d'un côté aurait laissé
// l'autre surface retomber en silence sur le libellé générique). Les
// clés miroirent le vocabulaire FERMÉ du cœur (`routage_expediteurs`),
// comme `vocabulaires.js` pour les horizons.
export const LIBELLE_ECARTE = {
  spam: 'portier.ecarteSpam',
  archive: 'portier.ecarteArchive',
  trash: 'portier.ecarteCorbeille',
};

export const LIBELLE_DESTINATION = {
  inbox: 'portier.versReception',
  feed: 'portier.versKiosque',
  paper_trail: 'portier.versRegistre',
};
