# Spell Audit — Fool's Mana (index 22 / 0x16)

**Verdict (original): PORT IS WRONG.** The port cast ONE real,
collectible mana sphere that added to the pool. Retail casts a SHOTGUN
SPREAD of SIX neutral FAKE-mana spheres that, when an enemy wizard tries
to possess one, DETONATE against the possessor (fireball / repeated
fireballs / lightning by tier) and vanish.

**STATUS: LANDED.** The cast burst landed 2026-08-02; the TRAP TICK —
and with it the AUTHORED ground spheres — landed 2026-08-03 (session 7).
See §3 for the port as it now stands and §6/OPEN-2 for the resolution
that turned this from "a spell-22 decoy feature" into "the (10,57)
tick's own law".

Citations: `EF:` = `reference/remc2/remc2/engine/EventsFunctions.cpp`,
`EV:` = `Events.cpp`, `L:` = `Level.cpp`. Port cites = `crates/…`.

---

## TL;DR — the player report is confirmed, tier for tier

| Tier | subspell `life_0x1A` | On possession attempt (retail) | Cite |
|---|---|---|---|
| L1 (tier 0) | 0 | fires **ONE fireball** at the possessor, then vanishes | EF:26631-26638 → `sub_36770` EF:26672 |
| L2 (tier 1) | 1 | fires **repeated fireballs** (every other delivery-tick, up to 8), then vanishes | EF:26640-26651 → `sub_36770` |
| L3 (tier 2) | 2 (or 3) | fires **ONE lightning bolt** at the possessor, then vanishes | EF:26652-26666 → `sub_36850` EF:26701 |

The fireball/lightning **homes on the wizard who tried to possess it**
(`word_0x96_150 = claimer id`, aimed via `sub_655C0`), carrying the
tier's damage payload. This is the whole point of the spell: bait an
enemy wizard's mana-collection AI (or a rival's possess) into eating a
bomb.

---

## 1. Identity — spell 22, the entity, the data

- **Spell enum:** `fools_mana = 22` (`global_types.h:158`).
- **Class-15 model:** 22. Its equipped manifestation's effect state is
  **`sub_6C870` (EF:57868)** — the fool's-mana cast tick (dispatch is
  the `strF0[3·22]` effect row; it is a **direct-effect** spell, it does
  NOT route through `sub_6DCA0`). Confirmed by
  `docs/traces/mc2-player-cast-path.md` §2.2 row model 22.
- **The effect entity IS (10,57)** — but NOT the authored ground sphere
  from `sub_50130`. `sub_6C870` spawns (10,57) via
  `IfSubtypeCallCreatingManaSphere_4A190(&caster.pos, 10, 57)`
  (EF:57894) and then **overwrites** the key fields to weaponize it (see
  §2). The (10,57) creator body and settle/physics tick are documented
  in `docs/traces/mc2-class10-m57.md`; the retaliation half of its tick
  (`sub_36680`) was NOT covered by that trace — this audit adds it.
- **Sounds:** cast sound **11** (`FoolMana_11`, `SoundInGameIndexes.h:15`,
  `Sound.cpp:6383`), fired once after the 6-sphere loop (EF:57924).
  Retaliation fireball plays sound **9** (EF:26688); the lightning
  retaliation plays no explicit sound.
- **SPELLS.DAT row 22 tiers:** the branch selector is
  `subspell[tier].life_0x1A` (port field `Mc2SubSpell::life`,
  `crates/mgc-sim/src/mc2/spells.rs:41`). The player's observed
  fireball/fireballs/lightning ⇒ life values **{0, 1, 2}** for tiers
  {0,1,2}. Exact bytes are data-driven — **verify from baked
  `spells.bin` row 22** (OPEN-1). `subspell[tier].subSpellIndex_2` is
  the damage payload carried onto every retaliation projectile.

---

## 2. RETAIL — the cast (`sub_6C870`, EF:57868)

Canonical effect-state skeleton (armed on `word_0x2E_46>0`, fires on the
FIRST tick `word_0x2E_46 == word_0x30_48`, `sub_68D50` afford-gate,
`sub_68DE0` commit — identical to every other cast, cast-path trace §1.5).
On the first tick it does:

