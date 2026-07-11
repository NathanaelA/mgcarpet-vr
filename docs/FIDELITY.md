# The fidelity record

This is the porting documentation: for every gameplay subsystem, what
the original engine does, what this engine implements, how that was
verified, and where behavior deliberately deviates. It is the durable
companion to [ROADMAP.md](ROADMAP.md) (the working session log): the
roadmap records *how we got here*, this file records *what is true
now*. When a subsystem changes, update its entry in the same change.

The project stance (see the README and the authenticity matrix below):
**the faithful original behavior is always the default and always
available**; every modern improvement is a named, opt-in alternate.

## How to read an entry

Each subsystem entry has five parts:

- **Original** — the retail behavior, with its evidence anchor
  (decompile function/lines, retail observation, recorded gameplay).
  Anchors of the form `sub_NNNNN :LLLLL` refer to the remc1
  decompilation (`sub_main.cpp` line numbers) unless marked remc2.
- **Port** — what we implemented and where it lives
  (`crate::module`).
- **Verified** — the strongest verification the entry has passed, one
  of the grades below.
- **Options** — the authenticity-matrix columns this subsystem
  exposes, with their class (P/G) and defaults. Absent = no options,
  the faithful port is the only behavior.
- **Deviations & interims** — every known divergence from retail:
  deliberate improvements (with their toggle), approximations pending
  a deeper trace, and honest gaps. "None known" is a claim, not a
  hope: anything the next playtest falsifies moves the entry back.

### Verification grades

Ordered weakest to strongest; an entry states the strongest grade it
has actually earned. The senior-source rule: **recorded original
gameplay outranks the decompile** — remc1 is a machine reconstruction
with known transcription errors (truncated tables, mis-fixed lines),
so when retail play contradicts it, retail wins.

1. **decompile-traced** — ported line-by-line from the remc1/remc2
   reconstruction; not yet exercised against the original.
2. **oracle-diffed** — output compared byte-/stream-exactly against
   original-engine output (reference dumps, instrumented DOSBox,
   the mc2-genlevel oracle).
3. **player-validated** — a targeted in-game check by the player
   confirmed the specific behavior.
4. **player-certified** — the player has played the subsystem at
   length and judges it faithful to retail ("as I remember" or
   better); residual deviations are treated as future spottings.
5. **retail-verified** — the specific behavior was reproduced in the
   original game side-by-side (DOSBox/MC1PLUS), the strongest grade.

### Option classes (the authenticity matrix)

Options are enums, not booleans — room for named alternates. Columns:
`mc1` (the faithful MC1 port, default), `mc2` (an MC2 behavior offered
as a faithful alternate in MC1 contexts), `improved` (a deliberate
modern deviation). Two engine-level classes:

- **P-class** (presentation): resolves at render/input time, never
  changes simulation outcomes. Freely flippable.
- **G-class** (gameplay): changes simulation state or RNG consumption.
  Recorded into replays; a replay taped under a non-faithful G option
  is not a faithful fixture.

Current option surface: `mgcarpet.json` + CLI flags (the generated
`mgcarpet.json.defaults` documents every option); an in-game menu is
planned.

---

## Terrain generation (MC1)

**Original.** MC1 levels ship no heightmap — each level stores 12
generator parameters (seed, raise, gnarl, river…) and the engine grows
the world at load: a seeded fractal midpoint field, normalized to
16-bit, classified into terrain types (water/lowland/rock/snow bands),
then shaded. The generator is exactly reproducible from the
parameters; its arithmetic wraps in load-bearing ways (the i16
corner-sum wrap), and one retail level (index 039) hits a degenerate
collapse — an all-negative field normalizing to a flat plateau — which
is plausibly why the campaign's hardcoded skip table exists.

**Port.** `mgc_import::mc1_terrain`, a native Rust port of the remc1
generator (heightmap, type classifier, shading, angle planes). Runs at
bake time; the engine never sees a seed (baked packages carry expanded
grids). Entity-driven terrain modification (craters, walls, building
flattening) is deliberately NOT baked: `mgc_sim::features` applies it
at load, as the original engine does after generation.

**Verified.** retail-verified. Heightmaps reproduce the
previously-oracle-validated reference output near-byte-exactly across
all 143 MC1/HW retail levels (1:1 validation pass, 2026-07-04,
player-checked against DOSBox renders); the level-039 degenerate
collapse reproduces exactly (player-confirmed in DOSBox).

**Options.** None — generation is bake-time and single-truth.

