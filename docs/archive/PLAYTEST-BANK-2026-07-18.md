# Playtest fix bank — 2026-07-18 (for the NEXT session)

Seven player reports from real campaign play, explored and banked
same-day so the next session jumps straight to fixes. Root-cause
pointers verified against the working tree at bank time.

## RESOLUTIONS (fix session, same day — playtest owed on all)

1. **LANDED** — `dev_spells` threaded into `ui::hud_quads`; wash
   gated `!dev && !castable`. (MC1 arm needed nothing: sim `bindable`
   already includes dev, world.rs:4074.)
2. **LANDED** — prune predicate now reads `mc2_book.ent` (option
   branch only; hashed collect gate untouched). Regression test
   grants via `mc2_grant_plausible` (book set, mask clear) → prunes.
   SIDE FINDING (re-collect) deliberately NOT touched — still open.
3. **LANDED** — MC1 (10,45) dwellings (the possession whitelist's
   only dwelling row; map-01 = ×14) join the life_frac allowlist,
   live state 52 only, denominator = f44 (the parked health that the
   damage mail drains; max_life 30 is the BUILD countdown).
4. **LANDED** — `mc2_campaign_order()` (mains 0-24, secrets after
   parents 4/7/11/17/19); plausible book scans the campaign-order
   prefix; non-campaign target = full campaign. Test pinned.
5. **LANDED (root fix + regression test)** — decompile+binary-grade
   trace REFUTED the banked H1: retail contact is the same 3-D box
   (sub_10630) and ours is faithful-plus (chord-march is our
   anti-tunnel addition); pitch caps row-60 22/22 faithful; the 0x71
   acquisition cone is faithful (a level shot can NEVER lock a
   16-tile flyer — retail identical; aim up at it). ONE divergence
   fixed: per-tick homing now raises the target to its z-box CENTER
   (`sub_65580`: z += f78 unless class 2 — the "model_0x40_64" name
   is the CLASS byte), matching the acquisition sites. Goldens hold.
   Do NOT widen ent_overlap / the cone — verified faithful.
6. **LANDED — VISSULUTH IS KILLABLE** (4 coupled fixes, all
   decompile- or binary-verified; NETHERW.EXE extracted from
   game.gog for byte-level ground truth):
   - The machine + wind-down arm are FAITHFUL — both "single-bit
     transcription error" hypotheses REFUTED at byte level
     (offsets 0x45e9c/0x45f01/0x45f11: and cl,0xBF + jz exactly as
     decompiled).
   - Ctor `flags |= 0x48800001` (EF:33980) — byte[3]&0x40 is the
     LOAD-BEARING render gate; the port had omitted the whole OR.
   - The wind-down escape bit 0x40 is armed by the RENDERER's
     detailed-draw pass (GameRenderOriginal.cpp:4918) — reproduced
     as the deterministic proximity analog in the tick prologue
     (radius = the machine's own 0xA00 far-gate; any radius ≥ it is
     behaviorally identical).
   - The AWAKE law: the kill-all exit clears the ctor's hidden bit
     (`byte[0] &= 0xFE`, EF:12983 — the port was missing it), which
     hands the pyramid to the STANDARD proximity self-wake; f58
     then gates `sub_22190` damage intake. Death (case 0xE) re-sets
     the bit (EF:12846). Boss design now legible: dormant-
     invulnerable through the opening ritual, then approach-to-
     engage, killable, kill-slot-379 → win (objective wiring was
     already present via mc2_bound_gone).
   - Bonus: u16 underflow in the hurl-away beam decay (case 7,
     exposed by the natural path; retail arithmetic is widened).
   - f52-vs-f54 awake-propagation worry RESOLVED: port f54 IS
     retail word_0x34_52 (multipart chain doc) — faithful, no change.
   - Regression: `mc2_doomsday_death_script` now runs the WHOLE
     fight naturally (no f71 jump): spawn → ritual → escalation →
     wake → pound → immortal-clamp floor → state-3 death → 0xC-0xF
     → apocalypse. Goldens all hold (no golden level spawns a
     pyramid).
