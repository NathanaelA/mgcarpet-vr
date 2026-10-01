#!/usr/bin/env python3
"""Generate docs/CONFORMANCE-REPORT.md from the conformance evidence.

Reads what the suite reads — the per-level manifests `conformance/<take>.json`
(one row per fixture: the retail state pair, its tick, its source take and
the law it pins) and the known-deviation roster
`conformance/known-deviations.json` — and, when `recordings/` is present
locally, which levels have a whole-take recording. Nothing in the output is
typed by hand: re-run after cutting a fixture or ruling a roster row, and
commit the result.

    tools/conformance_report.py            # writes docs/CONFORMANCE-REPORT.md
    tools/conformance_report.py --check    # exit 1 if the committed report is stale
"""
import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONF = ROOT / "conformance"
OUT = ROOT / "docs" / "CONFORMANCE-REPORT.md"

GAMES = [
    ("mc1", "Magic Carpet"),
    ("mc1hw", "Magic Carpet: Hidden Worlds"),
    ("mc2", "Magic Carpet 2: The Netherworlds"),
]

METHOD = """\
## How the port is verified

The original games are the oracle. Retail play was recorded under an
instrumented DOSBox that wrote down the complete state of every game
entity on every game turn — position, life, timers, the lot — together
with the player's input. The port replays the same input through its own
simulation and must reproduce that state exactly, turn by turn, byte for
byte. Where it does not, the tick is investigated until the cause is
known: either a port defect (fixed) or a behaviour of the retail
executable that cannot be reproduced from its data alone (reading past
the end of a table, damaged memory) — those are registered in the
roster below, each with its evidence, and the replay is graded around
them.

Two things come out of that work and live in this repository:

* **Fixtures** — for every law that was ever broken and fixed, the pair
  of retail states that witnessed it, cut to a few kilobytes and named
  for the law. The suite replays every one of them against the current
  simulation on every change (`cargo test -p mgc-conform`; it needs the
  baked game data, so it runs on the author's machine, not in CI).
* **The roster** — the finite list of ticks where retail diverges from
  its own data by definition, each ruled with a written reason.

The whole-take recordings (tens of gigabytes) are not in the repository;
their certified replays are what the fixtures were cut from.
"""


def headline(note, limit=110):
    """The first clause of a ledger note, as a one-line label."""
    s = re.sub(r"^[^\w`(]+", "", note.strip())  # leading emoji / rulings
    s = re.sub(r"\s+", " ", s)
    m = re.search(r"(\.\s|\s--\s|\s—\s|;\s|:\s\()", s)
    if m and m.start() >= 20:
        s = s[: m.start()]
    # Ledger headlines shout; the report does not.
    letters = [c for c in s if c.isalpha()]
    if letters and sum(c.isupper() for c in letters) / len(letters) > 0.6:
        s = s[:1].upper() + s[1:].lower()
    if len(s) > limit:
        s = s[: limit - 1].rstrip() + "…"
    return s.rstrip(" .:")


def scope(r):
    """A roster rule's footprint, from its structured fields only."""
    parts = []
    kind = r.get("kind")
    fields = r.get("fields") or ([r["field"]] if r.get("field") else [])
    if kind:
        parts.append(kind + (": " + ", ".join(fields) if fields else ""))
    if r.get("class") is not None:
        parts.append(f"entity class {r['class']}" + (f" model {r['model']}" if r.get("model") is not None else ""))
    if "ticks" in r:
        parts.append(f"{len(r['ticks'])} tick(s)")
    elif "t_min" in r:
        parts.append(f"turns {r['t_min']}–{r.get('t_max', '…')}")
    if "rect" in r:
        parts.append("within a map rect")
    return "; ".join(parts) or "—"


def level_key(take):
    """`mc1l48-nodeath` and `mc1l48` are both level 48's takes."""
    m = re.match(r"(mc1hw|mc1|mc2)l(\d+)(?:-|$)", take)
    if not m:
        return None
    return m.group(1), int(m.group(2))


def git_head():
    try:
        return subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT,
                              capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return "unknown"


def load():
    manifests = {}
    for p in sorted(CONF.glob("*.json")):
        if p.name == "known-deviations.json":
            continue
        d = json.loads(p.read_text())
        manifests[p.stem] = d
    roster = json.loads((CONF / "known-deviations.json").read_text())["rules"]
    recordings = {}
    rec_dir = ROOT / "recordings"
    if rec_dir.is_dir():
        for p in rec_dir.glob("*.mgcr"):
            take = p.name.split(".")[0]
            recordings[take] = "torn" if ".torn." in p.name else "whole"
    return manifests, roster, recordings


