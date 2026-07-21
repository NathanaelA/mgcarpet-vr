# DEVIATIONS — deliberate departures from retail behavior

The canonical register of every place where mgcarpet INTENTIONALLY departs
from retail Magic Carpet 1/2 behavior: gameplay rulings, enhancement- and
preference-class options, deliberate approximations, and fixed retail bugs.
Extracted from inline code comments during the 2026-07-19 comment sweep; the
source sites keep a terse `(deliberate)` marker pointing here.

Maintenance law: when introducing a new deliberate deviation, add its ruling
here (what deviates from retail, and why) and mark the code site
`(deliberate)`. When an audit finds code diverging from the decompile, check
this register BEFORE "fixing" toward retail. Companion doc: docs/FIDELITY.md.

A few rulings surface in more than one layer (e.g. `prune_owned_jars` in the
sim, the config registry, and the app loader; `smooth_motion` in config and
settings; the MC1 duel-pull knock-channel transport in world.rs and
rivals.rs) — each layer's entry describes its own surface.

The mgc-sim test suite deliberately contributes no entries: tests pin
faithful retail behavior only; enhancement touchpoints are documented at
their source.


## mgc-sim — shared world core (mc1/world.rs)

- **world.rs module header** — Faithful subset only: no AI wizard balloons (the probe/scan lists are the player alone); custom family behaviors beyond movement/combat (disguises, mana hunts, house building, teleports) stand still pending the AI track; class-12 pickup/mana transfer not ported (mana balls drop, merge and take claims but nothing collects them); sounds omitted. Tracked in docs/ROADMAP.md.
- **World::prune_owned_jars** — Unfaithful improvement (P-class option, default OFF): removes any spell jar the local player already owns. Retail leaves such jars in the world forever (life 0, uncollectable clutter). When on, an owned-spell jar self-culls on its next tick.
- **World::won** — On win, the app fades out and ends the game with no stats screen / campaign-stitching sequence (deliberate simplification; "yet" — still open work).
- **World::tick (MC1 duel pull)** — The caster's duel pull magnitude formula is traced from retail (:55228-48), but it is applied through our own knock channel rather than retail's transport (deliberate approximation).
- **World::player_land (jar scatter)** — The death jar scatter uses the same LCG constants as retail (:55519-47) but draws from the world RNG stream rather than the dying wizard's private stream, which is not modeled outside flight (deliberate approximation).
- **World::player_land (grave)** — On a full entity pool, retail retries the whole death landing next tick; ours proceeds graveless (loose mana balls stay player-owned) — a benign deliberate deviation.
- **World::cast_projectile (launch pitches)** — Down-arc terrain spells (Earthquake/Volcano/Crater) get a fixed downward pitch bias rather than retail's per-spell launch-pitch table (:65579-style) — deliberate approximation.
- **World::mc2_teleport_random** — The no-castle random teleport hop reuses `ent_rand` for determinism instead of retail's `9377·r+9439` LCG stream (deliberate; exact stream banked).
- **World::mc2_duel_tether_tick** — The tether grip range uses the tier's enforcement range because retail's own grip-write instruction is not isolable in the decompile (deliberate approximation; a farther grip would dissolve on the next enforcement pass anyway).
- **World::thrust_cancel (MC2 Speed)** — A braking (reverse) thrust interrupts the MC2 Speed armed window early and clears its burst timer. The literal decompile (`GetScroll_69DB0`) hard-overrides speed every tick with no brake input; interruptibility is restored from recorded gameplay over the trace (deliberate — recorded gameplay ranks above the decompile).
- **World::mc2_objective_targets** — The objective-guide overlay's "nearest target" arrow uses a torus-wrapped (shortest-way-round) distance metric, deliberately unlike the type-5 fly-to LATCH which uses retail's plain sign-extended abs. The overlay is a UI heuristic; the completion latch stays faithful.
- **World::spawn_from_thing (10,22 whirlwind)** — Disposition-spawned arm tornadoes are scaled to their tier under the same 8×charge law as the cast path, even though remc2's generate switch omits model 22 (which would leave a 500-tick roamer). Unified over the trace based on recorded retail (deliberate).
- **World::mc2_end_tick** — The MC2 ending sequence deliberately skips presentation elements: the retail moveTest terrain-block abort (the glue keeps the carpet above ground), the fov dolly-zoom (phase 5), and launch motion blur. The roll/pitch auto-level tail is handled app-side.
- **World::max_ground_tiles** — The extended-lift float-up cap anchors at the level's highest terrain tile so explicit lift can never reach a god's-eye view (deliberate gameplay ruling).


## mgc-sim — shared engine state (mc1/features.rs group)

- **features.rs::Gen::tick_building_live** — When a village building's ch1 possession is re-owned, retail also grants the claimer an immediate mana credit off the claimer's +48. The port omits that credit here; it is deferred to the mana-economy track.
- **features.rs::Gen::castle_balloons** — Balloon retargeting is re-picked every dispatch pass. Retail staggers retargeting by `castle+63 % fleet`, keeping a stale ball target between a slot's turns (an untraced nicety); the port approximates by re-picking the nearest free claimed ball every pass instead.
- **features.rs::Gen::tick** — During the load-time fixpoint pass `ctx` is `None`, so the terrain deformers (hill/digger/etc.) skip the ch0 damage broadcast and the loop-10 rumble. Retail's load pass DOES broadcast into the half-built pool, but nothing observable survives it, so the port suppresses it and only broadcasts once the world is running (ctx = Some).


## mgc-sim — MC1 combat + rivals

## combat.rs

