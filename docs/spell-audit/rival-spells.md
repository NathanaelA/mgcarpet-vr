# MC2 Rival-Fidelity Spell Audit — Rebound / Invisibility / Duel / Alliance

Baseline map of four spells the player will later test in the rival-fidelity
track. Recorded gameplay is senior; the vendored decompile is reference.
Cites are `file:line` in `reference/remc2/remc2/engine/` (`EF:` =
`EventsFunctions.cpp`) unless noted. Port cites are
`crates/mgc-sim/src/mc2/*.rs`. Tier data is dumped verbatim from
`baked/assets/mc2-*/spells.bin` (identical across day/night/cave for these
four rows).

## TL;DR

| spell | idx/model | retail one-liner | per-tier lever (SPELLS.DAT `life_0x1A`) | port state | headline gap |
|-------|-----------|------------------|------------------------------------------|-----------|--------------|
| **Rebound** | 8 | deflects a whitelist of incoming spell projectiles back at the sender, costs the deflector ¼ of the projectile's mana | life `[0,0,1]` → T0/T1 = random-scatter deflect, **T2 = straight-back + 2× damage** | boolean MC1 rebound channel (`player.rebound`) | no tier (no 2×/scatter split), no mana cost, no whitelist, no re-own |
| **Invisibility** | 11 / 0xB | armed-window cloak; unseen by AI + autoaim; **self-casting a spell may break it, per tier** | life `[1,2,3]` → break-on-cast law: **T0 any cast breaks, T1 all-but-possess breaks, T2 nothing breaks** | boolean `player.invisible`, armed window only | break-on-self-cast law (`byte_0x1BF_447`) entirely absent |
| **Duel** | 14 / 0xE | fires a marker projectile, then **tethers you circling the marked rival at a locked range and drains their mana (+life at T2)** | life `[0,1,2]` → T0 tether-only, T1 +mana-drain, T2 +mana+life-drain; range = subSpell `[5170,7720,7720]` | `note_misfit(15,14)` **STUB** (gate+mana only) | everything (projectile, marker, tether, drain) |
| **Alliance** | 24 / 0x18 | launches a homing subtype-25 flyer carrying a large payload; impact fires effect **(10,74)** | life `[16,26,32]`, subSpell `[610,1100,2710]` (carried, not a 0-3 charge) | flyer spawns + flies, but impact is **(10,0)** and no payload/charge carried | wrong impact + effect (10,74) unserved/untraced |

Confidence: Rebound HIGH, Invisibility VERY HIGH (data matches the player's
own L1/L2/L3 report exactly), Duel HIGH on mechanism, Alliance MEDIUM
(the (10,74) impact effect is untraced — flagged OPEN).

---

## 1. Rebound (spell index 8, model 8)

### 1.1 Identify
- Spell index/model **8**. SPELLS.DAT row 8, 3 tiers.
- Effect state: `sub_6AA00` (EF:56721) — the class-15 effect skeleton, **direct-effect** (no `sub_6DCA0`, no projectile of its own).
- Deflection engine: `sub_68740` (EF:55221), called from the projectile→wizard collision paths (EF:58770, 58892, 62939, 63162, 63484).
- Tier fields (`baked/.../spells.bin` row 8): `life_0x1A = [0, 0, 1]`, `subSpell = [0,0,0]`, `mana = [5000, 15000, 25000]`, `word_0x18` (duration) `= [125, 251, 125]`.

### 1.2 Retail behavior + per-tier law
`sub_6AA00` (EF:56742-56751), each armed tick, writes a caster flag at
offset 0xc keyed on the tier's `life_0x1A`:
```c
v3 = subspell[tier].life_0x1A;
if (v3) { if (v3 == 1) caster.byte0xc[0] |= 0x10; }   // life==1  → "precise" rebound
else     { caster.byte0xc[1] |= 0x80; }                // life==0  → "scatter" rebound
```
When the cast ends (`word_0x2E_46<=0`) both bits are cleared:
`caster.word0xc[0] &= 0x7FEF` (EF:56734, clears 0x10 and 0x8000).

