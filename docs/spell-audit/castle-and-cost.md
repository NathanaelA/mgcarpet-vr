# Castle spell audit — defensive towers (A) + mana-cost ladder (B)

Decompile-vs-port gap report for the two recorded-retail castle-spell gaps. Senior source =
recorded original gameplay; reference = the vendored decompile (remc2 `engine/*.cpp` +
remc1 `sub_main.cpp`). All cites are `file:line`. Port = `crates/mgc-sim`.

---

## TL;DR

- **(A) Defensive towers.** Retail castles grow **class-10 model-79 wall pieces** (the `(10,79)`
  "stage pieces") that are simultaneously the visible towers AND active turrets: they scan for
  enemies and fire spells. **Tower TYPE is set by the spell LEVEL used**, not independent — the
  castle spell's per-tier `life_0x1A` (`{0,1,2}` for tiers `{0,1,2}`) becomes the piece's
  `byte_0x43_67` part-type, and `sub_3AF00_castle_defend_event` (EF:30106) branches on it:
  **part-type 1 → FIRE tower (fires fireballs, spell 0); part-type 2 → LIGHTNING tower (fires
  lightning, spell 7)**; both rarely fire a meteor (spell 9). **The port has the piece ctor and
  the dwell states but the target-scan + fire states (3..8) are STUBBED, and the part-type source
  is hardcoded 0** (`mc2_castle_part_type` returns 0), so **zero `(10,79)` pieces ever spawn and
  none ever fire — the castle has no wall-tower defence at all.** (The separate `(5,15)` ground
  guards ARE ported and functional, but they are foot-soldiers, not the fire/lightning towers.)
- **(B) Mana cost.** Retail **MC2** (`GetSpellManaCost_6D710`, L:1714-1785) scales the castle
  spell by the OWN castle's level `[1000,10000,20000,40000,80000,160000,320000,300000000]` **and**
  multiplies by the spell tier (`×1.25` at tier 1, `×1.5` at tier 2). The port's `LADDER`
  (cast.rs:218) gets the level rungs right for 0..6 but **(1) has the wrong top rung — `0x3E8`
  (=1000) instead of `300_000_000`, so a max-level castle is recastable for 1000 mana, and
  (2) ignores the tier multiplier entirely** — a fire/lightning castle is charged the same as a
  plain one. Retail **MC1** (`sub_3C060`, remc1 `sub_main.cpp:48028`) is a **flat 1000** compile-
  time constant with **no** level scaling and **no** castle requirement; the port matches this
  faithfully. Both gaps make the MC2 castle "castable when it shouldn't be".

---

# ISSUE A — Castle defensive towers

## A.1 Identify (entities / models / fields)

| thing | value | cite |
|---|---|---|
| the standing castle | class 3, model 2 | EF:33378; runtime trace §Headline-1 |
| **the wall-tower / turret** | **class 10, model 79** `(10,79)` | ctor EF:36994-95 |
| tower ctor | `sub_508E0_castle_defend_create` (0x2318e0) — action `0x56`, maxLife 100000, sprite 66, `byte_0x46_70=0`, `byte_0x43_67=0` | EF:36987-37008 |
| tower per-tick AI | `sub_3AF00_castle_defend_event` (0x21bf00), registered | EF:30106; Events.cpp:2787 |
| tower spawner (per stage) | `sub_613D0` — one piece per `x_BYTE_DB038` offset, per researched stage | EF:62234 |
| **tower TYPE key** | `byte_0x43_67` (part-type) `= array_0x24E_590[9+level]` | set EF:62312; read EF:30232/236 |
| tower level tag | `word_0x4A_74 = stage` (drives fire arc + z height) | EF:62313 |
| part-type SOURCE | `SPELLS[2].subspell[tier].life_0x1A`, written per stage by castle research `sub_69AB0` | data-tables trace §2.2, EF:56121 |
| ground guards (separate) | class 5, model 15 `(5,15)`, per-stage count from `sub_60400` | EF:61488, :61523 |
| target scan cell | `mapEntityIndex_15B4E0` around the piece's tile | EF:30348 |
| fire spawn | `sub_6DCA0(castle, pos, v40, &SPELLS[v40].subspell[v42], …)` — the same dispatch player casts use | EF:30284 |

