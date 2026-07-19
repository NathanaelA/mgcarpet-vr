# Spell Audit — Meteor (index 9)

Decompile cites `file:line` in `reference/remc2/remc2/engine/` (EF =
`EventsFunctions.cpp`, EV = `Events.cpp`). Port cites `crates/mgc-sim/…`.
Baked values read from `baked/assets/mc2-*/spells.bin` (row 9). Trace date
2026-07-13.

---

## TL;DR

The meteor **payload IS tier-scaled** in the port (total damage 4000 / 8000 /
16000 per tier reaches the impact correctly). The bug is the **DELIVERY**: the
port never overrides the (10,17) meteor-impact's `maxLife` with the per-tier
**charge** (`life_0x1A` = 2 / 5 / 10). Retail `sub_66180` (EF:63372-73) sets the
impact `maxLife = life = byte_0x46_70` (the charge), which makes the impact tick
for **2 / 5 / 10** ticks and gives per-tick damage `subSpell/maxLife` =
**2000 / 1600 / 1600**. The port leaves `maxLife` at the ctor default **10** for
every tier, so all three tiers run **10 ticks**, grow the damage radius through
the **same 10 rings** (pitch 0→~1918), and split the payload as `payload/10` =
**400 / 800 / 1600** per tick. Net effect: **all three tiers cover an identical
wide, long-lived area** — tier-1 blankets a ~7.5-tile field for 10 ticks (should
be ~1.5 tiles for 2 ticks → "level 1 too powerful"), and tier-3 is only a
per-tick bump over that same footprint, so it does not read as a bigger meteor
("level 3 not powerful enough"). This is exactly the maxLife-override omission
flagged as the "charge-tiered fuse lands with 4.2" TODO at `proj.rs:135-138`.

