# Stubbed-arm re-verification — dormancy premises re-checked against the binary

Companion / successor to `docs/AUDIT-DORMANT-ARMS-2026-07-20.md` (the
prior full accounting; not duplicated here). That audit exited every
parked arm as fix | banked | FAITHFUL-NOP. This pass re-tests the
FAITHFUL-NOP / PROVEN-DORMANT premises of the form "gate never set /
state never entered / zero writers / zero readers / flee bit clear /
decompiler never lifted it", after one such premise was proven WRONG.

Motivating failure — the MC2 dragon fireball-dodge. The port dropped
`sub_1F0C0` (the class-5 m0/m3 tether) on the premise "its gate byte
`fontTypeIndex_0x3D_61` is zero from the ctor and NO recovered handler
arms it — dormant" (DEVIATIONS.md m0/m3 tether). FALSE: `sub_68BD0`
(EventsFunctions.cpp `sub_68BD0`, def EF:55454) sets
`fontTypeIndex_0x3D_61 = 32` (EF:55463) whenever a class-9 projectile's
auto-acquisition `sub_67CB0` (EF:54848) locks a class-5 m0 victim. A
grep for WRITERS of the offset fragment `_0x3D_` catches it. That gap
is already adjudicated; it is NOT re-opened here — it is the model.

Method: for each premise, resolve the gate field / state value / flag →
grep ALL of `reference/remc2/remc2/engine/*.cpp` for every writer/reader
(by hex-offset fragment to catch renamed variants; address-dispatch glue
calling subs with null args is NOT a live writer) → falsified iff a
writer/reader exists on a reachable path. Unlifted handlers dispatched
as no-ops were disassembled from `/tmp/claude-1000/NETHERW.EXE`
(file_offset = 0x34800 + (linear − 0x10000); `ndisasm -b32 -e <off>`);
`55 89E5 5D C3` (push ebp/mov ebp,esp/pop ebp/ret, 5 bytes) = genuine
empty stub. MC1 items are code-unverifiable here (no engine binary — see
§4).

Bottom line: re-verification found **no new false-dormancy premise**.
Every re-checked MC2 premise HELD, and several "retail-check pending" /
"unrecovered body" premises were CONVERTED to hard-confirmed by
disassembly. One reachable behavioral gap is confirmed but its premise
was stated honestly (§2).


## 2. FALSIFIED PREMISES (new gaps found)

**None** of the re-checked dormancy premises were falsified. The only
prior falsification is the dragon-dodge tether (§1), left as-adjudicated.

One CONFIRMED-REACHABLE gap surfaced whose premise was *honestly stated*
(the port claims "no *ported* writer", a true statement about the port,
not a false claim about retail) — logged here because retail behavior is
demonstrably missing:

