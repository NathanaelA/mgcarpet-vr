# Pedantic review: the MC2 port era (745eef4..HEAD), 2026-07-15

Scope: all 15 commits since 745eef4 ("MC2 support" start) — ~45,700 insertions
across 110 files. Method: 11 parallel decompile-anchored reviewers (one per
subsystem slab), each required to verify claims against `reference/remc2` /
`reference/remc1` / `reference/remc1hw` (EF: = EventsFunctions.cpp line) rather
than the trace bank alone; plus in-session ground-truth (build, full test
suite, fmt/clippy) and in-session re-verification of every P0 and several P1
findings by the coordinating reviewer.

Ground truth at HEAD:
- `cargo test --workspace` **does not build** (mgc-audio, see P0-1).
- Excluding mgc-audio: 284 tests green, all goldens hold.
- Tree is not fmt-clean (mgc-import/src/lib.rs + examples/gm_probe.rs); clippy
  reports warnings → a `v*` release tag today fails CI.

Overall verdict: the architecture held — MC1 bit-identity is intact (the one
MC1 golden re-pin was audited field-by-field and is genuinely layout-only),
hash/golden discipline is near-exemplary, flag registry collision-free,
catch-all despawn lesson heeded everywhere, and a very large share of the port
is decompile-verbatim (the flight model, flood column, terrain tables, sin
LUT, sprite params, cast dispatch, castle ladders all verified clean). The
defects concentrate in (a) the never-playtested rival brain, (b) the doomsday
endgame, (c) individual creature state machines, (d) spell-effect lifetimes /
XP stubs, and (e) verification infrastructure blind spots.

Severities: P0 = broken build/core system; P1 = wrong player-visible behavior
vs retail (HIGH); P2 = divergence in less-traveled paths (MEDIUM); P3 =
LOW/NIT ledger. ✅ = re-verified in-session by the coordinator.

---

## P0 — build + core-system breakers

1. ✅ **Workspace test build broken.** 4c93fe2 removed `tag` from
   `Source::World` (mgc-audio/src/mixer.rs:96) but left 11 `tag:` sites in the
   file's own unit tests → 11× E0559. The last commit shipped without a
   passing `cargo test`; CI (which compiles all tests) cannot be green at HEAD.

2. ✅ **MC2 rival spells are one-cast-per-life** (mc2/rivals.rs:1967, :1866).
   Every cast arms `f26 = f28.max(1)`, but only buff spells {3,5,6,8,0xB} are
   decremented (`mc2_rival_buffs` :988-1012); the readiness gate refuses
   {1,9,0x10,0x12,0x13,0x15} while `f26 > 0`. After the first possess/homing
   cast the spell is locked until death resets it (:2269). Retail:
   `word_0x2E_46` is a live countdown on the manifestation (EF:6997/7014/7065).

3. ✅ **MC2 rival `mana_max` pinned at 1000 forever** (mc1/world.rs:3116-3177
   credits only `self.rivals`; `mc2_rivals[*].mana_max` written only at ctor,
   rivals.rs:238). Downstream: castle ladder gate (:1187) never passes rung ≥1,
   ceiling gate (:1859) locks all expensive spells, possess-economy gate
   (:1129) never closes. Retail: `maxMana_0x8C_140` census-grown (sub_13CE0
   EF:6135). Together with P0-2, MC2 rivals are a fraction of retail threat.

4. ✅ **Doomsday endgame wipes the world** (mc2/doomsday.rs:479-484 and
   :361-366). The v7==1 checkpoint sets `flags |= 0x400` on EVERY
   `class64 != 0 && model65 != 10` entity (castles, wizards, effects); the
   case-0xE arm resets every entity's life to 140 (castles included). Retail
   walks `dword_38523` = class-10 models 0x27/0x28/0x39, the MANA-SPHERE
   family only (builder EF:40020-40032, walks EF:13045-13066, 12846-12852).
   Casting doomsday currently despawns the level roster ~70 ticks before
   retail's endgame even starts.