- **combat.rs module header (`sub_12B50` mail write)** — The decompile's inverted accumulate/overwrite in `sub_12B50` is NOT ported; the direct write uses the area writers' protocol (:17301-05). Deliberate: the transcription swap is a suspect maintainer artifact, like :21814.
- **combat.rs module header (m9 ranged thunk aim)** — The m9 ranged thunk aims at the TARGET, not the decompile's `atan2(0,0)` self-aim (:21947-48). Deliberate: the self-aim is a decompile casualty.
- **combat.rs module header (aim assist metric)** — Aim assist scores candidates by angular miss (Δyaw² + Δpitch²) with a distance tiebreak. Deliberate approximation of `sub_54A90`'s squared-miss-distance metric; exact port still open.
- **combat.rs module header (m9 lightning explosion)** — The `sub_535E0` (:63272) beam is a full port; at the explosion, the +146 field stamps hit-or-0 where the original writes garbage on a miss. Deliberate (remc2's `sub_66750` guards this).
- **Gen::proj_boulder_tick** — The Troll/Ape boulder (class-9 m14, flight state 15, `sub_3A1A0` :46281) has no TRANSCRIBED handler: remc1's class-9 tick table `str_25573C` (:4838) stops at state 0x0D while its address span holds 22 entries — the only short table in the block. Reconstructed. What is CERTAIN and drove the fix: state 15 is not state 13, so it must not roll the arrow quartet (ids 33-36 = `arrow1`..`arrow4`, emitted only at :63799 inside state 13's `sub_54180`); the proof is that `sub_1AE30` writes the impact descriptor `+68=10`/`+69=0` (:22103-04) which state 13 never reads. Two deliberate departures from the best-fit orphan `sub_542B0_54640` (:63841): (1) the flight stays STRAIGHT where the orphan steers toward `+146` — homing is not adopted on an inferred handler identity; (2) the `(10,0)` impact inherits the thrown `+44 = 780` (:22112) rather than the effect's own default 400, since the transcribed 780 write would otherwise be a dead store. OPEN: retail's real table entry.
- **Gen::proj_bolt_tick (arrow reuse is FAITHFUL)** — Recorded here because it reads like a bug and must not be "fixed": retail plays the `arrow1`..`arrow4` samples for EVERY user of class-9 state 13 — the archer-type creatures m4/m9/m10 and the m15 castle guard — including m9, whose projectile wears a different billboard (sprite 203, :21957 — row 203 is sprite family base 215 against 195's base 193, same 45x60 size and 5-view fold, so it is art-only). `:63799` is the binary's sole emitter of those ids. Genuine period asset reuse; leave it.
- **Gen::spawn_bomb_fuse / Gen::bomb_fuse_tick** — Global Death's m18 fuse (state 19) sits past remc1's transcribed class-9 table. Reconstructed from observed retail behavior (never a bolt: fire once, wait, blast lands around the caster) as a caster-anchored fuse — 21 ticks tracking the caster, then a generic +44-copying detonation into the (10,55) field at the caster's position. Deliberate reconstruction; the ctor's speed/aim/+150 target and the +26 charge byte stay unmodeled. OPEN: retail may allow multiple overlapping charges each on its own delay.

## rivals.rs

- **rivals.rs module header (hate feed timing)** — The AI hate ledger is fed at damage-intake and homing-acquisition time instead of the original's per-projectile one-shot ledger scan (`sub_16540`). Deliberate: equivalent inputs, slightly later timing.
- **rivals.rs module header (creature targeting)** — Creatures still target only the human wizard rather than the full wizard list. Interim deviation; widening the mob scans is a follow-up (OPEN).
- **rivals.rs module header / World::rival_damage_intake (duel pull)** — The duel pull on the CASTER is applied through the knock channel, with magnitude from the traced formula. Deliberate approximation of retail's transport.
- **World::rival_cast_castle (upgrade token)** — The castle upgrade delivers the ch5 mail token directly (`sub_293D0` :31033-34); the cosmetic (9,10)→(10,43) ball ride is skipped. Deliberate approximation (the painter/level-up is identical).


## mgc-sim — flight & controls (lib.rs, flight.rs, mc1/mobs.rs)

- **crates/mgc-sim/src/lib.rs::ThrustModel::Enhanced** — An enhancement-class thrust model alongside the faithful MC1 mover: hold-to-fly with automatic deceleration on release, generalizing the original's own hold-to-move strafe to the forward axis. Keeps the authentic level-plane thrust rule (aim pitch never steals horizontal mobility; vertical is the altitude model's law — see the move_enhanced entry). Selected once at the sim boundary; replays record it.
- **crates/mgc-sim/src/lib.rs::AltitudeModel::ExtendedLift** — An enhancement-class altitude model. The faithful model is terrain-follow only (the carpet floats up along rising ground and settles by itself; no fly-up control exists). ExtendedLift adds explicit float up/down keys — no original equivalent — capped at the level's highest terrain tile (see `lift_ceiling`) and never bypassing wall blocking or the cave roof.
- **crates/mgc-sim/src/lib.rs::FlightInput::full_stop** — The Backspace full stop is faithful under MC2 (retail action 0x27: zero actual+target speed, kill the Speed/Accelerate channel, recenter steering). It is enhancement-class in MC1/HW, where retail's Backspace is text-entry only.
- **crates/mgc-sim/src/lib.rs::Flyer::roll (camera bank)** — The faithful movers publish the filtered roll stick at full value (retail renders `u16_327` unhalved). The enhanced mover deliberately leaves camera bank at 0: no tilt in mouse-look.
- **crates/mgc-sim/src/lib.rs::FAITHFUL_CRUISE_TPS / ACCEL** — The Enhanced deviation changes only the control response (hold-to-fly vs. accelerate-buildup), never the speed ceiling. Its DRAG-governed terminal is pinned to the faithful cruise (80 engine units/tick) rather than tuned independently, so fixed-size hazards (kraken buffet, tethers) stay proportionally correct under the deviation.
- **crates/mgc-sim/src/lib.rs::move_enhanced (level-plane thrust) + flight.rs module header** — The enhanced hold-to-fly mover (float-based, in lib.rs) obeys the level-plane thrust rule: thrust and the Accelerate override act in the yaw ground plane at full magnitude however far you aim up or down (aim pitch never steals HORIZONTAL mobility). AMENDED 2026-07-19 (player-directed): vertical motion belongs to the altitude AXIS, on both thrust models. Under Faithful altitude the enhanced mover runs the faithful vertical law — pitch-driven climb/dive with the climb-authority band (raw dive; climb scaled by authority, inverted above the ground+band soft ceiling) plus the game-keyed passive decline at any speed (MC2's always-on row-0xe buoyancy above the clearance band, MC1's speed-0 sink above the band). This closed the crosstalk where the forward-flight decline existed under classic thrust (bundled in the ported movers) but not under enhanced thrust + Faithful altitude.
- **crates/mgc-sim/src/lib.rs::move_mc1 / move_enhanced idle-settle (ExtendedLift)** — With the hover keys idle under ExtendedLift, the carpet settles toward the terrain-follow floor at the faithful 8/tick passive-sink rate (at any speed on the enhanced arm). Gameplay assumes ground-contact pickups like spell jars; holding altitude forever would overfly them.
- **crates/mgc-sim/src/lib.rs::move_mc2 (ExtendedLift MC2 arm)** — On the MC2 arm the MC1 path's idle-sink branch is deliberately absent: the faithful row-0xe buoyancy already IS the idle settle, so only the lift key is layered on. Floor is ground+256 (MC2 clearance) and the cave roof re-clamps after so the deviation can't pierce it.
- **crates/mgc-sim/src/lib.rs::lift_ceiling** — The extended-lift float-up is capped at the level's highest terrain tile plus the original's soft-ceiling band (ground+1024 = 4 tiles) so it never reaches a god's-eye view. (The faithful model can't climb past it anyway — climb authority inverts above the band.)


