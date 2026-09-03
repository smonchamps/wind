// Les vocabulaires FERMÉS des horizons et du Nettoyage — le miroir JS
// unique des constantes du cœur (`HORIZONS_IMPORT`, `PLAGES_NETTOYAGE`,
// `PERIMETRES_NETTOYAGE` dans mail-core), à la frontière comme
// `reperes.js`. Revue 2026-08-30 : chaque composant portait sa copie —
// une valeur ajoutée d'un côté aurait laissé l'autre surface muette,
// ou offert un choix que la porte du cœur refuse.
export const HORIZONS_IMPORT = ['1m', '2m', '3m', '6m', '1a', '2a', 'all'];
export const PLAGES_NETTOYAGE = ['3m', '6m', '1a', '2a', '5a', 'all'];
export const PERIMETRES_NETTOYAGE = [
  'inbox',
  'folders',
  'foldersArchive',
  'archive',
];
