# Fix plan for the 2026-07-15 pedantic review

Companion worklist to docs/REVIEW-MC2-PORT-2026-07-15.md (the findings ledger —
item IDs like P0-2 / P1-14 refer to it; that file carries the failure scenarios
and EF: citations, this one tracks execution). Check items off here; the
ledger stays immutable.

Conventions per session: land fixes + a regression test per behavior change;
any golden movement gets a dated in-file justification comment; MC1 goldens
must not move unless the item explicitly says so; rerun `mc2census`/`mc2sweep`
after registry-adjacent changes.

---

## Session A — build green + P0 core breakers — ✅ LANDED 2026-07-15

- [x] A1 (P0-1) mgc-audio unit tests fixed (tag field dropped; the
      loudest-wins assert now pins volume, not the dead tag field).
- [x] A2 fmt clean, clippy 0 warnings (incl. the assert_eq!-bool NIT and the
      hwcensus doc-indent), full workspace suite green.
- [x] A3 (P0-2) `mc2_rival_buffs` now counts EVERY owned manifestation's f26
      down per tick (heal keeps its pre-decrement read). Test:
      `mc2_rival_armed_window_expires_for_homing_spells`.
- [x] A4 (P0-3) `recompute_mana` resets + credits `mc2_rivals[*].mana_max`
      alongside the MC1 vec. Test: `mc2_rival_mana_ceiling_grows_with_claims`.
- [x] A5 (P0-4) Both doomsday arms restricted to the sphere family
      (10, 39/40/57); re-verified against EF:12847-54 + EF:13048-66 in-session
      (KillAllCreatures_1B5F0's class-5 walk confirmed faithful as-is). Test:
      `mc2_doomsday_checkpoint_spares_the_world`.
- [x] A6 (P0-5) FULL MC2 per-id policy table ported from
      `PrepareEventSound_6E450` (Sound.cpp:6347-6536, EntitySounds_F4FE0[70]):
      SLOT_COUNT 47→70, `policy_mc1`/`policy_mc2` split selected via
      `Audio::set_mc2_danger_ramp` (app already gates it on is_mc2). MC2 ids
      < 47 now run the MC2 law too. D1 residue: playType-4 flush semantics
      (47/49, keep-running stand-in), Select/CantUse/Hit level-index gating,
      ShouldUpdateSound vs the −8 tolerance, MC2's vol/pan-law delta.

Goldens: NO re-pins needed — all fixtures pass unchanged (verified genuinely
running, not skipped). Consistent with the audit: the fixed paths (rival
casting, doomsday endgame, MC2 audio) are exactly what the fixtures never
exercised. Playtest owed: rival economy/casting feel, doomsday endgame, MC2
sounds ≥ 47 ear-check.

## Session B — rival brain faithful rework — ✅ LANDED 2026-07-15
(4 parallel decompile agents + in-session re-verification; all in
mc2/rivals.rs unless noted)

- [x] B1 (P1-6) Defense = metamorph MIMICRY (sub_15FC0/sub_161A0 traced in
      full): `unk_D3F91x` {2,0x13,0x19,0x10} = the DISGUISE-MODEL table
      (tier 0/0/1/2); pick = nearest wizard ≤ 0x1400 → nearest table-model
      creature ≤ 0x1400 OF THAT WIZARD → pre-arm tier, target the CREATURE;
      handler = refresh cast (300 cooldown), double z-step shadow at
      anchor+512, tier-0 LCG wiggle (2 draws) + 3×minSpeed, mid-band
      (0xA00..0x1400) engage flips target to the wizard. Disguise VISUAL =
      flagged APPROX (presentation). Queued SetSpell tier (f44) now drains at
      window expiry. Test: `mc2_rival_defense_picks_the_disguise_tier`.
- [x] B2 (P1-7) RaidCastle: steal DELETED; radii 2048/3584; casts on cadence
      only INSIDE the ring; hover-on-whiff at castle z+512. Possess: radii
      1024/3072; tier-walk cast; hover-on-whiff. TRACE CORRECTION to the
      ledger: the `< 0x1C` claim write IS on the rival's own state-6 handler
      (EF:5849-50 — the only such site in the binary), writing
      `ball.playerEntityIndex = rival id` on a SUCCESSFUL cast; the ball-side
      projectile-stamp path coexists. Strict `<` kept. Test:
      `mc2_rival_raid_never_steals_the_castle`.
- [x] B3 (P1-8) save-up hold deleted; `mc2_rival_tier_probe` (sub_15F20,
      SetSpell-even-on-failure side effect + 0/-1/spell returns) +
      `mc2_rival_walk_cast` wired into attack/heal/reactive/home/upgrade/
      build/possess picks. Fireball's 0-collision quirk (gate-refusal == the
      spell id 0 → pick returns it, executor whiffs) reproduced by
      construction. Test: `mc2_rival_attack_pick_tiers_down_instead_of_waiting`.
- [x] B4 (P2) war de-latch on a landed cast vs ANY wizard target
      (EF:5966-68). Test: `mc2_rival_war_delatches_on_landed_cast_vs_rival`.
- [x] B5 (P2) weave = retail FSM: wizard targets only; tick 0 = LCG dir roll
      + ACTUAL-yaw ±512; ticks 1-2 = setpoint jink + actual-speed pulse
      3·minSpeed·Reflexes/255; 3..19 coast; 20 restart. Strafe weave deleted.
- [x] B6 (P2) cruise casts: Perception-rolled Fool's-Mana 22 (not over own
      castle; retail's verbatim SpellIndex[2]-tier quirk replicated) + the
      speed-up top-up gated only on the live window.
