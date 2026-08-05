#!/usr/bin/env python3
"""Scan .mgcr files for tick-numbering gaps (frame skips).

For each file: total tick records, first/last t, and every gap
(prev -> next, missing count). Pairing in verify-deltas breaks at
gaps, so gap count directly prices the fixture-grade pair loss.
"""
import re
import subprocess
import sys

T_RE = re.compile(rb'^\{"t":(\d+),')

for path in sys.argv[1:]:
    dec = subprocess.Popen(["zstdcat", path], stdout=subprocess.PIPE, bufsize=1 << 22)
    first = last = None
    n = 0
    gaps = []
    for line in dec.stdout:
        m = T_RE.match(line)
        if not m:
            continue
        t = int(m.group(1))
        if first is None:
            first = t
        elif t != last + 1:
            gaps.append((last, t))
        last = t
        n += 1
    dec.stdout.close()
    dec.wait()
    print(f"== {path}: {n} ticks, t={first}..{last}, {len(gaps)} gap(s)")
    for a, b in gaps:
        print(f"   gap {a} -> {b} ({b - a - 1} missing)")