## mgc-sim — sim top level + small MC2 modules

- **mc2/morph.rs::Gen::mc2_summit91_tick** — the apocalypse mana-rain (10,91) skips spawning its three per-tick collectible spheres whenever the free pool has ≤ 200 slots. Retail has no such cushion and spawns unconditionally (relying on the fail-open pool exhaustion). The 200-slot cushion is a deliberate pool-exhaustion belt to avoid starving the pool during the endless apocalypse rain; the per-140-tick decay channel already bounds the rain at ~420 live spheres so the cushion rarely bites.


## mgc-sim — MC2 mobs + multipart

## mobs.rs

- **mobs.rs module header (deliberate approximations)** — The human wizard lives OUTSIDE the entity pool; wizard/class-3 scans visit it first via `MobCtx` (retail's slot-ordered list has the human in slot 1), then pool wizards. The archer arrow's impact effect (`sub_10C80`) is approximated by a channel-0 area-damage write of `f44` through the shared mailbox writer at the impact point (same observable). The arrow's hit probe (`sub_10780`) uses the shared tile-chain victim scan. `TransformEntityToManaSphere` spawns spheres through the MC1 (10,39) ball ctor and writes MC2 launch fields into the MC1 ball's field homes so the shared ball tick flies them, until MC2's own (10,39) handler is diffed. `sub_20130` (archer base+6) is missing from the decompile and stubbed as hold-state (unreachable for archers).
- **Gen::mc2_arrow_tick** — Arrow target-class filter approximation: arrows only strike WIZARDS (xtype=3). Retail keeps scanning the tile ring past a non-matching body within the same tick; the port instead lets the arrow fly on and re-probes next tick.
- **Gen::mc2_alliance_convert** — Retail also converts stage-HELD creatures (StageVar1 saved to `word_0x4A_74`, restored on expiry); the port skips creatures under a live hold or another charm.
- **Gen::mc2_alliance_clock** — On combat resolution retail returns controlled creatures to their `8m+7` controlled slot; the port's per-model state machines drop them to their wander phases 0/1 instead.
- **Gen::mc2_alliance_creature_tick** — Retail adopts the parent wizard's target/attacker words; the port's out-of-pool human keeps neither, so the observable equivalent serves: target the nearest pool entity currently targeting the parent, else the nearest enemy wizard.
- **Gen::mc2_proj_tick** — Non-(9,13) MC2 projectile states fall back to the MC1 projectile handler. With the MC2 spell column landed, live player casts stamp `F_MC2PROJ` and no longer reach this fallback; it survives only as the graceful-degradation contract for any non-(9,13) state that slips through (deliberate cross-column seam).
- **Gen::mc2_spawn_building** — VGA half-resolution footprint shrink (low-res render mode) skipped; `dword_0x10_16 = 2` has no ported consumer; the id-68 player-castle global is deferred to MC2 castles.
- **Gen::mc2_building_tick** — The one-at-a-time build carousel is skipped: all authored buildings raise concurrently at load (the retile / texture-band paint / pad-edge rings run at the retail cadence).
- **Gen::mc2_house_tick** — The `byte[2]&0x20` strong-claim lock is not modeled (all claims run the weak possess variant); the claimed sprite-row colorize rides the renderer's team tint instead of a pre-colored row band.

## multipart.rs

- **multipart.rs module header (deliberate approximations)** — `struct_byte_0xc` group markers (m27 byte[2]/byte[3], the m22 byte[2]|=0x20 sound split) are not modeled. The m27 segment show/hide writes flags bit 0 verbatim PLUS the port's 0x20 draw alias (the renderer's billboard skip) rather than widening to retail's 0x21 globally (which would break the MC2 map-only house pose and the cave balloon). `byte_0x5D_93` palette-shade is renderer-side and unmodeled.
- **Gen::m0_tick / m3_tick (m0/m3 tether)** — The `sub_1F0C0` tether is ported without its projectile lasso body: its gate byte is zero from the ctor and no recovered handler arms it (dormant in the recovered retail code), so the port omits the whole call. m0 state 0x06 / m3 state 0x1E are real table-enabled binary functions the decompiler never lifted; held inert (structural guess: the flee slot; retail-check pending).
- **Gen::m27_branch_bolt / m27_drive_branch** — `sub_2A7F0`'s low-power path perturbs the branch LCG by the global `setting_30` game-loop counter; unmodeled, so the branch stream diverges from retail after the first bolt roll. `sub_2A940`'s `x_DWORD_E9BA8` freeze gate reads as 0 (the normal path).
- **Gen::m27_tick (emerge/teleport)** — The `sub_102D0(_, _, 4)` second capability-mask probe is folded into the shared `mc2_path_blocked` (a3=1 arm) + roughness test, like the shared move core.
- **Gen::m22_tick (castle-drain 0xB2/0xB3)** — Reaches through the target player's `CastleEntityIndex_0x3A_58`. LIVE since rival castles landed: a possessor WITH a castle receives the full retail drain — the worm chases, delivers its mana to the castle, and self-consumes (vanishes; `sub_26BD0` EF:17373-17414, faithful). A castle-less possessor (typically the human) takes retail's own castle-less arm (LABEL_17 revert) and the worm survives.


## mgc-sim — MC2 rivals + doomsday