5. **MC2 sound ids ≥ 47 silently unplayable** (mgc-audio/src/mixer.rs:42,188).
   `SLOT_COUNT = 47` is MC1's request table (remc1 sub_55100); MC2 emits
   snd(47) cave-in/riser, snd(48) multipart, snd(58/59) roster, snd(63)
   morph+doomsday, snd(64) flood grab — all dropped. Also ids < 47 run MC1's
   `policy()` switch, never verified against remc2's dispatch.

---

## P1 — wrong player-visible behavior (HIGH)

### Rival brain (mc2/rivals.rs)
6. **Defense state is a misread**: `DEFENSE_REACTIVE` is retail's
   metamorph-TIER bucket table (sub_15FC0 EF:7616 — requires Metamorph owned,
   picks a disguise tier from nearby-entity buckets); the port walks it as a
   CAST list, so a rival "dodges" by casting Create Castle (:1419-1461,
   :1685-1703), with economy side effects.
7. **RaidCastle STEALS the castle** (`id24 = me` at :1592-1630) — retail state
   7 (sub_13710 EF:5872) casts at it on cadence, never claims it. Also
   Possess/Raid approach radii are swapped vs retail states 6/7 (1024/3072 vs
   2048/3584), and the ball claim-write at aimed `< 0x1C` lives on the BALL's
   state in retail, not the rival's.
8. **Invented "save up and WAIT" hold** (:1836-1841): first
   ceiling-affordable-but-cooling spell returns None — one cast of 0x10
   (AI_RECAST 400) stalls ALL attack casting 400 ticks. Retail walks every
   spell and TIER-DOWNS per spell via sub_15F20 (EF:7581, 7222-7241); the
   tier-down walk is also missing from heal/reactive/home/cruise picks.

### Stage engine (mc1/world.rs, mc2/stagevars.rs)
9. **Type-2 objectives complete one degradation stage early** (world.rs:4633).
   Retail re-points bound rows to the razed building's byte_3 successor
   (sub_59760's one call site = the (10,45) collapse handler, EF:28204,
   40775-78). 27 shipped levels author type 2.
10. **StageVar cadence-skip re-enters the chain** (stagevars.rs:236). Retail's
    skip path releases unconditionally (sub_12330 → sub_12470, EF:5008-12);
    the port routes through `mc2_stagevar_release(.., false)` which re-arms on
    the chain byte (:266-269) — "release this cycle" spawns stay frozen;
    soft-lock-shaped on kill objectives. (`via_chain=true` is never passed —
    the recursion guard is inert.)

### Creature machines (mc2/roster.rs, mc2/multipart.rs)
11. **m26 wraith leech inverted** (roster.rs:3263-3279): three retail
    "stay draining" paths (dist ≥ 2048 / non-avatar / hijack-roll, sub_28FF0
    EF:19338-76) exit to idle in the port — the leech barely drains.
12. **m19 firebug hover roll**: retail's cascading independent ifs became an
    exclusive else-if chain (roster.rs:2033-2057 vs EF:16447-63) — the
    bolt-strafe states 4/5 are dead code, spurious disengages, and the roll
    runs every tick instead of every 4th (RNG stream + 4× dive rate).
13. **m18 tank barrage unrecognizable**: (0,1) timer 400+rand%400 vs retail
    60+rand%60; (2,1) flat 10 in retail vs rand%200+200 (20-40× too long)
    plus an unconditional RNG draw retail doesn't make (roster.rs:1731-44 vs
    sub_253B0 EF:16166-227); and m18_face passes the turn cap in degrees not
    angle units (:1747-55 vs EF:16238-41 — `(a3<<11)/360` = 22/28 units) —
    tracks ~5.5× too slow.
14. ✅ **m22 worm can never be claim/possess-tagged**: ctor `f28 = 1`
    (multipart.rs:479) vs retail `byte_0x38_56 = 3` (EF:34404); the mail gate
    `c.f28 & (1<<ch)` (mc1/combat.rs:180) drops every ch1 message → the whole
    retarget→colorize→castle-acquire→deposit machine is player-side dead code.
15. **m22 hit-reaction steering reversed** (multipart.rs:681-693): the port
    turns the worm TOWARD the attacker; retail's `tan2(src, segment)` surges
    AWAY from the hit segment (sub_26D20 EF:17497-516), and the spin law is
    additive/orbit-signed, not replaced.
