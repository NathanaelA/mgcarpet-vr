# Dormant-arm audit — full accounting (2026-07-20)

Player-mandated after the level-04 mound saga: triage every parked/
stubbed/interim arm by **can normal play reach this state, and does
retail act there?** Exit per site: fix | banked symptom | FAITHFUL-NOP
citation — no third state. Method: marker-vocabulary grep over mgc-sim
(~117 hits/18 files) triaged per site against the retail sub bodies,
PLUS a completeness backbone that walked every finite per-class
dispatch table row by row in both games (markers only find what was
commented; the tables bound what exists).

## Coverage

MC1 (`dword_96902`, remc1 sub_main.cpp :5041): family 5 `str_254DCC`
121/121; family 9 `str_25573C` 14/14; family 10 `str_255998` 62/62;
family 11 `str_256038` 32/32; family 12 `str_2563D8` 72/72; family 13
`str_256938` 4/4 (all `return 0`); families 1/4 empty tables; families
6/7/8 whole-table `return 0` handlers; family 2 `str_254C3C` 14/14;
family 3 `str_254ADC` 11/11.

MC2 (`str_D4C48ar`, EventsFunctions.cpp): str20 21/21; str30 13/13;
str50 208/208 live cells; str90 31/31; strA0 98/98; strB0 46/46;
strD0 10/10 (all NULL); strE0 11/11; strF0 15 full + 63
pattern-verified (uniform 3-state-per-model layout); str10/40/60/70/
80/C0 verified single-NULL-row empty.

