# MC2 spell display names (general note 4)

**Audit date:** 2026-07-13 · **Method:** recorded gameplay senior, vendored decompile reference.
Cites `file:line`. DATA topic (string tables), not behavior.

## TL;DR

- MC2 has **two** retail spell-name string tables, both baked into `LANGUAGE/L*.TXT`
  (English = `L1.TXT`), and I extracted BOTH verbatim from `game.gog`:
  1. **Base names** (UPPERCASE), lang-string indices **160..185** — one per spell row
     (`FIREBALL`, `POSSESS`, `CASTLE`, …). Used by the **level-up notification**
     "Your ability to cast **%s** has improved." (fmt = lang-string **159**), site
     `sub_6DC40_improve_ability` (EF:44011).
  2. **Per-tier hint names** (mixed case), lang-string indices **186..265** — three per
     spell (with two 4-name exceptions), the encoded names the player described. Used by
     the **hover popup** (`SetSpellHelpPopupCoordinates_88D40` case 0, EF:49342) and the
     **change-spell notification** (EF:37925). The per-(spell,tier) index is
     `SPELLS_BEGIN_BUFFER_str[spell].subspell[tier].hintText_0x16x` (Spells.cpp static
     table, already carried in the port as `Mc2SubSpell::hint_text`).
- **There is NO runtime roman-numeral generator.** "Crater / Crater II / Crater III" are
  three *literal, distinct* strings in `L1.TXT`. Whether a spell reads as roman-numeral
  progression ("Crater I/II/III") or genuine renames ("Possession / Mana Magnet / Mana
  Lock") is purely a property of the baked string content — the engine just indexes a
  string per tier, no suffix logic anywhere.
- **Current port:** `hint_text` is *parsed and carried* per tier
  (`crates/mgc-sim/src/mc2/spells.rs:38,85`) but **no string table is imported or wired**.
  The UI shows a single hard-coded generic name per spell (`MC2_SPELL_NAMES`,
  `crates/mgc-app/src/ui.rs:1399`, "Console labels" only) — tier-independent, and the
  level-up notification surface plays only sound 61 with no text
  (`crates/mgc-sim/src/mc2/cast.rs:299-304`).
- **Fix = pure DATA import:** bake the `L1.TXT` string blob into the mc2 bundle, resolve
  hover = `strings[assets.spells[spell].tiers[tier].hint_text]`, level-up =
  `strings[159]` % `strings[160+spell]`. Port row order already matches L1.TXT order
  (verified: the 160..185 uppercase list equals the port's `MC2_SPELL_NAMES` order 1:1).

---

## 1. The mechanism (how a (spell,tier) becomes a display string)

### Struct + index
`type_SPELLS_BEGIN_BUFFER_str_sub` carries `int16_t hintText_0x16x` at struct offset 0x16
(Spells.h:13). Each spell row has 3 subspell tiers (Spells.h:20-25). The value is a
**lang-string index**, resolved through the global index buffer
`x_DWORD_E9C4C_langindexbuffer[...]`.

### Language buffer load + parse
- Loaded from `LANGUAGE/<Lx.TXT>` by `LoadLanguageFile` (MenusAndIntros.cpp:3438),
  path `GetSubDirectoryFile(cdFolder, "LANGUAGE", langfilename)` (MenusAndIntros.cpp:3458);
  filename is chosen by glob `LANGUAGE/L*.TXT` (English) / `D*.TXT` (MenusAndIntros.cpp:1190-1199).
- File layout: **4773-byte** header read into the font/position TAB (`a3`), then **12**
  bytes (`a3+4773`), then the remainder (`langfilelenght - 4785`) is the string blob
  (MenusAndIntros.cpp:3469-3479). So the string blob starts at **file offset 4785**.
- The blob is split by `sub_5B870_copy_sentence` (EF:42829-42847) into **471** pointers,
  one per **NUL-terminated** string: `langindexbuffer[i] = &blob[k]; while(blob[k++]);`.
  String index N = the Nth NUL-terminated run in the blob.

### The three use sites
| Site | file:line | What it shows |
|---|---|---|
| **Level-up notification** (`sub_6DC40_improve_ability`) | EF:44011 | `sprintf(buf, langindexbuffer[159] /*"Your ability to cast %s has improved."*/, langindexbuffer[160+ability])` — `ability` = spell row 0..25 → the **UPPERCASE base name**. Then `SetCurrentNotificationMessage_19760(...,200)` + sound **61** (EF:44012-44013). |
| **Change-spell notification** (Change Spell input 0x1F/0x20) | EF:37925 | `strcpy(...CurrentNotificationText..., langindexbuffer[ SPELLS_BEGIN_BUFFER_str[spellIndex].subspell[tier].hintText_0x16x ])` — the **per-tier hint name** of the tier just selected. |
| **Hover mouseover** (`SetSpellHelpPopupCoordinates_88D40`, `byte_0xa4` case 0 = "Selected Spell Name") | EF:49342-49355 | copies all three `subspell[0..2].hintText_0x16x` into `hintText[0..2]` and hands them to `SetHelpPopupTextAndCoords_884D0(85, typeC=3, hintText, typeA=3, typeB=subSpellIndex)` — the popup renders the tier names. |

So the player's "message when a new spell level is reached" = EF:44011 (uppercase base
name), and "hover mouseover display" = EF:49342 (mixed-case per-tier names). The
change-spell toast (EF:37925) is a third surface reusing the per-tier table.

### The MapType alternate (rows 4 & 19)
`LevelInit_56C00` rewrites two rows' tier-0 `hintText` every level init, keyed to MapType
(LevelInit.cpp:12-21):
- Row **4** (Metamorph): non-Day → **199** (`Firefly Morph`), Day → **198** (`Bee Morph`).
- Row **19** (Summon Army): non-Day → **245** (`Firefly Army`), Day → **244** (`Bee Army`).

This is why the hint-name list has **80** strings (186..265), not 78: rows 4 and 19 each
own **four** strings (198+199, 244+245) because tier-0 swaps by environment. Already ported
verbatim as `mc2::spells::level_init_patch` (`crates/mgc-sim/src/mc2/spells.rs:104-114`).

**No roman-numeral code exists.** Both use sites index a string directly; there is no
concatenation of a base name with a generated numeral. Confirmed by reading EF:37925,
EF:44011, EF:49342-49355 end-to-end.

---

## 2. THE DATA — the full 26×3 table (extracted verbatim from `game.gog` `L1.TXT`)

Base-name indices from the `160+ability` rule (EF:44011); per-tier indices from the
`SPELLS_BEGIN_BUFFER_str[26]` static initializer (Spells.cpp:2-107, the 6th field of each
subspell row is `hintText_0x16x`); string *contents* extracted from the English `L1.TXT`
blob embedded in `gamedata/Magic Carpet 2/game.gog` (offset ~3,445,000). **Every Spells.cpp
hintText index matched a string 1:1** — see verification note below.

Legend: **[R]** = roman-numeral / power-only progression · **[S]** = distinct-string
(functionality-rename) progression · **[mix]** = both.

| Row | Base (160+i, UPPERCASE) | Tier 0 (idx) | Tier 1 (idx) | Tier 2 (idx) | Kind |
|----:|---|---|---|---|---|
| 0 | 160 FIREBALL | FireBall (186) | Rapid Fire (187) | Fire Storm (188) | [S] |
| 1 | 161 POSSESS | Possession (189) | Mana Magnet (190) | Mana Lock (191) | [S] |
| 2 | 162 CASTLE | Castle (192) | Fire Tower (193) | Lightning Tower (194) | [S] |
| 3 | 163 SPEED UP | Speed Up (195) | Super Speed (196) | Super Speed II (197) | [mix] |
| 4 | 164 MORPH | Bee Morph (198, Day) / Firefly Morph (199, non-Day) | Cymmerian Morph (200) | Wyvern Morph (201) | [S] |
| 5 | 165 HEAL | Heal (202) | Aid (203) | Constitution (204) | [S] |
| 6 | 166 SHIELD | Shield (205) | Shield II (206) | Invulnerable (207) | [mix] |
| 7 | 167 LIGHTNING | Lightning (208) | Thunderbolt (209) | Thunderstorm (210) | [S] |
| 8 | 168 REBOUND | Rebound (211) | Rebound II (212) | Amplify (213) | [mix] |
| 9 | 169 METEOR | Meteor (214) | Meteor II (215) | Meteor III (216) | [R] |
| 10 | 170 TELEPORT | Teleport (217) | Teleport II (218) | Castle Port (219) | [mix] |
| 11 | 171 INVISIBLE | Invisible (220) | Possess Invisible (221) | Attack Invisible (222) | [S] |
| 12 | 172 BEYOND SIGHT | Beyond Sight (223) | See Invisible (224) | See All (225) | [S] |
| 13 | 173 STEAL MANA | Steal Mana (226) | Double Steal (227) | Burgle (228) | [S] |
| 14 | 174 DUEL | Duel (229) | Mana Drain (230) | Health Drain (231) | [S] |
| 15 | 175 TREMOR | Tremor (232) | Tremor II (233) | Tremor III (234) | [R] |
| 16 | 176 CRATER | Crater (235) | Crater II (236) | Crater III (237) | [R] |
| 17 | 177 EARTHQUAKE | Earthquake (238) | Earthquake II (239) | Earthquake III (240) | [R] |
| 18 | 178 VOLCANO | Volcano (241) | Volcano II (242) | Volcano III (243) | [R] |
| 19 | 179 SUMMON ARMY | Bee Army (244, Day) / Firefly Army (245, non-Day) | Cymmerian Army (246) | Wyvern Army (247) | [S] |
| 20 | 180 GRAVITY WELL | Gravity Well (248) | Gravity Well II (249) | Gravity Well III (250) | [R] |
| 21 | 181 WHIRLWIND | Whirlwind (251) | Whirlwind II (252) | Whirlwind III (253) | [R] |
| 22 | 182 FOOL'S MANA | Fool's Mana (254) | Rapid Fire (255) | Lightning Fire (256) | [S] |
| 23 | 183 MAGIC MINE | Magic Mine (257) | Magic Mine II (258) | Magic Mine III (259) | [R] |
| 24 | 184 ALLIANCE | Alliance (260) | Alliance II (261) | Alliance III (262) | [R] |
| 25 | 185 CAVE IN | Cave In (263) | Cave In II (264) | Cave In III (265) | [R] |

**Verification:** the per-tier `hintText_0x16x` values in Spells.cpp are strictly sequential
per row — row 0 = {0xBA,0xBB,0xBC}=186/187/188 … row 25 = {0x107,0x108,0x109}=263/264/265 —
with exactly two gaps at **199 (0xC7)** and **245 (0xF5)**, which are the LevelInit Day/non-Day
tier-0 alternates for rows 4 and 19. That is precisely the shape of the 80-string block
(186..265) I extracted. Rows 4/19 static tier-0 = 198/244 (the Day strings); LevelInit
swaps to 199/245 off-Day. Every index resolved to a sensible name. High confidence.

**Roman vs distinct tally:** ~10 pure roman-numeral spells [R] (Meteor, Tremor, Crater,
Earthquake, Volcano, Gravity Well, Whirlwind, Magic Mine, Alliance, Cave In), ~12 distinct-
string [S], ~4 mixed [mix]. Exactly the "some numerals, some real strings" the player
reported.

**Where the ASCII lives:** `LANGUAGE/L1.TXT` (English) inside
`gamedata/Magic Carpet 2/game.gog` (the CD ISO). Other languages ship parallel files
(`Cratere / Cratere II / Cratere III` French confirmed at another offset). NUL-separated,
blob starts at file offset 4785, 471 strings total (indices 0..470). Base names 160..185,
hint names 186..265. Format matches the importer's existing ISO reader
(`crates/mgc-import/src/iso.rs`).

---

## 3. Current port state (no name table wired)

- **Parse only, no resolve:** `Mc2SubSpell.hint_text` is read from `spells.bin`
  (`crates/mgc-sim/src/mc2/spells.rs:37-38, 85`) and the MapType patch is ported
  (`spells.rs:104-114`). But `hint_text` is **never consumed** — nothing turns it into text.
- **No string import:** `crates/mgc-import/src/gamedata.rs` only *mentions* `LANGUAGE/` in a
  doc comment (line 17); there is no `L*.TXT` reader, no `copy_sentence` equivalent, and
  `bundle.rs` emits no strings blob (it bakes `spells.bin` at bundle.rs:929-936 but nothing
  language-related).
- **UI shows one generic name per spell, tier-independent:**
  `crates/mgc-app/src/ui.rs:1399` `pub const MC2_SPELL_NAMES: [&str; 26]` — labelled
  "Console labels — MC2 shows hint text in-game." Consumed by
  `main.rs:1309` (`pane_spell_name`) and `ui.rs:196`. So the pane hover reads e.g.
  "Possession" for *every* tier of spell 1, never "Mana Magnet" / "Mana Lock". These are
  also hand-authored guesses (e.g. "Metamorph", "Summon Army", "Cave-In", "Fool's Mana"),
  **not** the retail strings.
- **Level-up notification is text-less:** `cast.rs:299-304` (`mc2_relevel`) plays sound 61
  only; the comment explicitly defers the "string 159 + 160+idx" message to "the 4.9
  presentation track." No banner text is produced.

Net: the `hint_text` datum is present and correct; the string table it points into is
absent; the two retail surfaces (level-up banner, tier hover) are unwired/stubbed.

---

## 4. Gap

1. **String table not imported.** `LANGUAGE/L1.TXT` is never read/baked. Need the blob
   (or at least indices 159..265) in the mc2 bundle.
2. **Hover shows the wrong granularity.** `MC2_SPELL_NAMES` is one name per spell, so the
   tier-specific encoded names ("Rapid Fire", "Mana Lock", "Crater III") never appear —
   the exact thing the player flagged. It's also a hand-authored list, not retail text.
3. **Level-up banner missing text.** `mc2_relevel` fires sound 61 but no
   "Your ability to cast POSSESS has improved." message.
4. **Change-spell toast missing** (EF:37925) — a third surface, also unwired.

No behavioral risk; this is presentation/data only.

---

## 5. Fix (DATA)

### 5a. Source the strings
Import `LANGUAGE/L1.TXT` (English; generalize to `L*.TXT` later for locale) via the
existing ISO reader (`crates/mgc-import/src/iso.rs`, same path the importer already uses for
`DATA/…`). Skip the **4785-byte** header (4773 + 12), then split the remainder on `\0` into
strings. Bake either the whole 471-string blob or the slice **159..=265** into the mc2
bundle as e.g. `spellnames.bin` / `langstrings.bin` (bundle schema: add an
`Option<&str>` spec field beside `spells` at `bundle.rs:98`, emit like `spells.bin` at
bundle.rs:932-936; register in `docs/FORMAT.md`). Follow the unified-bundle rule (one schema,
per-variant; MC1 can leave it `None`).

### 5b. The (spell,tier) → string-index map
Already in hand — **no new table needed**:
- **Hover / change-spell (per tier):** `idx = assets.spells[spell].tiers[tier].hint_text`
  (the parsed `Mc2SubSpell::hint_text`), then `strings[idx]`. The MapType alternate for
  rows 4/19 is already applied by `level_init_patch`, so `hint_text` is correct at runtime.
- **Level-up banner (per spell):** `idx = 160 + spell`, then `strings[idx]` (UPPERCASE base
  name), formatted into `strings[159]` (`"Your ability to cast %s has improved."`).
- Port row order == L1.TXT order (verified: 160..185 uppercase list is byte-for-byte the
  order of `MC2_SPELL_NAMES`), so `spell` indexes both directly.

### 5c. Roman-numeral rule
**None.** Do NOT synthesize "I/II/III". Each tier string is literal in the table; emit it
verbatim. (If a fallback is ever needed when the blob is missing, the table in §2 is the
authority.)

### 5d. The two (three) use sites
- **Level-up:** `crates/mgc-sim/src/mc2/cast.rs:299-304` — where sound 61 fires, also emit
  the notification text `format!(strings[159].replace("%s", strings[160+spell]))`
  (or a printf-style `%s` substitution) onto the MC2 notification surface (the
  `CurrentNotificationText` equivalent, 200-tick life per EF:44012). Mirrors
  `sub_6DC40_improve_ability`.
- **Hover:** the pane hover path (`crates/mgc-app/src/ui.rs` hover, `main.rs:1309`
  `pane_spell_name`) should return `strings[assets.spells[spell].tiers[sel_tier].hint_text]`
  for MC2 instead of `MC2_SPELL_NAMES[spell]`. `MC2_SPELL_NAMES` can stay as a
  missing-blob fallback / MC1-console label but is no longer the MC2 in-game name.
- **Change-spell toast** (optional, EF:37925): same per-tier lookup on a spell-switch input,
  onto the notification surface.

---

## 6. Confidence + open questions

**High confidence:**
- The two-table structure, the three use sites, the index arithmetic (159 / 160+i / 186+),
  and the `hintText_0x16x` mechanism — all read directly from Spells.cpp / EF / MenusAndIntros.
- The full §2 name table — extracted verbatim from the shipped `L1.TXT` and cross-checked
  1:1 against the Spells.cpp static indices (every index matched, gaps explained by the
  rows-4/19 MapType alternates).
- The port row order matches L1.TXT order (independent check via the uppercase 160..185 list).

**Open / to confirm:**
1. **`L1.TXT` header size across variants.** The 4773+12=4785 split is from the retail
   loader (MenusAndIntros.cpp:3469-3470); confirm the exact byte offset when writing the
   importer (the header is a resolution-dependent font/position TAB — see the
   `x_WORD_180660` branch at MenusAndIntros.cpp:3494). Safer: parse from the file's own
   TAB length rather than hard-coding 4785, or just locate string 159 by content.
2. **Stale Spells.h enum comment.** The `Entity Sub-Type - Spell` comment in Spells.h:28-54
   (0=Fireball,1=Heal,2=Speed Up,3=Possession…) does **NOT** match the SPELLS.DAT/L1.TXT
   row order (0=Fireball,1=Possession,2=Castle,3=Speed Up,4=Morph,5=Heal…). It is an old
   MC1-style subtype list — **ignore it for ordering**; the port's `MC2_SPELL_NAMES` order
   and the 160..185 uppercase list are the authority.
3. **String 172 "BEYOND SIGHT"** printed split by an embedded binary run in my raw dump;
   identity is unambiguous (row 12 Beyond Sight, mixed-case 223 "Beyond Sight"), but confirm
   the exact bytes when the importer parses it cleanly.
4. **`%s` substitution vs printf.** Retail uses C `sprintf` with the `%s` in string 159; the
   port should do a literal `%s`→name substitution (the string contains exactly one `%s`).
5. **Non-English locales.** `D*.TXT` (German) etc. exist in the ISO; same index scheme
   (French "Cratere II/III" confirmed). Locale selection is out of scope for the initial
   English bake but the schema should not preclude it.
