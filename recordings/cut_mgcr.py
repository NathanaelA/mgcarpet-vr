#!/usr/bin/env python3
"""Cut a conjoined .mgcr at a level transition.

    cut_mgcr.py <src.mgcr> <cut_t> <out_a.mgcr> <out_b.mgcr> <level_b>

Part A keeps ticks 0..cut_t-1 under the original header. Part B
re-bases ticks cut_t.. to t=0.. with header level=<level_b> and cut
provenance recorded in capture.cut_from. Find <cut_t> with
find_boundary.py (the embedded level record changes on the first tick
of the new level).
"""
import json
import re
import subprocess
import sys

if len(sys.argv) != 6:
    sys.exit(__doc__.strip().splitlines()[2].strip())
SRC, CUT_T, OUT_A, OUT_B, LEVEL_B = (
    sys.argv[1],
    int(sys.argv[2]),
    sys.argv[3],
    sys.argv[4],
    int(sys.argv[5]),
)

T_RE = re.compile(rb'^\{"t":(\d+),')

dec = subprocess.Popen(["zstdcat", SRC], stdout=subprocess.PIPE, bufsize=1 << 22)
enc_a = subprocess.Popen(["zstd", "-9", "-q", "-f", "-o", OUT_A], stdin=subprocess.PIPE)
enc_b = subprocess.Popen(["zstd", "-9", "-q", "-f", "-o", OUT_B], stdin=subprocess.PIPE)

header = dec.stdout.readline()
h = json.loads(header)
assert h["type"] == "header", h
enc_a.stdin.write(header)

hb = dict(h)
hb["level"] = LEVEL_B
hb.setdefault("capture", {})["cut_from"] = {"file": SRC.split("/")[-1], "t0": CUT_T}
enc_b.stdin.write(json.dumps(hb, separators=(",", ":")).encode() + b"\n")

na = nb = 0
last_a = first_b_line = None
for line in dec.stdout:
    m = T_RE.match(line)
    assert m, line[:60]
    t = int(m.group(1))
    if t < CUT_T:
        enc_a.stdin.write(line)
        na += 1
        last_a = t
    else:
        nt = t - CUT_T
        out = b'{"t":%d,' % nt + line[m.end() :]
        enc_b.stdin.write(out)
        if nb == 0:
            first_b_line = (t, nt)
        nb += 1

enc_a.stdin.close()
enc_b.stdin.close()
dec.stdout.close()
for p in (enc_a, enc_b, dec):
    p.wait()
print(
    f"A: {na} ticks (0..{last_a})  "
    f"B: {nb} ticks (orig {first_b_line[0]}.. -> {first_b_line[1]}..)"
)
