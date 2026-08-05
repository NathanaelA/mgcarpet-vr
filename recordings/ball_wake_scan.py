#!/usr/bin/env python3
"""Corpus test of the mana-ball wake-gate hypothesis (mc1/hw).

Balls are ballistic only while +58 (the awake counter) is nonzero.
Two candidate retail mechanisms for "mana starts rolling when
approached":
  A) REFRESH — some wake pass re-tops +58 for entities inside the
     awake radius (signature: f58 increases on a slot that was
     already a ball, same position, no respawn);
  B) STALL — the decrementing anim pass is itself awake-gated, so a
     far ball's f58 freezes mid-countdown and resumes near the
     player (signature: f58>0 held constant across adjacent ticks,
     correlated with distance).
Both are measured against player distance (tiles).
"""
import base64
import re
import subprocess
import sys
from collections import Counter

PATH = sys.argv[1] if len(sys.argv) > 1 else "recordings/mc1hwl0.mgcr"
POOL, STRIDE, NENT = 29795, 164, 1000
T_RE = re.compile(rb'^\{"t":(\d+),')
MARK = b'"struct_b64":"'

dec = subprocess.Popen(["zstdcat", PATH], stdout=subprocess.PIPE, bufsize=1 << 23)

# slot -> (f58, x256, y256, born_t, id24)
balls = {}
prev_t = -10
human_slot = None

refresh = []  # (t, slot, f58_from, f58_to, dist_tiles, age)
stall = Counter()
pop = Counter(); refresh_dist = Counter(); vals = Counter()
intervals = Counter(); last_refresh = {}; birth_f58 = Counter()
ticking = Counter()  # dist bucket -> decrement events
roll_seen = 0
n_lines = 0

for line in dec.stdout:
    m = T_RE.match(line)
    if not m:
        continue
    t = int(m.group(1))
    i = line.find(MARK)
    if i < 0:
        continue
    s = i + len(MARK)
    e = line.find(b'"', s)
    d = base64.b64decode(line[s:e])
    n_lines += 1

    # Human = class 3 model 0 (the (3,0) carpet).
    if human_slot is None or d[POOL + human_slot * STRIDE + 64] != 3:
        human_slot = next(
            (
                s2
                for s2 in range(NENT)
                if d[POOL + s2 * STRIDE + 64] == 3 and d[POOL + s2 * STRIDE + 65] == 0
            ),
            None,
        )
        if human_slot is None:
            continue
    ho = POOL + human_slot * STRIDE
    hx = int.from_bytes(d[ho + 72 : ho + 74], "little")
    hy = int.from_bytes(d[ho + 74 : ho + 76], "little")

    adjacent = t == prev_t + 1
    cur = {}
    for slot in range(NENT):
        o = POOL + slot * STRIDE
        if d[o + 64] != 10 or d[o + 65] not in (39, 40):
            continue
        f58 = d[o + 58]
        x = int.from_bytes(d[o + 72 : o + 74], "little")
        y = int.from_bytes(d[o + 74 : o + 76], "little")
        id24 = int.from_bytes(d[o + 24 : o + 26], "little")
        p = balls.get(slot)
        born = t if p is None else p[3]
        if p is None:
            birth_f58[f58] += 1
        if p is not None and adjacent and p[4] == id24:
            same_place = abs(x - p[1]) < 512 and abs(y - p[2]) < 512
            dx = abs(x - hx); dx = min(dx, 65536 - dx)
            dy = abs(y - hy); dy = min(dy, 65536 - dy)
            dist = max(dx, dy) / 256.0
            b = min(int(dist // 4) * 4, 60)
            pop[b] += 1
            if same_place:
                if f58 > p[0]:
                    refresh.append((t, slot, p[0], f58, dist, t - p[3]))
                    refresh_dist[b] += 1
                    vals[f58] += 1
                    last = last_refresh.get(slot)
                    if last is not None:
                        intervals[min(t - last, 40)] += 1
                    last_refresh[slot] = t
                elif f58 == p[0] and f58 > 0:
                    stall[b] += 1
                elif f58 < p[0]:
                    ticking[b] += 1
                if (x, y) != (p[1], p[2]) and f58 > 0:
                    roll_seen += 1
            else:
                born = t  # slot reused for a new ball elsewhere
        cur[slot] = (f58, x, y, born, id24)
    balls = cur
    prev_t = t

dec.stdout.close()
dec.wait()

print(f"scanned {n_lines} ticks; rolling-ball ticks seen: {roll_seen}")
print(f"\nA) REFRESH events (f58 increased in place): {len(refresh)}")
for t, slot, a, b, dist, age in refresh[:15]:
    print(f"   t={t} slot={slot} f58 {a}->{b} dist={dist:.1f} tiles age={age}")
if refresh:
    ds = sorted(r[4] for r in refresh)
    print(
        f"   dist tiles: min={ds[0]:.1f} med={ds[len(ds) // 2]:.1f} "
        f"max={ds[-1]:.1f}; ages>128: {sum(1 for r in refresh if r[5] > 128)}"
    )

print("\nrefresh value written:", dict(sorted(vals.items())))
print("birth f58 (top):", sorted(birth_f58.items(), key=lambda kv: -kv[1])[:8])
print("inter-refresh intervals (top):", sorted(intervals.items(), key=lambda kv: -kv[1])[:8])
print("\nREFRESH rate by wrap-corrected distance (refreshes / ball-ticks present)")
for b in sorted(pop):
    r, n = refresh_dist.get(b, 0), pop[b]
    print(f"   {b:3}-{b+4:3}: {r:6} / {n:8}  ({r/n if n else 0:8.5f})")
print("\nB) STALL vs TICKING by distance bucket (tiles: held / decremented)")
for b in sorted(set(stall) | set(ticking)):
    s, k = stall.get(b, 0), ticking.get(b, 0)
    frac = s / (s + k) if s + k else 0.0
    print(f"   {b:3}-{b + 4:3}: {s:7} held / {k:7} ticking  ({frac:5.1%} stalled)")
