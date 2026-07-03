#!/usr/bin/env python3
"""MC1-params-through-MC2-generator experiment.

For each selected MC1 level: pull GEN_MAP params + entities from the baked
.mgcl, synthesize a fake MC2 level buffer with the params at the offsets
mc2-genlevel reads, run the oracle, and score coherence: do water entities
(kraken/splash) land on generated water, do land entities land on land?
Renders each result as a PNG with entity overlay.
"""
import zipfile, json, struct, subprocess, sys, os
import numpy as np
from PIL import Image, ImageDraw

REPO = '/home/rain/projects/mgcarpet'
ORACLE = f'{REPO}/tools/mc2-genlevel/mc2-genlevel'
SCRATCH = os.path.dirname(os.path.abspath(__file__))
OUT = f'{SCRATCH}/mc1exp'
os.makedirs(OUT, exist_ok=True)

MC2_LEVEL_SIZE = 26116

def fake_mc2_level(g, lriver=0, map_type=0):
    buf = bytearray(MC2_LEVEL_SIZE)
    struct.pack_into('<H', buf, 0x00, 2)          # version
    buf[0x06] = map_type
    struct.pack_into('<H', buf, 0x17, g['seed'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x1B, g['off'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x1F, g['raise'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x23, g['gnarl'] & 0xFFFF)
    struct.pack_into('<I', buf, 0x27, g['river'] & 0xFFFFFFFF)
    struct.pack_into('<H', buf, 0x2B, lriver & 0xFFFF)
    struct.pack_into('<H', buf, 0x2F, g['sourc'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x33, g['snlin'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x37, g['snflt'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x3B, g['bhlin'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x3F, g['bhflt'] & 0xFFFF)
    struct.pack_into('<H', buf, 0x43, g['rkste'] & 0xFFFF)
    return bytes(buf)

def run_oracle(levelbytes, tag):
    inp, outp = f'{OUT}/{tag}-in.bin', f'{OUT}/{tag}-out.bin'
    with open(inp, 'wb') as f: f.write(levelbytes)
    r = subprocess.run([ORACLE, inp, outp], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f'oracle failed on {tag}: {r.stderr.strip()}')
    blob = open(outp, 'rb').read()
    ttype = np.frombuffer(blob[0:0x10000], np.uint8).reshape(256, 256)
    height = np.frombuffer(blob[0x10000:0x20000], np.uint8).reshape(256, 256)
    os.unlink(inp); os.unlink(outp)
    return ttype, height

# Water entities vs land entities for the coherence test.
WATER_CM = {(5, 6), (10, 5)}            # kraken, splash
LAND_CM  = {(2, 0), (2, 1), (2, 2), (2, 3),   # trees/stones
            (5, 4), (5, 7), (5, 9), (5, 12), (5, 13), (5, 14),  # ground units
            (5, 5), (5, 3)}             # crab, worm

def load_mc1(path):
    z = zipfile.ZipFile(path)
    g = json.load(z.open('genparams.json'))
    things = json.load(z.open('things.json'))['things']
    ents = [t for t in things if t['kind'] == 'entity']
    return g, ents

def coherence(height, ents, swap):
    water = height == 0
    def at(t):
        x, y = t['x'], t['y']
        return water[x, y] if swap else water[y, x]
    w = [t for t in ents if (t['class'], t['model']) in WATER_CM]
    l = [t for t in ents if (t['class'], t['model']) in LAND_CM]
    wr = sum(at(t) for t in w) / len(w) if w else None
    lr = sum(not at(t) for t in l) / len(l) if l else None
    return wr, len(w), lr, len(l)

HYPSO = [(0, (60, 90, 170)), (1, (200, 190, 130)), (30, (90, 140, 60)),
         (90, (130, 120, 70)), (150, (120, 110, 100)), (197, (240, 240, 245))]

def render(ttype, height, ents, path, swap):
    img = np.zeros((256, 256, 3), np.uint8)
    h = height.astype(int)
    for i in range(len(HYPSO)):
        lo, c0 = HYPSO[i]
        hi, c1 = HYPSO[i + 1] if i + 1 < len(HYPSO) else (255, HYPSO[i][1])
        m = (h >= lo) & (h < hi) if i else (h == 0)
        if i == 0:
            img[m] = c0
            continue
        t = ((h - lo) / max(hi - lo, 1)).clip(0, 1)[..., None]
        cc = (np.array(c0) * (1 - t) + np.array(c1) * t).astype(np.uint8)
        img[m] = cc[m]
    # hillshade from height gradient
    gy, gx = np.gradient(h.astype(float))
    shade = (1.0 - np.clip((gx + gy) * 0.04, -0.4, 0.4))
    img = (img * shade[..., None]).clip(0, 255).astype(np.uint8)

    im = Image.fromarray(img).resize((512, 512), Image.NEAREST)
    d = ImageDraw.Draw(im)
    for t in ents:
        cm = (t['class'], t['model'])
        x, y = (t['y'], t['x']) if swap else (t['x'], t['y'])
        px, py = x * 2, y * 2
        if cm in WATER_CM: col = (0, 255, 255)
        elif cm in LAND_CM: col = (255, 60, 60)
        elif t['class'] == 3: col = (255, 255, 0)
        else: col = (255, 0, 255)
        d.ellipse([px - 2, py - 2, px + 2, py + 2], outline=col)
    im.save(path)

def main(levels):
    print(f"{'level':<14} {'water%':>6} | {'kraken@water':>16} {'land@land':>14} (xy) | same swapped")
    for lv in levels:
        game, idx = lv
        path = f'{REPO}/baked/{game}/level-{idx:03}.mgcl'
        g, ents = load_mc1(path)
        tag = f'{game}-{idx:03}'
        try:
            ttype, height = run_oracle(fake_mc2_level(g), tag)
        except RuntimeError as e:
            print(f'{tag}: {e}'); continue
        waterpct = (height == 0).mean() * 100
        wr0, nw, lr0, nl = coherence(height, ents, swap=False)
        wr1, _, lr1, _ = coherence(height, ents, swap=True)
        fmt = lambda r, n: f'{r*100:5.0f}% ({n})' if r is not None else f'   —  ({n})'
        print(f'{tag:<14} {waterpct:5.1f}% | {fmt(wr0,nw):>16} {fmt(lr0,nl):>14} | {fmt(wr1,nw):>10} {fmt(lr1,nl):>10}')
        render(ttype, height, ents, f'{OUT}/{tag}.png', swap=False)

if __name__ == '__main__':
    main([('mc1', i) for i in [0, 1, 2, 5, 10, 20, 30, 40, 50, 60]])
