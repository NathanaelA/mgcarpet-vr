# Possession (spell index 1) — decompile-vs-port fidelity audit

**TL;DR:** The port casts all three possession tiers identically (a single weak `(10,12)` claim), so Mana Magnet (tier 1) never drops its mana-attracting aura and Mana Lock (tier 2) never sets the building lock. Retail selects the tier from the SPELLS row's `life_0x1A` byte (0/1/2): tier 0 = plain claim, tier 1 = plain claim **+ a `(10,54)` mana-magnet aura** (range = tier subSpell = 15 tiles), tier 2 = a **forced `(10,70)` claim that locks the building** against weak re-possession **+ a `(10,69)` aux**. The magnet infrastructure already exists in the port (`mc2_spawn_aura` / `mc2_aura_tick`) but is wired only to authored THING magnets, never to the possession cast.

Citations: decompile `reference/remc2/remc2/engine/` (EF = `EventsFunctions.cpp`, L = `Level.cpp`, `Spells.cpp`); port `crates/mgc-sim/src/`. Trace bank `docs/traces/mc2-possession-delivery.md`. CD-baked values read directly from `baked/assets/mc2-day/spells.bin`.

---

## 1. Identity & data

| Field | Value | Source |
|---|---|---|
| Spell index | **1** | cast.rs:157 (`CREATORS` row), Spells.cpp row 1 |
| Class-15 manifestation | **class 15, model 3**, action `sub_69640` "spell posses" | EF:1949/1953, EF:55915 |
| Class-9 projectile | **tier 0 → (9,1)** action 1 `CastPosses_65F60`; **tier 1-2 → (9,17)** action 18 `sub_674C0` "possess mana ii" | EF:56045 / EF:55950, EF:35132, EV:3350 |
| Class-10 impact (normal claim) | **(10,12)** action 12 `PossesHitMana_320E0` | EF:35574, EF:23546 |
| Class-10 impact (forced/steal, tier 2 only) | **(10,70)** action 0x4D `sub_32120` | EF:35596, EF:23559 |
| Class-10 auxiliary | **(10,54)** (tier 1) / **(10,69)** (tier 2), action 0x3B `AddAuxiliary_50500` = the mana-magnet aura `sub_38D80` | EF:36812, EF:28349 |
| SPELLS.DAT row | **#1** | Spells.cpp:8-13 |

**Per-tier SPELLS row (CD-baked `spells.bin`, verified identical in mc2-day & mc2-night; the CD table wins over the decompile fallback per the SPELLS.DAT-import note):**

| Tier | subSpell_2 | manaCost_6 | maxManaLimit_A | word_0x18 | **life_0x1A** | hint |
|---|---|---|---|---|---|---|
| 0 (Possession) | 10 | 100 | 0 | 3 | **0** | 189 |
| 1 (Mana Magnet) | 15 | 250 | 1000 | 41 | **1** | 190 |
| 2 (Mana Lock) | 20 | 1000 | 20000 | 51 | **2** | 191 |

`life_0x1A` (0/1/2) is **the tier selector** read by `sub_69640` (EF:55942), not a lifetime. The decompile fallback (`Spells.cpp` row 1) matches subSpell 10/15/20 and life 0/1/2; only `maxManaLimit`/`xpos` differ, which do not affect the effect.

---

## 2. RETAIL behavior per tier

Cast decision — `sub_69640` (EF:55915), fired when cooldown full & mana covered (gate `sub_68D50` EF:55548, debit `sub_68DE0` EF:55569):

```c
if (SPELLS[model].subspell[byte_0x46_70].life_0x1A) {        // EF:55942  life != 0
    if (life <= 3) {
        v4x = _4A190(&player->pos, 9, 17);                    // (9,17) projectile
        if (life == 1) v4x->byte_0x44_68 = 54;               // EF:55959
        else if (life == 2) v4x->byte_0x44_68 = 69;          // EF:55963
        v4x->byte_0x43_67 = 10;
        v4x->dword_0x10_16 = (subSpellIndex<<8)*(subSpellIndex<<8); // EF:55974 — aura range²
        PrepareEventSound_6E450(..., 40);                    // cast sound 40  EF:55982
    }
} else sub_69900(a1x, v1x);                                  // EF:55987 — tier-0 basic
```

