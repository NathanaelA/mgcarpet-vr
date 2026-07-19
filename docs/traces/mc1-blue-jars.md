# MC1 BLUE vs RED spell jars — remc1 verbatim trace (2026-07-11)

Background-agent decompile walk (citations demanded and delivered).
All citations `reference/remc1/sub_main.cpp:LINE` unless noted;
cross-checked against `reference/remc1hw/sub_main.cpp` where noted.
Struct offsets from `reference/remc1/engine/Basic.h`.

Player-observed ground truth (senior source): levels past ~25 strip
spells via the level mask and some re-deliver them as BLUE jars —
same spell, castable with NO castle/minimum-mana requirement, even
fully castle-less (maze-level survival).

## Entity/data model

- Spell jar = class 12. Ctor `sub_3BF70`: `+64 = 12`, `+65 = spell
  id`, `+70 = base state`, `+132 = a8` (the requirement), sprite/type
  `+86 = 77` via `sub_36FA0_37360(v10, 77)` (:47981-48013). The 24
  per-spell ctor thunks `sub_3C040..sub_3C480` (:48022+); base state
  a3 = spell_id × 3.
- Dispatch: `dword_96902[class].str_0[state].data6(entity)` (:52354,
  :43218); class-12 row = `{str_256038, str_256208, …}` (:5053).
- `off_987DE[model65].adress(&entity->+72)` — the 24-entry spell
  thunk table dispatched by +65 (:64884, :48853; table :5167-92).
  **model65 = spell id, CONFIRMED.**
- Castle `Type_160` per-spell arrays `[24]` (Basic.h:226-236):
  `var_532` collected slot, `var_676` owned-manifestation index,
  `var_796` level availability, `var_844` charges, `var_892`
  persistent collected mask, **`var_916` = BLUE/unrestricted flag**.

## Q1 — Blue vs red: THING `data_12`

Class-12 THING post-init (`sub_37560_37920`, :44043-54):

```c
else if (v2z == 12) {                       // class 12 = spell jar
  LOBYTE(v2z) = type1090->data_12 & 0xff;   // jar variant (0..5)
  BYTE1(v2z) = v2z + *(_BYTE *)(v2 + 70);   // add to base state +70
  *(_BYTE *)(v2 + 70) = BYTE1(v2z);
  if (type1090->data_12 >= 3u) {            // >= 3  ==>  BLUE JAR
    *(_BYTE *)(v2 + 70) = BYTE1(v2z) - 3;   // 3->0, 4->1, 5->2
    v3 = *(_BYTE *)(v2 + 18);               // +18 byte[2]
    *(_WORD *)(v2 + 86) = 280;              // BLUE sprite/type 280
    *(_BYTE *)(v2 + 18) = v3 | 4;           // the BLUE FLAG
  }
}
```

`data_12` ∈ {0,1,2} = RED; {3,4,5} = BLUE (−3 recovers the same
sub-state). Blue and red share the three placed-jar states; blue adds
`byte[2]|=4` + sprite 280. HW identical (remc1hw :50976-80, THING
init mirror).

## Q2 — Pickup difference

Jar→owned conversion: `sub_55A40_55F70` (:64729, the ":64843-58"
region) and `sub_55D30_56260` (:64875):

- :64845: `if ((a1x->var_29811_16.byte[2] & 4) != 0)
  a1x->var_u16_29927_132 = 0;` — **blue ZEROES the requirement**.
- :64897: `if ((result->byte[2] & 4) != 0) result->+86 = 280;` —
  blue sprite re-applied on the owned manifestation.

Red keeps the ctor-baked `+132`. Slot handoff (auto-own `byte[0]|=1`,
`var_532` linkage, ordered inventory `var_772`) identical
(:64843-68).

## Q3 — "Unrestricted" representation and readers

- Live: `byte[2]&4` + `+132 == 0` on the manifestation.
- **ONE threshold, not two**: `+132` encodes castle-presence/level
  AND minimum-mana — both gates are the same comparison, guarded by
  `req != 0`:
  - Spellbook selectability (:26924): `if (var_132 && (castle_slot==0
    || var_132 > castle.mana[+140]) …) → greyed`.
  - Cast-render unavailable (:27860-64): `if (a3->+132) { if
    (!castle.+50 || a3->+132 > castle_mana_pool) draw-unavailable }`.
  - `+132 == 0` skips both → always castable, castle-less.
- Persistence: death/serialize (:55531-35) writes
  `castle.var_916[spell] = (manifestation.+18 & 4) ? 1 : 0`;
  respawn/level re-grant (:54908-12) restores `+86 = 280; +132 = 0;
  byte[2] |= 4` when `var_916[spell]` is set. (HW :50967-80.)

## Q4 — Visual

Red = type/sprite index 77 (:48012); blue = 280 (:44052, :64897).
`+86` indexes the `word_99BA6` type table (readers :36849, :37261).
No separate palette-remap path — the distinction is the type index.

## Q5 — Level spell mask interaction

Level-start grant loop (:54896-920) walks `var_532`, spawns each as a
class-12 manifestation, applies blue ONLY from `var_916[spell]`
(:54908). Level mask load (:49216-23): `var_796[spell] =
level_record.var_230983[spell]`; equip when `var_230883 && var_796`
(or persistent `var_892`). The mask never sets `var_916` — masked
re-grants are RED unless blue-persisted.

UNCERTAIN: no path found seeding `var_916` at level load without a
physical blue THING jar. Treat "blue = comes from a blue jar" as the
rule; revisit if a playtest shows a castle-less spell with no blue
jar present.

## Q6 — model65 = spell id: CONFIRMED (see model above).

## PORT STATUS (landed 2026-07-11, mgc_sim::mc1::world)

- `BLUE_SPELL` entity flag (0x40000 = `+18 byte[2]|=4`); THING
  post-init already decoded data_12 ≥ 3 → −3, type86 280, flag.
- `spell_castle_req(id)`: owned manifestation with BLUE_SPELL → 0,
  else table `castle_req` — feeds `spell_gate` (:64909 port) and
  `loadout()` bindable (:26926 port). One field, both gates, per the
  original.
- `Player::death_owned_blue[24]` = var_916; written by the jar
  scatter, consumed by the respawn re-grant (flag + type 280 back on
  the fresh manifestation). Hash-neutral when unused (goldens hold).
- try_pickup: in-place conversion carries flag + type86 for free.
- Test: `blue_jar_unrestricts_its_spell_and_survives_death`.
