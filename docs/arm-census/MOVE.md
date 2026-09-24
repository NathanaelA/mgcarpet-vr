# ARM CENSUS — MOVE (wizard carpet movement and pose)

Scope: human vs rival, MC1(/HW) vs MC2, free-run vs pinned-pose, pooled vs native carpet anchor.
All paths below are relative to `crates/mgc-sim/src/` unless they start with `crates/`.
Line numbers are for the working tree as of 2026-09-24 (branch `re_recording`).
HW (`HIDDEN.EXE`) has no movement arms of its own: every MC1 arm below serves `GameId::Mc1Hw` too.

Retail facts established while reading (not verdicts):
- MC2 `sub_5D530` (the carpet mover) has exactly two callers in `NETHERW.EXE`: VA 0x5E134, the alive
  wizard tick `AddPlayer03_00_5E010`, which only the human reaches, and VA 0x5E31D, the death fall
  `sub_5E310`, which both the human and the rival corpse reach (cited in the doc comment of
  `mc2/rivals.rs` `no_mc2_rival_stuck_nudge`, and in the mc2l16 t=19784 fixture). An alive MC2 rival
  moves through a separate routine, `sub_146F0`.
- MC1 `sub_455D0` is the same shape. The human's state-0/1 dispatch and the state-2 death fall
  `sub_45FC0` both reach it, and `sub_45FC0` is shared by the human and the rival. An alive MC1 rival
  moves through `sub_14EB0` instead.
- The whirlwind victim body `sub_33340` is one routine. Its `v40 = (class==3 && model==0)` selects the
  wizard constants 56/384, the actSpeed=80 write and the roll crank. MC2 rivals are spawned as model 1
  (`mc2/rivals.rs:2610`), so in retail they take the creature constants 204/768. The pooled-vs-human
  split in the port matches that data split.

---

### MOVE-1: MC2 carpet mover `sub_5D530` (human flight + death fall vs rival corpse)
- **Retail routine(s):** `sub_5D530` (NETHERW.EXE, EF:59610). This is the carpet's per-tick move:
  pose integration, the ±16 speed servo, the climb ramp, the forward, strafe, knock and duel steps,
  the `moveTest_5D0A0` commit gate, the vertical resolution, and the `sub_5DD50` nudge when the gate
  refuses the move.
- **Port arms:**
  - `flight.rs:774 mc2_move`, called from `engine/world.rs:6716 step_player_flight_mc2`: HUMAN/MC2,
    alive and death-fall (free-run, pooled or native).
  - `mc2/rivals.rs:9924 mc2_rival_carpet_move`: RIVAL/MC2. Reached only from
    `mc2_rival_death_fall` (`:10075`).
  - `lib.rs:1546 move_mc2`: ALT/world-less fallback. It is unreachable while a world exists: a world
    with `ThrustModel::Mc1` takes `faithful_walk`, and the Enhanced thrust model takes
    `move_enhanced`. With no world it falls through to `move_mc1`.
  - Instrument arms that call `flight::mc2_move` directly: `crates/mgc-conform/src/pose_lane.rs:864`
    and `crates/mgc-conform/src/replay.rs:3802` (the pose-lane shadow steppers).
- **Status:** DRIFTED
- **Differences** (human arm vs rival arm):
  - **Block 0, pose.** The human integrates the roll and pitch filters from the stick, including the
    web-slow scaling. It folds in the whirlwind roll crank (`Mc2Ext::whirl_bumps`, ordered by
    `MGC_NO_MC2_WW_CRANK_BEFORE_VETO`) and the quake pitch seizure (`flood_spin`), then takes yaw
    += roll_f/8. The rival does no filter work: it stamps `f32 = 0` (the published aim pitch) and
    leaves yaw untouched.
  - **Block 2, climb ramp / eff_pitch.** The human computes `eff_pitch` from the row band and steps
    with it. The rival always steps at pitch 0. Retail's rival would pass whatever it holds as the
    stale eff pitch. Its storage home for an AI is not established (UNKNOWN), and the difference is 0
    if that home is 0.
  - **Block 3/4, slow and mobilize scaling of the forward and strafe steps.** Human only. The rival
    has no `move_speed`/`mobilize` channels. This is moot in SP because the debuff stamp only bites a
    model-0 wizard (`mc2/proj.rs:1037`).
  - **Block 5, knock.** The human uses `World::take_knock_step` (`engine/world.rs:22799`: clamp
    ±128, decay 4, snap below 4). The rival has an inline copy (`:9957-9969`) with `min(128)` and no
    lower clamp. The human's knock read is also gated by the stop veto through its own kill switch
    `MGC_NO_MC2_STOP_VETO_KNOCK` (`engine/world.rs:6989-6993`). The rival gets the same effect
    structurally, because its veto `return` sits above everything, and it has no switch.
  - **Block 6, one-shot xAdd/yAdd/zAdd mailbox and the water-counter decay.** Human only. The rival
    comment admits "Blocks 6 and 8 … this column still does not model". No port writer of the rival's
    xAdd is known, so this is inert today.
  - **Block 8, debuff decay.** Human only (same as block 3/4).
  - **Block 9, gate refusal side effects.**
    - The human zeroes `tgt_speed` on a cave refusal (`out.zero_speed`) and fires `accel_cancel`,
      which kills the Speed manifestation's window through `mc2_cancel_accel`.
    - The human bumps `water_ctr` on `wet` and on every cave refusal.
    - The rival ignores `out.zero_speed` and `out.wet` completely: `vdes` is not zeroed, its speed
      token is not killed, and no counter moves.
    - Retail's refusal block writes `speed_0xc_12 = 0` on the PLAYER struct and clears
      `SpellEnabled[3]`'s window. The comments present it as one block, not human-only (cited at
      `engine/world.rs` `mc2_cancel_accel`).
  - **Block 9, vertical.** The human uses `Mc2Row` (row 66 open / 104 cave: clearance 256, buoyancy
    −16/−8) and the `mobilize` −51 settle. The rival uses `BEHAVIOR[row156]` (row 67: v_12 128,
    v_14 −4) with no mobilize arm. This is row data: retail reads `a1x->dword_0xA0_160x`, and the
    rival's row is pinned to 67 by `sub_4A9C0`. Both use the ceiling − 384 cave clamp.
  - **Gate parameters.** Human: `player_mc2_gate` (fov 100 constant, the ghost cheat honoured). Rival:
    `mc2_flight_gate(e.f84, v_12, …, ghost=false)`. The helper is shared.
  - **Commit.** The human writes `d.s` and relinks later through `adopt_walk_pose`. The rival writes
    through `move_relink`, which also sets `mc2_pred_axis`.
  - **Stop veto clear site.** The human reads it in `mc2_player_stop_veto` (only when
    `mc2_carpet_slot != 0`, gated by `MGC_NO_MC2_WW_HUMAN_GRAB`). The clear is split across
    `step_player_flight_mc2` and the `mc2_carpet_stall` one-shot, which also skips the cave ambient
    tail. The rival tests and clears `flags & (1<<26)` inline, with no cave tail involved.
- **What reaches each arm in-game:**
  - Human arm: every live MC2 tick of the human carpet, and every tick of his death fall.
  - Rival arm: an MC2 rival wizard killed in flight. Every tick of its corpse's glide to the ground
    runs it.
  - The block-9 drift fires only when a rival corpse's glide is refused by the gate: a cave level
    (map_type 2) with the corpse gliding into rock or a sealed cell, or open water under the
    deep-water slide.
- **Already witnessed?:**
  - Human: yes (mc2l0 fixture "THE MC2 CARPET'S WHOLE DISPATCH RUNS AT ITS OWN WALK SLOT" t=4133;
    mc2l16 t=7841 fall puff; mc2l3 cave-refusal heads cited at `mc2_cancel_accel`, t=3665).
  - Rival glide: yes (mc2l6-rsg rival 378 t=1845 cited in code; mc2l16 fixture t=19784 refused gate
    over deep water, open level).
  - Rival corpse refused **in a cave**, where `zero_speed` would fire: no evidence found.
