#!/usr/bin/env python3
"""Carry fixture curation across a re-record (docs/CONFORMANCE.md
"Re-extract").

    carry_curation.py <manifest.json> [<git-rev>]

Signatures capture the STORY of a failure, so they are the bridge
between the old take's curated manifest (statuses + ledger notes) and
a fresh extract: every new fixture whose sig appears in the old
manifest inherits its status and note. Prints the reconciliation —
carried, new-story (unmatched new sigs), and vanished-story (old sigs
absent from the new extract: resolved, drifted, or just not exercised
by the new gameplay).
"""
import json
import subprocess
import sys

if len(sys.argv) < 2:
    sys.exit(__doc__.strip().splitlines()[2].strip())
path = sys.argv[1]
rev = sys.argv[2] if len(sys.argv) > 2 else "HEAD"

old = json.loads(
    subprocess.run(
        ["git", "show", f"{rev}:{path}"], capture_output=True, text=True, check=True
    ).stdout
)
new = json.load(open(path))

old_by_sig = {f["sig"]: f for f in old["fixtures"] if f.get("sig")}
new_sigs = {f["sig"] for f in new["fixtures"] if f.get("sig")}

carried = newborn = 0
for f in new["fixtures"]:
    sig = f.get("sig")
    if not sig:
        continue
    o = old_by_sig.get(sig)
    if o:
        f["status"] = o["status"]
        if o.get("note"):
            f["note"] = o["note"]
        carried += 1
    else:
        newborn += 1
        print(f"  NEW story t={f['t']}: {' '.join(f['atoms'])}")

vanished = [f for s, f in old_by_sig.items() if s not in new_sigs]
for f in vanished:
    note = f.get("note", "")
    print(f"  VANISHED ({f['status']}, old t={f['t']}): {note[:80]}")

json.dump(new, open(path, "w"), indent=2)
open(path, "a").write("\n")
print(
    f"== {path}: {carried} carried, {newborn} new stories, "
    f"{len(vanished)} old stories not re-extracted"
)
