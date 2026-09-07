"""A-vs-B table from the raw figures.

  python summarize.py raw/run-A.json raw/run-B.json
"""
import json
import sys


def q(values, p):
    s = sorted(values)
    return s[min(len(s) - 1, int(p * len(s)))]


def stats(values):
    return q(values, 0.5), q(values, 0.95), max(values), len(values)


runs = {}
for path in sys.argv[1:]:
    with open(path, encoding="utf-8-sig") as f:
        data = json.load(f)
    runs[data["variant"]] = data

variants = list(runs)
loads = ["idle", "sanitize", "attach", "both"]
gestures = ["open", "page", "pin"]

print("| Load | Gesture | " + " | ".join(f"{v} p50 / p95 / max (ms)" for v in variants) + " |")
print("|---|---|" + "---|" * len(variants))
for load in loads:
    for g in gestures:
        cells = []
        for v in variants:
            r = runs[v]["loads"].get(load)
            if not r:
                cells.append("—")
                continue
            p50, p95, mx, n = stats(r["gestures"][g])
            cells.append(f"{p50:.1f} / {p95:.1f} / {mx:.1f}")
        print(f"| {load} | {g} | " + " | ".join(cells) + " |")

print()
print("| Load | Heavy op (one iteration, page-side) | " + " | ".join(f"{v} p50 / p95 / max (ms) · n" for v in variants) + " |")
print("|---|---|" + "---|" * len(variants))
for load in loads:
    for k in ["sanitize", "attach"]:
        cells = []
        any_ = False
        for v in variants:
            r = runs[v]["loads"].get(load)
            t = r["load"]["times"][k] if r else []
            if not t:
                cells.append("—")
                continue
            any_ = True
            p50, p95, mx, n = stats(t)
            cells.append(f"{p50:.1f} / {p95:.1f} / {mx:.1f} · {n}")
        if any_:
            print(f"| {load} | {k} | " + " | ".join(cells) + " |")

for v in variants:
    for load in loads:
        r = runs[v]["loads"].get(load)
        if r and r["load"]["errors"]:
            print(f"\n{v}/{load}: {len(r['load']['errors'])} load errors, first: {r['load']['errors'][0]}")
