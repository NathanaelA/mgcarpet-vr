# MC2 RIVAL BRAIN — port-ready verbatim trace (PER-TICK AI)

Port-ready verbatim trace of the **MC2-native rival wizard PER-TICK AI**: the decision layer, movement,
targeting, and casting. Scope is the brain that runs every frame on an AI-controlled class-3 wizard —
NOT the lifecycle (spawn/init/records/mortality/respawn), which a sibling trace owns.

The project already ships a full MC1 rival AI (`crates/mgc-sim/src/mc1/rivals.rs`, traced in
docs/ROADMAP.md "HOSTILE WIZARDS (RIVAL AI) — TRACE BANK"). MC2 levels currently spawn NO rivals; Phase
4.3b ports them. **HEADLINE: the MC2 brain is the MC1 brain, function-for-function.** Every MC1
`sub_13xxx` handler has an MC2 twin at the same relative address, the state machine is byte-identical in
shape, the personality parameterization is the same three words, and the hate ledger is the same 0x601F
neutral. The deltas are a handful of constants, one genuinely new movement sub (water/obstacle steer
`sub_16580`), and the XP/token economy replacing MC1's jar-learn timer.

All citations to `/home/rain/projects/mgcarpet/reference/remc2/remc2/`: EF = `engine/EventsFunctions.cpp`,
otherwise file:line. Trace date 2026-07-12. Read `mc2-castle-builder.md` for transcription conventions.

---

## Headline findings (read first)

1. **THE AI WIZARD IS THE SAME class-3 ENTITY AS THE HUMAN, distinguished ONLY by `actionIndex_0x45_69`.**
   Human wizard = action **0** (`byte_0x1C1_449` unused); AI wizard = action **1** (EF:43696:
   `a2x->actionIndex_0x45_69 = a1x->IsAiPlayer_0x009_2BE4_11239 == 1`). Action-1's handler is `sub_12910`
   (EF:5243), which is the MC1 rival tick. So there is **no separate AI entity class/model** — same
   `(3, model)` core, just a different per-tick handler index. This matches the project's MC1 design
   (rivals are class-3 model-1, driven by `rival_entity_tick`).

2. **THE BRAIN IS A THREE-FUNCTION SANDWICH, one-to-one with MC1:**
   - `sub_12A70` (EF:5320) = **per-tick housekeeping** (MC1 `sub_132B0`): cooldown decrement, hate decay,
     at-castle grace, regen, the decision-cadence gate, the altitude clamp. Runs EVERY tick.
   - `sub_12E70` (EF:5495) = **the decision selector cascade** (MC1 `sub_136C0`): the priority walk that
     picks `byte_0x1C1_449` (the brain STATE). Runs at the tail of housekeeping and after each state
     handler.
   - `sub_12910`'s `switch(byte_0x1C1_449)` = **the state handlers** (`sub_12E70..sub_161A0`), one per
     brain state, each ending by calling the selector `sub_12E70` again.

