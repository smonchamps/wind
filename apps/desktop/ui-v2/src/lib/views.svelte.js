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

// The subscription itself lives HERE too (review, wave 3): four views
// carried the same ten-line effect — the per-surface wiring this
// module exists to end. A view calls `watchViews(reload)` once at
// setup; the pre-synced counter skips the mount (each view already
// loads itself on mount).
export function watchViews(reload) {
  let seen = views.generation;
  $effect(() => {
    if (views.generation !== seen) {
      seen = views.generation;
      reload();
    }
  });
}
