# MC2 Spell Audit — "Summon Creatures" ladder: Metamorph (4) + Summon Army (19)

Fidelity audit for the two spells the player flagged as the firefly/gargoyle/wyvern
ladder. Recorded gameplay is senior; the vendored remc2 decompile is the reference.
Cites: `EF:` = `reference/remc2/remc2/engine/EventsFunctions.cpp`,
`EV:` = `Events.cpp`, `Spells.h`, `Spells.cpp`, `global_types.h`. Port cites are to
`crates/mgc-sim/src/mc2/`.

---

## TL;DR

- **Metamorph (spell 4) IS the transform-into-a-creature spell.** Its effect state
  `sub_6A030` (EF:56294) spawns **one class-5 creature** at the caster and **hides the
  caster's carpet** (bit `0x20`) for the cast window, then un-hides + despawns on expiry.
  The creature model per tier is `SPELLS.DAT[4].subspell[tier].life_0x1A` (EF:56323):
  **tier0 = model 19 (non-Day) / model 2 (Day), tier1 = model 25, tier2 = model 16**.
  Sound **60** (Morph) on start and end. Ownership = the caster's team (allied).
  **PORT = STUB** (`note_misfit(15,4)`, cast.rs:845) — matches the player's
  "WARN: misfit thing (class 15, model 4) x1".

- **⚠️ CORRECTED 2026-07-13 (player-verified + re-trace): Summon Army (spell 19) DOES
  summon a creature army — the earlier "quake" reading was a model-vs-action
  conflation.** Its effect state `sub_6C170` (EF:57638) launches a `(9,24)` projectile
  whose impact is armed to **`(10,72)`** (EF:57665-67). `byte_0x44_68 = 72` is a
  **MODEL** index, not an action index; the prior trace matched it to *action* 72
  (`sub_39040`, the quake) by numeric coincidence. The real class-10 **model-72**
  creator is `sub_51800` (EF:37459, `strA1[72]`), which assigns **action 0x4F=79
  = `sub_3A5B0`** — a **class-5 creature spawner**. The full chain:
  `sub_6C170` (arms `f71 = life = 19/25/16`, Day-patched to 2) → `(9,24)` creator
  `sub_4E050` sets action 25 → **flight state `sub_67800`** (EF:59138, `str90[25]`)
  picks the army size from the creature model (**model 19/2 → 8, model 25 → 4, model
  16 → 2**), spawns a ring of `(10,72)` marker nodes via `sub_51800` (radius 512, angle
  `k·2048/N`), and stamps each node's model/owner → each node ticks `sub_3A5B0`
  (life 16→0) and at 0 spawns `(class 5, model)` allied to the caster, `actionIndex =
  8·model+7` (the SHARED controlled-creature state Metamorph also uses), 250-tick
  lifespan. **Per tier: T0 = 8× model-19 firefly (Day: model-2 bee), T1 = 4× model-25
  Cymmerian, T2 = 2× model-16 wyvern** — low tier = big weak swarm, high tier = small
  strong pack, matching the retail "Firefly/Bee/Cymmerian/Wyvern **Army**" names. The
  `{19,25,16}` triple is IDENTICAL to Metamorph's roster (spell 19 = "an army of what
  Metamorph turns you into"). **No terrain write / no castle-grab** in the real path —
  the player's "brief tremor + terrain mutation" impression is the expanding sprite-220
  summoning ring (visual), not a quake (confidence HIGH). **PORT = WRONG:** launches
  subtype 24 impact `(10,0)` charge=false (cast.rs ~891) → puffs harmlessly, nothing
  summoned. FIX RECIPE: arm `subtype 24, impact (10,72), charge true`, carry `f71 =
  spells[19].tiers[tier].life`; new `(10,72)` impact handler spawns the N-node ring →
  each node spawns the class-5 creature (blocked on the shared `8·model+7` controlled-
  creature action, also Metamorph's dependency). Confidence HIGH end-to-end (5 verified
  hops). This corrects `quake-family.md` too (its (10,72)=quake note inherited the same
  conflation).