- **MC2 (10,19) fire-spray singleton latch — RESOLVED (entry
  corrected, then fixed).** This entry's original claim ("the port
  ports neither the register nor a live latch") was ITSELF stale: the
  register IS ported — the summit-18 eruption controller
  (`mc2_summit18_tick`, mc2/morph.rs) latches `plume`/`erupting` (the
  `word_0x33`/`word_0x31` homes) and kills the previous column,
  mirroring EF:23962-64; only the (10,19) creation site exists
  (EF:23957 is retail's sole creator, inside that same controller).
  What WAS missing: the death release EF:24148 — the spray's tick
  never cleared `plume`, so a stale latch outlived the spray and the
  next eruption's "kill the previous column" write soft-killed
  whatever entity had re-used the slot (a silent arbitrary kill).
  FIXED: `mc2_fire_spray_tick` death arm now writes `plume = 0`
  (mc2/tail.rs). Level reset EF:39309 ≡ the fresh `Gen` per level.
  Lesson for this ledger: port-side dormancy notes go stale too —
  check ALL columns (the latch lived in morph.rs, the note in
  tail.rs).


## 3. RE-VERIFIED DORMANT (premise held)

**R1. MC2 class-5 s6 "flee" slots m15-28 never entered** (AUDIT-2026-07-20
FAITHFUL-NOP list; port `mc2/roster.rs:224/609/1575/1752/1986/2082/2358/
2667/2976/3208/3588/3704`, `mc2/multipart.rs:519/542/1268/2388`,
`mc2/mobs.rs`/`stagevars.rs:818` flee helpers). Verified THREE ways,
exhaustively:
  - *Mechanism.* State `base+6` (flee) is written ONLY inside the shared
    creature AI `sub_1BD90` (def EF:8945). All nine `a2 + 6` sites
    (EF:9004, 9171, 9228, 9462, 9497, 9512, 9521, 10466, 10857) are each
    guarded by the FLEE bit `dword_0xA0_160x->byte_160_0x20_32 & 8`
    (reads EF:9003, 9170, 9227, 9496, 9511, 9520, 10465, 10856); the
    else-branch is `base+2` (attack). No unconditional writer.
  - *No direct bypass.* The five direct literal writes
    `actionIndex_0x45_69 = 6` are class 2 dolmen (EF:33491), class 9 m6
    projectile (EF:34881), class 10 m6 (EF:35463), class 0xE scroll
    (EF:37383) and a castle (EF:61084) — none class-5, each its own
    unrelated state-6.
  - *Data.* Exactly 3 rows of `str_D7BD6[157]` carry FLEE (`byte_0x20 &
    8`): `[98]` (type 0x27/sub 0x2D), `[100]` (0x29/0x16), `[101]`
    (0x2A/0x16), plus the in-source duplicates `[147]/[149]/[150]`.
    Confirmed in the decompile (`Level.cpp` str_D7BD6 literal, byte
    field before the trailing stub) AND the auto-extracted port table
    (`mc2/behavior.rs` BEHAVIOR[98/100/101/147/149/150] flags=0x9). Those
    rows are hand-picked only by the prey ctors — goat (EF:33739 →
    row 98), townie (EF:34023 → row 101), villager (EF:34058/34115 →
    row 100). Models m15-28 use flee-clear rows (roster cites e.g. row
    73, row 84 — `Level.cpp` byte_0x20 = 0x01, no 0x8).
  - *Handlers.* The five roster arms citing MISSING subs disassemble to
    empty stubs: `sub_26420` (file 0x4AC20), `sub_28460` (0x4CC60),
    `sub_28F40` (0x4D740), `sub_29370` (0x4DB70), `sub_2B7A0` (0x4FFA0)
    — all `55 89E5 5D C3`. Even a forced entry is a no-op.

**R2. m3 state 0x1E `sub_1FA40` — empty stub** (port `mc2/multipart.rs:107/
542`, note "held inert; retail-check pending"). Disassembled file
0x44240: `55 89E5 5D C3` (5 bytes). The twin of the solved m0 state 0x06
`sub_1F2B0` — both genuinely empty. Resolves the pending retail-check.
NOTE: this clears only the STATE-6 handler; it does NOT touch the m0/m3
tether `sub_1F0C0` (the §1 case-study gap), which is a separate call.

**R3. `sub_20130` (MC2 archer base+6) — empty stub** (port `mc2/mobs.rs:94`,
"missing from the decompile and stubbed as hold-state"). Disassembled
file 0x44930: `55 89E5 5D C3`. Faithful no-op.

**R4. m27 freeze gate `x_DWORD_E9BA8`** (port `mc2/multipart.rs:108`,
"reads as 0, the normal path"). Exhaustive grep of all `*.cpp`: one decl
(EF:3473), one read `if (x_DWORD_E9BA8)` (EF:20577), ZERO writers. Always
0. Held.

**R5. doomsday-active global `word_0x36548`** (port `mc2/doomsday.rs`
module, "no reader in retail; savegame/debug only"). All refs: writers
EF:12692 (`= 1`, case 0) / EF:12873 (`= 0`, case 0xF); the only other
accesses are the save/load memcpy (`Basic.cpp:3149/3342`,
`engine_support_converts.cpp:766`). ZERO code readers. Held.

**R6. flood objects-hit counter `x_DWORD_E9B90`** (port `mc2/flood.rs`
module, "no ported reader"). All refs: reset EF:28511 (`= 0`), increment
EF:29343-45 (`v3 = E9B90+1; E9B90 = v3`). Never READ anywhere. Dead
counter (stats/debug). Held.

**R7. `mc2_spawn_building` `dword_0x10_16 = 2` — no building consumer**
(port `mc2/mobs.rs:100`). `dword_0x10_16` is polysemous (offset 0x10),
but every consumer resolves to a CASTLE: the mana-capacity ladder
`switch(entity2->dword_0x10_16)` reached via
`CastleEntityIndex_0x3A_58` (`Level.cpp:1729-1772`), the balloon-health
HUD `switch(v23x->dword_0x10_16)` (`GameUI.cpp:249`), `SetShiftByCastle_
49EC0(a1, dword_0x10_16)` (EF:4399ff). A plain spawned dwelling is not a
castle and reaches none of these. Held.

**R8. Empty match-arm sweep — all 27 non-wildcard `=> {}` in mc1/mc2
carry citations.** Beyond R1's flee arms: `roster.rs:2947` `2 => {}` =
decompile-cited no-op (m23 state-2 gate EF:18261 is strictly `== 1`);
`multipart.rs:1268` (m22 0xB6) / `multipart.rs:2388` (m27 0xDE) = no
unique body, behavior inlined in the parent `sub_27720` / `sub_298D0`
(and both are class-5 flee-offsets, R1-unreachable regardless);
`mobs.rs:2337` `233|234 => {}` = NULL table rows, body-driven via
`sub_29A90`; `mobs.rs:2852` `0xFE` / `stagevars.rs:641` `15` = inert
parking markers; `cave.rs:558` `2 => {}` = fall-through into the wave
tick (EF:22946); `mc1/mobs.rs:2772` `(13|14|15,0)` / `2841` `(13,3)|
(14,3)` = feeder idles (AUDIT-2026-07-20: feeders never self-promote).
No hidden gap.


## 4. UNVERIFIABLE / NEEDS BINARY WORK

- **MC1 engine binary absent.** The only MC1 executable present
  (`/tmp/claude-1000/mcx/CARPET.EXE`) is an 11 650-byte MZ real-mode
  launcher stub, not the flat engine — a linear address like 0x5B5D0 is
  past its end. All MC1 code-premise re-checks below need the real MC1
  engine binary + an address-map recipe (the remc2→NETHERW recipe does
  not transfer).
  - **B1** `sub_5B5D0` MC1 class-7 spawner return-0 (levels 051 @
    (29,190) / 063 @ (145,0); reachable proximity trigger, data10=1) —
    still the strongest MC1 unrecovered-body suspect; disassemble to
    classify empty-stub vs body.
  - **B6** MC1 c9 states 7/11 terminal-explosion emitters `sub_57040` /
    `sub_57800` — unreachable today (unported), disassemble when the
    castle-defense cluster lands.
  - MC1 feeder self-promotion writers for models 13/14/15 (backs
    `mc1/mobs.rs:2772`).
- **(10,19) spray (§1) severity** — the register EF:23962 is confirmed
  reachable in principle; whether normal play holds two (10,19) sprays
  concurrently (and thus how visible the missing single-instance latch
  is) needs a recorded-gameplay / playtest rating.
- **MC2 class-11 strB0 models 5-11; class-10 0x22 `sub_344A0` / 0x26
  `sub_357C0`** (AUDIT-2026-07-20) — dormancy rests on ZERO authored
  records + zero runtime spawn sites (a DATA walk, trusted per method);
  their binary bodies were NOT disassembled because spawn-absence, not
  empty-body, is the load-bearing fact. If ever a spawn site is found,
  disassemble the bodies then.
