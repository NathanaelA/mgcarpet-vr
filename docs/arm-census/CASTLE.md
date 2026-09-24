# CASTLE — arm census (castles and castle pieces, MC1/HW + MC2)

Built in four parts: A = MC1/HW castle cast, register, price and ball; B = MC2 castle cast, price, lock, upgrade and register census; C = destruction, damage, turrets, payout and balloons (both games); D = the shared (3,2) castle constructor. Read-only research on the 2026-09-24 tree (branch re_recording). Each part keeps its own "other subsystems" list, reproduced where it appears.

# CASTLE census, part A: MC1/HW castle arms

Scope: MC1 and Hidden Worlds (HW) only, human and rival columns. MC2 castle arms are covered in part B.
All paths are relative to `crates/mgc-sim/src/`. Line numbers are from the working tree on 2026-09-24.
Everything here was read-only. Each difference below was checked by reading both code bodies.

---

### CASTLE-A1: Create-Castle token machine (full-tick fire: gate → debit → ball mint → latch)
- **Retail routine(s):** `sub_57610_57B40` (CARPET.EXE, with a byte-identical twin in remc1hw :62103-06). It is the (12,16) Create-Castle token's own per-tick machine. It gates through `sub_55DD0`, debits through `sub_55E80`, mints the (9,10) castle ball through `sub_373F0`, splits create vs upgrade, and latches `+48 = +50-1`. It is one routine with no human/AI fork.
- **Port arms:**
  - `engine/world.rs:16907 manifestation_tick` (the `spell == 16` block, about :16920-16985) plus `engine/world.rs:15586 cast_castle`. Column: HUMAN/MC1+HW. `cast_castle` is also the MC2 human's mint (reached from `mc2/cast.rs:4837`).
  - `mc1/rivals.rs:3143 rival_castle_token_tick`. Column: RIVAL/MC1+HW.
  - (dead leftover) `engine/world.rs:13886` `cast_spell`'s `id == 16` block, using `castle_lock_active` / `castle_build_lives` (world.rs:14739-14763). `mc1_cast_command` (world.rs:~6146-6215) routes spell 16 through the latch path and returns before it ever calls `cast_spell`. So this block looks unreachable for MC1. It is still in the tree as a third copy of the "recast while building fizzles" rule.
- **Status:** DRIFTED. Several laws have a kill switch in one arm only, and the ball-stamp sources differ.
- **Differences:**
  - **Debit order (bill before mint).**
    - HUMAN: always debits before `cast_castle` (`self.mana_debit(...)` at the start of the full arm). There is no kill switch.
    - RIVAL: gated by `MGC_NO_MC1_CASTLE_BALL_DEBIT_ORDER` (`castle_ball_debit_order`, mc1/rivals.rs:567, wave 115 dig D23). The OFF arm restores spawn-then-bill.
    - Both are bill-first by default. Only the A/B coverage differs.
  - **Dry-pool latch retry (latch only inside `if (v3)`).**
    - HUMAN: gated by `MGC_NO_MC1_CASTLE_TOKEN_DRY_POOL_RETRY` (features.rs:4047, round 152 w152e law 2). The OFF arm latches `count-1` even when nothing was minted.
    - RIVAL: hard-coded early `return` before the latch ("stays FULL, re-fires next tick"). There is no switch.
    - Same default, different A/B coverage.
  - **Gate legs.**
    - HUMAN hard fail (every latched tick) is `player.state != Alive || (player.mana as i32) < 0`, bypassed by `dev_spells`. The full-tick leg is `mc1_token_gate(16)`: Alive, castle_req against `player_castle_bound()`'s `f140`, and `player.mana >= spell_cast_cost(16)`. A failure plays buzz `snd_player(29)`.
    - RIVAL is `ent[owner].act_life < 0 || f140 < 0 || (full && f140 < rival_castle_price)`, reading the owner record's live `+140`. `MGC_NO_MC1_RIVAL_TOKEN_GATE_LIVE_PURSE` restores a cached-purse read. There is no buzz and no castle_req leg.
    - The castle_req leg is inert for spell 16: `SPELLS[16].castle_req = 0`, mc1/spells.rs:149.
    - The purse source differs by column: the human's `player.mana` mirror vs the rival's live record `+140`.
  - **Price source.** Human `spell_cast_cost(16)` vs rival `rival_castle_price`. See CASTLE-A2.
  - **Ball `+44`.**
    - HUMAN: `SPELLS[16].damage` (10000), a static-table read.
    - RIVAL: the token's own `+44` (`*(v3+44) = *(a1+44)`).
    - The values match only while the token's `+44` still holds the ctor damage.
  - **Ball `+140`.**
    - HUMAN (MC1): `spell_cast_cost(16) / count`, recomputed.
    - RIVAL: a straight copy of the token's `+140`.
    - The MC2 human arm of `cast_castle` already copies the token's `f140`. The comment at world.rs:~15770 says "MC1 reaches cast_castle with token: None".
    - The values diverge whenever the token's `+140` is not `+136/101`, for example after `mc1_respawn_reprice`'s `/f50` or when the respawn reprice leaves `f140` unwritten (div==0).
  - **Launch position.**
    - HUMAN: `muzzle(p, right)`, a hand offset of ±256 at yaw∓512 with the inside-terrain revert. It is patched to the carpet axis when `castle_latch_bug` is patched. `z = p.z + PLAYER_HH`.
    - RIVAL: spawns at the wizard record's axis, then `z += f84`.
    - The rival's doc (mc1/rivals.rs:3132-35) argues the muzzle is a no-op for rivals: no 0x100/0x200 fire bits, so `sub_55EF0` doesn't sidestep. So the difference is state-driven, not a routine split. It is unverified for a rival that shares a hand register.
  - **Create/upgrade split.** Human `player_castle_bound()` (`MGC_NO_CASTLE_BIND_REGISTER`) vs rival raw `wiz_castle_reg` (`MGC_NO_MC1_RIVAL_CASTLE_TOKEN_REGISTER`, round 152 w152k). Both default to the register. The OFF arms are the same (scan + `f26 > 0`).
  - **Upgrade ball's dest triple.**
    - HUMAN: stamps `dest_x/dest_y` only in the create arm. `MGC_NO_MC1_CASTLE_UPGRADE_BALL_NO_DEST` (world.rs:2095) restores the stamp on upgrade.
    - RIVAL: stamps only in the create arm, and there is no switch.
    - `site_z` is gated by `MGC_NO_MC1_BOLT_DEST_STAMP` in both arms (identical).
  - **Launch sound.** Human `snd_player(15)` (local-player channel) vs rival `snd(15, ball)`. Sound is not graded.
  - **Owner-gone stall.** The rival has an explicit `owner slot == 0 → return` (a stall). The human has no equivalent; the human is never absent.
  - **Command-site arm (different retail routines, listed for completeness).**
    - Human `mc1_cast_command` (`sub_46B00_46E40` :55901-11): fizzle on `+48 != 0`, a silent `pre_mana < cost` refusal, then arm `+48 = +50`. The castle-less human always goes through the create ball.
    - Rival `rival_cast_castle` (mc1/rivals.rs:5731, `sub_155F0` case 0x10): `+48 != 0 || signed purse < price` refuses. A bound rival arms the token plus cooldown. A castle-less rival does a free direct plant (`spawn_class3(2, site)`, binds `castle_reg` at level 0) with no ball.
    - These are two distinct retail routines. This is not drift.
- **What reaches each arm in-game:**
  - HUMAN: in MC1 or HW, the player casts Create Castle. The first cast mints a create ball. A cast while a castle is bound mints an upgrade ball that homes on the castle.
  - RIVAL: an MC1 or HW AI wizard with a bound castle (standing, or planted this run) enters its Upgrade state and commits spell 16. Castle-less rivals never reach the token machine; they plant directly.
  - Dry pool (both arms): the pool must be exhausted at the fire tick (999 live records). This happens in late, crowded levels such as mc1l25 and mc1l49.
- **Already witnessed?:**
  - HUMAN:
    - yes (mc1l25 fixture "THE CASTLE TOKEN'S LATCH IS INSIDE `if (v3)`", t=7014 slot 674: debit-first plus dry-pool retry)
    - yes (mc1l48 "A DEAD OWNER'S CASTLE TOKEN FREEZES … RELEASES ITS LATCH", t=27413: mid-latch hard-leg release)
    - yes (mc1l0 "THE CASTLE LOCKOUT IS A LATCH" / "THE CASTLE BALL EATS THE CHARGE METER")
    - upgrade-ball no-dest is witnessed at mc1l2 t=567 per the switch doc (a raw-shadow lane, no fixture found).
  - RIVAL:
    - yes (mc1l49 "THE CREATE-CASTLE TOKEN BILLS BEFORE IT ASKS THE POOL", t=15551: rival debit-first plus the latch holding 101 on a dry pool)
    - yes (mc1l26 "THE RIVAL CASTLE TOKEN'S CREATE/UPGRADE SPLIT READS wizext+50", t=25107)
    - yes (mc1l5 "THE RIVAL'S CASTLE UPGRADE IS THE REAL CAST CHAIN").
  - Ball `+44`/`+140` source: no evidence found that a take has a token whose `+140 != +136/101` at a fire tick.
- **Recording ask:** The main laws are witnessed on both columns. The arm split that is still unwitnessed is the ball's `+140`/`+44` source. To exercise it: in MC1, let a castle-owning rival (or yourself) die while holding a castle, respawn at the castle (this runs the `/+50` re-price), then immediately cast Create Castle to upgrade. Compare the ball's `+140` on the human and rival sides. This is only interesting if the respawn re-price leaves the token at a value other than `cap/101`, which it does not in normal play, so the lane is effectively latent. Low priority; about 5 minutes on any MC1 level with a castle. A dry-pool castle cast by the human in HW would confirm the HW twin (remc1hw :62103-06 is claimed byte-identical).
- **Confidence / notes:** High on the switch-coverage differences. The dead `castle_lock_active` path was confirmed by reading `mc1_cast_command`'s spell list, which includes 16 and returns before `cast_spell`.

### CASTLE-A2: Create-Castle price resolver (the token's live `+136`)
- **Retail routine(s):** retail has no separate pricing routine. It reads `pool[wizext+676[16]].+136` inline in `sub_55DD0` (the gate), `sub_55E80` (the debit) and `sub_155F0` / `sub_15E90` (the rival commit and the cascade price test).
- **Port arms:**
  - `engine/world.rs:13725 spell_cast_cost` (the id==16 branch). Column: HUMAN/MC1.
  - `mc1/rivals.rs:3635 rival_castle_price`. Column: RIVAL/MC1.
  - `mc1/rivals.rs:~3699-3760 rival_selector`'s `castle_arm_registers` arm. It reads `pool[owned[16]].+136` raw with no class/liveness guard (`sub_15E90`). This is a third reader.
