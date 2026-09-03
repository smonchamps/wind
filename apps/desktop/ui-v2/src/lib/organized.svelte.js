// Organized mode (PLAN-MODE-ORGANISE E1, decision D2 amended on
// 2026-08-29): the state lives in SQLite `prefs`, NOT in localStorage
// — the CORE must read it (the No's rules from step E3 turn off
// with the mode), the UI only reflects it. The retention bound
// (first-activation epoch, D3 “arrivals only”) is written on the
// Rust side in the same gesture — never here.
import { call } from './transport.js';

const state = $state({ active: false });

// E1 review: the restore starts without await (PLAN-DEMARRAGE
// lesson — nothing precedes the list's first page); if the user
// toggles BEFORE it resolves, its stale read must never
// overwrite the fresh gesture.
let toggling = false;
let inFlight = false;

export function organizedMode() {
  return state.active;
}

// RETOURS-13 R3: THE mailbox label rule — in organized mode the
// Inbox is called the short name, the classic keeps the long one.
// A single expression for every surface (nav, list header, thread
// back, status, toasts); the review counted four copies that had
// already drifted apart in form.
export function mailboxLabelKey(id) {
  return id === 'inbox' && state.active
    ? 'mailbox.organizedInbox'
    : `mailbox.${id}`;
}

// Read once at startup, AFTER the list's first page (the App
// decides the moment — PLAN-DEMARRAGE E2 lesson). A failure leaves
// the mode off: the classic is the default.
export async function restoreOrganizedMode() {
  try {
    const read = Boolean(await call('organized_mode_get'));
    if (!toggling) state.active = read;
  } catch {
    /* the classic is the default, nothing to reflect */
  }
  return state.active;
}

// The toggle WRITES first, reflects after — a command failure never
// lets the UI say a mode the database does not have. A second click
// while in flight is ignored (otherwise both compute the same target
// and the toggle “sticks”).
export async function toggleOrganizedMode() {
  if (inFlight) return state.active;
  inFlight = true;
  toggling = true;
  try {
    const target = !state.active;
    await call('organized_mode_set', { active: target });
    state.active = target;
    return target;
  } finally {
    inFlight = false;
  }
}
