# MC2 POSSESSION — cast → projectile → claim delivery, end to end — Verbatim Trace Report

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, EV = `Events.cpp`, GameUI = `GameUI.cpp`). Port citations to `/home/rain/projects/mgcarpet/crates/`. Trace date 2026-07-10. Companion docs: `mc2-class9-spell-projectiles.md` (flight law, §State 0x01), `mc2-class10-m29-m5-m13.md` (dispatch-table anchors).

**Headline findings:**
1. **Possession is delivered by mail, not by the hit.** The projectile's impact spawns a separate class-10 "claim pulse" entity — (10,12) normal / (10,70) steal — whose per-tick action broadcasts the claim channel (`str_0x5E_94.word_0x68_104` = claimer id, `dword_0x64_100` = force flag) via `sub_112D0` (EF:4162) to every overlapping entity with **`byte_0x38_56 & 2`**. The building and the mana ball then each CONSUME that channel in their own tick (house: EF:28016-42; ball: EF:26069-94). A direct hit is NOT required — a pulse landing within box overlap (512-unit extents, ±2-tile cell scan) claims the building.
2. **The divergence in our port is one missing field.** The port's shared area writer (`area_write`, mc1/combat.rs:126) gates channel delivery on **`f28 & (1 << ch)`** (the cross-column damage contract). `spawn_mana_ball` sets `f28 = 3` (ch0+ch1) → balls claim. `mc2_spawn_building` faithfully mirrors retail's `byte_0x38_56` into `f56` (33, then `|= 2`) but **never sets `f28`** (default 0) → the ch1 claim mail is dropped at the gate, and `mc2_house_tick`'s claim intake (mc2/mobs.rs:2023) is never fed. Fix = extend the contract: `f28 = 1` in the ctor, `f28 |= 2` alongside `f56 |= 2`.
3. **Stone templates are excluded twice in retail:** the possession projectile's victim probe skips them (`bldgprm.byte_2 & 8` → fly through, EF:3853), AND they never receive `byte_0x38_56 |= 2` (EF:32799-802) so the broadcast can't reach them either.

---

## 0. Cast side — the class-15 model-3 spell-hand machine

Possession is not cast through the `sub_6DCA0` player-spell dispatcher. The equipped-possession entity is **class 15 model 3** (table `x_DWORD_D4C52ar_strF0` EF:1949, row 0x0003 → `0x0024A640` at EF:1953) whose action is `sub_69640` (EF:55915, remc2 author comment `//spell posses`; dispatch EV:3491-3492).

### 0.1 `sub_69640` (EF:55915) — fire-decision tick
- `word_0x2E_46 > 0` (cooldown live) and owner `v1x = Entities[parentId_0x28_40]` (the player).
- **`sub_68D50(self, player)`** (EF:55548) — the cast gate: player `mana >= 0`, `life >= 0`, and if the token has a `manaRegen_0x88_136` debt pending, the player's castle must cover it; fire allowed when `player.mana >= self.maxMana_0x8C_140 && word_0x2E_46 == word_0x30_48` (full-cooldown, mana-covered) — TRACED.
- **`sub_68DE0(self, player)`** (EF:55569) — the debit: on fire, `player.manaRegen_0x88_136 -= self.maxMana_0x8C_140` (a negative "mana debt" drained elsewhere by the regen loop) — TRACED.
- Spell level = `SPELLS_BEGIN_BUFFER_str[model].subspell[byte_0x46_70].life_0x1A` (EF:55946):
  - **level field 0** → `sub_69900(self, player)` (EF:55990) — the BASIC possession.
  - **level field 1..3** → spawn **(9,17)** directly (EF:55950): impact `byte_0x43_67 = 10`, `byte_0x44_68 = 54` (field==1) or `69` (field==2) (EF:55961-65); `dword_0x10_16 = (subSpellIndex<<8)²` (EF:55974); sound **40** (EF:55982).