```c
if (sub_4A810_get_0x35plus() > 6)          // EF:57888  CAPACITY GATE (see below)
{
    for (v6 = 0; v6 < 6; v6++)             // SIX spheres
    {
        v1x = _4A190(&caster.pos, 10, 57); // spawn a (10,57) sphere at the CASTER
        if (v1x) {
            v3 = clamp(4 * caster.actSpeed, 140, 280);           // EF:57897-57901
            caster.rng advance;
            v1x->actSpeed_0x82_130 = (rng & 0x7F) + v3;          // LAUNCH speed 140..407
            sub_68E50(caster, v1x, spellEntity);                 // muzzle/register
            v1x->parentId_0x28_40      = caster.id;              // OWNER = caster  EF:57905
            v1x->subSpellIndex_0x2A_42 = subspell[tier].subSpellIndex_2;  // DAMAGE payload  EF:57906
            v1x->byte_0x46_70          = subspell[tier].life_0x1A;        // RETALIATION TIER  EF:57907
            v1x->playerEntityIndex_0x94_148 = 0;                 // NEUTRAL = FAKE  EF:57908
            if (subspell[tier].life_0x1A >= 3) {                 // (only life>=3: make it look owned)
                v1x->playerEntityIndex = caster.id;
                SetManaSphereColorAndRot_36920(v1x);
            }
            caster.rng advance;
            v1x->yaw   = (caster.yaw - 85 + rng % 0xAA + handYawOffset) & 0x7FF;  // ±85 SPREAD  EF:57915-18
            v1x->pitch = handPitchOffset + caster.pitch;                          // EF:57919
        }
    }
    if (v1x) PrepareEventSound_6E450(v1x, -1, 11);   // cast sound 11  EF:57924
}
sub_68DE0(spellEntity, caster);            // commit mana
```

Key points:
- **Six** spheres, launched OUTWARD from the caster's hand in a **±85
  yaw cone** (~170 units of the 0x800 circle, a wide fan) at speed
  140..407. They arc/settle to the ground via the normal (10,57) tick
  (`sub_35FB0`, m57 trace §3), landing as a spread of ground "mana".
- **`playerEntityIndex = 0`** for tiers 0/1/2 (life < 3) → the sphere is
  **neutral/uncolored** — visually indistinguishable from genuine
  neutral ground mana. That is the "fool." (The sphere still gets a
  random mana value 0..1999 from its `sub_50130` ctor, so the sizes
  vary like real mana — reinforcing the disguise.)
- **`parentId = caster.id`** — this is what makes it a trap AGAINST
  OTHERS but harmless to the caster (the retaliation gate skips the
  owner, §3; and the possess probe requires `parentId != claimer`,
  possession-delivery trace §1 EF:3846).
- **`byte_0x46_70`** and **`subSpellIndex`** are the two fields the
  retaliation reads.
- **The capacity gate** `sub_4A810_get_0x35plus() > 6`: `get_0x35plus =
  D41A0_0.dword_0x35 + 1` (EF:33254-33256); `dword_0x35` is the
  per-step recycled-entity-slot counter (`Level.cpp:1281-1298`;
  `engine_support_converts.cpp:687` "entity counter"). So this is a
  **spawn-room guard** — "only fire the 6-sphere burst if ≥6 slots are
  free." It is NOT a player-count or difficulty gate. In the port it is
  effectively always-true under normal entity budgets (APPROX-safe to
  treat as a free-slot check or omit; flag if slot pressure ever
  matters).

## 2b. RETAIL — the retaliation (`sub_36680`, EF:26615)

The (10,57) tick `sub_35FB0` (m57 trace §3), each tick, checks whether
the sphere has been CLAIMED (`str_0x5E_94.word_0x68_104` = claimer id,
set by a possession pulse `sub_112D0` EF:4199 — possession-delivery
trace §3). When claimed, it calls `sub_36680`; if that returns true it
poofs (10,0) and despawns the sphere (EF:26362-26366):