- ⚖ **Player ruling 2026-09-24: WAIVED.** Retail never places rivals in a cave level, so the cave refusal leg is unreachable; the deep-water leg is witnessed. See ARM-CENSUS.md rulings.
- **Recording ask:** MC2 cave level (mc2l3 is cave). Kill a rival while it flies fast (ideally under
  Speed) close to a cave wall or low ceiling, so its corpse glides into rock before it lands. About 2
  minutes. What differs: retail should zero the corpse's commanded speed so it decelerates 16/tick;
  the port keeps gliding at the fall's servo target. Also useful: a rival killed while its Speed token
  is live, to see whether the corpse's refusal kills the token window.
- **Confidence / notes:** High on the code differences. The claim that retail's refusal block applies
  to the rival is inferred from `sub_5D530` being one function (the refusal block is inside
  `moveTest_5D0A0`, called from `sub_5D530`). The recording should confirm it.

### MOVE-2: `sub_5DD50` 128-unit stuck nudge (human vs rival corpse)
- **Retail routine(s):** `sub_5DD50` (NETHERW.EXE EF:60157, file 0x82550). When the commit gate
  refuses a move and the wizard sits on deep water, on a sealed cave cell, or (while latched) inside
  the ceiling collision margin, it shoves the wizard 128 units along its yaw and sets latch
  `byte_0x261_609`.
- **Port arms:**
  - `flight.rs:1105-1119` (tail of `mc2_move`) with `engine/world.rs:23196 player_mc2_stuck`:
    HUMAN/MC2.
  - `mc2/rivals.rs:10033 mc2_rival_stuck_nudge`: RIVAL/MC2 (corpse only).
  - Shared predicate: `mc2/cave.rs:222 mc2_flight_stuck`.
- **Status:** DRIFTED (small)
- **Differences:**
  - Kill switch `MGC_NO_MC2_RIVAL_STUCK_NUDGE` exists only on the rival arm. It landed in round 139,
    the ROADMAP's own example of a law on one call path. The human nudge has no switch.
  - Human: fov 100, honours `ghost`, latch in `Mc2Ext::nudge_latch`. Rival: fov `e.f84`, `ghost` is
    always false, latch in `Mc2Rival::nudge_latch`.
  - The human writes `st.x/y/z` with no relink and no `mc2_pred_axis` update. The rival uses
    `move_relink`, which writes pred_axis, so the corpse's death puff follows the shove. The human's
    puff reads `mc2_pred_axis` published from the settled pose in `mc2_player_fall` when
    `mover_ran`, which is equivalent unless something between them reads pred_axis.
- **What reaches each arm in-game:**
  - Human: an MC2 human whose move is refused while he sits over deep water on an open level, or on a
    sealed cell or wedged against the ceiling in a cave.
  - Rival: an MC2 rival corpse whose glide is refused over deep water or in a cave.
- **Already witnessed?:**
  - Rival: yes (mc2l16 fixture t=19784, "RETAIL'S MOVER ENDS IN sub_5DD50's 128-UNIT STUCK NUDGE").
  - Human: no fixture. Only a unit test (`flight.rs:2197 mc2_nudge_shoves_128_forward_when_wedged`)
    and the mc2l3 water-skim open lead in `docs/CONFORMANCE-FINDINGS.md:11886`.
- **Recording ask:** MC2 Day level with a coastline. Skim low over deep water toward a rising shore
  (hold forward, pitch down) so the gate refuses and the carpet gets shoved. Repeat 5-10 times over
  2-3 minutes. Optionally, in a cave level, fly into a narrowing ceiling until wedged.
- **Confidence / notes:** High.

### MOVE-3: MC2 death fall `sub_5E310` (human vs rival)
- **Retail routine(s):** `sub_5E310_multiplayer_test_die` (EF:60402). One routine for every class-3
  wizard: `sub_5D530`, then gravity on `word_0x2C_44` (−2/tick, terminal −256, floor ground + row
  `+0xc`), a (10,1) puff at `predictedAxis`, and the landing payout on exact floor contact.
- **Port arms:**
  - `engine/world.rs:7091-7117` (the `d.falling` gravity leg inside `step_player_flight_mc2`), plus
    `engine/world.rs:12209 mc2_player_fall` (puff and landing) and `:12326 mc2_player_land`:
    HUMAN/MC2.
  - `mc2/rivals.rs:10074 mc2_rival_death_fall`, plus `:10129 mc2_rival_death_impact`: RIVAL/MC2.
  - Native post-walk seat for the human: `engine/world.rs:10613-10620`.
- **Status:** DRIFTED (mostly deliberate, one piece of dead code)
- **Differences:**
  - **Velocity at the lethal transition.** The human resets `player.fall_speed = 0`. The rival
    carries `f46` across lives (`MGC_NO_RIVAL_FALL_CARRY` restores the reset,
    `mc2/rivals.rs:3440-3480`). This is a genuine retail split: two different lethal routines, with
    `AddPlayer03_00_5E010` storing `movw $0,0x2c` at 0x82953 and the AI's `sub_12A70` not storing it
    (0x3740A). ⚠ The MC2 human respawn also zeroes `fall_speed` (`engine/world.rs:12544`), which the
    rival respawn deliberately does not do. It is masked, because the human's lethal transition
    resets it anyway.
  - **Gravity storage.** Human: `death_fall_step()` on `player.fall_speed`, floor
    `Mc2Row.clearance`. Rival: inline on `f46`, floor `BEHAVIOR[row156].v_12`. Same math, different
    row data (256 vs 128).
  - **Puff axis.** Both read `mc2_pred_axis` (`MGC_NO_MC2_FALL_PUFF_PRED_AXIS`, one switch for both
    arms). The human refreshes it only when `mover_ran`; the rival gets it refreshed by `move_relink`.
    The human also has a pinned-pair reconstruction (`fall_pre_z` /
    `MGC_NO_MC2_FALL_PUFF_PINNED_PRE_Z`). The rival has none, and needs none, because it is always
    free-run.
  - **Grey-screen killer turn** (`engine/world.rs:7140-7154`). It is gated on the tick-head `d.dead`,
    but the Dead state already returned at `:6746`. Under the default law it can therefore only fire
    under `MGC_NO_MC2_REVIVE_TICK_TURN`, which makes it dead code on the fall. `sub_5E310` has no
    turn, and the rival arm has none either, so both arms agree in effect.
  - **Presentation.** `sub_5C800(a1x,7)` and the menu-cursor reset are human-screen only, and not
    modelled.
- **What reaches each arm in-game:** any MC2 wizard killed in flight: the human (e.g. Shift+K, or
  shot down) or a rival.
- **Already witnessed?:**
  - Human: yes (mc2l0 fixture "THE MC2 DEATH PAYOUT RUNS A TICK LATE" t=11190; mc2l16 t=7841 puff
    under whirlwind veto; mc2l3 t=7886 cited).
  - Rival: yes (mc2l6-rsg t=1845/14996/26634 cited in code; mc2l16 t=19784; mc2l4 t=8541 cited).
  - Rival corpse under the whirlwind veto (a stale puff axis on a rival): no evidence found.
- **Recording ask:** MC2 level with a live rival. Cast Whirlwind onto a rival and kill it while the
  funnel holds it (grabbed or inner-lifted), so its corpse's first fall ticks are vetoed. About 3
  minutes; this is the rival twin of mc2l16 t=7841.
- **Confidence / notes:** The dead-code reading of the turn block rests on the early return at
  `:6746`. Worth a unit check before any collapse.

### MOVE-4: MC2 dead wait `sub_5E7C0` / `sub_5E6C0` (human vs rival)
- **Retail routine(s):** `sub_5E7C0_multiplayer_test_banished` (EF:60611). Its first statement is
  `moveBoost = 0`, above the fork. The `IsAiPlayer == 1` arm counts down, respawns or banishes, and
  moves nothing. The ELSE arm calls `sub_5E6C0`: the corpse gaze (22-capped yaw servo toward the
  killer, or `yaw += 5` with no killer), z rewritten from terrain, and the filters and aim pitch
  zeroed.
