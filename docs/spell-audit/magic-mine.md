# Spell Audit — Magic Mine (index 23 / 0x17)

**Verdict: the port is wrong.** Retail Magic Mine LOBS a short-range carrier to a
ground point ahead of the caster; that carrier LANDS and spawns a **persistent,
stationary proximity mine** (class-10 model-78, life 1000, sprite 66) that arms
after a delay and detonates when an enemy wizard/castle comes within ~14 tiles.
The port fires the carrier straight from the muzzle as a generic flyer that
detonates on the **first contact** and spawns a **fireball (10,0)** — exactly the
player's report ("shoots a projectile / functions as a fireball / explodes on
impact for standard damage"). The whole persistent-mine half of the spell is
absent.

Citations: `EF:` = `reference/remc2/remc2/engine/EventsFunctions.cpp`; port =
`crates/mgc-sim/src/mc2/`.

---

## 1. Identify — the retail pipeline

Magic Mine is a **three-entity chain**, not one projectile:

**(a) The cast — `sub_6CAC0` (EF:57960), spell index 23.**
The player-cast path (`docs/traces/mc2-player-cast-path.md` §row 23) routes spell
23 to `sub_6CAC0`, a caster-parented spell-holder tick. On its trigger tick
(`word_0x2E_46 == word_0x30_48`) it spawns the carrier `_4A190(pos, 9, 29)` and
arms it (EF:57981-58005):
- `byte_0x43_67 = 10; byte_0x44_68 = 78` → **impact = (10,78)** (the persistent mine).
- `id_0x1A_26 = caster.id`; `mana = caster.mana`; `subSpellIndex = caster.byte_0x46_70` (the tier).
- `position.z += caster.fov`; then the **aim/destination** `axis_0x9A_154x` is set
  to the caster position moved **forward 4096 units (16 tiles)** along the caster
  yaw with `z = getTerrainAlt` (EF:57998-58001) — i.e. a **ground point ~16 tiles
  ahead**, the spot where the mine will be laid.
- `PrepareEventSound(...,-1,15)` → **sound 15**.

