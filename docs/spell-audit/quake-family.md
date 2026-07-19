# MC2 Spell Audit — the GROUND / QUAKE family

Scope: **Tremor · Crater · Earthquake · Volcano**. Recorded gameplay is senior;
the vendored decompile (`reference/remc2/remc2/engine/`, `EF:` = `EventsFunctions.cpp`)
is the reference. Port = `crates/mgc-sim/`.

---

## TL;DR

The four spells all fire a **class-9 flight projectile** whose **impact** is a
`_4A190(pos, 10, model)` **class-10 terrain effect**. Every impact handler
already exists in the port **and is dispatched by the class-10 action loop**
(`crates/mgc-sim/src/mc1/world.rs:1450-1520`) — they run for level-authored
placements. The bug is entirely in the projectile→impact router
`Gen::mc2_proj_impact` (`crates/mgc-sim/src/mc2/proj.rs:344-430`): its
`match (fc, fm)` only routes **(10,11)** (crater). **(10,71) tremor, (10,15)
earthquake, (10,9) volcano fall through the `_ =>` misfit arm** → a bare
`area_write` damage tick, **no terrain effect**. This matches the player
report 3-for-4 exactly.

| Spell | idx (`spell_t`) | class-9 subtype | impact model | port handler | routed at impact? | player report |
|-------|-----|-----|-----|-----|-----|-----|
| **Tremor**     | 15 | 23 | **(10,71)** fissure     | `mc2_spawn_fissure` / `mc2_fissure_tick` | **NO** → misfit | "effect absent completely" ✓ |
| **Crater**     | 16 | 5  | **(10,11)** scorch ring | `mc2_spawn_scorch_ring` / `mc2_scorch_ring_tick` | **YES** | "all 3 levels SAME effect" ✓ (see below) |
| **Earthquake** | 17 | 2  | **(10,15)** fire trail  | `mc2_spawn_fire_trail` / `mc2_fire_trail_tick` | **NO** → misfit | "effect absent" ✓ |
| **Volcano**    | 18 | 4  | **(10,9)** raise-dome   | `mc2_spawn_dome` / `mc2_dome_tick` | **NO** → misfit | "effect absent" ✓ |

**The fix is three match arms** in `mc2_proj_impact`. The common post-spawn
tail already propagates the tier's `subSpellIndex` into the effect's `f140`
(so per-tier **damage** scales for free); no `f71` propagation is wanted (the
ctors seed the correct phase byte).

**Crater is NOT a port bug.** In retail the crater's *geometry* (radius cap 32
tiles, −3/tick dig, 40-tick life) is **tier-independent**; only the burn
**damage** scales (`subSpell` 250/400/900). The port reproduces this
faithfully. "Same effect on all 3 levels" is retail-accurate for the *visible*
carve; the tiers differ only in (invisible) damage + mana cost. Visible
per-tier scaling would be an **enhancement**, not a fidelity fix.

### Name → index → arm resolution (verified)

- `spell_t` enum: `tremor=15, crater=16, earthquake=17, volcano=18`
  (`reference/remc2/remc2/engine/global_types.h:150-155`). The entity-subtype
  table in `Spells.h:36-39` (`06 Earthquake / 08 Volcano / 09 Crater`) is a
  DIFFERENT numbering (the class-9 *shot* subtype) and is **not** the cast
  index — do not confuse them.
- Port dispatch `Gen::mc2_dispatch_arm` (`crates/mgc-sim/src/mc2/cast.rs:664-690`)
  keys on the `spell_t` index and matches `sub_6DCA0` (EF:44020-44236) verbatim:
  `15→(sub 23, impact 10/71)`, `16→(sub 5, 10/11)`, `17→(sub 2, 10/15)`,
  `18→(sub 4, 10/9)`. All four are `charge:true`.
- The class-9 CREATORS (`cast.rs:150-165`) name subtype 2 "earthquake shot",
  4 "volcano shot", 5 "crater shot", 23 "tremor" — consistent.