16. **m27 kraken branch lives 3-6× too short** (multipart.rs:1251-77): the
    ladder `460·k+920` counts branches; retail counts every chain NODE
    (sub_2AD40 EF:20770 — positions 1/11/21/31/41 → up to 19780).
17. **m27 body turns ~4× too slow** (multipart.rs:2027): passes BEHAVIOR v_4=5
    where retail's live clamp is the LAST sub_58350 arg = v_2 = 22 (EF:20970;
    the same dead-third-arg trap the port documents at mobs.rs:238-244).
18. **m27 burrow flag inverted** (multipart.rs:1814-39): retail case 7/f69==8
    HIDES segments (`byte[0]|=1, &0xF7`, EF:20055); the port shows them — and
    the untargetable clear is unmodeled, so buried branches stay hittable.

### Castle (mc2/castle.rs)
19. ✅ **"No room" scan checks the wrong list** (:430-441): scans
    `(10,2)` entities; retail scans dword_38519 = CLASS-3 model-2 = other
    CASTLES (sub_11A10 EF:4449; the file's own header says so). Castles can
    be built overlapping a rival's.
20. **Killed castle's mana bank vanishes** (:203-217): the death path returns
    at `flags & 0x400` before `mc2_castle_eject`; retail's post-death
    sub_5FD00 level-0 arm spills the ENTIRE bank as owned (10,39) spheres
    (EF:61225-68). No test covers castle-death loot.

### Spell column (mc2/proj.rs, mc2/cast.rs)
21. ✅ **Earthquake trail life 8× retail** (proj.rs:433-441): quake shot action
    2 → sub_66160 (EF:63329) sets life 1× (16/32/64); only whirlwind's action
    27 (sub_678E0) uses 8×. The in-code "player-confirmed ~2× reach per level"
    note doesn't discriminate — relative tier scaling holds under both laws.
22. **Four missing action-wrapper life overrides** (proj.rs:401/443/444/425):
    Crater (sub_66280: life=charge {6,12,24}), Volcano (sub_66250: {7,9,11} +
    zero byte_0x46_70), Gravity Well (sub_677A0: {16,26,40} + f71→0), Tremor
    (sub_677D0: charge&0xF0 {48,80,112} + f71→0) — all currently
    tier-independent ctor defaults. (Meteor + Cave-In were ported right.)
23. **Castle-upgrade +1 spell XP and the whole quake-family effect-tick XP are
    still stubs** (castle.rs:35/:260 "banks with Phase 4.2" — which landed;
    stubs at tail.rs:1064, flood.rs:259/371, morph.rs:294/477). Retail:
    sub_6D8B0(owner,2,1) per upgrade (EF:61596) + per-effect awards
    (EF:23871/23517/23395…). Area spells never level through use; Fire/
    Lightning Tower unreachable through play.

### Cave/doomsday (mc2/doomsday.rs, mc2/cave.rs)
24. **Pyramid devours the wrong class and trips on the wrong stimulus**
    (doomsday.rs:405): retail sub_21F60 walks dword_38531 = the CLASS-9
    PROJECTILE list (builder EF:39988) — it eats incoming spell projectiles
    (bombardment-immune) and trips when devouring / spell-slot-8 armed
    (EF:13614-17). The port eats class-5 creatures and trips on player
    proximity — no player-proximity trip exists in retail.
25. **Pyramid summon repeat/duration writes missing/wrong**
    (doomsday.rs:565-607): retail f26=8/f38=3/f50=682 for all three creature
    ranges (EF:13160-205); port misses f26 (pick 3) or all three (picks 4/5 —
    m21/m25 summons may never fire); second-roll f38/f26 values wrong (bolts
    ~1× instead of 8-10×, seeds 5× instead of 1×).
26. **Mesa floor writes drop retail side effects** (cave.rs:1196-1207):
    sub_570F0 keys the per-cell retile on a4 (not a6) so retail ALWAYS retiles
    (AddBuildingToTerrain_46570) and clears the angle low nibble on h==0
    (EF:39602, 39655-700). Port skips both — stale shading/classes inside
    mesas; a floor carved to 0 stays walkable. Baked into the cave goldens
    (fix ⇒ deliberate re-pin).

### Verification infra
27. **The golden refactor guard never runs in CI** (.github/workflows/ci.yml +
    every fixture test's silent self-skip when `baked/` is absent — reporting
    PASS). No forcing mode; a path drift could disable the guard forever with
    green checks. The in-crate synthetic tests do run; the headline guard is a
    voluntary local ritual.

---

## P2 — MEDIUMs by subsystem

### Rivals
- War flag cleared only vs the human on a landed cast (:1649-53; retail clears
  for any wizard target, EF:5966-69) → permanent inter-rival wars (hate decay
  skips war-pinned).
- Whiff weave: retail wizard-targets-only, per-rival direction byte,
  `roll ±512` yaw jink at 3·minSpeed·refl/255; port strafes for all classes
  with invented magnitude (:1710-29 vs EF:5980-6033).
- Cruise omits retail's speed-up(3) and Perception-rolled 22 casts (EF:5680).
- Poverty release formula: retail /2-clamp applies only when max/4+6000 ≥
  maxMana (EF:7195-98); port `min(max/4+6000, max/2)`.
- Ball picker (sub_148E0 EF:6518): 5120 no-other-wizard test measured from the
  OWNER not the ball; no at-castle skip; hated-owner ranking from self not own
  castle; second-chain fallback missing (:1468-1509).
- Death fall adds horizontal drift + hardcodes floor+128 (retail z-only,
  row v_12; EF:60077-88) — displaces graves/tokens.
- Home/Upgrade states leave steer target 0 → water-steer detours toward a
  fabricated point instead of the castle (retail sets word_0x96_150,
  EF:6175/6117).
- Shield intake: retail nulls one hit + re-arms + awards shield XP
  (EF:60684-99); port always quarters. Hit sound 17 vs retail rand 54..57.
- Readiness gates keyed to wrong spell classes (buff/self + castle refuse on
  the armed window in retail; speed-up has NO cooldown; castle requires the
  aim cone) — fix together with P0-2 (:1852-94 vs EF:7014-80).

### Stage engine / objectives
- Hive imp (m9) held immediately; retail defers the arm until the 16-tick
  materialize completes (word_0x4A_74 → sub_122A0, EF:4771/11987-92) — visual
  + RNG divergence wherever imps are the held template. Related: m9
  materialize completion drops sub_122A0's spawn-point bookkeeping
  (roster.rs:407-12) — hold-gates keyed to hive population may never advance.
- `Mc2Stage.row` stores the raw checkpoint index; retail's m32 switch indexes
  the COMPACTED registered board (EF:54359-70, 40585-645) — any skipped row
  before an m32-referenced row stalls the chain (world.rs:4298, 5650-53).
- The "drop typed rows with stage==0" rule reads the decompile's
  read-before-write slot; if any level authors one, the port's board is a row
  shorter (shifting cursor + voiceover numbering) — census the levels
  (world.rs:4270-72 vs EF:40589-602).
- Type-5 fly-to latch adds a torus-wrap min retail doesn't have — latches
  across the map seam (world.rs:4531-32 vs EF:40806-14).
- Kind-3 release omits the `(action&7) != 2/6` phase gate (EF:5081-84);
  kind-9 "proximity fallback" is dead code (loader stores point only for
  kinds 1|2) — retail's quirk reads pointer bytes as coords; 3 levels author
  kind 9; type-9 lacks retail's 16-frame cadence; slot 0 live in port, inert
  in retail; kind-6 authored-zero timer releases immediately vs retail's
  ~65536-tick wrap; type-1/2 bind latches the FIRST spawn, retail re-points
  on every matching spawn (EF:40661-64).
- Doomsday endgame kills the StageVar subsystem at countdown 70
  (countStageVars zeroed, EF:12997-99) — port skips it; held creatures stay
  frozen through the apocalypse (doomsday.rs:471-87).

### Mobs / multipart
- Goat/archer/villager ctors drop `byte_0x3E_62 = ordinal++` (EF:33740/33901/
  34062) — herd-wide cadence fires the same tick; archer wake stagger
  degenerates. (Multipart got this right via mc2_ord.) Likely relevant to the
  observed too-loose flocking.
- `mc2_mana_spheres` fall arc uses div_euclid (floor); retail is C truncation
  (EF:26910) — off-by-one for deaths > 1024 above terrain.
- Archer wanted-scan filters during the scan (retail post-rejects the winner,
  EF:11768-800); and `mc2_arm_wanted`'s rival arm is an empty stub gated on
  "when the rival column lands" — it landed; archers/militia never retaliate
  against rivals.
- `mc2_move_core` retries skip the roughness/capability test when the step
  stays in-tile (retail runs sub_102D0 unconditionally, EF:8825-50).
- m27 case-1 scan reuses mc2_wizard_scan (skips invisible); retail sub_2A6F0
  has NO invisibility check; range strict < vs ≤.
- `mc2_awake_pass`: misses the hidden-skip (byte[0]&1) and doesn't run over
  dword_38523 (the sphere family) as retail does (:55469/:40025-40).
- m27 0xDF stage-command arms (pose 337, mass-attack broadcast on 0xDA,
  0xD8→StageVar2=15) unported — release choreography for stage-authored
  krakens missing (EF:19675-730).

### Roster / tail / scenery
- m23 leviathan siphon drops the 64-tick f26 timeout (EF:24266-70) — siphons
  forever on an unreachable ball; climb-out pins f44=0x2000 retail leaves
  stale; patrol timer one tick early.
- m14 far-trade threshold dropped a nibble: 0xE1_0000 vs retail 0xE100000
  (EF:14856) — 15 vs 60 tiles.
- m12 template walk wraps at the wrong bound and returns None on exhaustion
  (retail wraps 0x4C→17 and always returns 17, EF:14474-95).
- m24 acquire scans wizards only; retail walks the whole class-3 list
  (castles, balloons; sub_28690 EF:18738-71).
- Falling props: gravity integrates the new velocity (retail: position then
  decrement, EF:62650-52); kick modulus uses kick>>1 vs retail f44>>1; landing
  re-aim sub_654B0 unported (scenery.rs:248-61).
- Fire-trail child copies only f84; retail copies f80/f82/f84 (EF:23719-21) —
  traveling crater digs radius 8 vs retail 3 (tail.rs:1261-67).
- m25: water sprite/speed arm over-fires every swim tick + split's
  pool-exhaustion path skips the corpse burst + dword_0x364D2 census global
  has no counterpart (EF:19026-47, 19124-88).
- Tree ignite: untargetable clear missing, tick70=1 advanced unconditionally,
  extra RNG draw on spawn failure (EF:62424-43).
- Aura magnet drops the "unclaimed ball" gate (word_0x7A_122==0, EF:28365) —
  overlapping auras: last-writer-wins vs first-keeps-claim.
- Storm/blast ticks: one-tick-early despawns (post- vs pre-decrement), missing
  f26++, thunder clap unconditional on pool exhaustion (sounds are hashed).
- Whirlwind lift victim filter is a SUBSET of retail's (class-5 only; retail
  admits class-2 m7/8, non-castle class-3, class-10 subset, no owner skip) —
  the in-code APPROX register claims "superset", which is wrong.

