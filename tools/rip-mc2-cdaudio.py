#!/usr/bin/env python3
"""Rip MC2's redbook soundtrack from the GOG install's CD image.

The Magic Carpet 2 disc carries the game's music as 27 CD-audio tracks
after the data track (the in-game `SOUND/MUSIC.DAT` is only the MIDI
fallback for CD-less play). The GOG install ships the full raw disc as
`game.gog` with `game.ins` as its cue sheet; this pulls the audio
tracks out and encodes them to FLAC for the engine's future music
support. Data files are NOT extracted here — the importer reads those
straight from the image (`mgc_import::iso`).

Track boundaries come from the cue sheet's file-relative INDEX 01
times; the raw audio is 44.1 kHz signed 16-bit little-endian stereo,
2352 bytes/sector. Needs ffmpeg.

Usage:
    python3 tools/rip-mc2-cdaudio.py "gamedata/Magic Carpet 2" <out-dir>
"""

import os
import re
import shutil
import subprocess
import sys


def parse_cue_tracks(ins_path):
    """[(track_no, kind, start_sector)] from a single-FILE cue sheet."""
    tracks = []
    cur = None
    for line in open(ins_path, encoding="ascii", errors="replace"):
        m = re.match(r"\s*TRACK\s+(\d+)\s+(\S+)", line)
        if m:
            cur = (int(m.group(1)), m.group(2))
            continue
        m = re.match(r"\s*INDEX\s+01\s+(\d+):(\d+):(\d+)", line)
        if m and cur:
            mm, ss, ff = map(int, m.groups())
            tracks.append((cur[0], cur[1], (mm * 60 + ss) * 75 + ff))
            cur = None
    return tracks


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__.strip().splitlines()[-1].strip())
    install, out_dir = sys.argv[1], sys.argv[2]
    ffmpeg = shutil.which("ffmpeg") or sys.exit("error: ffmpeg not found")
    bin_path = os.path.join(install, "game.gog")
    tracks = parse_cue_tracks(os.path.join(install, "game.ins"))
    total = os.path.getsize(bin_path) // 2352

    audio = [(n, s) for n, kind, s in tracks if kind == "AUDIO"]
    os.makedirs(out_dir, exist_ok=True)
    for i, (num, start) in enumerate(audio):
        end = audio[i + 1][1] if i + 1 < len(audio) else total
        out = os.path.join(out_dir, f"track{num:02d}.flac")
        with open(bin_path, "rb") as f:
            f.seek(start * 2352)
            pcm = f.read((end - start) * 2352)
        subprocess.run(
            [ffmpeg, "-y", "-loglevel", "error",
             "-f", "s16le", "-ar", "44100", "-ac", "2", "-i", "pipe:0", out],
            input=pcm, check=True)
        print(f"{out} ({(end - start) / 75:.1f}s)")


if __name__ == "__main__":
    main()
