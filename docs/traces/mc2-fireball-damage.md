# MC2 PLAYER FIREBALL DAMAGE — Verbatim Trace + Port Delta

All decompile citations `file:line` in `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/`.
EF = `EventsFunctions.cpp`. Port cites are `crates/mgc-sim/…`.

Trace date 2026-07-11. Purpose: settle PLAYTEST-CALIBRATION — retail kills a SETTLER in
**5** tier-0 fireballs and a GOAT in **2**; our port takes **11** and **7**. The delivered
damage is ~40% of retail. This trace pins WHY, to the exact field/line.

Companion traces (read, not re-derived): `mc2-player-cast-path.md` (the cast dispatch),
`mc2-projectile-terrain-water.md` (the fireball detonation control-flow), `mc2-spell-xp.md`
(`SetSpell`, the SPELLS.DAT layout), `mc2-class10-m6-m9-m11-m28-m31.md` (the class-10
area-damage helpers `sub_10C80`/`sub_11400`).

---

## TL;DR — THE RETAIL LAWS (each proven below with verbatim lines)

1. **A tier-0 fireball carries exactly ONE damage number: `subSpellIndex_0x2A_42 = 250`**
   (SPELLS.DAT row 0, tier 0, `subSpellIndex_2 = 250`). The chain is:
   `SPELLS.DAT[0].tier0.subSpellIndex_2 (=250)` → (via `SetSpell_6D5E0`) the fireball spell
   **manifestation**'s `subSpellIndex_0x2A_42` → (EF:55864) the **projectile**'s
   `subSpellIndex_0x2A_42` → (EF:63191) the spawned **(10,0) fire** effect's
   `subSpellIndex_0x2A_42` → (EF:22719) `sub_10C80(fire, 0, 250)` area write.

2. **There is NO direct damage write to the struck victim at impact.** The victim path in
   `sub_65C20` (EF:63160-63173) only calls `sub_65580`/`sub_655A0` (z half-height nudges,
   EF:62750/62761) and `sub_65780` (accuracy stat counters, EF:62836). ALL fireball damage
   is delivered by the spawned **(10,0) ground-fire** effect at the victim's position.

3. **The (10,0) fire deals its `subSpellIndex` ONCE, as a channel-0 area write** — not
   per-tick. `sub_30D50` (EF:22692) gates the damage on `byte[0] & 2`, sets that bit on the
   first active tick and never clears it (EF:22713/22746). One-shot, radius = the fire's
   `array_0x52_82.pitch` (from `SetEntityShiftRot(128,128)`), the standard `sub_10C80`
   filter+falloff (none — flat inside radius). So a tier-0 fireball = **one 250-damage
   application** to the victim.

4. **The mailbox/health law is a STRAIGHT SUBTRACTION on channel 0, no scaling.**
   `sub_10C80` writes `victim.str_0x5E_94.dword_0x5E_94 += 250` and stamps the attacker id
   (EF:4141-4147). The creature inbox drain `sub_1BD90` then does
   `life_0x8 -= dword_0x5E_94` (EF:8966) with **no armor / no shift / no per-class modifier**;
   death when `life_0x8 < 0` (EF:8993). Fire uses **channel 0** (`byte_0x38_56 & 1`).

5. **Victim HP (ctors, verbatim):** SETTLER = the Townie/Villager (5,13) `AddVilliger_4BF40`,
   **`maxLife = 1000`** (EF:34052), `byte_0x38_56 = 1` (EF:34057). GOAT = (5,1)
   `AddCreature_4B490`, **`maxLife = 600`** (EF:33730), `byte_0x38_56 = 1` (EF:33738). Both
   copy maxLife→life once at spawn (`CopyMaxLifeToLife_49A20`, EF:54120) and **do NOT
   regenerate**. The GOAT's `dword_0xA0_160x = &str_D7BD6[98]` (EF:33739) = the "row 98" param.