```c
char sub_36680(a1x) {                       // EF:26615
  if (a1x->parentId == a1x->word_0x68_104) { // claimer IS the owner → no trap
      clear channel; return 0;               // (a wizard can't be fooled by its own)
  }
  v1 = a1x->byte_0x46_70;                    // the retaliation tier
  if (v1 == 0) {                             // TIER 0
      sub_36770(a1x);                        // → one fireball at claimer
      sub_6D8B0(parentId, 22, 1);            // caster earns fool's-mana XP
      return 1;                              // consume the sphere
  }
  else if (v1 == 1) {                        // TIER 1
      if (dword_0x10_16++ >= 8) { sub_6D8B0(parentId,22,1); return 1; }  // done after 8
      if (!(dword_0x10_16 & 1)) sub_36770(a1x);   // fireball every OTHER tick
      // returns 0 → sphere persists, keeps firing
  }
  else if (v1 <= 3) {                        // TIER 2/3
      if (dword_0x10_16++ == 0) { sub_36850(a1x); return 0; }  // one lightning, first tick
      if (dword_0x10_16 > 2) { sub_6D8B0(parentId,22,1); return 1; }  // despawn after
  }
  return 0;
}
```

`sub_36770` (EF:26672) — the **fireball** retaliation:
`_4A190(&pos, 9, 0)` = class-9 subtype **0** (fireball); sets
`word_0x96_150 = claimer` (homing lock onto the possessor),
`sub_655C0(fireball, claimer)` aims at it, `position.z += fov` (muzzle
lift), `subSpellIndex = sphere.subSpellIndex` (the payload), sound **9**;
water-landing → (10,5) splash + sound 27.

`sub_36850` (EF:26701) — the **lightning** retaliation:
`_4A190(&pos, 9, 9)` = class-9 subtype **9** (thunder bolt); sets impact
`byte_0x43_67=10 / byte_0x44_68=23` = **(10,23)**, `id = sphere.id`,
`word_0x96_150 = claimer`, `sub_655C0` aim at claimer, behavior row
`dword_0xA0_160 = &str_D7BD6[64]` (a homing row), `subSpellIndex =
payload`, and copies the victim's class/model into xtype/xsubtype.

So the retaliation projectile **hunts the possessor**, carrying the
tier's damage. XP (`sub_6D8B0(parentId, 22, 1)`) credits the CASTER when
the trap is sprung.

**OPEN-2 — RESOLVED 2026-08-03, and the answer is YES.** The SAME
`sub_36680` path runs for ANY (10,57) claim, including the authored
ground spheres from `sub_50130`, and their `byte_0x46_70` NewEvent
default IS 0 — so **every authored ground sphere is a live TIER-0 trap.**
Corpus proof (mc2l24, the level whose start spheres are ALL fool's mana —
player-reported and now pinned):

- t=0 census: 21 authored (10,57), slots 67-87, every one
  `own=0 pe=0 act=62 flags=0x2000c`, mana 34..1931, and raw
  **`b46 = 0`, `owner28 = 0`, `f2a = 100`** (the NewEvent
  `subSpellIndex` default), `scratch10 = 0`.
- All 21 die in t=0..1836, and **every single one dies by the trap**:
  the tick before each death the sphere's ch1 mail source
  (`word_0x68_104`) flips to 116 = the human, stamped by a co-located
  `(10,12)` possess pulse (`PossesHitMana_320E0` EF:22726, whose
  `sub_112D0` EF:4199 writes the latch); the next state shows the sphere
  with `flags |= 0x400` (`DisableEntityDrawing04`), life untouched at
  300/300, a **(10,0) poof at its exact position**, and a **(9,0)
  fireball with `word_0x96_150 = 116`** — homing the player. No sphere on
  this take is ever collected, damaged to death, or handed over.
- Per-sphere (slot → last m57 tick → poof slot / fireball slot):
  67→1322→569/589 · 68→1355→489/589 · 69→1358→622/402 ·
  70→1422→589/75 · 71→1402→75/622 · 72→406→539/627 · 73→854→524,618 ·
  74→294→524/599 · 75→786→430/524 · 76→1132→432/620 · 77→998→145/144 ·
  78→1452→75,179 · 79→1531→228/271 · 80→1649→280,340 · 81→1718→326/342 ·
  82→1700→345/363 · 83→1693→161/285 · 84→1573→155,322 · 85→1515→310/363 ·
  86→1835→469/478 · 87→959→609/73. (The four with two co-located poofs
  sit inside a busier neighbourhood; the poof-at-position and the
  0x400-with-full-life signature are unambiguous in all 21.)