- **Port arms:**
  - `engine/world.rs:6745-6780` (Dead arm of `step_player_flight_mc2`) plus `:12453
    mc2_player_dead_wait`: HUMAN/MC2.
  - `mc2/rivals.rs:10276 mc2_rival_dead_wait`: RIVAL/MC2.
  - `mc2/rivals.rs:3038-3052` (eliminated arm of `mc2_rival_entity_tick`): RIVAL/MC2 banished.
- **Status:** DRIFTED (switch shape). The movement itself is correctly split by retail's own fork.
- **Differences:**
  - **The knock-magnitude wipe** is one retail statement with two different port switches.
    - Human: `MGC_NO_MC2_KNOCK_DIR_KEEP` changes *what* is wiped (it restores the bearing wipe too);
      the magnitude wipe itself cannot be switched off. The end-sequence seizure also wipes it
      (`:5253`).
    - Rival: `MGC_NO_MC2_DEAD_WAIT_KNOCK_CLEAR` removes the wipe entirely, in both
      `mc2_rival_dead_wait` and the banished arm.
    - MC1, by contrast, uses ONE switch for both arms (see MOVE-8).
  - **The retail ELSE arm (`sub_5E6C0`) exists only on the human.** MC1's port has a rival path into
    the ELSE arm (`Rival::human_driven` / `rival_watch_track`, set by the scatter overflow). MC2's port
    has no such path. Whether MC2's `IsAiPlayer_0x009` can be flipped on a rival row in SP is UNKNOWN.
- **What reaches each arm in-game:** a landed MC2 human corpse waiting for Space, or a landed MC2
  rival corpse waiting to respawn or banished.
- **Already witnessed?:**
  - Human: yes (mc2l0 t=11193 cited; mc2l22 t=33,751 raw killer cited; mc2l24-crazy fixture t=12643).
  - Rival knock wipe: no fixture found; only the doc comment. It rides a graded knock lane only if
    the rival died with an unspent impulse.
- **Recording ask:** MC2. Kill a rival with a big hit (a heavy spell) just above the ground, so its
  corpse lands before the knock decays to 0, then watch the respawn. About 2 minutes. This checks the
  rival's `knock_mag` goes 0 on the first dead-wait tick while `knock_dir` holds.
- **Confidence / notes:** Medium-high.

### MOVE-5: Duel leash `sub_5DE30` (MC2) and duel grip lock tail (MC1 `sub_455D0` :55226-48)
- **Retail routine(s):**
  - MC2 `sub_5DE30` (EF:59721, inside `sub_5D530`): the caster's duel lock servoes yaw (cap 0x82),
    pulls the candidate along the raw bearing at the aim pitch (±120), and drains the opponent.
  - MC1's equivalent is the lock tail inside `sub_455D0`.
- **Port arms:**
  - `engine/world.rs:15173 mc2_duel_enforce`, applied in `flight.rs:1041-1045` (block 7): HUMAN/MC2
    caster, free-run.
  - `engine/world.rs:5402-5404`: HUMAN/MC2 PINNED-POSE (state half only; the leash is dropped).
  - `engine/world.rs:15211 mc2_duel_leash_recorded` / `:15231 mc2_duel_leash_at`: INSTRUMENT (pose
    lane / replay samplers).
  - `mc2/rivals.rs:9883 mc2_rival_duel_enforce`: RIVAL/MC2 caster, death fall only.
  - `flight.rs:326 mc1_duel_tail` in `mc1_move_duel`, plus the latch handling in
    `engine/world.rs:7261-7295`: HUMAN/MC1 caster.
  - `engine/world.rs:8358-8374`: MC1 kill-switch arm (`MGC_NO_MC1_DUEL_GRIP_EXACT`, the old
    knock-channel pull at the tick head).
  - MC1 RIVAL caster: no arm. `mc1/rivals.rs:2220` only latches when the caster is the human
    (`owner_slot_of_source == Some(0)`). The rival death-fall mover (MOVE-6) has no lock tail.
- **Status:** DRIFTED
- **Differences:**
  - **Rival MC2:** the leash only runs in the corpse mover, because alive rivals use `sub_146F0`, which
    has no leash call. It uses the literal `3*80/2` where the human uses `MC2_WIZARD_MIN_SPEED`. The
    opponent can be `PLAYER_TARGET` (read from `human_pose` / `player.life`). The yaw servo acts on
    raw `f30`, where the human's acts on the block-0-integrated yaw. The step pitch is the stamped 0,
    where the human uses `aim_pitch`. Kill switch `MGC_NO_MC2_RIVAL_DUEL_DEATH_TETHER` exists only on
    the rival arm.
  - **Human MC2 pinned-pose:** runs the break conditions and the drain but no pull.
  - **MC1 vs MC2 (different binaries, same shape):** MC1 releases on `dist >= 5120 || victim.life < 0
    || count == 1000` and counts +316. MC2 releases on the manifestation window (`SpellEnabled[14]`
    alive), victim life, and the tier range. MC1's cap re-stamps `+128` (the `dword_93A90` global 80).
- **What reaches each arm in-game:**
  - Human MC2/MC1: the human casts Duel on a rival and keeps flying while the lock holds.
  - Rival MC2: a rival that cast Duel and then died while its lock held. The doc comment states the
    AI never casts spell 14 in SP ("UNWITNESSED, AND STRUCTURALLY SO").
  - Rival MC1 caster: unknown whether the MC1 AI ever casts Duel. If it can, the grip is unported.
- **Already witnessed?:**
  - Human MC1: yes (mc1l48 fixtures t=59113/59117/59118 for dart/tether; the grip itself is cited at
    mc1l48 t=59119).
  - Human MC2: no fixture found naming the `sub_5DE30` pull (mc2l6-rsg t=1325/1374 cover the dart and
    tether only).
  - Rival arms: no evidence found.
- **Recording ask:**
  1. MC2 (mc2l6-like level with Duel learned): cast Duel on a rival, hold it 10-20 s while steering
     away and back, so the pull pushes and pulls, including up a hill. About 3 minutes. This covers
     the human MC2 leash, which has no fixture.
  2. The rival-caster arms are unreachable in SP unless an AI casts Duel; flag as ungraded.
- **Confidence / notes:** The rival MC2 arm exists only for imported locks.

### MOVE-6: MC1 carpet mover `sub_455D0` (human vs rival death-fall)
- **Retail routine(s):** `sub_455D0_45910` (remc1 :55110). Filters and yaw, speed servo, climb
  authority with the speed-0 sink above ground + v_10, polar step, strafe, knock (clamp 128, decay 4),
  duel lock tail, `sub_45410` wall gate with the trailing z-floor, and the every-64th-tick flutter
  draw.
- **Port arms:**
  - `flight.rs:367 mc1_move_duel` (and `:351 mc1_move`), called from `engine/world.rs:7147
    step_player_flight`: HUMAN/MC1+HW, alive and fall.
  - `mc1/rivals.rs:6016 rival_death_fall` (lines 6017-6124 are an inline re-implementation):
    RIVAL/MC1+HW, fall only.
  - `lib.rs:1337 move_mc1`: ALT/world-less. Its `Some(world)` branch is unreachable, same as MOVE-1.
  - Instrument: `crates/mgc-conform/src/pose_lane.rs:626`, `crates/mgc-conform/src/replay.rs:2948`.
- **Status:** DRIFTED
- **Differences:**
  - **Speed-0 sink.** Human: literal `z > g + 1024`. Rival: `BEHAVIOR[row156].v_10`, behind
    `MGC_NO_DEATH_SINK` (rival only).
  - **Z-floor.** Human: literal `ground + 128`. Rival: `ground + row.v_12`. Equal only if the rival
    row carries 128; not verified.
  - **Wall gate.** Human: `player_wall_gate_fixed`, which bypasses the gate for the ghost cheat and
    goes through the CommitGateVerb seam. Rival: `player_wall_slide` directly, behind
    `MGC_NO_MC1_RIVAL_FALL_WALL_GATE` (rival only).
  - **Knock.** Human: `take_knock_step` (clamp ±128). Rival: inline `min(128)`, no lower clamp.
  - **Pitch.** Human: `eff_pitch` climb and dive authority. Rival: pitch 0. For an AI, retail's stick
    words and aim pitch are 0, so this is equivalent unless the AI's stale v_28 is non-zero.
    UNKNOWN.
  - **Yaw.** Human: from the live stick (the fall keeps steering). Rival: fixed.
  - **Duel lock tail.** Human only (MOVE-5).
  - **Flutter.** Human: carpet-private `tick_ctr` + `rand`. Rival: entity `f63` + `rand`. Same law,
    different storage.
  - **Accelerate override.** Parameter present on the human arm but forced `None` on the faithful
    path; absent on the rival.
