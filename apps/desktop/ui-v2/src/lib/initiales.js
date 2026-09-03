// The initials avatar (UI v3, decision D2): VISUAL only — never
// a button, batch selection is a deferred feature. At most two
// letters, from the first two words; an empty name (rare:
// a draft with no recipient) renders a dash, never a blank.
// Shared by list/thread since field finding A45 (pane cards).
export function initials(name) {
  const letters = (name ?? '').trim().split(/\s+/, 2)
    .map((word) => word[0])
    .join('')
    .toUpperCase();
  return letters || '—';
}