- **AMBIGUITY (must flag):** the player expects BOTH spells to summon
  firefly/gargoyle/wyvern. The decompile only supports that for **Metamorph**. Spell 19's
  code is unambiguously a quake. Note the coincidence that spell 4 and spell 19 carry the
  **identical** `life_0x1A` triples `{0x13,0x19,0x10}` = `{19,25,16}` — but for spell 19
  those bytes only feed the quake's life (`& 0xF0` = 16 every tier), never a creature.
  Recommend the player eyeball spell 19 in the retail recording before we build a quake
  that the campaign may never actually show as an "army".

---

# PART A — METAMORPH (spell index 4)

## A.1 Identification

| Field | Value | Cite |
|---|---|---|
| Spell enum | `metamorph = 4` | global_types.h:140 |
| Manifestation | class 15, model 4 | (adopt) cast.rs:463 |
| Effect-state action | `3·4 = 12 = 0x0C` → strF0[0x0C] = `0x24B030` = **`sub_6A030`** | EF:1962, EF:56294 |
| Kind | channel / armed-window (not a projectile) | EF:56319 |
| Spawned entity | **class-5 creature**, model = `SPELLS.DAT[4].subspell[tier].life_0x1A` | EF:56323 |
| Cast sound | **60** (`Morph_60`) on start and on end | EF:56342, EF:56405 |

`life_0x1A` is field **[7]** of the 26-byte tier struct (`Spells.h:8-17`:
`subSpellIndex_2, manaCost_6, maxManaLimit_A, xpos1_E, xpos2_0x12, hintText, word_0x18,
life_0x1A, fontType`).

SPELLS.DAT fallback row for spell 4 (`Spells.cpp:19-22`):

| tier | subSpell | manaCost | maxManaLimit | word_0x18 (duration) | **life_0x1A** |
|---|---|---|---|---|---|
| 0 | 100 | 5000 | 20000 | 201 | **0x13 = 19** |
| 1 | 100 | 7500 | 20000 | 301 | **0x19 = 25** |
| 2 | 100 | 15000 | 25000 | 455 | **0x10 = 16** |

**Level-init override (tier 0 only):** `LevelInit_56C00` re-writes row 4 tier-0 `life`
per map type — **Day → 2**, non-Day (Night/Cave) → 19 (already ported verbatim,
spells.rs:104-113; trace mc2-class10-m9-dome-open-closure.md §4.5). So metamorph tier-0
becomes **(5,2)** on Day maps and **(5,19)** elsewhere. Tiers 1/2 are untouched.

Creature models (real class-5 roster entries — creators live in `str50[236]`, EF:1242):
`(5,16)` boss-class flyer (homing bolt, mana 50000, no flee — trace mc2-class5-m16-20.md),
`(5,19)` a flyer with ranged `(9,0)` bolt (subSpell 500) + melee (300) (same trace §"MODEL
19 VERDICT: m19 DOES fly"), `(5,25)` a castle-drainer (60 dmg to castle via mailbox, trace
mc2-class5-m25-26-28). `(5,2)` is the Day-map tier-0 creature.

> Naming caveat: the **firefly / gargoyle / wyvern** labels are the player's; the decompile
> gives model IDs 19/25/16 (+2 on Day), not names. `FireFly1_43 / FireFly2_44 / Wyvern_39`
> exist in the entity-sound table (Sound.cpp:6353,6438; WavIndexes.h) confirming those
> creatures are in the roster, but the exact sprite↔name binding is presentation-side
> (sprite/creature-name table, not read here). The FIX is data-driven regardless — read
> `row.tiers[tier].life`.

## A.2 Retail behavior per tier (`sub_6A030`, EF:56294-56413)