- The impact bytes are written by `sub_6DCA0` as `byte_0x43_67 = 10`
  (impact class) + `byte_0x44_68 = {71|11|15|9}` (impact model), plus
  `subSpellIndex_0x2A_42 = tier.subSpellIndex_2` and
  `byte_0x46_70 = tier.life_0x1A` (LABEL_59). On impact `sub_65820`
  (EF:62882) spawns `_4A190(pos, byte_0x43_67, byte_0x44_68)` and
  **propagates `subSpellIndex` and `byte_0x46_70` onto the effect**
  (EF:62994-62996: `v11x->subSpellIndex = a1x->subSpellIndex;
  v11x->byte_0x46_70 = a1x->byte_0x46_70;`).

### SPELLS.DAT tiers (baked `baked/assets/mc2-day/spells.bin`)

| Spell | tier0 subSpell / mana / life | tier1 | tier2 |
|-------|-----|-----|-----|
| Tremor (15)     | 200 / 4000 / 60  | 300 / 6000 / 80   | 800 / 10000 / 120 |
| Crater (16)     | 250 / 6000 / 6   | 400 / 9000 / 12   | 900 / 12000 / 24 |
| Earthquake (17) | 300 / 10000 / 16 | 500 / 12000 / 32  | 1000 / 15000 / 64 |
| Volcano (18)    | 400 / 12000 / 7  | 800 / 15000 / 9   | 1200 / 18000 / 11 |

