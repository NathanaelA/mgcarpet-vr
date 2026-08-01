#!/usr/bin/env python3
"""Freeze a suite manifest's fixture pairs into a self-contained
bundle (docs/CONFORMANCE.md "Freeze").

    freeze_fixtures.py <manifest.json> [<source-take.mgcr>]

A fixture IS its pair of retail states; referencing them inside a
re-recordable take made suites mortal. This copies, verbatim, every
fixture's tick window [t-(input_delay+2) .. t+1] out of the source
recording into `conformance/fixtures/<take>-fixtures.mgcr` (the
leading lines warm the runner's input-delay ring; line t = state to
import, line t+1 = obs to diff) and repoints the manifest. The
bundle is a normal .mgcr (window boundaries are ordinary gaps), so
the runner is unchanged; the full take remains the verify-deltas /
re-extract source, and a superseded take no longer orphans its
suite. Bundles are COMMITTED (via git-lfs, .gitattributes) — the
suite is self-contained in conformance/; fullsize recordings never
enter git.

Coverage is verified line-by-line: the suite silently reports an
unreachable pair as "not reached", so an incomplete bundle must fail
HERE. Re-freezing an already-frozen manifest needs the source take
(pass it explicitly if the default name moved); freezing from the
bundle itself is refused when any fixture would lose its window.
"""
import json
import re
import subprocess
import sys

if len(sys.argv) not in (2, 3):
    sys.exit(__doc__.strip().splitlines()[3].strip())
man_path = sys.argv[1]
man_dir = re.sub(r"[^/]+$", "", man_path) or "./"

man = json.load(open(man_path))
delay = int(man.get("input_delay", 0))
ts = sorted({f["t"] for f in man["fixtures"]})
if not ts:
    sys.exit(f"{man_path}: no fixtures")

rec_rel = man["recording"]
if len(sys.argv) == 3:
    src = sys.argv[2]
elif rec_rel.endswith("-fixtures.mgcr"):
    # Already frozen: prefer the full take (hand-appended fixtures
    # need lines the bundle lacks); fall back to re-slicing the
    # bundle itself — the coverage check below still guards.
    stem0 = re.sub(r"-fixtures\.mgcr$", "", rec_rel.split("/")[-1])
    take = man_dir + "../recordings/" + stem0 + ".mgcr"
    src = take if subprocess.run(["test", "-f", take]).returncode == 0 else man_dir + rec_rel
else:
    src = man_dir + rec_rel
stem = re.sub(r"(-fixtures)?\.mgcr$", "", src.split("/")[-1])
out = man_dir + "fixtures/" + stem + "-fixtures.mgcr"

# Merged tick windows: [t-(delay+2), t+1] per fixture.
lead = delay + 2
windows = []
for t in ts:
    lo, hi = max(0, t - lead), t + 1
    if windows and lo <= windows[-1][1] + 1:
        windows[-1][1] = max(windows[-1][1], hi)
    else:
        windows.append([lo, hi])
in_window = {n for lo, hi in windows for n in range(lo, hi + 1)}

T_RE = re.compile(rb'^\{"t":(\d+),')

dec = subprocess.Popen(["zstdcat", src], stdout=subprocess.PIPE, bufsize=1 << 22)
enc = subprocess.Popen(["zstd", "-9", "-q", "-f", "-o", out], stdin=subprocess.PIPE)

header = dec.stdout.readline()
h = json.loads(header)
assert h["type"] == "header", h
h.setdefault("capture", {})["fixture_freeze"] = {
    "source": src.split("/")[-1],
    "manifest": man_path.split("/")[-1],
    "pairs": len(ts),
}
enc.stdin.write(json.dumps(h, separators=(",", ":")).encode() + b"\n")

seen = set()
copied = 0
for line in dec.stdout:
    m = T_RE.match(line)
    assert m, line[:60]
    t = int(m.group(1))
    if t in in_window:
        enc.stdin.write(line)
        seen.add(t)
        copied += 1
enc.stdin.close()
dec.stdout.close()
for p in (enc, dec):
    p.wait()

# Every pair must be replayable: state line at t AND obs line at t+1.
broken = [t for t in ts if t not in seen or t + 1 not in seen]
if broken:
    sys.exit(
        f"{man_path}: source {src} lacks the pair lines for "
        f"t={broken} — an incomplete bundle would silently skip them; "
        f"pass the full take explicitly"
    )

man["recording"] = "fixtures/" + stem + "-fixtures.mgcr"
json.dump(man, open(man_path, "w"), indent=2)
open(man_path, "a").write("\n")
print(
    f"== {man_path}: froze {len(ts)} pairs ({len(windows)} windows, "
    f"{copied} lines) from {src} -> {out}; manifest repointed"
)
