# Spell audit — Castle Teleport (index 10 / 0xA)

**Audit date:** 2026-07-13 · **Method:** recorded gameplay senior, vendored
decompile reference. Handler fully read; SPELLS.DAT tier fields dumped.

## TL;DR

- **Retail keys the destination law on the per-tier `life_0x1A` byte**, which is
  **0 / 1 / 2** for tiers 0 / 1 / 2 (dumped from every baked `spells.bin`, row 10).
  The three tiers are genuinely different spells:
  - **life 0 (L1):** teleport TO own castle only (no return).
  - **life 1 (L2):** toggle — cast once = go to castle (save pos); cast again =
    return to the saved pos.
  - **life 2 (L3):** CYCLE through own castle + every other player's castle, one
    step per cast (a persistent 0..8 state counter on the spell entity).
- **Every tier** lands at castle-pos offset **-448 units along `(yaw-204)&0x7FF`**
  (not the castle centre), and **zeros the caster's flight speed** (`speed_0xc_12=0`).
- **No castle anywhere** (any tier) → **random 0x4000-unit LCG hop**; sound 22 is
  played **only on a real castle teleport**, never on the random fallback.
- **Current port ignores tier entirely.** MC2 `cast.rs` 0xA calls MC1
  `cast_teleport(m,p)`, a 2-state player-scoped toggle (`teleport_return`) that
  goes to the **raw** castle centre (no -448 offset, no speed-zero) and behaves
  identically for all three tiers — exactly the player's report ("all 3 levels
  identical … L3 uncertain"). L1 (to-only) and L3 (multi-castle cycle) are unported.

---

## 1. Identify

| Item | Value | Cite |
|---|---|---|
| Spell index | **10 (0xA)** | selector list `docs/traces/mc2-spell-selector-ui.md:337`; XP tbl `mc2-spell-xp.md:115` |
| Class-15 model | **model 10** (`byte_0x40_64 = 10`) | token map `mc2-class15-spell-tokens.md:242` ("model10 teleport — EFFECT") |
| Effect handler | **`sub_6AD60`** (0x24BD60) | `EventsFunctions.cpp:56860`; dispatched at `Events.cpp:3610` |
| SPELLS.DAT row | **row 10**, `byte_0` (tier count) = **3**, `isEnabled` = 8 | dumped `baked/assets/mc2-*/spells.bin` |
| Per-tier fields | tier0 `life=0` mana 5000 · tier1 `life=1` mana 20000 · tier2 `life=2` mana 40000; `sub_spell=0`, `font=0` all tiers (CLICK-to-fire, no payload) | dump; matches `mc2-cast-input.md:244` (`[0,0,0]/[1,1,1]/CLICK`) |
| Cast sound | **22** | `mc2-player-cast-path.md:238` |
| First-tick XP | **+1 to spell 0xA** (`sub_6D8B0(parent,0xA,1)`) | `EF:56909` |

The destination selector is the tier's **`life_0x1A`** value, read fresh each
resolve tick at `EF:56910`:
`v3 = SPELLS_BEGIN_BUFFER_str[model_0x40_64].subspell[byte_0x46_70].life_0x1A;`
Because SPELLS row 10 stores life 0/1/2 in tier order, tier == life here.

State fields on the spell manifestation entity `a1x` (class-15):
- **`word_0x96_150`** — the persistent return/cycle state counter.
- **`axis_0x9A_154x`** — the saved caster position (for the L2 return jump).

---

## 2. Retail behaviour per tier (`sub_6AD60`, EF:56860-57051)

Resolve fires on the **first cast tick** (`word_0x2E_46 == word_0x30_48`,
EF:56905). `v29x` = the caster (`Entities[parentId_0x28_40]`). `v28` = success
flag, initialised **1**. XP +1 is awarded unconditionally at EF:56909 *before*
the destination logic.

Shared placement primitive (all castle branches):
```
dest = castle->position_0x4C_76
MoveEntity_57FA0(&dest, (caster.yaw_0x1C_28 - 204) & 0x7FF, 0, -448)   // -448 along yaw-204
CopyEntityPosition_57CF0(caster, &dest)                                 // relocate caster
```

### life 0 — tier 0 (L1: "to castle") — EF:56913-56931
- `word_0x96_150 = 0` (state reset, EF:56916).
- Castle = `caster.dword_0xA4_164x->CastleEntityIndex_0x3A_58`.
- Castle **invalid** → `v28 = 0` (fall to random hop).
- Castle **valid** → placement primitive; **no** saved pos, **no** return. `v28`
  stays 1.

### life 1 — tier 1 (L2: "to castle and BACK") — EF:56933-56962
- Castle invalid → `v28 = 0` (random hop).
- Castle valid, **state (`word_0x96_150`) == 1** → `CopyEntityPosition(caster,
  axis_0x9A_154x)` (return to saved pos), `state = 0`. (the "back")
- Castle valid, **state == 0** → `axis_0x9A_154x = caster.pos` (SAVE), placement
  primitive to castle, `state = 1`. (the "to")