(`subSpell` = `subSpellIndex_2` = the propagated damage payload; `life` =
`life_0x1A` = the projectile's `byte_0x46_70`.)

---

## Tremor  (spell 15 → subtype 23 → impact (10,71) FISSURE)

**1. Identify.** `spell_t::tremor = 15` (global_types.h:151). Arm
`cast.rs:681` = `(subtype 23, impact (10,71), charge)`; `sub_6DCA0` a3=15
branch EF:44120-44131. Tier fields: `subSpellIndex_2` = 200/300/800,
`life_0x1A` = 60/80/120.

**2. Retail effect + per-tier law.** Impact model **(10,71)** = the
expanding-fissure disc. Ctor `sub_51790` (EF:37439): `maxLife=life=120`,
`subSpellIndex=20000`, extents `(1280,2048)`, action `0x4E`, no sprite. Tick
`sub_3A2D0` (EF:29443): phase-0 init sets `word_0x2C_44 = maxLife>>3`,
`byte_0x46_70=1`, and **per-beat magnitude `subSpellIndex = 4*(subSpell/maxLife)`**;
each tick a disc of radius that ramps up→pins at `3*word_0x2C_44`→ramps down
(clamped `[0,15]`, 1-in-5 phase-jump roll) takes a **±1 heightmap jitter**
(sign = `life & 1` — the ground vibrates; no terrain-TYPE write, no children).
`byte_0x46_70 > 1` adds a half-radius inner pass; `byte_0x46_70 > 3` = terminal
tail-off (life-only). Scaling handle = **`subSpellIndex` (→ jitter magnitude)**;
the ±1 amplitude and radius envelope are otherwise fixed. Which handler ticks
it: `mc2_fissure_tick` via `world.rs:1513` (action `0x4E`). XP: id-0xF
spellbook report row (banked with 4.2).

**3. Current port.** `mc2_spawn_fissure` (`tail.rs:434`) + `mc2_fissure_tick`
(`tail.rs:709`) exist and are dispatched. **But (10,71) is absent from
`mc2_proj_impact`'s match** (`proj.rs:352-411`) → falls to `_ =>`
(`proj.rs:411-416`): `note_misfit(10,71)` + one `area_write` damage pulse,
**no fissure spawned**. Effect entirely absent, as reported.

**4. Gap.** One missing match arm. The handler + dispatch are already correct.

**5. Fix data.** Add to `mc2_proj_impact`:
`(10, 71) => self.mc2_spawn_fissure(x, y, z),`. The common tail (`proj.rs:421-427`)
sets `f140 = dmg` (= projectile `f44` = tier `subSpell` 200/300/800), so
per-tier magnitude scales for free. **Do NOT propagate `f71`** — the ctor seeds
`f71=0` so the phase-0 init runs; propagating the raw tier `life_0x1A`
(60/80/120) would make `byte_0x46_70 > 3` and drive the fissure straight to its
terminal no-op (see Open Questions). Spawn column: `mc2::tail`.

**6. Confidence / open / test.** Confidence **high** for "impact not routed".
Open: retail propagates `byte_0x46_70` = the tier `life` (60/80/120), which by
`sub_3A2D0`'s own `>3 → terminal` guard would make the fissure inert — so
retail must reset/consume `byte_0x46_70` in the class-9 tremor-shot action
wrapper before impact (cf. the cave-in wrapper `sub_67910` that the port models
at `proj.rs:398-402`). This action-wrapper trace for subtype 23 is UNVERIFIED;
leaving `f71=0` is the safe, effect-producing choice. Test: cast Tremor at flat
ground, watch for a vibrating disc of heightmap jitter (terrain "shivers") for
~120 ticks; confirm `note_misfit(10,71)` disappears; compare tier0 vs tier2
per-beat magnitude (`4*200/120≈6` vs `4*800/120≈26`).

---

## Crater  (spell 16 → subtype 5 → impact (10,11) SCORCH RING)  — FAITHFUL

**1. Identify.** `spell_t::crater = 16` (global_types.h:152). Arm `cast.rs:682`
= `(subtype 5, impact (10,11), charge)`; `sub_6DCA0` a3=0x10 branch
EF:44136-44146. Tier `subSpell` = 250/400/900, `life` = 6/12/24.

**2. Retail effect + per-tier law.** Impact model **(10,11)** = the scorch
ring / crater. Ctor `NewAdd0A0B_4E840` (EF:35553): `maxLife=40`,
`subSpellIndex=200`, `word_0x26_38=11` (XP row key), extents `(2304,0x2000)`,
action 11, invisible. Tick `sub_31FB0` (EF:23490): radius counter
(`dword_0x10_16`) grows every 3rd frame, capped at `pitch>>8 − 1 = 0x2000>>8 − 1
= 31`; each tick digs the disc `[0,r]` by **−3** (`sub_31F00`) and, on reaching
cap, stamps the outer ring once; area-burn each tick = full `subSpellIndex`
first tick then `/25`; XP `sub_6D8B0(id, 0x10, hits)` (row 16). **Geometry
(radius cap 31, −3/tick, 40-tick life) is a hard constant — it does NOT read
`byte_0x46_70` or `subSpellIndex`.** The ONLY per-tier handle is
`subSpellIndex` → burn **damage** (250/400/900, propagated over the ctor's 200
by `sub_65820`). Handler: `mc2_scorch_ring_tick` via `world.rs:1477` (action 11).

**3. Current port.** WIRED: `(10, 11) => self.mc2_spawn_scorch_ring(x, y, z)`
(`proj.rs:356`). Common tail sets `f140 = dmg` = tier `subSpell` → the burn
scales 250/400/900. `mc2_scorch_ring_tick` (`tail.rs:227`) uses `f80>>8` for the
constant radius cap and `dig_disc_minus3` for the constant −3 carve — matching
retail exactly.

**4. Gap.** **None on fidelity.** The visible carve is tier-independent in
retail, so "all 3 levels look the same" is CORRECT. The tiers differ only in
burn damage (which is not visible as terrain) and mana cost. Verify the damage
actually reaches the ring (it should: `f140 = dmg` from the tier).

**5. Fix data.** No fidelity change required. IF the player wants visible
per-tier scaling as an opt-in *enhancement* (not faithful), the natural lever is
the extents pitch: scale the `mc2_shift_rot(i, 2304, 0x2000)` pitch by tier so
`f80>>8` (radius cap) grows — but flag this clearly as a divergence from retail.

**6. Confidence / open / test.** Confidence **high** that the port matches
retail. Open: none material. Test: cast Crater I/II/III on a building and read
the damage dealt — expect ≈ 250 / 400 / 900 scaling (first-tick full, then /25);
the hole itself should be identical across tiers, confirming faithful behavior.

---

## Earthquake  (spell 17 → subtype 2 → impact (10,15) FIRE TRAIL)

> **LANDED 2026-07-13 + re-trace-confirmed (agent, HIGH conf).** The
> `(10,15)` impact arm was added earlier, but `mc2_fire_trail_tick` was
> laying the WRONG child: `mc2_spawn_fire_spray` **(10,19)** (a
> 240-life fire effect that spews `(10,14)` smoke every odd tick)
> instead of the `(10,11)` SCORCH RING. A trail dropping one spray per
> tick over its 128-life flooded the entity pool (~+823 entities/cast,
> player-reported exhaustion) and rendered as explosions. Fixed to
> `mc2_spawn_scorch_ring` (10-tick child → ~11 concurrent, +1 net
> measured) — the travelling earth-carve. The `(10,11)`-every-tick
> cadence is CORRECT (re-verified `sub_32530` EF:23716; the child life
> override to 10 at EF:23722 is what keeps the population trivial).
> Same `(10,11)`-vs-`(10,19)` numbering trap the cave column hit.

**1. Identify.** `spell_t::earthquake = 17` (global_types.h:153). Arm
`cast.rs:683` = `(subtype 2, impact (10,15), charge)`; `sub_6DCA0` a3=0x11
branch EF:44150-44160. Tier `subSpell` = 300/500/1000, `life` = 16/32/64.

**2. Retail effect + per-tier law.** Impact model **(10,15)** = a **wandering
fire trail** that lays scorch-ring craters as it walks. Ctor `sub_4ECD0`
(EF:35707): `maxLife=128`, `actSpeed=256`, `subSpellIndex=100`, random start
yaw, extents `(1024,0x4000)`, action 15. Tick `sub_32530` (EF:23694): each tick
random-walks the yaw (`±45..±90` about current, `%0x5B`), advances 256 units,
and spawns a child **(10,11) scorch ring** at the new spot with `life=10`,
`word_0x26_38=15` (so those craters award **Earthquake** XP via the ring tick's
`==15 → sub_6D8B0(id, 0x11)` branch), inheriting the trail's extents
(pitch/roll/fov = carve size). Despawns at `life < −1` or 8 consecutive
water/blocked cells. **The trail life (128) and step (256) are fixed; the child
rings carry the ctor-default `subSpell=200`.** Per-tier scaling in retail is
weak here — the trail geometry does not read `byte_0x46_70`; the propagated
`subSpellIndex` (300/500/1000) lands on the trail's own `f140`, which
`sub_32530` does not consume (the children re-default to 200). Handler:
`mc2_fire_trail_tick` via `world.rs:1469` (action 15).

**3. Current port.** `mc2_spawn_fire_trail` (`tail.rs:137`) + `mc2_fire_trail_tick`
(`tail.rs:1150`) exist and are dispatched. **(10,15) is absent from
`mc2_proj_impact`'s match** → `_ =>` misfit (`note_misfit(10,15)` + bare
`area_write`), **no trail**. Effect absent, as reported.