**SPELLS.DAT row 2 (castle spell), dumped from `baked/assets/mc2-day/spells.bin`:**

| tier | subSpell | manaCost | life_0x1A | ⇒ tower type |
|---|---|---|---|---|
| 0 | 0 | **1000** | **0** | none (plain castle) |
| 1 | 0 | **1250** | **1** | **FIRE tower** |
| 2 | 1 | **5000** | **2** | **LIGHTNING tower** |

## A.2 RETAIL behavior (cites)

**Spawn (`sub_613D0`, EF:62234).** For a castle at level `L`, walk `v4` DOWN from `L` to the
highest researched stage (`array_0x24E_590[9+v4] != 0`, EF:62274-77). Spawn one `(10,79)` piece
per `x_BYTE_DB038` tile offset for that stage (offsets decoded in data-tables trace §1.3 —
L1: 1 centred, L2/3: 4 corners, L4/5: 4 corners of 35², L6/7: 8 pieces of 48²). Each piece gets
`byte_0x43_67 = array_0x24E_590[9+v4]` (the part-type, EF:62312), `word_0x4A_74 = v4`
(EF:62313), and z = `terrainAlt + 384` (L≤1) else `+224` (EF:62316).

**The defend state machine (`sub_3AF00_castle_defend_event`, EF:30106), keyed on `byte_0x46_70`:**
- **0/1/2 — rise/dwell.** Latch home pos; seed `dword_0x10_16 = rand%48 + 16` and count it down;
  at 0 → state 3 (EF:30176-30193).
- **3 — SCAN (every 64 ticks, `byte_0x3E_62 & 0x3F`, EF:30195).** Read the map cell at the piece's
  tile (EF:30348) and walk its entity chain for a HOSTILE: **class 3 with model ≤1 or ==3 and a
  DIFFERENT owner id** (enemy castle / townsfolk / creature, EF:30360-73), OR **class 5, model ≠22,
  different owner** (with a `StageVar2==14` parent-check special, EF:30376-84). On a hit set
  `byte_0x46_70 = 4`, `word_0x96_150 = target index` (EF:30388-91).
- **4/5 — windup.** `word_0x36_54 += 160` cadence per sub-step (EF:30214); `dword_0x10_16 = 4`
  countdown → state 6 (EF:30204-21).
- **6 — pick fire mode from `byte_0x43_67` + `rand%100`** (EF:30222-48):
  - `v13 = rand%100`; if `v13 == 0` → mode 4; elif `v13 ≤ 5` → mode `(byte_0x43_67==1)+2`; else
    mode `(byte_0x43_67 != 1)`.
  - **part-type 1 (FIRE):** mode 0 (~94%), mode 3 (~5%), mode 4 (~1%).
  - **part-type ≠1 (LIGHTNING):** mode 1 (~94%), mode 2 (~5%), mode 4 (~1%).
  - `fontTypeIndex_0x3D_61 = 6` if mode ≤1 else 1 = the **burst count** (6 shots for the common
    modes, 1 for the rare high-tier shot).
- **7/8 — FIRE.** Map mode → `(v40 spell, v42 tier)` (EF:30258-82): mode0→`(0,1)` fireball tier1,
  mode1→`(7,0)` lightning tier0, mode2→`(7,1)` lightning tier1, mode3→`(0,2)` fireball tier2,
  mode4→`(9,0)` meteor tier0. Spawn via `sub_6DCA0` (EF:30284); aim at the target with `sub_655C0`
  (EF:30292); local-player fireball swaps to muzzle sprite 42 (EF:30290-91). Recoil counter
  `byte_0x44_68` bumps 1..5 (EF:30301-12) driving a lateral kick in the LABEL_74 tail
  (offsets 0/115/230/334/368/384, EF:30405-27); `fontTypeIndex` counts down and at 0 → back to
  scan (state 1, EF:30313-23).