### 0.2 `sub_69900` (EF:56039) — the basic possess launch
```c
v2x = IfSubtypeCallCreatingManaSphere_4A190(&player->position, 9, 1);   // EF:56045
v2x->actSpeed += player->actSpeed;            // carpet-speed boost
sub_68E50(player, v2x, token);                // muzzle placement/trail
v2x->byte_0x43_67 = 10;  v2x->byte_0x44_68 = 12;   // impact = (10,12)  EF:56051/56053
v2x->word_0x26_38 = tokenIndex; v2x->id_0x1A_26 = player->id;
v2x->mana = token->mana; v2x->dword_0x10_16 = 200;
... aim from player yaw/pitch (+head offsets), designated point 10240 ahead;
PrepareEventSound_6E450(projIdx, -1, 40);     // CAST SOUND 40   EF:56068
```
TRACED. The projectile ctor `SummonManaPosession_4D3B0` (EF:34764): class 9 model 1, action 1, speed 384/384, `maxLife = 4096/384 = 10` ticks, behavior row `str_D7BD6[61]` (yaw/pitch homing caps 113/113), sprite 209, box ×2/×2.5 via `SetEntityShiftRot_49EA0`, `xtype_0x41_65 = 10` (xsubtype stays -1 from `NewEvent_4A050`, EV:561).

---

## 1. Flight + victim probe — `CastPosses_65F60` (EF:63210, class-9 action 1)

Per-tick (full law in mc2-class9-spell-projectiles.md §0x01): one-shot auto-aim `sub_67CB0`, homing `sub_65610` (caps 113/113) when a target is locked, move at 384, terrain/cave z-clamp, then the **specialized probe `sub_108B0`** (EF:3783) — NOT the generic `sub_10780`:

```c
// per map cell in radius (box.pitch+255)>>8, per entity with byte[0]&8:
v4 = class;                                     // EF:3823
if (class == 5  && model == 22)        → candidate;         // possessable creature (5,22)
if (class == 10 && model in {0x27,0x28})→ candidate;        // mana spheres (10,39)/(10,40)
if (class == 10 && model == 0x2D) {
    if (str_D93C0_bldgprmbuffer[byte_0x46_70].byte_2 & 8)   // EF:3853
        NOT a candidate;                        // STONE building: projectile flies through
    else → candidate;                                        // BUILDING (10,45)
}
if (class == 10 && model == 57
    && parentId_0x28_40 != caster.id && sub_106C0) return hit;  // EF:3846 (no owner filter)
// candidate accept filter (EF:3862-3867):
if (caster.id != target.id_0x1A_26
 && caster.id != target.playerEntityIndex_0x94_148           // NOT already owned by caster
 && sub_106C0(self, target))  return target;
```
TRACED. `sub_106C0` (EF:3720 → `sub_10630` EF:3712) is a full 3D box test: `|dx| < pa+pb && |dy| < ra+rb && |za+yawa − zb−yawb| < fa+fb`.

On hit: snap onto victim (`sub_65580`/pos-copy/`sub_655A0`), impact flag set. On terrain-stop or life (10 ticks) expiry: impact flag also set (EF:63293-63299) — **the pulse spawns even on a ground miss**, which is why a near-miss still claims (§3 overlap).

Impact (EF:63306-63319): spawn `_4A190(pos, byte_0x43_67, byte_0x44_68)` = **(10,12)**; `sub_65780` (accuracy stats only, EF:62836 — NOT the claim); if a victim was hit, possession-XP `sub_6D8B0(id, 1, 1)` (EF:63314); copy `id/yaw/pitch` to the pulse; despawn projectile. No impact sound of its own.

---

## 2. The claim pulses — (10,12) normal and (10,70) steal

### 2.1 Ctors (identical bodies, different action)
`NewAdd0A0C_4E8C0` (EF:35574): class 10 model 0x0C, **action 12**, life 8, `subSpellIndex = 64000`, sprite 41, `byte[0] = (…&0xF6)|1`, box **512/512/512** via `SetEntityShiftRot_49EA0(512,512)` (EF:32874: pitch=roll=shift, fov=arg2). RNG 0.
`NewAdd0A46_4E950` (EF:35596): same but model 0x46, **action 0x4D**. RNG 0.