**4. Gap.** One missing match arm. Handler + dispatch already correct. (The
per-tier weakness is a retail property, not a port defect — see Open Questions.)

**5. Fix data.** Add `(10, 15) => self.mc2_spawn_fire_trail(x, y, z),`. Common
tail propagates `f140 = dmg` (300/500/1000), matching retail's `sub_65820`. No
`f71` propagation (the trail does not phase on it). Spawn column: `mc2::tail`.
Confirm the child scorch rings spawned by `mc2_fire_trail_tick` route through
the existing (10,11) path so they carve + credit XP.

**6. Confidence / open / test.** Confidence **high** for "impact not routed".
The wrapper question is RESOLVED (2026-07-16, §Trace-bank corrections 6 +
Session F2): the class-9 action table maps the quake shot to `sub_66160`, which
applies the charge at **1×** (trail life = `byte_0x46_70` = 16/32/64) — only the
whirlwind uses the 8× `sub_678E0`. The port's old hard-coded 128 was the
tier-0 value pre-multiplied by 8, making every tier's reach 8× LONGER than
retail; the faithful 1× landed in Session F (playtest owed).
Test: cast Earthquake, watch a fire trail snake across the ground dropping a
chain of scorch craters; confirm `note_misfit(10,15)` gone and Earthquake XP
accrues from the child rings.

---

## Volcano  (spell 18 → subtype 4 → impact (10,9) RAISE-DOME)

**1. Identify.** `spell_t::volcano = 18` (global_types.h:154). Arm `cast.rs:684`
= `(subtype 4, impact (10,9), charge)`; `sub_6DCA0` a3=18 branch EF:44162-44172.
Tier `subSpell` = 400/800/1200, `life` = 7/9/11.