- **Tier 0 — Possession.** `sub_69900` (EF:56039) spawns **(9,1)** with impact `(10,12)`, `dword_0x10_16 = 200`, cast **sound 40** (EF:56068). The projectile homes (sprite 209, row 61, caps 113/113), stops on the first claimable target (probe `sub_108B0` EF:3783: creatures `(5,22)`, spheres `(10,39/40)`, non-stone buildings `(10,45)`), then spawns the `(10,12)` claim pulse — which every tick broadcasts `sub_112D0(pulse, 0)` (EF:4162, **a2=0 weak**) to overlapping entities carrying `byte_0x38_56 & 2`. The house intake (EF:28016-42) changes owner but does **not** set the lock, so a rival's weak claim can re-flip it.

- **Tier 1 — Mana Magnet.** `(9,17)` with `byte_0x44_68 = 54`, `dword_0x10_16 = (15<<8)²`. Impact `sub_674C0` (EF:59032-59059):
  ```c
  if (byte_0x43_67 != 10 || byte_0x44_68 != 69)
      _4A190(pos, 10, 12);          // NORMAL weak claim pulse
  else _4A190(pos, 10, 70);         // (tier-2 path)
  _4A190(pos, byte_0x43_67, byte_0x44_68);   // ALWAYS the aux = (10,54)
      → copies dword_0x10_16 (the range²) onto the aux    // EF:59057
  sub_6D8B0(id,1,1);                // possession XP
  ```
  So tier 1 = a normal weak claim **plus** a `(10,54)` aura dropped at the impact point. The aura `sub_38D80` (EF:28349): over squared range `dword_0x10_16` it drags every **unowned mana sphere** toward its eye, pull speed `min(dist, 42)`, merging coincident spheres — **this is the mana attraction** the player expects. Range in tiles = `subSpellIndex` = **15 tiles** for a cast Mana Magnet (formula EF:55974).

- **Tier 2 — Mana Lock.** `(9,17)` with `byte_0x44_68 = 69`, `dword_0x10_16 = (20<<8)²`. Impact spawns the **forced `(10,70)`** pulse (because `byte_0x44_68 == 69`) which broadcasts `sub_112D0(pulse, 1)` (**a2=1 forced**), **plus** a `(10,69)` aux. The house intake's forced branch (EF:28021-28028): sets new owner, chime 4, `SetEntityIndexAndRot(177)`, and **`byte[2] |= 0x20` — the CLAIM LOCK**. Once locked, the normal branch is gated `if (!(byte[2] & 0x20))` (EF:28030), so weak claims bounce; only another forced claim can steal it. **Mana Lock = a possession that permanently locks the building/creature against rival weak possession.** (The `(10,69)` aux also routes to `AddAuxiliary_50500`; whether it attracts mana like `(10,54)` or is inert is OPEN — see §6.)

Feedback signals (verification): cast **sound 40** at the projectile (EF:55982/56068); ownership-change **chime 4** at the claimer (EF:28024/28034); building sprite → **177 + claimer colour row**; `byte[0]` bit0 cleared → flag flies / map blip toggles; possession XP idx 1 only on a hit victim.

---

## 3. CURRENT PORT behavior per tier

**All three tiers collapse to one path.** `mc2_spell_fire` spell 1 (`crates/mgc-sim/src/mc2/cast.rs:725-738`) hardcodes:

```rust
1 => {
    self.mc2_launch(spell, m, &DispatchArm { subtype: 17, impact: (10, 12), charge: false }, sub, p);
    self.g.snd_player(40);
}
```