3. **`byte_0x1C1_449` IS THE BRAIN STATE (MC1's Type_160+415).** The state set is IDENTICAL to the
   project's `AiState` enum: 0=idle-ish/decide, 1=upgrade-castle, 3=build-castle, 4=possess-ball,
   6=raid-castle, 7=attack-wizard, 8=raid-balloon, 9=hunt-mana, 11=home, 12=cruise, 13=defense,
   14=defense-acquire (see §1.2 table). Same "cut" gaps (2/5/10) with no selector setter.

4. **PERSONALITY IS THE SAME THREE WORDS, loaded AI-only at spawn** (EF:43764-43766) from
   `WizardMapSettings_0x360D2[color]` (BasicTerrain.h:20-34):
   `word_0x242_578 = Aggression`, `word_0x244_580 = Perception`, `word_0x246_582 = Reflexes`, plus
   `word_0x24A_586 = Life` (scales maxLife, EF:43772). **Aggression** drives hate pacing + wealth-scaled
   war thresholds; **Perception** drives `rand%255 < p` notice rolls + aim-cone width; **Reflexes** drives
   decision cadence `byte % (64 − reflexes/4)` + turn rate + burst-lockout length. The map struct carries
   all four in the range 0..255 (int16_t fields, seeded by the editor).

5. **THE HATE LEDGER IS `array_0x1FC_508`, neutral 0x601F, war flag at `[+5]`** — the exact MC1 design
   (MC1's `str_456`, `HATE_NEUTRAL = 24607 = 0x601F`). Below neutral it rises by `Aggression+1`/tick;
   above neutral it decays by `256−Aggression`/tick UNLESS the war flag `[4*color+5]` is set (EF:5377-5393
   = MC1's `rival_hate_decay`). It is fed by the projectile-scan pass `sub_159E0` (EF:7320, the MC1
   `sub_16540` twin): +500 base, +3000 for models {≥12 excl 11}, +1000/+5000 for balloons/creatures
   hitting a CASTLE (EF:7384-7429).

6. **CASTING GOES THROUGH THE SAME SPELL PLUMBING AS THE HUMAN.** The AI cast executor `sub_14E10`
   (EF:6759) is literally `sub_15730`-shaped: it calls `sub_5F660` (the shared cast router) and stamps
   `SpellEnabled[spell] = x_WORD_D3F4C[spell]` (the AI recast cooldown table). It does NOT emit
   projectiles itself — it flips the manifestation on, and the shared class-10/12 spell entities carry the
   owner tag. **Create Castle (verb 2) for the AI runs through the SAME `sub_14E10` case 2u** that the
   human's `sub_15730` case 2u uses (EF:6820 vs EF:6820 — same body), confirming castle-builder §1.3.

7. **ONE GENUINELY NEW MOVEMENT SUB: `sub_16580` (EF:7879), a WATER/OBSTACLE STEER.** MC1's AI carpet
   "ignores walls." MC2's AI carpet runs `sub_16580` after EVERY movement helper — it probes
   `mapTerrainType_10B4E0 == 8` (water) on the four neighbor tiles and snaps `yaw` to a lookup
   (`x_WORD_D3FCE`/`x_WORD_D3FE8`) to steer around water, zeroing speed on a turn. **This is a real MC2
   brain delta the project's `rival_movement` must gain** (see §2.2).

---

## 0. The cast of the MC2 rival brain (function map, MC1 → MC2)

| role | MC2 (EF) | MC1 twin | project rivals.rs |
|---|---|---|---|
| AI wizard tick (action 1) | `sub_12910` :5243 | sub_13170 | `rival_entity_tick`/`rival_alive_tick` |
| per-tick housekeeping | `sub_12A70` :5320 | sub_132B0 | top of `rival_alive_tick` |
| decision selector cascade | `sub_12E70` :5495 | sub_136C0 | `rival_selector` |
| projectile hate feed | `sub_159E0` :7320 | sub_16540 | `rival_add_hate` (folded to intake) |
| movement filter/step | `sub_146F0` :6415 | sub_14EB0 | `rival_movement` |
| **water/obstacle steer** | **`sub_16580` :7879** | — (absent) | **ABSENT — must add** |
| approach helper | `sub_14C90` :6713 | sub_15470 | `rival_approach` |
| cast readiness | `sub_15170` :6887 | sub_15A00 | `rival_cast_ready` |
| cast executor | `sub_14E10` :6759 | sub_155F0 | `rival_cast` |
| spell-usable probe (level scan) | `sub_15F20` :7581 | — (inline) | (folded into readiness) |
| attack pick (wizard) | `sub_15790` :7175 | sub_16030 | `rival_attack_pick(vs_wizard)` |
| attack pick (castle) | `sub_15910` :7246 | sub_16310 | `rival_attack_pick(castle)` |
| STATE handlers | see §1.2 | sub_137xx.. | `rival_state_tick` match arms |

### Selector-condition subs (predicates the cascade walks)

| condition | MC2 (EF) | picks state | project |
|---|---|---|---|
| need a castle | `sub_13B00` :6056 | 3 (build) | selector step 1 |
| flee home hurt | `sub_13DC0` :6163 | 11 (home) | selector step 2 |
| upgrade castle | `sub_13C50` :6107 | 1 (upgrade) | selector step 3 |
| raid enemy castle | `sub_13E40` :6182 | 7 (attack via 6?) → 7 | step 4 |
| attack enemy wizard | `sub_14030` :6233 | 8 | step 5 |
| intercept balloon | `sub_14250` :6292 | 9 | step 6 |
| reactive defense | `sub_15FC0` :7616 | 14 | `rival_defense` |
| possess mana ball | `sub_13CE0` :6122 | 6 | step 7 |
| hunt mana holder | `sub_14530` :6341 | 13 | step 8 |
| idle fallback | `sub_14630` :6383 | 11 or 12 | step 9 |

> **NOTE on state numbering:** MC2's `byte_0x1C1_449` values are NOT identical integers to the project's
> `AiState` discriminants — they are the ORIGINAL's raw state ids. The mapping is by SEMANTICS (which
> state handler each cascade branch selects), tabulated in §1.2. Do not assume the project enum's ordinal
> equals `byte_0x1C1_449`.

---

## 1. THE BRAIN TICK

### 1.1 Per-tick housekeeping `sub_12A70` (EF:5320) — VERBATIM

Runs EVERY tick before the state dispatch. Order of operations:

```c
// EF:5354  sub_12A70(wizard a1x), v1x = a1x->dword_0xA4_164x (the player-extension struct)
sub_15EE0();                                    // scratch: cache last model-0 wizard in x_DWORD_E8840
if (v1x->word_0x1A2_418 < 0) v1x->word_0x1A2_418++;   // BURST lockout recovery (the fireball burst counter)

for (v4 = 0; v4 < 26; v4++)                     // ── per-spell RECAST cooldown decrement ──
    if (SpellEnabled[v4] > 0) SpellEnabled[v4]--;   // array_0x367_871x.SpellEnabled[26] = the AI cooldowns

for (i = 0; i < 8; i++) {                       // ── HATE LEDGER decay, one per player color ──
    if (array_0x1FC_508[4*i+4] < 0x601F) {                       // below neutral → RISE
        array_0x1FC_508[4*i+4] = word_0x242_578 + 1 + array_0x1FC_508[4*i];   // += Aggression+1  (+base [4*i])
        if (array_0x1FC_508[4*i+4] > 0x601F) array_0x1FC_508[4*i+4] = 0x601F; // clamp up to neutral
    }
    if (array_0x1FC_508[4*i+4] > 0x601F) {                       // above neutral
        if (!array_0x1FC_508[4*i+4+1])                           //   war flag [+5] CLEAR →
            array_0x1FC_508[4*i+4] -= 256 - word_0x242_578;      //   DECAY by (256 − Aggression)
        if (array_0x1FC_508[4*i+4] < 0x601F) array_0x1FC_508[4*i+4] = 0x601F;  // clamp down to neutral
    }
}                                               // war flag set → hate PINNED above neutral (no decay)

v2 = 0;                                          // ── AT-CASTLE test ──
v16 = CastleEntityIndex_0x3A_58;
if (v16 && sub_106C0(a1x, castle))  v2 = 1;      // AABB overlap with own castle
if (v2) word_0x159_345 = 2;                      // set grace = 2 while sitting on castle

if (word_0x159_345) {                            // ── SPAWN/AT-CASTLE GRACE ──
    memset(&a1x->str_0x5E_94, 0, 36);            //   DISCARD the whole damage mailbox (36 bytes)
    word_0x159_345--;
}
else if (sub_5EFA0(a1x) == 2) {                  // ── DAMAGE INTAKE (shared human sub) → 2 = LETHAL ──
    a1x->actionIndex_0x45_69 = 2;                //   flip to action 2 = the death fall
    return 0;
}

sub_146F0(a1x);                                  // ── MOVEMENT filter/step (§2.1) ──
if (byte_0x154_340 < 200) byte_0x154_340++;      // "settled" age counter, caps 200

a1x->mana_0x90_144 += a1x->manaRegen_0x88_136;   // ── MANA REGEN (delta applied first) ──
life += lifeRegen_0x163_355; clamp(life, -1, maxLife);
if (dword_0x16D_365) dword_0x16D_365--;          // some 2000-init countdown (post-death immunity window?)

if (v2 || byte[1]&0x10) {                        // ── REGEN RATES: at home OR flagged ──
    manaRegen = maxMana / 200;  if (manaRegen < 1000) manaRegen = 1000;   // home mana /200 (min 1000)
    lifeRegen = maxLife / 200;                                            // home life /200
    byte[1] &= 0xEF;
} else {                                          // afield
    manaRegen = maxMana / 2000; if (manaRegen < 100) manaRegen = 100;     // afield mana /2000 (min 100)
    lifeRegen = maxLife / 500;                                            // afield life /500
}
clamp(mana, 0, maxMana);

// ── DECISION-CADENCE gate (Reflexes) ──
v25 = word_0x246_582;                            // Reflexes
if (!(byte_0x3E_62 % (64 - (v25/4)))) {          // every  (64 − Reflexes/4)  ticks:
    v26x = sub_15CB0(a1x);                        //   scan for a nearby jar/spell scroll to auto-pick
    if (v26x) { sub_15D20(a1x); sub_15D40(v2, a1x, v26x); }   //   (spell acquisition, §4)
    if (life < maxLife) {                         //   HEAL: if hurt, cast HEAL (spell 5) if known & ready
        for (j = SpellIndex[5]; j >= 0; j--)
            if (sub_15F20(a1x, j, 5) == 5) { sub_14E10(a1x, 5u); break; }
    }
}

// ── ALTITUDE HARD CLAMP (behavior-row band) ──
v28 = getTerrainAlt_10C40(&pos);
if (pos.z > v28 + row->word_160_0xa_10)  pos.z = row->word_160_0xa_10 + v28;   // ceiling
if (pos.z < v28 + row->word_160_0xc_12)  pos.z = row->word_160_0xc_12 + v28;   // floor
return 1;
```

**Deltas vs MC1 / vs project:**
- **AI regen** is `life /200 home, /500 afield` (EF:5441/5449) — the project's `rival_alive_tick` uses
  `max/200 home, max/500 afield` (LANDED). Mana `/200 home (min 1000), /2000 afield (min 100)` — the
  project matches. **SAME.** (Survey §mana-economy "AI wizards: life /200 home /500 afield" CONFIRMED.)
- **Grace/at-castle:** the AI at its own castle sets `word_0x159_345 = 2` and MEMSETs the whole 36-byte
  mailbox — i.e. **the AI DISCARDS damage at its castle** (does not forward to the castle). This confirms
  the project's ported asymmetry (`rivals.rs` "AI at its castle DISCARDS damage"). **SAME.**
- **Heal-when-hurt** is on the DECISION-CADENCE tick (only every `64−Reflexes/4` ticks), spell 5, gated by
  `sub_15F20` (owned + affordable + level-usable). The project does this in `rival_alive_tick` (`if think
  && life < maxlife → cast(1)` — NOTE the project casts spell **1** as heal; MC2 heal is spell **5**; MC1
  heal was spell 1 — a **SPELL-INDEX REMAP** the port must honor, see §7 spell-id table).

### 1.2 The state set (`byte_0x1C1_449`) and its handlers

`sub_12910` (EF:5243) dispatches `switch (byte_0x1C1_449)` after running housekeeping:

| `byte_0x1C1_449` | handler (EF) | semantics | MC1 twin | notes |
|---|---|---|---|---|
| 0 | `sub_12E70` (twice) :5255 | DECIDE (run selector) | fresh | case 0 calls the selector directly |
| 1 | `sub_12FF0` :5259→:5579 | **UPGRADE castle**: fly to castle, cast speed-2 en route, hover, cast | sub_13800 | approach 512/2048 |
| 2 | `_nmemneed` :5263 | (death fall — action-2, not brain) | — | placeholder in decompile |
| 3 | `sub_13100` :5267→:5620 | **BUILD castle**: fly to scouted `axis_0x9A_154x`, cast castle | sub_138F0 | approach 2048/4096 |
| 4 | `sub_131F0` :5271→:5658 | **POSSESS**: approach ball 256/2048, claim (`word_0x96_150` target) | sub_13BA0 | |
| 5 | `_nmemneed_0` :5275 | (cut) | — | |
| 6 | `sub_135C0` :5279→:5822 | **RAID CASTLE**: approach 1024/3072, cast possess-1 on it, claim if aimed ≤0x1C | sub_13CA0 | writes `playerEntityIndex` = self on aim |
| 7 | `sub_13710` :5283→:5872 | **ATTACK WIZARD**: approach 2048/3584, on cadence pick+cast via `sub_15910` | sub_13DC0 | castle-raid attack pick |
| 8 | `sub_13830→sub_13890` :5287→:5937 | **RAID BALLOON / general attack**: approach 3328/4608, `sub_15790` pick, STRAFE weave | sub_13DC0 | the strafing combat state |
| 9 | `sub_13870→sub_13890` :5291 | **HUNT-MANA attack** (same body as 8) | sub_13DC0 | |
| 0xB (11) | `sub_133B0` :5295→:5745 | **HOME**: no castle → cast bolt-11/possess-3; has castle far → speed-1; approach 256/2048 | sub_13A70 | |
| 0xC (12) | `sub_13270` :5299→:5680 | **CRUISE**: Perception-gated scroll grab (spell 0x16), else possess-3, else amble | sub_13A10 | |
| 0xD (13) | `sub_13850→sub_13890` :5303 | **general attack** (same as 8) | sub_13DC0 | hunt-mana combat |
| 0xE (14) | `sub_161A0` :5307→:7724 | **DEFENSE**: cast reactive spell 4, weave, re-pick nearest wizard 0xA00..0x1400 | sub_16800 | the incoming-threat dodge |
| default | `sub_12E70` :5311 | DECIDE | | |

Every non-decide case ends with `result = sub_12E70(event)` (EF:5256 etc.) — **the selector re-runs after
every state handler on the same tick**, so a state can immediately hand off. This is the project's
"`rival_state_tick` then `rival_selector`" ordering, and the fresh-runs-twice behavior (project runs the
selector a second time when `state == Fresh`) is the `case 0: sub_12E70(); result = sub_12E70();` double
call at EF:5255-5256. **SAME.**

### 1.3 The decision selector cascade `sub_12E70` (EF:5495) — VERBATIM ORDER

```c
// EF:5516
if (sub_13B00(a1x)) { byte_0x1C1_449 = 3; }              // 1. NEED A CASTLE → build (state 3)
else {
    if (sub_13DC0(a1x)) { byte_0x1C1_449 = 11; return 1; } // 2. HURT (life < maxLife/2) & has castle → HOME (11)
    v2 = word_0x246_582;                                   // Reflexes
    if (!(byte_0x3E_62 % (64 - (v2/4)))) {                 // ── the rest ONLY on the decision-cadence tick ──
        if (sub_13C50(a1x)) { byte_0x1C1_449 = 1;  return 1; }   // 3. UPGRADE castle (1)
        if (sub_13E40(a1x)) { byte_0x1C1_449 = 7;  return 1; }   // 4. RAID enemy castle (7)
        if (sub_14030(a1x)) { byte_0x1C1_449 = 8;  return 1; }   // 5. ATTACK enemy wizard (8)
        if (sub_14250(a1x)) { byte_0x1C1_449 = 9;  return 1; }   // 6. INTERCEPT balloon (9)
        if (sub_15FC0(a1x)) { byte_0x1C1_449 = 14; return 1; }   // 7. reactive DEFENSE (14)
        if (sub_13CE0(a1x)) { byte_0x1C1_449 = 6;  return 1; }   // 8. POSSESS mana ball (6)
        if (sub_14530(a1x)) { byte_0x1C1_449 = 13; return 1; }   // 9. HUNT mana holder (13)
        sub_14630(a1x);                                          // 10. IDLE fallback → home(11) or cruise(12)
    }
}
return 1;
```

**Cadence split (an MC1-identical detail):** steps 1 (need castle) and 2 (flee hurt) run EVERY tick; steps
3-10 run only on the `byte_0x3E_62 % (64 − Reflexes/4) == 0` decision tick. The project's `rival_selector`
takes a `think` bool and gates steps 3+ on it (EF-faithful: `if !think { return; }` after step 2). **SAME.**

**One ORDER delta vs the project's rivals.rs:** MC2 evaluates **defense (`sub_15FC0`) BEFORE possess
(`sub_13CE0`)** (step 7 before step 8). The project runs defense OUTSIDE the cascade (in
`rival_alive_tick` on the think tick, before the state handler) as its own `rival_defense`. Functionally
equivalent (defense is checked on the same tick), but the MC2-native ordering puts it as cascade step 7,
between balloon-intercept and possess. **DIFFERENT placement — reconcile: either keep the project's
pre-cascade defense or move `sub_15FC0` in as cascade step 7 (recommended for fidelity).**

### 1.4 Selector predicate details (VERBATIM math)

**Need a castle `sub_13B00` (EF:6056):** if no castle AND `sub_146C0(a1x, 2)` (owns Create-Castle
manifestation) AND `sub_15730(a1x, 2)` (`maxMana ≥ castle-spell maxMana`), scout a site: a **4×4
supercell sweep** from the wizard's own supercell (`pos >> 14`), each candidate at supercell corner
`(cell&3)<<14`; `sub_14B10(scratch, 2)` finds the nearest OTHER castle; site OK when
`sub_583B0(nearest, cand) > 0x3000` (dist² in the 2-candidate probe). On accept, `a1x->axis_0x9A_154x =
candidate` and return 1. **This is exactly the project's `rival_scout_site`** (4×4 supercell, foreign-castle
distance gate). The project uses `12288²` where MC2 uses `0x3000` on `sub_583B0`'s metric — **VERIFY the
metric: `sub_583B0` is a supercell-scaled distance, not raw 3D; the project's `12288*12288` threshold
should be re-derived from `0x3000` in `sub_583B0` units** (OPEN — see worklist).