6. **Arithmetic (250/hit, death at life<0):**
   - Settler 1000 HP: after 4 hits life = 0 (alive); 5th hit → −250 dead ⇒ **5 fireballs.** ✓
   - Goat 600 HP: after 2 hits life = 100 (alive); 3rd → −150 dead ⇒ **3 fireballs** by this
     single-application law. Retail is reported as **2** — see §6 (a small goat-only
     discrepancy that does NOT change the port fix; the SETTLER number is exact and both
     port numbers are exact — see PORT DELTA).

7. **Charged tier 2 (life_0x1A ≥ 2) is a DIFFERENT law** — subtype 28 / impact **(10,76)**
   fire-spheres, not (10,0) — out of scope here (§7). The tier-0 law above is (10,0)-only.

**THE PORT DEFICIT (one line):** the port's fireball arm passes `payload: false`
(`cast.rs:626`), so `mc2_launch` never copies the tier's `subSpellIndex_2 (250)` onto the
projectile's `f44` (`cast.rs:833-834`). The projectile keeps `new_event`'s default
**`f44 = 100`** (`features.rs:856`), and the (10,0) fire is handed 100 instead of 250. 100
vs 250 = 0.40 — exactly the observed 11-vs-5 / 7-vs-3 ratio.

---

## 1. Q1 — the carried damage payload (the field chain)

### 1.1 SPELLS.DAT row 0, tier 0 (baked, verified)
Dumped from every baked bundle (`baked/assets/mc2-*/spells.bin`, 26×80 bytes):

```
row0 byte0=3  tier0: sub=250 mana=100 maxmana=0      xp1=0     hint=186 w18=5  life=0
row0 byte0=3  tier1: sub=160 mana=250 maxmana=0      xp1=400   hint=187 w18=11 life=1
row0 byte0=3  tier2: sub=180 mana=2500 maxmana=160000 xp1=12000 hint=188 w18=15 life=2
```

So **tier-0 fireball `subSpellIndex_2 = 250`** (the task's believed value — CONFIRMED). (Note
tiers 1/2 carry 160/180 — the *base* number DROPS at higher tiers because the higher tiers
change shape: tier2 `life=2` switches the projectile to the charged (10,76) body, §7.)

### 1.2 SPELLS row → manifestation — `SetSpell_6D5E0` (L:1505, cited in mc2-spell-xp.md §3.1)
`entity->subSpellIndex_0x2A_42 = SPELLS_BEGIN_BUFFER_str[model].subspell[tier].subSpellIndex_2;`
For the fireball manifestation (model 0) at tier 0 this stamps `subSpellIndex_0x2A_42 = 250`.

### 1.3 manifestation → projectile — the fireball effect state `sub_693F0` (EF:55832)
The first-tick spawn (EF:55848-55880) calls `sub_6DCA0(caster, &pos, 0, &subspell[tier], …)`
then copies fields onto the returned projectile `v6x`:
```c
v6x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42;   // EF:55864  ← 250 from the manifestation
```
(`a1x` = the class-15 fireball manifestation.) Note the a3=0 arm of `sub_6DCA0` (EF:44080-93)
does **NOT** itself set the projectile subSpellIndex for the fireball — it only picks the
subtype (0 uncharged / 28 charged) and the impact `(byte_0x43_67, byte_0x44_68) = (10, 0)` /
`(10, 76)`. The 250 rides in solely via EF:55864.

So the tier-0 fireball projectile flies with **`subSpellIndex_0x2A_42 = 250`** and impact
effect **(10, 0)**.

### 1.4 projectile → impact effect — `sub_65C20` (EF:63183-63191)
On detonation (victim OR dry terrain), `LABEL_35` spawns the impact effect and copies the
payload onto it:
```c
v18x = IfSubtypeCallCreatingManaSphere_4A190(&pos, a1x->byte_0x43_67, a1x->byte_0x44_68);  // (10,0) fire
if (v18x) {
    …
    v18x->subSpellIndex_0x2A_42 = a1x->subSpellIndex_0x2A_42;   // EF:63191  ← 250 → the fire
    v15x->id_0x1A_26 = a1x->id_0x1A_26;                         // owner
    …
}
```
This OVERWRITES the (10,0) ctor's default `subSpellIndex = 400` (EF:35341) with the
projectile's **250**. So the fire that actually burns the victim carries **250**, not 400.

---

