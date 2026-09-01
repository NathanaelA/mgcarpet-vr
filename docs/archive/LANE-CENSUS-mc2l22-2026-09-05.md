# ROUND 105 — THE mc2l22 ALL-LANE CENSUS (main session, 2026-09-05)

The player's steer for this round: **"huge jump when missing lanes are identified.
Let's build on that." — hunt missing lanes DIRECTLY instead of tripping over them.**

Instrument: `MGC_RAW_SHADOW=1 MGC_RAW_SHADOW_ALL=1 verify-deltas recordings/mc2l22.mgcr`
(7m33s, whole take, 65,556 pairs). Raw output:
`/tmp/claude-1000/wave105/main/l22-shadow-all.txt`.

**91 lanes are RECORDED. Only 18 are graded unconditionally + 3 conditionally.
So 70 recorded lanes are invisible to the pair census and to `replay`.**

## ⚠ FIRST RESULT: THE CENSUS'S OWN BIGGEST LANE WAS AN ARTIFACT
```
(9,9) max_life: 577,985 rows   e.g. retail -1  port 4294967295
```
**577,985 of 843,608 rows = 68.5% — the same 32 bits rendered signed on one side
and unsigned on the other.** `RetailEntMc2::max_life` is `i32`; the port's
`Ent::max_life` is the `u32` the importer *deliberately* seats bit-preserving
(retail's lightning trail stamps maxLife to -1, `sub_66750` EF:58336-43).
Round 104 banked this as "a display artifact"; round 105 measured what leaving it
in the table costs. **FIXED in `retail_ent_lanes_mc2` (`as u32 as i64`),
instrument-only, with a corpus-sweep + fixture-suite neutrality proof.**
⭐ MC1's twin needs no change — `RetailEntMc1::max_life` is already `u32`. Checked.

## THE CENSUS WITH THE ARTIFACT REMOVED — 265,623 rows, ranked by LANE
```
   115512 rows  70 families   next16     <- THE TILE CHAIN
   112048 rows  68 families   prev18     <- THE TILE CHAIN
    12020 rows   7 families   target96
    10459 rows   9 families   roll
     6165 rows   8 families   f2e
     2037 rows   4 families   scratch10
     1685 rows   8 families   f2c
     1631 rows   3 families   mail4.src
     1374 rows  36 families   f36
      895 rows   2 families   dest_y
      878 rows   2 families   dest_x
      363 rows   1 family     phase3e
      130 rows   3 families   mail4.amt
```

## ⭐⭐⭐ LEAD 1 — THE TILE CHAIN IS RUNNING BACKWARDS (227,560 rows, 86% of the census)
```
(10, 0) next16: 47301 rows across 919 slots   e.g. t=65 slot 648: retail 339  port 0
(10, 0) prev18: 47015 rows across 917 slots   e.g. t=65 slot 648: retail 0    port 339
```
**Read those two lines together: at t=65 slot 648 retail holds `next=339, prev=0`
and the port holds `next=0, prev=339`. The link is EXACTLY REVERSED.**
Same shape on `(9,9)`, `(5,9)`, `(10,23)`, `(10,45)` — **70 of the take's families.**

This is not cosmetic. The importer's own comment says so:
> "Chain order is load-bearing: the first-hit probes return the first admissible
> entity in walk order (mc2l3 t=268: the possession bolt's endpoint cell held
> firebug 152 → sphere 137 → sphere 241; retail claims 137, the old ascending
> rebuild put 241 at the head and claimed it instead — the first divergent pair)."

And the campaign already banked the general form (round 68): **RETAIL ASKS THE
PER-MODEL ROSTER CHAIN, THE PORT ASKED THE POOL.** The shadow module's own comment
says **"chain ORDER is where every membership law this campaign has found landed
first."**

The importer rebuilds the chains correctly from the recording (head-insertion in
reverse, `conformance.rs` ~2455-2490). So the divergence appears **within ONE TICK
of a perfect import** ⇒ the defect is in the port's **per-tick relink**
(`Gen::link`), not in the import.
⚠ Note `for head in 1..n` in that rebuild — **the slot-0 skip again** (slot 0 is a
law surface, 3 instances). Do not assume; measure.
⚠ Also note `mapEntityIndex_15B4E0` (the per-tile HEAD array) is NOT in the
recording — so a "the heads are unrecoverable" refutation is available and must be
disproved rather than assumed.

## ⭐⭐ LEAD 2 — `target96`, 12,020 rows across 7 families
```
(10,23) target96: 7159 rows across 815 slots  e.g. t=1094 slot 843: retail 44546 port 0
(10,79) target96: 4636 rows
```
**Round 101's single biggest win (−118 mc2l22 segments) was a `target96` law on an
ungraded lane.** This is the same lane on different families, still untouched. The
port leaves it at 0 where retail carries a value.

## ⭐ LEAD 3 — `roll`, 10,459 rows across 9 families
```
(9, 1) roll: 3590 rows across 619 slots t=14..64949  e.g. retail 0    port 7
(5, 2) roll: 3208 rows across  20 slots t=1198..6132 e.g. retail 1579 port 1892
(9, 9) roll: 1708 rows across 547 slots             e.g. retail 0    port 334
```
`(9,1)` and `(9,9)` are **retail 0 / port non-zero** — the port writes a roll retail
never writes. That is the "a port-side register retail does not have is a law
surface" class (round 99). `(5,2)`'s roll is *correlated with its `f2e`* (4,219
rows, same 20 slots, same t=1198..6132 window) — one family, two lanes, one law.

## ⭐ LEAD 4 — clean-ratio and clean-shift tells (round 103: a clean ratio means a
   missing MULTIPLIER, not a state bug)
```
(15,23) d88:       retail 50000     port 300000   <- EXACTLY 6x
(10,54) scratch10: retail 26214400  port 20       <- 26214400 = 400 << 16
(10,77) f2c:       retail 0         port 100      <- 440 slots
```

## LEAD 5 — the wizard block
```
wiz 0 xp_vol: 8934 rows t=14..64803 across 12 idx  e.g. retail 0 port 1
wiz {1,3,4,5,6,7} hate[0] ALL diverge at exactly t=10056:
   retail 40921 / 40911 / 40916 / 40920 / 40910 / 40927   (a 17-wide cluster)
   port   65391 / 24607 / 24809 / 32086 / 24607 / 65535   (scattered)
```
Retail writes a near-uniform value into EVERY rival's hate-toward-the-human on one
tick. **A single event the port does not replicate** — small row count, but a
one-tick law with an obvious witness.

## LEAD 6 — the recycle stack
```
recycle stack: 2 / 65556 boundaries mismatched
  t=38224: len retail 0 / port 10, top retail None / port Some(789)
```
The free stack is **0 / 65556** (the allocator is bit-exact, re-confirmed a 4th
time). The RECYCLE stack is not: the port holds 10 entries where retail holds none.

## WHAT IS *NOT* HERE (refutations, so nobody re-digs them)
- **`free stack: 0 / 65556`.** ⛔ THE ALLOCATOR IS BIT-EXACT. 4th confirmation.
- **OBJECTIVE BOARD: 0 mismatches over 65,556 boundaries.** Clean.
- `(10,0) z`: only **12 rows, t=20932..33739** — so the t=15668 `(10,0) z` head
  (dig W1-A) is genuinely INHERITED, not a per-tick z law. Consistent.