### Castle / terrain paint
- Ring scan idealized full ring vs retail's quirky partial walk (sides = my
  rows × mx cols, second column near CENTER, x-cursor resets; EF:4460-4530) —
  spurious "no room" aborts on the eastern half.
- All four cave/bit3 arms of the castle painter missing (second-heightmap
  rise, cave-vs-noncave bit3/bit7 splits, the countdown==2 angle sweep,
  phase-B promotion gate; EF:27871-909) — cave-level castle builds corrupt
  seal state.
- `mc2_castle_unstamp` substitutes retile-region for retail's 3×3 height
  smoothing finalizer (SetHeightmapByBuildingArea_48B50, EF:28171).
- Balloon tether/delivery distances 2-D vs retail 3-D (EuclideanDistXYZ,
  EF:61797/61826).
- `mc2_castle_eject` draws raw 32-bit LCG where MC2 entity rand is u16 (same
  hazard at cave.rs:469, 814).
- Level-7 haircut i64-widening comment claims "same integer result as retail"
  — false at the always-overflowing rung (retail wraps negative); register as
  APPROX.

### Cave / flood / doomsday geometry
- (10,83) dome + (10,84/85) pit/hill measure to tile-center +128; retail
  measures to tile<<8 exactly (EF:25496-98, 25664-66) — asymmetric bowls.
