# MC1 class-12 encoding: spell tokens, jars, manifestations

Decoded 2026-07-29 from the HW conformance recordings + remc1; this is
retail's ACTUAL class-12 scheme, which differs from the port's
(`DROPPED_JAR = 3`, `MANIFEST_BASE + spell`). The conformance importer
keeps retail's encoding raw; `World::strict_retail` selects which law
`class12_tick` applies.

## The retail encoding

`tick70 = spell*3 + phase` with `model65 = spell`, dispatched through
the class-12 state table `str_2563D8` (registry row `//c`; the per-class
registry rows at remc1 sub_main.cpp:5042-5056 are keyed by the CLASS IN
HEX in the trailing comment — `//9` bolts, `//a` class-10 effects,
`//b` triggers, `//c` class 12).

- **Phase 0 — the owned-spell TOKEN.** Every wizard carries one
  class-12 entity per ACQUIRED spell (`+42` = the wizard's pool slot,
  `+24 id` likewise). This is the acquisition list as pool entities —
  what the Type_160 hand indices (+940/944) point into. Idle tokens
  (`+48 == 0`) are inert and persist forever. An ACTIVE duration spell
  runs its per-spell phase-0 handler with `+48` counting down from
  `+50`:
  - spell 1 `sub_56270_567A0` — HEAL-over-time: target `+12 actLife +=
    5% of +8 maxLife` per tick (capped), pays `+136` mana, sound 25 on
    the first tick.
  - spell 2 `sub_56380_568B0` — SPEED: target's Type_160 `+12`
    cmd-speed = 3× `+128` max-speed on the first tick then 2×, sound 19
    at start, and a `(10,2)` ambient puff every 4th token-tick
    (`+63 & 3 == 0`) at the target's position with `id24` inherited and
    `act_life *= 4` (8→32). Expiry restores cmd-speed and clears the
    0x80 flag. THIS is the HW recording's puff emitter (an active
    speed token, counter 237, on the wizard at slot 473).
  - spell 3 `sub_56510_56A40` — emits `(9,1)` bolts (payload
    `+68/+69 = 10/12`) from the wizard.
  - other spells: per-row handlers in `str_2563D8[spell*3]`.
- **Phases 1/2 — world JARS** (`sub_56250`/`sub_56260` →
  `sub_55DB0_562E0`/`sub_55D30_56260` for every spell). Resting jars
  never self-decay.

## The port's encoding (unchanged, play mode)

Port spawns use tick70 0..=2 for THING jars, 3 = death-scatter decay,
`MANIFEST_BASE(200)+spell` for manifestations. The COLLISION that
motivated this trace: retail tick70=3 = spell-1 (heal) TOKEN, which the
port's scatter-jar decay reaped one tick after conformance import —
178/289 pairs poisoned until `strict_retail` made imported class-12
entities follow retail's law (currently: everything inert; active
token handlers unported).

## Class-10 additions that came with this

`str_255D0C[2]=sub_3A570` / `[3]=sub_3A5D0` ctors + `str_255998[2]=
sub_252B0` / `[3]=sub_253F0` ticks (bare pre-decrement family) are now
ported: (10,2) ambient puff (life 8, silent, spriteless, UNLINKED),
(10,3) smoke puff (life 7, sprite 36, linked). An imported puff used to
fall through to the terrain-feature dispatch self-kill catch-all.

## Open (next session)

- Port the ACTIVE token handlers (heal/speed/bolt emitters) under
  strict_retail — the remaining (10,2) 39 and (9,1) losses.
- Corpse cascade `sub_1A800`: `(+63 & 7)==0` → mana drop `sub_27690`
  (read its body — ball only when carried mana > 0, fresh slot), then
  the corpse slot spawns `(10,1)` (id24 inherited) and reaps. Port
  `mob_corpse` has the same shape (corpse_drop/corpse_puff) but the
  outcome differs at the boundary (port ball-on-own-slot vs retail
  puff): 57 missing (10,0) + 33 missing (10,39) per 289 pairs.
- Consider re-encoding the port's class-12 to retail's scheme outright
  (removes the dual-encoding split brain).