**Deviations & interims.**
- `hmap2` (the original's second heightmap, the water-reflection
  plane) is not derived — needed only by a future reflections render
  pass; rebuilt post-load in the original, so nothing is missing from
  the bake.

---

## Player flight (MC1)

**Original.** remc1 `sub_455D0`: mouse steering is a turn *rate*
(stick-like: offset = rate, not position); aim pitch is absolute;
speed is impulse-based — accelerate/decelerate keys add ±16/tick while
held and the speed *holds* on release (no friction stop); thrust acts
in the level ground plane regardless of aim pitch. Vertical motion is
terrain-follow with a soft ceiling: climb authority is full below
ground+768, fades to zero at ground+1024, inverts above — but level
flight *holds* any altitude reached (the wall-climb move: ride a slope
up, fly off level). The camera pitches at HALF the aim pitch. The
human player is class 3 model 0 with an explicit wall gate
(`sub_45410`) — flying monsters cross walls; the player cannot.

**Port.** `mgc_sim::flight`, verbatim: rate stick, absolute aim
pitch, impulse speed, soft-ceiling climb authority, half-pitch camera,
the wall gate in the flyer. Mouse-forward = dive is the authentic
polarity.

**Verified.** player-certified 2026-07-07 ("as frustrating and
useless as I remember"); the wall-climb altitude acceptance test is a
standing regression test.

**Options.** Three orthogonal enums (`flight.*` in config/CLI),
faithful defaults:
- `thrust` — G-class: `mc1` (default) | `enhanced` (hold-to-fly with
  auto-deceleration; keeps the authentic level-plane thrust rule).
- `altitude` — G-class: `faithful` (default) | `extended-lift`
  (adds explicit float keys E/Q, float-up capped at the level's
  highest terrain; wall blocking intact).
- `bindings` — P-class: `classic` (default; mouse aims, arrows
  accelerate/strafe) | `wasd`.
- `mouse_sensitivity`, `invert_y` — P-class preferences (the
  originals shipped an invert option too).

**Deviations & interims.**
- Camera ROLL is unrendered (the original banks slightly in turns).
- An `mc2` normalize-key thrust tier and the player's torso-aim
  design are banked, not implemented.
- Tick/time rate: the original locked simulation ticks to framerate,
  so game speed varied with resolution — there is deliberately NO
  faithful target here; we run a fixed timestep and will pick
  canonical rate constants (likely MC2's) in a dedicated timing pass.

---

## Spell jars — owned-spell removal (unfaithful improvement)

**Original.** Both MC1 and MC2 leave every spell jar in the world even
when the local player already owns that spell. The pickup gate
(`sub_68FF0` MC2 / the class-12 pickup MC1) flies you *through* an
already-owned jar without collecting it, and AUTHORED/PLACED jars carry
`life = 0` so they never decay. The authors scattered redundant spell
sources as a safety net for players who missed one — but for a player
who already has the spell they become permanent, unidentifiable clutter
(you can't tell what a jar holds without flying into it). Only
DEATH-scattered jars self-cull (life 200-289).

**Improvement (P-class, opt-in; player-directed 2026-07-14).** With
`enhancements.prune_owned_jars` on, any jar whose spell the local player
already owns — and therefore can never pick up — is removed. The
criterion is exactly the retail pickup gate ("owns `s`" = "can't take
it"). Single-player entity removal (in MP, gate on the local human or
make it presentation-only — deferred until MP exists). Implemented as a
per-tick self-cull at each game's jar tick, which covers BOTH the
level-load sweep and the instant the player gains the spell (every jar
of it despawns on its next tick). **This is the lone enhancement that
defaults ON** — player-judged "one no one will ever complain about";
the faithful behavior is a `--no-prune-owned-jars` (config `false`)
away. CLI `--prune-owned-jars` / `--no-prune-owned-jars`.

**Verified.** `owned_spell_jars_are_pruned_when_enabled` (MC1) +
`mc2_owned_spell_tokens_are_pruned_when_enabled` (MC2): with it off the
jar remains, on removes it. The sim default (`World::prune_owned_jars`)
stays OFF so state-hash goldens are unaffected; only the app config
turns it on.

---

*Entries to come (the full subsystem list, in rough dependency
order): terrain features & villages; triggers, events & portals;
monsters (per-model AI); combat & damage channels; projectiles &
autoaim; the 24-spell repertoire (per-spell); mana economy & castles;
player mortality & the castle weapon; rival wizards; map & HUD;
audio; campaign progression & the skip table.*