## 2. Q2 — the impact (victim path) + the (10,0) fire tick

### 2.1 (a) NO direct damage at impact — the victim block, verbatim (EF:63160-63173)
```c
if (v8x->struct_byte_0xc_12_15.word[0] & 0x8010) {           // shielded target → ricochet
    if (sub_68740(a1x, v8x, 0x5Bu, 45)) return 0;
} else {
    if (v8x->dword_0xA0_160x->byte_160_0x20_32 & 0x10)
        a1x->subSpellIndex_0x2A_42 = 1;                      // (rare) "fragile" flag drops payload to 1
    sub_65580(v8x);                                          // EF:62750: z += half-height (nudge only)
    CopyEntityPosition_57CF0(a1x, &v9x->position_0x4C_76);   // snap burst to victim
    sub_655A0(v9x);                                          // EF:62761: z -= half-height (undo nudge)
    v20 = 1;                                                 // detonate flag
}
```
`sub_65580`/`sub_655A0` are pure z position adjusters (`if model!=2: z ±= array_0x52_82.yaw`,
EF:62755-62767). `sub_65780` (EF:62836, called at LABEL_35 EF:63186) is the accuracy-stat
bookkeeper (`dword_0x165_357++` shots-hit, `dword_0x169_361++`, EF:62867-62874). **None of
these touch the victim's `str_0x5E_94` mailbox or `life_0x8`.** There is **no direct-hit
damage write** — the entire damage is the spawned (10,0) fire (EF:63183/63191).

> One niche modifier (EF:63167-63168): if the victim's param struct has
> `byte_160_0x20_32 & 0x10`, the projectile's payload is FORCED to 1 (a near-immune flag).
> Neither the goat (str_D7BD6[98]) nor the settler (str_D7BD6[100]) is known to set it; flag
> as CONFIRM-if-goat-anomaly (§6).

### 2.2 (b) the (10,0) fire tick `sub_30D50` — ONE-SHOT damage (EF:22692), verbatim spine
```c
void sub_30D50(type_entity_0x6E8E* a1x) {
    if (a1x->dword_0x10_16 & 3) { a1x->dword_0x10_16--; }               // optional fuse
    else {
        a1x->life_0x8--;
        if (a1x->life_0x8 >= -1) {
            a1x->struct_byte_0xc_12_15.byte[0] &= 0xFEu;                // clears bit0 (NOT bit1)
            v3 = getTerrainAlt_10C40(&pos);
            if (!(a1x->struct_byte_0xc_12_15.byte[0] & 2)) {            // ← damage GATE (bit1)
                …
                if (!(a1x->struct_byte_0xc_12_15.byte[2] & 1))
                    sub_10C80(a1x, 0, a1x->subSpellIndex_0x2A_42);      // EF:22719  AREA DAMAGE, ch0, =250
                … terrain scorch/paint …
                a1x->struct_byte_0xc_12_15.byte[0] |= 2u;              // EF:22746  SET bit1 → no more damage
                … flicker rand, sound 3 …
            }
            sub_580E0(&pos, v3, 0, 0, word_0x2C_44);                   // z flicker above ground
            … cave ceiling clamp … sub_585A0 (frame advance) …
        } else DisableEntityDrawing04_57F10(a1x);                       // despawn at life < -1
    }
}
```
- **One-shot, not per-tick.** The damage `sub_10C80(a1x, 0, subSpellIndex)` fires only while
  `byte[0] & 2 == 0`; that bit is set the same tick (EF:22746) and only bit0 (not bit1) is
  ever cleared afterward (EF:22711). So over the fire's `maxLife = 8` ticks the victim is
  written **exactly once**.
- **Radius / falloff:** the write covers the box radius `array_0x52_82.pitch` (the fire's
  `SetEntityShiftRot_49EA0(128,128)`, EF:35348 → pitch ≈ 128 → r ≈ 0..1 tile). **No falloff**
  — inside the radius every eligible victim gets the full `a3` (§3). The victim, being at the
  burst center, is always inside.