### 2.2 Actions (strA0 rows EF:1614 / EF:1679)
```c
void PossesHitMana_320E0(entity)   // EF:23546, action 0x0C
{  entity->dword_0x10_16++;
   if (entity->life_0x8-- < 0) DisableEntityDrawing04_57F10(entity);
   else { sub_585A0(entity); sub_112D0(entity, 0); } }        // NORMAL claim, a2=0

void sub_32120(entity)             // EF:23559, action 0x4D
{  ... identical ...  sub_112D0(entity, 1u); }                // FORCED/steal claim, a2=1
```
TRACED: the pulse broadcasts **every tick of its 9-tick window** (life 8, post-decrement) while animating sprite 41.

---

## 3. The claim broadcast — `sub_112D0` (EF:4162)

```c
void sub_112D0(type_entity_0x6E8E* a1x, unsigned __int16 a2)   // a1x = pulse, a2 = force flag
{
    // cell scan: radius v2 = (box.pitch + 255) >> 8  → 512 ⇒ ±2 tiles   EF:4175-4181
    for each entity v5x in cells:
        if (a1x->id_0x1A_26 != v5x->id_0x1A_26        // not the claimer's own entity
            && v5x->class_0x3F_63                     // live
            && v5x->struct_byte_0xc_12_15.byte[0] & 8 // targetable
            && v5x->byte_0x38_56 & 2                  // ← THE POSSESS-CHANNEL GATE   EF:4193
            && (a1x->xtype == -1 || class/model filter)   // pulse xtype = -1 ⇒ any  EF:4194-4196
            && sub_106C0(a1x, v5x))                   // 3D box overlap              EF:4197
        {
            v5x->str_0x5E_94.word_0x68_104 = a1x->id_0x1A_26;   // claimer id        EF:4199
            v5x->str_0x5E_94.dword_0x64_100 = a2;               // 0 normal / 1 force EF:4200
        }
}
```
TRACED verbatim. No range check beyond box overlap, no LOS, no ownership check here (the consumer decides), no mana check (paid at cast). Multiple entities in the box all get the mail.

---

## 4. Consumers of the claim channel

### 4.1 Building — `AddHouse0A_2D_38330` (EF:27959), the state-52 house tick, intake at EF:28016-28042
```c
v7 = event->str_0x5E_94.word_0x68_104;
if (v7) {
    if (v7 != event->playerEntityIndex_0x94_148) {            // owner CHANGE only
        if (event->str_0x5E_94.dword_0x64_100) {               // FORCED (from (10,70))
            event->playerEntityIndex_0x94_148 = v7;
            PrepareEventSound_6E450(v7, -1, 4);                // chime 4 AT THE CLAIMER  EF:28024
            event->struct_byte_0xc_12_15.dword &= 0xFFDFFFFE;  // clear byte[0] bit0 + byte[2] bit5
            event->struct_byte_0xc_12_15.byte[2] |= 0x20u;     // set the CLAIM LOCK      EF:28026
            SetEntityIndexAndRot_49CD0(event, 177);
            event->word_0x5A_90 += Entities[v7]->dword_0xA4_164x->playerColorIndex_0x38_56;
        }
        else if (!(event->struct_byte_0xc_12_15.byte[2] & 0x20)) {  // NORMAL, not locked
            event->playerEntityIndex_0x94_148 = v7;                 // new owner
            PrepareEventSound_6E450(v7, -1, 4);                     // chime 4            EF:28034
            event->struct_byte_0xc_12_15.byte[0] &= 0xFEu;          // FLAG FLIES (bit0 clear) EF:28035
            SetEntityIndexAndRot_49CD0(event, 177);                 // claimed sprite row      EF:28036
            event->word_0x5A_90 += ...playerColorIndex_0x38_56;     // + claimer colour row    EF:28037
        }
    }
    event->str_0x5E_94.word_0x68_104 = 0;                      // consume               EF:28040
    event->str_0x5E_94.dword_0x64_100 = 0;
}
```
TRACED (confirms + extends the banked protocol). Preconditions on the CONSUMER side: none beyond "claimer ≠ current owner" and (normal path) the `byte[2]&0x20` lock being clear. No mana, no range, no level-stage, no building-state gate here — everything else was enforced at delivery (`byte_0x38_56 & 2`) and at the probe (stone skip). A normal claim does NOT set the lock, so weak claims can ping-pong between players; only the forced claim ((10,70), spell level-2 possession) sets the lock, after which only another forced claim can steal.