Marker triage: engine (world.rs/features.rs/flight.rs/ids.rs), mc1/*
and mc2/* columns all walked; mc2/stagevars.rs pre-cleared (real
machinery).

## DANGEROUS — reachable, retail acts, we don't (fix list)

**D1. MC1 crab-egg (10,52) hatch chain — 3-stacked mound + ACTIVE
misroute.** Retail creator `sub_3B860` (:47613, registered :4539)
sets state 56 (hatch timer 600, life 100000); state 56 `sub_296A0`
(:31097) counts down then → state 57; state 57 `sub_29700` (:31120)
spawns a class-5 m5 crab + possess-flash and despawns. Our
`spawn_creator` default arm (features.rs:1933) stamps `tick70=52`,
which the world dispatch special-cases into `tick_building_live` — so
every egg (261 authored records across 30+ levels, plus every
crab-laid egg) masquerades as a LIVE VILLAGE BUILDING instead of
hatching. Crab ecology never self-sustains; eggs behave like phantom
villages. Fix = all three together: init state 56 + state-56 timer arm
+ state-57 hatch arm (hashed path — goldens).

**D2. MC1/HW m15 castle-guard brain — 2-stacked mound.** Guards ARE
spawned in normal play: castle dispatcher `sub_47400` (MC1 :56428, HW
:52492; ours features.rs:3123-3141) fields 4..34 (5,15) archers at
castle L3+. But (a) our `grid_walk` omits `sub_1FF60`'s wizard-
acquisition scan (:25733-63 → chase 0x5C), and (b) `(15,2)` falls to
generic `mob_chase` instead of `sub_201D0`'s (9,13) bolt cast
(:25848). Symptom: castle guards patrol but never engage or shoot —
rival guards harmless, own guards defend nothing. Both arms together.
MC2's m15 is unaffected (own brain, walked MATCH).

**D3. MC2 Magic-Mine trip XP dropped.** `mc2_mine_detonate`
(effects.rs:310) omits retail's `sub_6D8B0(id,23,1)` award (EF:29979).
The comment's "Gen can't reach the spellbook" is stale — the
`mc2_cast_xp` mail queue is pushed from Gen at proj.rs:709 and
morph.rs:302. One-line-class fix; feeds the spellbook → hash →
goldens.

**D4. MC1 m4 militia pair-up — 2-stacked mound (minor).** Idle
pair-up scan (:22679-83 → pack 0x1B) stubbed in `militia_idle`
(mobs.rs:1517) AND `(4,3)=>{}` (mobs.rs:2529) where retail
`sub_1BBE0` runs mob_pack. 1260 spawns; movement-cosmetic (loners
instead of escort pairs). Fix both together or not at all.

**D5. MC1 state 0x11 fire-ring damage.** `sub_25CE0` (expanding fire
ring) writes per-tick area damage via `sub_120B0`; our catch-all
despawns it. Verify the parent eruption/blast port doesn't already
emit equivalent damage; if not, small real damage gap. (Its 4 sibling
debris risers 0x0D/0x0E/0x19/0x37 are cosmetic-only.)

## Needs-ruling / banked symptoms

**B1. MC1 class-7 spawner volume.** Levels 051 + 063 each author one
(7,4) behind a REACHABLE proximity trigger; retail spawn is enabled
(data10=1, creator `sub_398C0` builds a live entity) but the tick
handler `sub_5B5D0` is a `return 0` one-liner that may be an
unrecovered decompiler stub, not a proven no-op. Banked symptom:
"unknown retail behavior when the player trips the volume at
level-051 (29,190) / level-063 (145,0)" — resolve via recorded
gameplay or a better decompile.

**B2. MC2 castle-turret shell re-aim (`sub_66D00`, EF:58559).** Lost-
lock shells re-aim at the castle's stored aim-point + HIBYTE(yaw)+=4
sidestep jink; ours use generic homing. Live (turrets player-tested),
low severity. Banked.

**B3. MC1 dolmen updraft ((2,2) state 6, `sub_49AD0` :57781).**
Overlapping wizards get flag 0x10 → 10× climb thrust — consumed only
by the AI-wizard flight handler (human brain never reads it). ~151
MC1 + ~123 HW placements. Single missing arm (consumer already
ported); rival-flight fidelity only. Easy fix or bank.

**B4. MC2 scroll pickup +4-XP-to-owned-spells** (`UpdateExperience_
6E090`, EF:44262, called from `UpdateScroll_59C80` EF:41183) — RESOLVED,
NOT A GAP (traced + decompile-verified 2026-07-20). The "mana-pool
pickup" label was a MISNOMER: the trigger is a class-14 SCROLL, and the
award (+4 single-player to every OWNED spell, castle-XP clamped to 7) is
already live at world.rs:5982-5986 via a direct `mc2_award_xp` call
(human-gated). The audit's `mc2_cast_xp` push-site census missed it
because the scroll bypasses the mail — the very "reachable path ported
under a different name" failure mode this audit warned about. NOT the
"main passive XP engine": `UpdateExperience`'s only real XP caller is
this scroll; the other caller (Events.cpp:3906) passes countXP=0 (clamp
only). Stale "banked until Phase-4.2" comments reworded.

**B5. MC1 GAP-10C terrain sub-spawners** (states 0x0F/0x10/0x13:
bouncing digger, physics bomb, fire field — each spawns children).
Reachability unconfirmed (need spawner trace); reclassify UNREACHABLE
or fix after the trace.

**B6. GAP-9A latent: c9 states 7/11 terminal-explosion law.** Wizard-
only-explode (`sub_530C0` law) vs our generic payload explode. Their
only emitters (`sub_57040`/`sub_57800`, castle defense) are unported —
unreachable TODAY; MUST land with the castle housekeeping cluster.

**B7. MC2 class-10 0x22 scorch-mover (`sub_344A0`) + 0x26 mass-summon
(`sub_357C0`).** Authored-THING-only, no confirmed shipped record →
"Model helpers" debt with an importer-census caveat.

## FAITHFUL-NOP / UNREACHABLE — closed with citations

- MC1 parked band 102..119: states 0x66-0x77 → sub_20B90/BA0/BB0 all
  `return 0` (:4641-43), table data10=0 (:4790-4807). The ROADMAP
  "data10=0 for ALL?" question is ANSWERED-YES.
- m13/14/15 idles (:4636/4638/4640) + m13/14 chase/pack (:4637/4639)
  `return 0`; feeders never self-promote → unreachable states.
- MC1 family 11 triggers: 31 MATCH + 0x1F `return 0` (:67655).
- MC1 family 12 spells: all 24 stage-0 handlers ported (72 rows; the
  3-cycle helpers are presentation, reproduced renderer-side).
- MC1 families 1/4 empty, 6/8/13 whole-table `return 0` + never spawn.
- MC1 rebound mana-debit INTERIM: on the human-deflector arm, which
  nothing reaches (no MC1 player rebound); reachable Pool arm faithful.
- MC1 objective claim + world.rs:6873 balloon probe: retail
  `sub_5A090` filters `[i+65]==0` = human carpet only — player-only
  scan IS faithful for class-11 triggers.
- MC2 objective types 4/6: census 0 of 165 levels; type 6
  un-completable in retail.
- MC2 class-5 s6 "flee" slots m15-28: flee bit clear in every
  str_D7BD6 row + handler bytes unrecovered → never entered. m29
  s1/s2 disabled rows (`0,0`). m0 tether = DEVIATIONS.md:102.
- MC2 class-11 models 5..11 (strB0 0x05-0x0B, unrecovered bodies):
  PROVEN DORMANT — zero authored records in all 165 baked levels,
  zero runtime spawn sites. ROADMAP suspect confirmed-and-closed.
- MC2 class-2 trees: fully ported (scenery.rs) — suspect REFUTED by
  two independent walks.
- MC2 class-13 strD0 all-NULL; class-3 rows 0x07/0x08/0x0A empty EV
  `break;`; class-14 rows 0x00-0x05 empty case; classes 1/4/6/7/8/12
  single-NULL tables.
- MC2 strA0: dead rows 0x39/0x3F/0x5F/0x61 (no ctor); 0x38/0x3D/0x42/
  0x43/0x4B invisible self-despawning markers; 0x3A blast ring +
  0x38/0x3D/0x4B creators = fully-authored ORPHANED content (no 4A190
  caller, not load-whitelisted, level-63/189 slots dis≠−1 inactive);
  0x4D caller-less; 0x4C = cross-table collision (class-5 m9 s4,
  ported). 0x3B aura reachable and ported (`mc2_aura_tick`).
- MC2 (10,57) sphere column rides MC1 ball physics = DEVIATIONS.md:89
  deliberate APPROX (surface note: includes Fool's-Mana spell-22
  decoys, levels 10/20/34).

## Stale markers + ledger lines to scrub (docs debt, no behavior)

- mobs.rs:1594 "class-2 tick column unported" — ported (scenery.rs).
- proj.rs:428 "(9,20)/(9,21) debuffs unported" — fully wired.
- rivals.rs:36-42 DEFENSE-pending phrasing — body transcribed; only
  disguise visual remains (DEVIATIONS L111).
- ROADMAP "steal-mana/cruise-scroll-grab casts unwired" — scroll-grab
  (0x16) wired at rivals.rs:1968; steal-mana (0xD) absent from
  retail's rival attack rotation (not a gap).
- morph.rs:18 "spell-XP intake deferred" over-broad — dome XP awarded
  (morph.rs:302); only summit-rain flood XP genuinely dropped.
- effects.rs:308 "Gen can't reach the spellbook" — false; mail queue
  exists (fix = D3).
- ROADMAP:124 "type 68 sub_21F60 unported" — STALE: ported as the
  doomsday devour pass (doomsday.rs:466).
- ROADMAP "m15 mimic interim stub (mobs.rs:1392/1458)" — stale refs;
  m15 brain at roster.rs:1151-1307, m16 egg at roster.rs:1364.
- DEVIATIONS.md:94 MC1-proj fallback — narrowed: no live player MC2
  spell reaches it (cast stamps F_MC2PROJ).

## Mound-pattern verdict

Three genuine stacked-stub mounds found, ALL in MC1: D1 (egg,
3-part + misroute), D2 (m15 guards, 2-part), D4 (militia pack,
2-part). MC2 came back clean — no stacked stubs anywhere (the one
mound-shaped candidate, class-11 5..11, closed by dormancy census).

## Lessons re-confirmed

- Model-vs-state table indices alias in remc1 (both are "52" in D1) —
  always resolve which table a sub is registered in (:45xx creator
  rows vs :48xx state rows) before naming behavior.
- A regex census over decompile call-sites misses casts in arguments
  (`(axis_3d*)(a1+72), 5, 15` escaped the first (5,15) sweep) —
  confirm negatives two ways.
- "return 0" one-liners in remc1 are proven-faithful only when the
  table row also carries data10=0 or the state is provably never
  entered; a reachable data10=1 entity with a return-0 tick is an
  UNRECOVERED-body suspect (B1), not a FAITHFUL-NOP.
