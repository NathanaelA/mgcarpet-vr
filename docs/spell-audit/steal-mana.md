# Spell Audit — Steal Mana (index 13 / 0xD)

Fidelity audit for the MC2 spell-verification track. Recorded gameplay is
senior; the vendored decompile (`reference/remc2/remc2/engine/`,
`EventsFunctions.cpp`=`EF`) is reference. Player report: "completely absent,
all 3 … WARN: misfit thing (class 15, model 13) x1".

## TL;DR

Steal Mana is **NOT** a direct-effect mana-magnet — the cast-path trace
§2.2 (mc2-player-cast-path.md:241) was **WRONG** on this row. It is a normal
**homing projectile** spell. Its class-15 effect state `sub_6B3E0` (EF:57177)
is the standard projectile skeleton and calls `sub_6DCA0(caster, pos, 0xD,
&subspell[tier], actSpeed, 1)` (EF:57195), which spawns a **class-9 subtype-8**
homing bolt (sprite 214, homing row 63) carrying impact **(10,25)** and the
**tier index** in `byte_0x46_70`. The bolt homes onto and must **directly
strike an enemy wizard** (class-3 model 0/1); on that hit it spawns the
**(10,25)** burst (`sub_33E20`, EF:24817) whose `sub_10C80(self, 3, tier)`
AoE-**stamps a "steal" marker** (channel a2=3) into nearby wizards. Each
marked wizard, on its own next tick, runs `sub_61050` (EF:62076) which reads
`SPELLS[13].subspell[tier]` and drains mana **to the caster**:

- **L1 (tier 0)** — `life=0` → flat **2000** mana drained from the victim's
  personal pool → credited to the caster.
- **L2 (tier 1)** — `life=0` → flat **4000** mana, same mechanism.
- **L3 (tier 2)** — `life=1` → **castle drain**: **10 %** of the victim's
  **castle** mana, re-emitted as `(10,39)` mana spheres from the *caster's*
  castle (falls back to a trivial 10-point personal drain if either side has
  no castle). Awards steal-mana XP (index 13) to the caster.

Current port: `cast.rs:845` routes 13 into the `4 | 0xD | 0xE` `note_misfit`
stub. The cast still **charges full mana** (10 000 / 20 000 / 30 000) but does
nothing — worse than absent. Fix = wire 13 as a projectile arm + a new
channel-3 "steal" inbox + a drain consumer.

---

## 1. Identify — the chain

| element | value | cite |
|---|---|---|
| spell index / class-15 model | **13 (0xD)** | — |
| class-15 effect state | `sub_6B3E0` | EF:57177 (dispatch E:3648) |
| dispatch call | `sub_6DCA0(caster, &pos, **0xD**, &subspell[tier], actSpeed, 1)` | EF:57195-57202 |
| 0xD arm → projectile | class-9 **subtype 8** via `_4A190(pos,9,8)` | EF:44112 |
| impact stamped on bolt | `byte_0x43_67 = 10`, `byte_0x44_68 = 25` → **(10,25)** | EF:44116-44117 |
| cast sound (`v6`) | **15** (default; 0xD arm falls to LABEL_60, never sets it) | EF:44042 / 44233 |
| subtype-8 creator | `sub_4D7D0`: class 9, model 8, action 8, **sprite 214**, homing **row `str_D7BD6[63]`** (yaw cap 11 / pitch cap 22), actSpeed 384, maxLife 21 | EF:34920 |
| flight state 8 | `sub_662E0` | EF:63419 (str90 slot 8 = `0x2472E0`) |
| (10,25) action fn | `sub_33E20` (str-table entry `{…,0x19,0x214E20,…}` → subtype 25) | EF:1627, 24817; dispatch E:2448 |
| burst primitive | `sub_10C80(self, **3**, byte_0x46_70)` — type-3 AoE stamp | EF:24831 |
| drain consumer | `sub_61050(victim)` — gated `if (word_0x74_116) …` in class-3 tick | EF:62076, gate EF:60666 |
| steal-mana XP channel | `sub_6D8B0(casterId, **0xD**, 1)` | EF:62123 |
| castle re-emit sphere | `_4A190(pos, 10, **39**)` from caster castle | EF:62166 |

**SPELLS.DAT row 13** (baked `spells.bin`, identical across day/night/cave;
decoded 2026-07-13). Fields per tier: `subSpellIndex_2` (offset 0),
`manaCost_6`, `maxManaLimit_A`, `hintText_0x16`, `word_0x18` (cast dur),
`life_0x1A`:

| tier | L | subSpell | manaCost | maxManaLimit | dur | **life** | hint |
|---|---|---|---|---|---|---|---|
| 0 | L1 | **2000** | 10000 | 10000 | 5 | **0** | 226 |
| 1 | L2 | **4000** | 20000 | 16000 | 7 | **0** | 227 |
| 2 | L3 | **10** | 30000 | 32000 | 21 | **1** | 228 |

`life_0x1A` is the **mode selector** for the drain (§2): `0` = flat personal
drain of `subSpell`; `1`/`2` = castle drain of `subSpell` **percent**.

---

## 2. RETAIL behaviour, per tier

### 2.1 Delivery (all tiers)
`sub_6B3E0` (EF:57177) is the canonical projectile effect state (identical
skeleton to fireball's `sub_693F0`): on the first cast tick it calls
`sub_6DCA0(…,0xD,…)` and then post-writes the returned bolt (EF:57204-57221):
`id` = owner, `mana` = spell per-tick mana, `dword_0x10_16` = charge counter,
**`byte_0x46_70 = spell-entity byte_0x46_70` = the TIER INDEX 0/1/2**
(EF:57213), plus yaw/pitch/aim. NOTE: `sub_6DCA0`'s 0xD arm does **not** set
`subSpellIndex` or `byte_0x46_70` on the bolt (it exits at LABEL_60) — the
tier index reaches the bolt only via `sub_6B3E0`. The bolt homes (row 63,
yaw cap 11).

**The bolt only detonates on a direct wizard hit.** In `sub_662E0`'s impact
block (EF:63531-63562): if the struck victim `v5x` is missing, not class 3,
or not model 0/1, it just books stats (`sub_65780`) and despawns with **no
(10,25) spawn** — the cast fizzles on terrain/creatures/expiry. Only when
`v5x` is a **class-3 wizard (model 0 or 1)** does it spawn `_4A190(pos,10,25)`
and copy `subSpellIndex`, `id`, `yaw`, `pitch`, **`byte_0x46_70` (tier)**
onto the burst (EF:63544-63559).

### 2.2 The (10,25) burst
`sub_33E20` (EF:24817) ticks once (`byte[0]&2` one-shot gate): reads
`v2 = byte_0x46_70` (the tier), calls `sub_10C80(self, 3u, v2)`; if it
stamped ≥1 target, sets `life = 0` (self-despawn). No sound.

`sub_10C80(self, a2=3, a3=tier)` (EF:3953): with `a2 ∈ {3,4}` it takes the
class-3-only branch (EF:4034-4060) — scans map cells around `self` (radius
from box pitch), and for each **class-3** entity in AABB range that is not the
owner, stamps **channel a2** (channel stride 6 B from `str_0x5E_94`: a2=3 →
`dword_0x70_112` value + `word_0x74_116` owner), but **only if that channel is
free** (`!word_0x62_98`-equivalent): `dword_0x70_112 = tier`,
`word_0x74_116 = self->id` (the caster). No damage is dealt by the burst — it
only plants the "you owe mana to caster X, at tier T" marker.

### 2.3 The drain — `sub_61050` (EF:62076)
Runs in the **marked victim's own** class-3 tick (gate `if (word_0x74_116)`,
EF:60666). `v34x = Entities[word_0x74_116]` = the **thief/caster**;
`tier = dword_0x70_112`; `v4 = SPELLS[13].subspell[tier].life_0x1A`.

- Award steal XP to caster: `sub_6D8B0(casterId, 0xD, 1)` (EF:62123).
- **`life == 0` (L1, L2) — flat personal drain** (EF:62194-62199 → 62202-62220):
  `amount = subSpell`; `caster.mana += amount`; `victim.mana -= amount`; both
  clamped to `[0, maxMana]`. So L1 steals **2000**, L2 steals **4000** of the
  victim's personal mana (effectively `min(amount, victim.mana)`, caster
  capped at its `maxMana`).
- **`life ∈ {1,2}` (L3) — castle percent drain** (EF:62135-62188):
  `v6x` = victim's castle, `v31x` = caster's castle.
  - If **both castles exist** and victim-castle mana `> 0`:
    `stolen = castleMana * subSpell / 100` (L3 → **10 %**); victim castle mana
    is reduced by that amount (EF:62146). The stolen total is re-emitted as a
    loop of `(10,39)` mana spheres launched from the **caster's castle**
    position, each carrying up to **500** mana (chunked, random yaw/speed —
    2 RNG draws/orb via `9377*r+9439`, EF:62158/62170/62174); when
    `life == 2` the orb's `playerEntityIndex` is set to the caster (homed),
    else 0 (free spheres to be collected) — L3 has `life == 1`, so **free
    spheres near the caster's own castle**.
  - Else (either castle missing, or victim castle empty): `v35=1`, falls to
    the flat path with `amount = subSpell = 10` — a trivial 10-point personal
    drain. L3 is therefore an **anti-castle weapon**, near-useless without an
    enemy castle.