The m57 trace's "collectible mana" reading was the gap: the sphere LOOKS
collectible (the AI-avoidance gate `word_0x244_580` even makes rivals
sometimes skip it), but any actual claim detonates it.

---

## 3. THE PORT (as landed 2026-08-03)

The trap is a property of the **(10,57) TICK**, not of the cast — which
is the whole point of OPEN-2's resolution. Two rounds:

**Round 1 (2026-08-02) — the cast.** `mc2_cast_fools_mana`
(`crates/mgc-sim/src/mc2/cast.rs`) throws the SIX-sphere ±85 yaw fan
modelled on `sub_6C870`, each sphere neutral (`f144 = 0`) with a random
disguise mana value, and `mc2_fools_retaliate` / `mc2_fools_bolt` port
`sub_36680` / `sub_36770` / `sub_36850`. Cast sound 11; XP moved off the
cast onto the trap's spend points.

**Round 2 (2026-08-03, session 7) — the tick, and the authored spheres.**
The round-1 gate was `is_fool = Mc2 && f52 != 0` — a CAST-DECOY marker.
Retail has no such gate, so the authored ground spheres (f52 = 0) fell
through to the (10,39) ball's ownership-transfer arm and were handed
over as legit mana. Landed:

- **`ball_tick` gate** (`crates/mgc-sim/src/mc1/combat.rs`) is now the
  (10,57) identity — `model65 == 57 || tick70 == 62` — and the arm is
  retail's shape: claim latch set → `mc2_fools_retaliate`; on `true`
  spawn the **(10,0) consume poof** (`mc2_spawn_fire`) and soft-kill with
  `flags |= 0x400` (EF:26363-65); either way the claimed sphere runs NO
  physics that tick (retail's `else if`).
- **The field homes are retail's, so the conformance importer already
  carries them**: parentId → `id24` (@0x28 fused), tier `byte_0x46_70` →
  `f71` (@0x46), payload `subSpellIndex_0x2A_42` → `f44` (@0x2A), counter
  `dword_0x10_16` → `f26` (@0x10), and the claim latch `word_0x68_104`
  IS the ch1 mail SOURCE (`mail[1].1`, @0x68) — never cleared except on
  the owner arm, exactly like retail. (Round 1 used port-private lanes
  f52/f50/f136/f146/f56, none of which the importer feeds; `f136` is the
  observed `mana_max` lane and `f146` is the balloon tether target, so
  they were actively harmful.)
- **Tier > 3** falls out with no trap and no transfer (EF:26665's
  fallthrough): the sphere freezes claimed forever.
- **Native discriminator.** `sub_50130` gives its sphere **action 0x3E**
  where every other sphere ctor takes 0x29; `mc2_spawn_mana_sphere(57, …)`
  now stamps `tick70 = 62` so a natively-spawned (10,57) is recognisable.
  It still carries the port's `model65 = 39` sphere-family model (the
  pre-existing simplification in `spawn_mana_ball`); the action is the
  load-bearing lane, and the model residual is listed in §6 as OPEN-6.
- **Census.** The world-mana denominator excludes CAST decoys only (an
  action-62 sphere whose `id24` is a real caster); authored ground
  spheres keep counting exactly as they did.

---

## 4. GAP (closed)

| Aspect | Retail | Port | Status |
|---|---|---|---|
| Sphere count | 6 | 6 | ✔ |
| Launch | ±85° fan, speed 140..407 | ±85 fan, short outward toss | ✔ shape (APPROX speed) |
| Owner (`parentId`) | caster | `id24` = caster | ✔ |
| Fake flag | `playerEntityIndex=0`, neutral | `f144 = 0` | ✔ |
| Payload | `subSpellIndex_2` on each | `f44` | ✔ |
| Retaliation tier | `byte_0x46_70 = life` | `f71` | ✔ |
| Possession → | detonate + despawn + (10,0) | detonate + despawn + (10,0) | ✔ |
| AUTHORED ground sphere | tier-0 trap for everyone | tier-0 trap for everyone | ✔ (2026-08-03) |
| L1 | 1 fireball | 1 fireball | ✔ |
| L2 | repeated fireballs | repeated fireballs | ✔ |
| L3 | lightning | lightning | ✔ |
| Caster XP | on trap sprung | on trap sprung | ✔ |
| Bolt launch z | `+= array_0x52_82.fov` | `+= f84` | ✔ (2026-08-03, session 8 — OPEN-5 CLOSED) |
| Sphere model | 57 | 57 on both paths | ✔ (2026-08-05, OPEN-6 CLOSED — §7) |
| Bolt aim at HUMAN claimer | `sub_655C0` at the claimer entity | ctx-pose aim (out-of-pool sentinel) | ✔ (2026-08-03, player-reported "wrong direction"; test `fools_trap_fireball_aims_at_the_human_claimer`) |