- **What reaches each arm in-game:**
  - Human: every live MC1/HW tick and the human death fall.
  - Rival: an MC1/HW rival killed in flight.
  - The row-vs-literal differences fire whenever a dying rival hovers between its row band and
    1024 / 128.
- **Already witnessed?:**
  - Human: yes (mc1l42 t=17330, mc1l0 many).
  - Rival: yes (mc1l3 fixtures t=1858 "THE CORPSE KEEPS ITS JINK", t=1889 flutter; mc1l12 t=901 wall
    gate; mc1l2 t=8284 knock; mc1l49 slot 620 sink cited).
- **Recording ask:** Both arms are witnessed. The open divergences are: (a) row data vs literals,
  which a take where a rival corpse falls from high altitude over rising ground would settle (MC1,
  any rival level, ~2 min); (b) ghost, which is a cheat, not retail; (c) the lock tail on a rival,
  see MOVE-5.
- **Confidence / notes:** Verify `BEHAVIOR[rival row].v_10/v_12` equal 1024/128 before deciding the
  row/literal question can be skipped.

### MOVE-7: MC1 death fall wrapper `sub_45FC0` (human vs rival)
- **Retail routine(s):** `sub_45FC0_46300` (remc1 :55434-90). One routine: mover, gravity on +46, the
  (10,1) trail at the scratch, and the landing impact on floor contact.
- **Port arms:**
  - `engine/world.rs:7325-7340` (gravity leg in `step_player_flight`), plus `:11435
    mc1_mortality_pass` (trail and touchdown) and `:11534 player_land`: HUMAN/MC1.
  - Native post-walk seat: `engine/world.rs:10656`.
  - `mc1/rivals.rs:6016 rival_death_fall` / `rival_death_impact`: RIVAL/MC1.
- **Status:** DRIFTED
- **Differences:**
  - **Lethal transition.** The human sets `fall_speed = 0` (`engine/world.rs:11415`). The rival
    enters the fall with its live `f46` (test `a_rival_corpse_enters_the_fall_at_its_live_climb_rate`,
    `mc1/rivals.rs:9203`). Retail uses different routines for the two lethal transitions (human
    `sub_46540`, AI `sub_132B0`). Whether the human's +46 is ever non-zero alive is UNKNOWN.
  - **Trail puff spawn position within the dispatch.** Human: in `mc1_mortality_pass`, after
    `mc1_wizard_pass` and `player_regen_block`. Rival: inline right after gravity. Allocation order
    relative to same-dispatch spawns may differ.
  - **Touchdown test.** Human: `z <= ground + 128`. Rival: `body.z == floor` (row v_12).
  - **Pinned-pair reconstruction** (`fall_pre_z` / `mc1_fall_entry_z` / `mc1_fall_scratch`): human
    only. The rival is always free-run.
- **What reaches each arm in-game:** any MC1/HW wizard killed in flight.
- **Already witnessed?:** Human: yes (mc1l42 t=17330, mc1l32 t=39873). Rival: yes (mc1l2 t=8284,
  mc1l3 t=1858).
- **Recording ask:** none essential. For the puff-ordering question: MC1, kill a rival in the same
  tick the human casts (e.g. a fireball that kills while you fire again). ~2 min.
- **Confidence / notes:** Medium on the retail-routine split for the lethal +46 write.

### MOVE-8: MC1 dead wait `sub_46480` / watch aim `sub_463B0` (human vs rival)
- **Retail routine(s):** `sub_46480` (:55594). Its first statement wipes the knock magnitude (+22).
  The `roster byte == 1` arm is the AI countdown and respawn. The ELSE arm runs `sub_463B0`: the watch
  aim re-aims +34/+36 at the killer via a raw pool read and steps +30 by 22; +32 is written then
  zeroed.
- **Port arms:**
  - `engine/world.rs:7199-7228` (Dead arm of `step_player_flight`) plus `:11508-11523`
    (`mc1_mortality_pass` Dead: knock wipe + respawn command): HUMAN/MC1.
  - `mc1/rivals.rs:1905-1908` (knock wipe) + `:6463 rival_watch_track` + `:6498 rival_dead_wait`:
    RIVAL/MC1.
- **Status:** DRIFTED (small)
- **Differences:**
  - The knock wipe shares ONE switch across both arms (`MGC_NO_MC1_DEAD_WAIT_KNOCK_CLEAR`).
  - **Killer out of pool range.** Human: clamps the index to the last slot. Rival: returns with no
    turn.
  - **Killer == PLAYER_TARGET.** Rival: reads `human_pose`. Human: n/a.
  - **Setpoints and filters.** The human does not store +34/+36 setpoints; it zeroes aim pitch and
    both stick filters. The rival stores `f34`/`f36` and zeroes `f32`.
  - **Clock.** The human bumps `tick_ctr` and publishes the carpet rand. The rival relies on the
    walk's `f63` bump.
  - The rival watch fires only on `human_driven` rows, behind `MGC_NO_HUSK_WATCH` (rival only).