8. **(follow-up report, same day) LANDED — level-024 gauntlet hydra
   lock-out.** The opening wall-gauntlet's second/third gates are
   (11,30) ANY-slot kill switches (slots 43/120; first gate = the
   (11,35) firefly watch). Our ANY census scanned slots 0..=0xB +
   0x10..=0x1C — but retail's `sub_6F300` a2==-1 scan loop is
   bounded `<= 16` (binary-verified: NETHERW.EXE @0x93BA6
   `cmp eax,0x10; jng`; the `<= 0x1C` arm of the condition is DEAD
   past the bound). Shipped law = slots 0..=0xB and 0x10 ONLY —
   high models 0x11..=0x1C (incl. the authored wandering (5,27)
   hydra at slot 168) are structurally excluded, exactly the
   "exclusion" the player intuited. Same effective law as MC1's -1
   variant (which our MC1 arm already had right). Fixed the m==30
   arm + regression test (`mc2_any_slot_switch_ignores_high_models`)
   + corrected docs/traces/mc2-class11-switches-class14.md §Model
   0x1E (it had transcribed the dead condition). Goldens hold.

9. **(round-2 report) LANDED — meteors "barely hurt" wyverns = the
   IMPACT LANDING Z.** Probe-driven (level 024 runtime): the meteor
   locks instantly and box-contacts the wyvern fine — but the
   victim-hit relink copied the victim's RAW ORIGIN, while retail
   lands the projectile at the victim's z-box CENTER (`sub_65580`
   raise → CopyEntityPosition → `sub_655A0` restore, EF:62941-43).
   The (10,17) burst then spawned ~3 tiles BELOW the wyvern's box
   (wyvern f78 large) and its `sub_10C80` 3-D area window never
   reached the victim → zero damage. Fixed: victim-hit relink now
   lands at z + victim f78 (class-2 exempt) — probe shows 3000/tick
   for the full fuse. Also closed the m16 ctor APPROX: retail wyvern
   array.yaw = 5·word(D9F50+294)/8 = the CONSTANT 937 (row-21 word
   0x5DC, offset never written at runtime) — was sprite-derived 750.
   Regression test extended (lands at box center + burst damages).
   Goldens hold. UNPORTED noted: retail impact also runs sub_65780
   (shot-stats counter) + sub_686D0 (caster last-victim latch) and
   stamps the victim into the burst's word_0x96_150 — all
   no-consumer bookkeeping in our port today.

10. **(round-2 report) FAITHFUL, NO CHANGE — wyverns attacking the
   Vissuluth pyramid.** Full victim-selection trace: wyverns scan
   ONLY the wizard list and the (10,45) dwelling list — the pyramid
   (class 5) is on neither, so proactive acquisition is impossible
   in retail AND our port. The one path that reaches it — universal
   RETALIATION (mailbox attacker latch, no class filter, and the
   pyramid passes target validation with its clamped life) — is
   retail-identical: wyverns caught in the pyramid's crossfire (its
   bolts/meteors/whirlwinds splash-tag them) chase and bolt it back.
   Do NOT special-case the pyramid out of the target set (would
   break faithful retaliation).