The deflection is gated on those bits at the collision site:
`if (!(victim.word0xc[0] & 0x8010) || sub_68740(proj,victim,0x2D,22)==0) …`
(EF:58892; same gate EF:62939). So **either** bit = rebound active.

`sub_68740(proj a1x, victim a2x, a3=0x2D, a4=22)` (EF:55221):
1. **Cost gate** (EF:55234): if `proj.mana/4 > victim.mana` → return 0 (can't afford, no deflect).
2. **Whitelist gate** (EF:55236-55268): deflectable only if `proj.byte_0x43_67==10` **and** `proj.byte_0x44_68 ∈ {0,1,9,0xB,0xF,0x11,22,0x43,0x47,89}` (the offensive-spell impact subtypes), **or** `proj.model==13`. Other projectiles pass through. **CORRECTED 2026-07-17:** the fiddly range branch (EF:55247-53) FAILS `0x44-0x46` (`v6>0x43 && v6<0x47 → fail`) — the earlier transcription wrongly listed 0x44-0x46 as passing; the ≥0x11 pass set is exactly `{0x11, 22, 0x43, 0x47, 89}`. PORTED 2026-07-17 (`mc2_rebound_deflect`, gated in the MC2 flyer + arrow movers; player report "does not rebound" — the engine had never been gated into any MC2 mover).
3. On success (EF:55272-55305): sound **28**; award rebound XP (`sub_6D8B0(victim,8,1)`); **drain victim `mana/4`** (EF:55274); reverse heading (`roll = yaw+180°`, pitch inverted); then the **tier split** (EF:55285):
   - **`byte0xc[0] & 0x10` set (life==1 → tier 2 only):** exact straight-back reflection (`yaw = roll`) **and `subSpellIndex *= 2`** — the returned bolt does DOUBLE damage.
   - **else (life==0 → tiers 0/1):** random scatter `yaw = roll + rand%0x2D - 22` (±~22° cone around the reversal), damage unchanged.
   - re-own the projectile to the victim (`word_0x96_150 = old id; id = victim.id; life = maxLife`) so the deflected bolt can now strike the original caster; reposition at victim + fov.

Per-tier summary (row-8 data): **T0** = scatter-deflect, normal damage, 125-tick window; **T1** = identical mechanic, but a **251-tick** window and 15000 mana (just "stays up longer / costs more"); **T2** = **precise straight-back + 2× damage**, 125-tick window, 25000 mana. (No row-8 tier has `life>=2`, so the "no-flag / no-rebound" branch in `sub_6AA00` never fires in shipped data — all three tiers rebound.)

The wider variant `sub_68740(a1x, v8x, 0x5B, 45)` (EF:63162) is used by a different (non-spell) projectile class — a ±45° scatter — not the spell-column path.

### 1.3 Current port
`crates/mgc-sim/src/mc2/cast.rs:765-767` — the fire arm sets
`self.player.rebound = true` and awards XP; expiry clears it
(`cast.rs:650, 8 => self.player.rebound = false`). That boolean rides the
**MC1** rebound channel (`mc1/world.rs:3098 14 => self.player.rebound`,
mirrored to `g.player_rebound` at `mc1/world.rs:1236`, consumed by the MC1
projectile deflect). Rivals mirror the same boolean (`mc2/rivals.rs:1799-1804`).
It does **not** read `life_0x1A`, so every tier behaves identically.

### 1.4 Gap
- No tier split → T2's straight-back + **2× damage** is missing; T0/T1 scatter cone is missing.
- No `mana/4` deflect cost on the defender; no cost-gate.
- No MC2 impact-subtype whitelist (`byte_0x43_67==10` + subtype set / self-model 13).
- No re-own of the deflected projectile to the defender (retail lets a rebounded bolt kill the original caster).
- Sound 28, XP award on deflect, ±22° cone constants (`a3=0x2D`, `a4=22`) all absent.

### 1.5 Fix data
- Row 8 `life = [0,0,1]`; flags at entity offset 0xc: `byte[0]|0x10` (life==1) / `byte[1]|0x80` (life==0); active-mask `word[0] & 0x8010`; clear-mask `word[0] &= 0x7FEF`.
- `sub_68740`: cost gate `proj.mana/4 > victim.mana → skip`; drain `victim.mana -= proj.mana/4`; whitelist above; reflect `yaw+=180°, pitch=-pitch`; if life==1 `damage*=2, yaw=exact`, else `yaw += rand%0x2D - 22`; re-own to victim; sound 28; XP `sub_6D8B0(victim,8,1)`.

### 1.6 Confidence + open questions
HIGH — `sub_6AA00`/`sub_68740` fully read, data confirms all-3-tiers-rebound.
OPEN: exact `byte_0x44_68` whitelist boundary transcription (range branch at EF:55242-55268 is fiddly — the trace `mc2-class9-flyers.md §0.8` gives the same set; re-verify one boundary before landing). The `mana*3/4` figure in that trace vs the raw `mana/4` in EF:55234/55274 — the decompile arithmetic resolves to `/4`; confirm on port.

---

## 2. Invisibility (spell index 11 / 0xB, model 11)

### 2.1 Identify
- Spell index/model **11**. SPELLS.DAT row 11, 3 tiers.
- Effect state: `sub_6B1C0` (EF:57068) — direct-effect, no projectile.
- Break-condition engine: `sub_5F7E0` (EF:60982), called from the ARM path `sub_5F7B0` (EF:60973→60979) on **every** spell the caster arms.
- Tier fields (row 11): `life_0x1A = [1, 2, 3]`, `subSpell = [0,0,0]`, `mana = [9000, 18000, 36000]`, `word_0x18` (duration) `= [181, 183, 183]`.

### 2.2 Retail behavior + per-tier law (the break-conditions)
`sub_6B1C0` first tick (EF:57083-57090):
```c
sub_6D8B0(caster, 0xB, 1);                       // award invis XP
caster.ctx.word_0x159_345 = 0;                   // clear the near-castle "safe" reveal timer
caster.ctx.byte_0x1BF_447 = subspell[tier].life_0x1A;   // <-- INVIS STRENGTH = tier's life byte
caster.byte0xc[0] |= 0x20;                        // set the invisible flag
```
It is an **armed-window** effect (lasts `word_0x18` ≈ 181-183 ticks); at
expiry (EF:57108-57109) it clears `byte0xc[0] &= 0xDF` and zeroes
`byte_0x1BF_447`. Same re-arm-extends behavior as the other channel spells
(`cast.rs` gate case `4|6|8|0xB|0xC|0xE`).

**The per-tier break law** lives in `sub_5F7E0` (EF:60987-60989), run
whenever the caster ARMS any spell (`sub_5F7B0`):
```c
s = caster.ctx.byte_0x1BF_447;                    // invis strength (0 if not invisible)
if (s < 2  ||  (s <= 2 && spellBeingCast.model != 1))
    caster.byte0xc[0] &= 0xDF;                     // BREAK invisibility (clear 0x20)
```
Resolving against row-11 `life = [1,2,3]`:
- **Tier 0 (s=1):** `s<2` true → **any** spell you cast breaks invisibility.
- **Tier 1 (s=2):** `s<2` false, `s<=2 && model!=1` → breaks on every spell **except possess (model 1)**. → Possession keeps you cloaked.
- **Tier 2 (s=3):** both clauses false → **no self-cast ever breaks it**. Fireball, possess, anything — you stay invisible.

This is an **exact** match to the player's report (L1 breaks on everything;
L2 mana-possession won't terminate it; L3 mana/fireball won't terminate it).
"model 1" = possess = the mana-drain the player called "mana".

