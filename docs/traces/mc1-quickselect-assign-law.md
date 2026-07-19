# MC1 QUICKSELECT KEY → SPELL ASSIGNMENT LAW — Verbatim Trace (remc1 / remc1hw)

Citations are `file:line` in `/home/rain/projects/mgcarpet/reference/remc1/sub_main.cpp`
(MC1) and `/home/rain/projects/mgcarpet/reference/remc1hw/sub_main.cpp` (Hidden Worlds).
Every conclusion is marked **CONFIRMED** (with a citation) or **INFERRED**.

Player report being verified: *"In retail MC1, collecting a NEW spell automatically assigns
the first free quickselect key (1-9) to it. The port doesn't do this."* — **CONFIRMED**, with
the exact mechanism traced below.

---

## 0. TL;DR

- The quickselect bank is a **per-player `int8 var_772[24]` position→list-slot map**
  (`var_15198_1875_772`). Keys `1..9,0` address positions `0..9`.
- Positions do **not** hold spell ids directly — they hold an **index into the ordered
  owned-spell list** `var_532[24]` (`var_14958_1635_532`), which in turn holds an entity index
  → the class-12 spell manifestation → spell id.
- On a **new-spell pickup** the engine: (a) adds the spell to the first free `var_532` slot
  `v24`, (b) **auto-equips it to the LEFT hand** (`var_940 = v24`), and (c) **assigns the
  FIRST FREE quickselect position** — it scans `var_772[0..9]` for the first `== -1` and writes
  `v24` there, capped at 10 positions. **CONFIRMED** `remc1:64817-64867`.
- Picking up a jar for a spell you **already own** does **nothing** (no re-assign, no re-equip):
  the same scan finds the spell already in `var_532`, sets `v25=1`, and the whole block is
  skipped. **CONFIRMED** `remc1:64831,64843`.
- **Starting spells are pre-assigned at level init**: the per-player rebuild wipes `var_772`,
  then for each owned spell (in canonical order `byte_99B88`) sets `var_772[i]=i`, and sets the
  two hands `var_940`=first-owned, `var_944`=second-owned. **CONFIRMED** `remc1:49141-49259`.