- `subtype: 17` → always the `(9,17)` body (CREATORS row cast.rs:157). **No branch on `tier`** (available as `let tier = self.g.ent[m].f71` at cast.rs:697) or on `sub.life`.
- `impact: (10, 12)` for every tier. The projectile impact `mc2_proj_impact` (`crates/mgc-sim/src/mc2/proj.rs:351-354`) handles only `(10,12) → area_write(i, 1, dmg) ; None` — a plain ch1 claim, **no aux spawn, no forced-pulse branch**.
- The claim intake in the house tick (`crates/mgc-sim/src/mc2/mobs.rs:2108-2124`) reads `mail[1]`, sets `f144 = src`, clears flag bit 0, chime 4, sprite 177 — **no force-flag distinction and no lock bit** (comment at mobs.rs:2054-2056 explicitly defers the `byte[2]&0x20` lock to "the MC2 spell column"; all claims run the weak variant).
- The mana-magnet machinery **exists but is unreachable from the cast**: `mc2_spawn_aura` (`tail.rs:263`, model 54, action 0x3B, `f26` = range tiles default 14) and `mc2_aura_tick` (`tail.rs:1245`, drags/merges unowned spheres over `(f26<<8)²`) are dispatched at `crates/mgc-sim/src/mc1/world.rs:1481` (`tick70 == 0x3B`). But the only spawner is the authored-THING disposition path (the 2026-07-13 magnet fix that overrides `f26` from `stageTag`/`swi_id`); **the possession spell never calls `mc2_spawn_aura`.**

**Reconciliation of the 2026-07-13 magnet fix vs the player report:** that fix made *authored* magnet entities `(10,54)` pull mana (range from `stageTag`, merge takes the bigger ball). It did **not** wire the possession SPELL to spawn one. So casting Possession at tier 1 attracts nothing — correct per the current code, and exactly the reported gap. The magnet attraction is (in retail) driven by the possession spell's tier via the `(10,54)` child; in the port that link is missing.

---

## 4. The gap, precisely

1. **Tier is ignored at cast.** cast.rs:725 launches `(9,17)`→`(10,12)` for tiers 0/1/2 alike → "all 3 levels act the same." (The correct tier-0 body is `(9,1)`, but observationally tier 0 is fine because the port never spawns an aux anyway.)
2. **Tier 1 Mana Magnet drops no aura.** The impact never spawns `(10,54)`, so no sphere attraction → "Mana Magnet does not attract mana." Root: the `sub_674C0` two-child impact (EF:59032-59059) is not reproduced; proj.rs:351 only emits the claim.
3. **Tier 2 Mana Lock has no lock.** No forced `(10,70)` pulse, and the house intake (mobs.rs:2108) carries no force flag and sets no `byte[2]&0x20` lock → a Mana-Locked building is re-flippable by any weak claim, indistinguishable from tier 0.

---

## 5. Fix data (enough to implement without re-tracing)

**A. Cast dispatch** — in `mc2_spell_fire` spell 1 (cast.rs:725), branch on `tier` (= `self.g.ent[m].f71`, already bound at cast.rs:697). Launch `(9,17)` (subtype 17) all tiers, but set the projectile's impact model and carry the aura range:

| Tier | proj impact `(f68,f69)` | aura model | aura range (tiles) = tier `subSpell` | claim kind |
|---|---|---|---|---|
| 0 | `(10, 12)` | none | — | weak (a2=0) |
| 1 | `(10, 54)` | 54 | 15 | weak (a2=0) |
| 2 | `(10, 69)` | 69 | 20 | **forced (a2=1)** |