## ROUND-3 BANK (2026-07-18 late — the endgame playthrough; player:
## "bank these for next session"). Vissuluth DIED and the level was
## finishable — these are the rough edges.
##
## ROUND-3 RESOLUTIONS (fix session 2026-07-19 — ALL FIVE CLOSED;
## playtest owed):
##
## 11. **LANDED — the pyramid-summon RELEASE CHAIN.** Root: retail's
##     +7 slot IS `sub_1D5D0`, and its cases 16 (0x10) and 17 (0x11)
##     were ported NOWHERE — any creature spawned into StageVar2
##     16/17 hit the dispatcher's `_ => {}` no-op and froze,
##     mailbox unread (= unkillable). Ported both: site_z 17 =
##     `sub_1E320` spin-up (intake, aim, `f126 -= 8`, at ≤16 the
##     per-model cruise m0→30/m19→76/m21→96/m25-unchanged, drop to
##     16; life<0 despawns outright), site_z 16 = `sub_1E580`'s
##     no-decrement arm (parent-death expire with fire puff; kill →
##     corpse stands at f46=1 until the pyramid dies, EF:10864-67;
##     hit → retaliate at the attacker unless parent/same-species,
##     flee rows +6; quiet → 8-tick aim + 64-tick jink + same-model
##     crowd steer-away — the anti-pile-up; engage inside row v_28 →
##     the model's +2 attack). Parent link = scan-resolved single
##     (5,10) (`parentId_0x28_40` has no entity home). Full walk in
##     mobs.rs `mc2_doom_summon_{spinup,home}_tick`; test
##     `mc2_pyramid_summons_release_fight_and_expire`. The banked
##     "+7 wrapper sweep": agent-enumerated ALL class-5 +7 wrappers —
##     the ONLY universal gap was cases 16/17 (now closed); noted
##     residues: m19's wrapper tail zeroes its attack phase on the
##     154-handoff (fresh ctors make it moot for summons) and m21's
##     `sub_268F0` anim tail — both benign APPROX, documented here.
## 12. **FAITHFUL, NO CHANGE — wyverns besieging the pyramid.** The
##     round-2 retaliation verdict SURVIVES the player's refutation:
##     the latch source is the pyramid's own DEVOUR-FIRE — the
##     (10,0) fire it spawns per devoured projectile carries the
##     pyramid's id + wildcard filter and 400/tick, splash-tags any
##     wyvern in reach; `mc2_state_head` latches on SOURCE (amount
##     irrelevant) and both idle and chase arms re-home f146 onto it.
##     Byte-checked faithful end-to-end (EF:13612-13, Events.cpp:
##     570-72, sub_10C80). The wyverns DID take damage — the player
##     couldn't see it (and item 13 made the pyramid look harmless).
##     Self-reinforcing loop: their own bolts get devoured → more
##     fire → dead-set siege. Do NOT special-case.
## 13. **LANDED — pyramid deals player damage now.** The fix-9 raise
##     was missing on the PLAYER arms: retail's player IS a boxed
##     pool wizard and `sub_65580` lifts it like any victim at BOTH
##     the homing aim and the victim-hit relink; the port aimed and
##     landed at raw ctx.pz (the FEET). Fixed: proj.rs homing tz and
##     the relink Player arm now use `ctx.pz + PLAYER_HH` (=100, the
##     box center). Plus the doomsday case-1/2 payload audit: bolts
##     now stamp f68=10/f69=0 (case 1) / f69=23 the (10,23) BLAST
##     (case 2 — the ctor default silently spawned plain fire) and
##     row156=62 (retail's homing row; ctors left 64/63). f44=800
##     verified FAITHFUL (EF:13315/27). Test
##     `mc2_hostile_bolt_lands_at_the_player_box_center` (fun fact
##     it uncovered: player spawn grace wipes ALL damage mail for
##     ~100 ticks — faithful :55367-71 — burn it off in tests).
## 14. **LANDED — morph.rs overflow crash.** Retail's eruption clock
##     `dword_0x10_16` is an **i32** (LevelStructs.h:328) — the port
##     narrowed it into i16 f26, which panics at 32767 (~30 min).
##     `wrapping_add` would be UNfaithful (retail never wraps; the
##     self-latched controller just counts up inert forever — its
##     restart roll is gated on the vortex register it holds).
##     Fixed as `saturating_add`: behaviorally identical since every
##     gate reads > 2500 / < 128 / == 0. Sibling sweep: summit91 has
##     no counter, doomsday counts DOWN — no other post-win-unbounded
##     counters. Test `mc2_summit_vortex_clock_saturates`.
## 15. **LANDED — pyramid opaque again.** GRO read: the bit-23 →
##     raster-2 ladder lives ONLY in `DrawSprites_3E360`'s billboard
##     arm (GRO:3779-3805); the second sprite pass `sub_3FD60` →
##     `DrawSprite_41BD3(2)` (GRO:2205-12 LABEL_70 — the pass the
##     pyramid rides, same family as fix 6's wind-down arm) takes
##     raster mode from the static descriptor ALONE. Port fix:
##     exclude (5,10) from the live_poses bit-23 blend arm
##     (world.rs, presentation-only, no hash).
##
## ROUND-4 (2026-07-19 replay — player: "level 024 near perfect"):
##
## 16. **PLAYER-CERTIFIED round 3**: Vissuluth "very killable",
##     summons killable, level completes. Player tactic note: attack
##     the pyramid alone or its devour-fire recruits the creatures
##     to help you (item 12's faithful loop, now understood).
## 17. **LANDED — the mana-rain DECAY channel.** The post-victory
##     apocalypse fountain must be TIMED window dressing (retail:
##     every rain sphere fades out roughly at the end of its roll-
##     out; permanent mana = unfaithful + a possible overflow risk).
##     The module doc's known APPROX gap closed: rain spheres now
##     carry retail's `byte[1] |= 0x20` (port flag bit 13) + life
##     140, and `ball_tick` grew the retail decay tail (EF:26289-307,
##     the MC2 sphere mover): life ticks down, bit 24 (67% fade) at
##     12, bit-23 ghost at 6, expire at 0; decaying spheres never
##     INITIATE a merge (EF:26268) but can still be absorbed by a
##     live sphere — retail's own retention loophole (magnet/balloon
##     consolidation), which the player calls irrelevant. Balloon-
##     tethered balls return before the decay tail = the pickup-
##     retains behavior. Flag set nowhere else → MC1/goldens
##     untouched. Test `mc2_rain_spheres_decay_and_expire`.
## 18. **FAITHFUL (explained) — the 3-4 frozen fireflies.** The
##     freeze-at-0-life is item 11's standing-corpse law working as
##     ported: a summon killed while still in the site_z-16 HOMING
##     slot (before the engage handoff to its +2 attack) stands
##     dead at f46=1 with no death animation until the pyramid dies
##     (EF:10864-67 verbatim), then expires with a fire puff. Kills
##     landed after handoff die through the model machine normally —
##     hence "no pattern" across hundreds of kills. WATCH: if a
##     frozen one ever persists AFTER the pyramid's death, that
##     would indict the parent probe (scan for live (5,10)) — none
##     expected.
##
## BANKED UNFAITHFUL IMPROVEMENTS (later, opt-in per the
## authenticity matrix; player: "not a priority now"):
## - Exclude CREATURES from the pyramid's attack damage (or its
##   devour-fire's splash) so its crossfire can't recruit an assist
##   army — kills the item-12 loop without touching retaliation law.
## - Give slot-16 summon corpses a normal death animation instead of
##   the retail standing freeze (item 18).
##
## Original round-3 bank entries (pointers as banked):