- **Tail (LABEL_74).** Ground-track z at the piece's level height every tick (EF:30438-72).

**So: the tower fires FIREBALLS if its part-type is 1, LIGHTNING if part-type is 2** — and the
part-type is `SPELLS[2].subspell[tier].life_0x1A`, i.e. **the castle spell LEVEL the player built
that stage with.** Spell level is NOT independent of the build tier — it selects the tower type
(and the cost, §B). The research handler `sub_69AB0` (EF:56086) writes
`array_0x24E_590[v4] = subSpellIndex_2` (HP factor) and `array_0x24E_590[v4+9] = life_0x1A`
(part-type) for stage `v4 = castleLevel+1` (data-tables trace §2.2). The `(5,15)` ground guards
(`sub_5FF50`, EF:61343→:61488) are a SEPARATE per-stage foot-soldier roster
(stage→count: 1/2→0, 3→4, 4→6, 5→14, 6→18, 7→34, EF:61523).

## A.3 CURRENT PORT (cites)

- `(10,79)` piece ctor — **PORTED**: `mc2_spawn_castle_piece` (castle.rs:1220): class 10, model 79,
  action 0x56, maxLife 100000, `f67 = part`, `f71 = 0`, sprite 66. ✔
- `sub_613D0` piece builder — **PORTED shell**: `mc2_castle_stages` (castle.rs:1164) does the
  walk-down + `x_BYTE_DB038` offset spawn. **But** the part-type source
  `mc2_castle_part_type(own, stage)` **hardcodes `0`** (castle.rs:1212-1214). The walk-down loop
  (castle.rs:1175-1181) therefore never finds a nonzero part, `stage` decrements to 0, and the
  function **returns before spawning any piece** (castle.rs:1182-1184). **No wall towers spawn.**
- `sub_3AF00` AI — **PARTIAL**: `mc2_castle_piece_tick` (castle.rs:1254) implements only dwell
  states 0/1/2 and the ground-clamp tail. States **3 (scan), 4/5 (windup), 6 (fire-mode), 7/8
  (fire) are all STUBBED** — the `_ => {}` arm at castle.rs:1275, commented "the launch machinery
  banks with 4.2 cast machinery". **No target acquisition, no firing.**
- `sub_69AB0` castle research (the part-type + HP-factor writer) — **UNPORTED** (the sole reason
  `mc2_castle_part_type` is 0).
- `(5,15)` ground guards — **PORTED and functional**: spawned by `mc2_castle_roster`
  (castle.rs:681-692, `mc2_spawn_m15` roster.rs:1027) with a real brain
  (`m15_scan`/`m15_brain`/`m15_engage`, roster.rs:1155-1212). These work — but they are the
  courtyard soldiers, not the fire/lightning wall towers the player is asking about.

## A.4 The GAP

1. **No wall-tower defenders exist in the port.** The `(10,79)` piece chain never spawns because
   `mc2_castle_part_type` is stubbed to 0 (research unported). Even if forced to spawn, the pieces
   would stand inert — the scan + fire states are stubbed.
2. **Tower type → spell level mapping is absent.** Retail: castle spell tier 1 = fire towers,
   tier 2 = lightning towers (via `life_0x1A` → part-type). The port has no path from the cast
   tier to a tower type.
3. **The tower's offensive AI (scan/target/fire fireball-or-lightning) is entirely missing** — the
   single biggest fidelity gap the player observes ("towers that actively defend").

## A.5 FIX DATA (exact tables / formulas)

