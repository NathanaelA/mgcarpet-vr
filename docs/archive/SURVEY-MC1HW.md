# Hidden Worlds (MC1HW) — support survey & implementation spec

Scope for adding full **Magic Carpet: Hidden Worlds** support — MC1's 1995
data disk, shipped inside "Magic Carpet Plus". Produced 2026-07-14 from a
side-by-side read of the two decompiles (`reference/remc1` vs
`reference/remc1hw`) by a parallel-agent sweep, with every load-bearing
claim re-verified directly against the decompiles. **This is a scoping
document — no code has been changed.**

> Method note for anyone re-reading the decompiles: a raw text diff of the
> two `sub_main.cpp` files is useless (~68k lines differ from decompiler
> noise — independent decompilations, different naming/completeness). HW
> marks **456 functions `//SYNCHRONIZED WITH REMC1`** (author-verified
> identical). Compare semantically; two agent headlines this session did
> **not** survive verification (see "Corrections" at the end).

---

## TL;DR

Hidden Worlds is the **MC1 engine binary with a single compiled `bool
IsHiddenWord` flipped to `true`**, plus different data files. It is *not* a
fork. The Rust chassis is already fully in place (`GameId::Mc1Hw`,
`Game::HiddenWorlds`, the `DDLEVELS` bake path, the `mc1-arctic` bundle,
the `mc1hw:N` CLI selector) — **HW bakes, loads, and runs today**, treated
byte-identically to MC1.

The remaining work is a **small gameplay/data delta**, dominated by **one
spell**. There is **no importer, level-format, asset-pipeline, or loader
work**. The renderer-effects wishlist (sky/reflections/blur) is a
*separate, shared* MC1+MC2 track, not part of HW — **and there is no
snowfall** (a mid-session myth, corrected below).

---

## 1. Current state — what already exists (baseline)

`mc1hw` bakes + loads + runs end-to-end today:

- **Identity** — `GameId::Mc1Hw` (`crates/mgc-sim/src/ids.rs:28`); aliases
  MC1 in every profile: `chassis()`→`ChassisParams::MC1` (`:36`),
  `verbs()`→`VerbSet::MC1` (`:44`), `known_thing()`→`mc1::known_thing`
  (`:65`); differs only in `name()`→`"mc1hw"` (`:161`). The future
  override point is already flagged at `ids.rs:60-62`.
- **Formats** — `Game::HiddenWorlds`, `#[serde(rename="mc1hw")]`
  (`crates/mgc-formats/src/lib.rs:85-86`).
- **Bake** — `(Game::HiddenWorlds, "mc1hw", "LEVELS/DDLEVELS")`
  (`crates/mgc-import/src/bake.rs:511`), via the shared `bake_mc1_archive`.
- **Assets** — the `mc1-arctic` bundle variant (`crates/mgc-import/src/
  bundle.rs:136-150`): `PAL1-0` / `DTABLES.DAT` / `BLK1-1` / `TMAPS1-0` /
  `BUILD1-0` / `HSPR1-0`.
- **App** — `HiddenWorlds → tileset 1 → "mc1-arctic"`
  (`crates/mgc-app/src/main.rs:319,331,362`); CLI `--level mc1hw:N`
  (`:2447-2463`).
- **Auto-bake** recognizes `mc1hw` (`crates/mgc-app/src/bakecheck.rs:107`).
- **Baked output present**: `baked/mc1hw/` (73 levels), `baked/assets/
  mc1-arctic/`, `baked/maps/mc1hw/` (73 PNGs).
- **Tests green**: `mc1_hidden_worlds_levels_extract`
  (`crates/mgc-import/tests/levels.rs:53`),
  `hidden_worlds_levels_parse_and_hold_invariants`
  (`tests/level_mc1.rs:103`). HW rides MC1's full sim/golden suite (no
  dedicated mc1hw sim test yet).

ROADMAP anchors: `docs/ROADMAP.md:143-155` (arctic tileset RESOLVED),
`:2619-2620` ("sim treats Mc1|Mc1Hw identically everywhere — verified"),
Phase 4.8 `:5191-5219` (the HW delta pass, unstarted).

---

## 2. Engine-logic delta — the complete set (12 `IsHiddenWord` branches)

Both decompiles reproduce the **same 12** `IsHiddenWord` sites; that IS the
entire logic delta. Of them exactly **one** is a live gameplay fork, one is
a dormant seam, and ten are data-path / front-end / inert.