11. **Pyramid summons FROZEN — unkillable, inert, pile into an
    "impenetrable barrier".** PINNED to the +7-state seam: the
    summon exec (doomsday.rs:864-869) spawns m0/m21/m25/m19 directly
    into their `base+7` action (7/175/207/159 — retail's C7 action
    overrides) with `site_z = 17` (stage tag, doomsday.rs:849). The
    dispatcher's held seam only covers site_z 1..=10|15, so they
    route to the model machines' +7 arms — the EXACT "+7 wrapper"
    family the creature-bank session TODO'd ("sweep ALL class-5 +7
    wrappers for dropped tails", duel-devils memory). Frozen + no
    inbox = the +7 arm parks them and nothing runs the release into
    the live brain. NEXT: trace retail's +7 handlers for kinds
    0/19/21/25 under stage tag 17 (what transitions them to the
    fight states; sub_1D5D0's tag-17 arm?) — likely ONE shared law.
12. **Wyverns proactively besieged and KILLED the pyramid — the
    round-2 "faithful retaliation" verdict is REFUTED by the player:
    the pyramid dealt them no damage** (see 13) **yet they were
    dead-set and finished it.** With the wake fix the pyramid now
    takes any mail, so whoever attacks it can kill it. Split next
    session with an f146 log on live m16s: (a) if PROACTIVE — audit
    our m16 dwelling/wizard scans against the agent's cited filters
    (roster.rs:1444 (10,45)-only; mobs.rs:464 class-3 ≤1) for what
    actually admits the pyramid at runtime; (b) if latched — find
    who mails wyverns with the pyramid's id when its attacks
    otherwise deal nothing (its burst area writes tag creatures
    even if the player-damage path is broken — id-immunity only
    covers its OWN summons, doomsday.rs:848). Related: the pyramid
    death wiping the map's mana (case 0xE walks the sphere family
    to life 140 → decay) looked FAITHFUL (decompile-cited EF:12847-
    54); the "not enough castle mana afterwards" is the intended
    consequence.
13. **The pyramid's attacks deal NO damage to the player** (he casts
    everything, reasonably cadenced — but nothing hurts). Suspect
    the victim=PLAYER impact path: the Player arm of the victim-hit
    relink (proj.rs) lands at ctx raw z and `area_write`'s player
    probe rides `player_overlap` — the same origin-vs-box-center
    family as fix 9, or the burst-vs-player z-window. Trace retail's
    player-hit geometry (the player entity IS a boxed pool wizard in
    retail and sub_65580 raises it like any victim — our headless
    player is poseonly). Also check mc2_arm_proj's PLAYER_TARGET arm
    damage payloads (f44=800 bolts, doomsday.rs:809/816).