**Flee hurt `sub_13DC0` (EF:6163):** `if (maxLife/2 <= life) return 0;` — i.e. flee only when
`life < maxLife/2` AND a castle exists. Sets `word_0x96_150 = castle`, `word_0x98_152 =
sub_14C40(castle)` (the target SIGNATURE). **SAME** as project step 2.

**Upgrade `sub_13C50` (EF:6107):** castle exists AND `castle->actionIndex_0x45_69 == 4` (the buildable
steady state) AND `castle->word_0x30_48 == 0` (not mid-build) AND `sub_155E0(a1x)` (can afford + space +
aim-cone ok). `sub_155E0` (EF:7101) checks `maxMana ≥ castle-spell maxMana`, `sub_11A10(castle)` (space),
and the Perception-scaled aim cone `sub_582B0(yaw,roll) < ((255−Perception)/4 + 20)·2048/360`.
**SAME** as project step 3 (which checks `castle.tick70 == 4` and `castle_upgrade_space_ok`).

**Raid castle `sub_13E40` (EF:6182):** gate `sub_164B0` (owns any of the offense spells
{0x11,0x10,0x12,7,9,0x14,0x13,0x15,0}) AND (has castle OR no castle-spell). Walk the class-3 model-2
chain (`dword_38519`, castles), skip own. **Two accept conditions (OR):**
```c
// EF:6201  hated-and-undefended:
(50000 - Aggression*(ownerMaxMana/10)/255 < hate[4*ownerColor+4])      // hate over wealth-scaled threshold
    && EuclideanDistXY(owner, castle) > 0x3840000                       // owner FAR from its castle (undefended)
    && !sub_106C0(owner, castle)                                        // owner not physically at castle
// OR  plain-poorer (EF:6204):
|| myCastle->mana > 640*(255 - Aggression) + theirCastle->mana         // my stored mana >> theirs + margin
```
Nearest such castle within `row->word_160_0x1c_28²` (behavior-row range). **SAME** as project
`rival_pick_castle_target` (hate+undefended | poorer; `640*(255-agg)` margin CONFIRMED; undefended dist
`0x3840000` ≈ 7680² CONFIRMED).

**Attack wizard `sub_14030` (EF:6233):** gate `sub_15E60` (owns any of {0,7,0x12,0x10,0x14,0x15,9}). Walk
`dword_38519` (all wizards), skip own, model 0/1 only (live carpets), skip if `sub_15760(target, 0xB)`
(target invisible/cloaked, spell 0xB has a live burst). Accept if **war flag set** (`hate[+5]==1`,
EF:6257) OR **hated** (`50000 − targetMaxMana/10·Aggression/255 ≤ hate[+4]`, EF:6263) OR **bully the
homeless rich** (target castle-less AND `targetMana + 32·(255−Aggression) < myMana`, EF:6266). Nearest
within `(range+10)²`. **SAME** as project `rival_pick_wizard_target` (war | hate | bully; `32*(255-agg)`
CONFIRMED; invisible skip CONFIRMED; range+10 CONFIRMED).

**Balloon `sub_14250` (EF:6292):** gate `sub_15E60`. Walk `dword_38519` model-3 (balloons), skip own,
accept if hated (same wealth threshold) AND `10·(275−Aggression) < balloonMana` (cargo gate) AND
`!sub_106C0(balloon, ownerCastle)` (not sitting at its castle). Nearest within range². **SAME** as project
`rival_pick_balloon_target` (`10*(275-agg)` cargo gate CONFIRMED; at-castle skip CONFIRMED).

