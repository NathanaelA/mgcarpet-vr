#!/usr/bin/env python3
"""Render README clips from recordings: a SHOT LIST in, GIF/MP4/still out.

A shot is `(take, from, to)` plus a camera and a few encoding knobs.
The binary renders it with `--replay ... --film` (a fixed clock, one
frame per 1/rate turn, nothing dropped — the same shot renders the same
frames on any machine), this script hands the frames to ffmpeg. Every
picture in the README is therefore a REPLAY of a certified take and is
re-rendered after any visual change by re-running this script.

    tools/film.py                      # every shot in docs/media/shots.json
    tools/film.py mc2-battle           # one shot by name
    tools/film.py --take recordings/mc2l7.mgcr --from 3000 --to 3120 \\
                  --thirdperson --name try1   # a one-off, not in the list

The shot list (`docs/media/shots.json`) is a JSON array of objects:

    {
      "name": "mc2-battle",            # output basename (docs/media/<name>.*)
      "take": "recordings/mc2l7.mgcr",
      "from": 3000, "to": 3120,        # game turns (24 per second)
      "rate": 1,                       # frames per turn (1 = 24 fps)
      "camera": "thirdperson",         # or "firstperson" (default)
      "args": ["--fog-distance", "40"],# any extra mgcarpet flags
      "width": 640,                    # output width (all clips)
      "webp": true, "quality": 75,     # animated WebP, film fps (the default)
      "gif": false, "gif_fps": 12,     # GIF, opt-in (see below)
      "mp4": false,                    # H.264, film fps
      "still": 60,                     # also copy frame 60 as <name>.png
      "note": "the volcano under the rival's castle"
    }

Only `name`, `take`, `from`, `to` are required. Needs `ffmpeg` on PATH
(or `--ffmpeg`), a display (the film runs the windowed game — under
Xvfb is fine), and a built binary (`target/release/mgcarpet`, or
`--bin` / `$MGCARPET_BIN`).

Why WebP: a 3D flythrough changes every pixel every frame, which is
the worst case for GIF — measured on a 5 s third-person shot at 640
wide, GIF was 24 MB at 24 fps and still 12 MB at 12 fps; animated
WebP (which GitHub renders in a README) was 3.3 MB at 24 fps. GIF
stays available for places that cannot show WebP, at 12 fps.
"""
import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SHOTS = ROOT / "docs" / "media" / "shots.json"
OUT = ROOT / "docs" / "media"
TICK_RATE_HZ = 24

DEFAULTS = {
    "rate": 1,
    "camera": "firstperson",
    "args": [],
    "width": 640,
    "webp": True,
    "quality": 75,
    "gif": False,
    "gif_fps": 12,
    "mp4": False,
    "still": None,
}


def run(cmd, dry, quiet=False):
    """Run `cmd`; with `quiet`, swallow its output unless it fails
    (the game prints its whole settings summary at boot)."""
    print("+", " ".join(str(c) for c in cmd), flush=True)
    if dry:
        return ""
    r = subprocess.run([str(c) for c in cmd], cwd=ROOT, text=True,
                       capture_output=quiet)
    if r.returncode != 0:
        if quiet:
            print("\n".join((r.stdout + r.stderr).splitlines()[-20:]))
        sys.exit(f"{cmd[0]} exited {r.returncode}")
    return r.stdout or ""


def film(shot, binary, frames, dry):
    """Render the shot's frames into `frames` with the game binary."""
    cmd = [
        binary,
        "--replay", ROOT / shot["take"],
        "--film", frames,
        "--film-from", shot["from"],
        "--film-to", shot["to"],
        "--film-rate", shot["rate"],
        "--" + shot["camera"],
        *shot["args"],
    ]
    out = run(cmd, dry, quiet=True)
    for line in out.splitlines():
        if line.startswith("film:"):
            print("  ", line)


