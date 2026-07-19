# Spell Audit — Fool's Mana (index 22 / 0x16)

**Verdict: PORT IS WRONG.** The current port casts ONE real, collectible
mana sphere that adds to the pool. Retail casts a SHOTGUN SPREAD of SIX
neutral FAKE-mana spheres that, when an enemy wizard tries to possess
one, DETONATE against the possessor (fireball / repeated fireballs /
lightning by tier) and vanish. The trap mechanic is entirely absent from
the port, and the one thing the port does spawn (a genuine pool-adding
sphere) is the opposite of the intended effect.

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

*(Ambiguity worth flagging: the SAME `sub_36680` path runs for ANY
(10,57) claim, including the authored ground spheres from `sub_50130`
whose `byte_0x46_70` is the NewEvent default. If that default is 0, an
authored ground sphere would ALSO fire a fireball when possessed — which
would contradict the m57 trace's "collectible mana" reading. Resolve by
checking the NewEvent default of `byte_0x46_70` and whether authored
spheres are ever possessed vs only AI-flown home. OPEN-2.)*

---

## 3. CURRENT PORT — what it actually does

`mc2_spell_fire`, arm `0x16` (`crates/mgc-sim/src/mc2/cast.rs:805-811`):

```rust
0x16 => {
    let (mx, my) = (p.x, p.y);
    let z = self.g.ground_z(mx, my) as i16;
    self.g.mc2_spawn_mana_sphere(57, mx, my, z);   // ONE sphere, at the player's feet
    self.mc2_award_xp(PLAYER_TARGET, 22, 1);
    self.g.snd_player(11);
}
```

`mc2_spawn_mana_sphere(57, …)`
(`crates/mgc-sim/src/mc2/effects.rs:160-176`) rides `spawn_mana_ball`
(the MC1 ball machinery), grants `f140 = rand % 0x7D0` (0..1999) mana,
sets `f144 = 0` (neutral), `ball_resize`. The (10,57) sphere then uses
the **MC1 ball tick**, whose claim/possession path transfers ownership
(`id24`/`f144`) and makes it a **genuine collectible mana** that credits
the pool when flown to a castle (effects.rs:154-159 doc admits the
0x29/0x3E tick columns are unported/APPROX).

**Divergences from retail, all present:**
1. Spawns **1** sphere, not 6.
2. At the player's feet with no launch (`actSpeed`/spread), not a
   ±85° fan of 6 thrown spheres.
3. Never sets `subSpellIndex` payload or `byte_0x46_70` retaliation tier.
4. `parentId` is left neutral (from `spawn_mana_ball`), not the caster —
   so even the ownership/trap identity is missing.
5. **No `sub_36680` retaliation** anywhere — the sphere is a real,
   pool-adding, freely collectible mana ball. This is the exact bug the
   player described.

---

## 4. GAP

| Aspect | Retail | Port | Gap |
|---|---|---|---|
| Sphere count | 6 | 1 | missing 5 |
| Launch | ±85° fan, speed 140..407 | dropped at feet | missing spread/throw |
| Owner (`parentId`) | caster | neutral | trap identity lost |
| Fake flag | `playerEntityIndex=0`, neutral | neutral (but real pool value) | looks right, behaves wrong |
| Payload | `subSpellIndex_2` on each | none | no retaliation damage |
| Retaliation tier | `byte_0x46_70 = life` | none | no branch selector |
| Possession → | detonate + despawn | collect + add to pool | **inverted mechanic** |
| L1 | 1 fireball | — | missing |
| L2 | repeated fireballs | — | missing |
| L3 | lightning | — | missing |
| Caster XP | on trap sprung | on cast | timing wrong (minor) |

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
verbatim and match the player's tier-by-tier report exactly. The port's
single-real-sphere behavior is confirmed at cast.rs:805 + effects.rs:160.

**Open questions:**
1. **SPELLS.DAT row 22 `life_0x1A` per tier** — inferred {0,1,2} from the
   observed fireball/fireballs/lightning. Confirm from baked `spells.bin`
   row 22 (and whether tier 2 is life 2 or 3 — the retaliation branch
   `v1 <= 3` and the cast's `life >= 3` owned/colored path diverge only
   at exactly 3).
2. **(10,57) `byte_0x46_70` NewEvent default** — decides whether the
   AUTHORED ground spheres (`sub_50130`, m57 trace) also retaliate when
   possessed, or are inert. This determines whether the retaliation path
   must be gated on "was cast by fool's mana" vs universal to (10,57).
3. **`subSpellIndex_0x2A_42` field mapping on the sphere** — ensure the
   payload rides a field distinct from the sphere's mana (`f140`); the
   port's mana-sphere ctor currently owns `f140` for the mana value.
4. **The `dword_0x35 > 6` capacity gate** — port equivalent (free-slot
   count) vs safe-omit; only matters under entity-budget pressure.
5. **Homing/`word_0x96_150` + `sub_655C0` aim** onto the claimer in the
   port — confirm the port's mana-sphere/possession fields expose a
   homing-target slot the retaliation can write.

**Suggested test:** In a level with a rival wizard, cast Fool's Mana at
tier 1 near the rival's mana-collection path. Assert: (a) 6 neutral
(10,57) spheres appear in a fan, NOT 1; (b) the player's mana pool does
NOT increase and the spheres are NOT collectible-to-pool by the player;
(c) when the rival's collector/possess touches a sphere, a fireball
spawns homing the rival and the sphere despawns; (d) tier 0 → single
fireball, tier 2 → a lightning bolt instead. Add a sim-level regression:
cast tier 0, inject a possession claim onto one sphere, assert exactly
one class-9 subtype-0 projectile spawns owned by the caster and the
sphere is gone; repeat tier 2 asserting a subtype-9 (impact 10,23)
projectile. Golden re-pin: MC2 state-hash only (MC1 untouched).
