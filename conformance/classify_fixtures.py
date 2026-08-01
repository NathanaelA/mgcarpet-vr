#!/usr/bin/env python3
"""Auto-classify a fresh extract's open fixtures against the
known-deviation roster (docs/CONFORMANCE.md "Triage").

    classify_fixtures.py <manifest.json> <verify-deltas.tsv>

For each fixture with no note yet, the pair's CSV rows (rule column =
the roster's verdict, empty = unexplained) decide the status:

- every row matched and every matching rule is `capture`, no rng
  mismatch -> status `capture` (closure-domain, not a port bug);
- anything else (an `open` rule, an unexplained row, an rng
  mismatch) -> stays `open`.

The note lists the matched rule ids (each rule cites its ledger
entry — the provenance chain is fixture -> rule -> ledger) plus the
unexplained-row count. Hand-curated notes (carried by
carry_curation.py) are never overwritten.
"""
import csv
import json
import sys
from collections import Counter

if len(sys.argv) != 3:
    sys.exit(__doc__.strip().splitlines()[3].strip())
man_path, csv_path = sys.argv[1], sys.argv[2]

man = json.load(open(man_path))
roster = json.load(open("conformance/known-deviations.json"))
rules = roster["rules"] if isinstance(roster, dict) else roster
rule_status = {r["id"]: r["status"] for r in rules}

# t -> (rule id -> rows, unexplained rows, rng mismatch)
per_t = {}
with open(csv_path) as f:
    for row in csv.DictReader(f, delimiter="\t"):
        t = int(row["t"])
        hit, unex, rng = per_t.setdefault(t, (Counter(), [0], [False]))
        if row["kind"] == "rng":
            if row["want"] != row["got"]:
                rng[0] = True
            continue
        if row["rule"]:
            hit[row["rule"]] += 1
        else:
            unex[0] += 1

counts = Counter()
for fx in man["fixtures"]:
    if fx["status"] == "conforming" or fx.get("note"):
        continue
    got = per_t.get(fx["t"])
    if got is None:
        counts["no-rows"] += 1
        continue
    hit, unex, rng = got
    statuses = {rule_status.get(r, "open") for r in hit}
    parts = [f"roster: {', '.join(r for r, _ in hit.most_common())}"] if hit else []
    if unex[0]:
        parts.append(f"{unex[0]} row(s) unexplained")
    if rng[0]:
        parts.append("rng")
    clean_capture = hit and statuses == {"capture"} and not unex[0] and not rng[0]
    fx["status"] = "capture" if clean_capture else "open"
    fx["note"] = "auto-triage vs known-deviations: " + "; ".join(parts or ["no rows?"])
    counts[fx["status"]] += 1

json.dump(man, open(man_path, "w"), indent=2)
open(man_path, "a").write("\n")
print(f"== {man_path}: {dict(counts)}")