def encode(shot, ffmpeg, frames, out, dry):
    fps = shot["rate"] * TICK_RATE_HZ
    src = [ffmpeg, "-hide_banner", "-loglevel", "error", "-y",
           "-framerate", fps, "-i", frames / "%06d.png"]
    w = shot["width"]
    scale = f"scale={w}:-2:flags=lanczos"
    made = []
    if shot["webp"]:
        path = out / f"{shot['name']}.webp"
        # (compression_level 6 was measured at 80x the time of the
        # default 4 for 10% smaller — not worth it.)
        run([*src, "-vf", scale, "-c:v", "libwebp_anim", "-lossless", "0",
             "-quality", shot["quality"], "-loop", "0", path], dry)
        made.append(path)
    if shot["gif"]:
        gif_fps = shot["gif_fps"] or fps
        # Two-pass palette: one palette from the whole clip (diff
        # stats weight the moving parts), then dithered use of it.
        vf = (f"fps={gif_fps},scale={w}:-1:flags=lanczos,"
              "split[a][b];[a]palettegen=stats_mode=diff[p];"
              "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle")
        path = out / f"{shot['name']}.gif"
        run([*src, "-vf", vf, "-loop", "0", path], dry)
        made.append(path)
    if shot["mp4"]:
        path = out / f"{shot['name']}.mp4"
        run([*src, "-vf", scale, "-c:v", "libx264",
             "-pix_fmt", "yuv420p", "-crf", "20", "-movflags", "+faststart", path], dry)
        made.append(path)
    if shot["still"] is not None:
        frame = frames / f"{int(shot['still']):06d}.png"
        path = out / f"{shot['name']}.png"
        print(f"+ cp {frame} {path}")
        if not dry:
            shutil.copyfile(frame, path)
        made.append(path)
    return made


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("names", nargs="*", help="shot names from the list (default: all)")
    ap.add_argument("--shots", type=Path, default=SHOTS, help=f"shot list (default {SHOTS.relative_to(ROOT)})")
    ap.add_argument("--out", type=Path, default=OUT, help="output directory")
    ap.add_argument("--bin", default=os.environ.get("MGCARPET_BIN", ROOT / "target" / "release" / "mgcarpet"))
    ap.add_argument("--ffmpeg", default=shutil.which("ffmpeg") or "ffmpeg")
    ap.add_argument("--keep-frames", action="store_true", help="leave the PNG frames beside the output")
    ap.add_argument("--dry-run", action="store_true", help="print the commands only")
    one = ap.add_argument_group("one-off shot (instead of the list)")
    one.add_argument("--take")
    one.add_argument("--from", dest="from_", type=int)
    one.add_argument("--to", type=int)
    one.add_argument("--rate", type=int)
    one.add_argument("--name")
    one.add_argument("--thirdperson", action="store_true")
    one.add_argument("--width", type=int)
    one.add_argument("--mp4", action="store_true")
    one.add_argument("--gif", action="store_true")
    one.add_argument("--no-webp", action="store_true")
    one.add_argument("--quality", type=int, help="WebP quality 0-100 (default 75)")
    one.add_argument("--still", type=int, help="also copy this frame as <name>.png")
    one.add_argument("--gif-fps", type=int)
    one.add_argument("--game-args", default="", help="extra mgcarpet flags, one quoted string")
    a = ap.parse_args()

    if a.take:
        if a.from_ is None or a.to is None:
            ap.error("--take needs --from and --to")
        shot = {"name": a.name or f"{Path(a.take).stem}-{a.from_}-{a.to}",
                "take": a.take, "from": a.from_, "to": a.to}
        if a.rate: shot["rate"] = a.rate
        if a.thirdperson: shot["camera"] = "thirdperson"
        if a.width: shot["width"] = a.width
        if a.mp4: shot["mp4"] = True
        if a.gif: shot["gif"] = True
        if a.no_webp: shot["webp"] = False
        if a.quality is not None: shot["quality"] = a.quality
        if a.still is not None: shot["still"] = a.still
        if a.gif_fps: shot["gif_fps"] = a.gif_fps
        if a.game_args: shot["args"] = shlex.split(a.game_args)
        shots = [shot]
    else:
        shots = json.loads(a.shots.read_text())
        if a.names:
            by = {s["name"]: s for s in shots}
            missing = [n for n in a.names if n not in by]
            if missing:
                sys.exit(f"not in {a.shots}: {', '.join(missing)}")
            shots = [by[n] for n in a.names]

    if not a.dry_run and not Path(a.bin).is_file():
        sys.exit(f"no game binary at {a.bin} (cargo build --release, or --bin)")
    a.out.mkdir(parents=True, exist_ok=True)
    for raw in shots:
        shot = {**DEFAULTS, **raw}
        for k in ("name", "take", "from", "to"):
            if k not in shot:
                sys.exit(f"shot {raw} lacks {k!r}")
        print(f"\n== {shot['name']}: {shot['take']} turns {shot['from']}..{shot['to']} "
              f"({(shot['to'] - shot['from']) / TICK_RATE_HZ:.1f} s, {shot['camera']})")
        if a.keep_frames:
            frames = a.out / f"{shot['name']}.frames"
            shutil.rmtree(frames, ignore_errors=True)
            frames.mkdir(parents=True)
            tmp = None
        else:
            tmp = tempfile.TemporaryDirectory(prefix=f"film-{shot['name']}-")
            frames = Path(tmp.name)
        try:
            film(shot, a.bin, frames, a.dry_run)
            n = len(list(frames.glob("*.png")))
            if not a.dry_run and n == 0:
                sys.exit("the film produced no frames (does the take reach --from?)")
            made = encode(shot, a.ffmpeg, frames, a.out, a.dry_run)
        finally:
            if tmp:
                tmp.cleanup()
        for p in made:
            if p.exists():
                shown = p.relative_to(ROOT) if p.is_relative_to(ROOT) else p
                print(f"   {shown}  {p.stat().st_size / 1e6:.2f} MB")


if __name__ == "__main__":
    main()