| remc1 line | What differs | Class |
|---|---|---|
| `:24` / hw `:8` | the flag itself (`false` vs `true`) | already `GameId::Mc1Hw` |
| `:31148` | actor life-tick: base decrements actLife+visual, HW skips | **sim** |
| `:62711/63115/63720` | mana-shield reflect accepts model **53** as well as {1,17} | **sim (latent)** |
| `:64891` | flag-bit OR-vs-set — mathematically identical | noise |
| `:72898` | VESA mode `0x4501`→`0x4F01` | DOS video |
| `:41543` | skips `sub_34070()` — an **empty stub** | inert |
| `:49842` | asset dir: `tmaps0-0`+`levels` → `tmaps1-0`+`ddlevels` | data (seamed) |
| `:59422` | world-map grid rows 20→10 | app UI |
| `:59940` | world-map cells 50→25 | app UI |
| `:60285/60305` | intro title art (`title02→04`, `title-01→title-03`) | app art |

**Napalm geometry (the one live fork).** The Wall-of-Fire fire-curtain
handler (class-10 **state 58**): base lays a **112-unit** grid spawning
`(10,6)`, cap 14; HW lays a **160-unit** grid spawning `(10,0)`, cadence
`(+2)%7`, sets a persistent flag and fires sound 30 once. remc1
`:31140-31237` vs remc1hw `:29740-29773`. **Ported MC1-only, NO seam**, in
`crates/mgc-sim/src/mc1/combat.rs:2683` (`napalm_tick`). ⚠️ **The doc
comment there mislabels the HW else-branch as a "multiplayer branch" — it
is the Hidden-Worlds path.** Fix the comment and add a `Mc1`/`Mc1Hw`
branch. (Relationship to the spell-20 rework in §3 is unconfirmed — the
napalm cloud and the fire projectile are different entities; treat as a
related-but-separate edit until traced.)

**Model-53 shield reflect (latent).** When a class-10 projectile strikes a
mana-shielded target, the reflect+¼-drain fires for model ∈ {1,17} (MC1) vs
{1,17,53} (HW). Ported at `combat.rs:2149` but currently **model-agnostic
and dormant** (the shield flag is never set yet — see the module note at
`combat.rs:25`). No action until wizard shields are wired; then add the
gated model check.

Everything else (world-map grid, intro art, VESA, the empty stub, the
no-op flag arithmetic) is app/front-end or inert — no sim work.

---

## 3. The spell-20 rework — the headline (VERIFIED)