Runs every tick for the learned manifestation while `word_0x2E_46 > 0` (the armed timer).
`v2x` = the caster (`Entities[parentId_0x28_40]`).

**First tick** (`word_0x2E_46 == word_0x30_48`, EF:56319):
1. XP award: `sub_6D8B0(parentId, 4, 1)` (EF:56321).
2. **Spawn the creature:** `v4x = IfSubtypeCallCreatingManaSphere_4A190(&caster.pos, 5,
   SPELLS[4].subspell[tier].life_0x1A)` (EF:56323). If non-null (EF:56325):
   - `StageVar2_0x49_73 = 12` (EF:56328)
   - `actionIndex_0x45_69 = 8·creature.model + 7` (EF:56327,56329) — the model's dedicated
     **+7 "player-controlled/metamorph" action slot** (class-5 models are 8 actions apart,
     e.g. m16 base 128 = 8·16).
   - `parentId_0x28_40 = caster.parentId`; `id_0x1A_26 = caster.id` → **allied to the
     caster's team** (EF:56330-56331).
   - `byte[0] |= 1` (active/visible) (EF:56334).
   - **`caster.byte[0] |= 0x21`** — sets `0x20` = **hide the caster's carpet** + `0x01`
     (EF:56336). This is the visual transform: the wizard's carpet vanishes, the creature
     shows in its place.
   - `manifestation.word_0x96_150 = creature` (link for teardown) (EF:56333).
   - Remote-owner branch (`playerColorIndex != LevelIndex`): flips which of creature/caster
     is drawn (EF:56337-56341).
   - **Sound 60** on the caster (EF:56342).