### life 2 — tier 2 (L3: "cycle all castles") — EF:56963-57017
Loop up to 9 iterations advancing `state` (mod 9), stopping at first valid castle:
```
v27=0; v28=0
while v27<9 && !v28:
    switch(state):
      case 1:                      # skip slot
          (nothing)
      case 2..9:                   # v15 = state-2 → player color index
          if v15 != caster_own_colorIndex_0x38_56 and v15 < NumberOfPlayers_0xe:
              other = Entities[ D41A0.array_0x2BDE[v15].playerIndex ]
              if other valid: castle = other.CastleEntityIndex; goto PLACE
      default (state 0, ...):      # own castle
          castle = caster.CastleEntityIndex; goto PLACE
    PLACE:
      if castle valid:
          placement primitive (castle pos, (yaw-204)&0x7FF, -448); v28 = 1
    state++;  if state>=9: state=0
    v27++
```
So the cycle order is: **state 0 = own castle → state 1 = skip → states 2..8 =
players 0..6** (own colour auto-skipped by the `v15 != own` guard). Each cast
lands on the next castle found; state advances once per iteration and wraps at 9.

### Random fallback (`v28 == 0`, any tier with no reachable castle) — EF:57019-57027
```
axis_0x9A_154x = caster.pos
rand = 9377*rand_0x14_20 + 9439                       # LCG advance
MoveEntity_57FA0(&axis, rand & 0x7FF, v20(=0), 0x4000)  # random 0x4000-unit hop
CopyEntityPosition(caster, &axis)
state = 0
```

### Post-relocation (EF:57028-57031)
- `caster.dword_0xA4_164x->speed_0xc_12 = 0` — **flight speed zeroed** (carpet
  stops dead on teleport). Applied unconditionally here **and again at cast
  expiry** (EF:57043).
- **Sound 22** played **only if `v28 != 0`** (a real castle teleport). The
  random fallback is silent.

**Cooldown:** none teleport-specific. `word_0x36_54` (f54) decrements at the tail
(EF:57048-50) exactly like every effect state — the shared 64-tick cooldown set
at adopt time, not a per-cast lockout. Re-cast lockout: teleport is in the
`9|0xA|0xD|0xF|0x10..0x18` LABEL_16 band (`cast.rs:557`) — **no re-arm while a
cast window is live**, but the window is short (duration = `word_0x30_48`).

---

## 3. Current port

**MC2 dispatch** — `crates/mgc-sim/src/mc2/cast.rs:772-776`:
```rust
0xA => {
    self.cast_teleport(m, p);              // ← MC1 single-destination channel
    self.mc2_award_xp(PLAYER_TARGET, 10, 1);
    self.g.snd_player(22);                  // unconditional (retail gates on castle success)
}
```
The module comment (`cast.rs:769-771`) already flags this as APPROX and names
`sub_6AD60` as the banked deep-trace.

