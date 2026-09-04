// The view invalidation signal (PLAN-AUDIT-V3 E7 — closes D-48).
//
// ONE mechanism: anything that changed what a view shows — a gesture
// handler, the resting probe seeing the shell's generation move, a
// routing removed in Settings — bumps this counter; every MOUNTED view
// subscribes and reloads itself. No more `list?.reload()` wired per
// surface (D-48's stated fix): a new view subscribes once and is
// covered by every existing writer, and the shell's own generation
// bumps (the write commands, E7) reach it through the resting probe.
export const views = $state({ generation: 0 });

export function invalidateViews() {
  views.generation += 1;
}
