# ARM CENSUS — WORLD subsystem

Scope: world-level effects and non-creature entities (quake/flood, whirlwind, eruption, mine, morph,
terrain paint, houses, balloons, switches, jars, dolmens, death scatter) + the replay-import vs
native twin class. All paths are relative to `crates/mgc-sim/src/`. Line numbers are as of the
working tree on 2026-09-24.

⚠ **A fact that matters for every "rival" row below: in BOTH games the AI wizard carpets are class 3
MODEL 1** (`mc2/rivals.rs:2610 e.model65 = 1`, MC1 `spawn_class3(1, …)` at `mc1/rivals.rs:1578`). Only the
human is (3,0). So every retail branch keyed on `class == 3 && model == 0` (the whirlwind's `v40`,
the flood's wizard z-leg, `sub_3A200`'s pitch-512 store, the switch fire probe's `byte_0x40 == 0`) is a
HUMAN-only branch in single player, and the rivals correctly take the generic/pooled branch. I
checked the whirlwind and the switch fire probe against that. Neither is a human-vs-rival drift.

---

### WORLD-1: Quake/flood shove victim — human seat arm vs pool arm (+ legacy human arm)
- **Retail routine(s):** `sub_39B60` (NETHERW, the (10,67) quake/Gravity Well radius shove) and its close-band callback `sub_3A200`. One routine per victim. The human carpet is a normal chain member in retail.
- **Port arms:**
  - `mc2/flood.rs:224 Gen::flood_shove` pool body (the loop at ~257-402) + `mc2/flood.rs:130 Gen::flood_shove_hit`: POOL/MC2 (creatures, (3,1) rival carpets, markers, corpses)
  - `mc2/flood.rs:512 Gen::flood_shove_human`: HUMAN/MC2, the seat arm (default)
  - `mc2/flood.rs:456-501`, the tail of `flood_shove`: LEGACY HUMAN arm, reachable only with `MGC_NO_MC2_FLOOD_HUMAN_SEAT=1`
  - the carpet-side drain `Gen::player_flood_pull` in `engine/world.rs` (walk hook ~9979-9990, `mc2_flood_walk_pose` 23113, `mc2_dispatch_record_pose`): HUMAN transport
- **Status:** DRIFTED (the close band). The shove band is equivalent.
- **Differences:**
  - Close-band kill mail. POOL mails the victim's own `life + 1` as a signed 32-bit add with no floor (`MGC_NO_MC2_QUAKE_KILL_MAIL_SIGNED`, w149f), as a bare `mail[0] +=` (`MGC_NO_QUAKE_MAIL_ACCUM`, dig 99-8), gated on victim `f28 & 1` (`byte_0x38_56 & 1`), and forced/suppressed for class-5 models 12/0x12/27. HUMAN mails a constant **32000** through `mail_write` (the area protocol, which overwrites a consumed box) with **no `f28&1` gate**. The module doc marks it a deliberate APPROX ("Gen cannot read the player's life").
  - Toss/grab latch. POOL stamps `F_TOSSED | F_QUAKE_GRAB | bit0` (`MGC_NO_QUAKE_TOSS_BIT0`). The action-74 release clears them on every entity in the disc (`MGC_NO_QUAKE_RELEASE_BIT0`). HUMAN writes neither on the pinned human record (module doc APPROX), and nothing ever releases him. Retail does `|= 0x100001` on the human record too.
  - Pitch 512. The `class==3 && model==0` store `f32 = 512` in `flood_shove_hit` is dead in single player (only the human is (3,0), and he is not in the pool). HUMAN gets it as `player_flood_pull.spin` (`MGC_NO_MC2_FLOOD_HUMAN_SPIN`, w146f). Rivals (3,1) correctly get nothing.
  - Shove band. Same formula (`v6` clamp [4,128], `pull = 48*((4096-v5)<<8>>12)>>8`, floor at the stepped point's ground). POOL runs retail's three-way z split (wizard, deep-sink snap `MGC_NO_FLOOD_GROUND_SNAP`, pull+clamp). HUMAN always takes pull+clamp, which is the (3,0) wizard arm, so this is faithful. HUMAN's own z-leg switch is `MGC_NO_MC2_FLOOD_HUMAN_Z_PULL` (legacy arm only).
  - Filter. POOL goes through `flood_shovable` (`sub_39FA0`, `MGC_NO_MC2_FLOOD_HELD_BIT0`). HUMAN runs only the same-owner test. That equals retail's class-3 model-0 arm, so it is faithful.
  - Transport. POOL does `move_relink` at the quake's slot. HUMAN accumulates a delta in `player_flood_pull`, republished to walkers above the quake only on POOLED anchors (the walk hook), and drained at the carpet (`MGC_NO_MC2_FLOOD_MAIL_SEAT`).
  - Chain rewalk. Both follow the re-read cursor (`MGC_NO_FLOOD_CHAIN_REWALK`). The human seat re-entry has its own `seat_block`/`headed` bookkeeping, a second implementation of the same walk.
  - Legacy arm (switch on). One shove after the sweep, carried on `player_knock` (clamped and decaying). Close band = spin + 1/7 roll + 32000.
- **What reaches each arm in-game:** POOL: any creature, marker or corpse, or a **rival carpet (3,1)**, inside a (10,67) quake disc (Earthquake / Gravity Well cast by anyone). HUMAN shove band: the human flies inside 13 tiles of someone else's quake centre, less than 4096 above its dome reference. HUMAN close band: the human sits within 32 units of the centre, or skims under `ref+96` (hugging the crater floor), while the 1-in-7 flood roll fires.
- **Already witnessed?:**
  - POOL creatures: yes (mc2l6 "the-flood-chain-walk-rereads-its-cursor-off-the-moved-victim", mc2l6 t=26515 "one-retail-bit-two-port-homes", mc2l22 t=22075).
  - POOL rival (3,1) victim: no evidence found.
  - HUMAN shove band: yes (mc2l18 t=27160 "the-quake-shoves-the-human-at-his-chain-seat", mc2l23 t=6932, mc2l18 t=27158).
  - HUMAN close band: spin witnessed per the switch doc (mc2l18 t=27261), but no fixture was found. The 32000-vs-`life+1` kill mail is not witnessed.
- **Recording ask:** MC2, a level where a rival casts Earthquake. mc2l22 has rival 584 with an earthquake token, and `mc2l6-rsg` has quakes. (a) Hover the human **dead centre** of a rival's quake, below the rim, for 20-40 s. You must be in the close band (`d<=32` or `z-ref<=96`) on a tick where the 1-in-7 roll hits. Use a shielded or high-life run if you want to survive and see the mail amount. (b) Cast your own Earthquake so it covers a **rival carpet** (fly it onto a rival parked at its castle) and let it run the full 120-tick life. The rival must be inside the 13-tile disc but not in the close band, so it gets shoved.
- **Confidence / notes:** High on the code facts. The close-band divergence is a registered APPROX, so it is known, but it is still a twin drift: the pool arm got 3 laws that the human arm lacks (signed add, bare `+=`, `f28` gate).

### WORLD-2: Whirlwind lift/grab — human seat arm vs pool arm (+ legacy knock-spiral arm)
- **Retail routine(s):** `sub_33340` (NETHERW, the (10,22) funnel's lift/throw pass). Also `sub_338D0` (teardown). One routine, with the `v40 = class==3 && model==0` wizard constants inside it.
- **Port arms:**
  - `mc2/tail.rs:1819 Gen::mc2_whirlwind_lift`, pool loop ~1942-2120: POOL/MC2 (creatures, (3,1) rivals, spheres)
  - the same fn, "THE HUMAN'S VISIT" block ~2186-2385, plus the publish ~2503-2615: HUMAN seat arm
  - the same fn, legacy tail ~2630-2716 (`!human_law`): LEGACY HUMAN arm (`MGC_NO_MC2_WW_HUMAN_GRAB` / `MGC_NO_MC2_WW_MIDRING`)
  - `mc2/tail.rs:2786 Gen::mc2_whirlwind_teardown`: human block vs pool chain loop
  - carpet drain `World::apply_player_whirl` / `take_player_whirl` (`engine/world.rs` walk hook ~10000-10036): HUMAN transport
- **Status:** STRUCTURALLY-DIFFERENT (the same retail body, re-implemented with a separate chain-walk engine for the human). Only a few terms are drift candidates.
- **Differences:**
  - Constants. POOL uses yaw step 204 and lift threshold `768+rand%768`. HUMAN uses 56 and `384+rand%384`, plus the `roll_0x155 += 28` crank and `actSpeed=80` block. This is **faithful**: `v40` selects only (3,0). mc2l6 rival 378 shows `204*n` yaw steps.
  - Cave ceiling clamp. POOL uses `ceiling − f84` (the victim's `array_0x52_82.fov`). HUMAN uses `ceiling − 100` (a hard-coded carpet fov).
  - Float band floor/sink. POOL uses `BEHAVIOR[row].v_12` / `v_14`. HUMAN uses `Mc2Row::clearance` / `buoyancy` (the carpet row). These are equivalent homes, not verified equal on caves.
  - Tail band/publish. POOL does an unconditional `mc2_alt_core` + `move_relink`. HUMAN publishes only when seized, mid-ring or visited (`MGC_NO_MC2_WW_TAIL_BAND`, `MGC_NO_MC2_WW_TAIL_PUBLISH`, `MGC_NO_MC2_WW_MIDRING_ABS`), through the single `PlayerWhirl` mailbox (`MGC_NO_MC2_WW_CRANK_ACCUM`, `MGC_NO_MC2_WW_CRANK_COUNT`).
  - Chain walk. POOL re-reads `next20` after the relink (`MGC_NO_MC2_WW_WALK`). HUMAN runs a separate seat/resume engine (`MGC_NO_MC2_WW_HUMAN_CHAIN_ORDER`, `…_SEAT_REACHED`, `…_MOVE_TEST`, `…_SEAT_RELINK`, `…_RELINK_WALK`) with its own phantom-restart guard.
  - Billing. Both use `mail_write_single` on the grabbed arms only (`MGC_NO_WHIRLWIND_SINGLE_BILL`). IDENTICAL.
  - Reap. POOL has the opt-in legacy `MGC_MC2_WW_REAP_SKIP`. HUMAN has no counterpart.
  - Teardown. POOL clears the latches by walking the disc's tile chains. HUMAN clears them if his live pose is in a ring cell (not a chain walk).
- **What reaches each arm in-game:** HUMAN: fly into anyone's whirlwind, not your own. POOL: creatures, spheres, or a **rival carpet** swept by a whirlwind. The mc2l16 trap switches mint funnels.
- **Already witnessed?:**
  - HUMAN: yes (mc2l24 t=7918 "a-funnel-below-the-carpet-writes-the-dolmen…", mc2l24 t=7999 "the-funnel-walk-follows-the-wizard…", mc2l16 t=7845; switch docs cite mc2l1 t=269/272, mc2l30 t=2973-2986, mc2l24-crazy t=69105).
  - POOL rival: yes (mc2l6 t=10087 "the-rival-mover-s-first-statement…", t=10109 "the-whirlwind-grab-latch-was-never-imported", rival 378).
  - POOL creature: yes (mc2l0 t=12378, mc2l6 t=10089).
  - HUMAN in a **cave** funnel (the `−100` vs `f84` clamp): no evidence found.
- ⚖ **Player ruling 2026-09-24:** no rivals in caves, so a trap switch's whirlwind only.
- **Recording ask:** MC2 **cave** level. Get grabbed by a whirlwind (a rival's, or a trap switch's) under a low ceiling. Stay in the far-grab/near-grab band ~10 s, so the thrown pose hits the ceiling clamp.
- **Confidence / notes:** The constants split is retail-faithful (verified EF:24251-60). The human arm is a parallel implementation of the same walk and is the natural first collapse target once the carpet is in-pool.

### WORLD-3: Castle/building contact passes — whirlwind `sub_33710` vs flood `sub_3A090`
- **Retail routine(s):** `sub_33710` (whirlwind every-8th-tick contact) and `sub_3A090` (quake one-shot damage/grab). **Two distinct retail routines** with the same castle block (shake 30, owner, subSpell `+=`).
- **Port arms:** `mc2/tail.rs:2729 Gen::mc2_whirlwind_contact` (WHIRLWIND); `mc2/flood.rs:915 Gen::flood_damage_pass` (FLOOD)
- **Status:** IDENTICAL on the castle arm, except that the flood also sets `F_QUAKE_GRAB` (retail does too). The building arm differs by design: the whirlwind mails, the flood erases.
- **Differences:** The accumulate-protocol laws are switched separately (`MGC_NO_WHIRLWIND_SINGLE_BILL` vs `MGC_NO_QUAKE_MAIL_ACCUM`). Both walk the whole pool instead of the class lists (the order is irrelevant: no RNG). The flood also skips class 0.
- **What reaches each arm in-game:** a whirlwind or a quake overlapping a castle or village house.
- **Already witnessed?:** no fixture found for either castle arm.
- **Recording ask:** Low priority. MC2: drive a whirlwind across a rival castle (~10 s), and cast Earthquake on a rival castle. The damage and 30-tick shake are graded.
- **Confidence / notes:** Listed for completeness. A shared helper would be the collapse.

### WORLD-4: Volcano eruption driver — MC1 `eruption_tick` vs MC2 `mc2_summit18_tick`
- **Retail routine(s):** MC1 `sub_25EC0` (CARPET) and MC2 `sub_32A70` (NETHERW). Same shape in both games.
- **Port arms:** `mc1/combat.rs:5655 Gen::eruption_tick` (MC1/HW); `mc2/morph.rs:513 Gen::mc2_summit18_tick` (MC2)
- **Status:** DRIFTED (minor; mostly binary-faithful).
- **Differences:**
  - Ground moved under an activation. MC2 clears `erupting`; MC1 does not. **Faithful to each binary**: EF:23931 clears `word_0x31`, remc1 `goto LABEL_5` does not.
  - Counter re-read after the kick. MC1 is behind `MGC_NO_MC1_ERUPTION_COUNTER_REREAD`; MC2 always re-reads (`t_now`) with no switch.
  - Plume handover null guard. MC1 is behind `plume_handover_is_guarded` (switchable); MC2 is always guarded.
  - Patch `volcano_register_revalidate`. The MC2 plume test also requires `old != col`; MC1's `plume_ok` lacks that term.
  - Kick store. MC1 does a raw `+26 = 250` with a class-12 exemption (`volcano_kick_spares_token_burst`). MC2 uses `mc2_write_raw10`'s per-class home table.
  - MC1 pushes `volcano_blind` telemetry; MC2 does not.
  - Counter width: MC1 is i16 wrap, MC2 is i32 (`summit10`). This is a binary difference.
- **What reaches each arm in-game:** a Volcano spell / authored vortex in either game. The stale-register arms need a recycled register slot (a busy pool).
- **Already witnessed?:** MC1: yes (mc1hwl0 t=35673 "the-re-arm-is-not-a-tick-of-its-own", mc1l49 t=14797 "a-dry-pool-plume-handover-flags-nobody", mc1l3 t=5009). MC2: switch docs cite mc2l22 t=23012 and mc2l24 t=17, but no fixture was found.
- **Recording ask:** Only the patch-arm term differs, and patches are forced off in replay, so there is no retail recording need.
- **Confidence / notes:** High.

### WORLD-5: Village house live tick — MC1 `tick_building_live` vs MC2 `mc2_house_tick`
- **Retail routine(s):** MC1 `sub_28DC0` + `sub_29640`; MC2 `AddHouse0A_2D_38330` (+ `CompareEvent08_38B00`). Same shape across games.
- **Port arms:** `engine/features.rs:10999 Gen::tick_building_live` (MC1/HW); `mc2/mobs.rs:4399 Gen::mc2_house_tick` (MC2)
- **Status:** DRIFTED
- **Differences:**
  - **Claim chime.** MC1 plays sound 4 at the claimant for any wizard (human `snd_player`, rival positional). MC2 plays it only when `src == PLAYER_TARGET`. Retail MC2 EF:28023/28032 is `PrepareEventSound_6E450(v7, -1, 4)` for any claimant. So a rival's house claim is silent in the MC2 port.
  - The `possessed_footprint` patch (restore extents after the flag sprite) exists on MC1 only.
  - Lethal hit. MC2 snaps z to ground and publishes the v34 residue. MC1 returns with no z write.
  - Hit register / mail consume. Both clear `f40` at the top and consume the mail only on a non-lethal hit (MC1 behind `MGC_NO_MC1_HOUSE_HIT_REGISTER`).
  - Wanted arm. MC1 `flag_village_wanted` resolves the wizard slot of the id. MC2 `mc2_arm_wanted` tests class 3 model ≤1. Retail MC2 tests model only (EF:27973).
  - Periodic spawn (every 40 ticks + `f140 = occ<<8` in MC1, every 32 in MC2) and the force-claim lock (MC2 only) are binary differences.
- **What reaches each arm in-game:** a rival possesses (claims) a village house in MC2; any hit on a house.
- **Already witnessed?:** MC1: yes (mc1l2 t=5674 "a-claimed-building-is-not-immune-to-its-owner"). MC2 rival claim: the switch doc cites mc2l22-new 2661 (colour only). The chime is **ungraded**: sound is in no lane.
- **Recording ask:** No grading lane for sound. The drift is a code-reading fact, so no recording is needed.
- **Confidence / notes:** High (verified against EF:28016-42).

### WORLD-6: Castle balloon fleet register — MC1 `castle_balloons` vs MC2 `mc2_castle_roster`
- **Retail routine(s):** MC1 `sub_47400` (:56329-49); MC2's roster tail in the castle tick. I did not verify the MC2 address.
- **Port arms:** `engine/features.rs:9637 Gen::castle_balloons` (MC1/HW; it also carries a dead `is_mc2` branch); `mc2/castle.rs:1620 Gen::mc2_castle_roster` (MC2)
- **Status:** DRIFTED
- **Differences:** MC1 landed **class-blind, life-only seats** (`MGC_NO_MC1_BALLOON_SEAT_LIFE_ONLY`, mc1l20 t=18191). A seat whose record was sacrificed and re-minted as another class is kept while its life is ≥0, then cleared without a same-pass spawn. MC2 still **pre-clears** any seat that is not a live own (3,3), unconditionally, so it would respawn in the same pass. MC1 also has the `mc1_oob_castle_inert("quota")` probe and the owner-house tally snapshot, which MC2 lacks.
- **What reaches each arm in-game:** a castle with ≥1 balloon on a pool so full that the allocator sacrifices a live balloon (mass spell spam, lightning storms, a volcano eruption at pool ~999).
- **Already witnessed?:** MC1 yes (mc1l20 t=18191 "the-fleet-register-is-life-only-and-class-blind"). MC2 no evidence found.
- **Recording ask:** MC2, a late level with several rival castles flying balloons. Drive the pool to exhaustion (chain lightning storms / meteor rain / whirlwinds for ~1-2 min) near a rival castle, so balloons are seized mid-flight. A fresh balloon in the same tick shows retail pre-clears; a one-pass gap shows the MC1 law.
- **Confidence / notes:** Needs an MC2 decompile check of the register walk. Overlaps the CASTLE subsystem.

### WORLD-7: Switch/trigger fire probe + rearm probe — MC1 vs MC2
- **Retail routine(s):** fire probe MC1 `sub_5A090_5A5A0` / MC2 `InitSwitchChainZaxisAndSound_6F850`; rearm probe MC1 `sub_5A120_5A630` / MC2 `sub_6F8E0`.
- **Port arms:** `engine/world.rs:22022 World::balloon_probe` (MC1 fire); `engine/world.rs:21750 World::mc2_switch_probe` (MC2 fire); `engine/world.rs:22185 World::rearm_probe` (MC1 rearm); `engine/world.rs:21972 World::mc2_switch_rearm_probe` (MC2 rearm)
- **Status:** DRIFTED (fire probe)
- **Differences:**
  - **Roster-blank gate.** MC1 misses when a mid-tick seizure nulled the class-3 roster head (`wiz_roster_head_blanked`, `MGC_NO_TRIGGER_ROSTER_BLANK`, mc1l48 t=10545). MC2 has **no such gate**, although the MC2 allocator blanks `wiz_chain.cut` on a seizure too (`engine/features.rs:5932`, `MGC_NO_MC2_SEIZE_BLANK`), and retail's MC2 probe walks the same `dword_38519` head.
  - The 3-D `sub_118C0` box (MC1) vs the 2-D `CompareAxisWithShift_10750` box (MC2) is a binary difference.
  - Chime 41 plays on every MC1 hit, but only for model > 3 in MC2.
  - Rearm. Both landed the player table (`MGC_NO_MC1_…`/`MGC_NO_MC2_SWITCH_REARM_PLAYER_TABLE`) and polarity (shared `MGC_NO_MC2_SWITCH_REARM_POLARITY`), so the rearm is IDENTICAL modulo 3-D/2-D.
  - Human-only fire probe. Correct in both games: retail is model-0-only, and rivals are (3,1).
- **What reaches each arm in-game:** the human inside a trigger volume on its 8-tick probe phase. The drift needs a same-tick slot seizure (pool full) BEFORE the switch's slot.
- **Already witnessed?:**
  - MC1 fire+blank: yes (mc1hwl0 t=23480 "the-trigger-fire-probe-reads-the-tick-top-roster"; the switch doc cites mc1l48 t=10545).
  - MC2 fire: yes (mc2l16 t=7903 "a-dead-carpet-is-not-on-the-switch-fire-roster").
  - MC2 fire under a seizure blank: no evidence found.
  - MC2 rearm: yes (switch docs mc2l8, mc2l18).
  - MC1 rearm with a rival/corpse parked inside: **no** (the doc says "⚠ UNWITNESSED").
- **Recording ask:**
  - (a) MC2 at a trap-switch level (mc2l16/mc2l24 storm switches). Exhaust the pool (whirlwinds/lightning) while sitting in a switch volume, so a seizure lands in the same tick as a probe window.
  - (b) MC1, a level with a repeating (11,2)/(11,3) switch. Lure or kill a rival inside its box and keep the human outside for ~30 s after it fires.
- **Confidence / notes:** Retail-MC2 blank semantics are inferred from the shared roster; not byte-verified here.

### WORLD-8: Dolmen shrine stamp — dolmen tick (rival loop + human latch) vs MC2 carpet-side probes
- **Retail routine(s):** MC1 `sub_49AD0_49E10`, MC2 `AddDolmen02_02_65080`. Each stamps every overlapping live player at the DOLMEN's slot.
- **Port arms:**
  - `engine/world.rs:10927 World::dolmen_tick`, rival loop (both games): RIVAL, `act_life>=0 && ent_overlap` → `flags |= 0x1000`
  - the same fn, human latch: HUMAN (`Alive && g.player_overlap(i, ctx)` → `mc1_at_shrine`), switches `MGC_NO_MC1_SHRINE_LATCH` / `mc2_no_shrine_latch`
  - `engine/world.rs:5769 World::mc2_regen_boost`: HUMAN/MC2 NATIVE + legacy probe (inline overlap at the carpet pre-pose)
  - `engine/world.rs:5838 World::mc2_regen_boost_warp`: HUMAN/MC2 WARP tick (a per-dolmen pivot-slot split)
  - `engine/world.rs:~5732-5745`: MC1 legacy live scan
- **Status:** DRIFTED
- **Differences:**
  - MC2 **native** play (`mc2_carpet_slot == 0`) never uses the latch. It keeps the live pre-pose probe (comment at ~5337). The dolmen-slot phase law is therefore on pooled anchors only: a native-only hole that replay cannot see.
  - `mc2_regen_boost`'s inline box has no life gate and uses unsigned extents with hard-coded 121/100/100. The latch path uses `player_overlap` (signed extents, per-game HW, `Alive` gate).
  - The warp arm re-derives the dolmen probe per slot instead of using the latch.
- **What reaches each arm in-game:** park at a dolmen (human or rival). A teleport or whirlwind moves the human across a dolmen.
- **Already witnessed?:** MC2 human latch: yes (mc2l24 t=7918). Warp: the switch doc cites mc2l24 t=2457. MC2 rival at a dolmen: unit tests only. MC1 human/rival at a dolmen: no fixture found.
- **Recording ask:** MC1 level with a dolmen. Park the human on it ~20 s, and let a rival use it if the AI does. Any take works for the rival arm (its `f12` bit is graded through the flags import). The native-vs-latch split is not recordable.
- **Confidence / notes:** Medium on the MC1 rival side (I did not find a take).

### WORLD-9: MC1 jar poll — STRICT (replay) arm vs NATIVE arm
- **Retail routine(s):** `sub_55A40_55F70` (jar pickup poll, :64729-872) + the `sub_55D30` phase-2 wrapper. One routine.
- **Port arms:** `engine/world.rs:16160 World::class12_tick`, `if self.strict_retail { … }` block ~16233-16702 (STRICT/replay); the same fn's native tail ~16703-16798 + `engine/world.rs:16829 World::try_pickup` (NATIVE)
- **Status:** STRUCTURALLY-DIFFERENT (and drifted)
- **Differences:**
  - Pickup cadence. STRICT polls only on `f63 & 3 == 0`. NATIVE's pickup (`player_overlap` → `try_pickup`) runs **every tick**; only the known-stamp is cadence-gated.
  - Jar motion. STRICT has the −128/tick z-servo + water sink to −768 (`MGC_NO_JAR_WATER_SINK`) + an `act_life` countdown. NATIVE has the `jar_ground_snap` patch + `DROPPED_JAR` `f26` decay.
  - Grant. STRICT has no eager `player.owned` write (owned is derived at the carpet), an `acq_holds` refusal and no `f44` stamp. NATIVE writes `player.owned` eagerly, stamps `f44 = damage` and `f26 = 0`, has the dev-mint conversion, and gates on `inert`.
  - Box. STRICT uses an inline 119/`PLAYER_HH` box at `human_pose` (unsigned extents). NATIVE uses `player_overlap(ctx)` (signed).
  - Common laws are switched separately on each path (`MGC_NO_MC1_ACQ_LIST_FULL` both; `MGC_NO_MC1_NATIVE_JAR_KNOWN_STAMP` native only).
- **What reaches each arm in-game:** STRICT: every replay and conformance run. NATIVE: actual play.
- **Already witnessed?:** STRICT: yes (mc1l1 t=3143, mc1l32 t=81/6718, mc1l48 t=3761, mc1l49 t=18525, mc1l24 t=15897). NATIVE: **ungraded by construction** (`--replay` forces strict).
- **Recording ask:** None possible. Retail recordings exercise only the strict arm. The native arm should be collapsed onto it; the strict arm is the witnessed one.
- **Confidence / notes:** High. This is the "REPLAY IS BLIND TO A NATIVE-ONLY HOLE" class.

### WORLD-10: MC1 death jar scatter — human (`player_land`) vs rival (`rival_death_impact`)
- **Retail routine(s):** the death-impact scatter in `sub_45C10_45F50` (:55485-569). One routine for every wizard.
- **Port arms:** `engine/world.rs:11534 World::player_land` (HUMAN/MC1); `mc1/rivals.rs:6174 World::rival_death_impact` (RIVAL/MC1)
- **Status:** DRIFTED
- **Differences:**
  - Owned register. HUMAN clears per scattered entry under `!strict || !owned_survives_scatter()`. RIVAL wipes the whole array only under `!owned_survives_scatter()`, ignoring `strict`. They diverge in native play (human clears, rival keeps).
  - RNG. HUMAN draws from the carpet-slot record's `rand`, falling back to the **global** `g.rand` when there is no carpet slot (native). RIVAL uses its own `ent_rand`.
  - HUMAN has an acq-list-rebuilt-from-owned fallback when the list is empty. RIVAL has none.
  - HUMAN has the `no_spell_loss` patch (cosmetic re-mint). RIVAL has none (ruling owed, "`no_spell_loss` for RIVALS").
  - RIVAL has the `husk_watch_gate` side effect; HUMAN has none.
  - HUMAN wipes `player_mail`. I saw no mailbox wipe in `rival_death_impact`'s body (it may live elsewhere; not verified).
  - Grave re-point (`MGC_NO_MC1_GRAVE_REPOINT_CHAIN`) and owner-keep (`MGC_NO_MC1_SCATTER_JAR_OWNER_CLEAR`): both arms, IDENTICAL.
- **What reaches each arm in-game:** the human or a rival dies holding spells.
- **Already witnessed?:** RIVAL: yes (mc1l4 t=6884 "the-jar-scatter-walks-the-532-acquisition-list", mc1l24 t=15897; the switch doc cites mc1l49 t=39094). HUMAN (strict): no fixture found (unit test `the_death_scatter_keeps_the_jar_record_s_owner_tag` only).
- **Recording ask:** MC1, any level. Learn 4+ spells, then die, ideally shot down over flat ground. Wait out the fall and ~5 s of dead-wait so the scatter positions and jar lives are captured. The native-only differences (owned clear, global RNG) are not recordable.
- **Confidence / notes:** High on the code. The native RNG fallback only matters with no carpet slot.

### WORLD-11: MC2 death payout/scatter — human (`mc2_player_land` + `mc2_scatter_spells`) vs rival payout
- **Retail routine(s):** `sub_5E310_multiplayer_test_die` touchdown block (EF:60120-80): the mailbox memset, the 26-token scatter, the (10,40) grave, sphere re-point, action 3 + timer.
- **Port arms:** `engine/world.rs:12326 World::mc2_player_land` + `mc2/cast.rs:5903 World::mc2_scatter_spells` (HUMAN); `mc2/rivals.rs:~10160-10260`, the rival touchdown payout (RIVAL)
- **Status:** DRIFTED
- **Differences:**
  - Token action. HUMAN **sets** `tick70 = 3M+1`. RIVAL **increments** `tick70 += 1`, which is retail's `actionIndex++`. They differ whenever a book token is not at `3M` (e.g., mid-steal action 78).
  - Respawn timer. HUMAN sets 1200 unconditionally, before the scatter. RIVAL sets action 3 + 1200 **inside** the grave-spawn success arm (a pool-full death stays in the fall).
  - `MGC_NO_MC2_SCATTER_KEEPS_WINDOW` and `no_spell_loss` exist on HUMAN only. RIVAL never touches the window, so the defaults agree.
  - Grave z. HUMAN uses the `floor` argument; RIVAL uses the corpse z. They are normally equal at touchdown.
- **What reaches each arm in-game:** the human or a rival MC2 wizard dies.
- **Already witnessed?:** HUMAN: yes (mc2l19 t=3256, mc2l5 t=17379). RIVAL: yes (mc2l16 t=19784, mc2l6 t=3083).
- **Recording ask:** The divergent conditions are rare. (a) MC2: get the human killed while a wraith is carrying off one of his spell tokens (the action-78 arc). (b) Die with the pool at capacity. Each is ~minutes of setup; low priority.
- **Confidence / notes:** Medium. I did not verify retail's human touchdown ordering for the timer.

### WORLD-12: Magic Mine detonation aim — human tripper vs pool tripper
- **Retail routine(s):** `sub_3A8B0` cases 4/5 (scan + relaunch). `sub_6DCA0` then `sub_655C0` aims at the tripper through the `sub_65580` raise (`z += array_0x52_82.yaw` unless model 2).
- **Port arms:** `mc2/effects.rs:926 Gen::mc2_mine_scan` and `mc2/effects.rs:995 Gen::mc2_mine_detonate`: the `v17 == PLAYER_TARGET` branch (HUMAN) vs the pool branch (RIVAL/creature record)
- **Status:** DRIFTED
- **Differences:**
  - **The human-raise bracket.** POOL aims at `t.aim_z()` (`z + f78`). HUMAN aims at raw `ctx.pz` with no `+PLAYER_HH`. This is the `sub_65580` family the roadmap lists; the castle turret landed it as `MGC_NO_MC2_PIECE_HUMAN_RAISE` (`mc2/castle.rs:3412`), and the mine did not.
  - Tripper liveness. HUMAN `!ctx.pdead` vs POOL `act_life>=0 && !0x400`.
  - Scan order. The human is always the first candidate (strict `<` ⇒ he wins distance ties) instead of taking his roster position.
- **What reaches each arm in-game:** a charged Magic Mine (someone else's) whose 3584-unit sphere the human, or a rival (3,1), enters.
- **Already witnessed?:** Mine birth, bend, swallow and sink: yes (mc2l6 t=13544/13831/14025). **Detonation on any tripper: no evidence found.**
- **Recording ask:** MC2, a level where a rival casts Magic Mine (mc2l6-rsg has mines). Fly the human within ~14 tiles of the rival's charged mine and let it fire its swallowed spell at you (~30 s). Separately, lay your own mine near a rival and let a rival carpet trip it. The bolt's birth pitch is the graded difference.
- **Confidence / notes:** High that the code differs. Retail's raise for the human record is inferred from `sub_655C0` being model-generic.

### WORLD-13: `sub_118C0`/`sub_106C0` carpet-vs-entity AABB — helpers vs inlined copies
- **Retail routine(s):** MC1 `sub_118C0` (+`sub_11950`); MC2 `sub_106C0`/`sub_34E30` and the 2-D `CompareAxisWithShift_10750`. Every extent is read `movswl` (signed).
- **Port arms:**
  - `mc1/combat.rs:885 Gen::ent_overlap` and `mc1/combat.rs:940 Gen::player_overlap`: signed via `aabb_ext` (`MGC_NO_MC1_AABB_SIGNED_EXTENTS`)
  - inline UNSIGNED copies:
    - `engine/world.rs:22103 World::overlap` (trigger/portal/pad)
    - `engine/world.rs:~16590` (MC1 strict jar hit)
    - `engine/world.rs:~20598-20610` (MC2 jar pickup)
    - `engine/world.rs:5769` (`mc2_regen_boost`)
    - `engine/world.rs:21853 mc2_switch_overlap` and `21972 mc2_switch_rearm_probe` (2-D)
    - `mc2/flood.rs:556 Gen::mc2_overlap_xy` (2-D; flood + whirlwind contact)
    - `mc1/rivals.rs:4345`, `mc2/rivals.rs:6240`, `mc2/roster.rs:5250`
- **Status:** DRIFTED
- **Differences:**
  - The signed-extent law (an extent ≥ 0x8000 collapses the box) exists only in the two helpers. The inline copies read `e.f80 as i32` (unsigned).
  - The carpet half-width is chosen per site: the `mc2_trigger_hw_off` split, `MC2_PLAYER_HW`, or a literal `121`.
- **What reaches each arm in-game:** only an entity with a stale/huge extent. The known case is a (10,17) blast ring whose `+26` was hit by a stale volcano register kick (extents 48000 ⇒ −17536), overlapping a trigger, jar or castle.
- **Already witnessed?:** helper arm: yes (the switch doc cites mc1l26-froze t=25647). Inline arms: no evidence found.
- **Recording ask:** Effectively unrecordable on purpose (needs a volcano register alias). Collapse without a recording.
- **Confidence / notes:** High on the code; the practical yield is tiny.

### WORLD-14: IMPORT twin — `mc2_applied_mana_delta` vs native `mc2_manifestation_tick`/`mc2_afford`
- **Retail routine(s):** `sub_68D50` (afford gate) + `sub_68DE0` (first-tick debit / mid-burst regen pin), run from each class-15 token's own dispatch.
- **Port arms:**
  - `engine/world/conformance.rs:4526 mc2_applied_mana_delta` + `:4458 mc2_token_afford_retail` + `:2773 World::mc2_full_stop_import`: IMPORT (re-derives the carpet's applied regen at a pair boundary)
  - `mc2/cast.rs:3475 World::mc2_manifestation_tick` + `mc2/cast.rs:3360 World::mc2_afford`: NATIVE/live
- **Status:** DRIFTED
- **Differences:**
  - Upkeep leg. IMPORT tests `tok.d88 != 0` (retail `test`) against the castle record's `mana`. NATIVE tests `f136 > 0` against the castle's `f140`.
  - Caster gate. IMPORT refuses on `wiz.mana < 0 || wiz.life < 0`. NATIVE tests only `LifeState::Alive`, and its first-tick purse test is `player.mana as u64 >= max_life as u64`, so a **negative purse passes** on the native path.
  - First-tick test. IMPORT uses raw `f2e == f30`. NATIVE uses `f26 == f28.max(1)`.
  - **Duel fizzle** (`sub_6B610` LABEL_19 skips `sub_68DE0`): NATIVE only. IMPORT has no duel arm.
  - Speed brake. NATIVE uses `mc1_v14` collapse. IMPORT uses the full-stop skip (`MGC_NO_MC2_FULL_STOP_KILLS_SPEED`). These are different triggers for the same "skip".
  - The possess re-press release (`f56`) arm is NATIVE only.
  - Castle/heal. NATIVE uses early returns. IMPORT skips heal and exempts via `NO_MID_BURST_REGEN_PIN`.
  - Shield III predecrement. Both, sharing `MGC_NO_MC2_SHIELD3_PREDECREMENT` plus the import-only `MGC_NO_MC2_IMPORT_TOKEN_GATE`.
- **What reaches each arm in-game:** every MC2 pair boundary where a human spell token sits at a lower pool slot than the carpet (IMPORT), vs the same token's own tick (NATIVE).
- **Already witnessed?:** yes for the afford/shield/full-stop/heal halves (mc2l24 t=7284/9753 "a-refused-token-tick-and-shield-iii…", mc2l24 t=40238, mc2l20 t=31826, mc2l22 t=53203). Duel-fizzle on the import path: no evidence found. Negative purse: no evidence found.
- **Recording ask:** MC2, a take where the spellbook token for Duel is at a low slot (a level start, before deaths re-pop the stack). Cast Duel, then **re-press cast within ~20 ticks** (the retrigger cancel), repeating 3-4 times over a minute. Separately: get the purse driven negative (wraith/leech drain) and then cast.
- **Confidence / notes:** This straddles the CAST subsystem. It is listed here as the brief's named import-twin example.

### WORLD-15: IMPORT twin — castle pad replay `mc2_build_pad_stamp` vs live `mc2_castle_painter_tick`
- **Retail routine(s):** `AddTerrainMod0A_2A_37BC0` (the (10,42) castle painter, EF:27648).
- **Port arms:** `mc2/pads.rs:~233 Gen::mc2_build_pad_stamp` via `:122 mc2_castle_pad_reconstruct` (IMPORT-ONLY, the collapsed terminal form); the live painter `Gen::mc2_castle_painter_tick` (NATIVE)
- **Status:** STRUCTURALLY-DIFFERENT (a hand-collapsed limit of the 18-tick lerp)
- **Differences:** The import re-implements the rise terminal (the pristine auto-flat promotion, the absolute height, the bit3/bit7 juggling, the paint re-interpretation) instead of re-running the tick. By contrast, `mc2_building_pad_reconstruct` and `mc2_riser_reconstruct` **do** re-run the native tick (unified). Everything is disabled when the take has a measured terrain channel (`conformance.rs:3919 off = measured_terrain || …`), and by `MGC_NO_PAD_REPLAY`.
- **What reaches each arm in-game:** only a format-1 (no terrain channel) take whose pair lands after a castle was built or levelled.
- **Already witnessed?:** n/a (format-2 takes bypass it).
- **Recording ask:** None. Re-recording a take gives it a measured channel, which retires this arm.
- **Confidence / notes:** High.

### WORLD-16: Terrain retile/shade — MC1 `retile_and_shade` family vs MC2 `mc2_retile_region` family
- **Retail routine(s):** MC1 `sub_33B90`/`sub_33E10` (+ passes); MC2 `sub_462A0`/`AddBuildingToTerrain_46570`. Same shape; MC2 adds the night/cave arms.
- **Port arms:**
  - `engine/features.rs:6708 retile_and_shade`, `:6776 recompute_protected`, `:6804 recompute_unprotected` (MC1)
  - `mc2/terrain_paint.rs:485 mc2_retile_region`, `:518 mc2_add_building_region`, `:543 mc2_blend_shade_passes` (MC2)
  - the inline shading pass in `mc2/terrain_paint.rs:614 mc2_ridge_stamp`
- **Status:** IDENTICAL (verbatim duplicates; the MC1 copy already carries MC2's night and cave arms)
- **Differences:** None found in the blend or shading bodies.
- **What reaches each arm in-game:** any terrain edit (dig, building, crater, road).
- **Already witnessed?:** yes, broadly (terrain-check corpus).
- **Recording ask:** none. Pure dedup.
- **Confidence / notes:** High.

### WORLD-17: Metamorph ctor + puppet tick — human vs rival
- **Retail routine(s):** `sub_6A030` (Morph first tick: mints the sv2=12 puppet) and `sub_1E4D0` (puppet copies the parent pose).
- **Port arms:** `mc2/cast.rs:5322 World::mc2_cast_metamorph` (HUMAN ctor); `mc2/rivals.rs:~9290` rival spell-4 arm (RIVAL ctor); `mc2/mobs.rs:4965 Gen::mc2_metamorph_creature_tick` (one fn with a human branch and a pool-parent branch)
- **Status:** DRIFTED (ctor). The puppet tick is IDENTICAL now: both `MGC_NO_MC2_MORPH_PUPPET_ROLL` and `MGC_NO_MC2_HUMAN_PUPPET_ROLL` landed.
- **Differences:**
  - Retail (EF:56340-50): the puppet gets `byte[0] |= 1`, the **caster** gets `|= 0x21`, and for a non-local caster the puppet's bit 0 is cleared again and the caster's bit 0 is set.
  - HUMAN: sets the puppet bit 0 (`MGC_NO_METAMORPH_PUPPET_HIDDEN`) and cloaks via `player.invisible` (`MGC_NO_METAMORPH_CLOAK`).
  - RIVAL: sets only `0x20` on the caster (`MGC_NO_RIVAL_METAMORPH_CLOAK`). The **caster's bit 0 is not set**.
  - `MGC_NO_MC2_MORPH_KEEPS_CTOR_10`: both arms. The puppet tick's pool branch also returns early when the parent's class is 0.
- **What reaches each arm in-game:** the human or a rival casts Morph in MC2.
- **Already witnessed?:** HUMAN: yes (mc2l0 t=7281; the switch doc cites mc2l6-rsg t=28033). RIVAL: yes (mc2l22 t=1198 "metamorph-mints-a-pose-puppet-record"). But the rival **caster bit 0** is not examined in any note.
- **Recording ask:** No new take needed. mc2l22 t=1198-1199 (rivals 557/530) already holds retail's caster flags. Check `flags & 1` on the rival carpet there.
- **Confidence / notes:** Medium. I have not confirmed which port home would carry the rival record's bit 0, or whether anything reads it.

---

## Checked and NOT a cluster (so nobody re-digs them)
- Whirlwind 56/384 vs 204/768: retail's `v40` is victim (3,0) only, and rivals are (3,1), so the pool arm is right for rivals (EF:24251-60).
- The switch fire probe testing the human alone (both games): retail probes model 0 only.
- Doomsday pyramid face/mail: human-only (faces the local player). The `MGC_NO_MC2_PYRAMID_TURN_RATE_IMPORT`/`MGC_NO_MC2_DOOM_BEAM_IMPORT` import items are field seats, not re-derived logic.
- Riser and building pad reconstructs reuse the native ticks (unified).
- The `(9,0)` spawn-roll absence covers both bolt spawners (`mc2_summit18_tick`, `mc2_storm_tick`).
- MC2 aura (`mc2_aura_tick`) vs MC1 magnet (`mana_magnet_tick`): different retail semantics (first-claimant map vs last-stamper overwrite). They look binary-specific. I did not verify the MC2 retail listing.

## Summary

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| WORLD-1 | Quake shove: human seat vs pool (+legacy) | DRIFTED | 3 (+drain) | y |
| WORLD-2 | Whirlwind lift: human vs pool (+legacy) | STRUCTURALLY-DIFFERENT | 3 (+teardown) | y (cave only) |
| WORLD-3 | Castle/building contact: ww vs flood | IDENTICAL | 2 | n (optional) |
| WORLD-4 | Eruption driver MC1 vs MC2 | DRIFTED (minor) | 2 | n |
| WORLD-5 | Village house tick MC1 vs MC2 | DRIFTED | 2 | n (sound ungraded) |
| WORLD-6 | Balloon fleet register MC1 vs MC2 | DRIFTED | 2 | y |
| WORLD-7 | Switch fire/rearm probe MC1 vs MC2 | DRIFTED | 4 | y |
| WORLD-8 | Dolmen stamp: tick vs carpet probes | DRIFTED | 5 | y (MC1) |
| WORLD-9 | MC1 jar poll strict vs native | STRUCTURALLY-DIFFERENT | 2 | n (native unrecordable) |
| WORLD-10 | MC1 death jar scatter human vs rival | DRIFTED | 2 | y |
| WORLD-11 | MC2 death payout human vs rival | DRIFTED | 2 | y (low) |
| WORLD-12 | Magic Mine detonation aim human vs pool | DRIFTED | 2 | y |
| WORLD-13 | Carpet AABB helpers vs inline copies | DRIFTED | ~11 | n |
| WORLD-14 | Import mana delta vs native manifestation | DRIFTED | 2 | y |
| WORLD-15 | Import castle pad stamp vs live painter | STRUCTURALLY-DIFFERENT | 2 | n |
| WORLD-16 | Terrain retile MC1 vs MC2 | IDENTICAL | 2 (+1 inline) | n |
| WORLD-17 | Metamorph ctor human vs rival | DRIFTED | 2 | n (check mc2l22) |

## Seen, but belonging to other subsystems
- **FLIGHT/walk:** the human pose-republication walk hooks (`adopt_walk_pose` for flood/whirl/hurl, `no_mc2_whirl_seat_republish`, `no_mc2_whirl_below_seat_republish`, `engine/world.rs` ~9960-10036) exist on POOLED anchors only. Native MC2 drains at the carpet step. This is a native-vs-replay twin.
- **FLIGHT/CAST:** teleport human-pose republish, landed per game separately (`no_mc2_teleport_human_pose` vs `no_mc1_teleport_human_pose` vs `no_mc1_warp_cast_pose`, `engine/world.rs` ~1947-2058).
- **FLIGHT/WORLD boundary, not examined:** MC1 `portal_tick` (`engine/world.rs` ~22260) has a human-pose warp arm plus a rival-record warp arm. The MC2 (10,34) pad (`sub_35390`) twin was not examined.
- **CAST:** MC1 `class12_tick` human `manifestation_tick` vs the `rival_*_token_tick` family (`engine/world.rs:16160-16205`). MC2 rival shield-III predecrement (`mc2/rivals.rs:886`) vs human.
- **CREATURE:** `mc2_summon_lock_ok` human (`!ctx.pdead`) vs pool (`act_life > 0`), `mc2/mobs.rs`. House wanted arm `mc2_arm_wanted` vs `flag_village_wanted`.
- **DEATH:** the kill-tally gate in `mc2_player_land` (killer class 3 model 0|1) vs the rival payout.
- **CASTLE:** `castle_balloons` keeps a dead `is_mc2` branch next to the live MC2 `mc2_castle_roster` (see WORLD-6).