- (10,83) sync calls cave_seal_fixup (pins ceiling=floor−1); retail's is
  sync-only (EF:25521-26).
- Tube carve wall ring off-by-one box (sub_34B00(ox−1,oy−1,side+1,side+1),
  EF:25243).
- Doomsday beam (case 7) direction inverted: retail HURLS the player away
  with a 1024→10 decaying ramp + moveTest; port drags toward at f>>3
  (EF:13440-70 — the "tractor beam" trace reading is wrong).
- Doomsday phase transitions miss retail's same-tick fall-throughs (cases
  2/4/6/8/0xA/0xC) — every phase runs a tick long, first summon delayed.
- Summon launch positions: retail steps 640 along pyramid yaw at z+768 for
  every summon; port launches at center/ground (EF:13320-76). Summoned
  creatures missing retail's actionIndex overrides (3→7, 4→175, 5→207,
  6→159) + parentId + StageVar2=17 (EF:13415-32). Devour-tripped laser skips
  the ramp init (subSpell |= 2 missing on the bit0 path, EF:13126).

### MC1 side
- Held Lightning (15): repeat-fire commit makes the full cast body re-run
  every held tick — 1000/tick debit (retail re-arm is free; per-shot 500 via
  the emitter), and pool-empty → machine-gun buzz 29 + auto-resume (retail:
  silent, stops until re-click). Hash-visible (sounds hashed).
  (mc1/world.rs:2641-75 vs remc1 :55893-97, :48005, :55908-10.)