- **Damage carrier:** `subSpellIndex_0x2A_42` = 250 (overwritten from the projectile,
  EF:63191). The (10,0) ctor's own default is 400 (EF:35341); it is only used when the fire
  is spawned by something that does NOT overwrite it (e.g. the big-explosion cluster).

---

## 3. Q3 — the mailbox / health law (channel, scaling, death)

### 3.1 The write — `sub_10C80(a1x, 0, a3)` (EF:3953), channel-0 arm
For `a2 == 0` the map-cell walk (EF:4120-4151) selects victims and writes the mailbox:
```c
if (a1x->id_0x1A_26 != v8x->id_0x1A_26
    && (v8x->class_0x3F_63 != 3 || v8x->model_0x40_64 != 2)     // not player-body-2
    && (1<<a2) & v8x->byte_0x38_56                               // channel-0 enrolled  (goat/settler: byte=1)
    && v8x->struct_byte_0xc_12_15.byte[0] & 8                    // targetable
    && (v8x->class_0x3F_63 != 10 || v8x->model_0x40_64 != 45)    // not a building
    && sub_106C0(a1x, v8x)                                       // LOS / range
    && xtype/xsubtype filter passes)
{
    if (v8x->str_0x5E_94.word_0x62_98) v8x->str_0x5E_94.dword_0x5E_94 += a3;   // EF:4142  ACCUMULATE
    else                               v8x->str_0x5E_94.dword_0x5E_94  = a3;   // EF:4144
    v8x->str_0x5E_94.word_0x62_98 = a1x->id_0x1A_26;                           // stamp attacker
}
```
- **Channel = 0** for fire (`sub_10C80(…, 0, …)`). Eligibility needs `byte_0x38_56 & 1`
  (set = 1 by both ctors) + targetable bit 8. **No per-class damage modifier here** except the
  building/tree tenth handled by OTHER helpers (`sub_11400` reduces class-2 model-0 to a3/10;
  irrelevant to goats/settlers).
- The mailbox **accumulates** (`+=`) across multiple hits within one drain window.

### 3.2 The drain — `sub_1BD90` (EF:8944), the creature inbox
```c
if (a1x->str_0x5E_94.word_0x62_98) {
    a1x->life_0x8 -= a1x->str_0x5E_94.dword_0x5E_94;   // EF:8966  STRAIGHT SUBTRACTION, no scale
    a1x->str_0x5E_94.word_0x62_98 = 0;                 // consume
    …
}
…
if (a1x->life_0x8 < 0) { v2 = 2; … }                    // EF:8993  DEATH when life < 0
```
**No armor, no shift, no channel mask on the drain, no per-class factor.** `life -= damage`,
die at `life < 0`. This is the same law the melee/other mailbox drains use (EF:9097, 9259,
9390 …). Fire → channel 0 → this straight subtraction.

---

## 4. Q4 — victim HP ctors (verbatim)

### 4.1 SETTLER = Villager/Townie (5,13) — `AddVilliger_4BF40` (EF:34037)
```c
event->class_0x3F_63 = 0x05;  event->model_0x40_64 = 0xD;
event->maxLife_0x4 = 1000;                      // EF:34052   ← HP
event->byte_0x38_56 = 1;                        // EF:34057   ← channel-0 enrolled, targetable
event->dword_0xA0_160x = &str_D7BD6[100];       // EF:34058   param row 100
… CopyMaxLifeToLife_49A20(event);               // EF:34064   life = 1000
```
**Settler maxLife = 1000.** No regen (nothing writes life upward in its action states; the
goat/villager handlers only decrement via the mailbox).

### 4.2 GOAT (5,1) — `AddCreature_4B490` (EF:33720)
```c
event->class_0x3F_63 = 5;  event->model_0x40_64 = 1;
event->maxLife_0x4 = 600;                        // EF:33730  ← HP
event->byte_0x38_56 = 1;                         // EF:33738  ← channel-0 enrolled, targetable
event->dword_0xA0_160x = &str_D7BD6[98];         // EF:33739  ← the task's "row 98"
event->xtype_0x41_65 = 3;                        // EF:33742
… CopyMaxLifeToLife_49A20(event);                // EF:33744  life = 600
```
**Goat maxLife = 600.** No regen.

