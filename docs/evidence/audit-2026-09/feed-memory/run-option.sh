#!/usr/bin/env sh
# Spike D-53: put ONE option's files in place, then run the bench.
#   sh spikes/feed-memory/run-option.sh <A|B|C> <run> [ko]
# The e2e launcher rebuilds the dist and re-embeds it (rebuild-v2.mjs).
set -e
ROOT="C:/Users/smonc/OneDrive/Documents/Repositories/wind/.claude/worktrees/agent-ad93e6fc48e0f6771"
OPT="$1"; RUN="$2"; KO="${3:-100}"
SP="$ROOT/spikes/feed-memory"
cp "$SP/options/Feed-$OPT.svelte" "$ROOT/apps/desktop/ui-v2/src/Feed.svelte"
if [ "$OPT" = "B" ]; then
  cp "$SP/options/body-B.js" "$ROOT/apps/desktop/ui-v2/src/lib/body.js"
else
  (cd "$ROOT" && git checkout -- apps/desktop/ui-v2/src/lib/body.js)
fi
cd "$ROOT/e2e"
node "$SP/bench.mjs" "$OPT" "$RUN" "$KO" > "$SP/raw/$OPT-${KO}k-run$RUN.log" 2>&1
echo "EXIT $?"
tail -1 "$SP/raw/$OPT-${KO}k-run$RUN.log"
