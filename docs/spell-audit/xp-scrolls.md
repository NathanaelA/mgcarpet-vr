# MC2 Experience Scrolls — fidelity audit (general note 3)

Cites: **EF** = `reference/remc2/remc2/engine/EventsFunctions.cpp`, **EV** = `Events.cpp`,
**Snd** = `Sound.cpp`, **SIG** = `SoundInGameIndexes.h`. Port cites are `crates/…:line`.
Method: recorded gameplay senior, vendored decompile reference.

## TL;DR

- **Entity:** the XP scroll is **class 14, model 5** (creator `sub_51610` EF:37370, sprite
  set 280, actionIndex `0x0A`). Its per-tick handler is **`UpdateScroll_59C80`** (EF:41158,
  strE0[0x0A]=0x23AC80, EF:1937). This is a distinct object from the class-15 spell-token jars
  and from the archer/switch "scrollN" event-cases (those are mislabeled comments in EV).
- **Distribution law (retail, DISCONFIRMS the player's "equal-split" model):** a scroll adds the
  **FULL** `countXP` to **every spell the player OWNS** (`SpellEnabled[i] != 0`) — it is **not**
  split, not round-robin, not fractional. `countXP = 4` single-player, `50` multiplayer
  (`UpdateExperience_6E090`, EF:44262). The only per-spell exception is the **castle spell
  (index 2)**, whose volatile XP is clamped to **7**.
- **Sound:** id **63 = `SpellUp_63`** (SIG:67) — a short "spell level-up" jingle (matches the
  player's "flutes playing a few tones"), played at the picking player via
  `PrepareEventSound_6E450(playerIndex, -1, 63)` (EF:41184).
- **Current port:** the (14,5) scroll IS recognized, detects player overlap, **emits `snd(63)`**,
  sets the consumed flag, and **counts the pickup in `mc2_scrolls`** — but **awards ZERO XP**
  (`mgc-sim/src/mc1/world.rs:4472-4482`; explicitly "banked … until the Phase-4.2 XP system").
- **GAP:** no XP is granted at all (so the player's "so small it's hard to verify" is actually
  "none"). Sound 63 *is* requested by the sim; if it is inaudible that is a bake/playback-layer
  issue, not a missing call.

---

## 1. The entity, its handler, XP amount, and sound

**Entity identity.** MC2 map objects in class 14 share creator `sub_514E0(pos, model, actionIndex,
sprite)` (EF:37315). The scroll's concrete creator is `sub_51610` (EF:37370, addr 0x232610):

```c
type_entity_0x6E8E* sub_51610(axis_3d* position){          // EF:37370
    event = sub_514E0(position, 5, 10, 280);               // model 5, actionIndex 0x0A, sprite 280
    if (event){
        SetEntityShiftRot_49EA0(event, 768, 1280);         // pickup AABB extents
        if (setting_38545 & 4) event->…byte[0] |= 1u;       // XP-disabled → hide
    }
}
```

The class-14 action table `x_DWORD_D4C52ar_strE0` (EF:1926) maps **actionIndex `0x0A` → `0x23AC80`
= `UpdateScroll_59C80`** (EF:1937; cf. index 6 = riser `sub_59F60`, index 7 = cave pillar). So the
XP scroll = **(class 14, model 5)**, sprite 280.

**Handler `UpdateScroll_59C80` (EF:41158, addr 0x23AC80):**

```c
int UpdateScroll_59C80(type_entity_0x6E8E* entity){
    if (setting_38545 & 4){ entity->…byte[0] |= 1u; DisableEntityDrawing04_57F10(entity); }
    else {
        entity->position.z = getTerrainAlt_10C40(&entity->position);           // pin to ground
        for (e2 = dword_38519; e2 > Entities[0]; e2 = e2->next){               // active-entity list
            if (!e2->model_0x40_64 && e2->life_0x8 >= 0){                       // model 0 = a player body, alive
                if (sub_106C0(e2, entity)){                                     // AABB overlap
                    int playerIndex = e2->…playerColorIndex_0x38_56;
                    if (playerIndex == LevelIndex_0xc){                         // the local human only
                        int countXP = (setting_byte1_22 & MULTIPLAYER_MODE) ? 50 : 4;
                        UpdateExperience_6E090(&e2->…str_611, countXP);        // ← the distribution
                        PrepareEventSound_6E450(playerIndex, -1, 63);          // ← SpellUp_63
                        if (MULTIPLAYER_MODE) sub_6DBD0(); else sub_6DB50(0,1); // relevel/level-up notices
                    }
                    DisableEntityDrawing04_57F10(entity);                      // consume (hide)
                }
            }
        }
    }
    return 1;
}
```

- **XP amount:** `countXP = 4` single-player, `50` multiplayer.
- **Sound:** `PrepareEventSound_6E450(playerIndex, -1, 63)` → `SpellUp_63` (SIG:67). In
  `Sound.cpp` case `SpellUp_63:` (Snd:6407) it is a queued one-shot (`playType=1`, `EntitySounds_F4FE0[63]`),
  sample record index 63 in the SFX bank — a musical "level-up" jingle. `sub_6DB50(0,1)` then walks
  all 26 spells (`sub_6D9C0`) to apply any resulting level-up (that is where any *further*
  level-up chrome/notices come from), it is **not** a second XP grant.

## 2. RETAIL distribution law (the exact algorithm)

`UpdateExperience_6E090` (EF:44262) is the entire law:

```c
void UpdateExperience_6E090(type_str_611* spells, int countXP){    // EF:44262
    for (int i = 0; i < 26; i++)
        if (spells->SpellsEnabled_0x333_819x.SpellEnabled[i])       // spell i is OWNED (entity id != 0)
            spells->spellsExperience_0x2CB_715x.at(i) += countXP;   // +FULL amount to each
    if (setting_byte2_23 >= 0 && spells->spellsExperience[castle] > 7)  // castle (idx 2) clamp
        spells->spellsExperience[castle] = 7;
}
```

**Conclusions:**

1. **Not a split.** Every owned spell receives the **full** `countXP` (4 sp / 50 mp) — there is no
   division by the number of spells, no round-robin, no fractional carry. The player's "distributed
   EQUALLY (round-robin or fractional)" hypothesis is **DISCONFIRMED**; the truth is "flat +N to
   each owned spell."
2. **"Owned", not "below max".** The filter is `SpellEnabled[i] != 0` = the spell's manifestation
   entity id (i.e. the player has that spell in the book). There is **no** explicit "not already at
   max level" filter — the only cap is the castle-spell clamp below. Because XP simply accumulates
   and level is *derived* from thresholds (`xpos1` ladder, see `mc2-spell-xp.md`), spells already at
   max just keep accumulating harmlessly. So the player's "goes to all spells not at highest level"
   is *approximately* the visible effect (maxed spells show no change) but the *mechanism* is
   unconditional +N to all owned.
3. **Target is VOLATILE XP.** It writes `spellsExperience_0x2CB` (this-level accumulator), not
   `SpellExperience_0x263` (banked). Effective XP = banked + volatile (`mc2-spell-xp.md` §0).
4. **Castle cap:** the castle spell (index 2) volatile XP is clamped to **7** when
   `setting_byte2_23 >= 0` (the single-player / non-special default; `< 0` disables the clamp).

## 3. CURRENT PORT behaviour

`mc1/world.rs::mc2_class14_tick` case `10` (world.rs:4472-4482):

```rust
10 => {
    let (x, y) = (self.g.ent[i].x, self.g.ent[i].y);
    self.g.ent[i].z = self.g.ground_z(x, y) as i16;          // ground-pin ✔
    let (px, py, _) = self.human_pose;
    let e = &self.g.ent[i];
    let wrap_d = |a,b| ((a.wrapping_sub(b)) as i16 as i32).abs();
    if wrap_d(px, e.x) < e.f80 as i32 && wrap_d(py, e.y) < e.f82 as i32 {  // AABB overlap ✔
        self.g.snd(63, i);            // sound 63 requested ✔
        self.g.mc2_scrolls.0 += 1;    // pickup COUNTED (Mc2Quiet), NOT awarded  <-- BUG
        self.g.ent[i].flags |= 0x400; // consumed ✔
        self.entities_dirty = true;
    }
}
```

- The scroll is spawned/sprited correctly (world.rs:4425-4440 mirrors `sub_51610`: sprite 280,
  extents 768/1280).
- **No XP award.** `UpdateExperience_6E090` is **not** ported here; the pickup only bumps the
  `mc2_scrolls` counter (`mc1/features.rs:674`, a `Mc2Quiet` debug tally) and the doc-comment at
  world.rs:4446 says XP is "banked … until the Phase-4.2 XP system". Phase 4.2 has since landed
  (`mc2_award_xp`, cast.rs:330) but this pickup was never rewired to call it.
- Sound **is** emitted (`snd(63)`); the sim-side call is present and faithful.

The award plumbing that *should* be used already exists: `mc2_award_xp(owner, spell, amount)`
(cast.rs:330) writes `mc2_book.xp_vol[spell] += amount` and re-levels; `mc2_relevel` already applies
the castle clamp `if xp_vol[2] > 7 { xp_vol[2] = 7 }` (cast.rs:285-286). Ownership = `mc2_book.ent[s]
!= 0` (the exact analogue of `SpellEnabled[i]`).

## 4. GAP

| aspect | retail | port | gap |
|---|---|---|---|
| pickup detect / consume | AABB, hide | AABB, flag 0x400 | none |
| **XP grant** | +4 to **every owned spell** (volatile), castle clamped 7 | **none** (counter only) | **MISSING — the whole mechanic** |
| sound | `SpellUp_63` at player | `snd(63)` at scroll pos | call present; audibility unverified (see §6) |
| level-up notices | `sub_6DB50(0,1)` post-award | n/a (no award) | follows from the XP gap |

## 5. FIX DATA (exact)

In `mc2_class14_tick` case `10`, on overlap (single-player), replace the counter bump with the
distribution:

```
countXP = 4                       // single-player (50 if MULTIPLAYER_MODE — not our target)
for s in 0..26:
    if mc2_book.ent[s] != 0:      // spell s is owned  (== SpellEnabled[i] != 0)
        mc2_award_xp(PLAYER_TARGET, s, countXP)   // += xp_vol[s], relevels, castle-clamps
```

Notes:
- `mc2_award_xp` already: writes volatile XP, re-levels, and clamps `xp_vol[2]` (castle) to 7 —
  matching `UpdateExperience_6E090` exactly for the single-player path. No separate clamp needed.
- Keep `snd(63, i)` (correct: `SpellUp_63`) and `flags |= 0x400` (consume). Emit the sound **once**
  per pickup, after the award (retail order: award → sound → relevel).
- Retail plays the sound / awards at the human only (`playerIndex == LevelIndex`); the port already
  tests against `human_pose`, so single-player is faithful. Rivals do **not** collect these scrolls
  in retail (`UpdateScroll` gates on `playerColorIndex == LevelIndex`) — do not extend to rivals.
- `setting_38545 & 4` (global XP-disable) hides the scroll and is a dev/mode gate; can stay unmodeled
  unless the XP-disable option is wired.
- The `mc2_scrolls` counter can remain as a debug tally alongside the real award.

**Sound:** id 63 (`SpellUp_63`) is the correct pickup jingle. If it is inaudible in-game, the fix is
in the **bake/playback layer**, not the sim — verify the MC2 SFX bank baked record index 63 exists
and the app maps sound-event id 63 → that sample (the sim call is already correct).

## 6. Confidence, open questions, test

**Confidence: HIGH** on entity identity (class 14 / model 5), handler, XP amount (4 sp / 50 mp),
distribution law (full-amount-to-every-owned, not split), castle clamp (7), and sound id (63) — all
directly from the decompile with verbatim cites, and the port state is read directly. The player's
"equal split" is confidently DISCONFIRMED.

**Open questions:**
1. **Missing-sound root cause.** The sim emits `snd(63)`; whether the baked MC2 SFX bank contains
   record 63 and the app's sound-event map routes id 63 to it is unverified here. Needs a
   bake/playback check (the WAV/TAB record name for index 63 would confirm the "flute" sample).
2. **`setting_byte2_23` sign in single-player.** Assumed `>= 0` (castle clamp active), consistent
   with the port's unconditional `xp_vol[2] > 7` clamp. Low risk; not separately confirmed.
3. **`dword_38519` list membership.** Retail scans the `dword_38519` active-entity chain and filters
   `model==0` = player bodies; the port tests only `human_pose`. Equivalent in single-player.

**Suggested test:** on an MC2 level with an XP scroll (14,5) and the player owning ≥2 spells not yet
maxed, record `mc2_book_view().xp` before and after pickup. Expect **every owned spell's volatile XP
to rise by exactly 4** in one hit (castle capped at 7), the scroll to vanish (flag 0x400), and one
`SpellUp_63` sound event. A regression fixture can assert the +4-to-all-owned delta and the castle
clamp directly against `mc2_book.xp_vol`.

---

# RESOLUTION 2026-07-16 — the "scrolls attract fire + steal autoaim" report (FIXED)

Full Opus trace, all decompile-cited. **The banked leads were half wrong; the fix landed.**

- **Retail's victim probe ADMITS scrolls** — no class/flag exclusion exists. The (14,5) ctor keeps
  `byte[0]&8` (EF:37315/37365), a player bolt flies with the `xtype = −1` wildcard (Events.cpp:571;
  no writer in the class-9 flight range), and `sub_10780` (EF:3739) gates only on targetable-bit +
  xtype + owner + AABB.
- **The extents lead is REFUTED**: `SetEntityShiftRot_49EA0` (EF:32874) writes `array_0x52_82`
  directly — that IS the collision box `sub_10630`/`sub_106C0` sums. Our `extents(768,1280)` is
  faithful and load-bearing for pickup.
- **Autoaim is structurally blind to class-14**: the per-frame typed-list builder (EF:39969-40070)
  has no `case 0x0E`, so `sub_67CB0` can never enumerate a scroll. Our `mc2_aim_scan` (class
  3/5/10 only) is faithful — there was never a separate autoaim bug.
- **The real divergence**: retail's fireball probe is ≈0-box (sprite 340 `speed_6 = 0`), scans
  ring 0 only (`AddE7EE0x_10080(0, (255)>>8)`), once, at the post-move endpoint (EF:63127-28) —
  it practically never reaches the scroll's 768/1280 PICKUP box. Our deliberate anti-tunneling
  probe (`victim_scan`'s `r.max(1)` 3×3 + the ≤128-unit chord-march, mc2/proj.rs) DOES reach it —
  producing both "fireballs detonate on scrolls" and the mid-flight interception that looked like
  "scrolls steal autoaim".
- **FIX (landed)**: `victim_scan` (mc1/combat.rs) skips class-14 map objects — observable-parity
  with retail (documented over-correction: retail could in principle same-cell-detonate on a
  scroll; the guard never will). MC1 has no class-14; goldens hold. The possession whitelist
  (`claim_admits`) is untouched.
- **F6 side-question SETTLED: retail's victim probe admits mana spheres** (the (10,39)/(10,57)/
  (10,40) ctors keep `byte[0]&8`, EF:36607/36631/36658) — their small boxes keep over-catch mild;
  the port stays faithful (spheres NOT excluded). Offensive autoaim never scans the sphere list
  (only the possession branch does) — also already faithful.

(Note 2026-07-16, player-confirmed: the earlier **Open questions** above are all resolved — the
missing pickup sound was the deep review's missing-sound finding, since landed. With the class-14
targeting guard this file's ledger is CLOSED.)
