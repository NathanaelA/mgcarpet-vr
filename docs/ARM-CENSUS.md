# ARM CENSUS — the twin map before the collapse

**Started 2026-09-24 (player-started, multi-session).** This is the groundwork for the
post-conformance refactor in `ROADMAP.md` ("COLLAPSE THE PER-COLUMN ARMS"). Retail has
ONE routine where the port has several arms (human/rival, MC1/HW/MC2, flight/pool,
free-run/pinned-pose, per-model copies). Laws have landed arm by arm. Before collapsing
twins into one routine, we need to know **which arms exist, how they differ, and what
in-game situation reaches each one**, so the player can record a retail witness for each
arm and the collapse isn't done blind.

⚖ **This census does NOT rule which arm is right.** "DRIFTED" means the arms differ, not
that one is a bug. The recordings decide.

## Files

| file | subsystem | clusters |
|---|---|---|
| [arm-census/MOVE.md](arm-census/MOVE.md) | wizard movement & pose, falls, knock, whirl/quake on wizards, pads | 21 |
| [arm-census/LIFE.md](arm-census/LIFE.md) | damage intake, death, dead-wait, respawn, kill credit, mana floors | 16 |
| [arm-census/CAST.md](arm-census/CAST.md) | cast gates, spell tokens, spellbook, pricing | 20 |
| [arm-census/CASTLE.md](arm-census/CASTLE.md) | castles: register, token, ball, lock, demolish, payout, balloons | 26 |
| [arm-census/PROJ.md](arm-census/PROJ.md) | aim/acquire, projectiles, rebound, debuffs, AABB/area writes | 13 |
| [arm-census/MOB.md](arm-census/MOB.md) | creatures: chase cores, charm/alliance, summons, rosters, wrapper tails | 30 |
| [arm-census/WORLD.md](arm-census/WORLD.md) | quake/whirl/mine/switch/dolmen/jars/balloons + replay-import twins | 17 |
| [arm-census/leads/](arm-census/leads/) | lead scripts (`routine_citations.py`, `kill_switch_uses.py`) + the agent brief | — |

Each cluster has the same fields: retail routine, port arms (`file:line fn`), status,
differences, what reaches each arm in game, whether each arm is already witnessed, and a
recording ask. Every subsystem file ends with a summary table and a list of clusters it
handed off to other subsystems.

**Totals: 143 clusters.** 108 DRIFTED · 18 STRUCTURALLY-DIFFERENT · 16 IDENTICAL ·
1 UNKNOWN. **81 want a recording**, 60 don't, and 2 need a data check first.

## How it was built (so it can be redone)

1. `leads/routine_citations.py`: every `sub_XXXXX` cited in `mgc-sim`, and the fns that
   cite it (doc-comment citations are likely implementations). 1,167 routines; 407 are
   cited from ≥3 files, but most of those are just CALLS.
2. `leads/kill_switch_uses.py`: all 619 `MGC_NO_*` switches mapped to the fns that consult
   them. ⭐ A law consulted in arm A but not twin B is the strongest drift signal.
3. Seven Opus research agents, one per subsystem, working from one brief
   (`leads/AGENT-BRIEF.md`). They were read-only, told to read both bodies before claiming
   a difference, and told not to give verdicts.
4. Main-session spot checks (all held): MOVE-16 (the MC2 pad has no rival warp; the port's
   own doc says "owed"), PROJ-2/WORLD-12 (the mine aims at the human's raw `pz`, pool
   victims at `aim_z()`), MOB-2 (retail `EventsFunctions.cpp` has 11 `sub_1ED30` hits, the
   port wires 2 callers and says the rest are OWED), CASTLE-B6 (the MC2 rival first-castle
   plant writes no `castle_reg`; the only MC2 writer is the level-up commit
   `mc2/castle.rs:867`), and rivals being `(3,1)`.
   ⚠ **Everything else is agent-claimed and still needs checking** before any code acts on it.

## Themes that run across subsystems

These are drift CLASSES. One law reached some homes and not others. They are the best
units for the collapse:

- **Castle register vs pool scan (MC2 rival column).** The human column reads
  `CastleEntityIndex_0x3A_58`; every MC2 rival castle reader still does a pool scan:
  CASTLE-B2..B6, C3; CAST-17/18. MC1 rivals moved to the register, but the MC1 (10,43)
  upgrade token didn't (CASTLE-A4). The arms only disagree when a wizard has **two
  castles, a fresh level-0 castle, or a dying castle.**