- Victim hit-flash: `PlayerHitFrameTime_406=4`, `dword_0x18D_397=16`,
  `word_0x24C_588=64` (EF:62221-62223); clears `word_0x74_116` (EF:62227).

**Credit / ownership:** the stolen personal mana goes straight into the
caster's `mana` (L1/L2); the castle-stolen mana becomes collectable spheres
by the caster's castle (L3). XP index 13 to the caster.

---

## 3. CURRENT PORT — confirmed stub

- `cast.rs:667-690` `mc2_dispatch_arm` lists only spells
  0/7/9/15/16/17/18/20/21/25 — **13 is absent**, so `mc2_spell_fire`
  (cast.rs:707) falls through to the direct band.
- `cast.rs:843-847`: `4 | 0xD | 0xE => self.g.note_misfit(15, spell as u16)` —
  the only action. `note_misfit(15, 13)` is exactly the player's
  "misfit thing (class 15, model 13) x1" warning (class 15 = spell-token
  class, model 13 = the spell).
- **Mana is still charged.** The cast-in-progress machinery arms and drains
  the tier cost (`maxMana` 10 000 / 20 000 / 30 000) independent of the
  stubbed effect body — casting steal mana today burns mana for zero result.
- Infrastructure that already exists and the fix should reuse:
  - `area_write(i, channel, amt, ctx, …)` (the `sub_10C80` equivalent) and
    per-entity `mail[]` channel inboxes — `mail[0]`=damage, `mail[1]`=possess
    claim (proj.rs:352), `mail[5]`=castle upgrade (castle.rs:248). Consumers
    drain their `mail[N]` each tick (castle.rs:228, doomsday.rs:190).
  - `mc2_proj_impact` routing table, proj.rs:344 — `(10,25)` currently hits
    the `_ =>` misfit arm (proj.rs:406) → logs + a bogus **channel-0 damage**
    write.
  - `mc2_spawn_mana_sphere` `(10,39)` (effects.rs:160) — spawner present, its
    rest/fly/claim tick flagged "unported" (the L3 path depends on it).

---

## 4. GAP

1. Spell 13 never becomes a projectile (missing `mc2_dispatch_arm` arm) — it
   sits in the `note_misfit` stub.
2. No `(10,25)` impact handler; it degrades to a channel-0 damage write.
3. No **channel-3 "steal" inbox** and no drain consumer — the whole
   `sub_61050` law (flat vs castle-%, the `life`-selector, XP, hit-flash) is
   absent.
4. No L3 castle-percent path / `(10,39)` re-emit.
5. The projectile must carry the **tier INDEX** in `f71`, but the port's
   generic `charge=true` path sets `f71 = sub.life` (cast.rs:884) — for row 13
   `life` = 0/0/1, which would mis-select the drain tier. **Special-case
   required** (see §5).

---

## 5. FIX DATA

### 5.1 Projectile arm (cast.rs)
- Add to `mc2_dispatch_arm`: `13 => arm(8, (10, 25), <see note>)`. Impact
  `(10,25)`, subtype 8. Cast sound: default **15** (the existing `v6` match
  already yields 15 for non-0/7 spells — no change).
- Remove `0xD` from the `4 | 0xD | 0xE` stub arm (leave 4 and 0xE).
- **Tier byte:** the bolt's `f71` must equal the **tier index** (`tier` =
  `self.g.ent[m].f71`, cast.rs:697), NOT `sub.life`. Either stamp `f71 = tier`
  explicitly in `mc2_launch` for spell 13, or give this arm its own launch.
  (Retail `sub_6B3E0` copies the spell-entity `byte_0x46_70`, EF:57213; the
  0xD arm of `sub_6DCA0` never touches it.) The bolt's `subSpell`/`f44` is
  irrelevant to the drain (the drain re-reads `SPELLS[13]` fresh), so it may
  be left at the row value or 0.
- Homing/flight: subtype-8 rides row 63 (yaw cap 11, pitch cap 22), sprite
  214, maxLife 21 — reuse the existing subtype-8 flight if present; steal mana
  should home (weakly) like the other bolts.