- **rivals.rs (module) — hate feed** — The rival's hate ledger is fed from the shared damage-intake path (`mc2_rival_intake`, on the mailbox) rather than retail's per-projectile scan `sub_159E0`. Same inputs, slightly earlier; this is the MC1-column position and an APPROX pending a per-projectile port.
- **rivals.rs (module) — DEFENSE disguise visual** — The DEFENSE state's disguise VISUAL (retail draws the metamorph creature in place of the AI carpet) is presentation-side and unported. The state machine, tier pick, shadowing and speed law are faithful (`sub_15FC0`/`sub_161A0`); only the on-screen model swap is missing. Open.
- **rivals.rs::mc2_rival_buffs (Heal channel)** — Rival Heal (spell 5) heals at an APPROX rate of maxLife/20 per armed tick — the MC1-column certified rate; the MC2 numeric rate is not yet pinned from the trace.
- **rivals.rs::mc2_duel_drain** — The duel life-regen drain term uses the afield /500 rate; retail reads the stored `lifeRegen_0x163_355`, which only differs while the rival sits at its own castle. APPROX.
- **rivals.rs::mc2_rival_pick_ball** — A ball owned by a NOT-hated wizard, when NO wizard exists in the world at all, is skipped (matches the retail quirk). A hated owner's balls rank from the rival's OWN castle; a castle-less rival anchors that ranking to self (retail reads the Entities[0] sentinel there — documented idealization, not the literal null read).
- **doomsday.rs (module) — state timers** — Sprites 343/344/345 auto-size their state timer to the animation length via `sub_221F0`/frame table; the sim doesn't carry TMAPS frame counts, so the seeded timers (16/32) stand. Cadence-only deviation.
- **doomsday.rs (module) — palette flashes** — `sub_5C800` palette flashes (case-7 beam flash 6) are presentation and skipped, like every flash effect.
- **doomsday.rs (module) — projectile burst acquisition** — The (9,3)/(9,26) projectile bursts (selectors 9/8) are pre-locked at the avatar via `mc2_arm_proj`; retail self-acquires on tick 1. Acquisition-timing APPROX in the proj module.
- **doomsday.rs (module) — case-0xE global reset** — Retail's case-0xE wipe writes `byte[1] |= 0x20` (an unmapped, name-inferred render bit) on every entity. The port applies the life/maxLife=140 reset and skips the render bit.
- **doomsday.rs (module) — per-list scans** — Retail's per-list scans (`dword_38531` buckets) are reproduced as pool slot-order scans over the mobs list. APPROX.
- **doomsday.rs (module + mc2_pyramid_do_summon case 7) — HURL-AWAY beam** — The case-7 hurl-away beam displaces the human via the shared knock channel (`Gen::player_knock`) rather than retail's MoveEntity + moveTest + floor clamp on the pose (the app owns the pose). Same observable: violent outward displacement, 944 units on the first push decaying to 10.
- **doomsday.rs (module) — LCG perturb** — The `rand += setting_30` LCG perturb after the two summon-pick rolls is unmodeled (project-wide convention).
- **doomsday.rs (module) — word_0x36548** — The doomsday-active global `word_0x36548` (set case 0, cleared case 0xF) has no reader in retail (savegame/debug only) and is not carried.
- **doomsday.rs::mc2_doomsday_tick (render-arm proximity analog)** — The wind-down escape bit is armed by retail's DETAILED-pass render writer (`subSpellIndex |= 0x40`), which a headless sim can't couple to. Reproduced as a deterministic proximity analog: any radius >= the machine's own 0xA00 far-gate is behaviorally identical (far ticks just re-clear the bit), so the far-gate distance is used.


## mgc-sim — MC2 cast

- **mc2/cast.rs::World::mc2_aim_preview** — Retail MC2 draws NO crosshair/reticle; the aim feedback is the sprite-42 projectile visibly curving toward its target (docs/traces/mc2-autoaim.md §4, mc2-mouse-aim.md §4). This method is an opt-in (Preference-class) predictor that returns the target the hand's spell projectile would acquire on its first flight tick — a pure twin of the real aim scan. It is an enhancement, not a faithful surface; keep it gated as an instrument rather than "fixing" MC2 to always show a reticle.


## mgc-sim — MC2 roster + tail

- **mc2/effects.rs::mc2_mine_detonate (blast box, bolt, spent-mine teardown)** — Retail's post-trigger path is the untraced `sub_6DCA0` relaunch (`spell-audit/magic-mine.md` §6 Q2), and in retail the player could not get a mine to trigger AT ALL (§6 Q1: nothing is known to write the `word_0x36_54` armed gate). Three deliberate choices on the player's ruling, all "better than retail" rather than faithful-to-a-dead-spell: (a) the detonation opens a 1024 (4-tile) blast box for its `area_write` — the ctor sets no extents, so `ent_overlap` summed to a POINT and a wizard beside the mine took nothing; the box is restored afterwards so it does not linger through the sink; (b) the mine SPITS a (9,0) bolt at whatever tripped it, since §5 step 4 describes the detonation as a spell *relaunch*, not a bare area write; (c) the spent mine is handed to retail's OWN expiry teardown (`f71 = 6` → 7 → 9: hang, sink, puff, despawn) instead of vanishing on the spot.
- **mc2/roster.rs::module (+6 states)** — Every `+6` creature state whose body is missing from the decompile (m2, m9, m16, m17, m18, m19, m20 nominal, m21, m23, m25, m26, m28) holds inert rather than dispatching (which would crash in remc2). Retail can never reach these states (the rows' flee bit is clear).
- **mc2/roster.rs::module (m18 sub_253B0)** — m18's `sub_253B0` duration table is only partially pinned: the trace lists the formulas but not the (state,sub)→formula map, so the mapping is approximated.
- **mc2/roster.rs::module (m26 human drain)** — m26's human mana drain uses a flat `+14` because the human's manaRegen isn't modeled yet.
- **mc2/roster.rs::module (m12 site scans)** — m12's site-jitter / footprint-clear scans (EF:13991-14093) are shaped, not verbatim, because the overlap helpers are untraced.
- **mc2/roster.rs::m2_tick (vertical homing)** — m2 vertical homing toward the target's top (:11509-20) uses the human's carpet z as its top because the human's half-height isn't modeled.
- **mc2/roster.rs::m9_tick (grounded consume)** — the m9 grounded-variant consume sweep reuses the shared seek path as its mirror rather than a separate scan.
- **mc2/roster.rs::m20_tick (melee rush)** — retail also gates the human melee rush on the mobilize counter (MC2 flight state); that gate is not modeled, so our m20 commits without it.
- **mc2/roster.rs::m23 descend** — "aligned over node" is approximated as a 2-D closeness test standing in for retail's sub_28390/sub_28060 alignment.
- **mc2/roster.rs::m24 castle approach** — "in range" of the castle is approximated as an AABB box overlap standing in for retail's CompareAxisWithShift_10750.
- **mc2/roster.rs::m28 strike** — the strike animation length is fixed at 16; retail's count comes from the anim bank, which isn't modeled.
- **mc2/tail.rs::module (spellbook hit counts)** — the `sub_6D8B0(id, kind, hits)` spellbook reports ((10,17) kind 9, (10,23) kind 7, (10,15)'s spray kind) compute their hit counts and drop them; the spell-XP intake is emitted by the spell-XP column instead.
- **mc2/tail.rs::module / mc2_fire_spray_tick ((10,19) singleton latch)** — the (10,19) spray's `word_0x33` singleton latch (EF:23962 registers a new spray and disables the previous from a different action's context) has no ported writer, so the on-death release write is a no-op. (OPEN.)
- **mc2/tail.rs::module (AddEvent2 children)** — `AddEvent2_847D0` attached lights/children (e.g. (10,23)'s (128,9,0), the fire-orb satellites' (128,1,0)) are presentation only and left unported.
- **mc2/tail.rs::module / mc2_aura_tick ((10,54) creature list)** — the (10,54) aura scans a pool slot-order list over mobs.rs standing in for retail's `dword_38523` creature list.
- **mc2/tail.rs::mc2_whirlwind_lift (player sway)** — the tornado's pull on the human is approximated: the full grab/lift/camera-roll takeover (the deferred FlightVerb seam) is not ported, so the "funnel drags you in" observable rides the `player_knock` channel as a ~45°-tangential spiral toward the eye. Also: the HUMAN player arm (yaw-step 56, threshold 384, camera roll, actSpeed 80) awaits the FlightVerb takeover seam, so until then the player is damaged when overlapping the eye ring but not lifted; and the victim z-float band (`sub_580E0` row args) collapses to the computed lift z (the row hover clamp needs the behavior rows' word_0xa/0xc homes).
- **mc2/tail.rs::mc2_aura_tick (magnet-path reuse)** — the (10,54) mana-magnet reuses the established magnet path (ball_tick dest drift + merge) rather than retail's homing triplet (target stamp + pull-speed word); the triplet collapses to the same observable motion.
- **mc2/tail.rs::mc2_whirlwind_lift (reap-skip guard)** — a 0x400 reaped-slot skip is added to the victim scan that retail's gate does not have, to avoid acting on reaped slots.


