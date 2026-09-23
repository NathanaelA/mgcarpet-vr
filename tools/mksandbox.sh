#!/usr/bin/env bash
# PARALLEL-DIG SANDBOX BUILDER — recreated once per round since round 137
# until it was finally committed here (round 138).
#
# Gives one dig a PRIVATE tree so concurrent digs never race each other's
# binary. Round 99 swapped the shared `target/release/mgc-conform` ~30 times
# mid-comparison and one dig read a horizon that was purely a swap.
#   tools/mksandbox.sh <digid>          # SBROOT=.claude/sb by default
#   tools/mksandbox.sh --clean          # REMOVE every sandbox + target dir
#
# ⚠⚠⚠ **RUN `--clean` AT THE END OF EVERY ROUND.** Nobody owned this job
# until round 138 and /tmp had silently accumulated **35 GB of dead dig
# sandboxes going back three weeks** (wave134 2.4 GB; round 126's eleven
# sandboxes ~12 GB; assorted pool dumps 1.8 GB) — a 40 GB partition at
# 93% before a single byte of this round was written. A sandbox is
# scratch: it is reproducible from this script in seconds and is never
# worth keeping past the round that made it.
#
# ⚠ SBROOT must NOT be $TMPDIR here: /tmp is a 40 GB partition shared with
# the rest of the system and four sandboxes filled it to 100% in round 138.
# `.claude/` is gitignored and lives on /home.
#
# ⭐ WARM-START THE BUILD instead of compiling four trees from scratch:
#     mkdir -p .claude/sb/.cargo-target-<digid>
#     cp -a target/release .claude/sb/.cargo-target-<digid>/release
# cargo accepts the copied artifacts as up-to-date (rsync -a preserves
# mtimes), so `./build` finishes in 0.2 s and every dig's binary comes out
# byte-identical to HEAD — which is also a free reproducible-build check.
# Creates $SBROOT/<digid> : a private copy of the repo source with the big
# read-only trees SYMLINKED, its own CARGO_TARGET_DIR, and private `build` /
# `conform` wrappers so a dig NEVER races the main session's binary.
set -eu
SRC=/home/rain/projects/mgcarpet
SBROOT="${SBROOT:-$SRC/.claude/sb}"
if [ "${1:-}" = "--clean" ]; then
	[ -d "$SBROOT" ] || { echo "nothing to clean: $SBROOT"; exit 0; }
	echo "removing $(du -sh "$SBROOT" 2>/dev/null | cut -f1) of sandboxes from $SBROOT"
	rm -rf "$SBROOT"
	exit 0
fi
DIG="${1:?usage: mksandbox.sh <digid> | mksandbox.sh --clean}"
DST="$SBROOT/$DIG"
mkdir -p "$DST"
rsync -a --delete \
  --exclude 'target/' --exclude '.git/' --exclude 'recordings/' \
  --exclude 'gamedata/' --exclude 'gamedataX/' --exclude 'baked/' \
  --exclude 'assets/' --exclude 'reference/' --exclude 'saves/' \
  --exclude 'comparison/' --exclude 'recordings-new/' --exclude '.claude/' --exclude 'target' \
  "$SRC/" "$DST/"
for d in recordings gamedata gamedataX baked assets reference comparison; do
  [ -e "$SRC/$d" ] && ln -sfn "$SRC/$d" "$DST/$d"
done
mkdir -p "$DST/saves" "$SBROOT/.cargo-target-$DIG"
cat > "$DST/build" <<'WRAP'
#!/usr/bin/env bash
set -eu
cd "$(dirname "$(readlink -f "$0")")"
export CARGO_TARGET_DIR="$(pwd)/../.cargo-target-$(basename "$(pwd)")"
# ⚠ THE DISTRO CARGO CANNOT BUILD THIS TREE (95 × E0658 unstable
# features). The toolchain is the user-local rustup; a sandbox that
# inherits a bare PATH gets /usr/bin/cargo and fails at the first
# build. Source it here so a dig never has to know.
[ -r "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
exec nice -n 10 cargo build --release "$@"
WRAP
cat > "$DST/conform" <<'WRAP'
#!/usr/bin/env bash
# private memory-capped mgc-conform, pinned to THIS sandbox's own binary
set -u
cd "$(dirname "$(readlink -f "$0")")"
export CARGO_TARGET_DIR="$(pwd)/../.cargo-target-$(basename "$(pwd)")"
while [ $# -gt 0 ]; do
  case "$1" in
    --env) export "${2?}"; shift 2 ;;
    --env=*) export "${1#--env=}"; shift ;;
    *) break ;;
  esac
done
ulimit -v "${MGC_CONFORM_VMAX:-8000000}"
exec nice -n 10 "$CARGO_TARGET_DIR/release/mgc-conform" "$@"
WRAP
chmod +x "$DST/build" "$DST/conform"
echo "sandbox: $DST"