- **Tick-top roster vs live pool.** The "walk the list built at tick start" law is missing
  from about 10 creature scans in each game (MOB-3/4/16/28), and from the human-death
  reads (`pdead` vs `pdead_top`).
- **The rival copy of the carpet mover only ever moves a CORPSE.** Retail's mover
  (`sub_5D530` MC2 / `sub_455D0` MC1) is called only by the human's alive tick and by the
  shared death fall; alive rivals use `sub_146F0`/`sub_14EB0`. So the MOVE rival-arm asks
  are all "a rival DIES near rock/water/a funnel" (MOVE-1..7, 11).
- **The human-raise (`sub_65580`/`sub_655C0`) and signed-AABB laws.** Both are mostly
  landed. The leftovers are the Magic Mine (PROJ-2 = WORLD-12) and about 11 hand-rolled
  carpet boxes (PROJ-6 = WORLD-13, ungraded in practice; unify on `aabb_ext`).
- **Rival-only / human-only kill switches.** Many clusters apply the same law in both arms
  under separate switches. That's no behaviour risk, but the collapse must merge the
  switches (MOVE-9, CAST-5/9/20, PROJ-8/11/12, MOB-7/21).
- ⭐ **Rivals are `(3,1)` in both games.** Every retail `class==3 && model==0` branch is
  HUMAN-ONLY in single player. Whirl constants, the flood z-leg, pitch-512 and the switch
  probe are therefore not rival drifts; don't re-dig them.
- **MC1 vs HW creatures:** the retail routines spot-checked line-identical, and the port
  has no HW branch in `mc1/mobs.rs`. No MC1-vs-HW recordings are needed.

## ⚠ Arms a retail recording CANNOT reach

These are native-only vs replay-only twins. Replay imports state, so it never runs the
native arm (see memory "REPLAY IS BLIND TO INITIALISATION"). These need a
**native-vs-pooled harness or a unit test**, not a take:

| id | twin |
|---|---|
| MOVE-18 | carpet dispatch anchor pooled vs native (MC1 native runs the danger clock while dead; MC2 native respawn seat lands after the turn) |
| MOVE-21 | the respawn register clear is hand-copied in 4 places (app, both drivers, MC2 in-walk) |
| CAST-4 | MC1 Shield: `mc1_shield_token_tick` (replay) vs the `manifestation_tick` skeleton (native); different cost operands, and the native arm drops the bit on the last tick |
| CAST-12 | MC2 respawned rival's book below its pool slot: in native, possibly **no token body runs at all** (only Speed/Castle have stand-ins). Needs a native run. |
| WORLD-8 | dolmen: native live probe vs replay slot latch |
| WORLD-9 | MC1 jar poll strict (every 4th tick) vs native (every tick, eager `owned`) |
| WORLD-14 | import `mc2_applied_mana_delta` vs native manifestation/afford (negative purse passes natively; the import has no duel-fizzle arm) |
| WORLD-15 | import castle-pad stamp vs the live painter |

## 🐛 Candidates that look like outright gaps (not verdicts; verify first)

- **MOVE-16:** the MC2 teleporter pad never warps rivals (`engine/world.rs:22354`, "owed").
- **MOB-2:** the charm resolver's null arms are missing for 8 of 10 retail callers.
  Charmed m9/m17/m18/m19/m21/m24 never drop a target the caster isn't fighting.
- **MOB-1:** the pyramid summon (SV2 16) can only lock the human, never a rival wizard.
- **CASTLE-B6:** the MC2 rival's first-castle plant doesn't write the castle register
  (retail does, EF:6836-38). Suspected effect: at least a one-tick lag, until the level-up
  commit writes it.
- **PROJ-4 / CAST-14:** the port cannot represent a rival's precise-tier (tier-1) Rebound
  at all (no writer).
- **LIFE-1 / LIFE-12:** MC1 Steal Mana by a rival credits no one, and MC1 Tether by a
  rival records no duel lock (MC1 rivals cast both).
- **LIFE-11:** the MC2 rival re-mint never sets the "in the book" flag bit (medium
  confidence).

## ⚖ Player rulings