14. **Post-death mana-rain crash** (player repro: kept collecting
    the decaying death-fountain mana; balloon pickup un-decays it —
    believed faithful): `morph.rs:462 attempt to add with overflow`
    — `self.ent[i].f26 += 1` on the summit/eruption machine ticks
    UNBOUNDED (i16 overflows ~32k ticks ≈ half an hour, matching
    "after a while"). The module doc already flags that this machine
    "never despawns itself (retail relies on the endgame teardown —
    trace OPEN-1)" — our session keeps simulating after the win, so
    it eventually trips. Fix direction: retail 16-bit wraparound
    semantics (wrapping_add) or the teardown law; check the sibling
    counters in the same machine for the same trap.
15. **Pyramid renders TRANSLUCENT throughout the fight (player: not
    right).** REGRESSION FROM FIX 6: the ctor's faithful
    `flags |= 0x48800001` sets bit 23, and our live_poses blend law
    (world.rs:1390-1401) maps flags bit 23 → blend mode 2 for EVERY
    MC2 entity. Retail's byte[0xE]&0x80 raster override is read in a
    SPECIFIC draw path (GRO:3779-3805, the billboard/particle arm —
    the dual-purpose m26 wraith ghost bit); the pyramid evidently
    draws through a pass that ignores it. Fix direction: exclude
    (5,10) from the bit-23 blend arm (or gate the arm to the classes
    the retail path actually covers — needs the GRO read). App-side
    presentation, no hash.

7. **RESOLVED, NO CHANGE (faithful)** — level 024 fires segments
   {1, 2, 9} (start briefing row0+1, row-0 completion → 2, end → 9);
   baked row 24 = {0, 2, 9}. The hole is seg 1 = the in-level start
   briefing, missing from retail's own segmentation → subtitle-only
   is correct. Clean-audio-override hook can fill it later.

## 1. Dev-spells exception missing from the hand grey-wash (minor)

Commit cad5892 ("Fix spell hands greying") added the retail
canSummon wash to the HUD hand box, but without the dev_spells
exception the CTRL pane already has — so under the G instrument the
hand draws greyed yet casts fine (dev bypasses the afford gate for
real).

