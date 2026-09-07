#!/usr/bin/env sh
# Spike D-53: a sequence of runs, strictly one after the other.
#   sh spikes/feed-memory/run-all.sh <ko> A2 B1 B2 C1 C2
SP="C:/Users/smonc/OneDrive/Documents/Repositories/wind/.claude/worktrees/agent-ad93e6fc48e0f6771/spikes/feed-memory"
KO="$1"; shift
for r in "$@"; do
  OPT=$(echo "$r" | cut -c1); RUN=$(echo "$r" | cut -c2)
  echo "=== $OPT run $RUN ($KO KB)"
  sh "$SP/run-option.sh" "$OPT" "$RUN" "$KO" | cut -c1-160
done