**2. Retail effect + per-tier law.** Impact model **(10,9)** = the three-phase
**raise-land dome** (the same morph the endgame apocalypse re-uses). Ctor
`NewAdd0A09_4E760` (EF:35513): `maxLife=11`, `life=17`, `subSpellIndex=2000`,
extents seed `ShiftRot(7,0x4000)` (overwritten by init), action 9. Tick
(EF:23245+, port `mc2_dome_tick` `morph.rs:189`): **init** fixes radius
`R = maxLife|1`, base z = perimeter-min under the footprint, height `= 2R+100`
(clamped ≤255); **grow** eases each footprint cell toward a raised-cosine height
profile `(1+cos)/2 · height` over the box, `1/life` per tick, raise-only (cave
levels also lift the ceiling to keep clearance); **finalize** flattens to
`summit−24`, stamps a 2×2 cap, despawns. Combat: `area_write(f140)` each
non-apocalypse tick (`subSpellIndex` burn) + row-18 XP. **Dome radius/height
key off `maxLife` (fixed 11 in the ctor); `sub_65820` propagates
`subSpellIndex`+`byte_0x46_70` but NOT `maxLife`, so retail's per-tier dome
geometry is essentially constant — only the burn `subSpell` (400/800/1200)
scales.** Handler: `mc2_dome_tick` via `world.rs:1507` (action 9,
`apocalypse=false` for the spell).

**3. Current port.** `mc2_spawn_dome` (`morph.rs:90`) + `mc2_dome_tick`
(`morph.rs:189`) exist and are dispatched. **(10,9) is absent from
`mc2_proj_impact`'s match** → `_ =>` misfit (`note_misfit(10,9)` + bare
`area_write`), **no dome raised**. Effect absent, as reported.

**4. Gap.** One missing match arm. Handler + dispatch already correct.

**5. Fix data.** Add `(10, 9) => self.mc2_spawn_dome(x, y, z),`. Common tail
sets `f140 = dmg` (400/800/1200) → dome burn scales; the dome ctor leaves
`f71=0` so the phase-0 init runs (the common tail must NOT set `f71`). Spawn
column: `mc2::morph`. Note the dome is spawned at the projectile impact point on
the ground (`x,y,z`), raising a volcano-mound there; `apocalypse` stays the
World default (false), so it's the plain raise-land, not the endgame variant.

**6. Confidence / open / test.** Confidence **high** for "impact not routed".
Open: retail dome radius is fixed at `maxLife=11` regardless of tier — if the
player expects a bigger volcano per tier, check whether the subtype-4 action
wrapper overrides `maxLife` from `byte_0x46_70`/tier before impact (UNVERIFIED —
the subtype-4 class-9 wrapper trace is the missing piece; mirror the cave-in
`maxLife = charge` fixup at `proj.rs:398-402`). Test: cast Volcano on open
ground, watch a cosine dome of terrain rise (~2R+100 tall) then flatten to a
capped plateau; confirm `note_misfit(10,9)` gone and row-18 XP accrues. Regen
the MC2 state-hash goldens after wiring (terrain writes change hashes).

---

## Consolidated fix (all four)

In `Gen::mc2_proj_impact` (`crates/mgc-sim/src/mc2/proj.rs`, the `match (fc, fm)`),
add three arms alongside the existing `(10, 11)`:

```rust
(10, 71) => self.mc2_spawn_fissure(x, y, z),      // Tremor
(10, 15) => self.mc2_spawn_fire_trail(x, y, z),   // Earthquake
(10, 9)  => self.mc2_spawn_dome(x, y, z),         // Volcano
```

The existing common tail (`proj.rs:421-427`) already stamps `id24/f30/f146` and
`f140 = dmg` (= the tier `subSpellIndex` → per-tier burn) onto every spawned
effect. **No `f71` propagation is wanted** — each ctor seeds the phase byte the
tick expects (fissure/dome init on `f71==0`); propagating the raw tier
`life_0x1A` (60/80/120 / 7/9/11) would corrupt the phase machine. Crater
`(10,11)` is already correct and needs no change. After the change, re-run the
MC2 census/sweep instruments and re-pin the MC2 state-hash goldens (MC1 goldens
untouched — all changes are MC2-gated).

**Residual open item spanning all four — RESOLVED 2026-07-16** (§Trace-bank
corrections 6 + Session F2): the quake-family class-9 action wrappers were
traced — the quake shot routes through `sub_66160` (charge applied at **1×**);
only the whirlwind carries the 8× `sub_678E0`. The per-tier geometry
(16/32/64 charge → trail life, ~2×/level) landed with the Session-F tier-lives
batch; the family's per-tier reach/duration playtest is still owed.
