# Spell Audit — Lightning Bolt (index 7)

> **LANDED 2026-07-27 (visual-aim fix, BOTH GAMES + MC1's `sub_535E0`).**
> Player report: the visible bolt always fired straight ahead while the
> damage homed; retail points at the locked target and JUMPS between
> targets as they die. Two fresh retail traces (verbatim-quoted):
> - **MC2 `sub_66750`**: retail re-aims ONCE at launch — `sub_66610`
>   (EF:63583-99) runs the one-shot `sub_67CB0` acquisition and FULLY
>   SNAPS yaw/pitch (yaw=roll, pitch=fov) — then walks a dead-STRAIGHT
>   ray (NO per-step homing anywhere in the beam), snapping position to
>   the victim (EF:63604-08). The trail heading is saved AFTER the snap
>   (EF:58306-08); trail length = walked steps ×8 nodes at actSpeed/8
>   (EF:58321-23), so trail end = walk terminus = victim = the (10,23)
>   blast site (EF:58403). The port had BOTH halves wrong: trail along
>   the pre-snap cast facing, damage via the per-tick-homing flyer.
>   `mc2_lightning_beam_tick` now snaps first, marches with the lock
>   held aside (straight), and lays the trail by step count.
> - **MC1 `sub_535E0`**: retail runs the FIRST `sub_534C0` flight call
>   (which performs the one-time aim-assist snap of +30/+32) and only
>   THEN saves the chain heading (:63312 → :63313-14, restored
>   :63327-28) — chain and endpoint explosion follow the AIMED heading.
>   The port captured the heading before the snap; `proj_m9_tick` now
>   hoists the snap above the capture. Retail fire sites pre-aim
>   +30/+32 AND pre-lock +146 (:23255-60) — our rival/mob thunks
>   already do; the player cast leaves f146=0 and the snap aims, same
>   net heading either way.
> - Target jumping needs NO new mechanism: each RAPID re-fire is a
>   fresh bolt re-scanning from scratch (word150==0 at spawn).
> - Ranges verified correct both games (3456 = 384·9; MC1 3584/384).
> - Regression tests (lib, non-vacuity proven by bug re-introduction):
>   `mc2_lightning_beam_trail_points_at_the_locked_target`,
>   `mc1_lightning_chain_points_at_the_locked_target` (world.rs).
> - ENHANCEMENT (player-requested): `(10,23)` (both games' lightning
>   hit-blast) joined the enhanced-fire set — a scaled-down (boom 0.65)
>   quick fire burst at the beam terminus, retail sprite suppressed
>   (world.rs fire_life gate + entities.rs impact set). Classic mode
>   untouched.

Sources: `EF:` = `reference/remc2/remc2/engine/EventsFunctions.cpp`; trace bank
`docs/traces/mc2-class9-spell-projectiles.md` (class-9 flight states, verbatim);
port `crates/mgc-sim/src/mc2/cast.rs`, `proj.rs`, `spells.rs`. Recorded gameplay
is senior; decompile is reference. SPELLS.DAT row-7 values dumped live from
`baked/assets/mc2-*/spells.bin`.

> **LANDED 2026-07-13 (session 3, full — beam VISUAL + storm now real).**
> First pass made L0 a one-tick beam (correct dynamics) but DROPPED the
> visual (the bolt despawned before rendering) and L1/L2's `(10,38)` was
> an inert blast23 stand-in ("nothing happens"). A dedicated re-trace
> (agent, HIGH conf) resolved both:
> - **L0 flash:** `mc2_lightning_beam_tick` now lays `sub_66750`'s
>   cosmetic trail — ~80 sprite-216 `(9,9)` billboards (action 14 =
>   `sub_67410`, 1-frame life) from muzzle to impact with the ±1
>   random-walk jag. That line IS the visible flash; under RAPID
>   re-fire it crackles. (`mc2_lay_lightning_trail` /
>   `mc2_spawn_lightning_node` / `mc2_lightning_node_tick`.)
> - **L1/L2 storm:** `(10,38)` is now the real `sub_4FFB0` STORM CLOUD
>   (model 38, action 40, life 32, sprite 272) — it hovers +1024 above
>   terrain then RAINS 2 downward `(9,9)` beams/tick (`mc2_storm_tick` =
>   `sub_35640`), each a full beam striking the ground as `(10,23)` and
>   carrying the tier subSpell; the first of each pair claps thunder.
>   `mc2_spawn_lightning_burst` rewritten from the blast23 stand-in.
> - **"Explodes right in front" is AUTHENTIC** — subtype-12 maxLife 5 is
>   genuinely short-range; long flight needs a model-78 autoaim-beacon
>   lock (`sub_68940`, +32 life), deferred until beacon markers exist.
> Tests `mc2_lightning_l0_is_a_one_tick_beam` (asserts the flash renders)
> + `mc2_lightning_storm_rains_beams` (cloud + `(10,23)` rain, pool-bounded).

## TL;DR

The player is right on both counts, and the two symptoms have ONE shared root:
**the port serves every MC2 lightning subtype through the generic traveling-
projectile tick (`mc2_flyer_tick` = `sub_65820`), but neither of lightning's two
subtypes is that state.**

- **L0 (tier 0)** dispatches to class-9 **subtype 9**, whose retail action state
  is **9 = `sub_66750`** — a *one-tick hitscan BEAM* (walk to the first blocker,
  lay a jagged trail, detonate the same tick). The port instead flies it as a
  slow homing bolt (life 9 ticks, speed 384, row-63 homing). Because tier 0 is
  also RAPID (re-fires every held tick — this part IS authentic), the slow bolts
  stack into "a stream of projectiles" instead of flash-present-or-not. This is
  the exact analogue of the MC1 lightning-beam shape (`proj_m9_tick` /
  `sub_535E0`, combat.rs:1438) that resolves in ONE tick.
- **L1 / L2 (tiers 1, 2)** dispatch to class-9 **subtype 12** (action **0x0C =
  `sub_66FD0`**, "Lightning II" — a charged flying bolt). Retail's `sub_66FD0`
  impact is HARD-CODED to spawn **(10,38)** (a class-10 lightning burst) and
  *chains* the projectile's `(byte_0x43,byte_0x44)=(9,9)` onto that burst. The
  port has no `sub_66FD0` handler: `mc2_flyer_tick`'s generic impact naively
  spawns the projectile's own `(f68,f69)=(9,9)`, which no class-9 impact arm
  serves → `WARN: misfit thing (class 9, model 9)`, degraded to a bare damage
  write. The storms therefore have no visible effect.

The dispatch table (`mc2_dispatch_arm`, cast.rs:678-679), the RAPID cadence flag,
the sounds, and the SPELLS.DAT row are all CORRECT. The gap is entirely in the
projectile *serving* layer (`mc2_proj_tick` dispatch in mobs.rs).

---

## 1. Identification

**Spell index 7 = "Lightning Bolt"** (hint strings 208/209/210). Class-15
manifestation `model65 = 7`. Cast machinery is the standard MC2 column
(cast.rs). SPELLS.DAT row 7, dumped from all three baked bundles (identical —
Day/Night/Cave do not patch row 7; only rows 4/19 are level-init patched):

| tier | subSpell (dmg) | mana | maxMana | xpos1 | word_0x18 | **life_0x1A** | font | f59 cadence |
|---|---|---|---|---|---|---|---|---|
| 0 | 200 | 1000 | 30000 | 0 | 5 | **0** | 1 | **RAPID** (font&1=1 → f59=0) |
| 1 | 300 | 10000 | 90000 | 1000 | 35 | **1** | 0 | CLICK (font&1=0 → f59=1) |
| 2 | 800 | 20000 | 120000 | 2000 | 45 | **2** | 0 | CLICK |

**The dispatch split** (`sub_6DCA0` a3==7, EF:44042-44078, verbatim):

```
if (a4->life_0x1A) {                       // life != 0  → tiers 1,2
    if (a4->life_0x1A > 2) goto NOSPAWN;    // (never hit: max life is 2)
    v = _4A190(pos, 9, 12);                 // class-9 subtype 12
    v->byte_0x43_67 = 9;  v->byte_0x44_68 = 9;   // impact (9,9)
    v->subSpellIndex = a4->subSpellIndex_2;  v6 = 9;   // sound 9
} else {                                   // life == 0 → tier 0
    v = _4A190(pos, 9, 9);                  // class-9 subtype 9
    v->byte_0x43_67 = 10; v->byte_0x44_68 = 23;  // impact (10,23) blast23
    v->subSpellIndex = a4->subSpellIndex_2;  v6 = 23;  // sound 23
}
```

So **life_0x1A selects the subtype**, and it lines up with tier because the CD
table happens to set life = tier for row 7 (0/1/2). The port encodes this as
`7 if matches!(life,1|2) => arm(12,(9,9))  else => arm(9,(10,23))`
(cast.rs:678-679) — a faithful transcription.

**The two class-9 subtypes and their retail action states**
(trace mc2-class9-spell-projectiles.md, "ACTION STATES"):

- **Subtype 9 / model 9 / action 9 = `sub_66750`** (EF:58268) — the THUNDER/
  LIGHTNING BEAM. *Not a flight loop.* One tick: `actSpeed=minSpeed`; ray-walk
  `sub_66610` to the first blocker counting steps; lay `v27*8` class-9 model-9
  trail nodes along a per-node ±1 RNG jag (`SetEntityIndexAndRot(216)`); at the
  beam end probe `sub_10780`, spawn impact `_4A190(pos, byte_0x43, byte_0x44)`
  = **(10,23)**, `sub_6D8B0(id, 7, 1)` (lightning XP idx 7). The model-9 trail
  nodes are *cosmetic segments of the same-tick flash*, not projectiles.
- **Subtype 12 / model 12 / action 0x0C = `sub_66FD0`** (EF:58691) — "LIGHTNING
  II", the charged flying bolt. Generic flight (row-60 caps 22/22, ±2 speed
  ramp, homing, terrain/water/life) PLUS a drone-lock life-extension arm, PLUS a
  **hard-coded impact**: on detonation it spawns `_4A190(pos, 10, 38)` (NOT its
  own byte_0x43/44), copies id/yaw/pitch/subSpellIndex onto it, and **copies its
  own `(9,9)` into that (10,38)'s byte_0x43/byte_0x44** so the *burst itself*
  later chains a (9,9) (EF:58820-58838, verbatim above in this audit's research).

**Impact identities:**
- `(10,23)` = "blast23" — served: `mc2_spawn_blast23` (proj.rs:391). Tier-0
  impact is therefore already fine; only its *delivery* is wrong.
- `(10,38)` = a class-10 lightning-burst effect (the visible L1/L2 flash),
  UNPORTED. It carries the chained `(9,9)`.
- `(9,9)` = another subtype-9 beam (`sub_4D860`) — in retail it is only ever
  spawned as the *second-order* chain FROM the (10,38) burst, never directly at
  the bolt's death. The port spawns it directly → the misfit.

---

## 2. Retail behaviour per tier

**Tier 0 (life 0) — INSTANT BEAM, rapid-repeatable.** Every fire runs `sub_66750`
to completion in the SAME tick: a hitscan line from the muzzle to the first
solid/terrain hit, drawn as a jagged trail of model-9 segments, detonating in
(10,23). No traveling ball. Because font_type&1=1 → `f59=0`, holding the button
RAPID re-fires it every tick (mc2_cast_input, cast.rs:502), so the visual is a
crackle of flashes present-or-absent — exactly the MC1 Lightning Bolt shape
(`sub_535E0`/`proj_m9_tick`, combat.rs:1438: "resolves in ONE tick … re-laid
every tick, not a traveling ball"). Cast sound 23 (thunder).

**Tier 1 / 2 (life 1 / 2) — CHARGED FLYING BOLT ("storm").** `sub_66FD0` flies a
homing charged bolt (subtype 12, sprite 216, row-60 caps). On impact it spawns
the **(10,38)** lightning burst and chains a follow-on (9,9) beam from it; damage
`subSpellIndex` = 300 (t1) / 800 (t2). CLICK cadence (one bolt per press). Cast
sound 9. ~~The tier-1→tier-2 difference is purely damage + mana~~ **ERRATA
(2026-07-17):** this claim missed the CAST SITE. `sub_6A5C0` (EF:56599-56656,
the class-15 action 0x15) loops `(life_0x1A != 1) + 1` spawns: tier 3
(`life == 2`) fires **TWO** bolts fanned yaw ±113 (≈±19.9°) off the aim
heading — "two L2 bolts side by side" — cross-linked via `word_0x34_52`
(consumer: the beacon drone-lock despawn, `sub_66FD0` EF:58727-33), with
sound 9 once PER spawned bolt. Same subtype, same (10,38) impact per bolt.
The doubling law is duplicated verbatim in the (10,78) auto-caster
`sub_3A8B0` (EF:29984-30021). Ported 2026-07-17 (player report: T3 looked
identical to T2).

Ticked by: retail action table `str90` (trace §"Table anchors") — state 9 →
`sub_66750`, state 0x0C → `sub_66FD0`, dispatched in `Events.cpp:3260-3327`.

---

## 3. Current port behaviour — why it's wrong

**One dispatch, one flight function.** `mc2_proj_tick` (mobs.rs:2179-2196) routes
EVERY `F_MC2PROJ` class-9 entity except the (9,3) meteor shot straight to
`mc2_flyer_tick` (the generic `sub_65820`). Action state (`tick70`) is ignored.
So:

- **subtype 9** is created with `tick70=9`, life 9, speed 384, row 63
  (cast.rs CREATORS row `(9,9,384,9,63,216)`), and then *flown as a homing
  ballistic bolt* over 9 ticks — never the one-tick `sub_66750` beam. Tier 0's
  RAPID re-fire then emits one such slow bolt per tick → **"a stream of
  projectiles"** (player report). The impact `(10,23)` is correct, but the bolt
  crawls to it instead of flashing.
  - The RAPID flag itself is CORRECT and is NOT the bug — do not disable it. The
    stream reads wrong only because each shot is a slow ball rather than an
    instant flash.
- **subtype 12** (tiers 1/2) is likewise flown by `mc2_flyer_tick`. Its
  `mc2_proj_impact` (proj.rs:339-412) spawns the projectile's own `(f68,f69)`,
  which the dispatch set to **(9,9)**. The `match (fc,fm)` has no `(9,_)` arm, so
  it hits the `_ =>` branch (proj.rs:406-410): `note_misfit(9, 9)` + a bare
  `area_write` channel-0 damage. Result: **`WARN: misfit thing (class 9, model
  9)`** and no visible burst — the retail hard-coded `(10,38)` spawn + chain is
  simply absent. `sub_66FD0` is not ported at all.

Nothing serves class-9 model-9 as an *impact* because in retail nothing ever
spawns it as one — it only exists as a same-tick beam (subtype creator) or a
second-order chain from (10,38). The port surfaces it directly because it lacks
the `sub_66FD0` override.

---

## 4. Gap

| Concern | Retail | Port | Status |
|---|---|---|---|
| Dispatch subtype/impact per tier | (9,9)→(10,23) t0; (9,12)→(9,9) t1/2 | identical | ✅ correct |
| Cadence (t0 RAPID, t1/2 CLICK) | font&1 → f59 | identical | ✅ correct |
| Cast sounds (23 / 9) | v6 | identical (cast.rs:710-715) | ✅ correct |
| SPELLS.DAT row 7 tiers | life 0/1/2, dmg 200/300/800 | identical | ✅ correct |
| **Subtype 9 delivery** | one-tick BEAM `sub_66750` | slow homing bolt via `mc2_flyer_tick` | ❌ **L0 stream** |
| Subtype 9 impact effect (10,23) | blast23 | served | ✅ (only delivery is wrong) |
| **Subtype 12 flight** `sub_66FD0` | charged bolt + drone-lock life arm | generic `mc2_flyer_tick` (approx OK) | ⚠️ close, but… |
| **Subtype 12 impact** | hard-coded (10,38) + chain (9,9) | generic (9,9) → misfit | ❌ **L1/L2 absent** |
| Class-10 model 38 burst | lightning-burst effect | unported | ❌ missing |

---

## 5. Fix data

**Fix A — subtype 9 = one-tick beam (fixes L0).** Add an action-9 branch in
`mc2_proj_tick` (mobs.rs:2190-2194): when `model65==9 && tick70==9`, run a new
`sub_66750` port instead of `mc2_flyer_tick`. Behaviour (trace §"State 0x09"):
- `actSpeed = minSpeed` (384); walk the aim ray in `actSpeed/8` sub-steps until
  the first terrain/entity blocker, counting steps `n`; then `n *= 8`.
- Lay `n` cosmetic class-9 model-9 trail nodes (sprite 216) along a per-node ±1
  RNG jag (`rand=9377*rand+9439`, 2 draws/node: z-jag then xy-jag, EF:58359/
  58373) — these are visual segments, life 1, self-despawning; presentation may
  approximate them.
- At the beam end: probe `sub_10780`, spawn impact `(byte_0x43,byte_0x44)` =
  **(10,23)** → existing `mc2_spawn_blast23`, copy id/yaw/pitch/subSpell, award
  lightning XP `(id, 7, 1)`; despawn the beam THIS tick. No water splash, no
  deflection, no life countdown. Net: fire → instant flash → gone. RAPID cadence
  unchanged, so held fire crackles authentically.
- The wizard aim-range for model 9 already exists (`mc2_aim_scan`: `reach =
  minSpeed·maxLife`, proj.rs:782 / EF:54896).

**Fix B — subtype 12 impact = (10,38) + chain (fixes L1/L2).** Either add a
dedicated action-0x0C tick (`sub_66FD0`) or, minimally, special-case the impact.
The critical correction: subtype-12's detonation must NOT use its own (9,9).
Instead (EF:58813-58838, verbatim):
- spawn `_4A190(pos, 10, 38)` — the class-10 lightning burst;
- copy onto it: `id`, `yaw`, `pitch`, `word_0x96_150`=victim (or `0xae02` if
  none), `subSpellIndex` (= dmg 300/800), and `byte_0x43/byte_0x44` = the
  projectile's own **(9,9)** so the *burst* carries the chain forward;
- award `sub_6D8B0(id, 7, 1)`; despawn the bolt.

This requires porting **class-10 model 38** (the lightning burst that deals the
subSpell damage and later chains a (9,9) beam). Until it exists, a faithful
interim is to route `(10,38)` in `mc2_proj_impact` to a blast-style burst that
applies `subSpell` as area damage and (optionally) fires one `sub_66750` beam at
its position — but the honest fix is the real (10,38) effect + its chained beam.
Do NOT leave the generic `(9,9)` path — that is the misfit.

**Also add** a `(9,9)` and/or a routed handling so that even a stray direct
`(9,9)` (should never occur once B lands) does not misfit; but the primary fix is
removing the wrong spawn, not adding a (9,9) impact arm.

**Sounds / models / rows (all confirmed):** subtype creators — (9,9)=`sub_4D860`
sprite 216 life 9 row 63; (9,12)=`sub_4DA20` sprite 216 life 5 row 60
(cast.rs CREATORS rows 155-156). Cast sound 23 (t0) / 9 (t1/2). Impact (10,23)
blast23 (served); (10,38) burst (to port). XP index 7.

---

## 6. Confidence, open questions, suggested test

**Confidence: HIGH** on the identification and the two root causes. The dispatch,
SPELLS row, cadence, and sounds were read directly (baked binary + decompile +
the verbatim class-9 trace). `sub_66FD0`'s hard-coded (10,38)+chain is quoted
verbatim from EF:58813-58838. The one-tick-beam nature of `sub_66750` is
established in the class-9 trace §"State 0x09" and mirrored by the already-ported
MC1 beam (combat.rs:1438).

**Open questions:**
1. **Class-10 model 38** internals (its own life, whether/when it re-spawns the
   chained (9,9), its damage application) are not traced here — needs a class-10
   effect trace before a fully faithful L1/L2 burst can land. Flagged.
2. `sub_66750`'s exact blocker-walk (`sub_66610`) and the trail-node RNG count
   affect state-hash determinism if the trail nodes are real pool entities;
   decide whether the port lays real nodes (RNG-consuming, must be exact) or
   treats them as pure presentation (no RNG) — the latter keeps goldens simpler
   but diverges from retail RNG order. Flagged as a determinism decision.
3. Subtype-12's drone-lock life-extension arm (`sub_68940` branch, EF:58720-33)
   is a no-op without the (10,78) shield/guide entity (unported column) — safe to
   omit now, same as the ricochet gates.

**Suggested test:**
- **L0:** grant Lightning at tier 0, hold-fire at a wall/creature. Expect an
  instantaneous flash to the impact point every tick (crackle), NOT a line of
  slow-moving balls. Assert no traveling class-9 model-9 entity persists >1 tick;
  assert (10,23) blast spawns at the hit.
- **L1/L2:** select tier 1 then tier 2, single-click. Expect a charged bolt that
  flies and detonates into a visible burst; assert **zero** `misfit (class 9,
  model 9)` in the census/log, and that a (10,38)-class effect spawns carrying
  subSpell 300 (t1) / 800 (t2). Re-run `examples/mc2census`/`mc2sweep` after the
  fix to confirm the misfit ledger for (9,9) drops to zero.