Both match the port (`mc2_spawn_goat` max_life 600, `mc2_spawn_villager` max_life 1000 —
`mobs.rs:860/945`).

---

## 5. Q5 — the arithmetic

Death at `life < 0`, flat 250 per fireball (§1-§3), no regen (§4):

| victim | HP | hits to reach ≤0 (life after n = HP − 250n) | fireballs to KILL (first n with HP−250n < 0) |
|---|---|---|---|
| Settler | 1000 | n=4 → 0 | **5** ✓ retail |
| Goat | 600 | n=2 → 100, n=3 → −150 | **3** (retail reports 2 — §6) |

The **settler is exact** (5). The goat computes to 3 under the single-application law; the
retail-observed 2 is one fewer — this small goat-only gap is discussed in §6 and does **not**
alter the port fix (the port fix restores 250; the goat then dies in 3 in our port too, vs
the current 7).

**Port arithmetic (delivered 100 — see PORT DELTA):** Settler 1000: n=10 → 0, n=11 → −100 ⇒
**11**. Goat 600: n=6 → 0, n=7 → −100 ⇒ **7**. Both match the observed port numbers exactly,
proving the delivered amount is 100.

---

## 6. The goat 3-vs-2 residue (secondary; NOT the port bug)

Under the traced single-application law the goat needs 3 fireballs; retail is reported as 2.
Candidate explanations, none of which change the "restore 250" fix (all would make the port
MORE lethal, converging toward retail once 250 is delivered):

1. **A second overlapping (10,0) application.** The impact snaps the burst to the victim
   (EF:63170) and the fire's radius is ~1 tile; if the goat's own extents place it inside a
   neighbouring authored/standing fire, or if the shot's own trailing sub-effects land a
   second (10,0) that also catches it, the goat eats 500 in one salvo → 2 kills. The fireball
   projectile itself lays no trail at tier 0 (only the meteor `sub_66180` does), so this would
   have to come from terrain/adjacent fire — level-specific.
2. **`sub_106C0` LOS letting the fire hit the goat from an adjacent burst tile twice** — the
   3×3 cell walk in `sub_10C80` visits the goat's cell once per fire; only multiple fires
   double it.
3. **Playtest counting** (a grazing shot + a direct = "2") — the settler count (5) is the
   reliable anchor and is exact at 250.
4. **The `byte_160_0x20_32` payload override (EF:63167)** does the OPPOSITE (drops to 1), so
   it is not the cause; but CONFIRM neither goat nor settler sets it.

RECOMMENDATION: land the 250 fix (below), re-playtest; if the goat still reads 3 vs retail 2,
open a focused trace on the goat-tile fire overlap. This residue is **not load-bearing** for
the headline deficit.

---

## 7. Q6 — tier-2 charged fireball (out of scope, noted)

Tier 2 has `life_0x1A = 2` → `sub_6DCA0` a3=0 arm spawns subtype **28** with impact
**(10, 76)** fire-spheres, not (10, 0) (EF:44080-44091; `byte_0x44_68 = 76`). Its damage law
is the (10,76) sphere family (`docs/traces/mc2-class10-m76-fire-spheres.md`), a
different carrier — NOT the (10,0) one-shot above. The tier-0 law traced here is
(10,0)-specific. Tier 1 (`life_0x1A = 1`) still uses subtype 0 / (10,0) with payload 160
(a "repeat fireball", cadence flag), so the SAME (10,0) law applies to tier 1 with 160.

---

## PORT DELTA — where our delivered damage diverges (exact file:line)

