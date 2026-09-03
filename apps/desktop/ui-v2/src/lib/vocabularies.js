// The CLOSED vocabularies for the horizons and the Cleanup — the sole
// JS mirror of the core's constants (`IMPORT_HORIZONS`,
// `CLEANUP_RANGES`, `CLEANUP_SCOPES` in mail-core), at the
// boundary like `markers.js`. Review from 2026-08-30: each component
// carried its own copy — a value added on one side would have left
// the other surface mute, or offered a choice that the core's
// gateway refuses.
export const IMPORT_HORIZONS = ['1m', '2m', '3m', '6m', '1a', '2a', 'all'];
export const CLEANUP_RANGES = ['3m', '6m', '1a', '2a', '5a', 'all'];
export const CLEANUP_SCOPES = [
  'inbox',
  'folders',
  'foldersArchive',
  'archive',
];