- **Status:** DRIFTED (the guard shapes differ).
- **Differences:**
  - Token validation:
    - HUMAN requires `class64 == 12 && !0x400 && f136 > 0`. Otherwise it falls back to `SPELLS[16].possess_mana` (1000).
    - RIVAL requires `class64 == 12 && !0x400`. It returns `f136.max(0)`, so a token with `f136 <= 0` costs **0**, not 1000.
    - The selector arm (`castle_arm_registers`, `MGC_NO_MC1_CASTLE_ARM_REGISTERS`) takes `+136` with no class/liveness test, which matches the retail comment.
    - That is three guard shapes for one retail read. Retail itself has no fallback and no liveness test.
  - A dead or recycled `owned[16]` slot: the human pays 1000 from the static table and the rival pays 1000 from the static table (the rival's fallback also applies when the class/liveness check fails). The selector reads whatever lives in the slot.
- **What reaches each arm in-game:** HUMAN: any Create Castle cast or recast in MC1/HW. RIVAL: any rival castle commit or upgrade. The two arms only differ when the owned[16] register names a record that is not a live class-12 token, or a token with `+136 <= 0`. That needs slot aliasing (a recycled slot still named by the register, as seen in mc1l49 t=32741, where two wizards hold `owned[16] = 26`).
- **Already witnessed?:** HUMAN: yes for the normal path (the corpus prices 9/49/99/198, cited in `cast_castle`). RIVAL: yes (mc1l49 "THE CASTLE COMMIT RESOLVES ITS TOKEN BY INDEX"). The alias/dead-slot case: no evidence found for either column.
- **Recording ask:** Mostly latent. The differing condition needs an `owned[16]` register naming a recycled slot. That happens in pool-starved late MC1 takes (mc1l49-style) when a castle token is scattered on death and its slot is re-minted as something else before the owner re-picks one. You can't reliably force it. If you record a long pool-starved MC1 endgame (mc1l48/mc1l49) with rivals dying and re-casting castles, the lane may show up.
- **Confidence / notes:** Medium. I did not trace whether `owned[16]` can go stale natively in the port, since `rival_owned_rebuild` rebuilds it every respawn.

### CASTLE-A3: `wizext+50` castle-register readers, human vs rival (the "castle bind" family)
- **Retail routine(s):** every "do I have a castle" read in the MC1 binary is `pool + 164*wizext[+50]` with an index test only. The sites:
  - the respawn seat `sub_44D30` :54861 (shared by both columns)
  - the `sub_47DD0` re-price :55034 (shared)
  - the token gate `sub_55DD0` :64923 (shared)
  - the create/upgrade split `sub_57610` :65893 (shared, see A1)
  - the at-castle probe, which has **two distinct retail routines**: the human's `sub_45C90` :55346-52 and the rival's `sub_132B0` :17971
  - the dead-wait `sub_46480` :55609
  - human-only readers: demolish :55838, mail redirect :55353-62, teleport :65574, objective/win :52122/:67299, HUD :27214
  - rival-only readers: Home `sub_14310` / `sub_14DC0` / `sub_13A70`, and the cascade `sub_13F00` / `sub_145B0`.
- **Port arms:**
  - `engine/world.rs:14831 player_castle_bound` (`Gen::castle_reg[0]`). HUMAN, switch `MGC_NO_CASTLE_BIND_REGISTER`, whose OFF arm is the `player_castle()` pool scan. Readers:
    - `regen_boost` world.rs:5677
    - demolish :6003
    - `mc1_token_gate` :6225 and `mc1_token_gate_hard` :6263
    - mail redirect :6409
    - the objective :11088
    - `player_respawn` seat :11837 and re-price :11993
    - `cast_teleport` :14386
    - `spell_gate` :15526
    - `cast_castle` :15603
    - the shield token :16099
    - the HUD :17551
    - the win :21206
  - `mc1/rivals.rs:1819 rival_castle_reg` / `mc1/rivals.rs:4393 wiz_castle_reg`. RIVAL, with three separate switches:
    - `MGC_NO_MC1_HOME_CASTLE_REGISTER` (features.rs:5333, round 155) on `rival_castle_reg`: the Home arms, the respawn seat, the dead-wait, the token gate, the cast-16 split in `rival_cast_castle`.
    - `MGC_NO_MC1_RIVAL_CASTLE_REGISTER` (mc1/rivals.rs:1051) on the inline `wiz_castle_reg` reads: at-castle :1986, the cast-16 bound arm :5575, the respawn re-price :6628.
    - `MGC_NO_MC1_RIVAL_CASTLE_TOKEN_REGISTER` on the token split :3244.
  - Pair: human `regen_boost` (world.rs:5677) vs rival at-castle probe (mc1/rivals.rs:1986, inside `rival_alive_tick`).
  - Pair: human `player_respawn` (world.rs:11813) vs `rival_respawn` (mc1/rivals.rs:6543).
  - Pair: human `mc1_token_gate`/`_hard` vs `rival_token_gate` (mc1/rivals.rs:~3395).
- **Status:** IDENTICAL on the default arms: every listed reader in both columns reads the raw `castle_reg` word with a nonzero-and-in-bounds test. DRIFTED in A/B coverage and in the OFF arms.
- **Differences:**
  - **Kill-switch granularity.** The human has one switch for every reader. The rival has three switches, split by reader. The OFF arms differ in shape:
    - human OFF = scan, plus `f26 > 0` at the split/demolish/regen sites. Regen also has `MGC_NO_CASTLE_BIND_LEVEL`, which drops the `f26` filter.
    - rival at-castle OFF = scan + `flags & 2` (the first-commit latch).
    - rival split OFF = scan + `f26 > 0`.
    - rival home OFF = bare scan.
    - So "the pre-dig tree" is not one tree across the columns.
  - **At-castle overlap operand.** HUMAN `player_overlap(c, ctx)` uses the constant carpet half-extents (`PLAYER_HW` / `PLAYER_HH`) because the human's pool record is not live. RIVAL `ent_overlap(i, reg)` uses the rival record's own `f80/f82/f84/f78`. Both are summed-AABB, signed extents. They agree only if the rival's recorded extents equal `PLAYER_HW/HH`.
  - **At-castle consequence (retail's own asymmetry, two routines).** The HUMAN redirects the ch0 mail into the castle (world.rs:~6404). The RIVAL sets grace = 2 and discards the mailbox. The comment cites :17971-79 vs :55353-62; these are different retail routines. This is not drift.
  - **Castle-less respawn.** The HUMAN seats at the authored start at ground+256 and sets permadeath/`lost` (or WON if `completed`). The RIVAL returns early (seat unchanged). Elimination is decided earlier, in the rival dead-wait (mc1/rivals.rs:6512). This is one retail body `sub_44D30`; the human reaches it via case 0xF (Space) and the rival via its dead-wait countdown.
  - **Respawn re-price gate.** HUMAN `player_castle_bound()` + `MGC_NO_MC1_HUMAN_RESPAWN_REPRICE`. RIVAL raw `wiz_castle_reg` + `MGC_NO_MC1_RIVAL_CASTLE_REGISTER` + `MGC_NO_MC1_RESPAWN_REPRICE_OWNER`. Both default to the same shared `mc1_respawn_reprice` (mc1/rivals.rs:4407). The OFF arms differ: the human keeps `flags & 2`, the rival keeps `f26 > 0 && flags & 2`, and they use different token sources.
  - **Snapshot rebuild.** Not a column split, but it affects the rival more: `World::rederive_castle_reg` (world.rs:24739) rebuilds the register from `f26 >= 1` castles only. A rival castle planted at level 0 (bound at the plant, :19206) is therefore unbound after a snapshot restore until its commit. This is native snapshot only and is not graded.
- **What reaches each arm in-game:**
  - HUMAN, MC1/HW: hover over your own castle (regen boost, mail redirect); die with a castle (respawn seat and re-price); cast a castle-gated spell (token gate); Shift+L demolish; teleport home.
  - RIVAL, MC1/HW: an AI wizard hovers at its own keep (regen), dies and respawns at its keep, flees Home, or casts a castle-gated spell.
  - The columns can only differ on the default arm through the overlap operand: a rival carpet brushing the edge of its own castle's box, where its recorded extents differ from the human constants.
- **Already witnessed?:**
  - HUMAN:
    - yes (the mc1l0 t=1827 regen probe, cited in code)
    - yes (mc1l6 t=3907 bind level, cited in code)
    - yes (mc1hwl0 t=7629 human respawn re-price, cited)
    - yes (mc1l0 "THE CASTLE-LESS RESPAWN FALLS THROUGH TO THE AUTHORED START")
    - yes (mc1l49 "THE TOKEN GATE IS PER TICK", castle 353 is the human's).
  - RIVAL:
    - yes (mc1l20 slot 534, the at-castle register; mc1l5 t=11681 AABB edge, cited)
    - yes (mc1l27 "THE RIVAL GOES HOME TO HIS wizext+50 REGISTER" ×3)
    - yes (mc1l35 "A STALE REGISTER STILL RESPAWNS THE RIVAL")
    - yes (mc1l26 split, mc1hwl0 "THE PICK GATES READ THE RAW WIZEXT+50 REGISTER").
- **Recording ask:** For the default arms, only the at-castle overlap operand remains unwitnessed. It is the human carpet's constant box vs the rival record's extents. To exercise it: in MC1, hover your carpet at the very rim of your own castle's footprint, moving slowly in and out. The +1000 vs +100 mana/tick switch should flip exactly at the summed box edge. Do the same while a rival hovers near its keep's edge (for example, harass a rival so it flees Home). About 3 minutes on any MC1 level where you own a castle; mc1l5 is natural because Vodor camps his keep.
- **Confidence / notes:** High. All three rival switches and the one human switch read the same `Gen::castle_reg` array, so a collapse is mechanically simple. Merging the OFF arms is the only design choice.

### CASTLE-A4: (10,43) upgrade-token delivery — its castle resolution
- **Retail routine(s):** MC1 `sub_293D0` (remc1 :31009-40) and MC2 `sub_389F0` (EF:28265-97) are the same shape. The upgrade token dereferences its owner's castle register and tests overlap. A HIT mails the castle `(10, owner)` on mail[5]; a MISS releases the owner's charge pin (`sub_46D20` / `sub_5F890`).
- **Port arms:**
  - `engine/features.rs:~9086-9235` (the (10,43) upgrade-token tick). One function with two in-body arms:
    - the MC1/HW arm (the `else` at :9181): a POOL SCAN for the owner's `(3,2)` with `f26 > 0 && !0x400`, lowest slot first.
    - the MC2 arm (:9163-9180): `mc2_castle_reg_of(own)`, the register, with switch `MGC_NO_MC2_TOKEN_CASTLE_REGISTER`.
  - For contrast, every other MC1 `wizext+50` reader (A3) is already on the register. That includes the ball that CARRIES this token: `cast_castle` / `rival_castle_token_tick` stamp `ball.f146 = register castle`.
- **Status:** DRIFTED. The register law (ROADMAP round 139: "`CastleEntityIndex_0x3A_58` register read … the arm that missed it: the (10,43) upgrade token") landed on the MC2 arm only. The MC1 arm still carries the `f26 > 0` stand-in and has no kill switch.
- **Differences:**
  - MC1 resolves "the owner's lowest-numbered live castle at level ≥ 1". MC2 resolves "the register's record, whatever it is". So in MC1:
    - (a) **Two castles.** If the owner has two castles (a split), the token spawns at the register castle (the ball homed on `f146`), but the scan picks the lower slot. If that is the other castle, the overlap fails, so the token MISSES, releases the pin and no upgrade happens.
    - (b) **Level-0 bound castle.** A castle bound at level 0 (a rival plant, :19206) makes the scan return None, so the token misses. The register would HIT.
    - (c) **Stale register.** A register naming a re-minted slot makes the scan miss and the register hit whatever is there (it mails that record).
  - The OFF behaviour (the scan) is simply the MC1 default. There is no MC1 switch to A/B it.
- **What reaches each arm in-game:**
  - MC1/HW, human or rival: the upgrade ball reaches its castle and morphs into the (10,43), for every castle upgrade.
  - The arms differ only when (a) the owner owns two castles at once, or (b) a rival plants a castle and its upgrade ball arrives before the plant's first level-up commits.
- **Already witnessed?:**
  - MC1 common path (scan == register): yes (mc1l5 t=5152 rival upgrade "THE RIVAL'S CASTLE UPGRADE IS THE REAL CAST CHAIN"; human upgrades mc1l0 t=2174, mc1l2 t=567, cited in code).
  - MC1 differing condition: no evidence found.
  - MC2 arm: witnessed per the switch doc (mc2l7; part B).
- **Recording ask:** In MC1, get the human to own **two castles at once** and then cast an upgrade. The port has a test (`mc1_a_second_castle_ball_builds_a_second_castle_only_on_retail`, world.rs:56890) claiming retail's plain create arm has no owner test, so a create ball that lands far from your existing castle raises a second one. If you know a way to land a second create ball (for example two casts spaced so the first castle hasn't bound yet), do that, let both castles reach level ≥ 1, then cast Create Castle repeatedly. Watch which castle grows, or whether upgrades fizzle.

  Alternative: a rival that plants and upgrades within a few ticks (mc1l26-froze t=25107 is exactly this: plant at 25107, upgrade ball at 25108). If that take's (10,43) arrives before castle 985 commits level 1, the take already witnesses it. Check the (10,43) around t=25108-25130 in mc1l26-froze before recording anything new. About 5-10 minutes if a new recording is needed.
- **Confidence / notes:** High that the code differs. Medium on how reachable it is in retail.

### CASTLE-A5: MC1 castle ball — create arm vs homing (+146) arm
- **Retail routine(s):** `sub_53980` (:63453) dispatches on `+146`. Nonzero takes the homing arm (:63459-518). Zero falls to `sub_53B50_53E90` (:63525-621), the create/placement arm. Both are inside one retail dispatcher, with two bodies. The ball is shared by human and rival owners.
- **Port arms:**
  - `mc1/combat.rs:3951 proj_castle_ball_tick`. CREATE arm; also a leftover `upgrade` (f69==43) block that is MC2/legacy.
  - `mc1/combat.rs:4196 castle_ball_homing_tick`. HOMING arm.
- **Status:** STRUCTURALLY-DIFFERENT (retail has two bodies here). It is DRIFTED in one shared concern: the "delivery refused because the owner already holds a castle" test.
- **Differences:**
  - **Dry-pool handling.**
    - CREATE: the ball survives and retries, gated by `MGC_NO_MC1_CASTLE_BALL_DRY_POOL_RETRY` (features.rs:4002, round 152 w152e). There is no pin release.
    - HOMING: the ball survives and releases the owner's charge pin (`release_castle_charge_pin`). There is no switch.
    - Retail has the same asymmetry, per the comments (:63513-15 vs :63606-11).
  - **Owner-castle test at delivery.** The HOMING arm's `f68 == 3` morph refuses when the owner has a live `(3,2)` with `f26 > 0`: a POOL SCAN standing in for `wizext+50` (:63500-04). This is not the register. The CREATE arm has no owner test in retail mode; the patched arm uses the `castle_owned_by` scan.
  - **Reachability.** Neither `cast_castle` nor `rival_castle_token_tick` ever mints a homing ball with `f68 = 3`: create balls have `f146 = 0`, upgrade balls have `f68 = 10`. So the homing arm's scan-vs-register test looks unreachable in native MC1 play. It is only reachable through a conformance import of such a ball.
  - **Step-back.** `MGC_NO_MC1_CASTLE_BALL_STEPBACK_MOVES_BALL` is CREATE-arm only. The homing arm has no step-back.
- **What reaches each arm in-game:**
  - CREATE: the human's first Create Castle cast in MC1/HW, or a recast after the castle died. Rivals reach it only if their register is 0 at the token's fire tick, because castle-less rivals plant directly.
  - HOMING: every castle upgrade, human or rival.
- **Already witnessed?:**
  - CREATE: yes (mc1l25 "A CASTLE BALL SURVIVES A DRY-POOL CTOR", t=2615; mc1l48 "THE CREATE-CASTLE BALL'S REFUSED-SCAN STEP-BACK MOVES THE BALL ITSELF", human ball; mc1l32-castle-bug).
  - HOMING: yes for the flight (mc1l0 t=2174-77, cited). The homing pool-full pin release: no evidence found.
  - RIVAL-owned create ball: no evidence found. It is probably unreachable except via a register cleared between arm and fire.
- **Recording ask:** A homing (upgrade) ball arriving at its castle with the pool **exhausted**. The morph fails, the pin releases, and the ball retries. To set it up: in a crowded late MC1 level (mc1l49-style, 999 records), cast a castle upgrade while the pool is full. About 5 minutes, but it depends on reaching pool exhaustion. The `f68 == 3` homing test needs no recording; it is unreachable natively.
- **Confidence / notes:** Medium-high. The leftover `upgrade` block inside `proj_castle_ball_tick` (reached only when `f146 == 0 && f69 == 43`, or non-MC1 movement) is part B's concern.

### CASTLE-A6: resolving the owner's Create-Castle token (`wizext+708` = `owned[16]`) for pin, release and re-price
- **Retail routine(s):**
  - `sub_46D20_47060` (:55949-71): pins `+48 = +50-1` or releases it to 0 on the owner's `wizext+708` token.
  - `sub_47C60_47FA0` (:56572-84): the ladder stamp through the owner's `+708`, gated on owner `+70 <= 1u`.
  - `sub_47DD0` (:56617-73): the respawn re-price, same resolution.
  - All three are a bare index through the OWNER's wizext.
- **Port arms:**
  - `engine/world.rs:12648 castle_owner_token`. Used by the castle-tick PIN/RELEASE stamps (world.rs:~9690) and by the ladder-event stamp (world.rs:~9810).
    - Human: `player.owned[16]`, validated class 12, model 16, alive, with no fallback.
    - Rival: `rivals[..].owned[16]` validated the same way, then a fallback pool scan on `f144 == owner`.
  - `engine/features.rs:9241 Gen::release_castle_charge_pin`. Used by the (10,43) MISS, the homing ball's pool-full, and the create ball's launch-scan failure. It is a pool scan for the lowest `(12,16)` with `f144 == owner` and alive. It never consults `owned[16]`.
  - `mc1/rivals.rs:4407 mc1_respawn_reprice`. Human: `player.owned[16]` raw. Rival: `rivals.find(ent==own).owned[16]` raw. No class, model or liveness test (matching retail's "index-tested only").
- **Status:** DRIFTED. Three resolutions for one retail indirection.
- **Differences:**
  - The PIN (castle tick) uses the owned register first. The RELEASE (ball/token failure) uses the `f144` scan only.
  - When two wizards' `owned[16]` name the same slot (mc1l49 t=32741: slot 26 in both books), or the register names a token whose `f144` is another wizard (mc1l49 t=33081: token `+42` = 646 under wizard 594), the pin and the release can hit **different** tokens, or the release can hit none.
  - The human arm of `castle_owner_token` has no `f144` fallback; the rival arm does.
  - `mc1_respawn_reprice` takes the raw register with no model test. `castle_owner_token` requires model 16.
- **What reaches each arm in-game:**
  - PIN: any MC1 castle under blast-shake, repainting, or downgrading.
  - RELEASE: an upgrade ball or token that misses its castle, a create ball refused at launch, a homing ball on a full pool.
  - The resolutions only differ under token-slot aliasing, which happens in pool-starved late MC1 levels where tokens are scattered and re-minted.
- **Already witnessed?:**
  - PIN: yes (mc1l0 "THE CASTLE LOCKOUT IS A LATCH"; mc1l49 t=17810 leveler pin gate, cited).
  - RELEASE via the create ball's launch refusal: the launch-scan failure is witnessed in mc1l32-castle-bug (DEVIATIONS.md).
  - The aliasing case: no evidence found for the release path.
  - The re-price raw index: yes (mc1l35 "sub_47DD0 RE-PRICES THE CASTLE OWNER'S TOKEN").
- **Recording ask:** Needs the aliasing condition. In a long MC1 endgame with a starved pool and several castle-owning rivals dying and respawning (mc1l49-like), cast castle upgrades from spots where the ball's launch scan is refused (next to another castle, within 2048). Hard to force. Record opportunistically in a long late-level session; ungraded unless `+48` on the token lanes diverges.
- **Confidence / notes:** Medium. Whether retail's `sub_46D20` release really goes through `wizext+708` of `pool[ball.+24]` is taken from the port's own doc (features.rs:9230-40); I did not open the listing.

### CASTLE-A7: castle→token ladder writers (level-event stamp vs respawn re-price)
- **Retail routine(s):**
  - `sub_47C60_47FA0` → `sub_47BD0_47F10` (:56540-84). Its three callers are the level-up commit :56481, the teardown :56528 and the setup :54996. It writes `+136 = cap` and `+140 = cap / movswl token.+50`.
  - `sub_47DD0` (:56617-73). Its one caller is the respawn :55034. It uses an unsigned switch on `+26` whose default is 0, and `/ token.+50`.
  - These are two retail routines with the same arithmetic.
- **Port arms:**
  - `engine/world.rs:~9792-9840`, the ladder-event stamp in `tick_inner`'s castle case (both columns).
  - `mc1/rivals.rs:4407 mc1_respawn_reprice` (both columns).
- **Status:** DRIFTED (the arithmetic differs).
- **Differences:**
  - Divisor: the event stamp uses `f140 = cap / 101` (a literal). The re-price uses `cap / token.f50`, skipping the write when `f50 == 0`.
  - Level range: the event stamp uses `CASTLE_CAP[min(lvl, 7)]`, a clamp. The re-price uses `lvl <= 7 ? CAP[lvl] : 0`, retail's default 0, with no clamp.
  - The event stamp also carries its own gate logic:
    - `MGC_NO_MC1_CASTLE_LADDER_EVENT`: per-tick vs event-driven.
    - `MGC_NO_MC1_CASTLE_DOWNGRADE_REPRICE`: the bind stand-in `flags & 2`.
    - an owner-alive test (human `player.state == Alive`, rival `tick70 <= 1`) that matches the re-price's.
  - The teardown-to-0 arm writes `CAP[0]/101`.
- **What reaches each arm in-game:** the event stamp runs on any castle level change (upgrade commit, downgrade, raze) in MC1/HW. The re-price runs when any wizard respawns at a bound castle. The arms only differ if the token's `+50` isn't 101 (it always is for a genuine (12,16), since `SPELLS[16].count = 101` in both MC1 and HW), or if a castle level exceeds 7 (not reachable).
- **Already witnessed?:** Event stamp: yes (mc1l5 "LADDER STAMP IS BIND-GATED", mc1l45 "A CASTLE LEVEL EVENT NEEDS NO BIND", mc1l48 dead-owner freeze). Re-price: yes (mc1l35, mc1hwl0 t=7629/t=20761 cited).
- **Recording ask:** None useful. The differing condition (a token `+50` other than 101, or level > 7) is not reachable with genuine tokens. It could only show through `mc1_respawn_reprice`'s raw index naming a non-(12,16) record, which mc1l35 already witnesses on the re-price side.
- **Confidence / notes:** High on the code. The "`/+50`" for `sub_47BD0` comes from the `mc1_castle_ladder_needs_bind` doc (world.rs:3432-35).

---

## Clusters seen that belong to other subsystems (or to part B)
- **The human Shield token's inline gate** (engine/world.rs:~16092-16105) re-implements `mc1_token_gate_hard` / `mc1_token_gate` inline instead of calling them. This is a token-gate twin (SPELL TOKENS subsystem).
- **The MC2 objective's castle term** (engine/world.rs:19278, `0 =>` arm) reads `player_castle().filter(f26 > 0)`, a scan plus level stand-in. The long comment above it describes `CastleEntityIndex` as the real gate. It is not on `player_castle_bound()` like the other MC2 register readers (demolish/port/price/regen). Part B / OBJECTIVES.
- **MC2 human respawn seat** `mc2_player_respawn` (world.rs:12497) uses the `player_castle()` scan, not the register. The MC1 human respawn (A3) uses the register. Part B.
- **MC2 `mc2/cast.rs` castle-lock / price readers** still on `player_castle()` (mc2/cast.rs:2250, 2395, 2592, 3367, 4196, 4558). Part B.
- **The MC1 vs MC2 castle level-up commit / teardown register writes:** features.rs:10321 / 10706 (MC1 `sub_47960` / `sub_47A70`) vs mc2/castle.rs:867 / 1073 (MC2 `sub_60480` / `sub_605E0`). These are different retail routines; part B.
- **The snapshot register rebuild** `World::rederive_castle_reg` (world.rs:24739): `f26 >= 1` only, so it drops level-0 rival plant binds (and MC2 AI level-0 registers). This is snapshot/save, not graded.
- **`proj_castle_ball_tick`'s `upgrade` / `!mc1` branches** duplicate an MC2 castle-ball flight next to `mc2/proj.rs mc2_castle_ball_tick`. Part B.

---

# CASTLE — part B: MC2 castle CAST / PRICE / LOCK / UPGRADE arms (human vs rival)

Scope: MC2 (NETHERW.EXE) only, unless a cluster says otherwise. All paths are under
`/home/rain/projects/mgcarpet/crates/mgc-sim/src/`. Line numbers are as of the 2026-09-24 tree.

Background you need for B3-B6: MC2 keeps a per-wizard **castle register** in retail,
`CastleEntityIndex_0x3A_58`. The port stores it in `Gen::castle_reg[team]` for every wizard,
human and rival alike. It is written by the level-up commit (`mc2/castle.rs:867`) and zeroed by
the level-0 teardown (`mc2/castle.rs:1073`). The port also has three **pool-scan** helpers that
return the lowest-numbered live (3,2) the owner has:
- `World::player_castle` (`engine/world.rs:14767`)
- `World::rival_castle` (`mc1/rivals.rs:1802`, used by both games)
- `Gen::mc2_castle_of` (`mc2/roster.rs:5901`)

The two register accessors are `World::player_castle_bound` (`engine/world.rs:14831`, human only)
and `Gen::mc2_castle_reg_of` (`mc2/roster.rs:5954`, any owner). A scan and the register give
different answers in three cases:
- **(a) CASTLE SPLIT:** one owner has two live castles, e.g. a second castle ball lands inside
  the register lag; mc2l12 is the corpus example.
- **(b) FRESH LEVEL-0 CASTLE:** the ball has minted the (3,2), but the level-up commit has not
  written the register yet. The scan already finds it.
- **(c) DYING CASTLE:** the teardown has zeroed the register, or the record already carries the
  0x400 reap bit.

---

### CASTLE-B1: Castle-lock RELEASE and the deferred-tier drain (`sub_5F890` a2==0)
- **Retail routine(s):** `sub_5F890` (NETHERW, VA 0x5F890 / file 0x84090). It is one owner-generic
  routine. At every castle transform edge it pins or releases the castle owner's spell-2
  manifestation (the "castle upgrade lock"). Its release arm calls `sub_6D880` on the CASTLE,
  so the pending tier lands one tick later, in `sub_69AB0`'s spent-timer entry arm.
- **Port arms:**
  - `mc2/cast.rs:4253 mc2_castle_lock_stamp`: the SHARED dispatcher. The pin is shared; the
    release forks on `own == PLAYER_TARGET`.
  - `mc2/cast.rs:4187 mc2_castle_lock_release`: HUMAN/MC2.
  - `mc2/cast.rs:4513 mc2_rival_castle_lock_release`: RIVAL/MC2.
  - Deferred-tier apply, human: `mc2/cast.rs:4053 mc2_castle_spell_tick` entry arm (via
    `mc2_cast_expire(2,m)`): HUMAN/MC2.
  - Deferred-tier apply, rival: `mc2/rivals.rs:9009 mc2_rival_castle_token_tick`, the `f26 <= 0`
    arm: RIVAL/MC2.
- **Status:** DRIFTED. The main law has now reached both arms; residual differences remain.
- **Differences:**
  - **Tier-defer lag.** It landed first in the HUMAN arm (`MGC_NO_MC2_CASTLE_TIER_DEFER_LAG`,
    mc2l7 witness t=25588/25589). It reached the RIVAL arm later under its OWN switch
    (`MGC_NO_MC2_RIVAL_CASTLE_TIER_DEFER_LAG`, mc2l12 fixture "LANDED FOR THE HUMAN COLUMN AND
    NEVER FOR THE RIVAL TWIN"). Both defaults now defer. The two switches are independent, so
    an A/B of one leaves the arms out of step.
  - **Pre-law body.** With the kill switch set:
    - the human release runs the full `mc2_cast_expire(2,m)`: pending tier plus the
      player-effect `match`, where spell 2 has no arm;
    - the rival release does a bare `f44 → mc2_rival_set_spell`.

    Behaviour is equivalent today.
  - **Re-price pricer.** After the release, both arms re-run SetSpell on `f71`.
    - Human: `mc2_set_spell` prices through `mc2_price_castle()`, the **REGISTER**.
    - Rival: `mc2_rival_set_spell` prices through `rival_castle(own)`, a **POOL SCAN**.

    Both are gated by a pool scan (`player_castle().is_some()` vs `rival_castle(own).is_some()`).
    So in cases (a)/(b)/(c) the two arms re-price off different castles. See B3.
  - **Where the deferred-tier apply runs.**
    - Human: inside `mc2_castle_spell_tick`, which `return`s under the latch. That return skips
      the rest of the body, including any cooldown handling there.
    - Rival: inside `mc2_rival_castle_token_tick`. It is NOT gated by either kill switch, and it
      always falls through to the `f54` cooldown decrement.
  - **Where the rival tick is dispatched.** From `mc2_manifestation_pass` when
    `mc2_token_slot_dispatch()`. Otherwise from the rival brain's stand-in
    (`mc2/rivals.rs:~5345`). The two dispatch paths run at different slots.
- **What reaches each arm in-game:**
  - HUMAN: in MC2 the player owns a castle and changes the castle spell's tier in the spell
    pane while the castle is building, upgrading or downgrading (the lock is held). The tier is
    queued in `word_0x2C_44` and lands the tick after the castle settles.
  - RIVAL: a rival levels its castle spell, or its brain's tier-down walk retunes the token,
    while its own castle is transforming.
  - The pricer difference needs case (a), (b) or (c) on the release tick.
- **Already witnessed?:**
  - HUMAN: yes (mc2l7 fixture "THE CASTLE SPELL'S PENDING TIER LANDS ONE TICK AFTER THE LOCK
    RELEASE").
  - RIVAL: yes (mc2l12 fixture "THE CASTLE-SPELL DEFERRED-TIER LAG WAS LANDED FOR THE HUMAN
    COLUMN AND NEVER FOR THE RIVAL TWIN").
  - The register-vs-scan re-price on a RIVAL release: no evidence found.
- **Recording ask:** MC2, any level with an aggressive castle-building rival (mc2l6, mc2l12 or
  mc2l22 style). Damage one of the rival's castles until it downgrades, and keep doing it while
  the rival rebuilds or upgrades. Best of all: get the rival to hold TWO castles (a castle
  split), e.g. raze the castle to level 0 while its rebuild ball is in flight. About 20-40 min
  of play. The re-price lands in the class-15 `maxMana`/`mana` lanes (graded as `mana_max` /
  `mana` on the token).
- **Confidence / notes:** High on the bodies. The split scenario for rivals is hard to force.

### CASTLE-B2: The castle manifestation body and the ball MINT (`sub_69AB0`)
- **Retail routine(s):** `sub_69AB0` (NETHERW, EF:56086), one caster-generic class-15 handler.
  - Spent-timer entry arm: applies the pending tier.
  - Solvency re-test: `sub_68D50` failing clears the lock.
  - Mint on `word_0x2E_46 == word_0x30_48`: research write for `castleLevel+1`, `sub_68DE0`
    debit, `_4A190(9,10)` ball, lock latch at `@0x30-1`, then the stamps @0x26 / @0x2A / @0x90 /
    @0x10 / aim, then the create-vs-upgrade fork off `CastleEntityIndex_0x3A_58`.
- **Port arms:**
  - `mc2/cast.rs:4053 mc2_castle_spell_tick` plus `engine/world.rs:15586 cast_castle`, MC2 leg:
    HUMAN/MC2. `cast_castle` is shared with MC1 and forks on `mc2`.
  - `mc2/rivals.rs:9009 mc2_rival_castle_token_tick` plus `mc2/rivals.rs:9034
    mc2_rival_castle_mint`: RIVAL/MC2.
- **Status:** DRIFTED
- **Differences:**
  - **Create-vs-upgrade castle.**
    - Human: `player_castle_bound()`, the REGISTER (`MGC_NO_CASTLE_BIND_REGISTER` reverts it to
      `player_castle().filter(f26>0)`).
    - Rival: `rival_castle(own)`, a POOL SCAN with no level test.

    Retail reads the register for both (the EF:56487-502 quote in `cast_castle`). They differ in
    cases (a), (b) and (c). In case (b), a rival that re-casts over a fresh level-0 flag mints an
    UPGRADE token; the human arm mints a CREATE ball.
  - **Research stamp (`array_0x24E_590`).**
    - Human: stamped AFTER a successful spawn (a pool-full mint researches nothing). Tier =
      `mc2_book.sel[2]`, the SELECTED tier; the code calls this the "A.5 shortcut". Stage comes
      from the register castle.
    - Rival: stamped BEFORE the spawn, so a pool-full mint still researches, as retail
      EF:56118-21 does. Tier = the token's live `f71`. Stage comes from the scan castle.
    - `sel[2]` and `f71` differ whenever a tier change is still queued in `f44`. That happens
      if the player changes the castle tier while the lock is held, then casts on the first tick
      the lock is free and before the one-tick deferred apply (see B1).
  - **Launch point.**
    - Human: hand muzzle (`self.muzzle(p,right)`, 256 units to the firing side). z =
      `p.z + PLAYER_HH`.
    - Rival: the wizard's own axis. z = `wz + wizard.f84` (EF:56135).
    - The muzzle split is retail's `a3` hand bits (AI passes 0), so it follows one retail rule.
      The z formulas are written differently; they agree only if the human carpet's fov equals
      `PLAYER_HH`. Unverified.
  - **Lock latch.**
    - Human: after `mc2_spell_fire`, `mc2_castle_ball_aloft()` pool-scans for any live (9,10)
      the human owns, then sets `f26 = dur-1`.
    - Rival: latches directly on the spawn result (`Some(pr)`).
    - These differ only if the human already has another castle ball in the air when a fresh
      spawn fails (pool full).
  - **Debit.**
    - Human: `mana_debit(max_life)` after the fire, even when the spawn failed.
    - Rival: `mana_delta -= cost` before the spawn.
    - Retail's `sub_68DE0` comes before `_4A190`. The order is invisible unless the pool is full.
  - **Solvency re-test.**
    - Human: `mc2_afford(m)`. Liveness = `player.state == Alive`. Upkeep castle =
      `player_castle()` scan.
    - Rival: `mc2_rival_afford`. Liveness = `act_life < 0`. Upkeep castle = `rival_castle` scan.
    - The castle spell's `f136` upkeep is 0 in practice, so the upkeep leg is probably inert
      here.
  - **Re-derive arm.** The human arm keeps a whole pre-135 re-derive arm behind
    `MGC_NO_MC2_CASTLE_CAST_LATCH` (`mc2_castle_lock_active`, a live pool scan). The rival arm
    has no such arm; the switch does not reach it.
  - **Mint sound.** Rival plays `snd(15, pr)` (EF:56157). No sound call found in `cast_castle`'s
    MC2 leg. It may be elsewhere in `mc2_spell_fire` — not verified.
- **What reaches each arm in-game:**
  - HUMAN: in MC2 the player casts Create Castle (spell 2), either with no castle (create ball
    lands 4096 ahead) or with one (upgrade token homes on it).
  - RIVAL: a rival with a castle decides to upgrade. Its brain arms the (15,2), and the token's
    own tick mints the ball from the wizard's body.
  - The research-tier drift needs the player to change the Create Castle tier while a castle is
    transforming, then cast an upgrade immediately.
- **Already witnessed?:**
  - HUMAN mint: yes (mc2l0 fixture "THE MC2 CASTLE BALL COPIES THE TOKEN'S MANA"; mc2l3/mc2l4
    ball fixtures).
  - RIVAL mint: yes (mc2l6 fixture t=6428 "THE UPGRADE HOVER IS THE CAST'S FAIL ARM" — rival
    383's upgrade ball).
  - Research-tier drift: no evidence found. Register-vs-scan create/upgrade split: no evidence
    found for rivals.
- **Recording ask:**
  1. **Research tier (human).** MC2, mid-campaign, with Create Castle at tier ≥ 1 learned. Own a
     level ≥ 1 castle. Start an upgrade. While the tower rises, switch the Create Castle tier in
     the pane. Hold the cast button so it recasts the instant the castle settles. Then watch the
     next level-up spawn (or not spawn) its defender turret piece. About 10 min. Research is
     partly ungraded; the downstream piece spawn is graded.
  2. **Rival fresh-flag re-cast.** Get a rival to re-cast Create Castle while its first flag is
     still level 0 (rare; mostly luck on a level with rich rivals). About 30+ min.
- **Confidence / notes:** Medium-high. The pool-full differences are unreachable in the corpus
  (the 1,000-slot pool never fills).

### CASTLE-B3: SetSpell / GetSpellManaCost pricing (`SetSpell_6D5E0` + `GetSpellManaCost_6D710`)
- **Retail routine(s):** `SetSpell_6D5E0` (Level.cpp:1505) calling `GetSpellManaCost_6D710`
  (Level.cpp:1714). It is ONE routine for every manifestation. The spell-2 price is the
  register castle's ladder rung × tier multiplier, or raw `manaCost_6` when there is no castle,
  plus the `byte_0x1BE_446` +3000 surcharge.
- **Port arms:**
  - `mc2/cast.rs:2402 mc2_set_spell_at` + `mc2/cast.rs:2259 mc2_spell_mana_cost_at` (via
    `mc2_set_spell` → `mc2_price_castle` `mc2/cast.rs:2393`): HUMAN/MC2.
  - `mc2/rivals.rs:2814 mc2_rival_set_spell_at` (via `mc2_rival_set_spell` `:2806`): RIVAL/MC2.
    It has its own inline copy of the ladder × multiplier math.
- **Status:** DRIFTED
- **Differences:**
  - **Pricing castle.**
    - Human: `player_castle_bound()`, the REGISTER (`MGC_NO_MC2_SPELL_PRICE_REGISTER`, round
      136, mc2l12).
    - Rival: `rival_castle(own)`, a POOL SCAN, with an optional fallback (B4).
    - The rival doc comment itself says retail prices "off ITS OWN `CastleEntityIndex_0x3A_58`".
      The register law never reached this arm. They differ in cases (a)/(b)/(c).
  - **Surcharge.** The +3000 re-cast surcharge is HUMAN-only. The comment justifies this: the
    only writer is the human's `PlayerAction 0x2A` demolish.
  - **Cheat arm.** The human arm has the `dev_spells || mc2_free_spells` arm (retail's own cheat
    flag `OptionsSettingFlag_24 & 0x20`, L:1531-35): `f136 = 0`, `f140 = 1`. The rival arm has
    none. Retail's flag test sits in the one shared SetSpell. From the code comment alone I
    cannot tell whether it is owner-gated.
  - **Duplicated math.**
    - Rival: `result.clamp(0, i32::MAX)` on the ladder arm; no-castle returns `sub.mana_cost`.
    - Human: `base.saturating_add(surcharge)`.
    - Same otherwise.
- **What reaches each arm in-game:**
  - HUMAN: every castle level change, cast, tier select or respawn re-price of the player's
    Create Castle token.
  - RIVAL: the same events for any rival's castle token.
  - The drift fires when the owner has two castles (case a) or during the register lag
    (b/c). The cheat drift fires when retail's free-spells cheat is on.
- **Already witnessed?:**
  - HUMAN register: yes (mc2l12 fixture "THE CASTLE-SPELL PRICE IS TAKEN OFF THE REGISTER").
  - RIVAL price: yes, but only the tier multiply (mc2l22 fixture "THE RIVAL CASTLE PRICE IS
    GetSpellManaCost's TIER MULTIPLY"). Rival register-vs-scan: no evidence found.
  - Cheat on a rival token: no evidence found.
- **Recording ask:**
  1. **Cheat.** MC2 with retail's free-spells cheat turned on (the one `mc2_free_spells` replays)
     on a level where rivals own spells. Play 5 min. That shows whether a RIVAL token's
     `maxMana`/`mana` collapse to 0/1 too.
  2. **Rival split.** Get a rival to hold two castles: raze its castle while its replacement
     ball flies. Keep the level running while both stand. 30+ min, luck-dependent.
- **Confidence / notes:** High on the code. The cheat owner-gate needs the decompile to settle.

### CASTLE-B4: Ladder-sync drain — the castle HP/CAP stamp's token re-price (`sub_60810` → `sub_60780`)
- **Retail routine(s):**
  - `sub_60810` / `sub_60780` (NETHERW EF:62092 / 62067, file 0x84fbe-0x84fd6): on every castle
    level change, re-run SetSpell on the owner's (15,2) at its own tier, priced off the OWNER's
    register.
  - Level-0 rival tail: the `sub_605E0` token purge.
- **Port arms:** `engine/world.rs:7687 mc2_drain_ladder_sync`, one function with two inline
  arms:
  - the `if human {…}` arm (`~:7730-7775`): HUMAN/MC2;
  - the `else {…}` arm (`~:7776-7795`): RIVAL/MC2.

  The snapshot writer is `mc2/castle.rs:~1062-1072`, in the downgrade teardown.
- **Status:** DRIFTED
- **Differences:**
  - **Human arm** (`MGC_NO_MC2_LADDER_PRICE_REGISTER`, mc2l12). Price castle =
    - the register snapshot taken at the level-0 teardown (`snap`), or else
    - `mc2_price_castle()`, the register.

    Pre-law, it used the "mailing" castle plus `player_castle()`.
  - **Rival arm.** Price castle = `rival_castle(own)` (POOL SCAN), falling back to the mailing
    castle `c` (`MGC_NO_MC2_RIVAL_LADDER_PRICE_DYING_CASTLE`, mc2l18). It never reads `snap`,
    even though the downgrade writes `snap` for every team.
    - With a split: human = the register's rung; rival = the lowest-numbered castle's rung. When
      an ORPHAN castle changes level, the human re-prices at the register's rung, not the
      orphan's; the rival re-prices at whichever castle the scan finds first.
  - The mc2l18 fixture's title says "prices off the castle REGISTER". The port implements that
    law as scan + dying-castle fallback, which matches the register only while the rival has at
    most one castle.
- **What reaches each arm in-game:**
  - HUMAN: the player's castle levels up, is damaged down a level, or dies.
  - RIVAL: the same for a rival's castle.
  - The drift needs an owner with two castles where the non-registered one changes level.
- **Already witnessed?:**
  - HUMAN: yes (mc2l12 fixture "THE LADDER-SYNC DRAIN PRICES THE HUMAN'S CREATE-CASTLE TOKEN
    OFF THE CASTLE **REGISTER**").
  - RIVAL: yes for the dying-castle case (mc2l18 fixture "A RIVAL LADDER MAIL PRICES OFF THE
    CASTLE REGISTER…" t=6633). Rival split: no evidence found.
- **Recording ask:** MC2. Make a rival own two castles, then damage the OLDER, lower-slot one
  down a level while the newer one stands. Graded on the rival (15,2) `mana_max`/`mana`. Very
  luck-dependent. A cheaper proxy: any level where a rival's first castle is razed and it
  rebuilds while the old one is still dying. About 30+ min.
- **Confidence / notes:** High.

### CASTLE-B5: At-castle probe (regen boost / grace pin / "owner is home")
- **Retail routine(s):**
  - Human: `AddPlayer03_00_5E010`'s at-castle probe (EF:60021),
    `Entities[player->CastleEntityIndex_0x3A_58]` box test.
  - Rival: `sub_12A70`'s at-castle test (EF:5398).
  - Raid's undefended test: `sub_106C0(Entities[castle->id], castle)`.

  These are different retail call sites, but all test the owner against its (registered) castle
  with the summed-extents box.
- **Port arms:**
  - `engine/world.rs:5769 mc2_regen_boost`: HUMAN/MC2, the register
    (`MGC_NO_MC2_CASTLE_REGISTER`).
  - `mc2/rivals.rs:~3437 mc2_rival_alive`'s `at_castle` (`rival_castle` scan + `ent_overlap`):
    RIVAL/MC2.
  - `mc2/rivals.rs:6232 mc2_owner_at_castle` (castle given by the caller): one fn with a HUMAN
    arm (hand-rolled box off `human_pose` + `MC2_PLAYER_HW`/`PLAYER_HH`) and a RIVAL arm
    (`ent_overlap`).
- **Status:** DRIFTED
- **Differences:**
  - **Which castle.** Human probe = REGISTER. Rival probe = POOL SCAN. In a split the human
    tests the registered castle; the rival tests the lowest-numbered one. The mc2l12 t=18304
    human witness (d88 1293 vs 129) is exactly this, in code comments only.
  - **Which pose.**
    - `mc2_regen_boost` uses the PRE-move pose `pre` with constants HW 121 / LIFT 100 / HH 100.
    - `mc2_owner_at_castle`'s human arm uses `human_pose` (the settled pose) with
      `PLAYER_HH` = SPRITE_STATS[44].height/2 on both sides of the z leg, plus a life test.
    - Both are "human at castle" and are written differently. They differ at least in pose
      phase.
- **What reaches each arm in-game:**
  - HUMAN regen: the player docks the carpet on his own castle in MC2 and gets the
    +regen/grace.
  - RIVAL: a rival flies home to heal.
  - `mc2_owner_at_castle` human arm: a rival decides whether to raid the player's castle while
    the player is (or just stopped being) docked there.
- **Already witnessed?:**
  - HUMAN regen register: no fixture found (only the code comment's mc2l12 t=18304).
  - RIVAL at-castle box: yes (mc2l6 fixtures "THE RIVAL'S LIFE REGEN IS A STORED REGISTER…",
    "A DOCKED WIZARD KEEPS A DEAD TARGET").
  - Rival split: no evidence found.
  - Raid-undefended human arm while docked or undocking: no evidence found.
- **Recording ask:**
  1. **Raid while docked.** MC2 with a hostile rival that raids castles (mc2l6/l22 style).
     Repeatedly park on your own castle, then leave just as a rival approaches it to raid. About
     15 min.
  2. **Split.** Build a second castle while the first still stands (the multiple-castles
     window: cast a second ball before the first castle's first level-up), then dock at each in
     turn. About 10 min.
- **Confidence / notes:** Medium on whether the pose-phase difference is a real drift or
  intentional. Different callers want different phases.

### CASTLE-B6: The MC2 castle-register reader census (`CastleEntityIndex_0x3A_58` vs pool scan)
- **Retail routine(s):** every retail reader of `wizext->CastleEntityIndex_0x3A_58`. The port
  lists the confirmed ones as:
  - `GetSpellManaCost_6D710`
  - `PlayerEvents_51BB0` case 0x2A (demolish)
  - `sub_6AD60` (teleport)
  - `sub_389F0` (the (10,43) token)
  - `sub_28860` (m25)
  - `sub_14030` / `sub_133B0` / `sub_13E40` (rival brain)
  - the at-castle probe
  - `sub_69AB0` (create/upgrade)
  - `sub_60780` (re-price)
  - the ball's stale-create guard (EF:58831)
- **Port arms:** readers that use the REGISTER (`player_castle_bound` / `mc2_castle_reg_of` /
  `castle_reg[team]`) vs readers that SCAN (`player_castle` / `rival_castle` / `mc2_castle_of`).

  REGISTER (landed, each with its own `MGC_NO_*` switch):
  - human spell price `mc2/cast.rs:2393 mc2_price_castle`
  - human ladder drain `engine/world.rs:7687` (human arm)
  - human regen `engine/world.rs:5769`
  - human demolish `engine/world.rs:14857 mc2_demolish_castle`
  - human teleport own-hop `engine/world.rs:14877 mc2_teleport_castle`
  - human create/upgrade split `engine/world.rs:15586 cast_castle`
  - m25 brain `mc2/roster.rs:5954`
  - (10,43) token `engine/features.rs:~9177`, owner-generic
  - ball stale-create guard `mc2/proj.rs:~2998`, owner-generic

  SCAN, HUMAN-side:
  - `mc2/cast.rs:3360 mc2_afford` (upkeep castle)
  - `mc2/cast.rs:4196 mc2_castle_lock_release` (gate)
  - `engine/world.rs:12497 mc2_player_respawn` (respawn destination)
  - `engine/world.rs:19278 objective_mc2` (mana objective, `.filter(f26>0)`)
  - `mc2/doomsday.rs:1056/1177` (devour)
  - `mc2/rivals.rs:6549 mc2_rival_pick_wizard` (`player_castle().is_none()` as the human's
    "castle-less" flag)
  - UI: `mc2/cast.rs:2592 mc2_book_view`, `mc2_spell_mana_cost`
  - superseded demolish arm `engine/world.rs:8596`

  SCAN, RIVAL-side: every `rival_castle(..)` call in `mc2/rivals.rs`:
  - `mc2_rival_set_spell_at` :2861
  - `mc2_rival_alive` :3437
  - `mc2_rival_afford` :5181
  - `mc2_rival_selector` :5673
  - `mc2_castle_ladder_cost` :6082
  - `mc2_rival_pick_castle` :6271
  - `mc2_rival_pick_wizard` :6478/:6651
  - `mc2_rival_pick_balloon` :6776
  - `mc2_rival_pick_ball` :7044
  - `mc2_rival_pick_mana` :7279
  - `mc2_rival_state_tick` :7555/:7656
  - `mc2_rival_cast_ready` :8140
  - `mc2_rival_token_fire_at_slot` :8648
  - `mc2_rival_cast_castle` :8862
  - `mc2_rival_castle_mint` :9036
  - `mc2_rival_dead_wait` :10288
  - `mc2_rival_respawn` :10351

  Also on the scan:
  - `mc2/cast.rs:4529 mc2_rival_castle_lock_release`
  - `mc2/cast.rs:2994 mc2_steal_resolve` (both columns)
  - `engine/world.rs:14666/14686 mc2_teleport_cycle` (hops to OTHER wizards' castles)
  - `mc2/roster.rs:5248/5267 m23_owner_scan`
  - `mc2/multipart.rs:1588 m22_target_castle`
- **Status:** DRIFTED, as a class. The HUMAN column moved onto the register over rounds
  136-139; the MC2 RIVAL column is still entirely on the pool scan.
  - Cross-game contrast: MC1's rival column DID move (`mc1/rivals.rs:1819 rival_castle_reg`,
    `MGC_NO_MC1_HOME_CASTLE_REGISTER`), and so did MC2's owner-generic creature and token
    readers.
- **Differences:**
  1. **Human vs rival.** Price, drain, regen and create/upgrade read the register for the
     human and the scan for rivals (B2-B5).
  2. **Inside one fn.** `mc2_teleport_cycle` hops to the human's castle via the register
     (`mc2_teleport_castle`) and to rival castles via the scan (`rival_castle`).
  3. **MC2 rival PLANT writes no register.** Retail's castle-less case-2 arm writes
    `CastleEntityIndex_0x3A_58` at the plant (the port's own comments quote EF:6836-38 at
    `mc2/rivals.rs:8973` and `engine/features.rs:~9150`). `mc2_rival_cast_castle` sets only
    `id24`. `castle_reg` is written only by the level-up commit (`mc2/castle.rs:867`).
     - By contrast, MC1's rival plant does write it (`mc1/rivals.rs:1794`).
     - So between the plant and its first level-up commit, the register-reading owner-generic
       readers see the rival as castle-less: the (10,43) token, the m25 brain and the ball
       stale-create guard.
     - This window may be ≤1 tick if the level-up commit runs on the birth's first dispatch
       (see the mc2l3 "same-frame upgrade" note). Unverified.
  4. **Human-side readers still on the scan.** Retail's register reading is not cited for
     `mc2_afford`, `mc2_player_respawn`, `objective_mc2` or devour. These are unaudited, not
     proven twins. `mc2_player_respawn`'s destination is notable: a split castle would make the
     scan respawn you at the lowest-numbered castle.
- **What reaches each arm in-game:** any MC2 situation where an owner (human or rival) has
  - (a) two live castles,
  - (b) a castle freshly planted and not yet levelled, or
  - (c) a castle on its dying tick.

  For (3): a rival's FIRST castle planting (castle-less rival builds its first castle).
- **Already witnessed?:**
  - Human register readers: yes (mc2l12 fixtures: spell price, demolish, (10,43) token, m25,
    ladder drain; all from the mc2l12 castle split).
  - Rival register readers: no evidence found. The corpus split is human-owned.
  - Rival plant: yes for the free, unresearched plant (mc2l12 fixture "THE RIVAL'S FIRST CASTLE
    IS FREE…"; mc2l6 "THE RIVAL'S CASTLE-LESS MINT IS THE SHARED (3,2) CTOR"). The register
    write on the plant tick: no evidence found.
- **Recording ask:**
  1. **Rival castle split.** MC2 on a level with 2-3 castle-building rivals. Repeatedly raze a
     rival castle to level 0 while its rebuild ball is in flight, or let it rebuild while the old
     one burns, so a rival holds two castles for a while. Then attack or teleport to the rival's
     castles and watch where it heals and respawns. 30-60 min.
  2. **Human split.** Cast a second castle ball before the first castle's first level-up. This
     is retail's multiple-castles bug; the port has a patch `one_castle_per_wizard`, so record
     with retail. Then die, respawn and fulfil the mana objective. That witnesses the
     human-side readers still on the scan (respawn destination, objective, upkeep). About
     15 min.
  3. **Fresh rival plant.** Any MC2 level where a castle-less rival plants its first castle
     (mc2l6/mc2l12 already contain this). Just make sure a take covers the plant tick with a
     (10,43)- or m25-relevant reader nearby. Probably already in the corpus; needs a dig, not a
     recording.
- **Confidence / notes:** High that the code state is as listed. Medium on whether every
  human-side scan reader really corresponds to a register read in retail.

### CASTLE-B7: Rival castle-spell affordability readers (within the rival column, plus the human gate)
- **Retail routine(s):**
  - `sub_15170` / `sub_15730` / `sub_13CE0` (AI gates) read the TOKEN's stamped `maxMana_0x8C`
    (tier multiply included).
  - `sub_5F660`'s model-2 arm (shared player/AI cast router, EF:60907-13) refuses on a live
    lock and on `mana < token.maxMana`.
- **Port arms:**
  - `mc2/rivals.rs:6068 mc2_rival_afford_castle` and `:5810 mc2_rival_ball_arm_open`:
    RIVAL/MC2, token price (`MGC_NO_RIVAL_CASTLE_TOKEN_PRICE`).
  - `mc2/rivals.rs:8140 mc2_rival_cast_ready` case 2: RIVAL/MC2, token price (`e.max_life`).
  - `mc2/rivals.rs:8851 mc2_rival_cast_castle` entry: RIVAL/MC2, `mc2_castle_ladder_cost` =
    the bare rung, no tier multiply, rung castle by scan.
  - `mc2/cast.rs:3232 mc2_cast_gate` model-2 arm: HUMAN/MC2.
- **Status:** DRIFTED, within the rival column.
- **Differences:**
  - **Rival entry gate.** `mc2_rival_cast_castle` opens with
    `mana < mc2_castle_ladder_cost(ri)`: the pre-round-104 recompute that the token-price law
    replaced in the other three rival readers.
    - Upgrade leg: its later `mana < token.max_life` check makes it redundant, because the token
      price is ≥ the bare rung.
    - Castle-less leg: the bare rung is `MC2_CASTLE_COST[0] = 1000`, where retail's
      `sub_15170` compares against the token's raw `manaCost_6`. `cast_ready` already checks the
      token price first, so this gate only bites if `manaCost_6` < 1000 for the selected tier.
      Not verified.
  - **Human vs rival router.** The human gate buzzes sound 29 on the live-lock refusal; the
    rival re-implementation of the same `sub_5F660` model-2 arm plays no sound.
    - Retail's LABEL_23 buzz for an AI caster is unverified. The rival also runs the
      `mc2_castle_space_ok` probe before arming; that probe is AI-brain logic (`sub_14E10`,
      `sub_11A10`), so it is a different routine and not drift.
- **What reaches each arm in-game:** a castle-less MC2 rival with the castle spell at tier 1 or
  2 deciding to plant; any rival upgrading.
- **Already witnessed?:**
  - Token-price gates: yes (mc2l22 fixture "THE CASTLE SPELL'S READINESS GATE SPENDS THE
    TOKEN'S STAMPED PRICE").
  - Castle-less higher-tier plant through the entry ladder gate: no evidence found.
- **Recording ask:** probably none. It only bites if a tier's raw `manaCost_6` is below 1000. A
  dig against `SPELLS` row 2 settles that first. If it does bite: an MC2 level where a rival
  that already knows Create Castle at tier ≥ 1 loses its castle and re-plants with between
  `manaCost_6` and 1000 mana.
- **Confidence / notes:** Low impact, high confidence on the code.

---

## Not clusters (checked, already one shared body)
- `mc2/proj.rs:2825 mc2_castle_ball_tick`: ONE owner-generic body for human and rival balls.
  The lock mail, the stale-create register guard and the upgrade arrival all key on `id24`.
- `engine/world.rs:7619 mc2_drain_castle_lock_mail` / `:7669 mc2_castle_ball_owner_tier_drain`:
  one body. The owner fork only picks the field home (`player.fall_speed` vs rival `f46`); the
  logic is the same. The owner-drain seat is unreachable in the corpus (pool never fills).
- `mc2/cast.rs:4253 mc2_castle_lock_stamp` PIN arm and `mc2_castle_death_token_purge`: shared.
  The purge is rival-only by retail's `model == 1` test, not by port choice.
- `mc2/castle.rs:807 mc2_castle_upgrade` / `:878 mc2_castle_downgrade`: owner-generic. The
  human-only `mc2_cast_xp` push is retail's model-0 XP guard (`mc2_award_xp`).
- `engine/features.rs:~9140` (10,43) token delivery: owner-generic on MC2 (register). Its MC1
  leg is an MC1-vs-MC2 fork (below).

## Clusters seen that belong to another subsystem / part
- **MC1 vs MC2 castle upgrade lock (part A / MC1):**
  - MC1 `engine/world.rs:14758 castle_lock_active` (a live re-derive: `castle_build_lives` pool
    scan + `player_castle().tick70 != 4`, used at `:13886` for spell 16) plus the MC1 castle-pass
    stamp (`engine/world.rs:~9588-9660`, `castle_owner_token`, `MGC_NO_MC1_LEVELER_PIN_GATE`).
  - MC2 `mc2_castle_spell_tick` latch.
  - MC2 retired the live re-derive (round 135, now only behind `MGC_NO_MC2_CASTLE_CAST_LATCH`);
    MC1's cast-gate still uses one.
- **MC1 (10,43) token castle lookup (part A):** `engine/features.rs:~9185`, MC1 leg. A pool scan
  with `f26 > 0`, although MC1 now has `castle_reg` and the human MC1 readers moved to
  `player_castle_bound`.
- **CAST / manifestation subsystem:**
  - `mc2/cast.rs:3360 mc2_afford` vs `mc2/rivals.rs:5173 mc2_rival_afford` (`sub_68D50`).
    Liveness is `player.state == Alive` vs `act_life < 0`; the upkeep castle is scanned in both.
  - `mc2_cast_expire` (human) vs the rival's inline `f44` drains: generic `sub_6D880`, every
    spell.
- **CAST / spell subsystem:** `mc2/cast.rs:2994 mc2_steal_resolve` uses `rival_castle` (scan)
  for both victim and caster castles. Steal Mana tier 3 is a register candidate.
- **TELEPORT:** `engine/world.rs:14666/14686 mc2_teleport_cycle`. The own hop reads the
  register; hops to other wizards' castles read the scan (listed in B6).

---

# CASTLE — part C: destruction / damage / turrets / payout

Paths are relative to `crates/mgc-sim/src/`. Line numbers are from the tree as it stood on 2026-09-24.

### CASTLE-C1: Castle level-off / destroy (action 6), MC1 vs MC2
- **Retail routine(s):** MC1 `sub_470E0` (state-6 wrapper) → `sub_47A70_47DB0` (teardown), `sub_47130` (ejector), `sub_47400` (fleet). MC2 `sub_5FCA0` → `sub_605E0` (level off), `sub_5FD00` (ejector), `sub_5FF50` (roster). What it does: a lethally hit castle loses one level (the 10% capacity haircut spill, terrain un-stamp, ladder reset). At level 0 it dies, the whole bank scatters and the fleet dissolves.
- **Port arms:**
  - `engine/features.rs:10601 castle_downgrade` (+ `castle_tick` state 6 at :10146, dispatch `engine/world.rs:~9590`): MC1/HW
  - `mc2/castle.rs:711 mc2_castle_destroy` / `:725 mc2_castle_destroy_head` / `:732 mc2_castle_destroy_tail` / `:878 mc2_castle_downgrade` (dispatch `engine/world.rs:9489-9582`): MC2
- **Status:** DRIFTED. This is an MC1-vs-MC2 twin; the two binaries have the same routine shape.
- **Differences:**
  - **Where the book work runs relative to the eject.** MC2 splits the destroy into head and tail. The world-side `sub_605E0` book work (the re-price, the token pin, the level-0 rival token purge) runs BETWEEN the downgrade and the eject/roster tail (`MGC_NO_MC2_CASTLE_PURGE_BEFORE_EJECT`, `MGC_NO_MC2_CASTLE_PURGE_INLINE`). MC1 runs the whole `castle_tick` (downgrade, second eject, balloons) first, then stamps the token pin and re-price after it. This looks inert for MC1: its ejector has no GC and reads no token. It is a structural difference, not a proven drift.
  - **Haircut overflow.** MC2 has a patch arm (`patches.mc2_downgrade_overflow`, i64) next to a retail `wrapping_mul` arm. MC1 computes a plain `10 * f136 / 100` in i32, with no patch and no explicit wrap.
  - **Scratch slot 0.** Both write the un-stamp operand into pool slot 0 and leave it there as residue. MC2 does it behind `MGC_NO_CASTLE_SCRATCH_ARG` (x/y/site_z, `f71` = level, model 0, f26 0). MC1 always does it, sets `class64 = 10` for the duration, runs `tick_building_collapse(SCRATCH)` and resets class to 0. MC2 calls `mc2_castle_unstamp` and never touches the class byte.
  - **OOB probe.** MC1 only: `mc1_oob_castle_inert("teardown")` for level ≥ 8 (the level-250 castle).
  - **Repaint timer.** MC1 sets `f50 = 5` / `f59 = 0` only if the new level > 0. MC2's `mc2_castle_destroy_tail` writes `f59 = 0; f50 = 5` unconditionally, on a dead castle too (inert, because the castle is reaped).
  - **Level-0 token purge.** MC2 only: `mc2_castle_death_token_purge`, gated to a rival owner and to the level-gfx bit 4, `MGC_NO_MC2_CASTLE_DEATH_TOKEN_PURGE`. MC1 has no purge arm.
  - **HP ladder.** MC1 inlines `CASTLE_HP`/`CASTLE_CAP` with the overkill carry. MC2 calls `mc2_castle_ladder` (owner Life × research factor). This is a per-game difference and not in scope here.
  - **Shared, same in both:** no fleet conversion inside the downgrade (the MC2 comment says "MC1 ALREADY CARRIES THIS LAW"), register zeroed at level 0, `flags |= 0x400`, and the pool guard on action 6 (MC1 `MGC_NO_MC1_LEVELER_POOL_GATE`; MC2 `free.is_empty()` → `tick70 = 4`).
- **What reaches each arm in-game:** any castle (human or rival) takes a lethal hit or a Shift+L demolish. MC1 arm: MC1/HW castles. MC2 arm: MC2 castles. The overflow arm needs a maxed level-7 MC2 castle holding more than about 214M mana. The purge arm needs a RIVAL castle dying at level 1→0 on MC2 level 22 or 62.
- **Already witnessed?:**
  - MC1: yes. Code cites mc1l0 t=2217/t=1363 (castle death scatter) and mc1l49 t=17810 (starved pool). Fixture mc1l45 "a-castle-downgrade-reprices-an-unbound-owners-token".
  - MC2: yes. mc2l3 "the-balloon-sphere-is-the-last-pop-of-a-castle-d…", mc2l22 "a-dying-castle-purges-its-tokens-before-the-mana-spill", mc2l3 "the-death-downgrade-re-prices-at-the-level-0-run…".
  - Overflow arm: no evidence found.
- **Recording ask:** none needed for the main arms. Optional: MC2, a castle at level 7 overfilled well past its cap (Mana Magnet / Steal Mana farming) taking a lethal hit, which shows the retail i32 wrap (a negative cut). It needs hours of mana farming; low value.
- **Confidence / notes:** high on the listed diffs, which I read from both bodies. The claim that the MC1 book-work order is inert rests on MC1 having no ejector GC and no purge.

### CASTLE-C2: Action-6 Create-Castle token PIN / RELEASE at the castle's own pass, MC1 vs MC2
- **Retail routine(s):** MC1 `sub_46D20_47060` via `sub_47A70_47DB0` (:56529 pin / :56533 release). MC2 `sub_5F890` via `sub_605E0` (EF:61643 pin; EF:61645-63 level-0 if/else on the OWNER's model). What it does: while a castle is taking a level off, the owner's Create-Castle token is locked (pinned); at death it is released.
- **Port arms:**
  - `engine/world.rs:~9540-9573` (the `3 if Mc2 && model65 == 2` arm, `pin = match pre70`): MC2
  - `engine/world.rs:~9633-9680` (the `3 if model65 == 2` arm, `stamp = match pre70`): MC1/HW
- **Status:** DRIFTED, and possibly a genuine difference between the binaries.
- **Differences:**
  - **Level-0 death by owner type.** MC2 releases the pin only for a NON-rival owner. The `owner_m1` test is `own != PLAYER_TARGET && ent[own].model65 == 1`, with no class test. A RIVAL-owned castle dying at level 0 keeps its pin for good. MC1 releases (`Some(0)`) whenever the castle took `flags & 0x400`, whatever the owner.
  - **Pin condition.** MC2 pins only `if pre26 > 0`. MC1 pins on any non-dead action-6 pass where the leveler ran.
  - **Pool gate.** MC2's `downgraded = !free.is_empty()` has no kill switch. MC1's `leveler_ran` has `MGC_NO_MC1_LEVELER_POOL_GATE` / `MGC_NO_MC1_LEVELER_PIN_GATE`.
  - **Token resolution.** MC2 goes through the spellbook `mc2_castle_lock_stamp(own, pin)`. MC1 goes through `castle_owner_token(own)` (wizard +708) and writes `f26 = SPELLS[16].count - 1` directly.
  - **Owner-type predicate is spelled two ways inside MC2.** The pin site uses `model65 == 1` alone. `mc2/cast.rs:4343 mc2_castle_death_token_purge` uses `class64 == 3 && model65 == 1`. The two disagree only if the owner slot has been re-minted as a non-class-3 model-1 record.
- **What reaches each arm in-game:**
  - The human's castle knocked from level 1 to 0 (both games): the release fires in both.
  - A RIVAL's castle knocked from level 1 to 0: in MC2 the rival's castle token stays locked (pinned at `word_0x30_48 - 1`) for thousands of ticks. In MC1 the port releases it.
- **Already witnessed?:**
  - MC2 rival arm: yes. Code cites mc2l6-rsg t=4310 and mc2l8 t=11584. Fixture mc2l6 "the-castle-releases-the-upgrade-lock-at-its-own…".
  - MC2 human arm: yes (mc2l3 demolish t=15904 series).
  - MC1 human arm: yes (mc1l0 "the-castle-lockout-is-a-latch…").
  - MC1 RIVAL castle 1→0: partial. mc1l48 "the-dead-owners-castle-token-freezes-and-releases" is about a DEAD owner. I found no fixture for a LIVE rival whose castle dies.
- **Recording ask:** MC1 (any early level with an active rival, e.g. level 3-5). Take a rival's castle down to level 1, then kill it outright while the rival is still ALIVE. Then keep recording about 200 ticks while the rival tries to rebuild. That shows whether MC1 retail releases the rival's token lock at level 0, as the port does, or keeps it pinned as MC2 does. About 5-10 minutes.
- **Confidence / notes:** the MC1 comment claims the retail :56533 release is unconditional. I did not re-read the MC1 listing. If MC1's `sub_47A70` has the same model test, the MC1 arm is missing it.

### CASTLE-C3: Castle-death ladder re-price, HUMAN vs RIVAL owner (MC2 `sub_605E0` → `sub_60810` → `sub_60780` → `SetSpell_6D5E0`)
- **Retail routine(s):** `sub_60780` (NETHERW.EXE 0x84fbe…) calls `SetSpell_6D5E0` → `GetSpellManaCost_6D710`, which reads `owner->CastleEntityIndex_0x3A_58` (a REGISTER). It is one routine for any owner. What it does: when a castle takes or loses a level, the owner's Create-Castle token price is re-stamped at the rung the owner's REGISTERED castle stands on.
- **Port arms:**
  - `engine/world.rs:7687 mc2_drain_ladder_sync`, `if human` branch (≈7717-7758): HUMAN/MC2, priced via the register (`mc2_price_castle` / the snapshot `snap`)
  - same fn, `else` branch (≈7759-7780) → `mc2_rival_set_spell_at(m, tier, own, fb)`: RIVAL/MC2, priced via the `rival_castle` pool scan plus a mailing-castle fallback
- **Status:** DRIFTED. The switch doc says so: "THIS IS A FALLBACK, NOT THE REGISTER" (`engine/features.rs:3800-3812`).
- **Differences:**
  - **Human** (`MGC_NO_MC2_LADDER_PRICE_REGISTER`): the register's castle (`player_castle_bound`), or the death-arm snapshot of the register taken just before `mc2_castle_downgrade` zeroes it. With a castle SPLIT standing, an orphan's level change re-prices at the REGISTERED castle's rung.
  - **Rival** (`MGC_NO_MC2_RIVAL_LADDER_PRICE_DYING_CASTLE`): `rival_castle`, a reap-filtered POOL SCAN that returns the lowest-numbered live owned (3,2). It falls back to the mailing castle only when the scan misses. With a rival castle split, the rival arm prices at the lowest-numbered castle's rung, not the registered one.
  - The snapshot mechanism (`mc2_ladder_sync.1`) exists for the human only; the rival arm ignores `snap`.
- **What reaches each arm in-game:** HUMAN — the player's castle levels up, is damaged down a level, or dies. RIVAL — the same for a rival's castle. **The arms diverge only when a wizard owns TWO castles** (the `one_castle_per_wizard` window: a second castle ball landing inside the one-tick register lag) **or when the register names a castle other than the lowest-numbered live one.**
- **Already witnessed?:**
  - HUMAN: yes. mc2l12 (castle 678/293/877 split, per the code comment) and mc2l3 "the-death-downgrade-re-prices-at-the-level-0-run…".
  - RIVAL (simple death, fallback): switch doc cites mc2l18 pair 6633→6634; no fixture note matched.
  - RIVAL with a split: no evidence found.
- **Recording ask:** MC2. It needs a RIVAL with two castles, then the non-registered one (or the registered one) losing a level. You can't directly make a rival cast twice. The practical ask is a long MC2 take on a rival-heavy level with lots of castle-vs-castle fighting, hoping a rival double-castles. Low odds, so mark it opportunistic. The price itself is visible only in the rival's token; the (15,2) mana lanes are graded in pairs.
- **Confidence / notes:** high. Doc and code agree. The faithful fix (a `castle_reg` register read for the rival) touches about 20 rival-AI gates, so the RIVAL AI subsystem should own it.

### CASTLE-C4: Demolish (Shift+L), MC1 vs MC2, and demolish vs damage death
- **Retail routine(s):** MC1 the key word `dw_0 == 48` in the wizard pass (:55837-50), a bare `pool[wizext+50].actLife = -1`. MC2 `PlayerEvents_51BB0` case 0x2A (EF:37991-97; NETHERW.EXE 0x77442-0x7746f): register read ×3, `if level == 1` the +3000 surcharge latch `byte_0x1BE_446`, then `life = -1`. Two different routines in two binaries. What it does: the human razes his own castle by one level.
- **Port arms:**
  - `engine/world.rs:5991` (inside the MC1 wizard pass, post-walk): MC1/HW HUMAN
  - `engine/world.rs:7981-7998` (head of `tick_inner`, before the class-3 roster rebuild): MC2 HUMAN, live
  - `engine/world.rs:8595-8611`: MC2 superseded phase, live only under `MGC_NO_MC2_DEMOLISH_PHASE`, and it resolves via the `player_castle()` pool scan
  - downstream of all of them: the ordinary lethal path (`castle_settled_tick` :10449, and `mc2_castle_intake` :743's `act_life < 0 → 2`)
- **Status:** STRUCTURALLY-DIFFERENT (two retail routines). Downstream it is IDENTICAL with damage death, apart from the missing killer write (by design in both).
- **Differences:**
  - **Phase.** MC1: post-walk, in the wizard pass (it replaces the cast tail). MC2: input pre-phase, ahead of the tick-top class-3 roster rebuild, so a demolished castle is off the raid roster that same tick (`MGC_NO_MC2_DEMOLISH_PHASE` restores the old phase and the old `mc2_rival_pick_castle` live-life guard).
  - **Castle resolution.** Both now use the register (`player_castle_bound`). MC1 has `MGC_NO_CASTLE_BIND_REGISTER` (off arm: `player_castle()` filtered to `f26 > 0`). MC2 has `MGC_NO_MC2_DEMOLISH_REGISTER` (off arm: `player_castle()`).
  - **Surcharge latch.** MC2 only: a level-1 demolish latches `mc2_recast_surcharge`. MC1 has none.
  - **Demolish vs damage death.** Neither game writes the killer (`f38`) on a demolish (no mail); a damage kill writes `f38 = src`. In MC2 the damage kill also stays on the tick-top raid roster until the next tick, while the demolish comes off it immediately (the phase law above).
- **What reaches each arm in-game:** the human presses Shift+L with a bound castle (MC1/HW or MC2). Rivals never demolish.
- **Already witnessed?:**
  - MC2: yes. mc2l3 "the-demolish-witness-has-no-action-clause…", mc2l5 "the-demolish-witness-is-retails-own-surcharge-latch", mc2l12 "demolish-razes-the-registers-castle".
  - MC1: partial. The code comment cites mc1l0 t=2310 (a self-destructing castle spawns balloon 484). I found no MC1 fixture note mentioning demolish.
- **Recording ask:** MC1 (level 1 is fine). Build a castle, upgrade it to level 2+, press Shift+L once (level 2→1), then press it again with the castle at level 1 (the castle dies). If possible also press it (a) while the castle is still building or repainting (tick70 5), and (b) with a second castle standing (the double-cast window), which tests the register over the pool scan. About 5 minutes.
- **Confidence / notes:** high.

### CASTLE-C5: Castle damage intake, MC1 `sub_47EC0` vs MC2 `sub_609E0`
- **Retail routine(s):** MC1 `sub_47EC0` (:56678-711), called from `sub_46DB0`. MC2 `sub_609E0` (EF:61733-58, file 0x851E0). What it does: channel-0 damage mail is subtracted from castle HP; lethal → action 6; the ch5 mail from the owner's (10,43) token arms the upgrade.
- **Port arms:**
  - `engine/features.rs:10449 castle_settled_tick` (inline intake block ≈10483-10530): MC1/HW, all owners
  - `mc2/castle.rs:743 mc2_castle_intake` (called from `mc2_castle_standing` :533): MC2, all owners
- **Status:** DRIFTED, but only on an arm that is probably unreachable.
- **Differences:**
  - **ch5 upgrade channel.** MC1 clears the ch5 SOURCE word whenever it is non-zero, and only an owner-sent request arms `0x40`. MC2 clears the source ONLY inside the id match, so a non-owner value "sticks forever" (the comment calls it a faithful quirk). Both leave the ch5 amount as residue.
  - **Killer home.** Both write `f38` on the lethal arm. MC2 has the kill switch `MGC_NO_MC2_CASTLE_KILLER_HOME` (old home `f36`). MC1 has none.
  - **HUD alert.** Human vs rival: both bodies are owner-generic apart from the HUD latch (`castle_alert = 4` only when `id24 == PLAYER_TARGET`, a deliberate single-HUD model in both).
  - **Early out.** MC1 checks `act_life < 0` inline before reading mail; MC2 returns 2 at the top. Same logic.
- **What reaches each arm in-game:** any castle hit by anything (fireball, creature, rival bolt), in either game.
- **Already witnessed?:**
  - MC2: yes. The killer-home census cites mc2l9 slot 339 and mc2l8 slot 163, and fixtures carry the (3,2) killer lane.
  - MC1: yes. Code cites mc1l0 t=1188 (ch5 residue).
  - Non-owner ch5 sender: no evidence found. It is probably never authored.
- **Recording ask:** none. The only difference needs a non-owner to deliver an upgrade token, and I know of no in-game way to do that.
- **Confidence / notes:** medium on reachability.

### CASTLE-C6: Killer latch and "under attack" alert — castle intake vs balloon intake (both games)
- **Retail routine(s):** MC2 `sub_609E0` (castle) and `sub_60EA0` (balloon tail, EF:61939-47). MC1 `sub_47EC0` (castle) and `sub_481D0_48510` (balloon, :56814-36). What it does: a lethal hit stamps the killer into `@0x24`/`+38`. A hit on an owned castle or balloon flashes the owner's HUD panel.
- **Port arms:**
  - `mc2/castle.rs:743 mc2_castle_intake`: MC2 castle
  - `mc2/castle.rs:~2303-2321` (tail of `mc2_balloon_tick`): MC2 balloon
  - `engine/features.rs:10449 castle_settled_tick`: MC1 castle
  - `engine/features.rs:9975 balloon_tick`: MC1 balloon
- **Status:** IDENTICAL on the killer latch. DRIFTED on the HUD alert.
- **Differences:**
  - **Killer.** Both MC2 homes consult the same switch `MGC_NO_MC2_CASTLE_KILLER_HOME`, and both MC1 homes write `f38`. Consistent.
  - **Balloon alert.** MC1's balloon intake does NOT raise `balloon_alert`. Retail's write resolves through the balloon's own `+160`, which points at the allocator's dummy sink, so it is dead in retail (`MGC_NO_MC1_BALLOON_ALERT_SINK`; the `balloon_alert` lane is graded in the MC1 importer, conformance.rs:1525). MC2's balloon intake sets `balloon_alert = 4` unconditionally for a human-owned balloon ("Retail sets byte_0x197_407 for ANY owner", EF:61947). The MC2 twin has not had the sink question asked of it.
  - **Castle alert.** Both castle intakes set `castle_alert = 4` for the human's castle, with no sink law in either game. The MC1 balloon finding (a class-3 non-wizard's `+160` points at the sink) would apply to MC1's castle too unless its ctor re-points `+160`. I found no audit of that.
- **What reaches each arm in-game:** a rival or creature hits the human's balloon (or castle) without killing it.
- **Already witnessed?:**
  - MC1 balloon sink: yes, round 153, 8,651 pair rows over 36 takes (the switch doc). Fixture mc1l5 "the-balloon-has-no-self-kill…".
  - MC2 balloon alert: no evidence found.
  - MC1/MC2 castle alert: no evidence found.
- **Recording ask:** MC2 (any level where rivals raid, e.g. mc2 level 3-6). Let a rival or creature damage the human's balloon without killing it, and note whether the retail HUD balloon panel flashes. Do the same for the human's castle in MC1 and MC2. The HUD flash is visible on screen, and it is also a lane in the recorder's player block (`byte_0x197_407` / `+393`, `+391`). About 5 minutes per game.
- **Confidence / notes:** medium. Whether the MC2 `dword_0xA4_164x` on a balloon or castle points at a sink is unverified.

### CASTLE-C7: Castle mana ejector, MC1 `sub_47130` vs MC2 `sub_5FD00`
- **Retail routine(s):** MC1 `sub_47130` (:56162-256, hw:52258-320). MC2 `sub_5FD00` (EF:61260-320). What it does: mana stored above capacity, or the whole bank at level 0, is thrown out as 1..32 owner-tagged balls.
- **Port arms:**
  - `engine/features.rs:10741 castle_eject`: MC1/HW
  - `mc2/castle.rs:1408 mc2_castle_eject`: MC2
- **Status:** DRIFTED. MC1-vs-MC2; each dry-pool law is witnessed in its own game.
- **Differences:**
  - **Zero headroom.** MC1 returns with no draw and disarms the recycle stack (`mc2_recycle.stack.clear()`). MC2 runs the `sub_49F90` GC (`mc2_eject_gc`) and re-denominates the burst to 8 (`MGC_NO_CASTLE_EJECT_GC`).
  - **Failed ball allocation.** MC1 `continue` (skip the ball, keep looping). MC2 `break`.
  - **Magnets.** MC1 spawns four (10,54) mana magnets at 25 tiles after the balls, with one castle-LCG yaw draw per successful allocation. MC2 spawns none.
  - **Bank / tally source.** MC1 `owner_houses(owner)` (the per-team house tally register; `MGC_NO_MC1_OWNER_HOUSE_TALLY`). MC2 `mc2_owner_bank(own)` (a live pool scan of owned (10,45)).
  - **Owner tag on the ball.** MC2 re-reads `id24` live per ball (`MGC_NO_MC2_EJECT_OWNER_LIVE_READ`). MC1 uses the value cached at entry.
  - **RNG.** MC1 draws the castle's stream with `lcg32(&mut ent[i].rand)`. MC2 uses `ent_rand(i)`. Per-game stream widths; not verified whether they are equivalent.
- **What reaches each arm in-game:** a castle over capacity (houses plus stored greater than the cap, e.g. after a downgrade) on an even `f63` tick, and every castle death. The dry-pool arm needs the pool exhausted (about 999 live records: an eruption, mass lightning).
- **Already witnessed?:**
  - MC1 dry arm: yes. mc1hwl0 "the-castle-ejector-is-gated-on-pool-headroom".
  - MC2 dry arm: yes. mc2l22 "the-dry-castle-ejector-runs-a-gc-pass-and-re-den…".
  - Normal arms: yes in both (mc1l0, mc2l0 "the-ejected-mana-sphere-is-never-re-resized").
- **Recording ask:** none. Both games' behaviour is witnessed, and the binaries appear genuinely different here.
- **Confidence / notes:** high.

### CASTLE-C8: Balloon fleet dispatcher (roster), MC1 `sub_47400` vs MC2 `sub_5FF50`
- **Retail routine(s):** MC1 `sub_47400` (:56329-411). MC2 `sub_5FF50` (EF:61377-445). What it does: walk the owner's 3-seat balloon register, spawn into empty seats, reap dead balloons (cargo drop), retarget on the stagger turn, and cull over quota.
- **Port arms:**
  - `engine/features.rs:9637 castle_balloons`: MC1/HW (it also has a vestigial `is_mc2` branch that empties the register; MC2 never calls it)
  - `mc2/castle.rs:1624 mc2_castle_roster`: MC2
- **Status:** DRIFTED.
- **Differences:**
  - **Seat validation.** MC1: retail's register is CLASS-BLIND and life-only (CARPET.EXE VA 0x4752E/0x47536). The port pre-clears a seat by class/model/owner/`0x400` ONLY under `MGC_NO_MC1_BALLOON_SEAT_LIFE_ONLY`. By default a seat whose balloon was sacrificed and re-minted as another class KEEPS its seat while the new record's life is ≥ 0, and it is then cleared without a spawn (the replacement comes a pass late). MC2 always pre-clears a seat whose record is not a live own (3,3), then refills it in the same pass: this is the pre-dig MC1 behaviour. I found no MC2 binary citation saying `sub_5FF50` tests class.
  - **Full test.** MC1 uses `owner_houses` (tally register) plus stored. MC2 uses `mc2_owner_bank` (pool scan) plus stored.
  - **Dead-seat arm.** MC1 `corpse_drop` + soft-kill. MC2 `mc2_balloon_to_sphere`.
  - **Register kill switch.** `MGC_NO_BALLOON_REG` is consulted by both.
- **What reaches each arm in-game:** every castle with a fleet (level ≥ 1), on even ticks. **The seat difference fires only when the pool runs dry and the allocator SACRIFICES a seated balloon** (the recycle victim list) and re-mints it as something else in the same tick.
- **Already witnessed?:**
  - MC1 class-blind seat: yes. mc1l20 "the-fleet-register-is-life-only-and-class-blind" (t=18192).
  - MC2 register walk in general: yes. mc2l0 "the-mc2-balloon-fleet-walks-a-register-not-a-cen…".
  - MC2 with a sacrificed seat: no evidence found.
- **Recording ask:** MC2, a level where the human (or a rival) has a castle at level 1+ with balloons out. Drive the pool toward exhaustion: long Lightning / Meteor / Fire Storm spam while summoned creatures fight, near the castle. The recycle stack has to take a balloon, so it helps to have many balloons in flight. Record 10-20 minutes. The signal is a balloon slot re-minted as a projectile while its castle's next even-tick roster pass either keeps (retail MC1 behaviour) or refills the seat. ⚠ Whether an MC2 balloon is ever on the victim list (`byte[2] & 2`) is unverified. If it never is, the MC2 arm is unreachable and this collapses to IDENTICAL in practice.
- **Confidence / notes:** medium. It depends on whether MC2 balloons are sacrifice-eligible.

### CASTLE-C9: Castle turret (10,79) human-raise aim, turret vs the other `sub_65580`/`sub_655A0` homes
- **Retail routine(s):** MC2 `sub_655C0` (EF:63076-81, file 0x89DC0) brackets the angle calls in `sub_65580`/`sub_655A0` (`z += array_0x52_82.yaw` unless model 2). MC1 twin `sub_524C0`/`sub_524E0`. What it does: a shot aims at the target's z-box centre. The out-of-pool human needs a hand-added `PLAYER_HH` = 100.
- **Port arms (all inline):**
  - `mc2/castle.rs:3406 mc2_piece_fire` (human `+PLAYER_HH`, pool `Ent::aim_z`): TURRET/MC2
  - `mc2/proj.rs:~4161 mc2_aim_scan` human arm: SCORER (`sub_68490`)/MC2
  - `mc2/proj.rs:958, 3196, 3860, 4430`: other MC2 projectile homes
  - `mc2/roster.rs:998`, `mc2/mobs.rs:2851`, `mc2/cast.rs:2133` (fools aim), `mc2/rivals.rs:6243`
  - `mc1/combat.rs:2252, 2470, …`: MC1
  - `engine/features.rs:880 Ent::aim_z`: pool-side, already shared
- **Status:** IDENTICAL (after round 139). Every human arm adds `PLAYER_HH`; every pool arm uses `aim_z`. Each home has its own switch: `MGC_NO_MC2_PIECE_HUMAN_RAISE`, `MGC_NO_MC2_AIM_HUMAN_RAISE`, `no_carrier_aim_absence`, `no_fools_aim_phase`.
- **Differences:**
  - None in the landed law.
  - **Turret-only ordering.** The turret aims from the piece's own z and only THEN lifts the muzzle by `f78` (EF:30296). That is turret-specific, not a twin.
  - **Scorer.** `sub_685D0` (the castle-scoring variant of the scorer) is already one port body with `sub_68490` (`mc2_aim_score`).
- **What reaches each arm in-game:** an MC2 castle turret (a castle at level ≥ 2 with researched defender parts) shooting at the human or at a creature or rival.
- **Already witnessed?:** yes. Turret: mc2l16 and mc2l12 "castle-turret-aims-the-human-at-his-box-centre", mc2l3 "the-turret-aims-before-the-muzzle-lift…". Scorer: mc2l24 "the-auto-aim-scorer-lifts-the-human-before-the-pitch-cone".
- **Recording ask:** none.
- **Confidence / notes:** the collapse target is obvious: one `target_aim_pos(target)` helper that returns `pz + PLAYER_HH` for the human and `aim_z` otherwise. There are about 15 inline copies.

### CASTLE-C10: Turret ring scan (`sub_3AF00` case 3) vs the other class-5 roster predicates — the own-parent ALLY clause and the human's chain cell
- **Retail routine(s):** `sub_3AF00` per-node predicate (EF:30331-38, three clauses: model ≠ 22, and not an own-parent alliance summon) and its ring centre (EF:30204-05, `(x + 128) >> 8`) against `AddEventToMap_57D70`'s unrounded `x >> 8`. The fixture mc2l17 "a-shot-does-not-acquire-its-casters-own-charmed-ally" calls the ally clause "THE SAME THIRD CLAUSE … ON A SECOND CALL PATH".
- **Port arms:**
  - `mc2/castle.rs:3336 mc2_piece_scan`: TURRET/MC2 (`MGC_NO_MC2_PIECE_SCAN_ALLY`, `MGC_NO_MC2_PIECE_SCAN_CHAIN_TILE`)
  - `mc2/proj.rs:~4210` (the shot acquire's ally clause): PROJ/MC2 (`MGC_NO_MC2_AIM_ALLY`)
  - `mc2/mobs.rs:6255`: ALLIANCE brain, the same `site_z == 14 && mc2_allied == parent` test
- **Status:** IDENTICAL on the ally clause: the same predicate is written out three times inline. The human chain cell is landed in the turret only; the other ring scanners were not audited here.
- **Differences:**
  - Ally clause: none.
  - Human chain cell: the turret uses `x >> 8` for the out-of-pool human. `mc2/tail.rs:~2834` (the whirlwind grab) also uses `>> 8`. I did not check every other `ring_cells` caller.
- **What reaches each arm in-game:** an MC2 turret with a charmed or allied creature of its own owner nearby; the human half a tile off a ring edge.
- **Already witnessed?:** yes. mc2l17 fixtures (ally clause on both paths). Turret chain cell: mc2l17 t=6710 (code).
- **Recording ask:** none.
- **Confidence / notes:** high on the ally clause.

### CASTLE-C11: Wizard death PAYOUT `sub_5E310`, HUMAN vs RIVAL (MC2)
- **Retail routine(s):** MC2 `sub_5E310` (EF:60101-79), one routine for any wizard. What it does: a wizard's corpse touches down. `sub_49F90` rebuilds both stacks, the killer is credited, the mailbox is wiped, the 26 tokens scatter, the (10,40) grave spawns and owned spheres re-point to it, the corpse is hidden, and the victim list is popped (`--dword_0x11e6`).
- **Port arms:**
  - `engine/world.rs:12326 mc2_player_land`: HUMAN/MC2
  - `mc2/rivals.rs:10123 mc2_rival_death_impact`: RIVAL/MC2
- **Status:** DRIFTED.
- **Differences:**
  - **Victim pop.** Now in both: the human got it in a later round (`MGC_NO_HUMAN_PAYOUT_VICTIM_POP`); the rival has had it since round 98 (`MGC_NO_DEATH_PAYOUT_VICTIMS`).
  - **Victim-half rebuild.** The rival calls `mc2_rebuild_free` AND an extra `rebuild_recycle(0x2_0000)` behind `MGC_NO_DEATH_PAYOUT_VICTIMS`. The human calls only `mc2_rebuild_free`, whose victim half is behind `MGC_NO_MC2_REBUILD_VICTIM_HALF`. Under default switches the effect is the same (the rival's rebuild runs twice and is idempotent). Under `MGC_NO_MC2_REBUILD_VICTIM_HALF=1` only the human loses the victim rebuild.
  - **Kill credit.** The human arm credits only if the KILLER RECORD itself is a class-3 model 0/1 wizard (the EF:60110-22 cite), then `owner_slot_of_source`. The rival arm credits any killer resolvable by `owner_slot_of_source` except a (10,67) flood (the EF:60716 cite), and also bumps `g.kills` when the human is credited. So a rival killed by the human's FIREBALL is credited, while the human killed by a rival's FIREBALL is credited only if the killer id is the wizard itself.
  - **Grave-allocation failure.** The rival sets action 3 and the 1200 timer INSIDE the grave-spawn success arm (a full pool leaves the corpse in its fall). The human sets `LifeState::Dead` and `mc2_respawn_timer = 1200` unconditionally.
  - **Grave z.** Human: the landing `floor`. Rival: the corpse's `cz`. The human-arm comment says the rival "still passes ground_z", but the code now passes `cz`.
  - **Hide bit.** Human: `player.invisible = true; g.player_invisible = true`. Rival: `flags |= 0x20`.
- **What reaches each arm in-game:**
  - HUMAN: the human dies in MC2 (killed by a rival's spell, a creature, or Shift+K) and the corpse lands.
  - RIVAL: a rival wizard dies and its corpse lands.
  - The kill-credit difference fires whenever the killer was a PROJECTILE or creature owned by a wizard rather than the wizard record itself.
  - The grave-failure difference needs an exhausted pool at the moment of touchdown.
- **Already witnessed?:**
  - Human pop: yes (mc2l22 t=10021, from the code).
  - Rival pop: yes (mc2l22 t=73, from the code).
  - Kill credit: no evidence found. `kill_tally` feeds the stats screen, an ungraded lane (see the memory note "the two new counters sit on graded paths" about the stats session).
  - Grave failure: no evidence found.
- **Recording ask:** MC2. (1) Get killed by a rival's fireball (not melee or contact), let the corpse land, and after the level check the stats screen's kill column for that rival. (2) Kill a rival with your own fireball and check your kill count. About 5 minutes. The grave-failure arm needs the pool full at a death touchdown (mass-spell chaos); opportunistic.
- **Confidence / notes:** this is really WIZARD-DEATH subsystem territory. It is listed here because the parent brief named "payout". The kill-credit difference is the notable one; `owner_slot_of_source` should be read to confirm what the human arm's pre-filter excludes.

---

## Seen but belonging to another subsystem
- **MC2 castle LEVEL-UP castle XP is pushed for the HUMAN only** (`mc2/castle.rs:869-871`, `if own == PLAYER_TARGET { mc2_cast_xp.push((own, 2, 1)) }`), while retail `sub_60480` EF:61596 calls `sub_6D8B0(owner, 2, 1)` for any owner. This affects rival castle-tier XP → UPGRADE/TIER (part B) or XP subsystem.
- **`rival_castle` pool scan vs `castle_reg` register** for ~20 rival-AI gates (named in `engine/features.rs:3808-3812`) → RIVAL AI.
- **MC2 demolish phase's second half**, the `mc2_rival_pick_castle` live-life guard (`MGC_NO_MC2_DEMOLISH_PHASE`) → RIVAL AI (raid target).
- **MC1 house-tally register** `banked_houses` (human) vs `rival_banked_houses[t]` (rival), written in two places in `engine/world.rs:15390/15446` → MANA / HOUSES census.
- **Out-of-pool human's chain-cell arithmetic in the other SEARCH.DAT `ring_cells` scanners** (`mc2/tail.rs`, `mc2/mobs.rs:3306`, `mc2/multipart.rs:787`, `mc2/effects.rs:1241`): only the turret and the whirlwind were checked → SPELL EFFECTS / CREATURES.
- **Wizard-death payout (CASTLE-C11)** → WIZARD DEATH / RESPAWN; MC1's payout twin not examined.

---

### CASTLE-D1: the (3,2) castle ctor (sub_37920 MC1 / sub_4AA40 MC2) — three copies
- **Retail routine(s):** MC1 `sub_37920` (remc1 :44229; HW `sub_37920_37CE0`) and MC2 `sub_4AA40` (EF:33362-33433) — the class-3 model-2 castle anchor entity: tile-corner parity snap, state-5 build machine, life 40000, channel mask 33, sprite 177, link to map. Every castle birth (level-authored, castle-ball landing, rival free/castle-less plant) goes through this ONE ctor in each binary (MC2 reaches it via `IfSubtypeCallCreatingManaSphere_4A190(.., 3, 2)` from all sites, per the port's own citations at mc2/rivals.rs:8931 and :2946).
- **Port arms:**
  - `crates/mgc-sim/src/mc1/combat.rs:4352 Gen::spawn_castle` — SHARED ctor, both games (per-game `match verbs.movement` for the site datum and sprite). Callers: MC1 human castle-ball landing (mc1/combat.rs:4145, :4276), MC2 castle-ball landing for human AND rival balls (mc2/proj.rs:3015), MC2 rival castle-less free plant (mc2/rivals.rs:8950). — HUMAN+RIVAL/MC2 cast, HUMAN/MC1 cast
  - `crates/mgc-sim/src/mc1/mobs.rs:471 Gen::spawn_class3` model-2 arm — MC1 COPY. Callers: MC1 rival castle-less free plant `rival_cast_castle` (mc1/rivals.rs:5782), MC1 authored rival starting castle `spawn_starting_castle` (mc1/rivals.rs:1735), level-thing spawns (engine/world.rs:19702). — RIVAL/MC1 + AUTHORED/MC1
  - `crates/mgc-sim/src/mc2/rivals.rs:2898 Mc2Ctx::mc2_spawn_authored_castle` — MC2 INLINE HAND-ROLLED copy (new_event + field writes). — AUTHORED/MC2
- **Status:** DRIFTED (MC2 shared ctor vs MC2 authored inline copy); MC1 two copies IDENTICAL in effect.
- **Differences:**
  - MC2 authored copy gates the channel mask `f28 = 33` behind `MGC_NO_MC2_CASTLE_CH_MASK` (mc2/mobs.rs:572); `Gen::spawn_castle` writes it unconditionally (no switch). Same law, landed separately.
  - MC2 authored copy gates the link-z law (link at ground under the RAW point, `site_z` = perimeter-min only in +0x9E) behind `MGC_NO_MC2_AUTHORED_CASTLE_LINK_Z` (mc2/rivals.rs:245); the shared ctor has the same law unconditionally. The authored copy's raw point is the WIZARD's position (`ground_z(wx, wy)`), the snap source is also the wizard position — consistent with the ctor being called on the wizard axis.
  - Authored copy writes `id24` BEFORE link (shared ctor callers write it after return) and sets `tick70 = 4` first then overwrites to 5, plus explicit `f59 = 0`, `f50 = 0`; net state equals the ctor's (5, sub-state 0). No behavioural difference found.
  - Authored copy does NOT set `f26 = 0` in the ctor portion (it sets `f26 = castle_level-1` afterwards; the ctor would have 0 then the caller overwrites) — no visible effect.
  - Sprite: authored copy calls `mc2_set_sprite(c, 177)` only; the shared ctor's MC2 arm calls `mc2_set_sprite` and then re-stamps `frames89 = FRAME_COUNTS[SPRITE_PARAMS[177].byte_12]` UNCONDITIONALLY. `mc2_set_sprite` (mc2/mobs.rs:847) derives `frames89` from `mc2_sprite_frames(idx)` but only when `MGC_NO_MC2_FRAMES89` is unset — so the two arms differ only under that kill switch (the shared ctor ignores it). Ungraded `b5d` lane per the code comments.
  - MC1 copies: `spawn_class3` m2 vs `spawn_castle` MC1 arm — same snap (u16 `tx` wrapping vs u8 `cx` wrapping; both wrap tile 255→0 after the `<<8` in u16), same `site_z = link_z = ground_z(raw x,y)`, same f28/life/sprite. Only `spawn_castle` writes `f59 = 0` explicitly (new_event already zeroes). No difference found. Note the MC1 rival/authored callers pass a `gz` z argument that the model-2 arm ignores (as retail ignores the caller's z).
- **What reaches each arm in-game:**
  - Shared ctor / MC2: any castle raised by a castle BALL landing (human casts Castle with no castle, or upgrades in MC2 where the ball builds), or an MC2 rival that has no castle planting its first one (rival re-builds after its castle is razed).
  - MC1 copy (`spawn_class3`): an MC1/HW rival wizard with no castle planting one (castle-less rival after its castle is destroyed, e.g. mc1l5 Vodor's post-raze rebuild), and MC1 levels that author a rival starting castle.
  - MC2 authored copy: MC2 level start where a rival begins with a castle (mc2l22 levels 5/5/5/7, mc2l4/mc2l6 level-1 stumps); only at level load.
- **Already witnessed?:**
  - Shared ctor MC2: yes — birth ticks cited in code: mc2l0 t=7224 slot 4, mc2l30 t=234 slot 126, spells-galore t=1029, mc2l1 t=2069, mc2l6 t=70/t=104 (rival plant), mc2l6-rsg t=28706.
  - MC1 copy: yes — mc1l5 t=14771 slot 478 (rival plant), mc1l0 t=562 (site z 797, via castle-ball = shared ctor MC1 arm), mc1l5 record-0 castle 680 (authored).
  - MC2 authored: yes at record 0 — mc2l22 / mc2l22-new slot 476 (link-z, settled 2 ticks); but replay IMPORTS the pool at record 0, so the authored ctor's own writes are graded only on takes whose record 0 catches the castle before its first standing tick (mc2l22-new). Per memory, native init is never graded.
- **Recording ask:** Low priority — all arms have witnesses. If the collapse wants a second authored-MC2 witness: start any MC2 level that authors a rival castle on uneven ground (mc2l22) and ensure the recorder goes live at t=0/1 (the late-start fix), so record 0 still carries the castle's birth link-z before the standing refresh. ~1 minute.
- **Confidence / notes:** High on the arm map (read all three bodies). The authored copy is purely a structural duplicate; its two kill switches are the only drift signal.

---

## Summary table

| id | name | status | arms | recording needed |
|---|---|---|---|---|
| CASTLE-A1 | MC1 Create-Castle token machine | DRIFTED | HUMAN `manifestation_tick`+`cast_castle` / RIVAL `rival_castle_token_tick` | y (ball +140/+44 source after a death and respawn at the castle) |
| CASTLE-A2 | MC1 castle price resolver | DRIFTED | `spell_cast_cost` (floor 1000) / `rival_castle_price` (0) | y, latent (a recycled token slot in a pool-starved late level) |
| CASTLE-A3 | MC1 wizext+50 register readers | IDENTICAL on the default arms; switches and OFF arms drift | HUMAN bind readers / RIVAL `rival_castle_reg` | y (at-castle overlap box at the footprint rim) |
| CASTLE-A4 | MC1 (10,43) upgrade-token castle resolution | DRIFTED (register law on MC2 only, no MC1 switch) | MC1 pool scan `f26>0` / MC2 register | y (two castles, then upgrade); check mc1l26-froze t=25107-8 first |
| CASTLE-A5 | MC1 castle ball: create vs homing arm | STRUCTURALLY-DIFFERENT (2 retail bodies) | create / homing (+146) | y (homing ball arrives on an exhausted pool) |
| CASTLE-A6 | MC1 owner castle-token resolution | DRIFTED (3 resolutions) | `castle_owner_token` / `release_castle_charge_pin` / `mc1_respawn_reprice` | y, latent (aliasing in a starved pool) |
| CASTLE-A7 | MC1 castle→token ladder writers | DRIFTED (arithmetic) | level-event stamp / respawn re-price | n (unreachable) |
| CASTLE-B1 | MC2 castle-lock release + tier-defer drain | DRIFTED (residual) | HUMAN / RIVAL (two kill switches) | y |
| CASTLE-B2 | MC2 castle manifestation body + ball mint | DRIFTED | HUMAN (`sel[2]` research, register) / RIVAL (`f71`, pool scan) | y (tier switch mid-upgrade; rival re-cast on a level-0 flag) |
| CASTLE-B3 | MC2 SetSpell / GetSpellManaCost pricing | DRIFTED | `mc2_set_spell_at` (register) / `mc2_rival_set_spell_at` (scan) | y (free-spells cheat take; rival castle split) |
| CASTLE-B4 | MC2 ladder-sync drain re-price | DRIFTED | HUMAN register / RIVAL scan + dying-castle fallback | y (rival owns two castles, the older one damaged) |
| CASTLE-B5 | MC2 at-castle probe | DRIFTED | HUMAN / RIVAL | y (raid while docked; split castles) |
| CASTLE-B6 | MC2 CastleEntityIndex register census | DRIFTED (class) | human readers on the register / every rival reader on a scan; rival plant never writes `castle_reg` | y (rival castle split; human split; a fresh rival plant) |
| CASTLE-B7 | MC2 rival castle-spell affordability | DRIFTED (within the rival column) | 3 rival gates on the stamped price / 1 gate on the bare rung | n (a data check first) |
| CASTLE-C1 | castle level-off / destroy (action 6), MC1 vs MC2 | DRIFTED (MC1 vs MC2) | MC1 features.rs / MC2 castle.rs | n (optional overfilled level-7) |
| CASTLE-C2 | action-6 Create-Castle token pin/release, MC1 vs MC2 | DRIFTED | MC1 / MC2 | y (MC1: kill a live rival's level-1 castle, record the rebuild) |
| CASTLE-C3 | MC2 castle-death re-price, HUMAN vs RIVAL | DRIFTED | HUMAN register / RIVAL scan + fallback | y (a rival with two castles; opportunistic) |
| CASTLE-C4 | Demolish (Shift+L), MC1 vs MC2 | STRUCTURALLY-DIFFERENT (2 retail routines) | MC1 / MC2; demolish vs damage death | y (MC1 Shift+L at level 2 and at level 1) |
| CASTLE-C5 | castle damage intake, MC1 vs MC2 | DRIFTED (unreachable arm) | `sub_47EC0` / `sub_609E0` | n |
| CASTLE-C6 | killer latch + under-attack alert | IDENTICAL latch; DRIFTED HUD alert | castle intake / balloon intake | y (HUD flash on a balloon and castle hit) |
| CASTLE-C7 | castle mana ejector, MC1 vs MC2 | DRIFTED (both witnessed) | `sub_47130` / `sub_5FD00` | n |
| CASTLE-C8 | balloon fleet dispatcher, MC1 vs MC2 | DRIFTED | MC1 life-only register / MC2 own-live-balloon test | y (MC2 balloons out, pool exhaustion) |
| CASTLE-C9 | turret human-raise aim vs the other sub_65580 homes | IDENTICAL (after r139) | turret / ~15 inline homes | n |
| CASTLE-C10 | turret ring scan ally clause | IDENTICAL (3 inline copies) | turret / other roster scans | n |
| CASTLE-C11 | MC2 wizard death payout, HUMAN vs RIVAL | DRIFTED | HUMAN (killer = wizard only) / RIVAL (any owned source except the flood) | y (killed by a rival's fireball; kill a rival with a fireball) |
| CASTLE-D1 | (3,2) castle ctor, 3 copies | DRIFTED (MC2 authored inline copy); MC1 copies IDENTICAL | `Gen::spawn_castle` / `spawn_class3` m2 / `mc2_spawn_authored_castle` | n (optional: an early-start mc2l22 take) |

### Cross-cutting observations
- **The dominant drift in this subsystem is register vs pool scan.** On MC2 every rival castle reader still scans (B3-B6, C3); on MC1 the rivals moved to the register but the (10,43) upgrade token did not (A4). The scan and the register disagree only in three cases: an owner with **two castles**, a **fresh level-0 castle**, or a **dying castle**. So the single most valuable recording is a **castle split**: a rival (MC2) or the human (MC1 and MC2) holding two castles, or a castle being razed while its rebuild ball is in flight. mc2l12 (human split) is the only corpus witness found.
- **Kill switches on one arm only:** `MGC_NO_MC2_CASTLE_CH_MASK` and `MGC_NO_MC2_AUTHORED_CASTLE_LINK_Z` (authored ctor copy only, D1); MC1 debit-order switch on the rival only and dry-pool-retry switch on the human only (A1); `MGC_NO_MC2_TOKEN_CASTLE_REGISTER` on the MC2 arm only (A4).
- **Possible real port gap (unverified):** the MC2 rival castle-less plant (`mc2/rivals.rs:~8950`) does not write `castle_reg`, although the port's own citation says retail writes `CastleEntityIndex` there (EF:6836-38), and the MC1 rival plant does bind. See B6.