Downstream, invisibility makes the wizard **unseen by rival AI** (perception
skips `sub_15760(target,0xB)` — `docs/traces/mc2-rivals-brain.md` ~lines
300/644/650; the AI casts spell 0xB itself, line 737) and by autoaim.

### 2.3 Current port
`cast.rs:778-781` — fire arm sets `self.player.invisible = true` + XP; expiry
clears it (`cast.rs:648, 0xB => self.player.invisible = false`). The
"unseen" half **works**: `mc2/proj.rs:961` and the autoaim scan skip
`player_invisible` targets; `mc2/morph.rs` honors it for rendering. But
there is **no `byte_0x1BF_447` strength and no break-on-self-cast at all** —
invisibility is a plain armed-window boolean that never ends early.

### 2.4 Gap
- The entire per-tier break-on-cast law is missing. Notably **T0 is wrong in the player-favorable direction**: retail T0 should drop the moment you cast anything; the port keeps it up for the whole window.
- No `byte_0x1BF_447` field on the player/rival context.
- The break check must fire from the cast-gate/arm path (`mc2_cast_gate`) against the spell's model — currently nothing consults invisibility there.
- `word_0x159_345 = 0` reset on cast is not modeled (minor; see open).

### 2.5 Fix data
- Add an invis-strength field (`= subspell[tier].life_0x1A`, values 1/2/3), set on invis first-tick, zeroed at expiry.
- In `mc2_cast_gate` (or wherever a cast is armed), after committing to arm spell `m`: if strength `s` and `(s < 2 || (s <= 2 && ent[m].model65 != 1))` → clear the player's `invisible` flag and zero the strength.
- Row 11: `life=[1,2,3]`, `dur≈[181,183,183]`, `mana=[9000,18000,36000]`.