Carry the range on the projectile (retail `dword_0x10_16 = (subSpell<<8)²`; store `subSpell` = `sub.sub_spell` = 15/20 in a spare projectile field, e.g. reuse `f26`, and hand it to the aura's `f26`). Cast sound stays **40**.

**B. Projectile impact** — extend `mc2_proj_impact` (proj.rs:339) for the possess body (`tick70 == 18`) to mirror EF:59032-59059:
- Spawn the claim pulse: if `f69 == 69` → **forced** claim (mark the ch1 mail with the force flag); else the normal weak claim (current `area_write(i, 1, dmg)`).
- **Also** spawn the aux when `f69 ∈ {54, 69}`: `let a = self.mc2_spawn_aura(x, y, z)?; self.ent[a].model65 = f69; self.ent[a].f26 = <carried range 15/20>; self.ent[a].id24 = id;` (life 128, action 0x3B already set — it will tick via world.rs:1481 and attract spheres). Tier 0 (`f69 == 12`) spawns **no** aux.
- Keep the possession XP award (already at proj.rs:426; retail `sub_6D8B0(id,1,1)`).

**C. Forced claim + lock** — mirror EF:28021-28038 in the house intake (mobs.rs:2108) and the ball intake (EF:26069-94):
- Thread a **force bit** through the claim mail (currently `mail[1] = (?, src)`; use the tag slot or a new field for a2).
- Forced (a2=1): set owner, chime 4, sprite 177, and **set a lock flag** on the entity (a dedicated flags bit mirroring `byte[2]&0x20`).
- Weak (a2=0): apply only if the lock flag is clear (mirror `if (!(byte[2] & 0x20))`, EF:28030).
The delivery gate is unchanged — the building must have `f56 & 2` (mobs.rs:1822); stone templates stay closed (they never open `f56 |= 2`, matching retail EF:32799-802).

**D. Aura tick** already correct (`mc2_aura_tick` tail.rs:1245): range `(f26<<8)²`, pull `min(dist,42)`, merges unowned spheres model 39. Set `f26 = 15` (tier 1) / `20` (tier 2) from the carried subSpell; the ctor default 14 is wrong for a cast magnet.

**Sound/model/sprite IDs:** cast sound **40**; claim chime **4** at claimer; claim-pulse sprite **41**, box 512, life 8/9 ticks; building claimed sprite **177**; aura invisible, life 128, extents (1024, 0x4000). Impact models: normal claim `(10,12)`, forced claim `(10,70)`, aux `(10,54)`/`(10,69)`.

---

## 6. Confidence, open questions, test

**Confidence: HIGH** on tier structure, the two-child impact, and the numeric table — all read verbatim from EF:55915-55990, EF:59025-59059, and the CD-baked `spells.bin` (row 1: subSpell 10/15/20, life 0/1/2). **MEDIUM** on the exact magnet range: `dword_0x10_16 = (subSpellIndex_0x2A_42 << 8)²` is verbatim (EF:55974), but I did not re-trace that the token's `subSpellIndex_0x2A_42` equals the tier `subSpell_2` (15/20) end-to-end through the equip/`SetSpell` wiring — it is the strongly-implied source. **The port's magnet infra is confirmed present and dispatched** (tail.rs:263/1245, world.rs:1481).

**Open questions:**
1. Does the class-15 token's `subSpellIndex_0x2A_42` = the tier `subSpell_2`? (Confirms aura range 15/20 vs some other value.) Trace `SetSpell_6D5E0` (L:1505) / the equip path.
2. Is the `(10,69)` tier-2 aux a mana-magnet like `(10,54)`, or inert (with the lock being tier 2's whole feature)? Both route to `AddAuxiliary_50500`; check whether `sub_38D80` branches on model 54 vs 69.
3. Meaning of `word_0x18` (3/41/51) and `maxManaLimit` (0/1000/20000) for the possession tiers — untraced; likely HUD/hint layout, not effect.

**Suggested cast→effect test:** on a level with a cluster of unowned mana spheres beside an enemy building —
- **Tier 0:** cast at the building → flag flies, owner flips, spheres unmoved; a second (rival or self-restack) weak possession can re-flip it. Hear sound 40 (cast) + chime 4 (claim).
- **Tier 1:** same claim **plus** spheres within ~15 tiles of the impact drift toward it and merge into one within a few seconds.
- **Tier 2:** building flips and **locks** — a subsequent tier-0/1 possession on it does nothing (only another tier-2 forced claim re-takes it).
