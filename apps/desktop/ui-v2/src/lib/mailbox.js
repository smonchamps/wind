// A80 — the mailbox block rule (“Camille Roux on ▣ Work”), in
// ONE place. Born from the 2026-08-25 review: it lived in two
// copies — List.svelte and Thread.svelte — and the title rule had
// already drifted apart in the space of one increment. The CSS is
// shared (systeme.css, .boite); the DECISION is here.
//
// Pure function, no I/O or state: the decision here, the display at
// the caller's (STANDARD §4 pattern). The markup stays laid out by
// each component, like the marker badge — three spans that read on
// the spot are worth more than a component for so little.
//
// D7 (“the block lives ONLY where accounts mix”) reads here to the
// letter: a workstation with only ONE account mixes nothing, and
// “on <its own address>” on every row is the refrain that D7 refuses.
//
// The VIEW guard lives in `mixedView` below — a single
// expression for two callers (the list and the reading pane,
// field verdict from 2026-08-25, point 12).
// Does the current VIEW mix accounts? Unified inbox: yes, by
// definition. A view bound to one account: no — except in search,
// which crosses accounts and folders (D3 of A74).
//
// Field verdict from 2026-08-25 (point 12): the reading pane follows
// THE SAME rule as the list. D5 said “the same pattern in the pane”,
// and the field showed the asymmetry: the list stayed silent, the
// pane still spoke. One rule, two callers.
export const mixedView = (account, searching) => account === null || searching;

export function mailboxBlock({ accountId, address, markers = {}, names = {}, accounts = [] }) {
  if (accounts.length < 2) return null;
  // The label is the custom name (A78) if it exists, otherwise
  // the address: this is what makes the marker optional (D8).
  const label = names[accountId] ?? address;
  return {
    marker: markers[accountId] ?? null,
    label,
    // The address stays the tooltip's technical truth — but without
    // a custom name the two strings are identical, and “address
    // — address” would be a stutter.
    title: label === address ? label : `${label} (${address})`,
  };
}
