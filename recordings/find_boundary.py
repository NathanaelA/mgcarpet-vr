#!/usr/bin/env python3
"""Scan an .mgcr (zstd JSONL) for changes in the embedded level record
around struct offset 0x2FECE, without JSON-parsing the lines."""
import base64
import subprocess
import sys

PATH = sys.argv[1]
LO = 0x2FEC0  # slice start (bytes), 3-aligned below
HI = 0x2FF40  # slice end
K0 = LO // 3  # 3-byte group index
C0 = 4 * K0  # b64 char offset
NC = 4 * ((HI - LO) // 3 + 2)

MARK = b'"struct_b64":"'

proc = subprocess.Popen(["zstdcat", PATH], stdout=subprocess.PIPE, bufsize=1 << 22)
prev = None
n = 0
for line in proc.stdout:
    if line.startswith(b'{"type":"header"'):
        continue
    i = line.find(MARK)
    if i < 0:
        # tick without state channel?
        t = line[6 : line.find(b",", 6)]
        print(f"t={t.decode()} NO struct_b64")
        continue
    s = i + len(MARK)
    chunk = line[s + C0 : s + C0 + NC]
    raw = base64.b64decode(chunk)
    if raw != prev:
        # extract t
        tpos = line.find(b'"t":') + 4
        t = line[tpos : line.find(b",", tpos)]
        print(f"t={t.decode()} slice[0x{3 * K0:X}..] = {raw.hex()}")
        prev = raw
    n += 1
proc.stdout.close()
proc.wait()
print(f"scanned {n} tick records", file=sys.stderr)