**One-line fix:** in `mc2_proj_impact` give the `(10, 17)` arm the same
charge-override the `(10, 22)`/`(10, 89)` arms already have (read `f71`, write it
to the spawned impact's `max_life` and `act_life`).

---

## 1. Identify — the meteor chain + SPELLS.DAT row 9 (baked)

- **Spell index 9** → class-15 manifestation model 9. Dispatch `mc2_dispatch_arm`
  (`cast.rs:680`): `9 => arm(3, (10, 17), true)` — class-9 **subtype 3** projectile
  (the meteor shot, `sub_4D500` EF:34798: class 9 / model 3 / action 3, speed 384,
  maxLife 21, sprite 76), impact **class-10 model 17**, `charge = true`.
- **Impact ctor** `AddMeteor_4ED70` (EF:35731): class 10 / model 17 / action 17,
  `maxLife = 10`, `subSpellIndex = 3000` (default), untargetable, not
  map-registered.
- **Impact tick** action 17 = `sub_32880` (EF:23834), via strA0[0x11]=0x213880
  (EF:1619).
- `GetSpellIndex_6E020`: class-10 model 17 → SPELLS row **9** (`spells.rs:123`,
  `spell_index(17) = 9`).

**Baked SPELLS.DAT row 9 (identical across mc2-day/night/night-fog/cave):**

| tier | subSpell (payload) | manaCost | maxManaLimit | xpos1 | word_0x18 | **life (charge)** | font |
|---|---|---|---|---|---|---|---|
| 0 | **4000** | 6000 | 40000 | 0 | 3 | **2** | 0 |
| 1 | **8000** | 10000 | 80000 | 2000 | 7 | **5** | 0 |
| 2 | **16000** | 20000 | 150000 | 10000 | 11 | **10** | 0 |

`byte_0 = 3` (3 tiers), `enabled = 8`. The payload **doubles** per tier
(4000→8000→16000) and the charge scales 2/5/10 — both are meant to differentiate
the tiers. (The baked values are the CD table, not the decompile fallback — this
row is authority.)

---

## 2. RETAIL — the per-tier law (what scales the impact, exact numbers)

Two fields ride from the tier row onto the meteor **shot** (`sub_6DCA0` a3=8/9,
per `docs/traces/mc2-class9-spell-projectiles.md:134`):
- projectile `subSpellIndex_0x2A_42 = tier.subSpellIndex_2` (4000/8000/16000)
- projectile `byte_0x46_70 = tier.life_0x1A` (2/5/10) — **the charge**

On detonation the shot's action wrapper **`sub_66180`** (EF:63340) runs the flyer
core `sub_65820` (which spawns the (10,17) impact and copies the shot's
`subSpellIndex` onto it, the fireball-style EF:63191 copy → impact carries
4000/8000/16000), then **overrides the impact's fuse with the charge**
(EF:63369-74):

```c
if (v1x) {                               // v1x = the spawned (10,17) impact
    v1x->maxLife_0x4 = a1x->byte_0x46_70;   // maxLife = charge (2/5/10)
    v1x->life_0x8    = a1x->byte_0x46_70;   // life    = charge
}
```

The impact tick **`sub_32880`** applies damage as **`subSpell / maxLife` per
tick** for `life` (= maxLife) ticks (EF:23869):

```c
v4 = sub_10C80(a1x, 0, a1x->subSpellIndex_0x2A_42 / a1x->maxLife_0x4);   // ch0 area damage
if (v4) sub_6D8B0(a1x->id_0x1A_26, 9u, v4);                              // earthquake/shake kind 9
```

Each tick the damage **radius grows** with the ring counter
`SetEntityShiftRot((768*ring − sign·5)>>2, 512)` where `ring` cycles
`(ring+2)%11` (EF:23865, 23900); the ctor leaves `ring` = 0, so the pitch (=
`array_0x52_82.pitch` = the `sub_10C80` radius) walks 0, 382, 767, 1150, 1534,
1918, 190, … over successive ticks. The tick also sprays a ring of **(10,0) fire
children** flagged `dword |= 0x10080` (byte[2] bit0) → **damage-suppressed
visuals** (the fire tick's 0x1_0000 gate); they add no damage. So the meteor's
only damage is the per-tick `subSpell/maxLife` write.

**Retail per-tier delivery:**

| tier | payload | charge = maxLife | per-tick = payload/maxLife | ticks | radius growth (pitch) | total to centered victim |
|---|---|---|---|---|---|---|
| 0 | 4000 | **2** | **2000** | **2** | 0 → 382 (≈1.5 tiles) | 4000 |
| 1 | 8000 | **5** | **1600** | **5** | 0 → 1534 (≈6 tiles) | 8000 |
| 2 | 16000 | **10** | **1600** | **10** | 0 → 1918 (≈7.5 tiles) | 16000 |

`sub_10C80` (channel 0) is the flat straight-subtraction area write (no falloff,
no armor; `docs/traces/mc2-fireball-damage.md` §3): each in-radius, channel-0,
targetable victim gets `+= amt`, drained as `life -= amt`, death at `life < 0`.

---

## 3. CURRENT PORT — does the tier payload reach the impact? (yes; the fuse does not)

**Payload path (correct):**
- `mc2_launch` (`cast.rs:882`) sets the shot's `e.f44 = sub.sub_spell`
  **unconditionally** (the fireball-damage fix removed the old `payload` gate), so
  the meteor shot flies with `f44` = 4000/8000/16000. It also sets
  `e.f71 = sub.life` (= 2/5/10) because the meteor arm is `charge = true`
  (`cast.rs:883-884`).
- The shot detonates via `mc2_meteor_shot_tick` → `mc2_flyer_tick` →
  `mc2_proj_impact` (`proj.rs:643`).
- `mc2_proj_impact` spawns the impact and copies the payload onto it:
  `e.f140 = dmg` where `dmg = projectile.f44` (`proj.rs:340-342, 418`). So the
  meteor impact's `f140` = 4000/8000/16000 — **tier-scaled, correct.**
- The impact tick `mc2_meteor_tick` (`tail.rs:1120`) applies
  `amt = f140 / max_life` per tick — the faithful `subSpell/maxLife` law.

**Fuse path (the BUG):** the `(10, 17)` arm is a bare spawn (`proj.rs:357`):

```rust
(10, 17) => self.mc2_spawn_meteor(x, y, z),
```

It never reads `f71` (the charge) to override the impact's `max_life`/`act_life`.
Contrast the two arms directly beside it that DO carry the charge fuse:

```rust
(10, 22) => { let charge = self.ent[i].f71; … self.ent[s].max_life = 8*charge; … }   // whirlwind, proj.rs:365-372
(10, 89) => { let charge = self.ent[i].f71; … self.ent[s].max_life = charge; …    }   // cave-in,  proj.rs:397-404
```

So the meteor impact keeps `mc2_spawn_meteor`'s ctor default `max_life = 10`
(`tail.rs:123`) for **every tier**. It is **not** using an MC1 constant and it is
**not** stuck at the 3000 ctor default — the `tail.rs:1098` "300/tick" doc comment
is **stale** (it predates the `proj.rs:418` payload override; the impact now
carries the real payload). The single divergence is the missing maxLife fuse.

**Port per-tier delivery (current):**

| tier | f140 (payload) | max_life (NOT overridden) | per-tick = payload/10 | ticks | radius growth | total to centered victim |
|---|---|---|---|---|---|---|
| 0 | 4000 | **10** | **400** | **10** | 0 → 1918 (≈7.5 tiles) | 4000 |
| 1 | 8000 | **10** | **800** | **10** | 0 → 1918 | 8000 |
| 2 | 16000 | **10** | **1600** | **10** | 0 → 1918 | 16000 |

---

## 4. GAP — quantified (L1 too-strong / L3 too-weak / "all identical")

Single-target **total** is coincidentally correct per tier (payload sums the same
whether split over 2 or 10 ticks). The gap is **duration + radius growth + per-tick
shape**, all of which are identical across tiers in the port:

| metric | tier | retail | port | error |
|---|---|---|---|---|
| duration (ticks) | 0 | 2 | 10 | **5× too long** |
| | 1 | 5 | 10 | 2× too long |
| | 2 | 10 | 10 | ✓ |
| max radius (pitch) | 0 | ~382 (1.5 tiles) | ~1918 (7.5 tiles) | **5× too wide** |
| | 1 | ~1534 (6 tiles) | ~1918 | 1.25× |
| | 2 | ~1918 | ~1918 | ✓ |
| per-tick dmg | 0 | 2000 | 400 | 1/5 |
| | 1 | 1600 | 800 | 1/2 |
| | 2 | 1600 | 1600 | ✓ |

Reading the player's report against this:
- **"Level 1 too powerful"** — tier-0 should be a brief (2-tick) ~1.5-tile burst;
  the port spreads it over a 10-tick, ~7.5-tile growing field, so a single tier-0
  meteor now blankets and clears a wide area of weak mobs (settlers etc.) like a
  tier-3 should — an area/duration overpower.
- **"Level 3 not powerful enough / all identical"** — because tier-0 and tier-1
  already run the **full 10-tick / 10-ring footprint**, tier-2 only differs by a
  higher per-tick number over the **same** area and duration, so it does not read
  as a larger meteor. All three tiers share one footprint → "identical."

---

## 5. FIX DATA — exact per-tier payloads + impact formula

**Per-tier row-9 values the impact must honor** (baked, §1):

| tier | subSpell (→ f140) | charge = life_0x1A (→ maxLife) | resulting per-tick = subSpell/charge | ticks |
|---|---|---|---|---|
| 0 | 4000 | 2 | 2000 | 2 |
| 1 | 8000 | 5 | 1600 | 5 |
| 2 | 16000 | 10 | 1600 | 10 |

**Impact-damage formula (already correct in `mc2_meteor_tick`, `tail.rs:1120`):**
`per_tick = f140 / max_life`, applied via `area_write(i, 0, per_tick, …)` each of
`max_life` ticks; radius = the tick's grown `shift`/pitch. The ONLY missing input
is `max_life` = the charge.

**The change** — `proj.rs:357`, mirror the `(10, 22)`/`(10, 89)` arms:

```rust
(10, 17) => {
    let charge = self.ent[i].f71;               // 2 / 5 / 10 from mc2_launch (arm.charge)
    let s = self.mc2_spawn_meteor(x, y, z);
    if let Some(s) = s {
        let ml = (charge as u32).max(1);         // guard div-by-zero in the tick
        self.ent[s].max_life = ml;
        self.ent[s].act_life = ml as i32;
    }
    s
}
```

Faithful to `sub_66180` EF:63372-73 (`maxLife = life = byte_0x46_70`). `f71`
survives flight (the whirlwind/cave-in arms rely on the same, both
player-certified). No other change needed: `f140` already carries the tier
payload (`proj.rs:418`), and the `subSpell/maxLife` tick is already faithful. The
stale "300/tick" comment at `tail.rs:1096-1099` should be corrected to
`subSpell/charge = 2000/1600/1600 per tick over 2/5/10 ticks`.

Notes:
- The `(10, 0)` fire children remain damage-suppressed (`flags |= 0x1_0080`,
  `tail.rs:1136`) — matches retail; no damage change there.
- `sub_6D8B0(id, 9, hits)` (the earthquake/shake kind-9 return) is a shake event,
  not damage; its absence in the port does not affect the damage audit (banked
  with the spell-XP column per the tail.rs note).

---

## 6. Confidence + suggested test

**Confidence: HIGH.** The retail override is verbatim (EF:63372-73), the baked
per-tier values are read directly (identical across all four bundles), the port
payload path is traced line-by-line to the impact (`cast.rs:882` → `proj.rs:418`
→ `tail.rs:1120`), and the omission is the pre-flagged `proj.rs:135-138` TODO with
two adjacent arms (`10,22`/`10,89`) showing the exact pattern to copy. The one
soft spot: the single-target **total** is already correct, so the fix's visible
effect is duration/radius/per-tick, not total kill-count on a lone strong target —
confirm the area/duration feel in playtest.

**Suggested test (assert the fuse + per-tick, per tier):**
1. MC2 level; grant meteor (dev-spells); select tier `t` ∈ {0,1,2}.
2. Cast; step the sim until the shot detonates; locate the spawned `(10,17)`
   impact entity.
3. Assert `impact.max_life == [2,5,10][t]` and `impact.act_life == [2,5,10][t]`
   (pre-fix these are all 10 — the regression guard).
4. Assert first-tick `amt = f140 / max_life == [2000,1600,1600][t]`
   (pre-fix: `[400,800,1600][t]`).
5. Damage end-to-end: place a fixed-HP dummy (e.g. settler 1000 HP) at the burst
   center; tick to impact-expiry; assert total delivered == `[4000,8000,16000][t]`
   (unchanged by the fix — it is the delivery, not the total, that changes) and
   that the impact **despawns after `[2,5,10][t]` ticks** (the load-bearing
   assertion). A wide-field variant (ring of dummies at 2/4/6 tiles) will show
   tier-0 no longer reaching the far ring.