- **Manual rebinds do NOT persist across levels**: the level-init rebuild resets `var_772` to
  the canonical identity map every level. Within a level a manual rebind survives (auto-assign
  only fills free `-1` slots and won't clobber it). **CONFIRMED** `remc1:49189-49254` + `64858`.
- **Both mouse hands share ONE quickselect bank.** MC1 has two hands — left (`var_940`) and
  right (`var_944`) — both drawn in the HUD (icons at x=510 / x=574) and both indexing the same
  `var_532` list via the same `var_772` positions. The *command* chooses which hand receives the
  selected spell (action `0x18`=left, `0x19`=right). **CONFIRMED** `remc1:26459/26464`, `48747-48789`.
- **HW delta: none.** `byte_99B88`, the pickup handler, and the action processor are structurally
  and value-identical. **CONFIRMED** (canonical order byte-identical `remc1:5752` vs `remc1hw:4381`;
  pickup `remc1hw:61076-61089`).

---

## 1. The persistent per-player state

Per-player struct `str_13323[p].str_1103` (aka the wizard entity's `var_u32_29955_160`):

| field | decl | meaning |
|---|---|---|
| `var_14958_1635_532[24]` | `int32[24]` | **ordered owned-spell list** — each slot = entity index of a class-12 spell manifestation, `-1` = empty. |
| `var_15198_1875_772[24]` | `int8[24]` | **the quickselect bank**: position → index into `var_532`, `-1` = unbound. Positions 0..9 = keys `1..9,0`. |
| `var_u16_2043_940` | `uint16` | **LEFT hand** currently-selected: index into `var_532`. `255` = none. |
| `var_u16_2047_944` | `uint16` | **RIGHT hand** currently-selected: index into `var_532`. `255` = none. |

`var_676.var_u16[spellByGroup]` is the separate per-spell "do I own this" table (keyed by the
spell's group id `var_u8_29860_65`); the book/list build reads it. **CONFIRMED** `remc1:55315-55318`.

Canonical spell **display order** `byte_99B88[24]` (spell ids in book order): **CONFIRMED**
`remc1:5752-5756`
```c
uint8 byte_99B88[24] = { 0x00,0x03,0x02,0x10,0x01,0x0E,0x04,0x0C,0x06,0x09,0x07,0x08,
                         0x0F,0x12,0x11,0x13,0x0D,0x05,0x0B,0x0A,0x14,0x15,0x16,0x17 };
```
HW's copy is byte-identical (`char byte_99B88[24]` with the same 24 values). **CONFIRMED**
`remc1hw:4381-4407`.

---

## 2. Input → command → action

MC1's inputs are queued as a 2-byte "control command" `var_29715[player][0]=code, [1]=arg`
by `MakeControlCommand_188A0(code, arg)`. **CONFIRMED** def `remc1:20787`; the code-25 arm writes
`var_29715[p][0]=setting1; [1]=setting2` `remc1:20903-20908`.

The action codes relevant to spell selection, applied by the per-player action processor
`switch (var_29715[p][0])` **CONFIRMED** `remc1:48564`:

| action code | site | effect |
|---|---|---|
| `0x15` (21) | `remc1:48717` | set LEFT hand: `var_940 = arg1` (arg1 = a `var_532` index) |
| `0x16` (22) | `remc1:48725` | set RIGHT hand: `var_944 = arg1` |
| `0x17` (23) | `remc1:48733` | **manual REBIND**: clear any position holding `arg2`, then `var_772[arg1] = arg2` (arg1 = position/key, arg2 = `var_532` index) |
| `0x18` (24) | `remc1:48747` | **select-by-key into LEFT**: if `var_772[arg1] != -1` then `var_940 = var_772[arg1]` |
| `0x19` (25) | `remc1:48766` | **select-by-key into RIGHT**: if `var_772[arg1] != -1` then `var_944 = var_772[arg1]` |

The key-press site (flight/book states 0 & 4): with CTRL held (`pressedKeys[0x1D]`) and a digit
key (scancode 2..11 = `1..9,0`) down, it issues `MakeControlCommand_188A0(25, key-2)` →
action `0x19` = select the spell bound to position `(key-2)` into the RIGHT (casting) hand.
**CONFIRMED** `remc1:20340-20357`. (`0x17`/`0x18` are issued from the spellbook/rebind UI; all
three read the same `var_772` bank — this is the "both hands share the 1-9 bank" proof.)

---

## 3. New-spell pickup — the auto-assign (THE player report)

Spell-jar pickup handler (wizard collides with a class-12 spell manifestation `a1x`).
**CONFIRMED** `remc1:64817-64871`:

```c
v24 = -1;                                   // will become the first free var_532 slot
for (v16=0; v16<24; v16++) {                // scan the owned list
    v18 = base + 164 * var_532[v16];
    if (v18 <= base) { if (v24==-1) v24 = v16; continue; }   // empty slot → remember first free
    if (ent[v18].class==12 && a2==ent[v18].group) { v25=1; break; } // ALREADY OWN this spell
}
if (v25 || v24 == -1) goto SKIP;            // already own it, OR no free list slot → do nothing
...
var_532[v24] = (a1x - entBase);             // add spell to list slot v24         (:64854)
var_940      = v24;                         // AUTO-EQUIP to the LEFT hand        (:64855)
v22y = 0;
while (var_772[v22y] != -1) {               // scan the quickselect bank for
    if (++v21 >= 10) goto DONE;             //   the FIRST FREE position (cap 10) (:64858-64865)
    v22y++;
}
var_772[v22y] = v24;                        // ASSIGN it that quickselect key      (:64867)
```

So, precisely:
- **New spell → first free quickselect position.** Scan order is position `0 → 9`
  (key `1` first, key `0` last). **CONFIRMED** `remc1:64858-64867`.
- **Cap = 10.** If all 10 positions are taken, the pickup still happens but **no key is
  assigned** (`v21 >= 10` breaks out before the write). **CONFIRMED** `remc1:64864`.
- **New spell is also auto-equipped to the LEFT hand** (`var_940 = v24`). **CONFIRMED**
  `remc1:64855`. (Not strictly a "quickselect" fact but it is the same pickup event.)
- **Already-owned jar = no-op.** `v25=1` skips the entire assign/equip block. MC1 spells are
  permanent singletons — there is no second copy to stock. **CONFIRMED** `remc1:64831,64843`.

HW is byte-for-byte the same handler: `var_940 = v24` at `remc1hw:61077`, first-free `var_772`
scan `remc1hw:61081-61089`. **CONFIRMED** — no HW delta.

---

## 4. Level init — starting spells pre-assigned; rebinds reset

The per-player level-init rebuild (`do{…}while` over all players) **CONFIRMED** `remc1:49141-49265`:

1. `memset(var_29715[p],0,10)` clears queued commands `remc1:49143`.
2. `var_532[0..23] = -1` `remc1:49189-49196`; `var_940 = var_944 = 255` `remc1:49198-49200`.
3. For `v11` over `0..23` in **canonical order** `v14 = byte_99B88[v11]` `remc1:49215`: if the
   player owns spell `v14` (`v13=1`), append it — `var_532[v24x] = v14`, `var_772[v12] = v12`,
   and set the hands: first owned → `var_940 = v12` (`==0`), second owned → `var_944 = v12`.
   **CONFIRMED** `remc1:49243-49258`.

Consequences:
- **Starting spells ARE pre-assigned** to keys `1,2,3,…` in canonical `byte_99B88` order, and
  the first two owned spells auto-equip to the left/right hands. **CONFIRMED**.
- **Manual rebinds do NOT persist across levels.** This rebuild runs at every level init and
  resets `var_772` to the canonical identity map (`var_772[i]=i`), discarding any in-level
  `0x17` rebind. **CONFIRMED** (`var_772` fully overwritten `remc1:49216,49254`). Within a level
  a rebind survives, because the pickup auto-assign only fills `-1` slots. **CONFIRMED**.

Note the two "orders" differ but are self-consistent: **level init** lays spells out in
canonical `byte_99B88` order; **in-level pickup** appends to the first free slot (acquisition
order). Both surface to the player as "the number key next to the spell's badge in the book."

---

## 5. Two-hand HUD confirmation

The HUD draws two spell icons, both indexing the shared owned list via the two hand selectors —
left at x=510 (`var_532[var_940]`), right at x=574 (`var_532[var_944]`). **CONFIRMED**
`remc1:26459/26464` (HW `remc1hw:25003`). This is why one 1-9 bank serves both hands: the key
press selects a `var_532` slot; the action code (0x18 vs 0x19) routes it to a hand.

---

## 6. PORT WIRING (crates/mgc-app, crates/mgc-sim)

### 6.1 Where the bank lives today — app-side, NOT sim-hashed. **CONFIRMED**
- `App.quick_binds: [Option<u8>; 10]` — one spell id (or None) per key `1..9,0`.
  `crates/mgc-app/src/main.rs:856`, init `[None;10]` at `:981`.
- It is pure app state; the sim hash never sees it. **Auto-assign belongs entirely here — it
  cannot move MC1 goldens.** (The port stores the spell id directly, unlike retail's
  `var_532`-index indirection; semantically equivalent.)

### 6.2 Current key handling (manual bind only; no auto-assign)
- `crates/mgc-app/src/main.rs:1696-1738`: digit down → in the book, bind `self.hovered` to the
  digit (dedupes the spell out of other slots, `:1720-1725`); in flight, equip
  `quick_binds[d]` (Shift = right hand, `:1728-1734`). There is **no** pickup-driven assign.

### 6.3 What the app can diff to detect acquisition
- `World::loadout() -> LoadoutView` (`crates/mgc-sim/src/mc1/world.rs:3629`). `LoadoutView.owned:
  [bool;24]` (`world.rs:364`) is driven by `self.player.owned[s]` (the var_676 table,
  `world.rs:3635-3637`). A per-tick diff of `owned` catches any spell that flips false→true —
  level-start grants and (if the sim models them) in-level jar pickups alike.
- `LoadoutView.left/right` (`world.rs:365-366`) already mirror the sim's `var_940/944` hand
  equips — the sim owns the hand model; the app only owns the quickselect bank.

### 6.4 The effective-scheme gate. **CONFIRMED**
- `cfg.gameplay.enhancement.spell_selector.resolve(is_mc2) -> SelectorSurfaces`
  (`crates/mgc-app/src/config.rs:448`), stored as `App.selector` (`main.rs:944,808`).
- `SelectorSurfaces.map_book` is **true exactly when the MC1 map-screen spellbook (the 1-9 keys)
  is live**: `Auto`/`Mc1`/`Mc1Mc2` on an MC1 game; always **false** under MC2 (`config.rs:449-467`).
  This is the correct gate: MC2 has no quickselect keys, so no auto-assign there.

### 6.5 Insertion points (exact)
1. **Per-tick auto-assign** — inside the post-step block
   `if let Some(w) = self.sim.world.as_mut() { … }` at `main.rs:1967-1987` (runs once per frame
   after `self.sim.step`, `:1957`). Read `w.loadout().owned`, diff against a new app field
   `prev_owned: [bool;24]`, and for each spell that flipped false→true — when
   `self.selector.map_book` — if it is not already in `quick_binds`, write it to the first free
   slot (`quick_binds.iter().position(|b| b.is_none())`, capped at 10). Then store the new
   snapshot. No hashed state touched.
2. **Reset on level load / restart** — `App::new` (`main.rs:877`, currently `quick_binds =
   [None;10]` at `:981`) and `restart_level` (`main.rs:1075`, which rebuilds the world but does
   **not** currently reset `quick_binds`). To mirror retail level-init, reset `quick_binds` and
   `prev_owned` there; the first per-tick pass (6.5.1) then pre-assigns the starting owned spells
   to keys 1,2,3… on the level's first tick. (Optionally seed in canonical `byte_99B88` order for
   exact level-init faithfulness; acquisition-order from the diff is already close.)

---

## 7. PROPOSED IMPLEMENTATION

**Data (app-side only):**
- Keep `quick_binds: [Option<u8>; 10]` (spell id per key).
- Add `prev_owned: [bool; 24]` to `App` (init all-false).

**Assignment rule (per tick, gated on `self.selector.map_book`):**
```
let owned = w.loadout().owned;                 // [bool;24]
for s in 0..24 {
    if owned[s] && !self.prev_owned[s] {       // newly acquired
        let already = self.quick_binds.iter().any(|b| *b == Some(s as u8));
        if !already {
            if let Some(slot) = self.quick_binds.iter().position(|b| b.is_none()) {
                self.quick_binds[slot] = Some(s as u8);   // first free key, scan 0→9, cap 10
            }
        }
    }
}
self.prev_owned = owned;
```
This reproduces the retail law: first-free position (scan `1→…→9→0`), cap 10, no double-bind,
already-owned pickups are inert (the `owned[s] && !prev_owned[s]` edge only fires on the
false→true transition).

**Persistence:** none across levels — reset `quick_binds` + `prev_owned` in `restart_level` and
per level load, matching retail's level-init wipe (§4). Manual in-level rebinds (the existing
`:1720-1725` path) survive within the level because auto-assign only fills `None` slots.

**Gating:** the whole block runs only when `self.selector.map_book` is true — i.e. effective
scheme = MC1 or MC1+MC2 (Auto→MC1). MC2 (and `spell_selector = mc2`) has `map_book=false`, so no
quickselect keys and no auto-assign, faithful to MC2 having no number keys.

**Not required, but same retail event (optional):** retail also auto-equips a freshly picked
spell to the LEFT hand (`var_940 = v24`, §3). That is a **sim** behavior (the sim owns the hand
model); if the port's MC1 pickup path does not already set `player.left` on acquisition, that is
a separate sim-side fidelity item and is out of scope for this app-side quickselect feature.

**Goldens:** untouched — every write is to app-side `quick_binds`/`prev_owned`; the sim hash
never observes them.