**Part-type source** (replace the `mc2_castle_part_type` stub): per castle level `L`, the part-type
is `SPELLS[2].subspell[researchedTier].life_0x1A`. Values from `spells.bin`:
`life_0x1A = {tier0:0, tier1:1, tier2:2}`. Wiring requires porting the research write
(`sub_69AB0`): on stage-L research completion,
`array_0x24E_590[9+L] = SPELLS[model].subspell[byte_0x46_70].life_0x1A` and
`array_0x24E_590[L] = SPELLS[model].subspell[byte_0x46_70].subSpellIndex_2` (HP factor; the HP
ladder already consumes it — runtime trace §2 `sub_60810`). Until research is ported, a faithful
shortcut is to stamp `array_0x24E_590[9+L]` at build/upgrade time from the castle spell tier used
to cast the upgrade (the same tier that scaled the cost, §B).

**Tower fire-mode table** (`sub_3AF00` state 6→8), `p = byte_0x43_67`:

```
v13 = rand % 100
mode = (v13 == 0) ? 4 : (v13 <= 5) ? ((p==1)?3:2) : ((p!=1)?1:0)
burst = (mode <= 1) ? 6 : 1        // fontTypeIndex_0x3D_61
mode -> (spell, subspellTier):  0->(0,1) 1->(7,0) 2->(7,1) 3->(0,2) 4->(9,0)
```
`spell 0` = fireball, `spell 7` = lightning bolt, `spell 9` = meteor (MC2 spell ids). Fire via the
existing `sub_6DCA0` dispatch (`mc2_dispatch_arm`/`mc2_launch` already in cast.rs), owner = castle
id, aim at `word_0x96_150` (nearest hostile from the state-3 scan). Cadence: scan gated by
`tick & 0x3F` (every 64 ticks); windup `word_0x36_54 += 160`; recoil kick offsets
`{1:0, 2:115, 3:230, 4:334, 5:368, 6:384}`.

**Target predicate** (state 3): hostile = `(class==3 && (model<=1 || model==3) && owner != selfOwner)`
OR `(class==5 && model!=22 && owner != selfOwner)`.

**Piece offsets** already in the port as `MC2_STAGE_PARTS` / `x_BYTE_DB038` (castle.rs:1294) — verified.

## A.6 Confidence / open / test

- **Confidence: HIGH** on the entity identity, the fire-mode table, and the type↔tier mapping
  (verbatim EF:30106-30474 + the `spells.bin` dump). **MEDIUM** on the exact scan geometry (the
  `sub_10130`/`mapEntityIndex` cell walk was read structurally, not tile-by-tile) and the research
  timing that populates `array_0x24E_590` (the `sub_69AB0` trigger chain is OPEN — data-tables
  trace §6).
- **Open:** (a) does an authored/just-cast castle get its part-type immediately, or only after a
  research child completes? (data-tables §6 OPEN — affects whether newly built fire castles show
  towers at once). (b) The exact per-level research→tier binding that gives "L2 fire, L3 lightning".
- **Suggested test:** once ported, golden a level with an enemy in tower range: build a tier-1
  castle → assert `(10,79)` pieces spawn with `f67==1` and emit `spell 0` (fireball) projectiles at
  the enemy; tier-2 → `f67==2`, `spell 7` (lightning). State-hash the piece roster per stage against
  `x_BYTE_DB038` counts (1/4/4/8 by level).

---

# ISSUE B — Castle spell mana cost does not track castle level

## B.1 Identify

