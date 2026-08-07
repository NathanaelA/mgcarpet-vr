#!/usr/bin/env python3
"""Cut a conjoined .mgcr at a level transition.

    cut_mgcr.py <src.mgcr> <cut_t> <out_a.mgcr> <out_b.mgcr> <level_b>

Part A keeps ticks 0..cut_t-1 under the original header. Part B
re-bases ticks cut_t.. to t=0.. with header level=<level_b> and cut
provenance recorded in capture.cut_from. Find <cut_t> with
find_boundary.py (the embedded level record changes on the first tick
of the new level).

Format-2 terrain channel: the recorder emits ONE base record (the
take's first tick) and record-relative deltas thereafter — a level
transition is just a giant delta. A naive cut therefore leaves part B
base-less (mgc-conform: "terrain channel declared but no base record
seen"). This tool accumulates the running image through the cut and
rewrites part B's first record to carry it as base_b64 (its own
transition delta folded in), so both halves are self-contained.
"""
import base64
import json
import re
import struct
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
TER_RE = re.compile(rb'"terrain":\{"(base|delta)_b64":"([^"]*)"\}')

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

planes = (h.get("channels") or {}).get("terrain", {}).get("planes")
nplanes = len(planes) if planes else 0
img = None  # running plane image, maintained through the cut


def apply_terrain(line):
    """Fold the line's terrain record (if any) into the running image."""
    global img
    m = TER_RE.search(line)
    if not m:
        return
    blob = base64.b64decode(m.group(2))
    if m.group(1) == b"base":
        assert len(blob) == nplanes * 0x10000, len(blob)
        img = bytearray(blob)
        return
    if img is None:
        return  # delta before any base: nothing to accumulate onto
    off = 0
    for p in range(nplanes):
        (count,) = struct.unpack_from("<I", blob, off)
        off += 4
        pbase = p * 0x10000
        for _ in range(count):
            cell, val = struct.unpack_from("<HB", blob, off)
            off += 3
            img[pbase + cell] = val
    assert off == len(blob), (off, len(blob))


na = nb = 0
last_a = first_b_line = None
for line in dec.stdout:
    m = T_RE.match(line)
    assert m, line[:60]
    t = int(m.group(1))
    if t < CUT_T:
        if nplanes:
            apply_terrain(line)
        enc_a.stdin.write(line)
        na += 1
        last_a = t
    else:
        nt = t - CUT_T
        out = b'{"t":%d,' % nt + line[m.end() :]
        if nb == 0 and nplanes:
            # Part B's first record: fold its own delta in, then
            # carry the accumulated image as the half's base.
            apply_terrain(line)
            assert img is not None, "terrain channel declared but no base seen"
            obj = json.loads(out)
            obj["terrain"] = {"base_b64": base64.b64encode(bytes(img)).decode()}
            out = json.dumps(obj, separators=(",", ":")).encode() + b"\n"
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
    + (f"  B base materialized ({nplanes} planes)" if nplanes else "")
)