### 2.6 Confidence + open questions
VERY HIGH — data + `sub_5F7E0` + player report triangulate perfectly.
OPEN: (a) whether an **enemy hit** also breaks invisibility (not seen in
`sub_6B1C0`; `word_0x159_345` is a near-castle safe/latch-suppression timer,
EF:5399/43711/59978 — its interaction with cloak is untraced). (b) The invis
flag is `|=`'d on the spell entity (`a1x`, EF:57090) but the clears target
the caster (`v1x`, EF:57108; `a2x`, EF:60989) — a decompiler artifact vs a
real spell-entity-vs-caster split; the port models it as a player flag,
which is the intended surface.

---

## 3. Duel (spell index 14 / 0xE, model 14)

**Duel is NOT a no-op in retail** — the cast-path trace §2.2 ("duel-marker
state; no projectile") is incomplete. It fires a projectile and runs a
tether/drain machine.

### 3.1 Identify
- Spell index/model **14**. SPELLS.DAT row 14, 3 tiers.
- Effect state: `sub_6B610` (EF:57258) — spawns a projectile **directly** via `_4A190(pos,9,7)` (class-9 subtype **7**), NOT via `sub_6DCA0`.
- Projectile impact: `byte_0x43_67=10, byte_0x44_68=26` → **impact (10,26)** (EF:57299-57301).
- Duel-marker scanner (the (10,26) entity's tick): `sub_38D80` (EF:28348) — latches nearby wizards via `word_0x7A_122`.
- Duel-link setup: `sub_5EFA0` (EF:60633, the wizard tick) @ EF:60648-60657.
- Tether/drain machine: `sub_5DE30` (EF:59888), run on the caster's player tick.
- Subtype-7 creator: `sub_4D740` (EF:34898) — model 7, behavior row 60, sprite 213, speed 384, maxLife 21 (`docs/traces/mc2-class9-low-band-creators.md`). **Not in the port's `CREATORS` table.**
- Tier fields (row 14): `life_0x1A = [0, 1, 2]`, `subSpell = [5170, 7720, 7720]` (**= duel range**, not damage), `mana = [10000, 20000, 40000]`, `word_0x18` (duration) `= [195, 395, 603]`.

### 3.2 Retail behavior + per-tier law
**Cast** (`sub_6B610`, EF:57289-57316): on first tick, `_4A190(pos,9,7)` →
subtype-7 projectile launched along the caster's aim at speed 0x2800, impact
(10,26), carrying `subSpellIndex`, owner id, and `byte_0x46_70 = caster tier`;
sound **9**. The cast then holds; a **28-tick engage timer**
(EF:57280): if `word_0x2E_46 <= word_0x30_48 - 28 && ctx.word_0x146_326 == 0`
(no opponent linked within 28 ticks) → collapse the cast (abort).

**Link formation:** the (10,26) marker (`sub_38D80`) counts down its life and
each tick scans all wizards (`dword_38523` list); the first un-latched wizard
`ix` within `marker.dword_0x10_16` range gets
`ix.word_0x7A_122 = marker`, `ix.word_0x76_118 = radix distance` (EF:28364-28375).
On that wizard's own tick (`sub_5EFA0`, EF:60643-60660): reads `word_0x7A_122`,
sets the **caster's** `ctx.word_0x146_326 = the latched wizard` (the duel
opponent link), locks the ring distance `ctx.dword_0x142_322 = clamp(dist,
1024, 3072)`, copies `word_0x14A_330 = tier`, and awards the caster duel XP
(`sub_6D8B0(caster, 0xE, 1)`).

**Tether + drain** (`sub_5DE30`, EF:59906-59948), on the caster's tick while
the duel spell stays armed:
```c
opp = Entities[ctx.word_0x146_326];
if (duelSpell.word_0x2E_46                                       // still casting
    && dist(caster,opp) < subspell[word_0x14A_330].subSpellIndex_2   // within RANGE
    && opp.life >= 0) {
    // AUTOPILOT: steer yaw toward opp, set speed to hold the locked ring
    // distance dword_0x142_322 (approach if far, retreat if close),
    // clamped to ±1.5·minSpeed.
    v10 = subspell[tier].life_0x1A;
    if (v10 >= 1) {
        if (v10 == 2)  opp.life -= opp.lifeRegen + 2;             // T2: drain LIFE
        opp.mana -= opp.manaRegen + 8;  if (<0) 0;                // T1+: drain MANA
    }
} else ctx.word_0x146_326 = 0;                                    // break the duel
```
Per-tier (row-14 `life=[0,1,2]`):
- **T0 (life 0):** forced-circle tether only, **no drain**, range 5170, 195-tick window.
- **T1 (life 1):** tether + **mana drain** (`manaRegen+8`/tick), range 7720, 395-tick window.
- **T2 (life 2):** tether + **mana + life drain** (`lifeRegen+2`/tick), range 7720, 603-tick window.
Duel ends when the opponent dies, leaves range, or the caster stops holding
the duel cast.

### 3.3 Current port
`cast.rs:845-847` — the fire arm is `4 | 0xD | 0xE => self.g.note_misfit(15, spell)`.
The cast **gate + mana drain** run (gate case `…|0xE if armed>0` extends the
cast, EF:60914-928 analogue), but no projectile, no marker, no tether, no
drain. Subtype 7 is absent from `CREATORS` (`cast.rs:148-167`).

### 3.4 Gap
Everything: the subtype-7 projectile launch (direct `_4A190`, like possess),
impact (10,26), the (10,26) marker scanner, the `word_0x146_326` duel-link on
the caster context, the `sub_5EFA0` link handshake + XP award, and the
`sub_5DE30` tether-autopilot + per-tier drain.

### 3.5 Fix data
- Add subtype **7** to `CREATORS`: `(7,7,384,21,60,213)` (action 7, speed 384, maxLife 21, row 60, sprite 213).
- Duel fire arm (mirror possess `cast.rs:725-738`): direct `mc2_launch` with `subtype:7, impact:(10,26)`, sound **9**, launch speed 0x2800.
- New per-context fields: `word_0x146_326` (duel opponent id), `dword_0x142_322` (locked ring dist, clamp 1024..3072), `word_0x14A_330` (duel tier), `word_0x7A_122`/`word_0x76_118` latch (on the target).
- (10,26) marker: class-10 entity, `dword_0x10_16` = scan range, life countdown, scans `dword_38519`-equivalent wizard list, latches first un-latched wizard.
- Tether: steer yaw to opponent, speed to hold `dword_0x142_322`, gated `dist < subSpell[tier]` (range 5170/7720/7720); drain `mana -= manaRegen+8` if life>=1, `life -= lifeRegen+2` if life==2.
- 28-tick engage-or-abort in the effect state.

### 3.6 Confidence + open questions
HIGH on mechanism (all four functions read end-to-end; the
`word_0x7A_122`→`word_0x146_326` handshake is single-writer, so the data-flow
is unambiguous). OPEN: (a) confirm the (10,26) entity's action state is
`sub_38D80` by model binding (inferred from data-flow — `word_0x7A_122` is
written only there, EF:28375). (b) `sub_38D80.dword_0x10_16` (marker scan
range) provenance — likely seeded from the impact carry; trace before
landing. (c) whether the duel link is symmetric (does the OPPONENT also get
tethered, or only the caster) — `sub_5DE30` only steers the holder; the
opponent appears free to flee (which is what breaks the duel).

---

## 4. Alliance (spell index 24 / 0x18, model 24)

### 4.1 Identify
- Spell index/model **24**. SPELLS.DAT row 24, 3 tiers.
- Effect state: `sub_6CD20` (EF:58039) — direct-effect, spawns class-9 subtype **25** via `_4A190(pos,9,25)`.
- Impact set on the flyer: `byte_0x43_67=10, byte_0x44_68=74` → **impact (10,74)** (EF:58068-58070).
- Subtype-25 flyer creator: `sub_4E0F0` (EF:35244) — action 26, model 25, sprite **321**, behavior row **61**, speed 384, maxLife 10, mana 50 (homing flyer, in the autoaim acquisition set).
- Tier fields (row 24): `life_0x1A = [16, 26, 32]`, `subSpell = [610, 1100, 2710]`, `mana = [10000, 18000, 30000]`, `word_0x18` (duration) `= [23, 29, 63]`.

### 4.2 Retail behavior + per-tier law
`sub_6CD20` first tick (EF:58062-58086): spawn the subtype-25 flyer, then:
```c
flyer.actSpeed += caster.actSpeed;                       // speed boost
flyer.byte_0x43_67 = 10;  flyer.byte_0x44_68 = 74;       // impact (10,74)
flyer.id = caster.id;                                    // owner (immunity)
flyer.parentId = caster.parentId;                        // <-- inherit caster's owner chain
flyer.subSpellIndex = subspell[tier].subSpellIndex_2;    // payload 610/1100/2710
flyer.byte_0x46_70 = subspell[tier].life_0x1A;           // charge 16/26/32 (NOT a 0-3 charge)
flyer.dword_0x10_16 = caster.ctx.byte_0x154_340; ctx.byte_0x154_340 = 0;   // charge counter
launch along caster facing; sound 9.
```
The flyer then flies the shared class-9 homing state (sprite 321, row 61) and,
on impact, fires effect **(10,74)** carrying the tier payload. The per-tier
levers are the carried `subSpell` (610/1100/2710) and `byte_0x46_70`
(16/26/32) — both far larger than the 0-3 "charge" of other spells, so for
alliance these encode **strength/duration/count**, consumed by the (10,74)
handler, not the twin-shot charge logic. Sound **9**, 23/29/63-tick cast
window.

~~**What (10,74) does is not established**~~ **CLOSED 2026-07-17:** the
(10,74) chain is `strA1[10][0x4A] = 0x231800 = sub_50800` (EF:36945, a
positionless one-shot executor, action 0x51) → `strA0[10][0x51] =
0x21B650 = sub_3A650` (EF:29637): a **SAME-SPECIES AREA CHARM** centered
on the struck creature — square of tile-radius `byte_0x46_70` (16/26/32),
every same-class+model creature passing `sub_3A7F0` (EF:29701: class 5
only; models 12-15/22/23/26/27 barred; m25 barred when its byte_0x46_70
set; already-charmed StageVar2 ∈ {13,14,16,17} barred; action 232 barred)
converts: sound 6, StageVar2=14, `parentId = caster`, duration
`word_0x2E_46 = word_0x30_48 = subSpellIndex` (610/1100/2710), target
cleared (mid-attack) or action → `8m+7`. **Zero damage.** Allied tick =
`sub_1E9C0` (EF:10873): fights the parent's fight, never fellow allies,
Alliance XP to the caster on engage, reverts via kind 10 on expiry/parent
death. The "convert-to-ally" inference is CONFIRMED, refined to an AREA
charm. PORTED 2026-07-17 (`mc2_alliance_convert` + `mc2_alliance_clock` +
`mc2_alliance_creature_tick`; parent rides the `mc2_allied` side table —
`id24` keeps the authored disposition; stage-HELD conversion with the
StageVar1 save/restore is deferred).

### 4.3 Current port
`cast.rs:829-842` — the fire arm calls `mc2_launch(spell, m, &DispatchArm {
subtype: 25, impact: (10, 0), charge: false }, sub, p)` + sound 9. So the
subtype-25 flyer **is** spawned, gets the owner id (`mc2_launch` sets
`id24=PLAYER_TARGET`, `cast.rs:876`) and the tier `subSpell` payload
(`cast.rs:882`). But:
- impact is **(10,0)** — retail is **(10,74)** (WRONG).
- `charge: false` → `byte_0x46_70 = life` (16/26/32) is **not** carried (`cast.rs:883-885` only sets it when charge).
- `parentId` inheritance is not replicated (`mc2_launch` sets no `parent40`).

### 4.4 Gap
- Wrong impact subtype (0 vs 74) → the alliance effect never fires; the flyer just detonates as a generic (10,0).
- Missing `byte_0x46_70 = life` carry (16/26/32) and `parentId` inheritance.
- The (10,74) handler itself is unported (and untraced).

### 4.5 Fix data
- Flip the alliance arm's impact to **(10,74)**; set `charge: true` (or explicitly carry `f71 = life`) so 16/26/32 rides the flyer; set the projectile's parent to the caster's parent.
- Subtype-25 creator already present in the port's `CREATORS`: `(25,26,384,10,61,321)` (`cast.rs:161`) — matches `sub_4E0F0` (sprite 321, row 61, maxLife 10).
- Row 24: `life=[16,26,32]`, `subSpell=[610,1100,2710]`, `dur=[23,29,63]`, `mana=[10000,18000,30000]`.
- **BLOCKER:** trace class-10 model **74**'s effect state before implementing the payoff — until then the flyer + correct impact tag can land, but the (10,74) effect is a stub.

### 4.6 Confidence + open questions
MEDIUM. HIGH-confidence facts: the launch fields, the impact (10,74) tag, the
payload/charge carries (`sub_6CD20` read verbatim). LOW-confidence: what
alliance actually DOES — the (10,74) impact effect and the subtype-25 flyer's
target-selection semantics (does it seek enemy creatures? rivals? convert
allegiance?) are **untraced**. NEXT: deep-trace `_4A190(pos,10,74)` /
class-10 model 74 and the subtype-25 flyer acquisition filter before this
spell's payoff can be certified.

---

## Cross-cutting notes
- All four are cast through the **shared** gate/effect skeleton, so rival AI
  casting inherits the same port gaps (rivals mirror the same booleans:
  `mc2/rivals.rs:1799-1804` for rebound; the AI casts invis 0xB and treats
  duel 0xE as a homing-aimed class — `mc2-rivals-brain.md`). Alliance (24)
  does not appear in the AI attack-priority tables surveyed.
- Rebound + Invisibility currently ride **MC1 boolean channels**
  (`player.rebound`, `player.invisible`) — functional but tier-blind. Duel is
  a hard stub. Alliance is the only one whose projectile actually spawns, but
  with the wrong impact.
- Data source for all tier numbers: `baked/assets/mc2-{day,night,cave}/spells.bin`
  (identical for rows 8/11/14/24 across variants); dump reproduced via the
  `spells.rs` layout (tier block = row·80 + 2 + tier·26; `life` at +24,
  `subSpell` at +0, `mana` at +4, `word_0x18` at +22).