def build(manifests, roster, recordings):
    # Roster rows per take.
    rows_for = defaultdict(list)
    for r in roster:
        for t in r.get("takes", []):
            k = level_key(t)
            if k:
                rows_for[k].append(r)
    # Every level we know of, from either source.
    levels = defaultdict(dict)
    for take in list(manifests) + list(recordings):
        k = level_key(take)
        if k and (take == f"{k[0]}l{k[1]}" or k[1] not in levels[k[0]]):
            levels[k[0]][k[1]] = take

    out = []
    out.append("# Conformance evidence\n")
    out.append(f"_Generated by `tools/conformance_report.py` from `conformance/` on "
               f"{date.today().isoformat()} at commit `{git_head()}` — do not edit; "
               f"re-run the script._\n")
    out.append(METHOD)

    n_fix = sum(len(m["fixtures"]) for m in manifests.values())
    out.append("## Coverage by level\n")
    out.append(f"{n_fix} fixtures across {len(manifests)} levels; {len(roster)} roster rows "
               f"({sum(r['status'] == 'deviation' for r in roster)} ruled retail-side, "
               f"{sum(r['status'] == 'capture' for r in roster)} recording-capture limits, "
               f"{sum(r['status'] == 'open' for r in roster)} open).\n")
    if not recordings:
        out.append("_(`recordings/` was not present when this was generated, so the "
                   "recording column is blank.)_\n")
    for gid, gname in GAMES:
        lv = levels.get(gid, {})
        if not lv:
            continue
        out.append(f"### {gname}\n")
        out.append("| Level | Take | Recording | Fixtures | Roster rows |")
        out.append("|---:|---|---|---:|---:|")
        for idx in sorted(lv):
            take = lv[idx]
            rec = recordings.get(take)
            rec_s = {"whole": "whole take", "torn": "torn take (certified to the tear)"}.get(rec, "—")
            nf = len(manifests[take]["fixtures"]) if take in manifests else 0
            nr = len(rows_for.get((gid, idx), []))
            fx = f"[{nf}](#{take})" if nf else "0"
            out.append(f"| {idx + 1} | `{take}` | {rec_s} | {fx} | {nr or ''} |")
        out.append("")
    out.append("Level numbers are the ones the game shows (map index + 1). A level "
               "with no row has no recording. Fixtures pin laws that were broken "
               "and fixed; a level with few or none simply replayed cleanly from "
               "the start.\n")

    out.append("## Laws pinned, by level\n")
    out.append("One line per fixture: the turn of the retail state pair and the law "
               "it witnesses (the full investigation note is in the manifest).\n")
    for gid, gname in GAMES:
        for idx in sorted(levels.get(gid, {})):
            take = levels[gid][idx]
            if take not in manifests:
                continue
            fx = manifests[take]["fixtures"]
            out.append(f'<a id="{take}"></a>')
            out.append(f"<details><summary><b>{gname} — level {idx + 1}</b> (`{take}`, "
                       f"{len(fx)} fixtures, [manifest](../conformance/{take}.json))</summary>\n")
            for f in sorted(fx, key=lambda f: f["t"]):
                src = f" · from `{f['source']}`" if f.get("source") and f["source"] != take else ""
                out.append(f"- t={f['t']}{src} — {headline(f.get('note', f['file']))}")
            out.append("\n</details>\n")

    out.append("## Retail behaviour that diverges by definition\n")
    out.append("The known-deviation roster: ticks where the retail executable's "
               "state cannot be reproduced from its own data (out-of-bounds reads, "
               "in-memory damage) or the recording could not capture the truth. "
               "The replay is graded around exactly these, with the evidence kept "
               "in [`known-deviations.json`](../conformance/known-deviations.json).\n")
    out.append("| Rule | Status | Takes | Scope |")
    out.append("|---|---|---|---|")
    for r in roster:
        takes = ", ".join(f"`{t}`" for t in r.get("takes", [])) or "any"
        out.append(f"| `{r['id']}` | {r['status']} | {takes} | {scope(r)} |")
    out.append("")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="compare with the committed report")
    ap.add_argument("--out", type=Path, default=OUT)
    a = ap.parse_args()
    text = build(*load())
    if a.check:
        old = a.out.read_text() if a.out.exists() else ""
        strip = lambda s: re.sub(r"^_Generated by.*$", "", s, flags=re.M)
        if strip(old) != strip(text):
            sys.exit(f"{a.out.relative_to(ROOT)} is stale — run tools/conformance_report.py")
        print("report is current")
        return
    a.out.write_text(text)
    print(f"wrote {a.out.relative_to(ROOT)} ({len(text) // 1024} KB)")


if __name__ == "__main__":
    main()