- Channel spells (1/4/5/12/14) re-arm on HOLD; retail latch reissues only for
  `+60==0` spells — pre-existing, but the decompile contradicts the
  "player-validated" docstring; needs a ruling.
- `castle_lock_active` on MC1 is new gameplay resting on an MC2 decompile
  inference (truncated MC1 decompile) — documented, playtest owed.
- 24Hz global: MC1 was player-certified at 30Hz; all wall-clock pacing is now
  20% slower than certified. Deliberate, but re-certification owed.
- HW homing meteor: re-scans + eases every tick; retail latches ONE attempt on
  flags bit 2 and snaps (remc1hw :58733-49; the repo's own correct pattern is
  at combat.rs:1493). HW acquire cases 0x12/0x13 unported (Global Death
  doesn't home in HW).
- Kraken grip duty-cycle off-by-one (41 ON/132 vs 40/131; mobs.rs:880-82).

### Audio / app
- ✅ Channel key: retail = (owner word_12CD26, id) pairs — sub_483C0 matches
  `a1 != *v4 || a2 != v4[1]` per channel; the per-id request slot stands, but
  a different owner's same-id sound gets its own channel. Port's constant-0
  tag collapses owners (rival casts suppress/restart player sounds). The
  vestigial (tag,id) mixer machinery is the fix hook.
- Missing cue sheet drops the ENTIRE mc2-audio bundle (sounds.bin + music),
  not just the redbook rip, while printing "skipping redbook rip"
  (mgc-import/src/bundle.rs:546-60).
- Stale bank-1 docs contradict the shipped bank-0 code (mc2_music.rs:17-20,
  :47-48, FORMAT.md:394, bundle.rs:634 error string) — the exact trap that
  caused the wrong-aggressive-cave bug. FORMAT.md also still says fluidsynth.
- config-path `entity_pool_size` skips the CLI's 2..=60000 validation →
  u16 truncation (70000 → 4464 free slots; 65536 → none; 0 → panics)
  (mgc-app/src/main.rs:2912-15 vs :2543-52).
- ~6 registry options marked Live whose consumers snapshot at construction
  (spell_selector, thrust/altitude, invincible, prune_owned_jars,
  audio.arrangement, minimap alpha half of hud_transparency) — the future
  in-game menu would split-brain (settings.rs:185-89).
- war-stem fade linear-PCM vs retail's cc11 expression curve (endpoints match,
  mid-fade hotter); Suspend/StopMusic/Music-replace still hard-cut (same
  artifact class 4c93fe2 fixed for SFX).