- **2026-09-24: MC2 rival AI does NOT use Magic Mine.** The rival-mine arms are reachable
  only in multiplayer, so no single-player recording can witness them. **Implement them
  the way the dig says anyway.** Affected clusters:
  - PROJ-2 / WORLD-12: `mc2_mine_detonate` should aim at the human tripper with the same
    `+PLAYER_HH` box-centre raise that pool trippers get via `aim_z()`, like the other 8
    MC2 `sub_655C0` sites;
  - CAST-20: rival first-tick mine emission.
  Removed from the shopping list; they are unit-test-only arms.

- **2026-09-24: MC2 CASTLE SPLITS need no witness.** The player doesn't believe rival AI
  can reach a split castle. And even if it can, multiple castles must be PREVENTED (the
  `one_castle_per_wizard` patch: default ON in the app, a 2026-09-15 player ruling that
  already declined to model the split's blast radius faithfully). So the split-only
  behaviour of the register-vs-scan arms may be unified freely, without a retail-faithful
  split arm.
  Affected clusters: CASTLE-B2..B6, CASTLE-C3, CAST-11/17/18 (their split legs).
  ⚠ **Scope note:** register and scan ALSO disagree without any split:
  - a **fresh castle**: the register lands a tick after the mint, in the level-up commit,
    and the rival's first plant writes none (CASTLE-B6);
  - a **dying / level-0 castle**.
  Those windows occur in ordinary single-player play, so they keep their recording asks
  below. MC1 splits (CASTLE-A4, no owner test on the create arm) are outside this ruling.

- **2026-09-24: NO RIVAL EVER MEETS A PYRAMID SUMMON (MOB-1).** The final MC2 level is
  rival-less in every incarnation, so "a pyramid summon (SV2 16) locks a rival wizard"
  can never happen. **Take the cleaner unification**: one `sub_1E580`/`sub_1E700`
  routine for SV2 13/14/16, with the retail wizard walk as in 13. The port's
  "16 only ever locks the human" shortcut goes away.
  ⚠ The other 13-vs-16 differences are still reachable on the final level and are NOT
  waived: dead/0-life target within the tick, live vs tick-top human death, crowd steer
  while unlocked, parent-alive probe. Both arms carry witnessed laws (mc2l6 for 13,
  mc2l24 for 16) and switches the other lacks (`MGC_NO_DOOM_DEAD_TARGET`,
  `…_DOOM_CROWD_ROSTER`, `…_DOOM_DRIFT_HIT_ARM`, `…_DOOM_PARENT_AIM_ORDER`), so the merged
  routine must keep every fixture green.

- **2026-09-24: RETAIL NEVER PUTS RIVALS IN A CAVE LEVEL.** The rival AI has no real
  pathfinding and only flies straight lines, so caves would wreck it. Some unused
  experimental levels do place rivals in caves, which shows the intent, but no shipped
  cave level has any. So every "rival in a cave" leg is unreachable in shipped play and
  may be unified freely:
  - **MOVE-1:** the `zero_speed` refusal leg of the rival corpse glide fires only in a
    cave (map_type 2). The other refusal leg, deep water on an open level, is already
    witnessed (mc2l16 t=19784). MOVE-1 now needs NO recording.
  - **MOVE-13 / WORLD-2:** the cave whirlwind asks stand, but only with the human's own
    or a trap switch's whirlwind, never a rival's.

- **2026-09-24: TWO-CASTLE WITNESS: FOUND, BUT IT IS MC2 (CASTLE-A4 stays open).** The
  player remembered a take with the human owning two castles and fully upgrading the
  second. A scan of all 84 `mc1l*`/`mc1hwl*` takes in `recordings/` (every 12 ticks,
  `leads/castle_owner_scan_mc1.py`) found **no MC1/HW take where any wizard holds two
  `(3,2)` at once**. The take is **`recordings/mc2l12.mgcr`** (`leads/castle_owner_scan_mc2.py`,
  `castle_split_probe_mc2`); the register always latches the NEWER castle:
  - t=18274: 678 + 708 split, and 678 is orphaned at level 1 until ~t=42387;
  - 293 reaches level 7 by t=25460 beside the orphan;
  - 877 is fully upgraded 1→7 at t=44038-44183 beside 293;
  - 18 is fully upgraded 1→7 at t=48357-48506 beside 293.
  So the MC2 split legs HAVE a witness (on top of the split ruling), and the MC1 A4 ask
  stands unless an MC1 take in `recordings-new/` has it.

- **2026-09-24: MC1 CANNOT MAKE TWO CASTLES IN RETAIL (CASTLE-A4 waived).** The player
  tried in MC1 retail and couldn't get even a second castle ball out. MC1 keeps the Create
  Castle spell "in effect" while the ball is in the air AND while the castle transforms,
  so one cast yields one ball. The MC1 two-castle legs are therefore unreachable, and
  A4's register-vs-scan arms may be unified freely.
  ⚠ **Follow-up (a finding, not a ruling):** `docs/DEVIATIONS.md` `one_castle_per_wizard`
  and the test `mc1_a_second_castle_ball_builds_a_second_castle_only_on_retail`
  (`engine/world.rs`) claim MC1 is "looser": a 101-shot burst that sprays balls while you
  turn. That contradicts retail play. The port does model a castle charge pin
  (`release_castle_charge_pin`), so the claim may only be a stale rationale. But check
  whether the NATIVE port lets MC1 fire a second castle ball with the patch on its retail
  arm. If it does, that is a port bug in the token gate.
  Player clarification: in retail, neither rapid clicking nor click+hold ever puts more
  than ONE castle projectile in the air. If the port differs, fix the port. The player
  expects it already matches, so this is probably only the stale text and test rationale
  to correct.

## 🎬 Recording shopping list

Grouped by game. Each item names the clusters it feeds; the details (exact condition,
what to watch) are in the cluster's **Recording ask**. ★ marks high value: a real
behaviour split with no witness, and triggerable on purpose.

### Check existing takes FIRST (no new recording)
- CASTLE-A4: mc1l26-froze t=25107-25108 (the upgrade token's castle resolution).
- WORLD-17: mc2l22 t=1198-1199, rival carpets' `flags & 1` (the Metamorph ctor bit).
- PROJ-13: `dump-state` the human carpet's `+28` in any take. A full mask means no
  recording is needed.
- LIFE-11: verify bit 0 on the MC2 rival re-mint before recording.
- CASTLE-B7: a data check (is any tier's raw `manaCost_6` below 1000?).

### MC2
- ★ **Rival across a teleporter pad**: mc2l7 or mc2l24, the rival facing the pad. (MOVE-16)
- ★ **Castle register windows (no split needed, see ruling)**:
  - a rival's FIRST castle plant, and the ticks right after it (CASTLE-B6);
  - raze a rival's castle down to level 0 / to death and keep recording while it rebuilds
    (mc2l6/mc2l12/mc2l22);
  - switch the Create Castle tier while an upgrade is rising.
  (CASTLE-B1..B6, C3; CAST-11/17; split legs waived)
- ★ **Alliance on firebugs, tanks, devils, dive-bombers or brutes**: let them engage
  something you're not fighting, then switch targets, then let the charm expire mid-attack.
  Also charm a pack whose target dies mid-fight, and charm next to held same-species
  creatures. (MOB-2/6/9/10/25/27)
- **Summons with a dying human**:
  - on the final (pyramid) level, die while pyramid summons are out, and let several
    cluster with no target;
  - elsewhere, cast Summon Army, then die while the creatures are out.
  (MOB-1; the rival-lock leg is waived by ruling)
- ★ **A rival webbed twice in a row** by m17/m20 lobbers (mc2l22/mc2l24/mc2l3). (PROJ-5)
- ★ **Fire into a rival's precise-tier Rebound**, and get hit by a bolt right at your own
  Rebound expiry. (PROJ-4, CAST-14)
- ★ **Die in the open and stay dead** while rivals or archers keep firing, 2–3 deaths.
  Also die inside creature scan cones. (PROJ-1, MOB-28)
- **Respawned rivals fighting for a while** (mc2l22). (CAST-12; natively suspect as well)
- **Rival deaths in awkward places** (open levels only; rivals never fly caves): inside a whirlwind
  funnel; from a big hit just above the ground. (MOVE-3/4/11; MOVE-1 waived by the cave ruling)
- **Human hazards**:
  - skim low over deep water into a rising shore (MOVE-2);
  - get grabbed by a whirlwind mid-ring near the ground or a cave ceiling (MOVE-13,
    WORLD-2);
  - hover dead-centre of a rival's quake for 20–40 s, and quake a rival carpet (WORLD-1,
    MOVE-15);
  - cast Teleport under fire (MOVE-17);
  - hold a Duel, and re-press Duel within ~20 ticks, 3–4 times (MOVE-5, WORLD-14).
- **Death payout/intake**:
  - get killed by a rival's fireball, and kill a rival with yours, then check the stats
    screen (CASTLE-C11, LIFE-7);
  - get killed by the (10,67) effect and watch the corpse (LIFE-2);
  - die with a charged Shield and ~0 mana, then respawn (LIFE-2).
- **Invisibility (all tiers) then cast**, plus Metamorph. (CAST-15/19)
- **Pool exhaustion** near rival castles with balloons out (lightning/meteor spam for
  10–20 min). (CASTLE-C8, WORLD-6)
- **Village churn**: destroy a house, and let one be built, next to villagers and traders
  (mc2l0/l1). (MOB-4)
- **Class-3 churn near m9 hives / m23 leviathans**: kill a rival next to a hive, fill a
  balloon near a leviathan. (MOB-3)
- **Under-attack HUD flash** on a balloon hit and a castle hit. (CASTLE-C6)
- Creatures on the map-centre line, across the 0x8000 seam. (MOB-29)

### MC1 / HW
- ★ **A rival hits you with Steal Mana and Tether**, then kill it within ~20 tiles while it
  is tethered. Also tether a rival yourself, die, and respawn near it. (LIFE-1/5/9/12)
- ★ **Die to a rival, respawn, Shift+K**, and watch the corpse's facing for 3–4 s.
  (LIFE-9/10)
- ~~Own two castles, then upgrade~~: impossible in MC1 retail (player-tested; see ruling). (CASTLE-A4)
- ★ **Shift+L at castle level 2, then at level 1.** Also during a build or repaint, and with
  a second castle standing. (CASTLE-C4)
- **Kill a live rival's level-1 castle** and keep recording ~200 ticks while it rebuilds.
  (CASTLE-C2)
- **Invisible, then fireball** (with a castle). (CAST-6)
- **Die while Shield or Accelerate is running**, and die mid-burst. (CAST-1/7)
- **Heal while hurt, then at full life.** (CAST-3)
- **Die holding 4+ spells** over flat ground. (WORLD-10)
- **Your own area spells on your own castle guards.** (MOB-17)
- **Vultures hit and killed near a grave; a kraken promoted by a hit; a kraken tethering a
  rival.** (MOB-15, MOVE-20)
- **Genies among mana balls and dying rivals** (mc1l42); guard melee. (MOB-16)
- **A wyvern/guard pack whose follower dies.** (MOB-13)
- **Hover at the rim of your castle footprint**, in and out. (CASTLE-A3)
- **A rival parked in a repeating switch box.** (WORLD-7)
- **A dolmen**: park on it ~20 s. (WORLD-8)
- Opportunistic, long pool-starved endgames (mc1l48/l49-style): a castle token slot
  recycled, a homing ball arriving on an exhausted pool, death on a full pool.
  (CASTLE-A2/A5/A6, LIFE-6)

### Not recordable / ungraded (collapse on the unit test and goldens)
PROJ-2/WORLD-12 + CAST-20 (rival Magic Mine, MP-only per player ruling), PROJ-3, PROJ-6, WORLD-13, MOB-20, CASTLE-A7, CASTLE-C5, WORLD-4 (patch arm only), WORLD-5
(sound), plus the replay-blind table above.

## Next steps

1. **Verify** the agent claims that any code change or recording will rely on, starting
   with the ★ items and the gap candidates. Each cluster lists `file:line`.
2. **The player records** against the shopping list. Takes go through the normal intake;
   each new witness pins the arm it exercises.
3. **Collapse cluster by cluster**, one retail routine per port routine, with per-column
   differences passed as data. Guard: goldens + fixtures. ⚠ A collapse that moves a golden
   or reds a fixture has changed behaviour and must be re-derived, never re-baselined.
4. **Unrecordable arms go to future sessions** (player, 2026-09-24): the replay-blind table,
   the MP-only / ruled-unreachable legs, and the "not recordable / ungraded" list.
   Anything that happens in the port can be unit-tested or at least simulated, so each one
   gets a unit test or a native-vs-pooled harness run instead of a take.
