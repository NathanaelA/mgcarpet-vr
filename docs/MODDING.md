# Community overlay (modding)

Design doc for the `gamedata/overlay/` mechanism: how community-modified
or community-authored data enters the bake, and how a modded build
self-identifies so it can never masquerade as a faithful one. The
user-facing quick-start lives in `gamedata/README.md`; this document is
the normative spec and the extension guide.

## Goal

Two use cases, one mechanism:

1. **Personal modding** — a player drops replacement files into
   `gamedata/overlay/` and rebakes.
2. **A distributed "community mod"** — a separate git repository (e.g.
   `mgc-community`, not part of this project) cloned *as*
   `gamedata/overlay/`. The overlay directory is therefore allowed to
   contain repository furniture (`.git/`, `README*`, `LICENSE*`), which
   the bake ignores.

The seed case: community-"fixed" retail levels — loose `LEVnnnnn.DAT`
files extracted from a retail level archive, edited, and dropped back in
(e.g. a fixed MC1 map 32).

## Layout

```
gamedata/overlay/            ← optional; may be a git clone
├── README.md                ← repository furniture, ignored by the bake
├── mc1/
│   └── levels/
│       └── LEV00032.DAT     ← replaces LEVELS.DAT member 32
├── mc1hw/
│   └── levels/
│       └── LEV00007.DAT     ← replaces DDLEVELS.DAT member 7
└── mc2/
    └── levels/
        └── LEV00003.DAT     ← replaces LEVELS.DAT member 3
```

- Top level: one directory per game tag (`mc1`, `mc1hw`, `mc2` — the
  same tags as the baked tree), plus ignored repository furniture
  (dotfiles, `README*`, `LICENSE*`). Anything else draws a bake-time
  warning, so typos (`mc1_hw/`) cannot be silently inert.
- Per game: one directory per **category**. `levels/` is the only
  implemented category; unknown categories draw a warning naming them
  (an older build reading a newer community checkout tells the user
  exactly what it cannot apply, instead of quietly dropping it).
- There is exactly ONE overlay root, `gamedata/overlay/`. No overlay
  stacking/precedence: if you want a community mod plus personal edits,
  edit the checkout — git already models that (branch, stash, fork).

## Category: `levels/`

Files are named `LEVnnnnn.DAT` — `LEV` + zero-padded 5-digit decimal
member index + `.DAT`, case-insensitive. That is the naming used by the
community's archive-extraction tooling, and the index is the same one
the baked tree uses (`LEV00032.DAT` → `mc1/level-032.mgcl`). Each file
is one *decompressed* archive-member payload, byte-compatible with the
retail member it replaces (MC1/HW: exactly 38 812 bytes; MC2: exactly
the standard level record size — both enforced by the normal level
parser at bake time, so a truncated or wrong-game file fails the bake
with the overlay path in the error).

Rules:

- A file **replaces** the archive member with the same index. Members
  the pristine bake does not emit (empty slots, MC2's extended-format
  dev leftovers) cannot be targeted: such a file is reported as *not
  applied* and skipped. Authoring brand-new levels in empty slots is a
  future extension (it needs campaign wiring), deliberately out of
  scope here.
- Two files resolving to the same member index (e.g. `LEV00032.DAT`
  and `lev00032.dat`) are an error — the bake refuses ambiguous input
  rather than picking one.
- Any other non-dot file inside `levels/` draws a warning (catches
  `LEV0032.DAT`-style near-misses).

## Bake integration

`bake_all` locates the overlay next to the game installs
(`<gamedata>/overlay/`) and hands each archive baker the level list for
its tag (`mgc_import::overlay`). Substitution happens at the archive-
member seam: the member's payload is read from the overlay file instead
of decompressed from the archive, then flows through the exact same
parse → package → native terrain generation path as retail data. Every
application is printed (`mc1: level 032 OVERLAY mc1/levels/LEV00032.DAT`).

**The overlay is NOT epoch-tracked.** `BAKE_EPOCH` describes importer
*code*, not local mod state, and the auto-rebake check
(`mgc-app/src/bakecheck.rs`) only inspects epoch/version stamps. After
adding, changing, or removing overlay files, **delete `baked/` and let
the next run rebake** (or run `mgc-import bake` yourself). This is the
accepted contract — do not bolt overlay fingerprints onto the epoch.

## Provenance and the hash policy (the crux)

Level data feeds the sim: THING tables, wizard configs, gen params all
enter the state hash. A modded bake is therefore **not a faithful
fixture** — state-hash goldens and retail conformance are meaningless
against it. The policy, enforced at three layers:

1. **Per package** — an overlaid level's `meta.json` carries
   `overlay: "<overlay-relative path>"` (FORMAT.md). Its `source` block
   is kept (the member slot it replaces) but `entry_sha256` is the
   SHA-256 of the *overlay file*, not the retail entry, so the save
   identity check keeps working: a save made on a modded level refuses
   to load against the pristine bake of the same level, and vice
   versa, exactly like any other level-data mismatch.
2. **Per tree** — a bake that applied ANY overlay file writes a
   `MODDED` marker at the baked root listing every substitution; a
   bake that applied none removes it. One glance (or one `test -f`)
   answers "is this tree pristine?".
3. **Per run** —
   - the game prints one line per overlaid level at load, in the
     G-class style: `level: OVERLAY mc1/levels/LEV00032.DAT —
     community-modified data, not a faithful run`;
   - `mgc-conform` refuses to verify a recording against an overlaid
     package (hard error naming the overlay file);
   - the `mgc-sim` golden suites treat a `MODDED` baked tree as a
     golden-skip (and under `MGC_REQUIRE_GOLDENS=1`, skip = failure,
     so fixture CI can never silently bless a modded bake).

`manifest.sha256` stays what it always was — the hashes of what was
actually baked. The `MODDED` marker, not the manifest, is the
pristine/modded discriminator.

### Engine fixes vs data fixes

Some community level fixes address bugs this port already fixes
engine-side (e.g. buried unpickable jars vs the class-2 ground-snap).
Both simply apply: overlay data runs through the same engine, fixes and
all (`docs/DEVIATIONS.md` is unaffected — the overlay changes *data*,
never engine law). Where both fix the same symptom the result is
idempotent; nothing needs to "win".

## Extending the overlay (new categories)

Adding a category (say `mc2/sprites/`) is deliberately boring:

1. Teach `mgc_import::overlay` the category name and its file-naming
   rule (this removes it from the unknown-category warning).
2. Consume it at the equivalent seam in the bake (for bundle-fed data:
   before the bundle baker parses the source file), stamping the same
   provenance: `overlay` marker in the artifact's manifest, entry in
   the `MODDED` marker file.
3. Decide the hash question explicitly: data that feeds the sim
   (levels, tables, spell params) → full modded treatment as above.
   Presentation-only data (sprites, textures, sounds, music) does NOT
   feed the state hash — it may skip the per-run refusals, but still
   gets the `MODDED` marker + load-time print, because "which bake am
   I looking at" must stay a one-glance question.
4. Update the three docs in lockstep: this file, `gamedata/README.md`
   (user-facing), `docs/FORMAT.md` (if an artifact schema grew a
   field).

Categories that are anticipated but NOT designed yet: full custom
levels in empty slots (campaign wiring), asset/sprite/texture
replacement (bundle seam), audio replacement (mgc-audio is hash-safe by
construction), campaign/text overrides. Do not implement any of them
without their own design pass — this document only reserves the
structure they would slot into.