## mgc-sim — MC2 castle + projectiles

- **mc2/castle.rs::module (sub_5F890)** — The Create-Castle HUD build-ghost/widget sync calls (`sub_5F890`, EF:61029) are no-ops: no ported HUD widget exists.
- **mc2/castle.rs::module (owner palette shift)** — The castle's owner recolor (`word_0x5A_90 += TransformPlayerColorIndex`, EF:61139) is approximated by riding the renderer's team tint rather than a palette-index shift.
- **mc2/castle.rs::mc2_castle_roster (slot arrays)** — The balloon/guard slot arrays (`array_0x3C_60`/`array_0x5C_92`) are scan-collected each roster pass rather than kept as retail's per-slot index arrays. Same membership, no per-slot indices.
- **mc2/castle.rs::mc2_castle_downgrade (10% haircut)** — The 10% capacity haircut is computed in i64. Retail's i32 `10 * x / 100` overflows at the always-overflowing level-7 rung (10 × 300M) into a NEGATIVE cut, so a maxed level-7 castle downgrade *raises* its cap and scatters nothing; we keep the sane 10% instead.
- **mc2/castle.rs::mc2_castle_destroy (eject ordering)** — The downgrade death arm front-loads the balloon→sphere conversion where retail leaves it to the roster call, so balloon-spheres draw before the bank-spheres. Same total mana, different LCG interleave.
- **mc2/castle.rs::mc2_castle_intake / mc2_balloon_tick (HUD alert)** — The "castle under attack" / balloon-under-attack HUD flags (retail per-owner `byte_0x195_405` / `byte_0x197_407`, EF:61752/61947) are a single player-side latch; per-owner records await a rival defense-AI consumer.
- **mc2/castle.rs::mc2_piece_scan (turret target order)** — Retail walks the per-ring cell-offset tables nearest-ring-first and takes the FIRST hostile in walk order; we take the nearest by ring then pool order (same admission set, same 3-tile hole). The class-5 `StageVar2==14` own-parent exemption (EF:30378-81) is skipped (the stage binding lives in side-vecs).
- **mc2/castle.rs::mc2_research_stamp / mc2_castle_part_type** — Castle turret research (part-type + HP factor) is stamped at cast/upgrade time from the castle-spell tier rather than driven by retail's `sub_69AB0` research/production child. The HP factor is recorded but the ladder still runs identity.
- **mc2/proj.rs::mc2_proj_impact (unported effect)** — An impact whose (f68, f69) effect is unported applies its f44 as channel-0 area damage at the impact point (the effect IS the damage carrier in retail) and counts the misfit; damage lands, the visual gap stays in the ledger.
- **mc2/proj.rs::mc2_flyer_tick (no-target snapshot)** — A target-less flyer snapshots its aim once (the retail else-arm, EF:62914-16) and flies straight; the model-keyed acquisition sweep (`sub_67CB0`) serves player-cast spells only.
- **mc2/proj.rs::mc2_rebound_deflect** — The human deflector's mana debit is skipped (the wizard ledger is world-side); the returned bolt's xtype/xsubtype re-key (EF:55299) is ported for the human shooter only (a pool shooter id has no O(1) slot resolve). Pool victims deflect on the authored 0x8000 shield bit and always scatter — rival Rebound windows are not yet mirrored onto their entities.
- **mc2/proj.rs::mc2_proj_impact (possession magnet)** — The possession mana-magnet aura manifests only when the bolt actually claims a mana sphere; building/worm possession never magnets. Gated to spheres pending a retail trace (retail's magnet rides the claimed ball).
- **mc2/proj.rs::mc2_aim_scan (acquisition approximations)** — The owner lock range rides the shared wizard-row v_28 (4096); bucket 22 (worm family) is approximated as model-22 heads + their f54 chains; the cave-in on-ground filter is z within one step of terrain. The offensive branch's EF:54788 self-self distance is a decompile artifact — the correct two-point form is used instead.


## mgc-sim — MC2 stagevars / flood / terrain paint

- **stagevars.rs::set_mc2_stagevars (0xFF fill rows)** — retail's slot-count scan would include a 0xFF editor-fill tail and load it with a garbage out-of-table subtype read; the port treats the 0xFF fill as an empty slot. No shipped level can deliberately bind through the garbage read.
- **stagevars.rs module header — APPROX register (held reductions)** — the per-model phase-7 wrapper EXTRAS around retail's `sub_1D5D0` (ambient-sound draws + speed refresh, e.g. the goat's `AddGoat05_01_1F5B0` bleat, m18's ground re-snap) and the `sub_1EEE0` settle on the walk leg's hit path are not run — no idle SOUND rng is drawn while held.
- **stagevars.rs::mc2_stagevar_tick (deferred m9 arm)** — retail arms a deferred m9 (hive-imp) hold inside the materialize-completion tick (EF:11984-95); the port's pre-loop pass arms it one boundary later. Same observable sequence; no shipped level authors a held m9.
- **stagevars.rs::mc2_stagevar_tick (kind-9 proximity fallback)** — retail's kind-9 "proximity fallback" (EF:5108-12) reads pointer bytes written into the coord union, so the branch is unreachable garbage; the port does not reproduce it. All 3 shipped kind-9 levels release by death-watch.
- **stagevars.rs::mc2_held_hit** — retail follows the hold-break-to-aggro with a `sub_1EEE0` ground settle; the port skips it.
- **stagevars.rs::mc2_held_tick (predator/guardian speed tails)** — only the FLEE-prey (goat/townie) +7 wrapper speed tail is ported; predator/guardian wrappers (m18/m19/m21…) keep their spawn speed while held, their per-model tails and sound rolls skipped.
- **flood.rs module header — spell-XP / objects counter** — `sub_6D8B0(id,0x14,n)` spell-XP reports (EF:29367/:29436) have their counts computed and dropped except the player push; the global objects-hit counter `x_DWORD_E9B90` (EF:28527) has no ported reader.
- **flood.rs::flood_shove (human player arm)** — the player lives outside the entity pool, so retail's class-3 model-0 shove is approximated: the horizontal pull rides the `player_knock` channel, the z pull-down and pitch-512 spin bank on the FlightVerb takeover seam, and the close-range 1-in-7 kill mails a kill-scale 32000 (retail adds the victim's `life+1`, unreadable for the player).
- **flood.rs::flood_shove_hit (rival-wizard spin)** — the class-3 model-0 pitch-512 spin (body-flip presentation) is skipped; the damage roll is faithful.
- **flood.rs::flood_shove (action-74 visibility juggle)** — retail's local-player draw-latch juggle (EF:29118-29127, byte[0] bit0 set for the local wizard, cleared for everyone else) is collapsed: the port clears `F_TOSSED` for everyone (single-player observable: victims become shoveable again).
- **flood.rs::flood_shove (deep-sink skip)** — retail's deep-sink skip (`word_160_0xe_14 < -64`, the victim's z-velocity, EF:29106) has no ported home; the z pull always applies before the ground clamp.
- **flood.rs module header — mana field** — `mana_0x90_144 = 0` in phase 1 (EF:28548) has no ported reader on this column and is skipped.
- **terrain_paint.rs::mc2_smooth_pad_edge** — the packed-word neighbour index is reproduced with wrapping u16 (torus) math like the rest of the port. Retail's gate reads signed-negative offsets near row 0 (remc2's fix pins those to 0/natural); the torus wrap reads the far edge instead. Divergence only for footprints touching row 0.