- **What reaches each arm in-game:**
  - Human: a landed human corpse waiting for Space.
  - Rival: a landed rival whose roster byte the death-scatter overflow flipped (mc1l49's husks).
- **Already witnessed?:**
  - Human: yes (mc1l42 t=17390-96 and mc1l0-pd t=2647 cited in code).
  - Rival watch: cited at mc1l49 in code, no fixture found.
- **Recording ask:** none required; low-yield. If a human-driven husk take exists, a fixture should be
  cut from it.
- **Confidence / notes:** Medium.

### MOVE-9: Knock ARM (the damage mailbox stamps bearing + magnitude)
- **Retail routine(s):** MC1 `sub_46540_46880` ch0 (:55711-21); MC2 `sub_5EFA0` (EF:60701 /
  EF:61012-37). Each sets `v_24`/`yaw_0x1E_30` = tan2(source, victim) RAW and `v_22`/`moveBoost` =
  amt/10 clamped to [0,80], on any letter whose source is non-zero, the fatal one included.
- **Port arms:**
  - `engine/world.rs:11346-11360` (`apply_player_damage`): HUMAN, both games.
  - `engine/world.rs:6480-6491` (`player_mail_block` invincible arm): HUMAN dev god-mode.
  - `mc1/rivals.rs:2300-2315` (`rival_damage_intake`): RIVAL/MC1.
  - `mc2/rivals.rs:4130-4148` (`mc2_rival_intake`): RIVAL/MC2.
- **Status:** DRIFTED (switch shape only; default behaviour agrees)
- **Differences:**
  - **Dead-source guard.** It is re-addable only on the rival arms:
    `MGC_NO_MC1_KNOCK_FROM_DEAD_SOURCE` and `MGC_NO_KNOCK_FROM_DEAD_SOURCE`. The human arm has no
    guard and no switch.
  - **RAW bearing.** Shared switches `MGC_NO_MC1_KNOCK_DIR_RAW` / `MGC_NO_MC2_KNOCK_DIR_RAW`, read by
    all three arms.
  - **Source `PLAYER_TARGET`.** The rivals resolve it to `human_pose`; the human arm skips it (a
    self-hit gives no knock).
  - **Bearing pose.** Human: the RECORD pose the dispatch passes in (MOVE-16). Rivals: their own
    live entity x/y.
  - **Amount.** Human: `amt/10` after the shield quarter. Rival MC1: `dmg.max(0)/10`. Rival MC2:
    `dmg/10` clamp.
- **What reaches each arm in-game:** any hit on a wizard.
- **Already witnessed?:** Human: yes (mc1l0 t=1128, mc2l22 t=411). Rival: yes (mc1l2 t=8284, mc2l4
  t=8541, mc2l22-new t=3421).
- **Recording ask:** none.
- **Confidence / notes:** High.

### MOVE-10: Knock STEP (consume + decay)
- **Retail routine(s):** inside `sub_455D0` (:55204-19) / `sub_5D530` (EF:59695-711): clamp 128,
  polar step, decay 4, snap below 4.
- **Port arms:**
  - `engine/world.rs:22799 take_knock_step`: HUMAN, both games; also the alternate movers.
  - `mc1/rivals.rs:6081-6092` (inline): RIVAL/MC1 fall.
  - `mc2/rivals.rs:9955-9970` (inline): RIVAL/MC2 fall.
- **Status:** DRIFTED (trivially)
- **Differences:** The human clamps ±128; the rivals use `min(128)` with no lower clamp. MC2 rival
  decay is written `-= if mag<=0 {-4} else {4}`, which equals signum for non-zero values. A negative
  magnitude reaches only the human (MC2 web kick −80, MOVE-20), and that is the arm with the lower
  clamp, so the difference is currently unreachable on the rivals.
- **What reaches each arm in-game:** a knocked human (any tick), a rival corpse falling with an
  impulse.
- **Already witnessed?:** Human: yes. Rival: yes (mc1l2 t=8284).
- **Recording ask:** none.
- **Confidence / notes:** High. A trivial collapse candidate.

### MOVE-11: One-shot stop veto `byte[1] & 8` (F_STOP) on wizard movers
- **Retail routine(s):** the first statement of `sub_5D530` (0x81d39) and of `sub_146F0` (EF:6441-44).
  Armed by the whirlwind victim pass.
- **Port arms:**
  - `engine/world.rs:22832 mc2_player_stop_veto` + `:6989` + `:7015-7020` + `flight.rs:969`
    (`mc2_stop`): HUMAN/MC2.
  - `engine/world.rs mc2_take_player_stop_veto` (read + clear): HUMAN, enhanced mover.
  - `mc2/rivals.rs:4388-4391` (alive): RIVAL/MC2.
  - `mc2/rivals.rs:9927-9930` (corpse): RIVAL/MC2.
- **Status:** DRIFTED (structure)
- **Differences:**
  - **Human.** Gated by `mc2_carpet_slot != 0` (the native carpet never vetoes) and by
    `MGC_NO_MC2_WW_HUMAN_GRAB`. It suppresses the knock via `MGC_NO_MC2_STOP_VETO_KNOCK`, and the cave
    tail via `mc2_carpet_stall`. The command integration keeps running, which is correct.
  - **Rivals.** Plain inline test-and-clear, no switch.
  - **Native vs pooled.** Native MC2 has no pinned seat for `F_STOP`, so a grabbed native human is
    never vetoed. This is a native-only divergence and is ungraded (see MOVE-18).
- **What reaches each arm in-game:** a wizard grabbed or inner-lifted by a whirlwind funnel (MC2).
- **Already witnessed?:** Human: yes (mc2l30 t=2985-2999 cited; mc2l1 t=283). Rival alive: yes
  (mc2l6-rsg fixture t=10087). Rival corpse: no evidence found (see MOVE-3's ask).
- **Recording ask:** see MOVE-3 (a rival killed while held by a funnel).
- **Confidence / notes:** High.

### MOVE-12: AI rival carpet mover — MC1 `sub_14EB0` vs MC2 `sub_146F0` (MC1-vs-MC2 twin)
- **Retail routine(s):** MC1 `sub_14EB0` (:18780-859), MC2 `sub_146F0` (EF:6415). Band settle, level
  forward step, strafe/jink spend with 4/tick decay, ±16 speed servo, Reflexes/tempo-scaled turn
  clamped to row caps, with a snap on overshoot.
- **Port arms:** `mc1/rivals.rs:2556 rival_movement` (RIVAL/MC1+HW), `mc2/rivals.rs:4366
  mc2_rival_movement` (RIVAL/MC2).
- **Status:** STRUCTURALLY-DIFFERENT. These are two binaries' routines with the same skeleton.
- **Differences:**
  - **Altitude.** MC1 uses the 3-band `sub_42000`: full v_14 above ground + v_10, 25% between v_12
    and v_10, floor v_12. MC2 uses the 2-branch `sub_580E0` (v_14 whenever above bare ground, then
    the floor); fixture mc2l1 t=1.
  - **Stop veto.** MC2 only.
  - **Turn error.** MC1 masks `f34 & 0x7FF`; MC2 does not mask `f34`.
  - Neither applies knock or a wall gate, which is correct for both binaries.
- **What reaches each arm in-game:** every alive rival tick in its game.
- **Already witnessed?:** yes, both (the whole rival corpus; mc2l1 t=1, mc2l1 t=22 snap cited).
- **Recording ask:** none.
- **Confidence / notes:** High. Do not collapse these across games.

### MOVE-13: Whirlwind victim body `sub_33340` (pooled victims incl. rivals vs the human arm)
- **Retail routine(s):** `sub_33340` (EF:24229-24407). A ring walk over tile chains. Per victim: the
  far/near-grab/mid-ring/inner-lift arms, the tail band `sub_580E0`, the cave clamp,
  `CopyEntityPosition_57CF0`, and the `sub_11900` bill.
- **Port arms:**
  - `mc2/tail.rs:1960-2125` (pooled victim loop): RIVAL/MC2 plus creatures and objects.
  - `mc2/tail.rs:2126-2503` (the human visit at his chain seat, then the `PlayerWhirl` publish
    `:2504-2600`): HUMAN/MC2.
  - `mc2/tail.rs:2641-2715`: fallback human arms under `MGC_NO_MC2_WW_HUMAN_GRAB` (mid-ring-only
    `PlayerWhirl`) and `MGC_NO_MC2_WW_MIDRING` (the old knock spiral via `player_knock`) — kill-switch
    ladders, not in-game arms.
- **Status:** DRIFTED (the constants split is retail's; the switch and hunk distribution is not
  uniform)
- **Differences:**
  - **Constants.** Human 56/384; pooled 204/768; the human alone gets act80 and the crank. This is
    retail's `v40` split.
  - **Tail band + cave clamp.**
    - Pooled: always runs the band (`mc2_alt_core(v_12, float)`) and the cave clamp at
      `ceiling − f84`.
    - Human: runs the band only when `banded || !MGC_NO_MC2_WW_TAIL_BAND`, with the cave clamp at
      `ceiling − 100`. The mid-ring arm keeps dig W4's channel with no z (comment `:2308-2314`).
  - **Walk laws, pooled side.** The pooled walk carries `MGC_NO_MC2_WW_WALK` (link re-read after the
    body) and the reap-skip switch.
  - **Walk laws, human side.** The human carries a separate family:
    `MGC_NO_MC2_WW_HUMAN_CHAIN_ORDER`, `_SEAT_REACHED`, `_SEAT_RELINK`, `_RELINK_WALK`, `_MOVE_TEST`,
    `_TAIL_PUBLISH`, `_MIDRING_ABS`, plus the crank count/accum laws.
  - **Billing.** Both use `mail_write_single` (`MGC_NO_WHIRLWIND_SINGLE_BILL`, shared).
  - **Transport.** Pooled victims are written in place. The human is a mailbox (`PlayerWhirl`)
    drained by up to three consumers (MOVE-14).
- **What reaches each arm in-game:** MC2 Whirlwind (or the pyramid's (9,26) seed) catching a rival
  (pooled arm) or the human (human arm).
- **Already witnessed?:**
  - Pooled/rival: yes (mc2l6-rsg fixtures t=10086-10110; mc2l0-sg t=12378 goat).
  - Human: yes (mc2l24 fixtures t=7918 and t=7999; mc2l1 t=269/272/283 and mc2l30 t=2973-3000 cited;
    mc2l24-crazy t=69105 cited).
  - A human mid-ring visit that the band would lift (the mid-ring z-less channel): no evidence
    found.
- ⚖ **Player ruling 2026-09-24:** no rivals in caves, so use your own or a trap switch's whirlwind only.
- **Recording ask:** MC2 cave level. Let a rival's or your own whirlwind catch the human in the MID
  ring near rising ground or a low cave ceiling, where the tail band or cave clamp would change z.
  About 3 minutes.
- **Confidence / notes:** Medium. The tail-band asymmetry is noted in-code as deliberate pending
  evidence.

### MOVE-14: Seizure consume phase arms for the human (whirl / hurl / flood): free-run, pinned-pose, seat
- **Retail routine(s):** `sub_33340` (whirl), `sub_21AB0` case 7 (pyramid hurl), and `sub_39B60`
  (quake shove) each write the human RECORD at the writer's own walk slot through
  `CopyEntityPosition_57CF0`. There is one write in retail.
- **Port arms:**
  - **Carpet-dispatch drain** (`step_player_flight_mc2` `:6894` hurl, `:6906` flood pull, `:6964`
    whirl): funnel/pyramid/quake BELOW the carpet, free-run.
  - **Walk hook, above the carpet** (`engine/world.rs:10005-10070`): whirl only, needs a `drive`;
    gated by `MGC_NO_MC2_WHIRL_ABOVE_SEAT` and `MGC_NO_MC2_WHIRL_SEAT_REPUBLISH`.
  - **Walk hook, below the carpet** (`:10078-10097`): whirl republish only, and only when the payload
    is an absolute `grab`; gated by `MGC_NO_MC2_WHIRL_BELOW_SEAT_REPUBLISH`.
  - **Hurl walk hook** (`:9955-9978`): free-run applies and republishes; pinned-pair re-derives through
    `mc2_hurl_walk_pose` (`:23070`).
  - **Flood walk hook** (`:9989-10000`): republish via `mc2_flood_walk_pose` (`:23113`), both drive
    modes.
  - **Token-launch re-derivation** (`:9868`): `mc2_flood_walk_pose(mc2_hurl_walk_pose(player))`. No
    whirl or teleport term.
- **Status:** DRIFTED
- **Differences:**
  - Whirl above the seat has **no pinned-pair arm** (it requires `drive`), whereas hurl and flood both
    have pinned-pair republication.
  - The whirl below-seat republish skips a mid-ring payload published as heading+step (the non-`abs`
    case).
  - The class-15 launch pose composes hurl and flood but not a whirl seizure made at a lower slot. It
    does get the whirl through `adopt_walk_pose` when a republish happened.
  - `apply_player_hurl` bumps `water_ctr` on `wet`, but `mc2_hurl_walk_pose` has no counter, which is
    correct because it is read-only.
- **What reaches each arm in-game:** the human caught by a funnel, pyramid beam or quake. The seat of
  the writer relative to the carpet's slot decides the arm (a funnel spawned after the carpet sits
  above it).
- **Already witnessed?:** whirl above seat, yes (mc2l30 t=2973 cited; mc2l24 t=7913 cited). Whirl
  below seat, yes (mc2l24 fixture t=7918). Hurl, yes (mc2l24 fixture t=45039). Flood, yes (mc2l23
  t=6932, mc2l18 t=27158/27160).
- **Recording ask:** The free-run arms are all witnessed. The missing pinned-pair whirl-above-seat arm
  is an instrument gap; no recording can fix it. It needs a pair fixture on an existing take
  (mc2l30 t=2973).
- **Confidence / notes:** High.

### MOVE-15: Quake / flood shove `sub_39B60` + `sub_3A200` (pooled wizards vs human)
- **Retail routine(s):** `sub_39B60` (EF:29011) is one body per victim. The shove band pulls toward
  the centre and pulls z down, with a wizard (class 3 model 0) arm and a deep-sink ground snap for
  non-wizards. The close band calls `sub_3A200`: a 1-in-7 kill letter of `life+1` (bare `+=`, gated
  by `f28 & 1`), plus the model-0 pitch seizure.
- **Port arms:**
  - `mc2/flood.rs:563 flood_shove` pooled loop, including rivals (model 1 → the non-wizard path,
    WIZARD_ROW v_14 −4 → smooth pull), and `:469 flood_shove_hit`: RIVAL/MC2.
  - `mc2/flood.rs:851 flood_shove_human` (seat law): HUMAN/MC2.
  - `mc2/flood.rs:797-840`: HUMAN legacy under `MGC_NO_MC2_FLOOD_HUMAN_SEAT` (knock channel +
    `player_flood_pull`).
- **Status:** DRIFTED
- **Differences:**
  - **Close-band kill.**
    - Human: `mail_write(Player, 32000)` (area protocol, a fixed 32000 "APPROX", no `f28 & 1` gate).
    - Pooled: `life + 1` added raw (`MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED`,
      `quake_mail_accum_off`), gated `f28 & 1`.
    - The human also posts no spell-XP counterpart.
  - **Victim filter.** Model 1 excludes held/tossed victims (`F_TOSSED | bit0 | 0x20`). The human
    (model 0) filter is the id test alone, as in retail.
  - **The pooled `class==3 && model==0` pitch arm** (`:475`) is unreachable in SP. The human's pitch
    seizure rides `player_flood_pull.spin`.
  - **Chain re-walk** (`flood_chain_rewalk_law`) applies to both, but the human's relink is spliced
    separately.
- **What reaches each arm in-game:** an MC2 Gravity Well / quake (10,67) catching a rival or the
  human; the close band means within 32 units or within 96 of the dome reference.
- **Already witnessed?:** Pooled: yes (mc2l6-rsg t=26502/26515; mc2l22 t=22069/22075). Human shove:
  yes (mc2l18 t=27160, mc2l23 t=6932). Human close-band kill: no evidence found.
- **Recording ask:** MC2 level with Gravity Well available to a rival (or the pyramid). Fly into the
  centre of a quake repeatedly until the close band rolls a kill on the human, then record the mail
  amount and life drop. About 5 minutes; kills are 1-in-7 per tick in the band. This is a DAMAGE lane
  more than a MOVE lane.
- **Confidence / notes:** The 32000 is flagged "APPROX" in-code.

### MOVE-16: Portal vortex / teleporter pad warp (MC1 `sub_26A60` vs MC2 `sub_35390`)
- **Retail routine(s):**
  - MC1 `sub_26A60` (:29170): walks EVERY wizard (the human, then each rival carpet). A facing cone of
    0xAA warps the wizard to dest at ground + row v_12, restamping the portal's site_z.
  - MC2 `sub_35390` (EF:25761): "retail warps EVERY player in the list (AI wizards included)".
- **Port arms:**
  - `engine/world.rs:22226 portal_tick`: human arm `:22269-22304` (deferred via `pending_teleport`,
    with the vortex chain-peek `mc1_vortex_chain_peek_off`) and rival arm `:22305-22322` (immediate
    `move_relink`). MC1.
  - `engine/world.rs:22368 mc2_portal_tick`: human arm only. MC2.
- **Status:** DRIFTED
- **Differences:**
  - **The MC2 rival-warp arm is unported.** The doc comment says "the rival-warp arm is owed with a
    level that authors a pad near a rival start".
  - **MC1 human vs MC1 rival.** The human uses `overlap(i, player)` against the (peeked) record pose;
    the rival uses `ent_overlap(i, c)`. The human warp lands at the carpet's dispatch; the rival warp
    lands in place.
  - **MC2 human vs MC1 human.** MC2 has no pending-teleport chain peek, and MC2 stamps `site_z` behind
    `mc2_pad_stamp_off`.
- **What reaches each arm in-game:** a wizard flying into an MC1 portal vortex or MC2 teleporter pad
  while facing it.
- **Already witnessed?:** MC1 human: yes (mc1l1 t=9899, mc1l32 t=15113). MC1 rival: yes (mc1hwl0
  fixture t=10249, rival 473). MC2 human: yes (mc2l7 t=13328.., mc2l24). MC2 rival: no evidence found.
- **Recording ask:** MC2 level with a teleporter pad (mc2l7 or mc2l24's enclosure pads). Lure a rival
  across the pad, e.g. fly over the pad with a rival chasing you so it passes the pad facing it. About
  5 minutes. The goal is to see whether retail warps the rival and where it emerges. HIGH VALUE: the
  arm is missing outright.
- **Confidence / notes:** High that the arm is missing. Whether rival AI paths ever cross pads
  naturally is unknown.

### MOVE-17: "Record pose" peeks — what the carpet's own dispatch and higher walkers read after a lower-slot position write (MC1 vs MC2)
- **Retail routine(s):** One record (`+72` / `position_0x4C_76`) written by a teleport token
  (`sub_41C70` / `sub_6AD60`), a portal/pad, or a quake/hurl/whirl. Every later reader sees it: the
  at-castle probe, the mailbox knock bearing (MOVE-9), token launches, the rival lanes' `human_pose`.
- **Port arms:**
  - `engine/world.rs:6295 warp_peek_pose`: MC1 mail seat, teleport only (`mc1_teleport_mail_seat_off`).
  - `:23138 mc2_dispatch_record_pose`: MC2 mail seat + at-castle `pre`, flood only
    (`no_mc2_flood_mail_seat`).
  - `:6329 warp_peek_publish_at`: MC1 slot-aware `unconsumed` + cast pose (`no_mc1_warp_cast_pose`,
    `no_mc1_teleport_human_pose`); MC2 republishes with no slot test (`no_mc2_teleport_human_pose`,
    `mc2_teleport_ctx_off`).
  - `:5320-5345` + `mc2_regen_boost_warp`: MC2 at-castle teleport peek (`mc2_teleport_regen_off`).
  - MC1 at-castle teleport peek: in-walk at `:8910-8917`, native at `:10338-10345`.
- **Status:** DRIFTED
- **Differences:**
  - MC1's mail-seat pose includes a pending teleport but no flood.
  - MC2's mail-seat pose includes the flood shove but NOT a pending teleport or hurl. In free-run the
    hurl is already in `player` via the walk hook; in pinned-pair it is not.
  - MC2's `human_pose` republish has no "already consumed by the carpet" slot test. MC1 has one
    (`pending_teleport_slot` vs `mc1_carpet_slot`).
  - MC2 token launches get hurl + flood but no teleport-cast-pose analogue.
- **What reaches each arm in-game:** the human hit by a projectile in the same tick a lower-slot
  teleport or quake moved him, or casting in that tick.
- **Already witnessed?:** MC1 teleport mail seat: cited (mc1hwl0 t=20038 fixture; mc1l15 t=44456 cast
  pose). MC2 teleport republish: yes (mc2l22 fixture t=54237). MC2 flood mail seat: cited (mc2l23).
  MC2 teleport-tick knock bearing: no evidence found.
- **Recording ask:** MC2. Cast Teleport (castle leg) while under fire from a creature or rival, so a
  hit lands the same tick you warp. The knock bearing should point from the attacker to the
  DESTINATION if MC2 matches MC1. Try ~10 warps under fire, about 5 minutes.
- **Confidence / notes:** Medium. The MC2 mail seat may simply never have met a warp-tick hit.

### MOVE-18: Carpet dispatch anchor — pooled in-walk vs native post-walk (both games)
- **Retail routine(s):** One class-3 dispatch per game (`sub_45C90` MC1, `AddPlayer03_00_5E010` MC2)
  at the carpet's pool slot.
- **Port arms:**
  - MC1 pooled `engine/world.rs:8886-9006` vs MC1 native `:10330-10366`.
  - MC2 pooled `:8835-8859` vs MC2 native `:10214-10236`. Both call `mc2_carpet_dispatch` (`:5193`),
    which branches internally on `native`.
  - Mortality tail `:10600-10660`.
- **Status:** DRIFTED (UNGRADED: replay always imports a pooled carpet, so the native arms, i.e. the
  real game, are never replayed)
- **Differences:**
  - **MC1 danger clock.** Pooled decrements in-walk only when `!entry_dead`
    (`no_mc1_danger_walk_seat`). Native decrements in the post-walk tail every tick, dead or alive
    (`:10552-10558`). A dead native carpet therefore runs the clock down, where the pooled one holds
    it (mc1l49 t=3080-85 law).
  - **MC1 native.** No `adopt_carpet_rand`, no in-dispatch mortality. Mortality runs later in the
    tail (`:10656`), after the HUD alert cadence and the command processor ordering.
  - **MC2 native.**
    - `take_respawn` is not consumed in-walk (`if !native` at `:5443`). The respawn seat therefore
      lands post-turn in `lib.rs:1194`, and on the revive tick the mover runs one step from the
      CORPSE position. The pooled law: "THE RESPAWN SEAT LANDS BEFORE THE MOVER", mc2l3 t=7986.
    - The shrine latch is disabled.
    - The cast pass and mana tail are split pre-walk.
    - The stop veto is constant false (MOVE-11).
- **What reaches each arm in-game:** Native = every live app session. Pooled = conformance replay /
  imported worlds only.
- **Already witnessed?:** Pooled: yes (whole corpus). Native: no evidence possible via replay.
- **Recording ask:** No retail recording can grade this. It needs a native-vs-pooled A/B harness
  (e.g. `--replay-check`-style native run of an existing take, compared against retail). Situations
  to target once such a harness exists: dying and waiting in MC1 (the danger clock); an MC2 respawn
  while carrying speed.
- **Confidence / notes:** High on the code. The MC2 respawn phase gap is the most player-visible item:
  the revived carpet lands exactly on the castle seat instead of one servoed step past it.

### MOVE-19: Alternate (app-side) movers in `lib.rs`
- **Retail routine(s):** `sub_455D0` / `sub_5D530` (deviation/fallback copies).
- **Port arms:**
  - `lib.rs:1337 move_mc1`: its `Some(world)` branch is unreachable (`faithful_walk` is always true
    when a world exists under `ThrustModel::Mc1`); only the world-less branch runs.
  - `lib.rs:1546 move_mc2`: unreachable with a world; `None` → `move_mc1`.
  - `lib.rs:1637 move_enhanced`: the deliberate deviation (hold-to-fly).
- **Status:** STRUCTURALLY-DIFFERENT / dead code
- **Differences:**
  - `move_mc1`'s world branch keeps a `+80` accel-expiry edge, a cave-ceiling clamp and
    ExtendedLift.
  - `move_mc2` keeps a sign-preserving accel edge, `take_knock_step` without the stop veto, and no
    teleport/hurl/whirl/flood consumes.
  - These are copies of laws that live correctly in `step_player_flight(_mc2)`.
- **What reaches each arm in-game:** nothing reaches the world branches. Enhanced is reached when the
  player picks the enhanced thrust model.
- **Already witnessed?:** n/a (not retail).
- **Recording ask:** none. Candidate for deletion or collapse. Ungraded.

### MOVE-20: External knock/pose writers on a wizard (buffets and kicks): human vs rival arms
- **Retail routine(s):**
  - MC1 kraken tractor buffet (m6 `sub_1C4F0` :23219-23): writes the TARGET's `+22 = 80`, `+24`
    bearing, `+26 = 256`, whichever wizard `+146` names.
  - MC2 web stamps `sub_38E70` / `sub_38F70`: `moveBoost = −80`, bearing kept, plus the slow/mobilize
    counters.
- **Port arms:**
  - `mc1/mobs.rs:1650-1662` (human `player_knock = (dir,80)`) vs `:1663+` (rival via
    `mc1_buffet_post`, `MGC_NO_MC1_KRAKEN_BUFFET_RIVAL`, rival only): MC1.
  - `mc2/proj.rs:1037 mc2_debuff_stamp_tick`: the human arm gets the −80 kick (`no_stale_debuff_knock`)
    and slow/stun; the pooled model-0 wizard arm (`:1170-1185`) gets only the grunt and the melee
    write. MC2.
- **Status:** DRIFTED
- **Differences:**
  - The MC2 pooled model-0 arm has no −80 kick, no slow/stun channels, and an extra `flags & 0x400`
    guard. It is unreachable in SP, because rivals are model 1, and the human is out of pool.
  - MC1 kraken: both arms exist. The rival arm posts through a queue drained after the dispatch; the
    human arm writes directly.
- **What reaches each arm in-game:** a kraken tethering the human vs a rival (MC1); a spider web
  stamp on the human (MC2).
- **Already witnessed?:** MC1 human: only a unit test (`engine/world.rs`
  `kraken_beam_lays_segments_and_the_buffet_arms_the_knock`). The mc1l42 kraken fixtures t=6274/6453
  cover the dive clock and bolts, not the buffet. MC1 rival: cited mc1l14 t=2262-2276, no fixture
  found. MC2 human: cited mc2l22 t=411.
- **Recording ask:** MC1 level with a kraken (mc1l14 area). Let a kraken tether a rival for several
  duty cycles, ~3 minutes. That gives a fixture for the rival buffet arm.
- **Confidence / notes:** Medium.

### MOVE-21: Respawn re-seat and flight-register clears (4 port copies of one retail tail)
- **Retail routine(s):** MC1 `sub_44D30_45070` (:54857-83), shared by human and rival. MC2
  `sub_5C950` (EF:43630+), shared by human and rival. Seat at the castle's full position, then clear
  the commanded speed, the strafe and the knock triple; the actual speed and the fall velocity are
  kept.
- **Port arms:**
  - HUMAN MC1 app `lib.rs:1194-1227` (surgical clear under the Mc1 thrust model; full `from_tiles`
    rebuild under Enhanced).
  - HUMAN MC1 driver `crates/mgc-conform/src/replay.rs:394-409`.
  - HUMAN MC2 in-walk `engine/world.rs:5443-5449` (pooled only).
  - HUMAN MC2 driver `crates/mgc-conform/src/replay.rs:516-529`.
  - HUMAN world-side clears `engine/world.rs:11813 player_respawn` (knock (0,0)) and `:12495
    mc2_player_respawn` (knock (0,0), `fall_speed = 0`).
  - RIVAL MC1 `mc1/rivals.rs:6543 rival_respawn` (vdes, jink, knock_mag; `knock_dir` under
    `respawn_clear_list`).
  - RIVAL MC2 `mc2/rivals.rs:10350 mc2_rival_respawn` (vdes, strafe, knock_dir, knock_mag).
- **Status:** DRIFTED
- **Differences:**
  - **Fall velocity.** The MC2 human respawn zeroes `fall_speed`; the MC2 rival keeps `f46` (proven
    by mc2l6-rsg). This is masked on the human by the lethal reset (MOVE-3).
  - **Rival MC1 knock bearing.** Its clear sits behind a switch; the human clears both halves with no
    switch.
  - **Enhanced app path** rebuilds the carpet wholesale (deviation).
  - **MC2 respawn phase, native vs pooled.** See MOVE-18.
  - The same three-register clear is hand-copied in four places (app, two drivers, MC2 in-walk). The
    app/driver drift once already (comment at `lib.rs:1213-1219`).
- **What reaches each arm in-game:** any wizard respawning at its castle.
- **Already witnessed?:** Human MC1: yes (mc1l42 t=17398). Human MC2: yes (mc2l0 t=11219, mc2l3
  t=7986). Rival MC1: yes (mc1hwl0 t=20761/20762, mc1l49 t=21968). Rival MC2: yes (mc2l6-rsg fixture
  t=3083).
- **Recording ask:** none for retail. Collapse the four copies into one world-side seat.
- **Confidence / notes:** High.

---

## Summary table

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| MOVE-1 | MC2 mover `sub_5D530` human vs rival corpse | DRIFTED | human(flight::mc2_move) / rival corpse / lib.rs fallback / instruments | y (rival corpse refused in a cave) |
| MOVE-2 | `sub_5DD50` stuck nudge | DRIFTED | human / rival corpse | y (human over deep water) |
| MOVE-3 | MC2 death fall `sub_5E310` | DRIFTED | human (+native) / rival | y (rival killed inside a funnel) |
| MOVE-4 | MC2 dead wait `sub_5E7C0`/`sub_5E6C0` | DRIFTED | human / rival / banished | y (rival knocked corpse lands with impulse) |
| MOVE-5 | Duel leash / grip | DRIFTED | MC2 human free-run / pinned / instrument / MC2 rival corpse / MC1 human / MC1 rival (missing) | y (MC2 human duel hold); rival arms unreachable in SP |
| MOVE-6 | MC1 mover `sub_455D0` human vs rival fall | DRIFTED | human / rival inline / lib.rs / instruments | n (optional high-altitude rival death) |
| MOVE-7 | MC1 death fall `sub_45FC0` | DRIFTED | human (+native) / rival | n (optional) |
| MOVE-8 | MC1 dead wait / watch `sub_46480`/`sub_463B0` | DRIFTED | human / rival husk | n |
| MOVE-9 | Knock arm (mailbox) | DRIFTED (switch shape) | human / god-mode / MC1 rival / MC2 rival | n |
| MOVE-10 | Knock step/decay | DRIFTED (trivial) | human / MC1 rival / MC2 rival | n |
| MOVE-11 | Stop veto F_STOP readers | DRIFTED | human / enhanced / rival alive / rival corpse | y (via MOVE-3) |
| MOVE-12 | AI mover MC1 `sub_14EB0` vs MC2 `sub_146F0` | STRUCTURALLY-DIFFERENT | MC1 rival / MC2 rival | n |
| MOVE-13 | Whirlwind victim body `sub_33340` | DRIFTED | pooled / human / switch-ladder fallbacks | y (human mid-ring near ground/ceiling) |
| MOVE-14 | Seizure consume phase arms (whirl/hurl/flood) | DRIFTED | carpet drain / above-seat / below-seat / pinned / token-launch | n (instrument gap: pinned whirl-above-seat) |
| MOVE-15 | Quake shove `sub_39B60`/`sub_3A200` | DRIFTED | pooled / human seat / human legacy | y (human close-band kill) |
| MOVE-16 | Portal/pad warp | DRIFTED (MC2 rival arm missing) | MC1 human / MC1 rival / MC2 human | y (rival crosses an MC2 pad) — HIGH VALUE |
| MOVE-17 | Record-pose peeks (teleport/flood/hurl) MC1 vs MC2 | DRIFTED | MC1 warp_peek / MC2 dispatch_record_pose / publish_at / regen peeks | y (MC2 teleport under fire) |
| MOVE-18 | Carpet anchor pooled vs native | DRIFTED (ungraded) | MC1 pooled/native, MC2 pooled/native | n (needs a native harness, not a recording) |
| MOVE-19 | lib.rs alternate movers | STRUCTURALLY-DIFFERENT / dead | move_mc1 / move_mc2 / move_enhanced | n |
| MOVE-20 | External buffets/kicks | DRIFTED | MC1 kraken human/rival; MC2 web human/pooled-model-0 | y (kraken tethering a rival) |
| MOVE-21 | Respawn re-seat register clears | DRIFTED | app / 2 drivers / MC2 in-walk / MC1 rival / MC2 rival | n |

## Clusters seen that belong to OTHER subsystems
- **DAMAGE/MAIL:**
  - The quake close-band kill: human `32000` via the area protocol vs pooled `life+1` bare `+=` with
    the `f28&1` gate (`mc2/flood.rs:851` vs `:469`).
  - The MC2 web stamp's pooled model-0 arm lacks slow/stun (`mc2/proj.rs:1170`).
- **REGEN / at-castle probe:** the pre-move pose feeding `mc2_regen_boost` (`pending_respawn` /
  `mc2_dispatch_record_pose` / `human_pose_prev`, `engine/world.rs:5307-5318`) vs MC1's `ctx` teleport
  peek. Two different "pre-move pose" constructions.
- **HUD/AUDIO:** the MC1 danger clock, native vs pooled (MOVE-18). The MC2 cave ambient tail welded to
  the mover (`mc2_carpet_stall`).
- **AI/BRAIN:** the MC2 rival water steer `sub_16580` and the MC1 jink/weave set the `strafe`/`jink`
  and `f30` that MOVE-12 spends. Their brake word (`v14`) is the speed-token cancel.
- **RESPAWN/LIFECYCLE:** `mc2_player_respawn` zeroes `fall_speed` where `sub_5C950` does not (see
  MOVE-21). The MC1 rival `human_driven` overflow has no MC2 twin (MOVE-4).
- **CAST (token launch pose):** `mc1_cast_pose` (MC1 warp) vs MC2's launch pose, which composes only
  hurl + flood (`engine/world.rs:9868`).