### Verification infra
- Conditional-hash aliasing: adjacent hash-only-when-nonzero fields of equal
  width alias distinct states (mc2_player_drain/scrolls/spell_tokens,
  features.rs:749-57; mc2_apocalypse/mc2_doom_level bools, world.rs:2419-27).
  Fix: write a field tag before each conditional contribution.
- mc2_cave.rs re-pin justification contradicts its own assertion ("NOTHING is
  held" vs `held == vec![(447, 18, 9)]` five lines later) — mischaracterized
  as layout-only.
- No layout-independent observable projection is pinned as a golden to carry
  continuity across hashed-layout re-pins (the machinery exists in-tree —
  the pose/terrain digest — it just isn't pinned).
- c44021a (refire gate) landed with zero test coverage and zero golden
  sensitivity; a regression restoring the gate would be caught by nothing.
- mc2sweep's 64-tick, no-stages, no-combat scope is far narrower than the
  "runtime misfits EMPTY" banner implies; examples compiled but never run in
  CI; disposition storms everywhere cap at 1..=64 with no census that no
  level authors dis_id > 64.
- mc2_rivals.rs tests pin lifecycle/objectives but the BRAIN not at all (both
  P0 rival defects invisible); "carry their authored books" and "full of
  mana" claims in headers are unasserted.

---

## P3 — LOW/NIT ledger (compressed)

- rivals: scout-site sweep-all + invented second candidate + water veto vs
  retail first-hit (EF:6076-100); respawn keeps grown ceiling + zeroes all
  cooldowns (retail maxMana=1000 + staggered SpellEnabled[2]=4·color);
  cast_castle stamps cooldown before the affordability re-check; notification
  life 120 vs 100; grave arm unconditional vs spawn-gated.
- mobs/roster: m15 f30/f34 heading-home swap (self-consistent; leaks if
  routed to shared patrol/pack arms); m9/m12/m14 ctors drop xtype=3;
  blast23/storm one-tick-early + missing f26++; meteor quad −5 my_sign32
  misread; orb hit sound once/tick vs per-carrier; fissure jitter truncation
  vs rounding (file-internally inconsistent); whirlwind zero-draw facing
  0x7FF vs 0xFFFF; m15 volley increments the PLAYER's shots stat; kill credit
  drops rival tallies + self-id check; arrows admit any class-3 (can hit
  balloons) + shield ricochet still unported; packmate avoidance lacks
  act_life filter (dodging corpses ≤8 ticks).
- castle/cave: eject count ignores pool headroom (retail fewer-but-bigger);
  intake channel cleared on any nonzero vs owner-match; under-attack flags
  gated to player (rival brain consumer would miss); paint-ordering (classify
  sees post-rise heights); road-leg sign split at step_y==0 && rem_y<0;
  SE-corner wall stamp retail misses; mesa perimeter transpose (latent,
  square-only today); cave-in debris z reads neighbor cell; riser non-cave
  word-vs-byte dec at map edge; BUILD00 "row 7 degenerate" story false (row 8
  is the 1×1); piece dwell off-by-one + axis-home latch unstored (4.2 launch
  arms will need it); flood damage `.max(1)` floor retail lacks; pyramid mail
  misses the else-clear of f40; word_0x36548 doomsday flag never set;
  doom_meter=0 in case 0xF retail doesn't do; flatten_cell misses the cave
  ceiling arm.
- app/UI: F1/F2 print "on" when audio never opened; defaults file's "every
  option authentic" claim vs prune_owned_jars=true; --health-bars missing
  from usage; Ctrl-release force-grabs the cursor; selector drag loses hand
  on second button; MGC_HUD_OPAQUE only parses "0"/empty; stale
  enhancements.* key names in docs; expose_jar_spells taxonomy label.
- audio/import: cc116-at-nonzero-tick invariant unchecked (bank-1 alternate
  is planned); 24 hardcoded as float literals ×3 in mgc-audio (no const);
  bare i16 z-subtraction in spatial math (theoretical); negative-banks cast;
  level_mc2.rs "rides the next epoch" comments already ridden; FORMAT.md
  frames→ms law misstated (doc, not code); redbook INDEX-00 tail note.
- seams/docs: verbs.rs header + every "PENDING falls back to Mc1" comment
  false at HEAD; ids.rs registry comments drift from the arms; flight gate
  call hardcodes (100, 256) while the mover reads row clearance (rows agree
  today); mc2_stagevar_tick ordering comment claims retail order it doesn't
  have; frankenstein.rs module doc stale; hash-gate leak: mc2_speech_ramp
  mutates hashed sounds while itself hash-invisible on stage-less MC2 worlds;
  gitStatus squash workflow voids per-re-pin bisectability (in-file ledger
  substitutes).

---

## Trace-bank corrections (bank these — the CODE is right, the TRACE is wrong)

1. mc2-rivals-spawn-mortality.md §8.6 ("AI at castle keeps damage on ITSELF —
   correct the port") — WRONG; EF:5395-5414 pins grace=2 + full mailbox
   discard. Port + brain-trace correct.
2. The reactive-defense "always casts 6 after 8" reading — WRONG; the
   decompile's `a1 = result` captures sub_15F20's return; the port's
   else-fallback is right.
3. The doomsday "tractor beam" reading — WRONG; retail hurls the player AWAY
   (see P2). The port followed the trace.
4. The mixer's "id-only voice grouping" reading of word_12CD24 — INCOMPLETE;
   the channel key is (owner word_12CD26, id). The per-id request slot part
   is correct.
5. mc2-castle "BUILD00 row 7 degenerate 1×1 / retail memory stomp" — false;
   row 8 is the 1×1; the workaround is a harmless no-op.
6. Earthquake 8× "like whirlwind's sub_678E0" — the class-9 action table maps
   the quake shot to sub_66160 (1×); only whirlwind uses 8×.
7. memory/multi-game-architecture: "MC1 goldens never re-pinned" — stale; one
   audited layout-only re-pin at ae545a6 (verified genuinely layout-only).

## Verified solid (the big-ticket confirmations)

- MC1 bit-identity: all mc1/world.rs growth MC2-gated; the golden re-pin
  audited field-by-field and clean; MC1 goldens pass at HEAD.
- Flight model + gate: verified statement-by-statement vs sub_5D530/5F380/
  5D0A0 (row 66/104 confirmed from Level.cpp itself).
- SIN_DB750 byte-identical (2560 entries, programmatic); sprite_params all
  347 rows byte-identical; both castle terrain tables byte-identical.
- Flood column, cave-in geometry, pillar, riser walks, fissure, whirlwind
  driver, scorch ring, castle CAP/HP ladders + painter origins + DB038,
  cast dispatch + all 18 creator rows + SetSpell + mana ladders, XMI
  conversion + CD_TRACKS verbatim + oxisynth swap + resample fix + 24Hz sweep
  + BAKE_EPOCH discipline, water-steer tables (incl. u8 truncation), hate
  math, tokens, config wiring + guide overlay + transparency drawlist +
  shader layouts: all decompile/byte-verified clean.
- Flag registry: no collisions (bit-29 lesson heeded); catch-all despawn
  discipline: explicit arms everywhere it matters.
- No tautological tests found; every hash pin rides alongside behavioral
  assertions except the honestly-declared MC1 refactoring invariant.

## Suggested sequencing

1. Fix the build (P0-1: mixer tests → (owner,id) key restore fits here) +
   fmt/clippy → CI green.
2. Rival column session: P0-2 + P0-3 + readiness-gate rework (P1-6/7/8 + the
   rival P2 batch) — then the owed rivals playtest.
3. Doomsday/pyramid session: P0-4 + P1-24/25 + the doomsday P2 batch.
4. Audio session: P0-5 (MC2 request table + policy dispatch vs remc2) +
   channel-key + bundle fail-open + stale docs.
5. Creature-machine batch: P1-11..18 (each small and independent).
6. Spell-lifetime/XP batch: P1-21/22/23 + rival impact XP.
7. Castle/cave batch: P1-19/20/26 + P2 geometry items (re-pin cave goldens
   deliberately).
8. Stage-engine batch: P1-9/10 + P2 stage items.
9. Infra: golden-guard CI forcing mode, hash field tags, observable-projection
   golden, refire-gate test, MC1 24Hz re-certification playtest.