## mgc-app — main.rs

- **map edge-scroll (the whole letterbox bar scrolls)** — PLAYER-RULED. Retail scrolls the map when the confined cursor sits on the exact screen-edge pixel (MI:3132-75). Letterboxed, that edge is the PICTURE's, and the pointer can rest anywhere in the black bar beyond it — an edge-only trigger would leave a dead strip where the cursor is off the map and nothing happens. The test reads "at or beyond the edge", which takes in the whole bar on whichever axis is barred. Retail had no bars, so there is nothing to be faithful to.
- **App::new (no menu music under a launch intro)** — The campaign frontend normally starts its menu track (`csetup` / the MC2 SETUP render) at boot. When the intro chain is about to play, it does not: the movie owns the audio from its first frame, and starting the menu track only to stop it a frame later was audible as a blip of menu MIDI under the opening (player-reported). The chain hands back to `enter_main_menu`, which starts it properly.
- **main.rs::WorldInit::prune_owned_jars** — removes spell jars the local player already owns at every level load (the sim self-culls owned jars on their next tick). Preference-class improvement over retail, applies to both MC1 and MC2.
- **main.rs::load_level `--pool-slots` (chassis.pool_slots)** — dev flag bumps the entity pool beyond the game's pristine profile; limit-removing override, G-class, a run under a bumped pool is not a faithful fixture.
- **main.rs::load_level `--awake-range` (chassis.awake_gate_sq)** — dev flag overrides the creature awake gate (faithful = 24 tiles; 0 = always awake); G-class, not a faithful run.
- **main.rs::WorldInit::placeholders** — draws stand-in art for unported MC2 models (default on MC2 until its roster closes).
- **main.rs::App::quit_fade (end-of-game ending)** — on level WON the app fades to black and exits with no stats screen and no menu return; deliberate simplified ending (both games leave through the same door; campaign mode routes to the next level instead of exiting).
- **main.rs::App::tick_input (fly_assistant)** — the virtual-stick recenter (retail MC2 "fly assistant") is a Preference option defaulting OFF; MC1/HW never had it (enhancement-class there), and a parked cursor's deflection persists as retail MC1's visible-cursor scheme did.
- **main.rs::App::exit_confirm** — the retail MC2 "Abandon level?" OK/Cancel dialog is also shown for MC1 and single-level play, which retail left unguarded; deliberate extension so an accidental Esc cannot discard progress. Modality stays retail-faithful (world keeps running beneath the dialog).
- **main.rs::App RedrawRequested (view_pitch/view_roll)** — the enhanced (mouse-look) flight camera renders flat: no horizon bank from the roll stick, unlike the faithful camera which renders the full filtered roll (remc1 :52432). Deliberate for the mouse-look control model.


## mgc-app — ui.rs + config.rs

- **crates/mgc-app/src/ui.rs::UiAssets::hud_notification_anchor** — anchors the top-of-screen notification toast to the LIVE HSPR sprite geometry (below the info-boxes, right of the radar cap) instead of retail's 320-native `132,50` literal, which was authored against the half-size MSPR strip and doesn't map onto the larger HSPR panels; keeps the toast placed correctly at any resolution.
- **crates/mgc-app/src/ui.rs::book_quads (quick-select badge)** — the number badge on a hotkeyed spell's book cell is kept ALWAYS-ON; retail gates it on a per-spell countdown (+844, decremented per draw so the badge flashes after assignment) or a book-wide flag (+14421). Chosen as the more readable interpretation.
- **crates/mgc-app/src/ui.rs::hud_quads (win_tick)** — the GREEN "completed" recolour of the level-goal ticks on the mana ruler is our addition; retail has no completion recolour there (its ticks only alternate the two team-ramp entries per blink frame).
- **crates/mgc-app/src/ui.rs::exit_confirm_quads** — presentational additions to the in-level abandon dialog: a mild hover tint on the OK/Cancel buttons and a soft slab behind the prompt text for readability over bright terrain. Retail's only feedback was the cursor itself, and its palette font carried its own contrast.
- **crates/mgc-app/src/ui.rs::vitals_quads (spawn-grace shimmer)** — a thin white bottom-center strip that drains with the respawn-invulnerability window; retail shows NOTHING for grace. Behind `render.debug.grace_meter` (a debug cue, default off).
- **crates/mgc-app/src/config.rs::GameSpeed::Slow** — a half-speed (0.5×) game-speed level with no retail equivalent (retail's slowest is Normal); for sightseeing/accessibility. F3 cycles it in alongside retail's three.
- **crates/mgc-app/src/config.rs::RenderPreference::fog_distance** — default is 50 tiles, not retail's 20; retail's 20 was a pure period-performance choice. Menu stops offer 20 (faithful) / 50 (default) / 100 / 255.
- **crates/mgc-app/src/config.rs::RenderEnhancement::smooth_motion** — defaults ON: entities render interpolated between the last two sim ticks (frame-smooth at any fps). Retail steps at tick rate. Presentation only; toggle off for the tick-stepped retail look.
- **crates/mgc-app/src/config.rs::RenderEnhancement::smooth_shading** — defaults ON (player ask): terrain shade interpolated across tile centers instead of retail's one shade level per tile. Preference-class (retail MC2 itself ships a Shift+F7 flat-shading toggle); T / the menu restores the faceted retail look.
- **crates/mgc-app/src/config.rs::RenderEnhancement::hud_transparency** — default OFF (opaque HUD) for readability, especially the radar; MC1 always draws the HUD translucent and MC2 adds the opaque toggle. Faithfulness is deliberately not scored for this preference.
- **crates/mgc-app/src/config.rs::RenderEnhancement::map_owned_buildings** — opt-in that brings MC2's owned/possessed-dwelling map markers (owner's colour) to MC1, which never marks houses. Default off.
- **crates/mgc-app/src/config.rs::GameplayEnhancement::prune_owned_jars** — defaults ON: removes any spell jar whose spell the local player already owns (and can never pick up). Retail (both MC1 and MC2) leaves such jars in the world forever. Sweeps at level load and the instant the player gains a spell; disable with `--no-prune-owned-jars`.