---

## 5. FIX DATA (concrete)

### 5.1 Cast side — replace the single-sphere spawn with the 6-sphere trap burst
In `mc2_spell_fire` arm `0x16` (cast.rs:805), model on `sub_6C870`:
- Gate on entity-slot room (APPROX-safe: proceed if ≥6 free slots, else
  treat as always-true; retail `sub_4A810_get_0x35plus() > 6`).
- Loop **6×**, each spawning a (10,57) sphere at the **muzzle** (reuse
  `self.muzzle(p, right)`), then set on the spawned entity:
  - `id24 = PLAYER_TARGET` (parentId = caster). **This is essential** —
    it is the owner-skip gate in the retaliation and the possess-probe
    filter.
  - `f-payload (subSpellIndex)` = `sub.sub_spell` (the tier's
    `subSpellIndex_2`) — port field is the sphere's `f140`? NO: `f140`
    is the mana value on a sphere. The retaliation reads the sphere's
    **subSpellIndex** (a distinct field); map it to whichever port field
    mirrors `subSpellIndex_0x2A_42` (the projectile payload field the
    launch block already uses as `e.f44`/`sub.sub_spell` — verify the
    sphere's field name; do NOT clobber `f140`=mana).
  - `byte_0x46_70` (retaliation tier) = `sub.life` — port field `f71`.
  - `playerEntityIndex (f144)` = 0 (neutral) unless `sub.life >= 3`.
  - launch: `actSpeed (f126)` = `clamp(4 * caster.speed, 140, 280) +
    (rng & 0x7F)`.
  - `yaw (f30)` = `(p.heading - 85 + rng % 0xAA + handOff) & 0x7FF`;
    `pitch (f32)` = `handPitch + p.pitch`.
- Play sound 11 once after the loop.
- Move the XP award OFF the cast and ONTO the trap-sprung path (§5.3),
  matching retail (`sub_6D8B0` fires inside `sub_36680`).

The mana value per sphere stays `rng % 0x7D0` (from the (10,57) ctor) —
keep it; it is the visual disguise.

### 5.2 Sphere tick — a (10,57)-specific claim consumer
The (10,57) sphere must NOT use the MC1 ball's ownership-transfer /
pool-credit claim intake. Port `sub_35FB0`'s claim branch
(m57 trace §3, EF:26362): when the sphere's possess-claim channel is set
(the port's `mail[1]`/claim field written by the possession pulse
`possess_flash_tick`), call the new `mc2_fools_retaliate` (below) and, if
it returns true, spawn a (10,0) poof and despawn the sphere. Only (10,57)
spheres carrying a nonzero trap identity need this; a (10,57) with the
default (authored ground sphere) behavior stays as-is pending OPEN-2.

### 5.3 The retaliation `mc2_fools_retaliate` (port `sub_36680`)
Reads the sphere's `id24` (parentId), claimer id, `f71` (tier), and a
per-sphere counter (map to a free field, retail `dword_0x10_16`):
- claimer == parentId → clear channel, no trap.
- tier 0 → fire one fireball; award caster XP (spell 22); return true.
- tier 1 → counter++; ≥8 → award XP, return true; else every other tick
  fire a fireball; return false.
- tier 2/3 → counter++; first tick fire lightning, return false; after
  counter>2 → award XP, return true.

**Fireball** (`sub_36770`): reuse the class-9 **subtype 0** creator
(`CREATORS[0]` in cast.rs, fireball, dispatch impact (10,0)) via
`mc2_spawn_cast_proj(0, spherePos)`. Set `id24 = parentId`, homing target
= claimer (the port's homing/`word_0x96_150` analogue), aim at claimer
(`sub_655C0` twin), `z += fov`, payload = sphere's subSpellIndex, sound
9; water → (10,5) splash + sound 27.

**Lightning** (`sub_36850`): reuse class-9 **subtype 9** (thunder bolt,
`CREATORS` row `(9,…,216)`) with impact **(10,23)** — identical to the
existing lightning-uncharged dispatch arm
(`mc2_dispatch_arm` spell 7 → `arm(9, (10,23))`, cast.rs:678). Set
`id24 = sphere.id`, homing target = claimer, aim at claimer, homing
behavior row 64, payload = sphere subSpellIndex, copy victim class/model
into the projectile's xtype/xsubtype.

### 5.4 Sounds
- Cast: **11** (already correct, keep — once after the burst).
- Fireball retaliation: **9**. Water splash: **27**.
- Lightning retaliation: none.

---

## 6. Confidence, open questions, suggested test

**Confidence: HIGH** on the mechanic. `sub_6C870` (cast, 6 spheres,
neutral, payload+tier) and `sub_36680` (tier-branched fireball / repeated
fireballs / lightning retaliation homing the possessor) are transcribed
verbatim and match the player's tier-by-tier report exactly, and the
mc2l24 corpus pins the authored half of the mechanic end to end (§2b).

**Open questions:**
1. **SPELLS.DAT row 22 `life_0x1A` per tier** — inferred {0,1,2} from the
   observed fireball/fireballs/lightning. Confirm from baked `spells.bin`
   row 22 (and whether tier 2 is life 2 or 3 — the retaliation branch
   `v1 <= 3` and the cast's `life >= 3` owned/colored path diverge only
   at exactly 3).
2. ~~**(10,57) `byte_0x46_70` NewEvent default**~~ — **RESOLVED
   2026-08-03: the default is 0 and the authored ground spheres DO
   retaliate.** The retaliation is universal to (10,57), not gated on
   "was cast by fool's mana". See §2b.
3. ~~**`subSpellIndex_0x2A_42` field mapping on the sphere**~~ —
   **RESOLVED: `f44`** (the port's uniform @0x2A home, `new_event`
   default 100 = retail's NewEvent default, corpus-confirmed `f2a=100`).
   Distinct from `f140` (mana) and from `f136` (the observed `mana_max`
   lane, which round 1 wrongly used).
4. **The `dword_0x35 > 6` capacity gate** — port equivalent (free-slot
   count) vs safe-omit; only matters under entity-budget pressure.
5. ~~**Bolt launch z (`position.z += array_0x52_82.fov`, EF:26688/26718)**~~
   — **LANDED 2026-08-03 (session 8, bolt-launch-lanes dig).** `a1x`
   there is the SPHERE, so the lift is the LAUNCHER's own box fov —
   the same law shape as the possession cast's `position.z +=
   a2x->array_0x52_82.fov` (EF:56054 / EF:55969) with the wizard as
   launcher. Ported as `e.z + e.f84` in `mc2_fools_bolt`. Worth ~42
   units on a full-size sphere (mc2l24 t=1322: sphere z=846 afov=42,
   retail fireball z=898).

   **The victim-probe half is settled, and it was never a probe
   filter.** `sub_10780` (EF:3739-71) has NO launcher exclusion at
   all — it filters on `byte[0]&8`, the xtype/xsubtype narrowing, and
   `a1x->id_0x1A_26 != v5x->id_0x1A_26`. What keeps retail's bolt off
   its own sphere is two things neither of which the port has:
   (a) the sprung tier-0 sphere is UNMAPPED and class-zeroed **inside
   its own tick** — the entity walk runs `sub_57F20` (Events.cpp:551;
   body :5209 = `SetMapEntity_57E50` + `class = 0` + free-stack push)
   the instant `DisableEntityDrawing04_57F10` latches `byte[1] & 4`,
   so no later entity can see it; and (b) retail probes exactly ONCE,
   at the END of a full 384-unit step (`sub_65C20` EF:63126-29:
   MoveEntity → CopyEntityPosition → `sub_10780`). Our soft kill
   (`flags |= 0x400`) leaves the sphere linked until the tick-top
   reap, and our anti-tunnel chord march probes 128-unit sub-steps
   retail never visits. The exclusion therefore has to come from the
   OWNER gate, and it already does: `mc2_fools_bolt` stamps
   `id24 = sphere.id24`, and `victim_scan`'s `c.id24 != id` (the port's
   twin of EF:3769) drops the launcher for both an authored sphere
   (id24 = its own slot, the NewEvent default on both sides) and a
   cast decoy (id24 = the caster). Pinned, with the contrast arm, by
   `fools_trap_bolt_leaves_from_the_sphere_box_top_and_clears_its_own_muzzle`
   (world.rs) — a same-muzzle bolt with a FOREIGN owner still
   detonates on the sphere, which is the port's chord-march residual
   and is listed as OPEN-7.
6. ~~**Sphere model on native spawns**~~ — **LANDED 2026-08-05
   (session 9).** `mc2_spawn_mana_sphere` now puts retail's model back
   (`model65 = 57`, `mc2/effects.rs`); the action-62 stamp stays as
   belt-and-braces. The audit of every `model65 == 39` sphere gate is
   §7 below — retail's own laws split cleanly, and the port was on the
   wrong side of four of them.
7. ~~**Chord-march muzzle admission**~~ — **LANDED 2026-08-05
   (session 9), and it was NOT latent.** See §8.

---

## 7. THE SPHERE-MODEL GATE AUDIT (OPEN-6, 2026-08-05)

The organising fact is retail's class-10 scan chain `dword_38523`,
built at EF:40023-40062 from models **39, 40 AND 57**. Every sphere law
is either "walk the chain" (m57 included) or "walk the chain and test
`model == 39`" (m57 excluded) — and the census is a third thing, a
model switch that drops 57 outright.

| Law | Retail | m57? | Port |
|---|---|---|---|
| awake pass `sub_68BF0` | chain, no model test (EF:55489) | **YES** | `mc2_awake_pass` — 57 ADDED |
| mana-magnet aura | chain, no model test (EF:28362) | **YES** | `mc2_aura_tick` — 57 ADDED |
| Vissuluth ritual broadcasts | chain, no model test (EF:12848/13049) | **YES** | already `39\|40\|57` |
| rival mana hunt `sub_148E0` | chain; 39/40 first, then 57 under a Perception break (EF:6544-49) | **YES** | already two-pass; the native m57 was in the WRONG pass |
| m23 siphon find/validate | `model == 39` (EF:18396) | no | unchanged |
| balloon fleet target | `model == 39` (EF:61011) | no | unchanged |
| castle absorb | `model == 39` (EF:61105) | no | **native m57 used to be absorbed** |
| dead-wizard sphere re-point | `model == 39` (EF:60174) | no | unchanged |
| ball merge | `+66/+67` = (10,39) from the ball ctor | no | unchanged (`is_fool` also short-circuits) |
| world-mana census `sub_61F50` | model switch: 39 and 58 count, 45 banks, **everything else falls through** (EF:62012-35) | **NO** | census special-case DELETED — the model list is the filter |
| possess whitelist `sub_108B0` | `(10,57)` arm gated on `parentId_0x28 != caster` (EF:3846) | own only | **lane fixed**: `f40` (@0x26) → `id24` (@0x28) |
| map dot | fool mana must look like real mana | — | `(10, 39 \| 57)` share the arm |

Two of these were live port bugs the model stamp exposes:

- **Castle absorb.** A native fool's sphere overlapping a castle was
  eaten as real mana. Retail's `while` filters `model != 39` and moves
  on (EF:61105).
- **The census.** The port kept authored ground spheres in the
  world-mana denominator and dropped only cast decoys. Retail drops the
  whole model — the type-0 castle-share objective never sees a (10,57)
  at all. The old reasoning ("an uncollectable share dilutes the goal")
  was right; it just did not go far enough.

And one lane bug the stamp exposes by re-routing native spheres onto
retail's own `(10,57)` claim arm: that arm read `f40` (retail `@0x26`)
where retail reads `parentId_0x28`, whose port home is `id24`. Harmless
while only IMPORTED spheres reached it (both lanes read 0), fatal for a
CAST decoy — the caster's own possess bolt would have detonated on his
own trap. Fixed in `claim_admits` (`mc1/combat.rs`).

Corpus: **zero movement** on mc2l0 0+2000 and mc2l24 51500+600
(byte-identical), which is the expected result — `verify-deltas`
rebuilds entities from the recording, where an m57 already carried
model 57, so only NATIVE play is affected. Tests: the three fool's-mana
channel tests now count `(10,57)`, and
`mc2_authored_ground_sphere_is_a_tier0_trap` asserts the native model
directly.

---

## 8. MUZZLE ADMISSION (OPEN-7, 2026-08-05) — the latent class was live

Retail's flight states probe the victim test ONCE, at the END of a full
step (`sub_65C20` EF:63126-29: MoveEntity → CopyEntityPosition →
`sub_10780`). The port marches the chord in ≤128-unit sub-steps and
probed every one from the muzzle out.

**The law, as landed** (`mc2/proj.rs`, `mc2_flyer_tick` + the
`mc2_hit_covers` helper): a victim whose box already contains the
step's START is admitted only at `k == n` — retail's own probe point.
Anything the projectile ENTERS mid-chord still detonates at the
sub-step, so the anti-tunnelling the march exists for is untouched.

This was chosen over "skip `k == 1`" precisely because a PARKED
projectile's only probe IS the endpoint, and retail detonates that one.
Hence the pinned test's contrast arm
(`fools_trap_bolt_leaves_from_the_sphere_box_top_and_clears_its_own_muzzle`)
keeps its assertion: it pins retail's endpoint law, not the residual.
The residual gets its own test,
`a_projectile_born_inside_a_foreign_box_flies_clear_of_its_muzzle`
(world.rs) — a foreign-owned bolt born inside a fat sphere at speed 130
(sub-steps 65 / 130) now clears, while a bolt ENTERING the same sphere
mid-chord still detonates. Non-vacuity: `MGC_NO_MUZZLE_ADMISSION=1`
restores the old march and the first arm fails.

**The audit's "nothing is broken today" was wrong.** The corpus
exercises it. mc2l0 t=618 slot 165, a (9,1) possession bolt: the port
detonated it at its own muzzle (life 2 vs retail 3, position frozen at
82/180/3616 instead of retail's 84.57/181.94/3335) and spawned a
phantom (10,12) possess flash at slot 123. All five diff rows vanish,
none appear; mc2l0 0+2000 goes 1703 → **1704 conforming**. On mc2l24
51500+600 a (9,3) at t=51500 stops self-detonating (slot 720 life/x/y/z
now retail's) and three phantom (10,0) impact puffs go away; pair
verdicts are unchanged and the row detail reshuffles by the free-list
(±8 field rows) in an epoch already dominated by slot desync.

**Tests** (`crates/mgc-sim/tests/mc2_spell_channels.rs`):
`mc2_fools_mana_throws_six_decoys_that_trap_the_possessor` (six spheres,
tier-0 spring = one fireball, decoy consumed),
`mc2_fools_mana_tier2_retaliates_with_lightning`,
`mc2_fools_mana_decoys_do_not_count_toward_world_mana`, and
`mc2_authored_ground_sphere_is_a_tier0_trap` — an AUTHORED ownerless
sphere: owner reclaim is a no-op, a non-owner claim fires exactly one
fireball, leaves the (10,0) poof on the sphere's tile, and consumes the
sphere. Non-vacuity proven: restoring the `f52 != 0` gate fails the last
one with 0 fireballs.

Plus, in `crates/mgc-sim/src/engine/world.rs` lib tests (2026-08-03,
OPEN-5): `fools_trap_bolt_leaves_from_the_sphere_box_top_and_clears_its_own_muzzle`
— the bolt's launch z is the sphere's `z + f84`, and a PARKED bolt
(speed 0, so the chord march probes exactly the launch point) survives
its first flight tick while a second parked bolt at the same muzzle with
a FOREIGN owner is consumed by the same sphere on the same tick.
Non-vacuity: removing the lift fails the z assert; the contrast arm is
the probe half's own control.