**Possess `sub_13CE0` (EF:6122):** owns possess (`sub_146C0(a1x,1)`), and IF owns castle-spell then only
while `maxMana ≤ castleSpell->maxMana` (the economy loop — claim mana only until rich enough to upgrade).
`sub_148E0` (EF:6518) picks the ball: walk `dword_38523` (the mana-sphere chain), skip own claims,
model-57 (scroll) gated by `rand%255 < Perception`, at-war owners always eligible, neutral-owned only if
unguarded (`sub_16FC0` finds no owner wizard nearby). **SAME** as project `rival_pick_ball_target` +
the `maxMana <= castle_cost` economy gate.

**Hunt mana `sub_14530` (EF:6341):** gate `sub_15E60`. Walk all 29 creature buckets `bytearray_38403x[i]`,
any other-team creature with `mana_0x90_144 > 0`, nearest to the own castle (anchor = castle, or self if
none). No range cap. **SAME** as project `rival_pick_mana_target` (anchored at castle, no range cap).

**Idle `sub_14630` (EF:6383):** `if (life >= maxLife || no castle) → state 12 (cruise); else → state 11
(home, heal up)`. **SAME** as project step 9.

---

## 2. AI MOVEMENT

### 2.1 Movement filter/step `sub_146F0` (EF:6415) — VERBATIM

Called every tick from housekeeping. **This is the MC1 `sub_14EB0` twin — no wall gate, no drag, no
knockback — plus MC2's strafe channel.**

```c
// EF:6442
if (byte[1] & 8) { byte[1] &= 0xF7; }            // one-frame "teleported" skip
else {
    predicted = a1x->position;
    v6 = getTerrainAlt(&predicted);
    sub_580E0(&predicted, v6, row0xc, row0xa, row0xe);   // BAND-SETTLE altitude from behavior row (§2.3)
    MoveEntity_57FA0(&predicted, yaw, 0, actSpeed);      // FORWARD step (always level, pitch=0)
    MoveEntity_57FA0(&predicted, yaw+512, 0, strafeSpeed_0x10_16);   // STRAFE step (yaw+90°)
    strafeSpeed -= 4 * sign(strafeSpeed);                // strafe decay 4/tick toward 0
    CopyEntityPosition(a1x, &predicted);                 // COMMIT (no wall test)

    actSpeed += 16 * sign(speed_0xc_12 - actSpeed);      // accel 16/tick toward desired speed_0xc_12

    // ── TURN toward desired heading (roll_0x20_32), Reflexes-scaled rate ──
    v14 = sub_582B0(yaw, roll & 0x7ff);                  // angular error yaw→roll
    v15 = 255 - Reflexes;
    v16 = v14 / ((v15/16) + 8);                          // turn step = err / (8 + (255-Reflexes)/16)
    v18 = clamp(v16, row->word_160_0x4_4, row->subtype_160_0x2_2);   // clamp to behavior-row turn caps
    v19 = sub_582F0(yaw, roll) * v18;                    // signed turn
    yaw = (yaw + v19) & 0x7FF;
    // overshoot guard: if the turn crossed past roll, snap to roll (EF:6507-6511)
}
```

- **Desired heading = `roll_0x20_32`** (the AI reuses `roll` as its steering setpoint; the state handlers
  write it via `sub_581E0_maybe_tan2(pos, target)`). Actual heading = `yaw_0x1C_28`. Turn rate =
  `err / (8 + (255−Reflexes)/16)`, clamped to the behavior row's `[word_0x4, subtype_0x2]` caps. **The
  project's `rival_movement` uses `err / (8 + (255-tempo)/16)` — SAME formula, tempo == Reflexes.**
- **Strafe channel** (`strafeSpeed_0x10_16`, step written by combat states, decay −4/tick, at yaw+90°) —
  the project models this as `jink` (impulse 80, decay 4/tick). **SAME concept; NOTE the MC2 impulse is
  written by the combat state's weave (§2.4), value `3·minSpeed·Reflexes/255` in `sub_13890`, NOT a flat
  80.** The project's flat 80 jink is an MC1 approximation — **re-pin to the MC2 weave value.**
- **NO wall gate.** `CopyEntityPosition` commits unconditionally. The carpet clips through terrain
  exactly like MC1's AI. **SAME.**

### 2.2 THE NEW WATER/OBSTACLE STEER `sub_16580` (EF:7879) — VERBATIM (MC2-ONLY)

Called at the END of EVERY state handler (after aim/approach, before return). **MC1 has no counterpart.**

```c
// EF:7879
v1 = yaw_0x1C_28;
v3 = 0;
// str_611_byte_0x45E_1118 = a per-wizard "avoidance FSM" counter
if (byte_0x45E_1118 <= 2 || byte_0x45E_1118 >= 8) {
    v4 = (byte_0x45E_1118 <= 7) ? sub_169C0(a1x) : 0;    // sub_169C0 = probe the terrain ahead
} else v4 = 3;
switch (v4) {
  case 0: byte_0x45E_1118 = 0; return 0;                 // clear ahead → no steer
  case 1: v8 = sub_16730(a1x, 0); if (v8) yaw = x_WORD_D3FCE[v8]; goto commit;   // steer LEFT lookup
  case 2: v8 = sub_16730(a1x, 1); if (v8) yaw = x_WORD_D3FE8[v8]; goto commit;   // steer RIGHT lookup
  case 3..8: byte_0x45E_1118++; return 0;                // in a committed avoidance arc, count down
  default: commit:
    if (v8) {
        v3 = 1;
        byte_0x45E_1119 = v8;                            // remember the chosen exit
        if (v1 != yaw) { actSpeed = 0; speed_0xc_12 = 0; word_0xe_14 = 1; }   // TURN → full stop
        roll_0x20_32 = yaw;                              // realign setpoint
    }
}
return v3;
```

- `sub_16730` (EF:7955) probes the four neighbor tiles for `mapTerrainType_10B4E0 == 8` (**water**) and
  builds a 4-bit obstruction mask, then indexes `x_WORD_D3FCE`/`x_WORD_D3FE8` (yaw lookup tables) for the
  escape heading. `sub_169C0` classifies the situation (0 clear / 1 left / 2 right / 3+ committed).
- **Effect:** the MC2 AI carpet actively AVOIDS water (terrain type 8), unlike MC1's AI which flies over
  everything. On a detected obstacle it snaps yaw to a table heading and **zeroes speed for that tick**
  (the same speed-kill the MC2 player commit gate does, survey §wall-gate). `byte_0x45E_1118`/`_1119`
  hold the avoidance micro-FSM state.
- **PORT: this is a REQUIRED new behavior for the MC2 rival column.** The project's `rival_movement` must
  gain a post-step `mc2_ai_water_steer` that replicates the four-neighbor type-8 probe + yaw-table snap +
  speed-zero. The lookup tables `x_WORD_D3FCE`/`x_WORD_D3FE8` and `sub_169C0`/`sub_16730` need a verbatim
  transcription pass (OPEN — see worklist; both are short).

### 2.3 Altitude law

- Movement band-settle: `sub_580E0(&pos, groundAlt, row0xc, row0xa, row0xe)` (EF:6454) inside `sub_146F0`
  — same three-zone settle as MC1 (above `row0xa` fall by `row0xe`; between `row0xa..0xc` slow settle;
  below `row0xc` snap up). The project's `rival_movement` band-settle (`sub_42000` port) matches.
- Housekeeping HARD CLAMP (EF:5482-5486): `z ∈ [ground + row0xc, ground + row0xa]`, applied AFTER the
  movement step every tick. **SAME** as project's altitude hard clamp.
- Combat-state hover: each attack state nudges `z` toward `target.z + 512` by `±row->word_160_0xe_14`
  (e.g. EF:5596, :5854, :6035). The project's `rival_hover_toward` matches (`target z + 512`, step
  `row.v_14`). **SAME.**

### 2.4 Approach / retreat / strafe patterns

- **Approach `sub_14C90` (EF:6713):** `dist ≤ arriveR → stop (speed=0, word_0xe=1, return 1)`; `dist >
  boostR AND owns speed-up (spell 3) → cast speed-up`; else `speed = minSpeed, word_0xe=1, return 0`.
  **SAME** as project `rival_approach` (arrive/boost radii, boost casts spell 2 — NOTE MC1 speed-up was
  spell 2, MC2 speed-up is spell **3**; another spell-id remap, §7).