| thing | value | cite |
|---|---|---|
| MC2 cost fn | `GetSpellManaCost_6D710(event, spellIndex=2, subSpellIndex=tier)` | L:1714 |
| MC2 base cost | `SPELLS[2].subspell[tier].manaCost_6 = {1000,1250,5000}` | spells.bin dump |
| MC2 castle level field | `castle->dword_0x10_16` (0..7) | L:1729 |
| MC2 castle-less flag | `event->dword_0xA4_164x->byte_0x1BE_446` → +3000 | L:1723, :1778 |
| MC1 castle spell ctor | `sub_3C060` → `sub_3BF70(a1, 16, 48, 1000, 101, 1, 0, 0, 10000)` | remc1 sub_main.cpp:48028 |
| MC1 total cost (+136/+140) | a4 = **1000** (flat) | sub_main.cpp:48028 |
| MC1 castle requirement (+132) | a8 = **0** (none) | sub_main.cpp:48028 |
| port MC2 cost | `mc2_spell_mana_cost` + `LADDER` | cast.rs:212-225 |
| port MC1 cost | `SPELLS[16]` `possess_mana=1000, castle_req=0` | mc1/spells.rs:140 |

## B.2 RETAIL behavior (cites)

**MC2 — `GetSpellManaCost_6D710` (L:1714-1785), verbatim logic for `spellIndex == 2`:**

```
result = SPELLS[2].subspell[tier].manaCost_6           // {1000,1250,5000}
if (no own castle) {                                    // CastleEntityIndex resolves <= Entities[0]
    if (byte_0x1BE_446) result += 3000
    return result                                       // = the tier base (+3000)
}
switch (castle.dword_0x10_16) {                          // OWN castle level
    0:1000  1:10000  2:20000  3:40000  4:80000  5:160000  6:320000  default:300000000
} -> result
if (level >= 7) { if(byte_0x1BE_446) result+=3000; return result }   // NO tier multiply at cap
switch (tier) {                                          // the spell LEVEL multiplier
    1: result = ((320*result) - (sign<<8) + sign) >> 8   // ≈ ×1.25 (round toward zero)
    2: result = ((384*result) - (sign<<8) + sign) >> 8   // ≈ ×1.5
}
if (level != 0) { if(byte_0x1BE_446) result+=3000; return result }
// level == 0:
if (byte_0x1BE_446) result += 3000
return result
```

So the **cost to build the NEXT level = `LADDER[currentLevel] × tierMult[tier]` (+3000 surcharge
when `byte_0x1BE_446` is set)**, where the ladder is
`[1000,10000,20000,40000,80000,160000,320000, 300000000]` and `tierMult = {0:1.0, 1:1.25, 2:1.5}`
(applied for levels 0..6 only; level 7 is the 300-million wall). The tier multiplier is exactly the
player's "fire/lightning castles cost far more" — a lightning castle (tier 2) at level 6 costs
`320000 × 1.5 ≈ 479999` mana. `byte_0x1BE_446` is set when a castle is sold down to stage 1
(runtime trace §5, EF:37996) — a castle-less/rebuild surcharge.

**MC1 — `sub_3C060` (remc1 sub_main.cpp:48028), verbatim:**
`sub_3BF70(a1, 16, 48, 1000, 101, 1, 0, 0, 10000)` → the castle spell's total mana cost (a4) is a
**flat 1000**, castle requirement (a8) is **0**. This is a compile-time constant; MC1 has **no
`GetSpellManaCost`-style per-level scaler** for the castle spell — the cost is fixed regardless of
the castle's level. (The MC1 castle capacity ladder `sub_47DD0` is
`[5000,10000,20000,40000,80000,160000,320000,30M]` — features.rs:2786 — but it does NOT feed the
spell cost.)

## B.3 CURRENT PORT (cites)

**MC2 — `mc2_spell_mana_cost` (cast.rs:212-225):**
```rust
const LADDER: [i32; 8] = [1000, 10000, 20000, 40000, 80000, 160000, 320000, 0x3E8];
let lvl = self.player_castle().map_or(0, |c| self.g.ent[c].f26.clamp(0, 7) as usize);
return LADDER[lvl];      // for spell == 2; the `tier` argument is IGNORED
```
`f26` is the castle's `dword_0x10_16` level (used the same way in `mc2_castle_stages`, castle.rs:1166).
The gate consumes this cost as `ent[m].max_life` in `mc2_cast_gate` (cast.rs:562-566): refuse if
`player.mana < cost`.