### 5.2 Impact (proj.rs `mc2_proj_impact`)
- Add `(10, 25) =>` arm. **Faithful gate:** only act when the struck `victim`
  is a class-3 wizard (rival body / human) — retail spawns the burst solely on
  a model-0/1 class-3 hit (EF:63537). On such a hit, do a **channel-3**
  area stamp carrying the **tier** as the amount:
  `self.area_write(i, 3, tier /* = ent[i].f71 */, ctx, …)`, return `None`
  (no damage, no visible effect entity needed beyond the stamp; the retail
  `(10,25)` sprite 321 is cosmetic). `area_write`'s a2=3 branch must target
  **class-3 only** and write `mail[3] = (tier, casterId)` **only if empty**
  (mirror `sub_10C80` EF:4050 "if channel free").

### 5.3 Drain consumer (new — wizard/pool tick)
Where each class-3 body drains its `mail[]` (alongside the mail[0] damage and
mail[1] possession consumers), add a **mail[3]** handler = `sub_61050`:
```
let (tier, caster) = mail[3]; mail[3] = (0,0);
if caster is class-3 {
    award_xp(caster, spell=13, 1);
    let s = SPELLS[13].tiers[tier];        // tier is the stamped INDEX
    if s.life == 0 {                        // L1/L2 flat personal
        let amt = s.sub_spell;              // 2000 / 4000
        victim.mana -= amt; caster.mana += amt;   // then clamp both [0,maxMana]
    } else {                                // L3 castle %
        if victim.castle && caster.castle && victim.castle.mana > 0 {
            let stolen = victim.castle.mana * s.sub_spell / 100;   // 10%
            victim.castle.mana -= stolen;
            emit (10,39) spheres from caster.castle, ≤500 mana each,
                 total = stolen; playerEntityIndex = (s.life==2 ? caster : 0);
        } else {                            // fallback flat
            let amt = s.sub_spell;          // 10
            victim.mana -= amt; caster.mana += amt;  // clamp
        }
    }
    set victim hit-flash (PlayerHitFrameTime=4, etc.)
}
```
Clamp order (EF:62211-62220): victim `[0,maxMana]`, then caster `[0,maxMana]`.

### 5.4 Sounds
- Cast: **15** (default).
- Burst `(10,25)`: **none** (`sub_33E20` plays no sound — contrast the meteor
  `(10,17)` `sub_33D80` which plays 24, EF:24803).
- L3 spheres: whatever the `(10,39)` collection tick plays (out of scope).

---

## 6. Confidence, open questions, test

**Confidence: HIGH** on the full chain and the drain law. Every hop is
decompile-verified and the `SPELLS[13]` tier data (2000/4000/10, life 0/0/1)
was decoded from the baked `spells.bin` and cross-checks the `life`-selector
branches in `sub_61050`. The channel-3 offset (`dword_0x70_112` /
`word_0x74_116`) is inferred from the 6-byte channel stride and **confirmed**
by `sub_61050` reading exactly those two fields.

**Open questions**
- **Wizard-hit gate** (§2.1): `sub_662E0` spawns `(10,25)` only on a class-3
  model-0/1 victim. Confidence MEDIUM-HIGH that `v5x` there is the struck
  victim (matches `sub_65780(self, v5x, orig)` = `(self, victim, origTarget)`).
  If a faithful "must hit a wizard" gate feels too fragile in play, an AoE
  fallback (stamp on any impact) still only affects class-3 via `area_write`'s
  a2=3 filter — but that would loosen retail targeting.
- **`(10,39)` mana-sphere tick** is a separate unported system (effects.rs:160
  note) — the L3 castle path is only end-to-end once that lands; until then L3
  can drain the castle and credit the caster directly as an interim.
- **Charge counter** `byte_0x154_340 → dword_0x10_16` is copied onto the bolt
  (EF:57211) but never read by the drain — benign for steal mana.
- **Rival casters:** the drain resolves in *any* class-3 victim's tick, so
  once the consumer exists both human→rival and rival→human steal works; the
  rival AI's decision to cast 13 is separate (rivals brain, out of scope).

**Suggested test** (`mc2_steal_mana_*`)
1. L1/L2: place a rival wizard within homing range, cast steal mana, let the
   bolt connect. Assert rival `mana` drops by ~2000 / ~4000 and the human
   `mana` rises by the same (both clamped); steal XP (idx 13) increments.
2. L3: rival WITH a castle holding mana → assert rival **castle** mana drops
   ~10 % and `(10,39)` spheres appear near the human castle. Rival with **no**
   castle → assert only the 10-point personal fallback.
3. Fizzle: cast with no wizard in the bolt's path → assert no mana moves (bolt
   despawns on terrain/creature with no burst).
4. Re-pin the MC2 state-hash goldens (MC1 untouched — MC2-gated).