- **Strafe weave** lives in the general-attack state `sub_13890` (EF:5937): while in range and
  `word_0x1A2_418 ≥ 0` (not burst-locked), `sub_15790` picks a spell; on the WHIFF path it runs a
  weave micro-FSM on `str_611_byte_0x45D_1117` (a 0..20 counter): bands `<3` and `3..5` and `5..20` and
  `≥20` drive alternating `yaw ± 512` jinks and set `actSpeed = 3·minSpeed·Reflexes/255` (EF:5997-6000).
  **This is the real strafe impulse** — the project's flat `jink = 80` is an MC1-shaped approximation.
- **Castle-homing:** state 11 (`sub_133B0`) flies to the castle (approach 256/2048), casting speed-up en
  route; if no castle it casts the escape/possess spells and ambles. **SAME** as project `AiState::Home`.

---

## 3. AI CASTING

### 3.1 Cast readiness `sub_15170` (EF:6887) — VERBATIM (per-spell-class gates)

`sub_15170(a1x, spell)` returns the manifestation entity if the AI MAY cast `spell` right now, else 0.
It reads the spell's per-LEVEL cost from the SPELLS table (`SPELLS_BEGIN_BUFFER_str[spell].subspell[
manifest->byte_0x46_70]`), where `byte_0x46_70` = the AI's current level for that spell:

```c
// generic per-class checks (all cases):
maxMana >= subspell.maxManaLimit_A     // the castle-stored / ceiling unlock gate
&& mana  >= subspell.manaCost_6         // affordable NOW
&& !SpellEnabled[spell]                 // recast cooldown clear (SpellEnabled == cooldown counter)
&& mana  >= manifest->maxMana_0x8C_140  // (a second per-manifestation ceiling)
```

Then per spell CLASS an aim cone or state gate:
- **Precision-aimed {0,7,0xD,0xE,0x16}** (EF:6947): aim cone `sub_582B0(yaw,roll) <
  ((255−Perception)/4 + 20)·2048/360`.  ← Perception widens the cone.
- **Homing-aimed {1,9,0x10,0x12,0x13,0x15}** (EF:6979): manifest not mid-burst (`word_0x2E_46 == 0`) +
  same Perception cone.
- **Castle {2}** (EF:7014): space check `sub_11A10(castle)` + Perception cone + `maxMana ≥ manifest maxMana`.
- **Possess {3}** (EF:7051): affordable, no aim cone.
- **Buff/self {4,6,8,0xB}** (EF:7065): affordable, not mid-burst, no cooldown, no aim.
- **{5,0xA,0xC,0xF,0x11,0x14}** (EF:7085 → LABEL_43): the "big" spells (`a2 ≥ 0x1A` path), affordable +
  ceiling only.

**The project's `rival_cast_ready` folds this into one function** (owned + cooldown + mana + castle_req +
Perception-scaled aim cone `((255-acc)/4+20)`). **SAME formula** (project `acc` == Perception ==
`word_0x244_580`). The per-spell-class SWITCH is the detail the project flattens; the cone width formula
`((255−Perception)/4 + 20)·2048/360` is CONFIRMED verbatim (EF:6974).

### 3.2 Cast executor `sub_14E10` (EF:6759) — VERBATIM

```c
// EF:6792
if (!sub_15170(a1x, a2)) return 0;          // re-check readiness
byte[1] &= 0xFE;                             // clear a "wants-to-cast" bit
switch (a2) {
  case 0,1,7,0x16:                            // ── PRECISION-AIMED (fireball family) ──
    v6x = sub_146C0(a1x, a2);                 //   the manifestation
    if (!v6x || word_0x1A2_418 < 0 || sub_582B0(yaw,roll) >= 0xAA) return 0;   // burst-locked or off-aim
    pitch = sub_58210_radix_tan(&pos, &target);          // absolute aim pitch
    word_0x1A2_418++;                                    // BURST counter ++
    if (word_0x1A2_418 >= 8)                             // 8 shots → LOCKOUT
        word_0x1A2_418 = ((Reflexes - 255)/8) - 1;       //   negative = (Reflexes-255)/8 - 1 ticks
    if (sub_5F660(a1x, v6x, 0) != 1) return 0;           // ── SHARED CAST ROUTER (same as human) ──
    SpellEnabled[a2] = x_WORD_D3F4C[a2];                 // arm the recast cooldown
    return 1;
  case 2:  // ── CREATE CASTLE (identical to human sub_15730 case 2) ──  (EF:6820)
    ...  sub_5F660(castle upgrade) OR IfSubtypeCallCreatingManaSphere(3,2) for first build ...
  case 3:  // POSSESS: sub_5F660, arm cooldown, return 1
  case 4,9,0xD,0xE,0x12,0x13,0x15:  // ── HOMING-AIMED ──  cone < 0xE3, aim pitch, sub_5F660
  case 5,6,8,0xA,0xB,0xC,0x10,0x11,0x14:  // ── SELF/BUFF/BIG ── just sub_5F660 + cooldown
  case 0xF: return 0;                        // spell 0xF never AI-cast
}
```

- **Cast delivery goes through `sub_5F660`** — the SAME shared cast router the human uses (castle-builder
  §4 confirms `sub_5F660` is the re-cast/create router). The AI does NOT have a private emitter; flipping
  `SpellEnabled[spell]` + calling `sub_5F660` spawns the manifestation/projectile with the owner tag, and
  the generic class-9/10/12 plumbing serves it. **CONFIRMS the project's design** (rivals emit through the
  shared spawners).
- **Recast cooldown = `x_WORD_D3F4C[26]`** (EF:1070):
  ```
  {2,10,40,32,300,1,1,1,1,4,1,1,0,0,0,0,0,0,400,600,600,400,400,0,0,0}
  ```
  This is stamped into `SpellEnabled[spell]` and decremented in housekeeping. **This is the MC2 AI_RECAST
  table.** The project's `AI_RECAST` is the MC1 table `{2,1,32,10,1,0,0,4,400,0,1,0,1,0,1,1,40,600,...}` —
  **DIFFERENT VALUES, and indexed by the MC2 spell ids.** Must swap wholesale (§7).
- **Burst gun:** precision spells share ONE burst counter `word_0x1A2_418` (NOT per-spell): 8 shots, then
  lockout of `(Reflexes−255)/8 − 1` (negative counts up in housekeeping). Aim cone for firing = `0xAA`
  (≈30°); homing cone = `0xE3` (≈40°). **The project's `burst` (8 shots, lockout `(tempo-255)/8-1`, cones
  0xAA/0xE3) is CONFIRMED verbatim** (EF:6806, :6858, :6815). **SAME.**

### 3.3 Attack-spell pickers `sub_15790` (wizard) / `sub_15910` (castle) — VERBATIM

Both open with the **POVERTY LATCH** on `word_0x1A4_420`:
```c
// EF:7190  (sub_15790) / EF:7257 (sub_15910)
v1 = maxMana / 4;
if (v1 <= mana) {                            // above quarter
    if (word_0x1A4_420) {                    //   latched poor?
        v3 = min(v1 + 6000, maxMana/2);      //   release threshold = maxMana/4 + 6000, capped maxMana/2
        if (v3 <= mana) word_0x1A4_420 = 0;  //   recovered → release
    }
} else word_0x1A4_420 = 1;                   // below quarter → LATCH poor
if (word_0x1A4_420) return -1;               // poor → hold
```
**This is the project's `poverty` latch verbatim** (`< max/4` latch, release `min(max/4+6000, max/2)`).
**SAME.**

Then the priority walk over a spell-id table:
- **`sub_15790` (attack a wizard):** if the target holds a live spell-8 (`sub_15760(target, 8)` — target
  buffing) and `rand%255 < Perception`, prefer spell **7** (anti-buff). Then walk `unk_D3F80x`:
  ```
  unk_D3F80x[9] = { 0x10, 0x12, 0x09, 0x07, 0x14, 0x15, 0x13, 0x00, 0xFF }
  ```
  Spell **0x13** is only picked if the target is a wizard body (model 0/1). Returns the first owned,
  ready, level-usable spell (`sub_15F20`).
- **`sub_15910` (attack a castle):** walks `unk_D3F89x`:
  ```
  unk_D3F89x[8] = { 0x10, 0x12, 0x07, 0x09, 0x11, 0x14, 0x00, 0xFF }
  ```
- **Defense `sub_15FC0`'s reactive table** `unk_D3F91x[5] = { 0x02, 0x13, 0x19, 0x10, 0xFF }`.

**The project's `rival_attack_pick` uses the MC1 order `17 → 8 → (15) → 7 → 20 → 0 → 15` with an
anti-rebound insert.** The MC2 tables are DIFFERENT spell ids and DIFFERENT order (`0x10, 0x12, 9, 7, 0x14,
0x15, 0x13, 0`). **MUST swap the priority tables to the MC2 ids AND re-derive the anti-rebound insert:**
MC2's anti-buff is the "target holds spell-8 → prefer 7 at Perception%" branch (EF:7209), not MC1's
rebound-→-lightning. Port `unk_D3F80x`/`unk_D3F89x`/`unk_D3F91x` verbatim (§7).

### 3.4 Aim / lead model

The AI aims by writing `roll_0x20_32 = sub_581E0_maybe_tan2(pos, target)` (2-D bearing) as the steering
setpoint, and at cast time sets `pitch_0x1E_30 = sub_58210_radix_tan(pos, target)` (the 3-D pitch to the
target). **No lead/prediction** — it aims at the target's CURRENT position; the projectile's own class-9
homing does the tracking. **SAME** as the project (`rival_face_target` + `pitch_toward`, no lead). This
matches the PLAYTEST-12 "mouse-offset lead DISCONFIRMED" finding for the human — the AI has none either.

### 3.5 Possession use (MC2 flagship)

The AI absolutely uses possession — it is spell **3** (possess ball) via the POSSESS state (`sub_131F0`,
selector `sub_13CE0`) and **spell 1** (the possess/claim in `sub_14E10` case 1). Raid-castle (`sub_135C0`)
casts possess-1 on an enemy castle and, once aimed within `0x1C`, writes `castle->playerEntityIndex =
self` (steals it). **SAME concept as project `AiState::Possess`** — but NOTE the MC2 spell-id split
(3 = possess mana ball, 1 = the aggressive possess/claim). §7.

### 3.6 Castle create / upgrade decisions

- Build (state 3 `sub_13100`) and upgrade (state 1 `sub_12FF0`) both fly to the castle/site and call
  `sub_14E10(a1x, 2)` (Create Castle). `sub_14E10` case 2 (EF:6820) branches: has castle →
  `sub_5F660(upgrade)`; no castle → `IfSubtypeCallCreatingManaSphere(3,2)` (direct spawn). **This is the
  re-cast upgrade ladder** — castle-builder §4's `sub_60480` runs on the upgrade side.
- Upgrade gate: `sub_155E0` (EF:7101) requires `castle->actionIndex == 4` (buildable), `sub_11A10` space,
  `maxMana ≥ castleSpell maxMana`, Perception aim cone. **SAME** as project step 3.
- **NO free-instant AI castle at RUNTIME.** MC1's AI got its first castle free/instant. In MC2 the runtime
  path is the SAME as the human (spawn + build passes). The only "free" castle is the LEVEL-LOAD authored
  starting castle (EF:43777-43809, castle-builder §1.2), which is shared human/AI. **DIFFERENT vs MC1 —
  the project's `rival_cast_castle` free-plant path is an MC1 relic; MC2 casts the real thing.**

### 3.7 Cave-In / flood / doomsday participation

**NONE from the brain.** No AI selector branch or cast picker references the terrain-catastrophe spells
(quake/flood/volcano/doomsday, the class-10 model-45/67 family). The attack tables `unk_D3F80x`/`89x`/`91x`
contain only combat/utility spell ids (0,7,8,9,0x10..0x16). The AI weighs offense/possess/castle/heal/
speed/cloak only. **Terrain catastrophes are player/objective-driven, not AI-cast.** (Consistent with the
MC2 roster sweep: doomsday is the endgame spawner, not a rival tactic.)

---

## 4. AI SPELL LEARNING / XP

> **RETRACTION (2026-07-12, docs/traces/mc2-rivals-open-closure.md §3):** the "pickup" reading below is
> WRONG — `sub_15CB0`/`sub_15D20`/`sub_15D40` are a REACTIVE ANTI-PROJECTILE DEFENSE (scan class-9 aimed
> at self, jink, cast shield/recover), not a spell acquisition chain. MC2 AI spell learning is 100%
> LOAD-TIME (`InitialiseSpells_54A50`). The XP half of this section stands.

**MC2 REPLACES MC1's 200-tick jar-learn timer with a pickup + XP model — and the AI participates in BOTH.**

- **Pickup (not a timer):** on the decision-cadence tick, housekeeping calls `sub_15CB0` (EF:7435) which
  scans for a nearby jar/scroll whose `word_0x96_150 == self.id` (a spell entity keyed to this wizard) and,
  if found, `sub_15D20` + `sub_15D40` acquire it. The Perception roll `rand%255 < Perception` in the ball
  picker (`sub_148E0` EF:6546) gates model-57 SCROLL pickup. **So the AI LEARNS by physically claiming
  spell entities, exactly like the human — NOT by the MC1 "any-jar-exists 200-tick" conjure.** The
  project's `rival_learn_tick` (200-tick arm from any existing jar) is an **MC1 mechanism that MUST be
  REPLACED** for MC2 with the pickup path.
- **The starting book / learn mask** comes from `WizardMapSettings.StartingSpells_0x360E1x[26]` and
  `BlockedSpells_0x36115x[26]` (BasicTerrain.h:28,30) — the editor-authored per-wizard spell allowances.
  (`byte_0x360FBx[26]` is a third per-spell field, likely starting LEVEL.)
- **XP accrues the SAME way as the human.** The AI's casts run through `sub_5F660` → the shared effect/
  impact handlers → `sub_6D8B0` (spell-XP primitive, the prior-trace "sub_6D8B0 = spell-XP"). There is NO
  AI-specific XP path; every award the human earns per-EFFECT (fireball +1/hit, possess +1, castle +1 on
  build, terrain spells += tiles) the AI earns identically because it uses the same spawners. Spell LEVEL
  for the AI is read as `manifest->byte_0x46_70` in the cost lookups (`sub_15170`), i.e. the same
  per-spell level the XP ladder maintains. **The project must let the XP decorator fire for rival casts
  too** (owner-tagged) rather than only the human's.

---

## 5. AI ECONOMY BRAIN (the DECISIONS)

- **The economy loop is the possess-vs-upgrade gate.** In `sub_13CE0` (possess selector), the AI claims
  mana balls ONLY while `maxMana ≤ castleSpell->maxMana` (EF:6135) — i.e. it collects mana until it is
  rich enough to afford the NEXT castle stage, then STOPS claiming and switches to upgrade. Because the
  castle-spell's `maxMana` is the capacity-ladder cost at the current level (castle-builder §7), claiming
  RE-OPENS after every upgrade. **This is the project's economy loop verbatim.** SAME.
- **Bank vs fight:** the POVERTY LATCH (§3.3, `word_0x1A4_420`) stops ALL attack casting below `maxMana/4`
  and holds until `min(maxMana/4+6000, maxMana/2)`. So a poor AI stops attacking and reverts to
  possess/hunt-mana to refill. **SAME.**
- **Mana collection behavior:** claim balls (possess state), hunt mana-holding creatures (hunt state,
  anchored at the castle), intercept fat enemy balloons (balloon state, cargo `> 10·(275−Aggression)`).
  There is **no "send a balloon out" AI** in the brain — balloon spawning is castle/economy plumbing
  (castle-builder §6 `sub_60400`/`AddBallon`), not a brain decision. The AI TARGETS enemy balloons but
  does not dispatch its own. **SAME as project.**
- **At-home banking:** the AI heals + regens fast at its castle (life/mana /200), and the HOME state
  (`sub_133B0`) is entered when hurt (< maxLife/2) or idle-with-castle-and-hurt. **SAME.**

---

## 6. MC2-brain-specific behaviors surfaced by the sweep

- **Water avoidance (`sub_16580`, §2.2):** the one substantive new behavior — the AI carpet steers around
  terrain type 8. **Not cave-specific** (it is on the base movement path), but it interacts with cave
  water pools.
- **Perception-gated scroll spotting:** model-57 SCROLLS are only noticed at `rand%255 < Perception`
  (EF:6547, ball picker; EF:5692, cruise-state scroll grab of spell 0x16). This is the survey's
  "Perception = rival-brain input only, invisible model-57 spotting :5692/:6546" — CONFIRMED. **SAME**
  as the project's scroll Perception gate.
- **Objective awareness: NONE in the brain.** The AI has no read of the stage/objective engine
  (`sub_58F00`). It plays the generic build-fight-collect loop regardless of the level's win condition.
- **Fleeing:** the HOME state IS the flee (entered at `life < maxLife/2`), and the DEFENSE state
  (`sub_161A0`/`sub_15FC0`) is the dodge-and-reactive-cast. **Cloak-while-fleeing:** the HOME handler
  casts spell 0xB (invisibility) — the MC2 cloak spell id, vs MC1's spell 12. §7.
- **No night/cave BEHAVIOR gate found.** The prompt's anchored "(3,0) wizard cave behavior row 104-vs-66":
  see §8 CORRECTION — the behavior-row swap is a MOVEMENT-TABLE (creature) fixture, NOT a rival-brain gate.

---

## 7. FIELD-HOMES TABLE + spell-id remap

### 7.1 Brain field homes (retail → project)

| retail field | meaning | project (rivals.rs) |
|---|---|---|
| `actionIndex_0x45_69` | 0 = human tick, 1 = AI tick, 2 = death fall, 3 = dead | `Ent.tick70` (1/2/3) |
| `dword_0xA4_164x` | the player-extension struct (Type_str_164) | the `Rival` record |
| `byte_0x1C1_449` | **brain STATE** | `Rival.state` (AiState) |
| `word_0x242_578` | **Aggression** | `Rival.agg` |
| `word_0x244_580` | **Perception** | `Rival.acc` (named "accuracy") |
| `word_0x246_582` | **Reflexes** | `Rival.tempo` |
| `word_0x24A_586` | Life scale (maxLife ·L>>8) | (entity max_life) |
| `array_0x1FC_508[4*c+4]` | **hate[color]** (neutral 0x601F) | `Rival.hate[8]` |
| `array_0x1FC_508[4*c+5]` | **war flag[color]** | `Rival.war[8]` |
| `array_0x1FC_508[4*c+0]` | hate base bias (added on rise) | (folded into rise) |
| `SpellEnabled[26]` (array_0x367_871x) | **per-spell recast COOLDOWN** | `Rival.cooldown[]` |
| `SpellsEnabled_0x333_819x[26]` | owned manifestation slot per spell | `Rival.owned[]` |
| `SpellIndex[26]` (SpellLevels_0x41D) | per-spell current LEVEL | (manifest byte_0x46_70) |
| `word_0x1A2_418` | **burst counter** (8 shots, neg lockout) | `Rival.burst` |
| `word_0x1A4_420` | **poverty latch** | `Rival.poverty` |
| `word_0x96_150` | current TARGET entity index | `Rival.target` |
| `word_0x98_152` | target SIGNATURE (`id+model+class<<7`) | `Rival.target_sig` |
| `axis_0x9A_154x` | scouted BUILD SITE (also cast anchor) | `Rival.site` |
| `roll_0x20_32` | desired-heading SETPOINT | `Rival.f34`(vdes-heading) |
| `strafeSpeed_0x10_16` | strafe/weave channel | `Rival.jink` |
| `speed_0xc_12` | desired speed | `Rival.vdes` |
| `word_0x159_345` | spawn/at-castle GRACE | `Rival.grace` |
| `byte_0x154_340` | settled-age counter (caps 200) | — |
| `str_611_byte_0x45D_1117` | strafe-weave micro-FSM | (folded into jink) |
| `str_611_byte_0x45E_1118/1119` | **water-steer micro-FSM** | ABSENT — add |

### 7.2 Constants

| constant | value | cite |
|---|---|---|
| hate neutral | 0x601F (24607) | EF:5377 |
| hate rise/tick | Aggression + 1 (+ base bias) | EF:5379 |
| hate decay/tick | 256 − Aggression | EF:5389 |
| war threshold | 50000 − targetMaxMana/10·Aggression/255 | EF:6263,:7399 |
| decision cadence | byte_0x3E_62 % (64 − Reflexes/4) | EF:5460,:5534 |
| turn rate | err / (8 + (255−Reflexes)/16), clamped to row caps | EF:6488-6501 |
| aim cone (precision fire) | 0xAA | EF:6806 |
| aim cone (homing) | 0xE3 | EF:6858 |
| aim cone (readiness) | ((255−Perception)/4 + 20)·2048/360 | EF:6974 |
| burst limit | 8 shots | EF:6813 |
| burst lockout | (Reflexes − 255)/8 − 1 ticks | EF:6815 |
| poverty latch | mana < maxMana/4 | EF:7191 |
| poverty release | min(maxMana/4 + 6000, maxMana/2) | EF:7195-7198 |
| regen home | life /200, mana /200 (min 1000) | EF:5440-5443 |
| regen afield | life /500, mana /2000 (min 100) | EF:5448-5451 |
| recast table `x_WORD_D3F4C[26]` | {2,10,40,32,300,1,1,1,1,4,1,1,0,0,0,0,0,0,400,600,600,400,400,0,0,0} | EF:1070 |
| stamp-spell table `x_WORD_D3F4C` | (same table = the cooldown stamped into SpellEnabled) | EF:6818 |
| wizard-attack priority `unk_D3F80x` | {0x10,0x12,0x09,0x07,0x14,0x15,0x13,0x00,0xFF} | EF:1071 |
| castle-attack priority `unk_D3F89x` | {0x10,0x12,0x07,0x09,0x11,0x14,0x00,0xFF} | EF:1072 |
| defense-reactive `unk_D3F91x` | {0x02,0x13,0x19,0x10,0xFF} | EF:1073 |
| raid-castle undefended dist² | 0x3840000 (≈7680²) | EF:6202 |
| poorer-castle margin | 640·(255 − Aggression) | EF:6204 |
| bully margin | 32·(255 − Aggression) | EF:6266 |
| balloon cargo gate | 10·(275 − Aggression) | EF:6315 |
| approach: build | 2048 / 4096 | EF:5627 |
| approach: upgrade | 512 / 2048 | EF:5589 |
| approach: possess-ball | 256 / 2048 | EF:5665 |
| approach: raid-castle | 1024 / 3072 | EF:5838 |
| approach: attack-wizard | 2048 / 3584 | EF:5883 |
| approach: general-attack | 3328 / 4608 | EF:5956 |
| approach: home | 256 / 2048 | EF:5806 |
| claim aim gate | sub_582B0 < 0x1C | EF:5849 |
| strafe weave speed | 3·minSpeed·Reflexes/255 | EF:5997 |

### 7.3 SPELL-ID REMAP (MC1 → MC2) — CRITICAL for the port

The MC2 spell verb ids DIFFER from MC1's. The AI subs address spells by MC2 verb id. Cross-referencing
the cast cases in `sub_14E10`/`sub_15170`/`sub_15E60`/`sub_164B0`:

| role | MC2 verb id | MC1 verb id (project uses) |
|---|---|---|
| possess mana ball | 1 (`sub_146C0(a1x,1)`), and 3 = the ball-claim | 3 |
| Create Castle | **2** | 16 |
| speed-up | **3** (approach boost) | 2 |
| heal | **5** | 1 |
| invisibility / cloak | **0xB (11)** | 12 |
| offense set | {0, 7, 9, 0x10, 0x12, 0x13, 0x14, 0x15} | {0,15,8,17,20,7} |
| anti-buff / lightning | 7, 0x13 | 15 |
| scroll-grab spell | 0x16 (22) | — |

**This remap is the single biggest porting hazard.** Every hardcoded spell index in the project's
`rivals.rs` (heal=1, speed=2, possess=3, cloak=12, castle=16, offense {0,15,8,17,20,7}) is an MC1 id and
must be re-keyed to the MC2 ids above when the MC2 rival column is wired. The MC2 spell table
(`SPELLS_BEGIN_BUFFER_str`, spells.bin per the roster-sweep SPELLS.DAT import) is the source of truth for
the per-verb cost/level data the readiness sub reads.

---

## 8. MC1 CHASSIS REUSE MAP

For each MC1 `rivals.rs` mechanism: SAME / DIFFERENT / ABSENT in MC2.

| MC1 mechanism | verdict | detail |
|---|---|---|
| **Hate ledger** (0x601F neutral, rise agg+1, decay 256−agg, war pin) | **SAME** | `array_0x1FC_508`, EF:5377-5393; verbatim math |
| **Projectile hate feed** (+500/+3000, war at wealth threshold) | **SAME** | `sub_159E0` EF:7320; MC2 adds +1000/+5000 for hits on CASTLES (EF:7384) |
| **Decision period** (64 − reflexes/4) | **SAME** | EF:5460; Reflexes == tempo |
| **Selector cascade** (need-castle → flee → upgrade → raid → attack → balloon → possess → hunt → idle) | **SAME order** | EF:5516-5572 — but defense is cascade step 7 (project runs it pre-cascade); reconcile |
| **State set** (build/upgrade/possess/raid/attack/balloon/hunt/home/cruise/defense) | **SAME** | §1.2 table; state IDs are raw `byte_0x1C1_449`, not project ordinals |
| **Target signature** (id + model + class<<7) | **SAME** | `sub_14C40` EF:6701 = verbatim |
| **Commit / aim** (roll setpoint, pitch at cast, no lead) | **SAME** | `sub_146F0`/`sub_14E10` |
| **Burst gun** (8 shots, neg lockout (reflexes−255)/8−1, cones 0xAA/0xE3) | **SAME** | EF:6806-6858; verbatim |
| **Poverty latch** (<max/4, release min(max/4+6000, max/2)) | **SAME** | EF:7191-7198; verbatim |
| **Recast cooldown TABLE** | **DIFFERENT values + spell-id remap** | `x_WORD_D3F4C` EF:1070; swap wholesale (§7.2) |
| **Attack priority walk** | **DIFFERENT tables + order** | `unk_D3F80x`/`89x`/`91x` EF:1071-73; anti-buff is "target-holds-8 → 7", not rebound→lightning |
| **Approach helper** (arrive/boost radii, boost casts speed-up) | **SAME shape, remapped spell** | `sub_14C90` EF:6713; boost casts spell 3 not 2 |
| **Movement** (band-settle, level-forward, accel 16/tick, turn err/(8+(255−refl)/16), no wall gate) | **SAME** | `sub_146F0` EF:6415; verbatim |
| **Water/obstacle STEER** | **ABSENT in MC1 — NEW in MC2** | `sub_16580` EF:7879; MUST ADD (§2.2) |
| **Strafe / weave** | **SAME channel, DIFFERENT impulse** | `strafeSpeed_0x10_16`; weave value `3·minSpeed·refl/255`, not flat 80 |
| **Learn timer** (200-tick arm from any existing jar) | **ABSENT — REPLACED** | MC2 = physical pickup `sub_15CB0`/Perception scroll roll; port the pickup, drop the timer |
| **XP accrual** | **NEW (shared with human)** | via `sub_5F660`→`sub_6D8B0`; enable the XP decorator for rival casts |
| **Heal-when-hurt** (on think tick if life<max) | **SAME, remapped spell** | heal = spell 5 (not 1); EF:5468-5477 |
| **At-castle damage discard** (grace 2, memset mailbox) | **SAME** | EF:5400-5414; verbatim asymmetry |
| **Free-instant first AI castle** | **DIFFERENT — GONE** | MC2 runtime AI builds the real thing via `sub_14E10` case 2; only LEVEL-LOAD authored castles are free/instant (shared) |
| **Personality load** (3 words + Life, AI-only at spawn) | **SAME** | EF:43764-43772; from `WizardMapSettings_0x360D2[color]` |
| **Regen rates** (life/200 home, /500 afield) | **SAME** | EF:5440-5451 |
| **Economy loop** (claim mana only until maxMana≤castle cost) | **SAME** | EF:6135 |

### CORRECTIONS to prior claims

1. **Survey "wizard tick ONCE per frame → UpdateEntities × 1/4/8":** CONFIRMED but with nuance — the AI
   BRAIN (`sub_12910`, action 1) runs INSIDE `UpdateEntities` (the multiplied loop, EF:40108 `sub_68BF0`
   is the awake pre-pass, not the brain), so **the AI brain ticks 1/4/8× per frame like every other
   entity**, while the HUMAN's `PlayerEvents_51BB0` input tick runs ONCE per frame outside the loop. The
   AI is NOT throttled to once-per-frame; its own `byte_0x3E_62 % (64−reflexes/4)` cadence does the
   throttling. (Prior wording could be read as "AI ticks once/frame" — it does not.)

2. **The prompt's anchored "(3,0) wizard cave behavior row 104-vs-66":** this is a MOVEMENT/BEHAVIOR-TABLE
   fixture, NOT a rival-brain gate. There is no brain code path keyed on cave-vs-surface that swaps a
   behavior row for the wizard. The wizard's behavior row is set at spawn (row 59+model, survey §movement)
   and the AI brain reads it only for altitude band + turn caps. The "104 vs 66" swap, if real, lives in
   the CREATURE movement table / behavior-row assignment (survey rows 59-106), outside this trace's brain
   scope. **Flagged as a movement-table item, removed from the brain map.** (No `word_0x244_580`/brain
   read is gated on cave state.)

3. **Project `rivals.rs` "AI heals 4× the human afield rate":** MC2 does NOT — AI afield life regen is
   `/500` (EF:5449), human afield is `/2000` (EF:5449 is the AI sub; the human sub `sub_5EFA0` uses /2000
   per survey). So AI heals **4× faster afield** — this MATCHES the MC1 asymmetry and the project comment.
   CONFIRMED, not a correction, but worth banking: the /500-vs-/2000 4× gap is preserved in MC2.

4. **Defense placement:** the project runs `rival_defense` pre-cascade; MC2 runs `sub_15FC0` as cascade
   step 7. Behaviorally close, but for exact fidelity move it into the cascade (see §1.3).

---

## 9. PORT WORKLIST (Phase 4.3b, brain column)

1. **Spell-id remap (§7.3) — DO FIRST.** Re-key every hardcoded spell index in `rivals.rs` from MC1 ids to
   MC2 ids (heal 1→5, speed 2→3, possess 3→1, cloak 12→0xB, castle 16→2, offense set). Drive from the MC2
   spells.bin table.
2. **Swap the recast table** to `x_WORD_D3F4C` (§7.2) and the **attack priority tables** to
   `unk_D3F80x`/`unk_D3F89x`/`unk_D3F91x`. Replace the MC1 anti-rebound insert with the MC2 anti-buff
   branch (target-holds-8 → prefer 7 at Perception%).
3. **ADD the water/obstacle steer** `sub_16580` (§2.2): post-step four-neighbor `terrain_type == 8` probe,
   yaw-table snap, speed-zero-on-turn, with the `byte_0x45E_1118/1119` micro-FSM. Requires transcribing
   `sub_169C0`, `sub_16730`, and the `x_WORD_D3FCE`/`x_WORD_D3FE8` yaw tables (short — OPEN).
4. **Replace the learn timer** with the pickup path: `sub_15CB0` (own-keyed jar/scroll claim) on the
   decision-cadence tick + the Perception scroll roll in the ball picker. Drop `rival_learn_tick`'s
   200-tick arm.
5. **Enable the XP decorator for rival casts** (owner-tagged) so rivals gain spell XP/levels like the human
   (their cost lookups already read `byte_0x46_70` per-spell level).
6. **Re-pin the strafe impulse** to `3·minSpeed·Reflexes/255` (the weave value) instead of the flat MC1
   jink=80; wire the `str_611_byte_0x45D_1117` weave micro-FSM (EF:5980-6033).
7. **Drop the free-instant runtime AI castle** (`rival_cast_castle`'s castle-less direct-plant path is MC1);
   route AI Create Castle through the shared `sub_5F660`/build machinery (castle-builder §4).
8. **Move defense into cascade step 7** (or document the pre-cascade placement as an accepted approximation).
9. **Re-derive the scout-site distance threshold** from `sub_583B0`'s `0x3000` (§1.4) rather than the
   MC1-inherited `12288²`.
10. **Load personality + book from `WizardMapSettings_0x360D2`** (Aggression/Perception/Reflexes/Life +
    StartingSpells/BlockedSpells/byte_0x360FBx level array) — the MC2 editor struct (BasicTerrain.h:20-34),
    not the MC1 `wizards.json` shape. Coordinate with the sibling lifecycle trace (spawn/init owns the
    struct read; the brain owns the per-tick USE of the three words).

### OPEN items (need a short follow-up transcription)

- `sub_16580` support subs: `sub_169C0` (situation classifier) + `sub_16730` (neighbor probe) + the
  `x_WORD_D3FCE`/`x_WORD_D3FE8` yaw tables — dump verbatim before porting the water steer.
- `sub_583B0` metric (supercell-scaled?) — confirm the `0x3000` threshold's units for the scout site.
- `sub_15CB0`/`sub_15D20`/`sub_15D40` — the spell-pickup acquire chain (learning), transcribe fully for §4.
- `byte_0x360FBx[26]` in WizardMapSettings — confirmed to exist; verify it is starting LEVEL (assumed).
- `dword_0x16D_365` (init 2000, decremented in housekeeping) — purpose unconfirmed (post-death immunity?).
