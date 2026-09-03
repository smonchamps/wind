// R1 (PLAN-RETOURS-8, A74) — the account marker: the DEDICATED icon
// set (D2, reserved for accounts — A3 “one icon, one meaning” held by
// reservation) and the families of the measured swatch picker (D1).
// The hex values live in system.css (.repere[data-teinte]) and are
// measured by e2e/contraste.mjs; here, only the NAMES — the twin
// allowlist lives on the Rust side (commands.rs, repere_normalise) and
// is authoritative on write.
export const MARKER_ICONS = [
  'home',
  'work',
  'school',
  'star',
  'favorite',
  'flight',
  'shopping_bag',
  'account_balance',
  'sports_esports',
  'eco',
  'pets',
  'music_note',
];

export const MARKER_HUES = [
  'red',
  'orange',
  'ochre',
  'olive',
  'green',
  'pine',
  'blue',
  'indigo',
  'violet',
  'magenta',
  'pink',
  'brown',
];