### DELTA 1 (THE deficit): the tier-0 fireball payload is never armed
`crates/mgc-sim/src/mc2/cast.rs:626`
```rust
0 => arm(0, (10, 0), false, false),
//                    ^^^^^ payload = false
```
`mc2_launch` only copies the tier's `subSpellIndex_2` onto the projectile's `f44` **when
`arm.payload` is true** (`cast.rs:833-834`):
```rust
if arm.payload {
    e.f44 = sub.sub_spell.clamp(0, u16::MAX as i32) as u16;   // 250 — SKIPPED for fireball
}
```
With `payload:false`, the fireball projectile keeps `new_event`'s default
**`f44 = 100`** (`crates/mgc-sim/src/mc1/features.rs:856` — the retail `NewEvent_4A050`
subSpellIndex=100). At impact `mc2_proj_impact` reads that f44 and hands the (10,0) fire
`e.f140 = dmg (=100)` (`crates/mgc-sim/src/mc2/proj.rs:336` reads `dmg = e.f44`,
`proj.rs:366` writes `e.f140 = dmg`). The fire tick then applies 100
(`crates/mgc-sim/src/mc2/mobs.rs:1630-1631`). **Result: 100 delivered, retail 250.**

WHY `payload:false` is wrong even though retail's `sub_6DCA0` a3=0 arm doesn't set the
projectile subSpellIndex: retail arms the 250 one line later in the EFFECT STATE
(EF:55864, `v6x->subSpellIndex = a1x->subSpellIndex`), which our port folded into the launch
helper for the OTHER spells but omitted for the fireball. The `payload` flag conflates "does
`sub_6DCA0` set it" with "does the projectile carry it" — the fireball answers no/yes.

**FIX (either is faithful):**
- (preferred, matches EF:55864) In `mc2_launch`, unconditionally set
  `e.f44 = sub.sub_spell` for the projectile-band spells (drop the `payload` gate for the
  amount — every `sub_6DCA0` spell's projectile carries the tier subSpellIndex; the tier-1
  fireball then correctly carries 160), OR
- (minimal) flip the fireball arm to `arm(0, (10, 0), true, false)` at `cast.rs:626` and the
  charged arm `arm(28, (10, 76), true, false)` at `cast.rs:625` so the 250/160/180 payload
  rides along. Verify tiers 1/2 land 160/180 and the (10,76) path consumes it.

### DELTA 2 (verify, not a deficit): the (10,0) fire default vs the override
The port's `mc2_spawn_fire` defaults `f140 = 400` (`mobs.rs:1569`, faithful to EF:35341), but
`mc2_proj_impact` OVERWRITES it with the projectile's `dmg` (`proj.rs:366`) — faithful to
EF:63191. So once DELTA 1 is fixed the fire correctly carries 250. **No change needed here**,
but note the override is load-bearing: if a future refactor drops the `e.f140 = dmg` line the
fire would silently fall back to 400 (too high). Keep the override.

### NON-DELTAS (confirmed faithful)
- **One-shot fire, ch0, flat, no falloff:** `mc2_fire_tick` gates damage on the `flags & 2`
  latch (`mobs.rs:1627`, ≡ retail `byte[0] & 2`), writes once via `area_write(i, 0, amt, …)`.
  Faithful to `sub_30D50`.
- **Straight-subtraction health law, death at life<0:** the port's `mail_write` accumulates
  and the combat drain subtracts flat (no armor) — matches `sub_1BD90`. (Filter, targetable
  bit, building-tenth all mirrored in `area_write`, `combat.rs:130-199`.)
- **No direct-hit damage write:** the port's impact spawns the fire and does not separately
  damage the victim — matches EF:63160-63191 (the retail victim block is stats + z-nudge only).
- **Victim HP:** goat 600 / settler 1000 — match EF:33730 / EF:34052.

**NET:** the single change of arming `subSpellIndex_2 (250)` onto the tier-0 fireball
projectile restores retail damage (settler 11→5, goat 7→3). No other divergence contributes.
The goat 3-vs-2 residue (§6) is a separate, smaller, level-dependent question that the fix
does not depend on.

---

## OPEN / uncertain
- **Goat 3 vs retail 2** (§6): needs a focused re-playtest after the 250 fix; likely a
  fire-overlap/second-application effect, not the payload path.
- **`byte_160_0x20_32 & 0x10` payload→1 override** (EF:63167): confirm neither str_D7BD6[98]
  (goat) nor [100] (settler) sets it (it would make them near-immune, contradicting playtest,
  so almost certainly clear — but not dumped here).
- **Tier 1/2 amounts (160/180) and the (10,76) charged path** (§7): verify they land after
  the fix; the (10,76) sphere family has its own trace.