### 4.2 Mana ball — `TransformArcherToMana_35940` (EF:26015, class-10 action 0x29 move tick), intake EF:26069-26094
Byte-for-byte the same protocol with two differences: forced path clears mask `0xFFDFFFBF` (byte[0] bit6 + byte[2] bit5) and normal path clears **byte[0] bit 0x40** (EF:26090) instead of bit 0; no sprite-177 swap (ball recolor happens in the ball's own size/colour logic). Chime 4 at the claimer both paths (EF:26078/26089).

### 4.3 Other `byte_0x38_56 & 2` targets
- **(5,22) worm**: consumer in `sub_26D20` (EF:17447; channel reads/clears EF:17532/17579) — possessable creature.
- **(10,0x2A=42)** `sub_36AE0` (EF:26835): a marker that, when claimed by a class-3 wizard, transfers every entity owned by ITS slot to the claimer and despawns (grave/cache analogue, EF:26847-26857).
- Generic clear in `sub_12A70` (EF:5409) — death/reset path zeroes the channel.

### 4.4 Where the building's gate bit comes from — `sub_49A30` (EF:32753)
Building ctor `AddTerrainModification_50250` (EF:36677): class 10 model 0x2D, action 0x33 (build-up), `byte[0] = 9` (bit0 = UNPOSSESSED marker + bit3 targetable), **`byte_0x38_56 = 33`** (bit0 ch0-damage + bit5), sprite 177. Both spawn paths (PrepareEvents EV:348; THING/disposition EF:33090 `sub_49A30(v2x, entity->par1_14)`) then run `sub_49A30`:
```c
if (!(str_D93C0_bldgprmbuffer[a2].byte_2 & 8)) {   // NOT a stone template   EF:32799
    a1x->byte_0x38_56 |= 2u;                       // possess channel open   EF:32802
    a1x->mana_0x90_144 = 1000 * subSpellIndex >> 7;
}
```
TRACED — stone templates (flags&8) never open the channel, matching the probe-side skip (§1).

---

## 5. Leveled possession — the (9,17) chain

(9,17) ctor `sub_4DDD0` (EF:35132): class 9 model 17, **action 18 = `sub_674C0`** (EV:3350 `//possess mana ii`, strF/str90 row EF:1551), same sprite 209/row-61 homing as (9,1). Its impact (EF:59032-59059) spawns **TWO** children when a victim was struck:
1. The claim pulse: `(10,12)` normal — unless `xsubtype == 69`, then **`(10,70)`** = the FORCED pulse (EF:59036-59039).
2. The auxiliary `(10, xsubtype)` = (10,54) or (10,69): both strA1 rows point at `AddAuxiliary_50500` (EF:36812; rows EF:1758/1773) — a 128-tick action-0x3B cosmetic entity (possessed-target shimmer), plus possession-XP `sub_6D8B0(id,1,1)` (EF:59052).

So spell-level mapping (from §0.1): subspell `life_0x1A` 0 → weak claim; 1 → (10,54) aux + weak; 2 → (10,69) aux + **(10,70) forced claim/steal**. TRACED (data values per level live in SPELLS.DAT, not re-extracted here — OPEN-2).

---

## 6. Retail feedback worth banking as verification checks

| Signal | When | Cite |
|---|---|---|
| Sound **40** | at cast, anchored at the projectile | EF:56068 / EF:55982 |
| Sound **4** | on ownership change (building AND ball), anchored at the CLAIMER entity | EF:28024/28034, EF:26078/26089 |
| Building sprite → **177 + claimer colour row** | on claim | EF:28027-28028 / 28036-28037 |
| Building `byte[0]` bit0 cleared | on claim → map blip switches off the `UNPOSSESSED_BUILDING` palette entry (0x888) | EF:28035; GameUI:1173/1295, MapColourIndexs.h:10 |
| Claim pulse visual: sprite 41 anim, 9 ticks, 512-box | at impact/ground point | EF:35588, 23546-23555 |
| Possession XP idx 1 (`sub_6D8B0(id,1,1)`) | only on a HIT victim (not ground miss) | EF:63314, EF:59052 |
| Near-miss claims work | pulse box 512 vs building footprint box, ±2-tile scan | EF:4175-4197 |
| Weak claims can re-flip; level-2 (forced) sets `byte[2]&0x20` lock | claim protocol | EF:28021-28038 |
| Militia pop / mana production are damage/economy signals, NOT claim feedback | house tick | EF:27993-28014, 28043-28062 |

---

## 7. OUR PORT — the divergence, precisely

The bridge: `MC2_CAST_BRIDGE[1] = Some(3)` maps MC2 possession onto MC1's Possess (crates/mgc-app/src/ui.rs:1366-1368; equip path main.rs:1144). The MC1 machinery then runs:

1. **Lob flight + probe**: `possess_victim_at` (crates/mgc-sim/src/mc1/combat.rs:971-1004) — accepts class-10 models **39 | 40 | 45**, `flags&8`, not own (`id24`/`f144`), AABB. ⇒ the lob DOES stop on MC2 buildings (their ctor keeps default `flags=8` and ors bit0 → 9, mc2/mobs.rs:1712 + features.rs:792).
2. **Detonation**: state-12 ctor (mc1/combat.rs:2261-2273; sprite 41, extents 512, life 8) → `possess_flash_tick` (mc1/combat.rs:2014-2023) → **`area_write(i, 1, amt, …)`** every tick — the faithful mirror of `sub_112D0`.
3. **The gate** (mc1/combat.rs:170-177): candidates must pass
   ```rust
   c.id24 != id && c.flags & 8 != 0
       && c.f28 & (1 << ch) != 0          // ← ch=1 ⇒ needs f28 bit 1
       && Self::filter_admits(f66, f67, …) // flash f66/f67 = 0xFF wildcard (new_event default)
       && self.ent_overlap(i, j)
   ```
4. **Who passes**:
   - MC2 mana spheres ride `spawn_mana_ball` (mc2/effects.rs:144-156 → mc1/combat.rs:2378-2394) which sets **`f28 = 3`** (ch0+ch1) → mail[1] delivered → the ball tick's claim intake (mc1/combat.rs:2945-2958) fires. **Balls work.**
   - MC1 buildings set **`f28 = 33`** at spawn (mc1/features.rs:1486) and **`f28 |= 2`** in the house setup (mc1/features.rs:1619) → MC1 possession works.
   - **MC2 buildings**: `mc2_spawn_building` (mc2/mobs.rs:1692-1761) sets `f56 = 33` (line 1709) and `f56 |= 2` for non-stone (line 1756) — the faithful mirror of retail `byte_0x38_56` — but **never writes `f28`**, and `new_event` defaults it to 0 (mc1/features.rs:789-805 — `Ent::default()`, no f28 line). `area_write` ch1 therefore drops the claim, and the intake `mc2_house_tick` (mc2/mobs.rs:2023-2034) — which is correct and waiting on `mail[1]` — never receives it.

**Root cause in one line:** in our engine, retail's `byte_0x38_56` channel mask was split into two fields — `f56` (the faithful mirror, currently write-only for buildings) and `f28` (the field the shared writer actually reads, per the cross-column damage contract "MC2 ctors set f28=1") — and `mc2_spawn_building` only populated the mirror.

**Collateral of the same omission (INFERENCE, verify in playtest):** with `f28 = 0`, MC2 buildings are also invisible to every ch0 **area** writer (fire cells, explosion flashes — anything routed through `area_write`), because retail's `byte_0x38_56 = 33` bit0 is likewise unreflected. Direct-hit mail paths that bypass `area_write` still land, which is why building damage was observed working in PLAYTEST-2.

### The fix (concrete)
In `mc2_spawn_building` (crates/mgc-sim/src/mc2/mobs.rs):
```rust
// ctor block (next to `e.f56 = 33;`, line 1709):
e.f28 = 1;              // cross-column contract: byte_0x38_56 bit0 = ch0 area intake

// non-stone branch (next to `e.f56 |= 2;`, line 1756):
e.f28 |= 2;             // byte_0x38_56 bit1 = possess claim channel (sub_49A30 EF:32802)
```
That is the whole bug for the playtest deviation. Stone templates stay closed on both fields, matching retail. MC1 code is untouched (MC1 goldens safe); MC2 state-hash goldens may shift if `f28` participates in the hash — re-pin the MC2 goldens only.

### Secondary fidelity notes (follow-ups, not the bug)
1. `possess_victim_at` does not skip stone buildings the way retail `sub_108B0` does (EF:3853) — our bridged lob will STOP on a stone building and detonate uselessly instead of flying through. Cosmetic under the bridge; belongs to the Phase-4.2 MC2 spell column (which should also add the (5,22) worm and (10,57) probe targets).
2. ~~The forced/steal tier ((9,17) level-2 → (10,70) pulse → `byte[2]&0x20` lock, §5) has no bridge analogue — banked for the MC2 spell column.~~ LANDED 2026-07-27: the tier-2 impact arm (`mc2_proj_impact` (10,69)) broadcasts the claim with force = 1 (the ch1 mail AMOUNT now carries retail's `dword_0x64_100` force flag — no consumer ever read a ch1 damage); both intakes run the retail forced/locked protocol (`F_CLAIM_LOCK` = flags bit 29 mirrors `byte[2]&0x20`). The rival-side direct claim write (`sub_135C0` EF:5849-50) is lock-blind in retail too — kept faithful. Pinned by `mc2_mana_lock_forces_the_claim_and_locks_out_weak_steals`.
3. Claim chime anchored at the claimer (retail plays it for whichever wizard claims): ours is player-gated `snd_player(4)` (mc2/mobs.rs:2030) — fine until rival wizards possess buildings.

---

## Addendum 2026-07-23 — the probe's ownership gate, port fix

Player report: already-claimed mana balls near the caster CONSUMED the
bolt and shielded unclaimed balls behind them. Root cause: the port's
`claim_admits` (mc1/combat.rs) carried only the creator half
(`id_0x1A_26` → `id24`) of `sub_108B0`'s two-armed accept filter
(EF:3862-67) — the claim-owner half (`playerEntityIndex_0x94_148` →
`f144`, the field both claim intakes write) was noted APPROX-missing
and never wired. Retail's bolt flies THROUGH anything already owned
by the CASTER (either field) and still collides with rival-claimed
targets; the gate is in the scan itself (a true fly-through, not a
consumed-but-no-op hit) and applies identically at both call sites
(`CastPosses_65F60`, `sub_674C0`). Fixed: `claim_admits` now checks
`f144 != own` on the (5,22)/(10,39|40)/(10,45) arms — (10,57) keeps
its parent-tag-only early-return arm (EF:3846). The autoaim lists
(`mc2_aim_scan`) already had the `f144` skip. Non-vacuity test:
`mc2_possession_probe_skips_caster_claimed_targets` (world.rs);
goldens unchanged (they never fire projectiles).

## Addendum 2026-08-03 — the TIER-0 bolt is (9,1), and the port was launching (9,17) for every tier

§0.1's tier gate is not just a payload switch: it selects a different
ENTITY. `life_0x1A == 0` routes to `sub_69900` (EF:56039), whose
creator is `SummonManaPosession_4D3B0` (EF:34764) = class **9 model 1**,
**action 1**, speed/minSpeed 384, `maxLife = 4096/384 = 10`, mana 50,
row `str_D7BD6[61]`, `xtype_0x41_65 = 10`, sprite 209, and — the one
lane that separates it from its leveled twin —
`SetEntityShiftRot_49EA0(2*pitch, **5*fov/2**)` where `sub_4DDD0`
(EF:35132, the (9,17)) uses `2*fov`. Only `life_0x1A` 1..3 take the
inline (9,17) arm (EF:55950); `life > 3` casts NOTHING (the `<= 3`
gate).

`sub_69900`'s tail, field by field (EF:56047-68), verified against
mc2l4 t=13 slot 303 (`dump-state`):

| retail | value there | port home |
|---|---|---|
| `actSpeed += a2x->actSpeed` (EF:56048, **no clamp**) | 336 = 384 − 48 | f126 |
| `sub_68E50` muzzle placement | — | `World::muzzle` |
| `byte_0x43_67 / byte_0x44_68` | 10 / 12 | f68 / f69 |
| `word_0x26_38` = the TOKEN's slot | 267 | @0x26 → f40 (port spends f40 on the spell index — see the ledger) |
| `id_0x1A_26` = the caster | 265 | id24 (PLAYER_TARGET) |
| `position.z += a2x->array_0x52_82.fov` | — | already in `muzzle` (pose z + PLAYER_HH) |
| `mana_0x90_144` = the TOKEN's mana | **33** | f140 (ctor default 50 overwritten) |
| `dword_0x10_16 = 200` | 200 | @0x10 → f26 |
| `wizext.byte_0x154_340 = 0` | — | unported (input-latch reset) |
| `axis_0x9A_154x` = caster pos moved 10240 along the aim | (402, 6608, −1411) | unported (not a compared lane) |
| yaw/pitch = `wizext.nextEntity_0x18_24 + yaw` / `entityIndex2_0x1A_26 + pitch` | 36 / 121 | p.heading / p.pitch — **the head offsets are NOT recorded**, see the ledger's recorder lead |
| `PrepareEventSound_6E450(proj, -1, 40)` | — | `snd_player(40)` |

The **speed clamp** is the trap here: `sub_6DCA0`'s `[384, 0x2000]`
clamp (EF:44226-31) is that dispatcher's alone. Both possession arms
add the carpet boost RAW, so a REVERSING carpet genuinely launches a
sub-384 bolt.

Ported 2026-08-03 (session 8, bolt-launch-lanes dig): `CREATORS` gained
the subtype-1 row, `mc2_spawn_cast_proj` gained the possession pair's
`SetEntityShiftRot`, and `mc2_spell_fire`'s spell-1 arm + the rival
emitter (`mc2_rival_emit`, which hardcoded 17) both pick the entity off
`life_0x1A`. Pinned by
`mc2_tier0_possession_launches_the_basic_bolt_with_sub_69900s_tail`.

## OPEN items
1. ~~**SPELLS.DAT possession rows**~~ EXTRACTED 2026-07-27 from the baked CD `spells.bin` row 1: tier `life_0x1A` = **(0, 1, 2)** — tier 2 genuinely selects xsubtype 69 → the (10,70) FORCED pulse; subSpell (10, 15, 20), manaCost (100, 250, 1000), maxManaLimit (0, 1000, 20000).
2. ~~**(10,54)/(10,69) auxiliary tick**~~ TRANSCRIBED 2026-07-27: action 0x3B = `sub_38D80` (EF:28349) — NOT cosmetic: it is the MAGNET. Life countdown → despawn; per tick, walk the sphere list (dword_38523) and stamp every untagged sphere within `dword_0x10_16` range with the pull mail: `word_0x76_118` = pull magnitude (√dist capped 42), `word_0x7A_122` = the aura's index (the ball mover consumes + clears it, EF:26097-110). **No lock writes** — the aura's 128-tick life does NOT time out the claim lock. Full `byte[2]` writer sweep (whole remc2 tree, 2026-07-27): the 0x20 lock has NO timed clear anywhere — it is entity-lifetime; it "expires" in play only through entity churn (merges — inherited ONLY by sub_36D50's unclaimed-survivor arm EF:26936-40, ported — plus collection/absorb/despawn).
3. **(10,57) probe target** (EF:3846, parent-gated, no owner filter) — identify the entity (suspected tethered/carried object); not needed for building claims.
4. **`playerColorIndex_0x38_56` sprite-row offset** — our renderer uses team tint instead of pre-colored row bands (banked APPROX in mc2_house_tick's doc comment); revisit with the HUD/сolor parity track.