**MC1 handler** — `crates/mgc-sim/src/mc1/world.rs:2662-2677` (`cast_teleport`):
- **Does not read `tier`/`life` at all** — same behaviour for all three MC2 tiers.
- 2-state toggle on **`player.teleport_return: Option<(x,y)>`** (player-scoped, not
  the manifestation's `word_0x96_150`):
  - return pending → jump back to saved `(rx,ry)`.
  - else → save `(p.x,p.y)`, go to `player_castle()` at the **raw castle centre**
    (`ent[c].x, ent[c].y`) — **no -448 / (yaw-204) offset**; if no castle, a
    64-tile (0x4000) LCG hop along `ent_rand(m)&0x7FF`.
- Relocation is staged via `pending_teleport`; **caster flight speed is never
  zeroed**.

Net: the port implements only retail's **tier-1 (life 1)** toggle, minus the
placement offset and the speed-zero, and applies it to every tier. This is the
player's "all 3 identical" report; **L1 (to-only, no return)** and **L3
(multi-castle cycle)** are absent.

---

## 4. Gap

| Aspect | Retail `sub_6AD60` | Current port | Gap |
|---|---|---|---|
| Tier awareness | destination law switches on `life` 0/1/2 | none (identical all tiers) | **CORE** |
| L1 (life 0) | to castle, **no return** | to-castle-then-back toggle | wrong behaviour |
| L2 (life 1) | to-castle / back toggle | ✅ matches (functionally) | offset+speed only |
| L3 (life 2) | cycle own + all rival castles | to-castle / back toggle | **unported** |
| Landing spot | castle offset -448 @ (yaw-204) | raw castle centre | may spawn inside castle |
| Caster speed | `speed_0xc_12 = 0` on teleport | never zeroed | carpet keeps momentum |
| State scope | manifestation `word_0x96_150` / `axis_0x9A_154x` | `player.teleport_return` | needs per-spell state |
| Sound 22 | only on castle success | always (incl. random hop) | minor |
| Random hop | LCG `9377*r+9439`, `&0x7FF`, 0x4000 | LCG (MC1 `ent_rand`), `&0x7FF`, 0x4000 | LCG constant differs; behaviourally ok |
| XP +1 | unconditional, first tick | unconditional ✅ | none |

---

## 5. Fix data

Write a dedicated `mc2_cast_teleport(m, p, tier)` (do not reuse MC1's
`cast_teleport` — its 2-state toggle is only life 1). Read the tier's `life` and
branch, storing state ON THE MANIFESTATION `m` (retail semantics), not the player.

**State fields (map to existing entity fields on `m`):**
- `word_0x96_150` → **`f146`** (the established port mapping — `proj.rs:254`,
  `doomsday.rs:24`, `multipart.rs:40/555`). A class-15 teleport manifestation does
  not otherwise use f146, but confirm no collision before landing.
- `axis_0x9A_154x` → the saved `(x,y,z)`. There is no single 3-axis field pair
  free on the entity for this; simplest faithful option is a small
  `player`- or `World`-side `teleport_saved: [(u16,u16,i16); ...]` keyed per
  spell entity, OR three spare entity scalar fields. (Flag-bit/field-collision
  registry caution from memory — `mc2-rivals-port` bit-29 incident — applies:
  register whatever field you pick.)

**Placement primitive (all castle branches):**
```rust
let mut dest = (castle.x, castle.y, castle.z);
Gen::polar_step(&mut dest, (caster_yaw.wrapping_sub(204)) & 0x7FF, 0, -448);
// relocate caster to dest (stage via pending_teleport as today)
```
`polar_step` (`mc1/mobs.rs:567`) already handles negative dist. `caster_yaw` =
the carpet pose heading (`p.heading`).

**Tier law:**
- **life 0:** `f146 = 0`. If `player_castle()` Some → placement to it, success.
  Else → random hop.
- **life 1:** if no castle → random hop. Else if `f146 == 1` → relocate to saved
  pos, `f146 = 0`. Else → save `p.x,p.y`(,z), placement to castle, `f146 = 1`.
- **life 2:** loop ≤9 advancing `state = f146` (mod 9): state 0 → own castle;
  state 1 → skip; state 2..8 → the `(state-2)`-th *other* player's castle. Port
  player set = the human (`player_castle()`, slot 0) + `mc2_rivals` (each
  `rival_castle(r.ent)`, `mc1/rivals.rs:356`); skip own slot and eliminated
  rivals; NumberOfPlayers = 1 + live rivals. Land on first valid castle, set
  success, `f146 = (state+1) % 9`.

**Random fallback (no castle, any tier):**
```rust
// LCG: rand = 9377*rand + 9439  (retail EF:57023 — differs from MC1 ent_rand)
let yaw = advanced_rand & 0x7FF;
let mut dest = (p.x, p.y, 0);
Gen::polar_step(&mut dest, yaw, 0, 0x4000);
// relocate; f146 = 0
```

**Post (both paths):**
- Zero the caster's flight speed (the `speed_0xc_12 = 0` — the MC2 flight
  `speed` field; `mc2-flight-model.md`). Retail does it on resolve AND expiry.
- XP `+1` to spell 10 on first tick — keep (unconditional, correct).
- **Sound 22 only on castle success** — move `snd_player(22)` inside the
  castle-teleport branch; the random hop is silent.

---

## 6. Confidence, open questions, test

**Confidence: HIGH** on the tier law and the life 0/1/2 mapping — the handler is
fully read (EF:56860-57051) and the SPELLS row was dumped from all four baked
variants (life 0/1/2, unanimous). Medium only on port plumbing choices (which
entity fields carry the saved axis; the exact "player list" iteration order vs
retail's `D41A0.array_0x2BDE` color-index walk).

**Open questions:**
1. **`axis_0x9A_154x` port home** — no free 3-axis entity slot; needs a chosen
   field or a side table (register it against field-collision).
2. **L3 iteration order vs retail** — retail walks *color index* 0..6 via
   `D41A0.array_0x2BDE[v15].playerIndex`; the port's `mc2_rivals` order may
   differ, changing which castle each successive cast lands on. Cosmetic unless a
   test pins order.
3. **Random-hop LCG constant** — retail uses `9377*r+9439` on `rand_0x14_20`; MC1
   port uses `ent_rand`. Behaviourally a random hop either way; only matters if a
   state-hash golden must match retail exactly (it can't without the byte-level
   RNG, so treat as APPROX).
4. **`v20`/pitch arg** in the random hop is `v28`(=0) — flat hop; confirmed 0.
5. Sound 22 gating (castle-only) is read straight from `if (v22) PrepareEventSound
   (...,22)` at EF:57030 — high confidence.

**Suggested test:** MC2 level with the player's own castle + at least one rival
castle. (a) Tier 0: cast twice → both land at own castle offset (no return),
carpet speed drops to 0 each time. (b) Tier 1: cast → at castle; cast → back at
origin; repeat. (c) Tier 2: cast repeatedly → cycles own → rival(s) → own …,
each landing at the -448/(yaw-204) offset, sound 22 each castle hop. (d) Cast any
tier with no castle standing → single 0x4000 random hop, **silent**. Add a
state-hash golden after landing (MC2-gated; MC1 goldens untouched).