3. Commit mana: `sub_68DE0(manifestation, caster)` (EF:56345). Can't-afford → `word_0x2E_46
   = 1` collapse (EF:56349).

**Per tick (remote only):** an ownership/visibility toggle keeps exactly one of
creature/caster drawn as the timer counts (EF:56351-56391) — cosmetic for MP; single-player
caster is always `playerColorIndex == LevelIndex` so this branch is skipped.

**Expiry** (`word_0x2E_46` reaches 0, EF:56394-56409):
- `DisableEntityDrawing04(creature)`, `word_0x96_150 = 0` (EF:56399-56400) — despawn creature.
- `caster.byte[0] &= 0xDF` — **un-hide the carpet** (EF:56403).
- **Sound 60** again (EF:56405).
- `sub_6D880(manifestation)` — apply any pending tier change (EF:56408).

Duration = `word_0x18` = **201 / 301 / 455** ticks (tiers 0/1/2); mana upkeep divides the
`manaCost` across the window (the standard cast-tick skeleton, cast.rs:255-259).

## A.3 Current port

**STUB.** `mc2_spell_fire` routes spell 4 into the `4 | 0xD | 0xE` arm →
`self.g.note_misfit(15, spell as u16)` (cast.rs:845-847). The cast **gate + mana** ARE
ported: the model-4 cast-gate arm extends the timer to 7 on re-press (cast.rs:551-554), the
mana gate/commit run (cast.rs:563, 619). Only the FIRE effect (creature spawn + carpet hide)
is missing — exactly the "misfit thing (class 15, model 4) x1, degraded" the player saw.

## A.4 Gap

No creature spawns; the carpet is never hidden; no Morph sound; the manifestation link is
never set. The player sees only the misfit-ledger warning. Higher tiers untested but would
be identically stubbed (all route through the same arm).

## A.5 Fix data (verbatim)

- **Creature to spawn:** `(class 5, model = row.tiers[tier].life as u8)` at the caster's
  position. With the current bundle + level-init that is **tier0 → 2 (Day) / 19 (non-Day),
  tier1 → 25, tier2 → 16**. Read it from `self.g.assets.spells[4].tiers[tier].life` (data-
  driven — do NOT hardcode; the CD SPELLS.DAT wins over the fallback, EPOCH 6; spell 4 is
  not in the par1-override set 9/11/15 so the CD value is used as-is).
- **Count:** exactly **1**.
- **Creature init:** `actionIndex = 8·model + 7`, `StageVar2_0x49_73 = 12`, `parentId =
  caster.parentId`, `id = caster.id` (allied), active flag set.
- **Caster:** set the hide bit (`0x20`) at cast start, clear it at expiry — the port needs
  a `player.metamorph_hidden` (or reuse the invisible/draw-suppress channel) so the render
  layer drops the carpet and draws the creature instead.
- **Manifestation:** `word_0x96_150` ← creature entity index (for teardown; the port's
  equivalent link field).
- **Sounds:** `snd_player(60)` on start AND on expiry.
- **XP:** `mc2_award_xp(PLAYER_TARGET, 4, 1)` on the first fire tick.
- **Duration:** already flows from `word_0x18` via the generic cast-tick.

## A.6 Confidence / open / test

- **Confidence: HIGH** on the spawn law, ownership, carpet-hide, sound, and the model
  triple (all verbatim from `sub_6A030` + the SPELLS row + the ported level-init patch).
- **OPEN:** (1) exact sprite↔name binding (is (5,19) really a "firefly"?) — presentation
  table, unread. (2) The `actionIndex = 8·model + 7` "+7" state body per creature (how the
  player STEERS the creature — is it flight-locked to the carpet controls?) is a class-5
  action-state trace not done here; the metamorph creature is likely driven like a
  possessed mob. (3) Whether the creature takes carpet input or free-roams AI — needs the
  +7 state read.
- **Suggested test:** cast metamorph tier 0 on a Night level → expect the carpet to vanish
  and a `(5,19)` creature to appear under player control, Morph sound both ends, reverting
  after ~201 ticks. Cast on a Day level → expect `(5,2)` instead. Tier 1 → `(5,25)`,
  tier 2 → `(5,16)`.

---

# PART B — SUMMON ARMY (spell index 19 / 0x13)

## B.1 Identification

| Field | Value | Cite |
|---|---|---|
| Spell enum | `summon_army = 19` | global_types.h:155 |
| Manifestation | class 15, model 19 | cast.rs:463 |
| Effect-state action | `3·19 = 57 = 0x39` → strF0[0x39] = `0x24D170` = **`sub_6C170`** | EF:2007, EF:57638 |
| Projectile spawned | **`(9,24)`** subtype 0x18 = `sub_4E050` (class 9, model 24, sprite 281, row 60, maxLife 20) | EF:35221, mc2-class9-flyers §0x18 |
| Projectile flight state | action 0x18 = `sub_677D0` (full flight; on impact sets spawned life = `byte_0x46_70 & 0xF0`) | EF:59120 |
| **Impact** | **`(10,72)`** = `sub_39040` — a **terrain quake**, NOT a creature | EF:57665-67, EF:28452 |
| Cast sound | **9** | EF:57681 |

## B.2 Retail behavior (`sub_6C170`, EF:57638-57700)

First tick (`word_0x2E_46 == word_0x30_48`, EF:57656) spawns and arms the `(9,24)`:
- `v6x = IfSubtypeCallCreatingManaSphere_4A190(&caster.pos, 9, 24)` (EF:57659).
- `actSpeed += caster.actSpeed` (EF:57662); `sub_68E50` ownership (EF:57663).
- `byte_0x43_67 = 10`, **`byte_0x44_68 = 72`** → impact `(10,72)` (EF:57665,57667).
- `word_0x26_38 = manifestation` back-ref (EF:57666); `id = caster.id`; `parentId =
  manifestation.parentId` (EF:57668-69).
- `byte_0x46_70 = SPELLS[19].subspell[tier].life_0x1A` = **{19,25,16}** (EF:57670).
- `dword_0x10_16 = caster.dwordA4.byte_0x154_340`, then **that field is zeroed** (EF:57671-
  72) — summon_army **consumes an accumulated caster resource** (`byte_0x154_340`) and hands
  it to the projectile.
- Launch angles from `caster.dwordA4.nextEntity_0x18_24 + caster.yaw` and
  `entityIndex2_0x1A_26 + caster.pitch`, `MoveEntity(...,0x4000)` (EF:57674-80).
- **Sound 9** (EF:57681). Then `sub_68DE0` mana commit (EF:57684).

The `(9,24)` flies (state `sub_677D0`, EF:59120): on impact it spawns `(10,72)` with
`life = maxLife = (byte_0x46_70 & 0xF0)` (= **16** for every tier, since 19/25/16 & 0xF0 =
16) and zeroes the projectile's `byte_0x46_70`.

**Impact `(10,72)` = `sub_39040` (EF:28452)** — a 4-phase (`byte_0x46_70` 0→3) machine:
- phase 1 (EF:28538): sample terrain height, set the raise target, `dword_0x10_16 = 12`,
  **sound 64**.
- phase 2 (EF:28553-28701): interpolate `mapHeightmap_11B4E0` / `mapShading` upward across a
  30×30 tile block toward a dome profile (`sin_DB750` LUT); when `dword_0x10_16 == 5` call
  **`sub_3A090`** (EF:28661).
- phase 3 (EF:28702-28751): flatten terrain type to 1, **`AddBuildingToTerrain_46570`**
  (EF:28727), action → 73.
- `sub_39B60` (EF:29011): **shoves nearby entities** away from / up the rising terrain
  (class-3 avatars handled specially).
- **`sub_3A090` (EF:29316):** the destructive core — kills entities on the
  `dword_38527` list, **grabs class-3 model-2 CASTLES** on the `dword_38519` list
  (`byte[2] |= 0x10`, `word_0x30_48 = 30`, adds `subSpellIndex` damage, tags owner
  `id_0x1A_26`), flattens terrain, and awards `sub_6D8B0(id, 0x14, count)` XP. These are the
  **same two lists** the (10,67) flood/quake uses (mgc_sim::mc2::flood; trace
  mc2-class10-m67-flood-helpers.md).

**Net: spell 19 raises a mound/island and grabs castles + destroys buildings. It spawns no
creatures on any tier.**

## B.3 Current port

`mc2_spell_fire` spell `0x13` (cast.rs:789-802): launches subtype **24** with impact
**`(10, 0)`** and `snd_player(9)`. It does **not** set `byte_0x46_70` (life), does not
consume/pass `byte_0x154_340`, and does not use the `dwordA4` launch angles. `(10,72)`
`sub_39040` is **entirely unported**. The port author's comment ("summon_army launches
class-9 subtype 24 = the CREATORS summon (0x18) entry", cast.rs preamble) reflects the
*guess* that it summons — the decompile does not bear that out.

## B.4 Gap

1. **Wrong impact model:** port arms `(10,0)` (a generic puff) instead of `(10,72)`. This
   alone is why the shot "does nothing" — `(10,0)` just makes a fireball-style splash.
2. **Quake unported:** `sub_39040` (terrain raise + `sub_3A090` castle-grab + `sub_39B60`
   entity shove + `AddBuildingToTerrain`) does not exist in the port.
3. **Missing arming:** `byte_0x46_70 = life`, `dword_0x10_16 = byte_0x154_340`
   (consume-and-zero), and the `dwordA4` launch geometry are not written.
4. **Expectation mismatch:** neither retail nor the fix produces creatures — the player's
   "fireflies/gargoyles/wyverns" belief for spell 19 is **not supported by the decompile**.

## B.5 Fix data (verbatim)

- **Effect-state arm** (cast.rs:789 arm): `subtype 24`, **impact `(10, 72)`** (not 0),
  `charge = false`. Also set on the projectile: `byte_0x46_70 = row.tiers[tier].life`
  (= 19/25/16), and (if the `byte_0x154_340` accumulator gets ported) `dword_0x10_16 =
  that value` then zero it. Launch angles: retail adds `dwordA4.nextEntity` / `entityIndex2`
  offsets to the carpet yaw/pitch — APPROX with the plain carpet pose until that field pair
  is traced. Sound **9** (already correct).
- **Projectile `(9,24)`:** creator `sub_4E050` (EF:35221) — class 9, model 24, actSpeed=384,
  `maxLife = (0x2000/384) & 0xFC = 20`, row 60, sprite 281. Flight state 0x18 (`sub_677D0`):
  standard flyer flight; on impact set the spawned `(10,72)` `life = maxLife =
  (byte_0x46_70 & 0xF0) = 16`.
- **Impact `(10,72)` `sub_39040`:** a terrain quake. **Because it shares the
  `dword_38519`/`dword_38527` castle/building lists with the already-ported (10,67) flood,
  port it alongside `mgc_sim::mc2::flood`.** Key spawns/effects: raise `mapHeightmap` over a
  30×30 block (dome profile via the `sin_DB750` LUT), sound 64; `sub_3A090` grabs class-3
  model-2 castles + destroys `dword_38527` entities + XP `sub_6D8B0(id, 20, count)`;
  `sub_39B60` shoves nearby entities off the rise; `AddBuildingToTerrain_46570` finalizes.
- **Ownership:** the grabbed castles / awarded XP are tagged to the caster's `id_0x1A_26`.
- **Sounds:** cast 9 (done); impact **64** (from `sub_39040` phase 1).
- **No creatures. No allegiance army.** (Under the decompiled path.)

## B.6 Confidence / open / test

- **Confidence: HIGH** that spell 19 → `(9,24)` → `(10,72)` and that `(10,72)` is a terrain
  quake, not a creature summon (three independent confirmations: strF0 action 57 = 3·19,
  the `byte_0x44_68 = 72` arm, and the `sub_39040` body reading heightmap + `sub_3A090`
  castle-grab). **HIGH** that the port's `(10,0)` impact is a bug.
- **OPEN / AMBIGUITY (flag to player):**
  1. **Is spell 19 really an "army"?** The remc2 enum name `summon_army` is the decompiler's
     guess; the code is a quake. Either the name is a misnomer, or the player conflated
     spell 19 with Metamorph. **Recommend the player watch spell 19 in the retail recording
     before we invest in porting `sub_39040`** — if it visibly summons creatures in-game, we
     have a mis-mapping to chase (nothing in the traced path supports it).
  2. `byte_0x154_340` (the consumed caster accumulator) is untraced — what fills it, and
     what `dword_0x10_16` does with it downstream, needs a follow-up read (it is overwritten
     to 12 in `sub_39040` phase 1, so its role may be pre-impact only).
  3. The `dwordA4.nextEntity_0x18_24 / entityIndex2_0x1A_26` launch-offset pair is the same
     unknown field pair seen in the (9,29)/(9,25) launchers — a shared banked open item.
- **Suggested test:** once `(10,72)` is ported, cast spell 19 at open terrain → expect a
  mound to rise + sound 64; cast near an enemy castle → expect the castle grabbed/damaged
  (same as a (10,67) flood hit). Confirm NO creatures appear.

---

## Cross-cutting note

The identical `life_0x1A = {19,25,16}` in both spell 4 and spell 19 rows is the only thread
tying them together, and it is a red herring for spell 19 (masked to 16 for the quake). The
**only** spell in this pair that produces the firefly/gargoyle/wyvern-class creatures is
**Metamorph (4)** — and it does so by transforming the caster, not by summoning an army.
Porting Metamorph's creature spawn is the high-value, unambiguous win; Summon Army needs a
player-facing behavior confirmation before its quake port is worth the `sub_39040` effort.