- [x] B7 (P2) poverty release: `v = max/4+6000; if v >= max { v = max/2 }`,
      release at `mana >= v` (decompile re-read in-session; the ledger's
      reading confirmed, agent 3's min() reading wrong).
- [x] B8 (P2) ball picker = full sub_148E0: {39,40} pass then {57} pass,
      57-BREAK on the Perception roll (keeps best); isolation 5120 measured
      from the BALL (+ the no-wizard-in-world skip quirk); at-castle bbox
      skip vs nearest non-own castle; hated ranking from OWN castle
      (castle-less → self, documented idealization of retail's Entities[0]
      garbage read); class-5 model-22 flying-chain fallback.
- [x] B9 (P2) death fall: z-only (polar_step drift deleted), integrate-then-
      decrement, positive-vel zeroed, terminal -256, floor = ground + row
      v_12 (row-driven), EXACT-contact payout.
- [x] B10 (P2) Home/Upgrade/idle-home set target = own castle (the water-
      steer detour anchor).
- [x] B11 (P2) shield intake = two-stage (armed → null hit + promote to
      charged; charged → quarter, mana-paid, spent; `shield_state` field);
      hit sound = 54 + (LCG & 3) replacing the flat 17. Shield XP correctly
      NOT awarded to rivals (see B17).
- [x] B12 (P2) readiness re-keyed to the full sub_15170 table: armed-window
      refusal = {1,9,0x10,0x12,0x13,0x15} ∪ {4,6,8,0xB} ∪ castle-with-castle;
      speed-up NO cooldown; first-castle aim-free; cone = yaw-vs-SETPOINT
      (f30 vs f34), (255-P)/4+20°; approach/cruise speed-up gated on the live
      window (it would spam-cast otherwise).
- [x] B13 (P3) scout site: first qualifying sector corner in scan order
      (+x inner, +y outer, wrap mod 4), Chebyshev 12288 the only test; second
      candidate/water veto/+128 deleted.
- [x] B14 (P3) respawn: maxMana wiped to 1000; cooldowns KEPT except
      `cooldown[2] = 4·color`; war latches SURVIVE death (the old full war
      reset was invented too).
- [x] B15 (P3) cast_castle: affordability (fresh ladder) before any cooldown
      arm; cooldown arms only on a successful upgrade fire; first-castle
      spawn arms NO cooldown (EF:6831-40).
- [x] B16 (P3) notification life 100.
- [x] B17 **REFUTED by the decompile** (re-read in-session): sub_6D8B0's
      guard is `class == 3 && model == 0` — the HUMAN ONLY (EF:58240-41).
      Retail rivals have NO spell-XP progression; tiers are authored-for-life
      and the tier-down walk supplies the dynamics. The port's rival XP arm
      and `mc2_rival_relevel` were DELETED as unfaithful; the proj.rs
      PLAYER_TARGET award gate was correct all along. (The ledger's P1-4/B17
      claim came from an agent misreading `!model` — recorded here; the
      ledger stays immutable.)
- [x] B18 (tests) 5 new brain unit tests (see B1-B4) + the authored-books /
      castle-bank asserts added to `mc2_rivals_authored_castles` via the new
      `debug_mc2_rival_economy` hook.
- [x] B19 (docs) trace corrections 1-3 banked into
      mc2-rivals-spawn-mortality.md §8.6, mc2-rivals-open-closure.md §3.3,
      mc2-class5-m10-doomsday.md §2.9/case-7 (+ the port-worklist line).
- [x] BONUS (decompile-verified): the rival's tuning row is **67**, not the
      spawn-law 60 (`sub_4A9C0` pins `str_D7BD6[67]`, EF:33351) — fixes the
      altitude band (ground+128..768, was 0..1792), turn cap (256, was 22)
      and the v_28 engagement range (8192, was 4096) for every brain
      consumer.

Goldens: NO re-pins — MC1 goldens + the MC2 slice goldens all hold (the
fixtures never exercised the rival brain; the brain-determinism test is
self-consistent). fmt clean, clippy 0, 209 tests green.
Playtest owed (now unblocked): the full rivals feel pass — economy, raids,
possession contests, defense mimicry, respawn pacing, MC2 sounds ≥ 47.

## Session C — doomsday/pyramid column — ✅ LANDED 2026-07-15
(decompile agent re-verified the whole machine; 10 additional trace
corrections banked; all in mc2/doomsday.rs)

- [x] C1 (P1-24) Devour = the class-9 PROJECTILE walk (subtypes
      {2,4,5,22,23,25,30} within 0xC00 3-D): (10,0) mana-absorb at the
      projectile (owner = pyramid) + despawn; subtype 10 (the castle-build
      projectile) devours on bbox overlap with the player's castle and
      CANCELS the Castle spell (manifestation window zeroed, guarded).
      Trip = devoured anything OR the player's REBOUND window live
      (EF:13616-18). Creature-devour + player-proximity trip deleted.
      NEW FACT for the registry: building type 68 (`word_0x3654A`) reuses
      sub_21F60 as a projectile-devouring structure every tick
      (EF:40181-83) — unported, banked.
- [x] C2 (P1-25) Full pick table: f26=8/f38/f50 writes PRECEDE the cap test
      (persist on cap failure — retail quirk); roll-2 picks 8/9 = f38=1,
      f26=5 (was 5 shots!); picks 1/2 = 10/10 and 8/8; the population caps
      are the VERBATIM bucket-0 quirk (picks 4/6 cap against the MODEL-0
      population; only 3 and 5 count their own kind, 5 excluding action
      200); counts filter to live+bucketed like retail's chains.
- [x] C3 (P2) Case-7 beam = HURL AWAY (pyramid→player bearing applied
      outward), ramp 1024 → −80/tick → floor 10, FULL magnitude through the
      knock channel (the pose moveTest/floor-clamp stays the module's
      documented APPROX — the app owns the pose).
- [x] C4 (P2) Same-tick fall-throughs for the setup cases 0/2/4/6/8/0xA/0xC
      (the pick + the first shot now fire the same tick).
- [x] C5 (P2) The kill-all arm zeroes the StageVar subsystem at countdown 70
      (`countStageVars_0x36E00 = 0` → mc2_stagevars.clear()).
- [x] C6 (P2) Shared launch preamble for ALL cases: 640 along pyramid yaw at
      z+768; creatures step 1792 further at the stride bearing, z re-forced.
- [x] C7 (P2) Summoned-creature writes: StageVar2=17 (site_z), the
      actionIndex overrides (m0 1→7, m21 169→175, m25 201→207, m19 153→159,
      written LAST), f46=250/speed 320/yaw=roll=v32. parentId + the
      dword_0x364D2 spawn tally = APPROX-banked (no port field homes;
      noted inline, rides E23).
- [x] C8 (P2) The devour-tripped laser re-arms the beam ramp (bit1 set on
      the bit0/awake path); a trip while ASLEEP writes NO pick fields.
- [x] C9 (P3) bit-0x80 escalation clear wired into the pick (forces roll 1
      to 0); the (10,14) ring rolls on the PYRAMID's LCG; pyramid mail
      else-clears f40 (the attacker word — NOT the ring-angle field);
      word_0x36548 documented as reader-less (not carried); the case-0xF
      doom_meter=0 write deleted (retail leaves it at 1200).
- [x] C10 (P3) The sub_56F10 cave-ceiling arm added to the pyramid flatten
      (ceiling counter-shifts, saturate 255, char truncation); the shared
      dig_cell chassis ALREADY carried it — callers audited, no other gap.
- [x] C11 (docs) 10-item corrections block banked at the top of
      docs/traces/mc2-class5-m10-doomsday.md (headline: bit0 is the ATTACK
      trip not the death branch; the pyramid is damage-killable in states
      2..0xB — "unkillable" was wrong; the devour list misread; the
      hurl-away propagated).

Goldens: NO re-pins — the slice doomsday fixture passes unchanged (it pins
the extinction script's observables, not per-phase tick counts). Playtest
owed: the doomsday endgame feel (beam hurl, summon bursts, cave flatten).

## Session D — audio column — ✅ LANDED 2026-07-15
(dispatch-law agent verified Sound.cpp end-to-end; mgc-audio + mgc-import
+ one drain-time hook in mc1/world.rs take_audio)

- [x] D1 COMPLETE (was "residue"): playType-4 = the emitter-FED LOOP law
      (Policy::Feed for 47/49 — shared (0,id) channel, center pan, volume
      rides each feed, starvation = the EndLoop fade; ADJUDICATED vs the
      SDL loop-count ambiguity, remc1's dead-code arm corroborates);
      Select/CantUse level gate = RestartPlayerOnly for MC2 14/29 (4/17
      NOT gated in MC2, unlike MC1); the −8 slot law confirmed (slot
      arbitration only) with the drip 65-69 UNCONDITIONAL-overwrite bypass;
      the MC2 owner-collapse id set {7,32,38,42-44,46,47,49-53,58,59,62}
      keys on owner 0; MC2 channel cap = 10 (MC1 32, no stealing either);
      pitch jitter ±15% for 42-44 / ±10% for 46 on a mixer-side LCG.
      **BONUS: the shared vol/pan law was wrong vs BOTH retail games and
      is now verbatim** — XY-only distance (z removed), rear range 6144
      (was 9216 — rel folded 0..1024 unhalved), pan swing folded<<6 (full
      at 90°, CENTER directly behind; was half-swing with rear full-pan).
- [x] D2 Channel key = (owner, id): take_audio resolves the hashed
      emitter-index tag to the emitter's id24 at DRAIN time (frame not
      hashed — the snd() trap respected); Source::World carries `owner`;
      channels key on the pair. Test: `channel_key_is_the_owner_id_pair`
      + `mc2_feed_loop_follows_the_emitter`.
- [x] D3 bundle.rs: a missing cue sheet now loses ONLY the speech rip
      (sounds + music bake; empty speech.json; the manifest source entry
      says "cue sheet MISSING, speech clips not baked").
- [x] D4 Stale docs fixed: mc2_music.rs header + parse doc (bank 0 "C2" is
      gameplay; bank 1 = the `-music2` "C1" alternate), bundle.rs error
      string + 3 fluidsynth mentions → oxisynth, smf.rs header, FORMAT.md
      (bank-0 law, oxisynth, MGC_SOUNDFONT-only discovery).
- [x] D5 War-stem fade rides the GM cc11 expression square law
      (amp = (v/126)² — the linear gain ran the mid-fade hot).
- [x] D6 The remaining hard cuts ramped (~2.5 ms, the SFX declick class):
      StopMusic and Music-replace release-ramp (replacement installs after
      the ramp); pause/resume edges ease through `suspend_gain`.
- [x] D7 Nits: cc116-at-nonzero-tick guard in xmi.rs (mid-song FOR loop =
      hard error, the bank-1 alternate insurance); TICK_RATE const replaces
      the three bare 24.0s; the negative-banks i16 cast guard in
      mc2_music.rs; level_mc2.rs epoch comments verified current (field
      renames still pending a bump — not stale, left); FORMAT.md frames→ms
      already stated correctly at :396 (no change needed); the z-sub nit
      superseded by D1's z REMOVAL from the spatial law; redbook INDEX-00
      FIXED (a next-track pregap now ends the previous track's audio —
      insurance, the GOG sheet uses PREGAP directives).

Goldens: NO re-pins (audio is outside the sim hash; the one sim-side edit
is drain-time only). Ear-check owed: meteor grouping under the pair key,
rear/pan feel after the spatial-law fix, tornado/door feed loops, pause
edges, war-stem curve.

## Session D — audio column

- [ ] D1 (P0-5 completion) Port remc2's per-id request modes / policy table;
      verify ids < 47 against remc2's dispatch (they currently run MC1 law).
- [ ] D2 (P2) Channel key = (owner, id): carry the emitter owner word through
      `Source::World`, key channels on the pair, keep the per-id request slot;
      rewrite the mixer tests to pin pair semantics. Ear-check the meteor case
      still groups (same owner ⇒ one channel).
- [ ] D3 (P2) bundle.rs: missing cue sheet must not drop sounds.bin/music —
      fail only the redbook rip, and say so in the manifest.
- [ ] D4 (P2) Stale bank-1 docs: mc2_music.rs header, bundle.rs:634 error
      string, FORMAT.md:394 (+ fluidsynth→oxisynth mentions, MGC_FLUIDSYNTH).
- [ ] D5 (P2) war-stem fade: match retail's cc11 expression curve (or register
      the linear approximation as APPROX).
- [ ] D6 (P2) Ramp the remaining hard cuts: Suspend, StopMusic, Music replace.
- [ ] D7 (P3) cc116-at-nonzero-tick guard in xmi.rs; TICK_RATE const in
      mgc-audio (3 float literals); wrapping z-sub in spatial math;
      negative-banks cast guard; level_mc2.rs stale epoch comments;
      FORMAT.md frames→ms law wording; redbook INDEX-00 note.

## Session E — creature-machine batch — ✅ LANDED 2026-07-16
(5 parallel decompile-verification agents + in-session re-verification;
all 27 items resolved: 24 landed, E16 deferred to Session H by its own
ordering dependency, 3 E27 sub-nits REFUTED as already-faithful)

Headlines: E4 CORRECTED vs the ledger — the m18 barrage-1 turn cap is
retail's inline `0x400` snap (EF:16038), not (5<<11)/360=28; the m26
"%63 hijack" draw is kept for RNG parity but no roll exits the drain;
E12/E20 share a new `mc2_class3_scan` (the human pseudo-target IS in
retail's dword_38519 — the "exclude the player" gloss was wrong);
E12's rival wanted timers + E25's aura claim ride a new hash-quiet
`Mc2SlotMap` side channel (empty = silent, the goldens' friend);
E9 writes byte[0] bit 0 VERBATIM plus the port's 0x20 draw alias
(widening the renderer to retail's 0x21 law would break the certified
MC2 map-only house pose — documented in multipart.rs); E23 also
CLOSED the dword_0x364D2 census: it is the level-stats
creatures-killed-% denominator (EF:43498-505), wire with the stats
screen. E27 REFUTED: meteor quad sign (ring never negative), the
whirlwind 0xFFFF facing (no such site), the arrow xtype filter
(faithful in victim_scan).

Goldens: mc2_slice + mc2_cave re-pinned ONCE, dated in-file (E5 worm
f28, E10 ordinals, E13 mover, E15 awake — behavioral asserts all still
pass); MC1 goldens + rivals/castle/spell-channel fixtures UNTOUCHED.
6 new unit tests (m18 RNG parity, m26 stay-drain, aura claim, m12
fallback, m25 exhaustion burst, falling-prop gravity order). fmt
clean, clippy 0. Playtest owed: walker/herd feel (E10+E13), firebug
dives, tank barrage pacing, kraken branch fights, whirlwind pickup
breadth.


- [x] E1 (P1-11) m26 leech: the three "stay draining" paths stay in state 210.
- [x] E2 (P1-12) m19 firebug: cascading override rolls, `f63 & 3` gate on the
      whole block, restore states 4/5 reachability.
- [x] E3 (P1-13) m18 timers: (0,1)=60+rand%60, (2,1)=flat 10 no draw; draw
      only in the %-forms.
- [x] E4 (P1-13) m18_face: cap in angle units `(a3<<11)/360` → 22/28.
- [x] E5 (P1-14) m22 ctor f28=3.
- [x] E6 (P1-15) m22 relay: away-from-attacker yaw anchored at the hit
      segment; additive orbit-signed spin with retail clamp order.
- [x] E7 (P1-16) m27 branch life ladder counts chain NODES.
- [x] E8 (P1-17) m27_move turn cap = v_2 (22), the dead-arg law.
- [x] E9 (P1-18) m27 burrow: hide + untargetable (model the bit-3 clear in
      cases 7/0xA/0xF).
- [x] E10 (P2) goat/archer/villager ctors: per-model spawn ordinal → f63
      (herd cadence de-sync; re-check flocking feel after).
- [x] E11 (P2) mana-sphere fall arc: C truncation not div_euclid.
- [x] E12 (P2) archer wanted scan: post-reject the winner; arm wanted timers
      for rivals (the stub's gate condition landed).
- [x] E13 (P2) move_core retries: run the roughness/capability test
      unconditionally.
- [x] E14 (P2) m27 case-1 scan: no invisibility filter; strict <.
- [x] E15 (P2) awake pass: hidden-skip (needs the byte[0] bit0/bit5 registry
      entry) + run over the sphere family list.
- [x] E16 (P2, DEFERRED → Session H, LANDED with H6 2026-07-16) m27 0xDF
      stage-command arms — landed as `World::mc2_m27_held_tick` (sub_29930
      verbatim) on the new held-tick seam; the "preserve pre-hold
      actionIndex" concern dissolved (for m27 the held state IS 0xDF, its
      own action — release to 8m+1 is already retail's law).
- [x] E17 (P2) m23 siphon 64-tick timeout; climb-out altitude stale; patrol
      timer pre-decrement.
- [x] E18 (P2) m14 far-trade threshold 0xE100000.
- [x] E19 (P2) m12 template walk: wrap 0x4C→17, fallback returns 17.
- [x] E20 (P2) m24 acquire walks the class-3 list (castles, balloons).
- [x] E21 (P2) falling props: position-then-decrement gravity; kick modulus
      from written f44; port sub_654B0 landing re-aim (or APPROX-note).
- [x] E22 (P2) fire-trail child copies f80/f82/f84 (carve radius 3).
- [x] E23 (P2) m25: water arm no-op when already 314 & not-above; case-6
      speed-reset flag; split exhaustion path falls through to the burst;
      census the dword_0x364D2 reader question.
- [x] E24 (P2) tree ignite: spawn-gated advance, no extra RNG draw,
      untargetable clear.
- [x] E25 (P2) aura magnet: unclaimed-ball gate (word_0x7A_122==0).
- [x] E26 (P2) whirlwind lift: retail's victim classes (2 m7/8, non-castle 3,
      class-10 subset), drop the owner skip; fix the "superset" APPROX note.
- [x] E27 (P3) m15 f30/f34 swap (align to convention before anything routes it
      to shared arms); m9/m12/m14 xtype=3; blast23/storm pre-decrement lives +
      f26++; storm beam life/3 only + spawn-gated thunder; meteor quad
      my_sign32; orb per-carrier hit sound; fissure jitter rounding; whirlwind
      zero-draw facing; m15 volley player-stat; kill credit (killer class
      gate + rival tallies + self-id); arrow xtype/xsubtype filter; packmate
      avoidance act_life filter; m18 ctor ground-snap; m25 fall-through NIT.

## Session F — spell lifetimes + XP — ✅ LANDED 2026-07-16
(4 parallel decompile-verification agents + in-session re-verification)

Headlines: the XP mail widened to (owner, spell, AMOUNT) so Gen-level
effect ticks batch-award through the hash-quiet drain — 10 award
sites landed (castle +1, meteor/scorch-ring[f40-discriminated 16/17]/
blast23/dome/fissure/whirlwind-grab/whirlwind-contact[2×castles]/
flood-shove[+1]/flood-pass[2×castles]); the castle cast-gate cost
re-sync hack RETIRED (the award's spell-2 SetSpell branch is the
faithful path — the cost-gate test now guards it). F5's array_0x3E9
OR-arm RESOLVED: it collapses into the port's unified `ent[]` (no
grant-without-manifestation path exists); only the cave gate landed
(Cave-In never notifies/banks on surface levels; the level derive
stays unconditional). F1: Earthquake trail life = 1× charge
(16/32/64 — the 8× was whirlwind's law; ABSOLUTE reach is 8× shorter,
re-earcheck owed). F6 z-lift DEFERRED with an in-code note: applying
retail's launch-z lift verbatim self-detonates the trap because our
generic victim probe admits the launcher sphere — needs the
sub_10780 sphere-admission trace. Discovered en route: the tremor
carrier (model 23) has NO water exemption (models {4,22,24,26} do) —
a wet descent lane is a faithful splash-fizzle; the new tier test
casts down a dry lane.

Goldens: NO re-pins (the fixtures cast fireball only; the castle
fixture's XP is additive, its asserts extended not moved). New tests:
quake-family tier lifetimes (crater/gravity-well/tremor), castle
upgrade awards spell-2 XP, summon-army ring (F8), plus the earthquake
test re-commented to the 1× law. fmt clean, clippy 0. Playtest owed:
quake-family absolute reach/duration at each tier, Fool's Mana trap
XP + phases, tower-unlock progression through castle play.


- [x] F1 (P1-21) Earthquake trail 1× (life = charge; fix the test comment).
- [x] F2 (P1-22) Crater/Volcano/Gravity-Well/Tremor action-wrapper overrides
      (life=charge variants + f71/byte70 zeroes per ledger).
- [x] F3 (P1-23) Castle-upgrade +1 spell-2 XP; quake-family effect-tick XP
      (meteor/scorch/trail/dome/flood/whirlwind stubs) — Fire/Lightning Tower
      unlockable through play. Retire the cost re-sync workaround if obsolete.
- [x] F4 (P2) Lightning wizard-list pitch cone (0x71, 0x71).
- [x] F5 (P2) mc2_relevel v5 gate: cave gate for spell 25 (+ investigate the
      array_0x3E9 OR-arm).
- [x] F6 (P3) Fool's Mana nuances (XP on spend; tier phase/despawn ticks;
      xtype/row stamps; z-lift + splash; sound on projectile).
- [x] F7 (P3) possess re-press runs the invis-break law; cast sound only on
      successful spawn; f26 max(1) vs afford inconsistency + dead binding;
      CREATORS 255-row doc; proj.rs jitter comment.
- [x] F8 (tests) Summon-army ring: assert 8 + allied id24.

## Player reports round 3 (2026-07-16)

- [ ] PR-7 (LOW, player: "more or less a useless spell") Tremor's
      SHAKING effect is missing — a PRESENTATION gap separate from
      F2's landed tier lives: trace what retail drives the shake
      with (a camera-shake global off the (10,71) fissure tick? the
      f50 shake channel?) and whether the app consumes any shake
      signal at all.

## Session G — castle/cave geometry — ✅ LANDED 2026-07-16
(4 parallel decompile-verification agents + in-session re-verification;
2 items REFUTED, several ledger wordings corrected; ONE deliberate cave
re-pin at the end as planned)

Headlines: G1's ring walk kept as retail's VERBATIM QUIRK (faithful-quirk
ruling): every band — sides included — iterates `my` rows, so the whole
EAST border column and the lower side rows are never tested (my==0 tests
NOTHING), and band 4's first row starts near the CENTER then collapses
onto the left column; the ledger's "spurious eastern aborts" wording was
backwards — the east is UNDER-checked. G2 was a one-line unreachability
bug: the port's eject already carried the faithful f26==0 spill-all arm;
the destroy path just returned before it (retail runs the ejector
unconditionally, EF:61228). G4 CORRECTED vs the ledger: the countdown==2
sweep is a NON-cave arm (EF:27895) and the "gated phase-B promotion" was
already ported (the gate is first-build vs repaint, byte_0x3B_59 — not
cave); the real cave arms were the ceiling rise + seal (EF:27871-94, runs
for EVERY frame cell outside the active-delta gate) and cave-gating the
bit3 blind-set/bit3→bit7 promote. G6 REFUTED: `EuclideanDistXYZ_58490`'s
BODY is 2-D (Maths.cpp:738 — the name lies); the port was already right.
G9d REFUTED: both engines classify pre-rise. G9f exposed retail's
sub_34B00 asymmetry: SE corner structurally never stamped, and the
bottom-row/right-column stamps write angle+retile WITHOUT the type. G9j:
BUILD00 row 7 is 48×48 (rows 8-16 are the 1×1s) — the "degenerate row-7
memory stomp" story was false, the widening loop is a proven no-op. G5
consolidated into the pre-existing `mc2_smooth_pad_edge` (same retail
sub!), upgraded to packed-word neighbour math (word−0x101 borrows across
the y byte at x==0).

- [x] G1 (P1-19) space_ok scans class-3 model-2 castles (+ act_life ≥ 0);
      ring walk = the verbatim quirky partial walk (z-term documented
      tautological at fov 0x4000). Test:
      `space_check_blocks_on_castles_not_props`.
- [x] G2 (P1-20) The destroy path's early return deleted — the level-0
      death now spills the whole bank as owned (10,39) spheres; roster
      verified a no-op at level 0. Ordering nuance documented (balloon
      conversion front-loaded vs retail's roster-shed). Test:
      `castle_death_spills_the_whole_bank`.
- [x] G3 (P1-26) `cave_write_floor` a6=0: unconditional h==0 nibble clear
      + always-retile (`AddBuildingToTerrain`, keyed on a4). Test:
      `mesa_floor_write_retiles_and_clears_h0_nibble`.
- [x] G4 (P2) Cave ceiling rise + per-cell seal (every frame cell);
      bit3 set + phase-B promote now non-cave-gated; non-cave
      countdown==2 sweep added. Arm 4 refuted (already ported). Test:
      `cave_painter_carves_headroom_and_keeps_the_seal`. Golden-safe (no
      fixture builds a cave castle).
- [x] G5 (P2) Unstamp finalizer = `SetHeightmapByBuildingArea_48B50`
      (footprint box, no border, no retile) over the shared
      `mc2_smooth_pad_edge` (packed-word 3×3, cave re-seal). Test:
      `unstamp_smoother_averages_natural_neighbours`.
- [x] G6 **REFUTED** — retail's distance IS 2-D (the XYZ name lies);
      port correct all along, morph.rs already documented it.
- [x] G7 (P2) All three raw-lcg32 sites → the u16 `ent_rand` (eject
      dist/yaw EF:61312-16, drip sprite EF:37025, pit/hill depth
      EF:25639).
- [x] G8 (P2) Dome/pit/hill measure to tile<<8 (corner, no +128); dome
      sync bit3-ONLY (scope correction: pit/hill/mesa/tube DO pin
      ceiling=floor−1 — only the dome was wrong); tube ring side+1.
      Tests: `dome_sync_never_pins_the_ceiling`,
      `tube_wall_ring_covers_the_far_corner`.
- [x] G9 (P3) a: eject count = min(headroom, clamp) — full spill in
      fewer-but-bigger spheres; b: intake channel cleared only on owner
      match (sticky otherwise, faithful quirk); c: under-attack flags
      registered APPROX in FIDELITY.md (player-only HUD latch);
      d: **REFUTED** (classify already pre-rise both engines); e: road
      Y-strip family from the STEP sign with signed row count (both
      branches, EV:5423-33/5468-77); f: wall ring = sub_34B00 verbatim
      (SE corner never, thin bottom/right stamps); g: perimeter walkers
      transposed like retail's sub_48F20/48FD0 (rows=4th arg, bottom at
      3rd — square-only today); h: cave-in debris z = the stale
      one-past-the-box neighbor; i: riser build-Y non-cave column step =
      WORD decrement (build-X verified word in both branches — no
      split); j: BUILD00 row-7 comments corrected (row 8-16 are the
      1×1s); k: piece dwell same-tick fall-through + axis-home latch
      (dest_x/dest_y/site_z, the 4.2 launch anchor); l: balloon stagger
      modulus = the QUOTA; m: flood damage `.max(1)` floor dropped
      (retail has none); n: level-7 haircut overflow registered as a
      FIDELITY.md idealization (retail's i32 wrap RAISES the cap and
      scatters nothing — we keep the sane 10%).
- [x] G10 Cave goldens re-pinned ONCE, dated in-file, absorbing G3 + G7
      (drip/pit-hill) + G8a/b/c + G9f + G9h. All behavioral asserts
      hold; MC1 goldens + slice/castle/rivals/spell fixtures untouched.

Goldens: ONLY the four mc2_cave checkpoints moved (the intended set).
fmt clean, clippy 0, full workspace suite green (191 lib+fixture tests).
Playtest owed: castle overlap/upgrade blocking near rivals (G1), castle
death loot spill (G2), cave-level castle builds (G4 headroom bubble),
dome/pit/hill bowl symmetry + tube walls on cave levels (G8/G9f).

## Session H — stage engine — ✅ LANDED 2026-07-16
(4 parallel Opus verification agents + in-session decompile
re-verification; all 9 items + the E16 deferral landed; ZERO golden
re-pins — every fix is release-path-only, byte-order-preserving in the
hash, or on shipped content outside the pinned fixtures)

Headlines: H4 settled AGAINST THE LE BINARY (NETHERW.EXE carved from
game.gog and disassembled at the banked recipe's offsets) — InitStages'
"drop typed stage==0 rows" guard reads the memset-zero DESTINATION row
(EF:40589) and is DEAD CODE, so retail registers every authored row,
ACTIVE; the port's literal drop severed level-198's m32 chain and is
removed (13 levels regain rows; the 5 type-1/2 stage0 rows bind the
empty record 0 and are FAITHFULLY un-completable — retail ends those
levels via other paths, e.g. the model-31 X-marker latch). H6
escalated well beyond its P2 framing: HELD ≠ FROZEN — the new
`mc2_held_tick` seam (world class-5 dispatch → stagevars.rs) runs
sub_1D5D0's per-kind head every tick: held creatures are KILLABLE
(inbox drain → prekill a2+4), a foreign-class/model hit breaks the
hold into aggro (sub_1E040's FLEE split 8m+2/8m+6, StageVar2=10), and
kind-3 is the AMBUSH law — aggro on the WATCH itself within v_28 (the
agents' "join the fight" gloss was kind-4's law; re-verified in
sub_1D7C0 vs sub_1D700). The kind-3 release gate's (action&7)!=2/6
exists precisely to protect aggro-broken creatures from being
clobbered back to active-start — one mechanism, two review items.
E16 rode the same seam: `mc2_m27_held_tick` = sub_29930 verbatim
(sub_1D5D0 head FIRST — m27-exempt life inherit, kind-gated physics
via m27_move whose code-4 arms the 0xD8→StageVar2=15 interlock — then
pose 337/315 select on kind, life refresh, the 0xDA MASS-ATTACK
broadcast throwing every idle branch f71 1→2 at the body target).
Census (examples/tmp_svcensus.rs): 9 shipped levels hold an m27
(8 kind-3 ambush + 1 kind-1 proximity); NO shipped level holds an m9,
authors slot 0, a zero kind-6 timer, or a spawn-matching cadence+chain
slot — those fixes are latent-but-lawful. Held-idle APPROX register
(facing choreography, per-model wrapper extras, generic physics
settle; f63 stays the spawn ordinal — retail never increments it while
held) documented in stagevars.rs's module doc.

- [x] H1 (P1-9) Type-2 degradation-chain succession: sub_59760 re-point in
      mc2_house_collapse's chain branch + retail's !fontTypeIndex term in
      the predicate (read as `bldgprm[f71].chain == 0` on the dead husk).
      REFUTED the port's own "type 2 reduces to type 1" comment. The
      level-008 test was pinning the buggy early completion — now razes
      the whole chain and asserts the row survives the first collapse.
- [x] H2 (P1-10) Cadence-skip → unconditional full release (retail routes
      the skip straight to sub_12470, EF:5010-14); `via_chain` renamed
      `direct` + re-documented (it was never a recursion guard; the true
      split is sub_12410 chain-aware vs sub_12470 leaf).
- [x] H3 (P2) m9 deferred arm: hash-quiet `mc2_sv_deferred` (own hash gate,
      empty = silent) parks the slot at spawn; armed when the materialize
      completes (sub_122A0 — the port arms on the next pre-loop pass, same
      observable sequence). Hive-split imps stay unattached (census: zero
      shipped m9 holds; retail's pointer-match semantics don't map to a
      template-less split — documented, not guessed).
- [x] H4 (P2) Stage-board un-drop (LE-binary verdict above). Row indexing
      was already retail-aligned once the drop fell (the baker pre-compacts
      the -1 slots, so enumerate index == retail's compacted index). Test
      `mc2_stage0_typed_rows_register` pins level-198 (4×type-7) and
      level-038 (7-row board incl. the stuck type-1 row 6).
- [x] H5 (P2) Type-5 latch = retail's single sign-extended abs (EF:40803-14,
      no torus min — the seam discontinuity is retail's genuine quirk);
      guide overlay deliberately keeps its torus arrow metric, both
      comments disambiguated.
- [x] H6 (P2) Kind-3 (action&7)!=2/6 release gate + unconditional word74
      clear (word74 = retail's dual-use kind-6 timer / kind-3-4 watch-handle
      cache — Mc2Held.timer now mirrors both uses). Held-damage DECIDED +
      PORTED: killable, aggro-break, kind-3 ambush + kind-4 join arms,
      kind-10 re-raise, kind-15 inert. Tests: level-058 ambush kraken +
      synthetic kind-6 held kraken (animates, breaks hold on hit).
- [x] H7 (P2) Kind-9 pointer-bytes quirk: DOCUMENTED + the dead proximity
      branch deleted (unreachable in retail too — the union read can never
      pass the 3072 test; 3 shipped kind-9 levels all death-watch release).
      Kind-6 timer = u16 wrap, release at ==0 only (authored-zero holds
      ~65536 ticks). Slot-0 forced inert (retail's fill starts at index 1).
      Type-9 16-frame cadence LANDED (`mc2_turn & 0xF`, the hash-excluded
      frame counter — EF:40852). Type-1/2 re-point UNCONDITIONAL on every
      matching spawn (sub_58DA0 has no bound guard and no state gate — the
      row tracks the newest instance).
- [x] H8 (P3) Stagevar tick hoisted FIRST among the pre-passes (retail
      UpdateEntities order stagevar → awake → drip → loop, EF:40093-116;
      hash-neutral on the pinned set). mc2_ent_dead anchored on thing_slot
      identity (the LIFO-recycle guard mc2_bound_gone already had).
      Speech-ramp hash leak closed on the stage-less side only, appended
      AFTER the stages gate (the first attempt hashed it before the gate —
      byte-order change, cave pin moved, reworked; pins hold). Disposition
      census PINNED at 110 via test (class-0 garbage ids up to 30720
      excluded) + a non-golden 1..=120 storm on level-020 asserts zero
      misfits — the pinned 1..=64 golden storms stay untouched. Census
      tool "ported?" column NOT done (P3 tool nicety, banked).
- [x] H9 (P3) The stale "NOTHING is held" comment + re-pin note lived in
      tests/mc2_cave.rs, NOT src/mc2/cave.rs (ledger mislabel, flagged);
      both corrected to match the (447,18,9) assertion.

## Player-reported regressions (2026-07-15 playtest, MC1/MC1HW) —
## ✅ LANDED 2026-07-15 (6 parallel Opus trace/investigation agents +
## in-session decompile re-verification; traces banked in docs/traces/)

- [x] PR-1 Jars now ride the ground: idempotent per-tick ground snap in
      `class12_tick` (world.rs). TRACE CORRECTION: retail is
      spawn-at-ground (:44005) + STATIC-z — no runtime re-settle exists
      (terrain writes ignore class 12, :51729); ours was never
      spawn-buried either, HW's post-spawn level shaping diverged the
      static z. The snap is the minimal reconstruction of the resting
      invariant; hash-neutral while z already matches — goldens verified
      unchanged. Trace: mc1-jar-ground-law.md. Test:
      `jars_ride_terrain_changes`.
- [x] PR-2 Speed hold/re-cast fixed — culprit was 1012805, NOT c44021a:
      the Simulation::step MC1 arm fired BOTH cancel directions on any
      thrust; retail's v_14 arms only when the press MOVES v_12
      (:55766-80), and a boosted v_12 (±160/240) sits outside the ±80
      clamp, so only the RESISTING press cancels (decompile re-verified
      in-session; the lib.rs comment's "any press cancels" reading was
      wrong). Both thrust models now share the directional cancel.
      Trace: mc1-speed-spell-hold.md. Test (J4 companion):
      tests/accelerate_hold.rs.

## Player reports round 2 (2026-07-15, same-day session) — all
## resolved (3 fixes landed; crosshair player-ruled keep-as-is)

- [x] PR-3 MC1/HW monsters+dwellings instantly dying (balloon
      correlation): CONFIRMED = the balloon claim ticket is a RAW slot
      index; a collected ball's slot LIFO-recycled by a dwelling (10,45)
      passed the class-10 check and was "absorbed" (flags |= 0x400).
      Latent RETAIL bug (sub_47F90 :56742-73 checks class only) — fixed
      as bug-fix class: target must be model 39 or the claim drops to
      idle, both games (features.rs balloon_move + mc2/castle.rs
      mc2_balloon_tick; MC2's dispatcher already filters at assignment).
      Class-5 MONSTER deaths remain OPEN — no lethal raw-index carrier
      found; playtest question: were the victims actually class-10
      props? Root-fix follow-up banked: hash-excluded per-slot
      generation counter for the whole stale-ref class. Trace:
      mc1-instant-death-investigation.md. Test:
      `balloon_ignores_recycled_claim_slots`.
- [x] PR-4 Stuck castle transformation under meteor bombardment:
      CONFIRMED port regression ON TOP of the retail bug — retail keeps
      each transform commit inside the spawn-success arm (sub_47960
      :56471, sub_47020 :56107, sub_47080 :56126) and RETRIES next tick
      (a stall that self-heals); the port advanced f59 to pure-wait
      states unconditionally → permanent deadlock once a painter/leveler
      spawn failed on a full pool. Fixed: all three sites now commit
      only on spawn success (the level-up commit — gong/level++/HP/
      extents — moved inside too, so retries don't re-ring the gong).
      Pool stays 1000 (it feeds the state hash — a default bump would
      re-pin every golden); `--pool-slots` / entity_pool_size (clamp
      2..=60000) is the opt-in headroom, now merely comfort since
      exhaustion degrades to retail's brief stall. Trace:
      mc1-castle-transform-stuck.md. Test:
      `castle_transform_retries_failed_spawns`.
- [x] PR-5 MC1/HW quickselect auto-assign on spell pickup: retail law
      ported app-side (never sim-hashed) — first free key 1→9→0, cap 10,
      silent when full (:64858-67); level-init pre-seed reproduced by
      walking the canonical book order byte_99B88 (:49216-59, identical
      in HW) over the acquisition diff; wipes on restart like retail;
      manual rebinds still win within a level (auto fills empty slots
      only). Gated on `selector.map_book` (MC1 / MC1+MC2 / auto→MC1
      schemes). Trace: mc1-quickselect-assign-law.md.
- [x] PR-6 Crosshair pitch disparity too pronounced (port ~0.145/0.855
      screen-height at full aim vs retail ~1/3 / 2/3): root cause is the
      RENDERER's vertical projection model, not the crosshair math —
      retail is an affine horizon SHEAR (eye row = H/2 − W·(aim/2)/256,
      :33872/:38245, + fowDist·tan(α) elevation :36853) whose half-pitch
      shear near-cancels the full-aim elevation; the port perspective-
      projects at aim/2 with FOV_Y 60° and no cancellation (≈2.4×
      overshoot). Crosshair-only correction would desync it from where
      port-rendered shots visually fly (the certified predictor
      property). **PLAYER RULED 2026-07-15: keep the perspective
      renderer as-is** (retail's shear is the technically-wrong
      projection); registered in FIDELITY.md. Trace:
      mc1-crosshair-pitch-law.md.

## Session I — MC1-side rulings + fixes  [LANDED 2026-07-16]

- [x] I1 (P2) Held Lightning: LANDED — held ticks re-arm only while the
      burst is LIVE (dry stream needs a re-click, :20626-32), the mana
      check is SILENT (:55890 — no buzz 29, no reload), and the held
      re-arm debits per-shot cost/count = 500 (the firehose idiom)
      instead of re-running the full 1000 cast body every tick.
      Verified vs :55851-919/:20586-634 (Opus agent + in-session).
      Regression-pinned in tests/refire_gate.rs.
- [x] I2 (ruling) PLAYER RULED 2026-07-16: FAITHFUL edge-only. The +60
      latch table was fully enumerated: retail's hold set is exactly
      {2, 15, 21, 23} (+60==0); Heal/Shield/Beyond-Sight/Invis/Rebound
      are +60==1 = edge-only. 1/4/5/12/14 removed from the hold-channel
      arm (world.rs cast_spell); docstring corrected ("player-validated
      2026-07-06" retired as a misvalidation). Pinned in
      refire_gate.rs::shield_channel_is_edge_only.
- [x] I3 (P2) HW homing: LANDED — one-shot acquire latched on flags
      bit 2 even on a miss, heading SNAP on lock (remc1hw :58731-49),
      post-lock ease kept (= sub_52550); whole latch inside the
      is_hidden_worlds() gate so MC1 never writes bit 2. Sub-findings:
      pre-loop LCG draw REFUTED (no such draw; the 9377x+9439 LCG is
      only in the deflection branches); 0x12/0x13 (Global Death m18 +
      m19 homing) confirmed unported but is a larger spawn+flight
      reconstruction — BANKED (the port's m18 fuse is a documented
      player-validated reconstruction).
- [x] I4 (P3) Kraken: LANDED — grip duty-cycle compares PRE-increment
      (:23219-22; f26 reaches 41 = 41 ON ticks / 132 period; the port's
      own comment was right and the code wrong), growl 37 moved BEHIND
      the range gate next to f71=5 (:23232-42 — out-of-range cadence
      tick is silent; the ungated duplicate modulo block deleted).
      Shield-quartering note REFUTED: port already quarters in the
      display accumulator; retail god-mode skips intake entirely.
- [x] I5 (P3) LANDED — drawable() takes GameId and gates the MC2-era
      arms (classes 14/15; class-10 models 13/14/22/75/77 — 39/45 were
      pre-MC2, left unconditional), life_frac's class-10 bar arm gated
      Mc2, the f63 233/234 exemption gated Mc2. All render-only or
      MC1-no-op: zero golden movement (verified).
- [x] I6 (P3) REFUTED — the port bound is 2096 (chassis
      level_table_slots), deliberately ABOVE retail's 2000 as
      community-slot headroom (chassis.rs:31-33 documents it); scans
      are len-driven. known_thing class-11: models 5..=11 switch
      handlers are a tracked OPEN port gap (ids.rs:87-88), not a
      regression. No change.
- [ ] I7 (playtest) 24Hz re-certification pass of MC1 feel (regen/flight/
      burst pacing — certified at 30Hz). OWED (player).
- [ ] I8 (playtest) castle_lock_active MC1 window feel (P2 #3 of the MC1
      audit). OWED (player).

## Session J — infra + app polish  [LANDED 2026-07-16]

- [x] J1 (P1-27) LANDED — tests/common/mod.rs golden_skip(): every
      baked-data self-skip (18 sites, 6 files) prints a grep-able
      GOLDEN-SKIP line, and MGC_REQUIRE_GOLDENS=1 turns any skip into
      a PANIC. ci.yml tees the test log and reports the skip count;
      the required local pre-release step (MGC_REQUIRE_GOLDENS=1
      cargo test --workspace against real baked/) is documented in the
      workflow. No baked fixture committed (gamedata-pristine rule).
- [x] J2 (P2) LANDED — distinct field-tag bytes written INSIDE each
      conditional hash contribution: Mc2Quiet<TAG> (drain=1/scrolls=2/
      tokens=3), Mc2SlotMap<TAG> (aura_claim=4/wanted=5),
      Mc2PlayerDebuffs (6), apocalypse/doom_meter/doom_level
      (0xA1/A2/A3 inline). MC1 goldens unmoved (no tagged field ever
      fires there — verified); mc2_slice + mc2_cave re-pinned DATED
      LAYOUT-ONLY (instrumented: slice fires only tag 3 — the
      fireball+possess token baseline; cave fires 3 + 4).
- [x] J3 (P2) LANDED — World::observable_digest() (poses type/quantized
      x/z + height plane + population), pinned as a companion
      OBSERVABLE golden at every checkpoint of state_hash.rs /
      mc2_slice.rs / mc2_cave.rs. A layout-only re-pin claim is now
      machine-checkable: GOLDEN moves, OBSERVABLE must hold.
- [x] J4 (P2) LANDED — tests/refire_gate.rs (7 tests, no baked data:
      synthetic flat world): edge refire with no cadence gate
      (c44021a), held edge spell casts once, shield edge-only (I2),
      accelerate holds, firehose per-tick, lightning streams while
      held, lightning dry-silent/no-auto-resume/re-click (I1). Castle
      recast-fizzle already pinned in spell_castle.rs. Added
      World::debug_bless_owned_spells() (BLUE flag) test hook.
- [x] J5 (P2) LANDED — config-path entity_pool_size now rejects outside
      2..=60000 exactly like the CLI (main.rs, before load_level).
      Default untouched (pool size feeds the hash).
- [x] J6 (P2) LANDED — spell_selector + audio.arrangement flipped to
      Startup (pane rebuild / track reload are not trivial live
      applies); thrust/altitude/invincible/prune_owned_jars stay Live
      per the "can trivially gain a live apply path" clause with the
      pending hooks documented at the mutability() match (setters all
      exist; wire on menu landing). hud_transparency verified
      genuinely Live (per-frame read; no minimap snapshot found).
- [x] J7 (P3) LANDED, all 12: mc2sweep honest scope banner + --combat
      profile (mid-map firing sweep, 512-tick default); examples-in-CI
      documented as gamedata-gated (rides the J1 ci.yml note); F1/F2
      print "(no audio device)" when the device never opened; defaults
      _readme names the prune_owned_jars exception; --health-bars in
      usage; Ctrl-release restores the PRE-Ctrl grab state; second
      mouse button can no longer steal a live selector drag;
      MGC_HUD_OPAQUE parses 0/false/off + 1/true/on and warns
      otherwise; stale enhancements.* key names swept to canonical
      cfg paths (main.rs/ui.rs/entities.rs/FIDELITY.md; ROADMAP left
      as historical ledger); expose_jar_spells reclassified Debug
      under render·debug (cfg_path kept for config compat);
      hud_transparency under its own render·preference heading;
      Val::Override prints "default" instead of doubling the hint.
- [x] J8 (P3) LANDED — verbs.rs header + all 8 PENDING arm comments
      rewritten to the HEAD truth (columns LIVE; damage player-intake
      + MC1-spell acquire paths still note deliberate fallbacks —
      frankenstein pins that ledger, and its header now says so);
      ids.rs registry comments reconciled (22/71/76 tail band, 87
      effects band, 58 = the 2560 authored record); frankenstein.rs
      header rewritten; flight gate + stuck test now read
      mc2_carpet_row().clearance (behavior-identical; fov 100 is a
      genuine constant); stagevar ordering comment was ALREADY fixed
      by H8i. combat.rs aim_assist/corpse_drop + lib.rs move_mc1
      seam comments truth-updated too.
- [x] J9 (docs) LANDED — residual stale passages folded in
      mc2-rivals-spawn-mortality.md §6.2 (AI at-castle discard),
      mc2-rivals-open-closure.md headline (else-fallback law),
      mc2-class5-m10-doomsday.md §6 (hurl-away, not tractor),
      mc2-castle-data-tables.md (the BUILD00 story was an OFF-BY-ONE
      TAB read — re-parsed the baked TAB: rows 1-7 =
      8/21/21/35/35/48/48, 1×1 band starts at row 8; §4.4 table,
      note, constants row and the §6 OPEN item corrected/closed),
      quake-family.md + 00-PLAN.md (quake shot = sub_66160 at 1×;
      only whirlwind is 8×). FIDELITY.md gained 4 APPROX entries
      (doomsday register, held-idle reductions, rival disguise
      visual, worm link-length floor 96). Memory files corrected
      (mixer owner-pair key landed; the ae545a6 layout-only re-pin
      note).

---

Sequencing rationale: A unblocks everything (build + the systems that make
MC2 unplayable-as-retail); B before the owed rivals playtest so it certifies
the fixed brain; C/D/E/F/G/H are independent batches sized for a session
each; I needs player rulings/playtests; J hardens the guard so the later
batches can't silently regress.