## mgc-app — frontend/map/settings + mgc-render notes

- **entities.rs::map_dots_from_poses (MC1 villager green)** — Retail's minimap RGB->palette LUT[16] villager green decodes to (r0,g1,b0), a green so dark its nearest-palette match lands on black. The overhead map is gameplay-critical, so the port aims at a legible mid-green instead of the literal cube colour. Deliberate map-marker-legibility deviation.
- **entities.rs::mc2_map_dots / map_dots_from_poses (MC2 minimap)** — MC2's minimap COLOURS (DrawMinimapEntities_B_61A00, team pairs, blink phases) are drawn over an MC1-shaped rotating-radar projection; the projection itself stays MC1's, only the colours follow MC2. Deliberate approximation. (Also banked interim: class-3 castle/balloon, class-11 X-markers and class-10 flag families draw as 2x2 dots until the MSPRD stamp bank is baked; castle rope-line and Beyond-Sight enemy reveal wait for MC2 castles/rivals.)
- **entities.rs::map_dots_from_poses (map_owned_buildings enhancement, config-gated, default OFF)** — Owned/possessed dwellings draw a 2x2 grown dot in the owner colour instead of retail MC1's barely-distinct 1px; MC2's map behaviour brought to MC1 as an opt-in. Deliberate Enhancement-class deviation.
- **worldmap.rs::WorldMap::click (travel sample timing)** — Retail gates the carpet travel sample on leg length and starts it late (MI:3786); the port plays it immediately with the click (still only when the flyer actually moves). Deliberate.
- **settings.rs::registry (render.enhancement.smooth_motion)** — Faithful value is OFF (retail steps everything at sim rate); the option ships default-ON as a deliberate default deviation (Preference-class, so it does not flag the run).
- **settings.rs::registry (gameplay.enhancement.prune_owned_jars)** — Faithful value is OFF (retail leaves owned spell jars in the world forever); ships default-ON as a deliberate default deviation (Preference-class cleanup, does not flag the run).
- **frontend_mc1.rs::Mc1Menu.player_name / EditName modal** — MC1 rename dialog pre-fills the current save name (edit rather than retype), and retail's two prompts ("Enter your name" + "Enter your call-name", etext 34/35, only the call-name used) are collapsed to ONE name field. Deliberate, matching MC2's name dialog.
- **frontend.rs::MainMenu::escape (MC2 menu)** — Retail's Esc auto-selects the Exit button (MI:5842-43); the port makes Esc close the modal only and never quit, because Esc doubles as the in-play pointer-release/abandon key. Deliberate. Quitting is the Exit button's job.


## mgc-render