**(b) The carrier — subtype 0x1D creator `sub_4E2A0` (EF:35310).**
`actionIndex = 30; class = 9; model = 29; actSpeed = minSpeed = 384;
maxLife = 10 (literal); mana = 50; row 60; sprite 66`
(matches `docs/traces/mc2-class9-flyers.md` §0x1D and the port's `CREATORS` row).

**(c) The carrier tick — action 30 = `sub_67960` (EF:59240).** This is NOT the
generic flyer `sub_65820`; it is a dedicated mine-lander:
- First tick (`life==maxLife && subSpellIndex>=2`): scans the class-10 list
  (`dword_38535`) for an **enemy model-78 mine** (`word_0x32_50 != self.owner`,
  `subSpellIndex<2`) within the behavior-row range `word_160_0x1c_28`; if found,
  locks it (`word_0x96_150`), sets `maxLife=32`, homes (EF:59267-59298). (Mine-vs-mine
  seek; rarely relevant.)
- Otherwise: `MoveEntity` forward by `actSpeed` (384), then clamp to terrain
  (`getTerrainAlt > z`) / cave ceiling; decrement `life`; when it reaches the
  ground OR `life<0` → **detonate: spawn `(byte_0x43,byte_0x44)=(10,78)`** at the
  landing position, copy owner into the mine's `word_0x32_50` and the tier into
  its `subSpellIndex`, then despawn (EF:59331-59362). If it instead reached a
  homed model-78 target it spawns **(10,0)** and tags it (EF:59326-59330).

**(d) The persistent mine — (10,78) = (0xA,0x4E), creator `sub_50840` (EF:36960).**
`maxLife = 1000; actionIndex = 0x55; class = 0xA; model = 0x4E; sprite 66;
word_0x32_50 = self (owner slot); byte_0x43/44 = 1/0`. Long-lived, sits on the
ground. **Sprite 66 is the same sprite as the carrier** — the "mine" you see.

**(e) The mine tick — action 0x55 = `sub_3A8B0` (EF:29749).** The class-10 action
table maps `0x55 → 0x21B8B0` (EF:1687). A `byte_0x46_70`-phased state machine:
- **phase 0** (EF:29880): reads `SPELLS_BEGIN_BUFFER_str[23].subspell[tier].subSpellIndex_2`
  → sets `maxLife/life` (the mine's **lifespan**); `axis_0x9A = position` (rest spot);
  blast intensity `byte_0x43` from `tier.life_0x1A` (**0→1, 1→2, 2→4, 3→8**); → phase 1.
- **phase 1** (EF:29918): wait until armed (`word_0x36_54 != 0xffff`) → phase 2.
- **phase 2** (EF:29926): clear target-eligible bit, `id = owner`, seed a random
  **arming delay `dword_0x10_16 = rand%0x32 + 16` (16–65 ticks)** → phase 3.
- **phase 3** (EF:29943): count the arming delay down → phase 4.
- **phase 4** (EF:29949) — **PROXIMITY SCAN**, only every 16 ticks (`!(byte_0x3E_62 & 0xF)`):
  scan the class-3 list (`dword_38519` = wizards/castles) for **model ≤ 1** entities
  (avatars/castles) within **distance 3584 (= 14 tiles)**, excluding the owner; lock
  nearest → phase 5.
- **phase 5** (EF:29970+) — **DETONATE**: award XP `sub_6D8B0(owner, 0x17, 1)`, then
  relaunch the damaging blast via `sub_6DCA0(owner, &mine.pos, word_0x36_54,
  subspell, 0, 1)` (the standard player-spell projectile launcher; fireball-7
  special fires twice). This is where the actual HP damage happens.

---

## 2. Retail behavior per tier (0 / 1 / 2)

Placement, arming and trigger geometry are **tier-independent**; only lifespan,
mana and blast intensity scale. Values decoded from `baked/assets/mc2-*/spells.bin`
row 23 (identical across night/day/cave):

| Tier | `subSpell` = **mine lifespan** (ticks) | `manaCost` | `maxManaLimit` | `life_0x1A` → **blast `byte_0x43`** |
|------|-----|------|--------|------|
| 0 | 1000 | 10000 | 300000 | 0 → **1** |
| 1 | 5000 | 12000 | 350000 | 1 → **2** |
| 2 | 10000 | 24000 | 400000 | 2 → **4** |

All tiers:
- **Placement**: carrier flies ~16 tiles ahead and lands on the ground (terrain
  clamp) — the mine is not laid at the caster's feet.
- **Arming delay**: `rand()%50 + 16` = **16–65 ticks** after landing (phase 2→3).
- **Proximity trigger radius**: **3584 units = 14 tiles**; scan cadence **every 16
  ticks**; only fires on class-3 **model ≤ 1** targets (enemy wizards / castles),
  never the owner (EF:29949-29968).
- **Detonation**: `sub_6DCA0` relaunch of the spell's blast at the mine position;
  intensity `byte_0x43` = 1/2/4 by tier; XP award to owner.
- **Lifespan**: mine self-expires after its `subSpell` ticks (1000/5000/10000) if
  never triggered (phase 0 sets life; the countdown at EF:29841 flips `byte_0x46_70=6`).
- **Sound**: cast plays **sound 15** (EF:58004). (Detonation sound rides `sub_6DCA0`.)

---

## 3. Current port — what it actually does

`crates/mgc-sim/src/mc2/cast.rs:812-826` (spell `0x17`):
```
0x17 => { self.mc2_launch(spell, m, &DispatchArm { subtype: 29,
                                                     impact: (10, 0),
                                                     charge: false }, sub, p);
          self.g.snd_player(15); }
```
- `mc2_launch` (cast.rs:857) spawns model-29 via `mc2_spawn_cast_proj` **from the
  firing-hand muzzle** (cast.rs:872), sets launch angles = the carpet pose, and
  applies the caster speed-boost — i.e. a normal forward-fired projectile, NOT a
  ground-lob to a point 16 tiles ahead.
- Impact is armed **(10,0)** (`f68=10, f69=0`).
- The projectile is ticked by the **generic** `mc2_flyer_tick` (`proj.rs:453`,
  the `sub_65820` port) — which detonates on the **first entity or terrain
  contact**. There is no port of action-30 `sub_67960` (land-then-place).
- On impact, `mc2_proj_impact` (`proj.rs:339`) matches `(10,0) => mc2_spawn_fire`
  (`proj.rs:345`) — a **fireball explosion**.
- `snd_player(15)` — sound 15 is correct.

So the port = "muzzle-fired flyer that explodes on first contact into a fireball."
**Confirmed** to match the player report. `CREATORS` row (cast.rs:164) does carry
the right ctor stats `(29,30,384,10,60,66)`, but nothing consumes action 30 or
impact 78.

---

## 4. Gap

| Aspect | Retail | Port | Severity |
|--------|--------|------|----------|
| Launch geometry | Lobbed to a ground point ~16 tiles ahead (`axis_0x9A`, z=terrainAlt) | Fired straight from the muzzle | High |
| Carrier tick | Action 30 `sub_67960`: fly → land on terrain → place mine | Generic `mc2_flyer_tick`: detonate on first contact | High |
| Impact effect | **(10,78)** persistent proximity mine | **(10,0)** fireball | Critical |
| Persistent mine entity | (10,78)/`sub_50840` + tick `sub_3A8B0` (arm + proximity + detonate) | **Does not exist** | Critical |
| Proximity trigger | Arm 16–65 ticks, scan every 16, radius 14 tiles, model≤1 class-3 | None (contact-detonate) | Critical |
| Damage timing | Delayed, on enemy approach, via `sub_6DCA0` relaunch, `byte_0x43` 1/2/4 | Immediate fireball area damage on impact | High |
| Tier scaling | Lifespan 1000/5000/10000 + blast 1/2/4 | Only carries `f44` damage | Medium |
| Sound | Cast sound 15 | Sound 15 | OK |

Net: the port delivers **immediate contact damage** where retail delivers a
**deferred, placed, proximity-triggered** mine. Functionally a different spell.

---

## 5. Fix data

Port both missing halves.

**A. Carrier launch + tick (action 30 / model 29).**
- In the `0x17` cast arm, do NOT muzzle-fire. Replicate `sub_6CAC0`: spawn the
  (9,29) carrier and set its destination `axis_0x9A` = caster pos moved forward
  4096 units (16 tiles) along caster yaw, `z = ground_z`; `id = PLAYER_TARGET`;
  `subSpellIndex = tier`; `mana = caster mana`; impact **(10,78)**.
- Give model 29 a dedicated tick (port `sub_67960`): move forward at speed 384,
  clamp to terrain; when it reaches ground (`ground_z > z`) or `life<0` (life=10),
  spawn **(10,78)** at the landing point, carrying `word_0x32_50 = owner` and
  `subSpellIndex = tier`, then despawn. (The model-78 homing branch is a minor
  mine-vs-mine seek; can be deferred.)

**B. The persistent mine entity (10,78) — new class-10 model.**
Creator (`sub_50840`): `max_life = 1000`, `action = 0x55`, sprite **66**,
`word_0x32_50 = self`, owner-tagged, spawned at ground level. Then port
`sub_3A8B0` as the tick, a phased state machine:
1. **Init**: lifespan = `spells[23].tiers[tier].sub_spell` (1000/5000/10000);
   blast intensity from `tiers[tier].life` → 0/1/2 map to `byte_0x43` = 1/2/4 (and 3→8).
2. **Arm delay**: `rand()%50 + 16` ticks (respect the port's RNG law
   `r = 9377*r + 9439`, per `docs/traces/mc2-class10-*` RNG note).
3. **Proximity scan** every 16 ticks: class-3 list, `model ≤ 1`, distance `< 3584`
   (14 tiles), exclude owner. On hit → detonate.
4. **Detonate**: relaunch the blast at the mine position (port equivalent of
   `sub_6DCA0` for spell 23 at intensity `byte_0x43`) and award XP
   `sub_6D8B0(owner, 0x17, 1)` (the port's `mc2_cast_xp` push, spell 23).
5. Self-expire when lifespan runs out with no trigger.

**C. Impact routing.** Add `(10, 78) => <spawn persistent mine>` to
`mc2_proj_impact` (`proj.rs:344`) so the carrier's landing spawns the mine instead
of falling through the misfit `_ =>` branch (which currently degrades it to a bare
area-damage write).

**Sounds.** Cast: 15 (already correct). Detonation: whatever `sub_6DCA0` plays for
spell 23 — trace when porting the blast (OPEN).

---

## 6. Confidence, open questions, test

**Confidence: high** on the shape and on the numeric fields cited (creator stats,
SPELLS.DAT row 23 tier values, radius 3584, arm delay 16–65, blast map 1/2/4/8,
impact (10,78), sound 15). The (9,29)→(10,78)→`sub_3A8B0` chain is decompile-verified
end to end; the player report independently corroborates the port defect.

**Open questions:**
1. **`word_0x36_54` / `word_0x34_52` provenance on the mine.** Phase 1 waits on
   `word_0x36_54 != 0xffff` and phase 5's `sub_6DCA0` uses these as the blast's
   spell/subspell. `sub_50840` leaves `word_0x36_54 = -1`; the writer that sets it
   to spell 23 / the tier subspell (likely in `sub_6CAC0`'s `dword_0x10_16`
   propagation or the carrier) was not pinned in this pass. Trace before porting
   the exact detonation blast.
2. **Exact detonation blast of `sub_6DCA0` for spell index 23.** `sub_6DCA0`'s
   documented a3-map (`mc2-class9-flyers.md` §3.4) doesn't list 23; confirm which
   projectile family / damage it emits (and its sound) so the port's detonation
   matches, rather than assuming a fireball.
3. **Carrier count per cast.** `sub_6CAC0` fires on the `word_0x2E_46 ==
   word_0x30_48` tick — believed to be exactly one mine per cast; verify it does
   not re-lay while the spell-holder lives.
4. **Model-78 vs -78 homing** (`sub_67960` first-tick branch) — low priority; only
   matters if opposing mines coexist.

**Suggested test.** On an MC2 level, cast Magic Mine at open ground with no enemy
in front: retail lays a stationary sprite-66 mine ~16 tiles ahead that persists and
does nothing until a wizard/castle approaches within ~14 tiles, then detonates.
Port today: a fireball flies from the hand and bursts on the first thing it hits
(or the ground) immediately. A state-hash golden that casts the mine and steps ~80
ticks with a mob walked into range would lock the fixed behavior (mine placed →
armed → triggered on approach), and distinguish it from the contact-fireball
regression.
