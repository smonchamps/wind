// RETOURS-14 R9: THE comparison for the four section sorts — a single
// piece of code for the four surfaces (Feed, Paper trail, Screener
// history, Cleanup); `epochOf` and `whoOf` say where to read each
// row. The alphabet follows the UI's language (localeCompare, base) —
// the Feed's “Previously read” sort already did it this way.
import { currentLanguage } from './text.svelte.js';

export function sortComparator(sort, epochOf, whoOf) {
  const alpha = (a, b) =>
    (whoOf(a) ?? '').localeCompare(whoOf(b) ?? '', currentLanguage(), { sensitivity: 'base' });
  switch (sort) {
    case 'date-asc':
      return (a, b) => epochOf(a) - epochOf(b);
    case 'alpha-az':
      return alpha;
    case 'alpha-za':
      return (a, b) => alpha(b, a);
    default:
      return (a, b) => epochOf(b) - epochOf(a);
  }
}