**MC1 — `SPELLS[16]` (mc1/spells.rs:140):** `possess_mana = 1000, castle_req = 0` → flat 1000,
debited via the +136/+140 channel. **Faithful to remc1's constant.**

## B.4 The GAP (MC2)

1. **Wrong top rung — real bug.** `LADDER[7] = 0x3E8 = 1000`, but retail's `default` case is
   **300_000_000**. At castle level 7 the port lets you recast the castle spell for **1000 mana**
   instead of an effectively-uncastable 300M. (The cast.rs:210 comment "last rung 0x3E8" is a
   misread of the decompile — `0x3E8` is 1000, not the 300M sentinel.)
2. **Tier multiplier missing entirely.** `mc2_spell_mana_cost` ignores its `tier` argument for
   spell 2, so a fire castle (tier 1, should be ×1.25) and a lightning castle (tier 2, should be
   ×1.5) are charged the **same** as a plain castle. This is the core of "castable when it
   shouldn't be" — the expensive castle variants are underpriced by 25–50%.
3. **Castle-less base wrong.** With no own castle, `player_castle()` is `None` → `lvl = 0` →
   returns `LADDER[0] = 1000`. Retail returns the tier base `manaCost_6 = {1000,1250,5000}`
   (+3000). Only tier 0 matches; tier 2 castle-less should be 5000, port charges 1000.
4. **`byte_0x1BE_446` +3000 surcharge** is unmodeled (cast.rs:210 flags it OPEN).

**MC1:** the port's flat 1000 **matches the remc1 decompile constant** — no gap against the
reference. The player's recorded impression ("MC1 mostly static, ~= the mana the previous castle
holds") is consistent with "static"; the exact "≈ previous castle capacity" figure is NOT in the
decompile (which is a bare 1000). If recorded gameplay shows a higher/scaling MC1 cost, that
OUTRANKS the truncated decompile — flag as an open needing the recorded numbers (B.6).

## B.5 FIX DATA (exact table / formula)

Replace the spell-2 branch of `mc2_spell_mana_cost`:

```
// tier = the SELECTED castle spell tier (mc2_book.sel[2]); pass it in (currently ignored)
base = SPELLS[2].tiers[tier].mana_cost            // {1000,1250,5000}
match player_castle() {
    None => base + (if byte_0x1BE_446 { 3000 } else { 0 }),
    Some(c) => {
        let level = ent[c].f26.clamp(0, 7);
        const LADDER: [i64;8] = [1000,10000,20000,40000,80000,160000,320000, 300_000_000]; // FIX rung 7
        let mut r = LADDER[level as usize];
        if level >= 7 { return r + surcharge; }       // no tier multiply at the cap
        r = match tier {                               // verbatim sign-idiom (round toward zero)
            1 => ((320*r) - (sign(320*r)<<8) + sign(320*r)) >> 8,   // ≈ ×1.25
            2 => ((384*r) - (sign(384*r)<<8) + sign(384*r)) >> 8,   // ≈ ×1.5
            _ => r,
        };
        r + surcharge                                  // surcharge only when byte_0x1BE_446
    }
}
```
where `sign(x)=1` for `x>0`. Worked values (tier multiply, toward-zero rounding — note it lands ~1
below the exact product): `level1 tier1 = (320·10000−255)>>8 = 12499`; `level6 tier2 =
(384·320000−255)>>8 = 479999`; `level0 tier2 = (384·1000−255)>>8 = 1499`. Two call sites also pass
`tier`: `mc2_set_spell` (cast.rs:242, already passes `t` — just honor it) and `mc2_book_view`
(cast.rs:375, passes `sel[s]` — good). The `byte_0x1BE_446` surcharge needs a new castle-flag
(set on sell-to-stage-1) — can be deferred; the tier multiply + top rung are the load-bearing fixes.