HW turns spell **20** ("Fire Storm" / "Wall of Fire" / player's "raining
fire") into a **homing meteor**. This is a real behavioral change plus a
rebalance plus new visuals — **not** a mere stat tweak (see Corrections).

### 3a. New targeting (homing) — VERIFIED by direct switch diff

The one-time target-acquisition fn (`sub_54520`) switches on the
projectile's model (`+65`). Complete case lists (verified directly):

- **base MC1** (remc1 `:63977`): `{0, 1, 3, 4, 7, 8, 9, 0xB, 0xC}`
- **Hidden Worlds** (remc1hw `:60100`): `{0, 1, 3, 4, 7, 8, 9, 0xB, 0xC,
  0x10, 0x12, 0x13}`

HW **adds `0x10` (16), `0x12` (18), `0x13` (19)**, folding them into the
`{0,3,4}` homing group (HW also widens the horizontal acquire cone to
`0x100` vs the usual `0x71`). **Model 16 is the Fire Storm fire child** —
our port already spawns it as the state-17 "m16 firewall" bolt. In base
MC1 model 16 isn't in the switch → no acquire → flies straight (fire-rain);
in HW it acquires → **homes**. That is the "homing meteor."

Rust side: `crates/mgc-sim/src/mc1/combat.rs:1177` (`proj_firewall_tick`)
**already carries the dormant homing branch** (`if f146 != 0 { home }`) but
never calls the acquire scan for model 16 (it isn't an MC1 acquire case) —
so it's inert for MC1, matching remc1. **HW wiring = arm the acquire scan
for model 16 (+ 18/19) behind a per-game seam**, e.g. `TargetingVerb::
Mc1Hw`, plus the widened `0x100` cone. Existing seam concept, new code.

### 3b. Stat rebalance — VERIFIED (both agents agree)

Spell-20 spawn call is the **only** one of 24 that differs:
- base `sub_3BF70(a1,20,60,5000,51,1,0,12000,24464)` (remc1 `:48142`)
- HW   `sub_3C2B0(a1,20,60,5000,26,1,0,60000,5000)` (remc1hw `:44266`)

| field | base | HW | Rust (`crates/mgc-sim/src/mc1/spells.rs:~152`) |
|---|---|---|---|
| burst count (+50) | 51 | **26** | `SPELLS[20].count` |
| castle-req mana (+132) | 12000 | **60000** | `SPELLS[20].castle_req` |
| damage (+44) | 24464 | **5000** | `SPELLS[20].damage` |

(total mana 5000 and the fire/charge flags are unchanged.) Damage 24464 is
the value our port already flags "pretty useless"; HW's 5000 is the sane
intended value. The fire child inherits `+44`, so the damage change is felt.

There is **no per-game `SPELLS` table** today (one global const applied to
MC1 and HW alike). Needs a variant-gated override for row 20 — keep it
behind the variant so **MC1 goldens are untouched**.

### 3c. New visuals — REAL but exact wiring UNTRACED

Player-confirmed: new icon, new projectile, new effect. Agents split on the
mechanism: some sprite ids appear as literals in the projectile ctors
(e.g. model-17→sprite 76, remc1hw `:23609`; a model-51 spawn with sprite
177 at `:29353/40669/43642`), while HW's model→sprite descriptor table is
**stubbed in the decompile** (`off_97D12[1];//fix it`, remc1hw `:4045`), so
part of the difference resolves through the **arctic HSPR asset bank** at
bake time, not code. **Action:** one clean trace of spell-20's child →
sprite path + a check that `mc1-arctic` actually bakes distinct bitmaps at
those indices, before implementing. Visuals need **no renderer work** —
icon (`icon_sprite = id+6`) and projectile resolve through the variant
sprite bank automatically.

---

## 4. Levels — no work (VERIFIED)

HW levels use the **byte-identical MC1 format**. `LoadLevel` is the same
routine (remc1 `:49284-49325` vs remc1hw `:45405-45457`): same RNC
container, same `0x979C = 38812`-byte struct, same TAB indexing; the only
difference is the filename literal (`ddlevels` vs `levels`). No new
objective/mission TYPE — the win engine is not gated by the HW flag.

- **73 levels**: indices 0-69 contiguous + 102/198/199 (a dev-leftover
  tail band; 198 is a pure-terrain arena, already noted at
  `crates/mgc-import/tests/level_mc1.rs:32-40`). All already bake to
  `baked/mc1hw/` and pass parse/invariant tests.
- **`(10,53)` — dormant "mystery spell", NO work (census-settled).** Traced
  + censused 2026-07-15: authored in **0 levels** in either game (base MC1
  and HW alike), so it never spawns in play. It is dormant/cut content — a
  stationary parasitic attach-and-mimic entity, fully wired but unused. Full
  trace + revive-and-test recipe moved to **docs/MYSTERY-SPELL-1053.md**.
  Not on the HW delta.

---

## 5. Assets — arctic variant covers gameplay 100% (VERIFIED)

The engine's `IsHiddenWord` flag selects the arctic `{1}` family wholesale;
the `mc1-arctic` `VariantSpec` already bakes every gameplay-critical
catalog (palette, tables, atlas, tmaps, build, HSPR). Audio/font/search are
**shared** with MC1 (HW references only `snds0-0`/`music0-0` — no `-1`
audio exists). Level pack = `ddlevels`, baked as `mc1hw`.

Two arctic assets are **not** yet baked, both **symmetric gaps that also
affect base MC1** (so neither is an HW-parity blocker):
- **Textured sky `sky1-0.dat`** — the `VariantSpec` has no `sky` field; the
  app draws a flat sky color. (Belongs to the renderer track, §8.)
- **Half-size sprite bank `mspr1-0`** — the low-res UI alternate (320-wide
  mode); only HSPR is baked. Not needed unless a 320-wide UI is added.

Intro/menu screens (`title-04`, `mainmenu`, …) are not imported at all —
a future front-end track, not a tileset gap.

### The TMAPS1-0 corruption warning — FAITHFUL, no fix (VERIFIED)

A full bake logs (not a crash — the bake completes):
```
note: mc1-arctic: sprite 153: 1x122 with 5856-byte payload (flags 0x0002) — corrupt entry, baked frame-less
note: mc1-arctic: sprite 156: 1x122 with 1566-byte payload (flags 0x0002) — corrupt entry, baked frame-less
```
Arctic `TMAPS1-0` entries 153/156 ship with a mangled 6-byte header
(`1×122`) over a byte-perfect copy of the **previous** entry's pixels
(153 = 6 + 90×65 = entry 152's; 156 = 6 + 40×39 = entry 155's). Temperate
`TMAPS0-0` has valid 153/156, so **only HW hits this**.

Verdict: our frame-less skip (`crates/mgc-import/src/sprites.rs:93-111`) is
**faithful**. The HW engine's TMAPS loader + blitter are `SYNCHRONIZED WITH
REMC1` — identical payload struct (`Basic.h`), reads width/height from the
embedded header, **no fixup** (confirmed: zero `->xx =`/`->yy =` writes in
either decompile; the `group` field is index-only). Retail HW reads the
same mangled `1×122` header → blits a 1px transparent sliver = effectively
nothing, same as ours. It's a ship-with-it authoring bug in the arctic
data, not a loader gap. **No loader change.** Only nit: the code comment
implies `flags 0x0002` is part of the corruption — it's the normal value;
the corruption is solely the `1×122` dims.

---

## 6. Implementation plan — CORE LANDED 2026-07-15

All behind the variant; MC1 + MC2 goldens verified untouched (198 sim
tests green). The spell-20 delta + napalm fork are DONE.

1. ✅ **Per-game seam** — `spells(GameId)` accessor (spells.rs, `SPELLS_HW`
   overrides row 20 only); MC1-family `SPELLS[id]` reads routed through
   `World::spells()`. `TargetingVerb::Mc1Hw` + `VerbSet::MC1HW`, wired via
   `ids.rs::verbs(Mc1Hw)`. `Gen::is_hidden_worlds()` is the single HW
   predicate (keyed off the one HW-distinct verb).
2. ✅ **Spell-20 stat override** (§3b) — `SPELLS_HW[20]` = count 26 /
   castle_req 60000 / damage 5000.
3. ✅ **Spell-20 homing** (§3a) — `proj_firewall_tick` arms the acquire
   scan (new `aim_assist_mc1_cone`) at yaw cone **0x100** / pitch 0x71 for
   the m16 child under HW. CORRECTION to the survey: case `0x10` (model 16)
   is its OWN acquire case with the widened YAW cone (remc1hw :60322);
   models 18/19 fold into the `{0,3,4}` group at 0x71 but are behaviorally
   inert here (18 = the caster-ridden Global Death fuse, 19 = no
   projectile), so the only LIVE homing arm is model 16.
4. ✅ **Napalm branch** (§2) — `napalm_tick_hw` (160-grid `(10,0)` single
   EXPANDING ring, `(var26+2)%7` cadence, sound 30 once, `actLife`
   terminator) forked on `is_hidden_worlds()`; comment mislabel fixed. The
   (10,53) CREATOR is also forked (the emit trace, §3/§7): base MC1
   `sub_3B8E0` (life 128 / f44 100 / extents / random yaw) vs HW `sub_3BC60`
   (life 6 / f44 3000 / no extents / no yaw LCG draw).
5. ~~**`(10,53)` THING**~~ — **DROPPED** (census: 0 levels; §4).
6. **Spell-20 visuals** (§3c) — OPEN. After the sprite-path trace; likely
   asset-only (verify `mc1-arctic` bakes distinct bitmaps).
7. **App UI** (optional, §2) — OPEN. World-map grid 20→10 / 50→25.
8. ✅ **Tests** — `hidden_worlds_spell20_stats_diverge_only_at_row_20`,
   `hidden_worlds_verbset_wiring_preserves_discriminants`,
   `hidden_worlds_firewall_child_homes_in_the_widened_cone` (behavioral,
   end-to-end via `proj_tick`). Census instrument: `crates/mgc-sim/
   examples/tmp_hwcensus.rs`.

RESIDUAL / OPEN after this session:
- **PLAYTEST OWED** — none of the above is player-certified yet.
- **Base-MC1 spray gap** (trace §1): base MC1 spell-20 fires a MULTI-bolt
  spray per manifestation tick (the `while(*(a1+61)>=0)` loop, remc1
  :66135); HW fires a SINGLE homing bolt. Our port already emits one bolt
  per cast — so HW is right, but base MC1's multi-bolt spray is a
  PRE-EXISTING fidelity gap unrelated to HW (own follow-up).
- The **TMAPS-156 arctic-tree** reachability question (§5/§7) — separate
  loader/asset trace, unstarted.
- Steps 6 (visuals) + 7 (app UI).

Deferred/out-of-scope: the mana-shield model-53 gate (latent until shields
ship), `sky1-0`/`mspr1-0` (renderer/UI tracks), intro/menu front-end.

---

## 7. Must-verify before implementing (open questions)

- **Reachability census — DONE 2026-07-15** (`crates/mgc-sim/examples/
  tmp_hwcensus.rs`, run vs `baked/mc1hw` and `baked/mc1`). Results:
  - **spell 20**: allowed (mask) in **69/73** HW levels, pre-granted in
    **37** → the homing + rebalance are REAL, broadly reachable work.
  - **`(10,53)`**: **0 levels** in either game → dropped (§4).
  - **new-homing models 16/18/19**: appear only as unrelated creature/mana
    THINGS — the acquire switch keys on the *projectile's* model (the model-
    16 fire child, cast-time), so the homing work is gated purely by spell-
    20 availability, not by any placement.
  - **TMAPS/sprites**: base **153 never reached**; **76/177 never reached as
    placements** (both are cast-time projectile sprites — expected). BUT
    **base 156 REACHED in 31/73 levels** = `SPRITE_STATS[84]` = the `(2,0)`
    tree ODD variant (`RandomBit(83→base 140, 84→base 156)`). Sprite 156 is
    one of the two corrupt arctic TMAPS entries → ~half of all arctic trees
    resolve to it. **This overturns §5's "benign, no fix" for 156** — it is
    heavily reachable. NEW open question: does retail HW render entry 156
    blank (our frame-less skip) or as a copy of neighbor 155's pixels (whose
    byte-perfect copy the payload actually is)? Trace before touching the
    frame-less skip. (153 stays benign — unreachable.)
- **Spell-20 child → sprite path** (§3c): one clean trace to settle the
  visuals mechanism (code literal vs arctic asset bank).
- **Napalm ↔ spell-20 relationship**: are the state-58 napalm change and
  the model-16 homing two facets of one spell, or independent? Trace the
  spell-20 emit chain end to end.

---

## 8. Renderer-effects inventory (SEPARATE track — NOT HW)

Captured while sweeping, for the eventual from-scratch renderer rebuild.
These exist in the **shared** MC1/HW renderer (HW changes none of this
code, only the data fed in) and are unimplemented in our port. **There is
no snowfall** — see Corrections.

| Effect | Original behavior | Our port |
|---|---|---|
| **Textured sky** | `DrawSky_30730` samples a 256×256 sky bitmap, scrolls/tilts with camera yaw/roll | flat `sky_srgb` fill — biggest visible gap |
| **Reflections** | `reflections_8597` mirror pass under water/ice | none |
| **Speed/motion blur** | `blur_8604` frame-feedback buffer, auto-enables above `actSpeed 80` | none |
| **Fog / depth shading** | `fog_B7934` 64-level palette ramp (0-31→white, 32-63→black, step ±8) + distance band | ✅ already faithful (`shade_lut` + wgsl fog) |
| Stereo dual-eye | 640-wide anaglyph split (`var_u8_8606`) | skip — period curiosity |

Snow, if ever wanted, is a **net-new invented enhancement**, not faithful
to the original (which never had it).

---

## Corrections (agent headlines that did NOT survive verification)

Recorded so next session doesn't re-derive the wrong thing:

1. **"HW has snowfall"** — FALSE. Originated from my own truncated
   scouting grep (`| head` cut off base MC1's live call sites), which made
   `DrawSkyTerrainParticles` look HW-exclusive. It is the shared
   sky/terrain/reflections **rasterizer** (calls `DrawSky` + reflections),
   called live and identically in **both** games (remc1 `:38749…` /
   remc1hw `:35200…`). `grep -i "snow|weather|flake|precipit"` = **zero**
   in both decompiles. The "Particles" name is a decompiler guess; the
   `sin/cos`+`fixPos` terms are the stereo dual-eye camera offset.
2. **"Spell 20 is just a 3-scalar rebalance, no new homing"** — FALSE
   (one agent's conclusion). Refuted by the direct switch diff in §3a: HW
   adds acquire cases `0x10/0x12/0x13`, and model 16 is the Fire Storm fire
   child → it homes in HW, flies straight in MC1. The agent only read the
   spell-definition table and never checked `sub_54520`. The rebalance
   (§3b) is *also* real — the truth is the union, matching the player's
   own account (napalm → homing meteor, new icon/projectile/effect/
   targeting).