- **ui.wgsl::vs_main (pixel snap)** — Quad corners are ROUNDED to the pixel grid. The UI is authored at a fixed resolution and scaled by an arbitrary factor, so edges land between pixels and adjacent sprites (a scroll's pieces, a panel behind a button) leave a hairline gap or overlap. Rounding the corners rather than the origin keeps neighbours welded: two quads sharing an edge round it identically. Retail scaled nothing, so there is no retail behaviour to be faithful to.
- **ui.wgsl::fs_main (half-texel clamp)** — Sampling is clamped to the sprite's own atlas cell. `sprites::pack` leaves NO gutter between packed sprites, so a fragment reaching the far edge samples the NEIGHBOURING sprite — one stray row or column of foreign pixels along the edge (player-reported, with screenshots). Half a texel in from each side keeps every sample inside the cell without shifting the interior.
- **anti-aliasing (`render.preference.anti_aliasing`, default OFF)** — Off / MSAA / supersample 1.5x / supersample 2x. No retail analogue — DOS drew one 320x200 buffer and filtered nothing — so it is a display knob like vsync, fidelity-free either way. Off is the exact previous path: no offscreen buffer, no resolve pass, single-sampled pipelines. The two techniques are NOT equivalent here: creature and building silhouettes come from discarding palette index 0 (hard 1-bit transparency), and a discard kills every sample of its fragment, so MSAA cannot soften them — it reaches true geometry only, chiefly terrain against sky. Supersampling re-evaluates the discard at higher resolution and is the only mode that smooths sprites. MSAA is baked into all nine pipelines at construction, hence startup-only; the supersample factor is live. Player-measured on integrated graphics: off 175 fps, 1.5x 130, 2x 95, 3x 50 — 3x was dropped as both too costly and visibly destructive to thin HUD marks.

- **crates/mgc-render/src/lib.rs::BOOK_MAP_ZOOM** — The MC1 book-screen map spans the FULL 256-tile world instead of retail's ~251-tile zoom (retail passes 382/378/a8=170, leaving its edges clipped — the "questionable things at the edges"). Deliberate: span the whole world so nothing is cut; toroidal wrap makes it appear infinite. The original's rounding-error void-mobs that live at that wrap are not reproduced.
- **crates/mgc-render/src/lib.rs::MC2_MAP_VIEW_SPAN_TILES / map_pane_zoom** — The MC2 map-screen pane is zoom-faithful (318.75 tiles vertically = retail `DrawMinimap_63600` scaling 204 units/px over the 400px pane, EF:21840-49), but retail blits the TERRAIN as a square 318.75-tile region squished into the 382-wide strip while its ENTITY layer runs isotropic at 204 units/px — a ~4.6% horizontal terrain-vs-entity misalignment baked into retail. Deliberately NOT reproduced: both our layers use the isotropic entity geometry (304.4 tiles across the native width), so map stamps sit exactly on their terrain. At non-4:3 windows the vertical span stays anchored at 318.75 and only the horizontal wrap widens.
- **crates/mgc-render/src/lib.rs::render (mirror pass, mirrored sprites)** — The water-reflection pass mirrors OPAQUE billboards (world sprites) into the water. Retail (remc2 GameRenderOriginal reflects terrain only) never reflects sprites. Deliberate presentation choice: a monster over water should show in the water. Translucent sprites are excluded (not worth a sorted blend pass).


## mgc-app — movie.rs (FMV player)

- **RATE_SCALE (playback 25% slower than authored)** — PLAYER-RULED. The scripts cue a narration clip and then load the next sample bank a fixed number of frames later, and a bank load stops every voice, so a clip that outlasts its scene is cut mid-sentence. At the authored rate MC1's intro clips the last book page's line by about a second — a couple of words. Retail does the same (the timing is simply tight); stretching every delay by a quarter gives the voice room and leaves the relative pacing untouched. The factor is MEASURED against the real clip lengths (`narration_clips_fit_before_their_bank_is_swapped`): the binding constraint is `voc11`, 6.13 s of speech in a 5.11 s scene, which needs 1.20; 1.25 is that plus headroom.
- **SCRIPT_LEAD (script fires one frame early)** — PLAYER-RULED. Retail's order is events(N) → draw(N) → wait, which the port implements; played back, the long scene holds still landed a frame or two into the NEXT page-flip instead of on the settled page. Leading by one frame parks the hold where the animation rests. The transcription itself is unchanged.
- **MoviePlayer::next_stream / ::skip (a movie's score ends with it)** — PLAYER-RULED. Retail carries the music across movie boundaries: `M`/`Z` start a track and nothing stops it, so MC1's intro theme plays on under the flaming title, which has no music cue of its own to replace it. Reported on both paths (the skip first, then the natural transition, which the skip fix had masked). The track belongs to the movie and now ends with it, however it ended. The sample half of the same teardown IS faithful — retail's per-movie bank reload stops every voice.
- **script::SCRIPTS (`levelw2` gets its own table)** — Retail points BOTH win movies at one event script (`dword_4A5D8_4A918`), so `levelw2` plays scored with bank 6's `win1` at frame 200. An unreferenced table at 0x4A5FC is byte-identical but for bank 7 (`win2`) and cue frame 180 — plainly `levelw2`'s own script, orphaned by the shared pointer, and bank 7 exists on the CD holding exactly `win2`. The port gives each movie its own table: a deliberate bug fix, not a transcription.
- **MoviePlayer::frame (centred letterbox)** — Retail ran one 320x200 mode and never letterboxed. Non-4:3 windows centre the picture with black bars; retail has no case to be faithful to.
- **MoviePlayer::tick (wall-clock pacing)** — Retail busy-waits on the shared 120 Hz counter and zeroes it per frame. We accumulate delta time at the same rate and DROP frames under stall rather than catching up, so a slow host loses frames instead of seconds.
- **script (per-movie delay reset)** — Retail's `dword_9ADC4` is a process global that persists across movies; we reset to the default per movie. Every transcribed table opens with an `'A'` record, so the two are equivalent — this just cannot drift.


## mgc-audio

- **lib.rs::Audio::play_movie_sample (movie cues bypass the mixer)** — FMV sample cues go straight to output voices instead of through `FaithfulMixer`. The mixer is a ported 3-D ruleset with per-id request slots and a listener; a movie has no world and no listener, and retail plays these onto voices directly too. No session is alive during a movie, so the channels are free.
- **movie.rs script (`'H'`/`'O'`/`'P'` volume operands flattened)** — MC2 starts two intro ambiences looping at volume 0 and raises them to 127/80 with a paired key; the port starts them at full, so they arrive without their fade-in and one is louder than retail.
- **mixer.rs::FaithfulMixer::tick (MC2 pitch jitter)** — Deliberate approximation: the emitter-action +10..+30 pitch variant of the gloop (sound id 46) is unmodeled; only the base ±10% (id 46) / ±15% (devil calls 42-44) per-play jitter is applied (Sound.cpp:6331-45). The jitter runs on the mixer's own LCG and audio is outside the sim hash, so this never affects determinism.
- **output.rs::lerp_i16 (resampling)** — Enhancement over retail: music/speech/SFX are resampled to the device rate by linear interpolation rather than nearest-frame (zero-order hold). Retail's nearest-neighbor read stair-steps the waveform into a "grainy, low-bit-depth" buzz; interpolation removes it.
- **output.rs::Channel::release (SFX declick)** — Enhancement over retail's hard cut: a `Stop` ramps the voice out over ~2.5 ms instead of clearing `pcm` in one sample. A hard mid-waveform cut is an audible click, and the meteor fire trail restarts the same voice ~24×/s so the clicks stack into a crackle. The ~2.5 ms fade removes them without altering the (faithful) retrigger cadence.
- **output.rs::MusicState::release (music declick)** — Same enhancement class as the SFX declick, applied to StopMusic and track replacement: ramp out over ~2.5 ms rather than a hard `pcm = None` cut (which is an audible click/thump).
- **output.rs::Renderer::render (suspend edge ease)** — Enhancement over retail's instant mute: on game pause the whole output eases toward mute over ~2.5 ms (`suspend_gain`) and back on resume, rather than stepping mid-waveform. Playback positions drift a few ms during the ramp, then hold.
- **lib.rs::Audio::tick (voiceover duck recovery)** — Deliberate approximation of retail's 120 Hz FadeUpSoundVolume: after a voice line ends, music+sfx ramp from 1/3 back to full over ~0.7 s of sim ticks. The exact per-callback step is treated as a volume-scale detail; the span is calibrated to be tick-rate-independent.


## mgc-import

- **adlib.rs::render (OPL3 song renderer)** — Note velocity is ignored entirely. The original HMI AdLib driver's velocity handling is unknown, and retail songs carry velocities 1..9 on busy melodic channels in every arrangement, which only renders sensibly if the driver used raw patch levels; ignoring velocity reproduces that. CC7 volume is honored only at note-on, not re-applied on sounding notes as live ramps (retail songs only set CC7 up front). Fidelity pass still owed — playtest is the oracle.

- **cdtracks.rs (module) — frames→ms conversion** — The port applies retail's frames→ms conversion (`× 13.33333333333` truncated) uniformly to ALL rows, including the secret-level rows 25/26. Retail's secret-level path skips the conversion, a latent bug that would cut those clips 13× short (trace §1b); the port fixes it rather than reproducing the bug.

- **xmi.rs::parse_evnt — AIL FOR-loops (cc116/117)** — MC2 wraps every sub-song in one infinite whole-song FOR-loop; the parser drops the cc116/117 pair and instead loops the baked FLAC. A cc116 FOR-loop that starts mid-song (a real interior section repeat) is FLATTENED — played through once rather than repeated. Two bank-0 songs contain one (C2GAME3 tick 1, C2INTRO tick 11876). The flattened render is emitted with an audible note.