**MC1:** no change required for parity with the decompile (flat 1000). Only revisit if recorded
gameplay contradicts the constant.

## B.6 Confidence / open / test

- **Confidence: HIGH** for MC2 (verbatim L:1714-1785 + `spells.bin` dump confirming the
  `{1000,1250,5000}` bases and `{0,1,2}` life). **HIGH** for the MC1 flat-1000 constant (verbatim
  `sub_3C060`).
- **Open:** (a) `byte_0x1BE_446` exact set/clear lifecycle (only its +3000 effect + one set site
  at EF:37996 are traced). (b) The MC1 "≈ previous-castle-capacity" recorded impression vs the
  bare-1000 decompile constant — needs a recorded-gameplay mana reading to decide if MC1 should
  scale at all. (c) The sign-idiom's off-by-one rounding — reproduce the C expression exactly rather
  than a float `×1.25`.
- **Suggested test:** a table unit test of `mc2_spell_mana_cost` over `level ∈ 0..=7 × tier ∈ 0..=2`
  asserting the values above (esp. `level7 == 300_000_000` and `level6 tier2 == 479999`); plus a
  gate golden — with `player.mana` just under `LADDER[6]×1.5`, assert the castle cast is refused
  (sound 29) and the pane `cost[2]` reflects the tier multiply.

---

# RESOLUTION 2026-07-16 — the turret column LANDED

Sessions' worth of §A is now in the tree (mgc-sim/src/mc2/castle.rs):

- **Part-type source**: `Mc2CastleResearch` (features.rs, hash-quiet while empty) = the per-wizard
  `array_0x24E_590` slots; stamped at castle CAST/upgrade time from the castle-spell tier (the
  §A.5 shortcut — human: `cast_castle` from `mc2_book.sel[2]`; rivals: `mc2_rival_cast_castle`
  both branches from the book manifestation's f71). The real `sub_69AB0` research/production
  chain stays the banked follow-up (its trigger chain is still OPEN, data-tables §6).
- **Builder**: `mc2_castle_part_type` reads the stamp; the `sub_613D0` walk-down + `x_BYTE_DB038`
  ring spawn now actually fires. Verified: unstamped castle = zero towers (fresh retail state);
  level-N castle with only stage-1 research shows the stage-1 ring (the walk-down law).
- **Firing brain**: `mc2_castle_piece_tick` = the full `sub_3AF00` (EF:30106-30474): 64-tick ring
  scan 3..=12 (`AddE7EE0x_10080(3,12)` decoded — rings, min radius 3 = the castle's own
  footprint), windup (+160 z × 4), the mode table (94/5/1), 6-shot bursts, verbatim recoil arc
  0/115/230/334/368/384 out-and-back, idle bob, first-shot-only cast sound from `sub_6DCA0`'s
  tail (fireball 9 / lightning-t0 23 / t1 9 / meteor 15), NO XP back-ref (f40 = 0). Projectiles
  ride the shared `mc2_dispatch_arm`/`mc2_spawn_cast_proj`, owner = the castle's wizard, homing
  the scanned target. APPROX register: nearest-ring-first pick (retail = ring walk order); the
  class-5 `StageVar2==14` own-parent exemption skipped (side-vec stage binding); muzzle lift =
  f78 (retail `array_0x52_82.yaw`); turret scan sees through Invisibility (faithful — retail's
  predicate has no cloak test, unlike the m15 guards).
- **Issue B was already fixed** (the ladder carries 300M + the ×1.25/×1.5 tier multiplier in
  `mc2_spell_mana_cost`); only its doc comment was stale.
- Tests: `fire_turret_spawns_and_shoots_the_hostile`, `lightning_tower_and_the_walkdown_law`.
- Goldens hold (golden windows never build tier≥1 castles); the first playtest build with fire/
  lightning castles is the behavioral proof owed.