- The gate: [ui.rs:1439](../crates/mgc-app/src/ui.rs#L1439)
  `if !bv.castable[sp as usize][level]` → LOCKED_WASH. No dev arm.
- The pane's correct law to mirror: main.rs HUD block —
  `castable[s] = owned && (bv.castable[...] || dev)`.
- FIX: thread `cfg.gameplay.cheat.dev_spells` into `ui::hud_quads`
  (call site in main.rs) and gate the wash `!dev && !castable`.
  Purely presentational; no hash surface.

## 2. prune_owned_jars is a no-op in MC2 (root cause PINNED)

Full agent trace (2026-07-18, opus). SYMPTOM: owned-spell jars still
appear on rival corpses and as authored level jars with the option ON.

ROOT CAUSE: the MC2 prune predicate keys on the WRONG ownership
record. `mc2_spell_token_tick` reads `self.g.mc2_spell_tokens`
(the retail SpellEnabled mask mirror) at
[world.rs:5925-26](../crates/mgc-sim/src/mc1/world.rs#L5925) — but
that mask is only written by the level-start seed (fireball+possess,
cast.rs:1718) and in-level token COLLECT (world.rs:5954). The
authoritative record is the XP book `mc2_book.ent[spell]`, and the
central grant `mc2_adopt_manifestation` (cast.rs:729) sets the book
WITHOUT the mask — so campaign carry (`mc2_grant_plausible`,
cast.rs:702 → dev-grant per spell) leaves the mask at {0,1} every
level. `owned` is false for everything you actually carried → prune
never fires. (MC1 works because its arm reads the real
`player.owned`, world.rs:3842.)

FIX (conservative, hash-safe): inside the existing
`if self.prune_owned_jars` branch only, test
`self.mc2_book.ent[model as usize] != 0` instead of the mask. Do NOT
touch the faithful collect gate at world.rs:5936 (retail SpellEnabled
law, hashed). All jar sites funnel through `mc2_spell_token_tick`
(authored spawns world.rs:5484, rival-corpse scatter rivals.rs:2753,
replenish world.rs:5958) so one predicate covers everything. Add a
regression test: grant via `mc2_grant_plausible` (book set, mask
clear) → jar prunes; mirror `world.rs:12936`.

SIDE FINDING (faithfulness, separate decision): the same mask desync
means a carried spell's jar can be RE-COLLECTED (double
manifestation) — retail's SpellEnabled gate would block it. Root fix
= set the mask bit in `mc2_adopt_manifestation`, but that touches the
FAITHFUL hashed path → needs golden verification (fresh-world goldens
should be invariant since mask=={0,1}==book at pristine; verify
before landing). Confirm retail sets SpellEnabled on every adopt
(grant-side write site near remc2 sub_5C950/adopt; collect-side gate
= EF:55713).

## 3. MC1: possessable dwellings missing health bars

Example: several possessable dwellings in level 2 (map 01). The bar
overlay draws any pose with `life_frac`, published by the sim at
[world.rs:1322](../crates/mgc-sim/src/mc1/world.rs#L1322): the
allowlist is class-5 heads, class-3 models ≤3 (wizard family), and
MC2 buildings (10,45|52|79) — the comment even notes "MC1's (10,45)
is unrelated". MC1's destructible/possessable dwellings are simply
not in the allowlist.

NEXT SESSION: identify the MC1 dwelling (class, model) rows from the
map-01 roster (LESSON: from the LEVEL ROSTER, not decompile names),
check their life/max_life fields actually track damage in our port,
and extend the gate's MC1 arm. Debug-overlay only (health_bars is
render.debug) — no hash surface, but the life_frac closure runs in
live_poses: keep it read-only.

## 4. MC2 plausible-spellbook progression law (player directive)

Current: `plausible_spellbook_mc2`
([campaign.rs:234](../crates/mgc-app/src/campaign.rs#L234)) scans
sibling levels in ARCHIVE-INDEX order `0..meta.level` (documented
stopgap — "MC2 has no campaign-progression data").

DIRECTED LAW: use the CAMPAIGN order now that stitching defined it —
mains 0-24 with secrets 30-34 interleaved after their parent mains
(4/7/11/17/19; the parent table lives in campaign.rs's exit-routing
law):
- Target is a campaign level → scan the campaign-order prefix before
  it (a secret's prefix = mains through its parent + earlier
  secrets).
- Target is NOT a campaign level (dev filler, e.g. 027) → assume
  completion of ALL campaign levels (mains 0-24 + secrets 30-34).
Keep the scroll-XP heuristic as is. App-side only (the instrument is
off in campaign mode).

## 5-6. Wyverns untargetable + Vissuluth inert — ONE ROOT (agent trace)

Full opus trace, 2026-07-18; runtime-verified by booting level 024 in
the sim (all dispositions fired, 60 ticks).

IDENTIFICATION (from the ROSTER, per the project LESSON):
- Wyvern = (5,16), maxLife 60000, row 84, sprite 207. Runtime: 4
  live, `owned=false` (NOT allies — the "avoids like allies" feel is
  a downstream symptom), cruising at alt ≈ 13-16 TILES, damageable
  (crossfire had already chipped two).
- **VISSULUTH = (5,10) — PLAYER CORRECTION 2026-07-18.** The agent's
  first identification ((5,27) hydra) was WRONG — the hydra is an
  ordinary monster, works fine (player-tested on the same level).
  The trap bit the trace despite the warning. The evidence for
  (5,10): both 024 AND 027 author EXACTLY ONE (024: slot 379 at
  (40,213), dis 29 — spawns when the tower falls; 027: slot 152 at
  (182,227), dis 9 — the player's "inaccessible second Vissuluth"),
  and **level 024's stage checkpoint {index 1, stage 379} points at
  his THING SLOT — the kill-him-to-win objective.** Four dis-30
  (5,28) "fastest" guards ring him. Roster row: maxLife 300000,
  sprite 341, static flag 0x48800001, pars 1024,1280 (a huge box —
  contact is NOT his problem). He "sits mostly in place" = the
  ground-clamped static, matches.

RULED OUT (evidence in the trace): owner-immunity (id24=0 authored;
the summoned Wyvern-Army copy IS ally-owned, roster.rs:1347 — don't
confuse them), awake gate (f58=64 from ctor), class/model scan
filters, held-freeze, meteor not being an autoaim candidate (class 9
model 3 IS in the offensive branch, proj.rs:1181).

ROOT (H1, strongly ranked): homing projectiles fail 3-D BOX-CONTACT
with HIGH-ALTITUDE flyers. The victim probe (`victim_scan` →
`ent_overlap`, combat.rs:87-94) needs |Δz|-box overlap; a meteor
lagging in pitch (row-60 turn cap, proj.rs:875-81) against a small
z-box (f84 = sprite r8/2, mobs.rs:104-12) target 16 tiles up never
satisfies it, and `mc2_flyer_tick` then falls through to the
TERRAIN-CONTACT detonation (proj.rs:998-1046) — "explodes on the
ground below". The ground burst's area damage is also 3-D-box gated
(combat.rs:130+) → "barely damaging". H2 (secondary): the faithful
0x71 (~20°) pitch-cone in `mc2_aim_score` (proj.rs:1133-36) may
reject acquisition when not looking near the flyer. H3 (unlikely): a
residual 3-D-for-2-D metric the distance audit missed — none found.

VISSULUTH ((5,10)) — the machine EXISTS: our sim runs him as
`mc2_doomsday_tick` (doomsday.rs, dispatched world.rs:1637; full
16-state script — terrain flattening, summons, projectile DEVOUR,
the hurl-away beam; trace docs/traces/mc2-class5-m10-doomsday.md).
The trace read him as "THE DOOMSDAY PYRAMID, an UNKILLABLE boss
structure" — likely the decompile-naming trap AGAIN: the machine's
repertoire IS the Vissuluth boss fight (he does apocalyptic things
and sits in place). Level 024/027 are night maps (cave gate moot);
the machine dispatches. The player-reported gaps to investigate:
- "INDESTRUCTIBLE": the port treats him as unkillable — but the
  stage objective is kill-slot-379 and retail wins the game on his
  death. Find retail sub_4BD00's damage-intake / death case
  (EF:33965+) — vulnerability windows per script state? the static
  flag 0x48800001's damage semantics? — and wire the kill → win.
- "METEORS EXPLODE AT HIS FEET": very possibly FAITHFUL — the
  machine is an ANTI-MAGIC ZONE that devours incoming class-9
  projectiles (the devour cylinder just got its 2-D fix in the
  distance audit). If retail devours meteors too, the question
  becomes HOW retail lets you hurt him at all (specific spells?
  windows?). Answer before "fixing" anything.
- "DOES NOTHING / DEALS NO DAMAGE": runtime-check whether the
  16-state script actually ENGAGES on level 024 (state transitions,
  the seeded-timer cadence APPROX in doomsday.rs header, the dis-29
  spawn arriving with the right init) — an inert state-0 idle would
  explain the whole report.
The m27 hydra head-gauge law noted by the first trace is real but
IRRELEVANT here (hydra = a separate, working monster).

NEXT-SESSION KEY ACTIONS: (1) the wyvern instrumented aim trace —
(a) does `mc2_autoaim` write f146 = the wyvern? (b) if locked, does
any `victim_scan_at` sub-step ever contact, or always ground-fall-
through at proj.rs:998? Fix would touch `ent_overlap`'s z-test
and/or the ≤128-unit chord-march (proj.rs:955-70) — verify against
retail sub_65610 (EF:62781) homing first. (2) the Vissuluth kill-
path trace against retail sub_4BD00 + a runtime state log of his
script on level 024. Cross-refs: docs/traces/mc2-autoaim.md
§1.1/§2/§8, mc2-class9-flyers.md §0.6, mc2-class5-m10-doomsday.md;
the 2-D scorer fix is already landed (proj.rs:1137-47) — do NOT
re-fix.

## 7. Level-024 narration: subtitles but no audio (likely FAITHFUL)

Verified at bank time: the baked clip bank HAS row 24 = segments
{0, 2, 9} only (seg 0 = 16 s map briefing, from redbook track 26),
and the seg-0 clip is HEALTHY audio (mean −17.3 dB, comparable to
level 23 — NOT a muter casualty). `Audio::play_speech` no-ops on a
missing (row, segment) slot — retail's own zero-length-slot law —
while the subtitle resolves through the independent ETEXT index and
still draws. So if level 024's in-level triggers hand over any
segment ∉ {2, 9}, you get exactly "subtitles, no audio", and RETAIL
DID THE SAME (its CdTracks row has the same holes) — the player's
retail-bug hunch is probably right.

NEXT SESSION: enumerate the segments level-024's stage/narration
triggers actually fire (sim stage table for level 24) and compare
against {2, 9}. If the fired segs are missing from the redbook
segmentation → faithful retail hole; decide whether subtitle-only is
the correct (current) behavior or the clean-audio-override hook
should eventually fill it.
