# MC2 FULL-ROSTER SURVEY (2026-07-09)

Inventory of every remc2 entity class for the Phase-4.3 full-roster
sweep (player directive: "port all creatures... finish all of the
tabular data, then grind through levels and work out the
deviations"). Three Opus research agents against
`reference/remc2/remc2/engine/`; every claim cites file:line —
verify before porting. Companion to docs/SURVEY-MC2.md (the Phase-0
chassis survey). Sections land as the agents report.

Already ported at survey time (mgc_sim::mc2): class 5 models
1/4/13; class 9 model 13 (arrow); class 10 models 0/1/45; class 11
models 0-4 + 32; class 2 models 0/1/2 (ctors; ticks inert).

STATUS 2026-07-10 (Phase-4.3 WAVE A LANDED — ROADMAP has the full
record; verbatim traces in docs/traces/mc2-*.md): class 5 +models
2/9/12/14/16/17/18/19/20/21/23/24/25/26/28 (24 cave-gated); class 9
+the shared flyer core (sub_65820) + subtypes 0/9/20/21 + all
creature attack thunks; class 2 models 3-8 + the tree burn ladder;
class 11 +the slot-condition band 13..=30/33..=44 + X-markers 12/31;
class 14 models 0/3/4/5 (+1 inert riser); (10,1) corpse burst.
REMAINING: multipart 0/3/22/27 (trace banked), (5,10) doomsday,
class-10 effects band, class 15 tokens (trace banked), class-11
models 5..=11 (OPEN).

---

## SECTION 1: CLASS 9 + CLASS 10 (agent report, verbatim)

**Scope:** `reference/remc2/remc2/engine/`. Dispatch tables in `EventsFunctions.cpp`; address→function switches in `Events.cpp`.

### 0. How dispatch works (mechanics, cited)

- `str_D4C48ar[17]` — per-class table, `EventsFunctions.cpp:2060`. Row 10 at `:2071` (`dword_10`=action tbl `x_DWORD_D4C52ar_strA0`, `dword_14`=create tbl `x_DWORD_D4C52ar_strA1`). Row 9 at `:2070` (`strA0`=str90, `strA1`=str91).
- Table entry = `{0x2A5C44, word_4=subtype/state, address_6, dword_10=valid}`.
- **Creation** entry point: `IfSubtypeCallCreatingManaSphere_4A190(pos, type, subtype)` → `pre_sub_4A190_axis_3d(address_6, pos)` (`Events.cpp:5186-5191`). This is the generic `CreateEvent(class,subtype)`.
- **Action/tick** dispatch: per entity each frame via `pre_sub_4A190_0x6E8E(action_addr, entity)` (`Events.cpp:610`, called `Events.cpp:439` etc.).
- Both `pre_sub_4A190_*` are giant `switch(address)` blocks resolving raw MC2 binary addresses → C++ fns. Creation switch: `Events.cpp:4171-4900+`. Action switch: `Events.cpp:620-2900+`.
- Sprite link: creators call `SetEntityIndexAndRot_49CD0(event, spriteRow)`; models with **no** SetEntityIndex + no `AddEventToMap_57D70` draw nothing (invisible logic drivers).

### A.1 Class-10 action table `x_DWORD_D4C52ar_strA0[100]` (`EventsFunctions.cpp:1601`)

State index (= actionIndex) → handler (action switch `Events.cpp:2269+`); state N declared at `EventsFunctions.cpp:1602+N`.

| State | Addr | Handler fn (Events.cpp) | Note |
|---|---|---|---|
|0x00|211D50|`sub_30D50` :2273|ground fire tick (PORTED)|
|0x01|211F60|`AddQuickfair0A_01_30F60` :2277|big explosion tick (PORTED)|
|0x02|212100|`CastSpeedSpell_31100` :2281|speed spell|
|0x03|212890|`sub_31890` :2302|—|
|0x04|213520|(empty) :2365|no-op|
|0x05|2128B0|`AddAsh0A_05_318B0` :2306|water-splash end|
|0x06|212760|`sub_31760` :2294|—|
|0x07|212870|`sub_31870` :2298|—|
|0x08|212920|`sub_31920` :2311|—|
|0x09|212940|`sub_31940` :2315|—|
|0x0A|212E90|`sub_31E90` :2319|—|
|0x0B|212FB0|`sub_31FB0` :2327|—|
|0x0C|2130E0|`PossesHitMana_320E0` :2332|posses spell drop|
|0x0D|213160|`sub_32160` :2344|quest point 3|
|0x0E|2132A0|`sub_322A0` :2348|—|
|0x0F|213530|`sub_32530` :2368|—|
|0x10|213600|`sub_32600` :2372|—|
|0x11|213880|`sub_32880` :2376|—|
|0x12|213A70|`sub_32A70` :2381|—|
|0x13|213F40|`sub_32F40` :2389|—|
|0x14|2140F0|OPEN (addr not in switch)|—|
|0x15|214100|OPEN|—|
|0x16|214110|`sub_33110` :2393|—|
|0x17|214D80|`sub_33D80` :2444|—|
|0x18|214E10|OPEN|—|
|0x19|214E20|`sub_33E20` :2448|—|
|0x1A|214E80|`sub_33E80` :2452|—|
|0x1B|215110|`sub_34110` :2473|—|
|0x1C|215000|`sub_34000` :2468|—|
|0x1D|215210|`sub_34210` :2477|—|
|0x1E|215330|`sub_34330` :2481|—|
|0x1F|215350|`sub_34350` :2485|—|
|0x20|2153F0|`ApplyPointToPath_343F0` :2504|path (2 inst lvl1)|
|0x21|215480|`sub_34480` :2509|—|
|0x22|2154A0|`sub_344A0` :2513|—|
|0x23|216600|`sub_35600` :2565|—|
|0x24|216390|`sub_35390` :2556|—|
|0x25|216530|`sub_35530` :2560|—|
|0x26|2167C0|`sub_357C0` :2573|—|
|0x27|NULL|—|empty slot|
|0x28|216640|`sub_35640` :2569|lightning II|
|0x29|216940|`TransformArcherToMana_35940` :2578|mana-sphere move (archer→mana)|
|0x2A|217AE0|`sub_36AE0` :2627|enemy dead|
|0x2B|2187F0|`sub_377F0` :2661|—|
|0x2C|218BC0|`AddTerrainMod0A_2A_37BC0` :2665|terrain paint (groove castle)|
|0x2D|2199F0|`sub_389F0` :2682|cast castleII 2|
|0x2E|219AF0|(empty) :2686|no-op|
|0x2F|NULL|—|empty|
|0x30|218240|`ApplyTerrainModification_37240` :2648|terrain mod (PORTED; 255 inst lvl1)|
|0x31,0x32|NULL|—|empty|
|0x33|218240|`ApplyTerrainModification_37240` :2648|terrain mod (alias)|
|0x34|219330|`AddHouse0A_2D_38330` :2674|castle/building|
|0x35|2195C0|`RemoveCastleStage_385C0` :2678|castle stage remove|
|0x36|2162A0|`sub_352A0` :2547|—|
|0x37|2162C0|`sub_352C0` :2551|—|
|0x38|219B70|(empty) :2693|no-op|
|0x39|219B80|(empty) :2696|no-op|
|0x3A|219B90|`sub_38B90` :2700|—|
|0x3B|219D80|`sub_38D80` :2704|smoke tick|
|0x3C|219E40|`sub_38E40` :2712|smoke tick|
|0x3D|219E60|(empty) :2716|no-op|
|0x3E|216FB0|`sub_35FB0` :2582|—|
|0x3F|216940|`TransformArcherToMana_35940` :2578|(alias)|
|0x40|2133E0|`AddParticleSmoke0A_3B_323E0` :2352|checkpoint smoke (quest pt2)|
|0x41|213400|`AddParticleSmoke0A_3C_32400` :2357|smoke|
|0x42|215370|(empty) :2489|no-op|
|0x43|215380|(empty) :2492|no-op|
|0x44|215390|`sub_34390` :2495|—|
|0x45|2153C0|`sub_343C0` :2499|—|
|0x46|219E70|`sub_38E70` :2719|—|
|0x47|219F70|`sub_38F70` :2724|—|
|0x48|21A040|`sub_39040` :2728|—|
|0x49|21A6A0|`sub_396A0` :2732|—|
|0x4A|21A6D0|`sub_396D0` :2736|—|
|0x4B|219E20|`sub_38E20` :2708|—|
|0x4C|219D80|`sub_38D80` :2704|(alias)|
|0x4D|213120|`sub_32120` :2340|model 0x46 tick|
|0x4E|21B2D0|`sub_3A2D0` :2761|—|
|0x4F|21B5B0|`sub_3A5B0` :2766|—|
|0x50|21B630|`sub_3A630` :2770|—|
|0x51|21B650|`sub_3A650` :2774|—|
|0x52|NULL|—|player start (no tick)|
|0x53|2149B0|`sub_339B0` :2418|—|
|0x54|NULL|—|empty|
|0x55|21B8B0|`sub_3A8B0` :2782|—|
|0x56|21BF00|`sub_3AF00_castle_defend_event` :2787|castle defend|
|0x57|215520|`sub_34520` :2517|—|
|0x58|215540|`sub_34540` :2521|—|
|0x59|215910|`sub_34910` :2525|—|
|0x5A|215C40|`sub_34C40` :2539|—|
|0x5B,0x5C|215EE0|`sub_34EE0` :2543|—|
|0x5D|212120|`sub_31120` :2285|—|
|0x5E|213160|`sub_32160` :2344|—|
|0x5F|212750|OPEN|—|
|0x60|2121E0|`sub_311E0` :2290|—|
|0x61|212740|OPEN|—|
|0x62|213CF0|`sub_32CF0` :2385|—|

### A.2 Class-10 creation table `x_DWORD_D4C52ar_strA1[93]` (`EventsFunctions.cpp:1703`) — the per-model roster

Model N at `EventsFunctions.cpp:1704+N`; addr via creation switch (`Events.cpp:4171+`). **Cat** = semantic tag.

| Model | Addr | Creator fn : def line | Sprite | Tick(state) | Cat / purpose |
|---|---|---|---|---|---|
|0x00|22F320|`NewAdd0A00_4E320` :35332|7|st0|**spell fx** ground fire (PORTED)|
|0x01|22F3B0|`NewAdd0A01_4E3B0` :35355|41|st1|**visual fx** big explosion (PORTED)|
|0x02|22F430|`NewAdd0A02_4E430` :35376|**none**|st2|**logic driver** (no map/sprite)|
|0x03|22F490|`NewAdd0A03_4E490` :35394|36|st3|**visual fx**|
|0x04|22F500|`NewAdd0A04_4E500` :35415|**none**|st4|**logic driver** (invisible, life100)|
|0x05|22F570|`NewAdd0A05_4E570` :35436|244|st5|**smoke/particle** water-splash begin|
|0x06|22F5F0|`NewAdd0A06_4E5F0` :35458|228|st6|**visual fx** (rot/shift)|
|0x07|22F6A0|`NewAdd0A07_4E6A0` :35483|half-speed 78|st7|**smoke/particle** (rand life)|
|0x08|22F750|`sub_4E750` :35507|— returns 0|—|**other** stub (no entity)|
|0x09|22F760|`NewAdd0A09_4E760` :35513|**none**|st9|**logic driver** (sets `byte_0x36E03`)|
|0x0A|22F7D0|`NewAdd0A0A_4E7D0` :35533|**none**|st0x0A|**logic driver** (invisible)|
|0x0B|22F840|`NewAdd0A0B_4E840` :35553|**none**|st0x0B|**logic driver**|
|0x0C|22F8C0|`NewAdd0A0C_4E8C0` :35574|41|st0x0C|**spell fx** posses drop|
|0x0D|22F9E0|`SetParticleSmoke3B_4E9E0` :35618|SetSmoke4 idx67|st0x3B|**smoke family**|
|0x0E|22FA20|`SetParticleSmoke3C_4EA20` :35625|SetSmoke4 idx9|st0x3C|**smoke family**|
|0x0F|22FCD0|`sub_4ECD0` :35707|**none** (shift only)|st15|**smoke/particle** (yaw rand)|
|0x10|22FDC0|`sub_4EDC0` :35749|210|st16|**visual fx** (moving)|
|0x11|22FD70|`AddMeteor_4ED70` :35731|**none**|st17|**spell fx** meteor|
|0x12|22FED0|`sub_4EED0` :35777|**none** (life10000)|st18|**logic driver** (long-lived, invis)|
|0x13|22FF90|`sub_4EF90` :35824|228|st19|**visual fx**|
|0x14|230020|returns 0 :4618|—|—|disabled|
|0x15|230030|returns 0 :4622|—|—|disabled|
|0x16|230040|`AddWind_4F040` :35852|**none**; spawns 11x model 75|st22|**spell fx** wind (multi-entity)|
|0x17|2305F0|`sub_4F5F0` :36087|OPEN|st0x17|OPEN|
|0x18|230690|returns 0 :4646|—|—|disabled|
|0x19|2306A0|`sub_4F6A0` :36110|OPEN|st0x19|OPEN|
|0x1A|230720|`sub_4F720` :36130|OPEN|st0x1A|OPEN|
|0x1B|2307A0|`sub_4F7A0` :36151|OPEN|st0x1B|(lvl-data consumed)|
|0x1C|230800|`sub_4F800` :36170|OPEN|st0x1C|(lvl-data consumed)|
|0x1D|230A00|`sub_4FA00` :36274|OPEN|st0x1D|(lvl-data)|
|0x1E|2309A0|`AddPointToPath_4F9A0` :36256|OPEN|st0x1E|**logic driver** path point (lvl-data)|
|0x1F|230AC0|`sub_4FAC0` :36311|OPEN|st0x1F|**logic driver** (lvl-data)|
|0x20|230A60|`sub_4FA60` :36292|OPEN|st0x20|**logic driver** (lvl-data)|
|0x21|231020|`sub_50020` :36578|OPEN|st0x21|OPEN|
|0x22|230E40|`sub_4FE40` :36506|OPEN|st0x22|OPEN|
|0x23|230F20|`sub_4FF20` :36532|— no-arg|st0x23|OPEN|
|0x24|230F30|`sub_4FF30` :36538|OPEN|st0x24|OPEN|
|0x25|NULL|—|—|—|empty :1741|
|0x26|230FB0|`sub_4FFB0` :36559|OPEN|st0x26|OPEN|
|0x27|231080|`CreateManaSphere512_50080` :36595|(→CreateManaSphere)|st0x27|**economy** mana sphere 512|
|0x28|2311D0|`sub_501D0` :36659|OPEN|st0x28|**economy?** OPEN|
|0x29|231320|`sub_50320` :36717|OPEN|st0x29|OPEN|
|0x2A|231370|`sub_50370` :36734|OPEN "arrow1"|st0x2A|**spell fx** arrow (`Events.cpp:4787`)|
|0x2B|2312B0|`sub_502B0` :36697|OPEN "cast castleII"|st0x2B|**castle** (`Events.cpp:4779`)|
|0x2C|2313D0|`sub_503D0` :36753|OPEN|st0x2C|OPEN|
|0x2D|231250|`AddTerrainModification_50250` :36677|177|st0x33|**terrain** building (PORTED)|
|0x2E-0x31|NULL|—|—|—|empty :1750-1753|
|0x32|230DE0|`sub_4FDE0` :36488|OPEN|st0x32|OPEN|
|0x33|230D70|`sub_4FD70` :36468|OPEN|st0x33|**terrain?** OPEN|
|0x34|231430|`sub_50430` :36772|OPEN|st0x34|OPEN|
|0x35|2314B0|`sub_504B0` :36794|OPEN|st0x35|OPEN|
|0x36|231500|`AddAuxiliary_50500` :36812|OPEN|st0x36|**logic driver** auxiliary|
|0x37|231640|`sub_50640` :36864|OPEN|st0x37|OPEN|
|0x38|2316E0|`sub_506E0` :36888|OPEN|st0x38|OPEN|
|0x39|231130|`sub_50130` :36631|OPEN|st0x39|OPEN|
|0x3A|2310A0|`CreateManaSphere2560_500A0` :36601|(→sphere)|st0x3A|**economy** mana sphere 2560|
|0x3B|22FB50|`ArriveCheckpoint_4EB50` :35663|**none**|st0x40|**logic driver** checkpoint/quest|
|0x3C|22FC10|`AddSmoke_4EC10` :35685|**none**|st0x41|**smoke family** (1 inst lvl9)|
|0x3D|230860|`sub_4F860` :36188|OPEN|st0x3D|OPEN|
|0x3E|2308B0|`sub_4F8B0` :36205|OPEN|st0x3E|OPEN|
|0x3F|230900|`sub_4F900` :36222|OPEN|st0x3F|OPEN|
|0x40|230950|`sub_4F950` :36239|OPEN|st0x40|OPEN|
|0x41|231780|`sub_50780` :36912|OPEN|st0x41|OPEN|
|0x42|2317C0|`sub_507C0` :36928|OPEN|st0x42|OPEN|
|0x43|232730|OPEN (addr > switch range)|—|st0x43|OPEN|
|0x44|2315A0|`sub_505A0` :36836|OPEN|st0x44|OPEN|
|0x45|231500|`AddAuxiliary_50500` :36812|(alias)|st0x45|**logic driver**|
|0x46|22F950|`NewAdd0A46_4E950` :35596|41|st0x4D|**spell fx** (posses-drop clone)|
|0x47|232790|OPEN|—|st0x47|OPEN|
|0x48|232800|OPEN|—|st0x48|OPEN|
|0x49|232A00|OPEN|—|st0x49|OPEN|
|0x4A|231800|`sub_50800` :36945|— no-arg|st0x4A|OPEN|
|0x4B|NULL|—|—|—|empty :1779|
|0x4C|2302A0|`AddFireSpheres_4F2A0` :35936|**none** (gated ≥26 free)|st0x53|**spell fx** fire spheres|
|0x4D|NULL|—|—|—|empty :1781|
|0x4E|231840|`sub_50840` :36960|OPEN|st0x4E|OPEN|
|0x4F|2318E0|`sub_508E0_castle_defend_create` :36987|OPEN|st0x4F|**castle** defend|
|0x50|230B80|`sub_4FB80` :36352|OPEN|st0x50|OPEN|
|0x51|230B20|`sub_4FB20` :36329|OPEN|st0x51|OPEN|
|0x52|230BE0|`sub_4FBE0` :36374|OPEN|st0x52|OPEN (player start pass-1 record)|
|0x53|230C30|`sub_4FC30` :36397|OPEN|st0x53|OPEN|
|0x54|230CA0|`sub_4FCA0` :36421|OPEN|st0x54|OPEN|
|0x55|230CD0|`sub_4FCD0` :36433|OPEN|st0x55|OPEN|
|0x56|231960|`sub_50960` :37011|OPEN|st0x56|OPEN|
|0x57|22FA60|`sub_4EA60` :35632|SetSmoke4 idx67|st0x57|**smoke family**|
|0x58|231A10|returns 0 :4852|—|—|disabled (but LVL-DATA REFERENCES 0x58 — OPEN discrepancy)|
|0x59|231A20|`sub_50A20` :37037|OPEN|st0x59|OPEN|
|0x5A|231A80|returns 0 :4860|—|—|disabled|
|0x5B|22FF30|`sub_4EF30` :35797|**none** (life10000)|st0x62|**logic driver** invisible|

**Smoke/particle family** (share `SetSmoke4_4EAA0` :35639): 0x0D, 0x0E, 0x57, plus 0x3C. **Invisible logic drivers**: 0x02, 0x04, 0x09, 0x0A, 0x0B, 0x12, 0x1E-0x20, 0x3B, 0x5B. **Deps:** terrain-paint → 0x2C,0x30,0x33,0x2D; terrain-alt → 0x04-0x07,0x10; castle → 0x2B,0x34,0x35,0x4F,0x56; stage → 0x3B,0x1E; economy → 0x27,0x3A,0x29.

### B. Class 9 (projectiles / flyers)

Tables: **str90** action (`EventsFunctions.cpp:1532`, 32 states); **str91** creation (`EventsFunctions.cpp:1567`, 31 subtypes). Creators set `class=9`, `actionIndex=subtype`, sprite, and a fly-path pointer `dword_0xA0_160x = &str_D7BD6[...]`.

Creation (from switch `Events.cpp:4376+`):

| Sub | Creator : def line | Sprite | Launched by | Kind |
|---|---|---|---|---|
|0|`SummonFireball_4D2E0` :34729|340|fireball spell (:9690,:9952)|spell projectile|
|1|`SummonManaPosession_4D3B0` :34764|209|possess spell (:5462-67)|spell projectile|
|2|`sub_4D470` :34788|211|—|spell projectile|
|3|`sub_4D500` :34810|76|—|spell projectile|
|4|`sub_4D590` :34832|210|—|spell projectile|
|5..0x0C|`sub_4D620`…`sub_4DA20` :34854-34965|per-fn|0x0A = castle cast (`Events.cpp:4421`)|spell/castle projectiles|
|0x0D|`AddEvent09_0D_4DAB0` :35031|—|archers (:9722,:9750)|arrow (PORTED)|
|0x0E-0x1E|`sub_4DBC0`…`sub_4E210` :35053-35288|mixed|(9,20) :9824, (9,21) :9858, (9,9) :9893|creature-like flyers|
|0x1C|`sub_4D380` :34752|340|fireball clone|spell projectile variant|

Action states: 0 = `CastPlayerFire_65B30` (fireball flight), 1 = `CastPosses_65F60`, 2-8 = `sub_66160..662E0` spell flights, 0x0A = `CastCastleProjectile_66B30` (:58461), 0x0C = lightning II `sub_66FD0`, 0x0D = `AddArcherArrow_672E0` (:58852, PORTED), 0x0E-0x1C = flyer ticks (`sub_67410..67940`), 0x12 = possess mana II.

### C. Port waves (class 9/10)

- **Wave 1 — existing machinery:** 10: 0x03,0x06,0x10,0x13 (sprite fx), the smoke family 0x05,0x07,0x0D,0x0E,0x3C,0x57, 0x0C/0x46; 9: fly-path table `str_D7BD6` + states 0/1.
- **Wave 2 — castle subsystem:** 10: 0x2B,0x34,0x35,0x4F,0x56; 9: state 0x0A.
- **Wave 3 — MC2 spell column** (cast via `4A190(...,15,spellIndex)` Level.cpp:1313): 10: 0x11 meteor, 0x16 wind, 0x4C fire-spheres, 0x02/0x0C possess chain; 9: states 0-8.
- **Wave 4 — stage machinery:** 10: 0x3B checkpoint, 0x1E path, quest-point ticks 0x0D/0x40.
- **Wave 5 — invisible logic drivers:** 0x02,0x04,0x09,0x0A,0x0B,0x12,0x1F,0x20,0x5B (pure state machines).
- **Level-data-consumed models** (GenerateEvents, Events.cpp:152-282): pass1 0x52; pass2 0x09,0x0B,0x0F,0x1D,0x1E,0x1F,0x20,0x32,0x33,0x53,0x54,0x55,0x58; pass3 0x50,0x51; also 0x1B,0x1C and 0x2D (bldgprm flag split :258/:271).
- **Code-only-spawned:** 0x00,0x01,0x11,0x16,0x17,0x27,0x3A,0x4C,0x49, the possess/fire chain (10,27/30/32/51 — Events.cpp:5426-5606).

### OPEN (class 9/10)

- strA1 models 0x43/0x47/0x48/0x49: creator addrs outside the read switch range — need the tail of `pre_sub_4A190_axis_3d`.
- Sprite rows for models 0x17-0x59 marked OPEN — read each SetEntityIndex before porting.
- Model 0x58: level data references it but its creator slot returns 0 (`Events.cpp:4852`) — verify.
- `str_D7BD6` fly-path table undecoded — REQUIRED for any class-9 projectile port.
- Action states 0x14,0x15,0x18,0x5F,0x61 not found in the action switch (possibly no-op).

---

## SECTION 2: CLASSES 2 / 3 / 11 / 12-15 + SHARED SUBSYSTEMS (agent report, verbatim)

Dispatch recap (confirmed): `str_D4C48ar[class]` at `EventsFunctions.cpp:2060`; `dword_10` = ACTION table, `dword_14` = CREATION table; addresses resolved by the two `Events.cpp` switches (creation :4166, action :610). Address→fn rule: subtract 0x1E1000 (`0x22Bxxx→sub_4Axxx`, `0x250xxx→sub_6Fxxx`, `0x23Fxxx→sub_5Exxx`). Universal spawner `IfSubtypeCallCreatingManaSphere_4A190(pos, class, model)` (`Events.cpp:5186`) = "create entity" (misnamed).

### 1. CLASS 2 (scenery) — creation `str21` (:1188), action `str20` (:1162)

| Model | Creator (fn:line) | init action | Action-fn | Semantics |
|---|---|---|---|---|
| 0 tree | `AddTree_4AC40` :33433 (ported ctor) | 0 | `AddTree02_00_64E20` :62399 | growth/burn trigger |
| 1 stone | `AddStone_4AD70` (ported) | 3 | `AddStatue02_01_65040` :62518 | inert: draw flag + snap Z |
| 2 dolmen | `AddDolmen_4ADF0` (ported) | 6 | `AddDolmen02_02_65080` :62524 | marks nearby players (byte1|=0x10), snap Z |
| 3 | `sub_4AE80` :33503 | 9 | `sub_65110` :62536 | inert: draw flag + snap Z |
| 4 | `sub_4AF00` :33521 | 0xC | no-op (`Events.cpp:3199`) | pure static |
| 5 | `sub_4AF70` :33538 | 0xF | no-op | pure static |
| 6 cave-bee | `sub_4AFE0` :33555 (Cave only) | 0x12 | `sub_651B0` :62548 | burnable, on death mana (10,13) |
| 7 | `sub_4B0F0` :33587 via `sub_4B150` :33608 (non-Cave) | 0x14 | `sub_652A0`→`sub_652C0` :62599/62606 | falling/physics scenery |
| 8 | `sub_4B120` :33598 (non-Cave) | 0x15 | `sub_65280`→`sub_652C0` :62593 | falling/physics scenery |

**Tree lifespan/burn ticks (we hold inert today):**
- `AddTree02_00_64E20` :62399 — burn-hit flag `word_0x62_98` set → `life -= dword_0x5E_94` (:62417); life<0 → spawn (10,6) (:62421), re-seed life 130..190 (:62435), actionIndex=1 (:62441). Snap Z; on water → spawn (10,5) + despawn (:62452).
- `sub_64F60` :62462 (state 1, burning) — life -= 1/tick (:62469); life<60 → state 2 + charred sprite swap 226/227 (:62479); water→despawn.
- `sub_64FF0` :62500 (state 2, charred stump) — snap Z only; terminal.
- Burnable model 6: `sub_651B0` :62548 — on death actionIndex=19, sprite+4, spawn (10,13) mana.

### 2. CLASS 3 (wizards/castles) — creation `str31` (:1218), action `str30` (:1201)

| Model | Creator (fn:line) | What |
|---|---|---|
| 0 | `AddPlayer_4A920` :33317 | player wizard (life 10000, action 0) |
| 1 | `sub_4A9C0` :33341 | other-player wizard (action 1) |
| 2 | `sub_4AA40` :33362 | CASTLE (life 40000, action 5) |
| 3 | `sub_4ABA0` :33409 | creature-3 (mana 10000, action 7) |
| 4-11 | `sub_4A820..4A900` :33260-33313 | spawn-point recorders → `array_0x2362[0..7]`, return NULL |

Action `str30`: 0=`AddPlayer03_00_5E010` wizard tick, 2=`sub_5E310` die, 5=`BeginOfCastleCreation_5FA70`, 6=`sub_5FCA0` destroy-level, 4=`EndOfCastleProjectile_5F8F0` castle per-tick, B/C=`sub_5F8C0`.

- **Wizard tick** `AddPlayer03_00_5E010` :59955: regen; near own castle 10x (:60020); charge transfer to castle (:59972); life<0 → action 2 die, sound 16 (:60037).
- **Castle machine** via `word_0x2E_46` in `BeginOfCastleCreation_5FA70` :61123: stage 3 spawns (10,42) marker :61182; stage 4 waits for no (10,42) → level advance; stage 5 spawns (10,41) :61203. `EndOfCastleProjectile_5F8F0` :61055 pulls mana from nearby (10,39) spheres tagged to it (:61101-61114); `sub_5FD00` :61240 = the sphere SPLIT.
- MC1-vs-MC2 castle deltas: OPEN (no game branch in the entity code; deltas likely in level data). **BALLOONS: NO SYMBOL EXISTS — MC2 has no balloon entity; collection = castle↔sphere direct. OPEN: confirm nothing fills the role.**

### 3. CLASS 11 (switches) — remaining models

Every model's creator = `AddSwitchXX_50A90(pos, N, N)` :37059 (enumerated Events.cpp:4864-5017). Action `strB0[model]`:

| Model | Action-fn | condition → action |
|---|---|---|
| 5-0x0B | no dispatcher case | inert/reserved |
| 0x0C (12) | `sub_6F2B0` :54431 | X-MARKER single-fire: on proximity → set target actionIndex=12, clear linked class-14 (`word_0x36DFE`) draw flag, despawn. ARMS A CHECKPOINT |
| 0x0D-0x1C | `sub_6F420..6F620` :54510+ → `sub_6F300(a1, slot)` :54457 | slot-condition switches: `bytearray_38403x[slot]` occupied → wait; else countdown → chain-fire + despawn |
| 0x11 (17) | `AddSwitch0B_11_6F4A0` :54534 | "get scroll4" — scroll pickup switch |
| 0x1E (30) | `sub_6F7C0` :54684 → `sub_6F300(a1,-1)` | any-slot variant |
| 0x1F (31) | `sub_6F7E0` :54690 | X-MARKER/level-end: chain-fire, target actionIndex=11, clear linked class-14 (`word_0x36DFC`) |
| 0x20 (32) | `AddSwitch0B_20_6F1C0` :54353 (PORTED) | stage-gated |
| 0x21-0x2C | `sub_6F640..6F7A0` :54612+ → `sub_6F300(a1, slot)` | more slot switches |

Models 16/17 have plain ctors (:37137/:37143); ticks are slot-condition/scroll variants. The X-marker models DO act (arm checkpoints, clear class-14 map objects) — not inert.

### 4. CLASSES 12/13/14/15

| Class | Status |
|---|---|
| 12 | EMPTY table — never assigned in MC2 |
| 13 | pure sentinel class (10 NULL entries) — never assigned |
| 14 | special map objects / animated pickups: creator `sub_514E0` :37315 (+`sub_51570` :37340 model 3 → `word_0x36DFE`, `sub_515C0` :37353 model 4 → `word_0x36DFC`); action: models 0-5 no-op, 6=`sub_59F60`, 7=`sub_5B100`, 8-10=`sub_59C40/59C60`+`UpdateScroll_59C80`. Cleared by switch models 0x0C/0x1F |
| 15 | SPELLS: creator `AddSpellXX_XX_51120` :54124 (26 models, `SetSpell_6D5E0`); action `strF0` = 80 per-spell effect states (`sub_69xxx`). THE MC2 SPELL COLUMN'S entity class |

### 5. SHARED SUBSYSTEMS

- **(a) Multipart/segment chain** (class-5 actionIndex sentinels 0xB4=head?, 0xE8=inactive segment, 0xEA=render-excluded; list rebuild `EventsFunctions.cpp:39969-39998`, collision skip :24486): death propagates via `parentId_0x28_40` + `sub_6D8B0(parentId, code, 1)` (:10861,:10998,:26636,:56243). OPEN: the head/tail spawn ctor.
- **(b) Class-0 conditional spawn / empty-slot sentinel** (`NewEvent_4A050` :561): freed slots ARE class 0 (:590) and every scan skips class 0 (:39971) — a genuine class-0 record is indistinguishable from a dead slot; two free-lists (:565/:581), NULL on exhaustion (:607).
- **(c) Mana-sphere economy**: split `sub_5FD00` :61240 (overflow → ≤32 (10,39) spheres :61301, tagged playerEntityIndex, random launch); merge `EndOfCastleProjectile_5F8F0` :61101 (castle absorbs tagged spheres); wizard fall spawns (10,40) dead-marker and re-tags orphan spheres (:60164-74). Key models: (10,1) wizard-fall mana, (10,5)/(10,6) tree mana, (10,13) creature mana, (10,39) sphere, (10,40) dead marker, (10,41)/(10,42) castle stage markers.
- **(d) Wizard-death spell scatter** `sub_5E310` :60045: on ground hit (:60099) each of 26 `SpellEnabled[i]` entities re-enables as a physical pickup, scattered ±256, life 200..289 (:60137-61); then (10,40) marker, respawn timer 1200.

### PORT WAVES (remaining classes)

1. **Cheap now:** class-2 models 3-8 + the tree burn ticks; class-11 models 5-0x2C (slot-condition boolean machines + the 4 checkpoint/objective handlers); class-15 spell CREATORS (tokens exist/collect).
2. **New subsystems:** mana-sphere economy → wizard tick + death scatter → castle machine (in that dependency order).
3. **Biggest:** multipart chain; class-15 effect ticks (80 states — port per-spell as the campaign needs them).

### OPEN (remaining classes)

- Balloon: no MC2 balloon entity found — identify what (if anything) fills the role.
- MC1-vs-MC2 castle deltas: not in entity code — check Level.cpp/level data.
- Class-2 models 7/8 falling-physics terminal states (19/27) — verify.
- Multipart head/tail spawn ctor not located.

---

## SECTION 3: CLASS 5 — THE CREATURE ROSTER (agent report, verbatim)

Files: `EventsFunctions.cpp` (EF), `Events.cpp` (EV). Class-5 creation table `x_DWORD_D4C52ar_str51[30]` at EF:1481 (29 valid models); action table `x_DWORD_D4C52ar_str50[236]` at EF:1242 — **each model owns exactly 8 states, base = model*8**; model 27 owns extra segment states 0xE9/0xEA; the shared segment state 0xE8 lives under tag 0x2A5C38. Address→symbol: `sub_XXXX = address − 0x1E1000`.

### The canonical 8-state template (confirmed on models 1 and 4)

| base+ | Role | Shared primitive |
|---|---|---|
| +0 | patrol/idle | `sub_1BD90` (EF:8945) |
| +1 | awake/scan (model-specific) | `sub_1BF90` (EF:9064) or custom |
| +2 | attack/pack (model-specific) | custom, may call `sub_1C980` |
| +3 | chase | `sub_1C560` (EF:9345) |
| +4 | prekill | `PreKillEntity_1C890` (EF:9533) |
| +5 | kill | `KillEntity_1C930` (EF:9556) — mana-on-death baked in |
| +6 | hit/attack-apply | `sub_1C980` (EF:9572) |
| +7 | spawn/add | `sub_1D5D0` (EF:9977) |

### Creation table (model → creator)

| Model | Creator fn | Def |
|---|---|---|
|0|`sub_4B240`|EF:33642|
|1|`AddCreature_4B490` (PORTED vulture)|EF:33720|
|2|`sub_4B590`|EF:33751|
|3|`sub_4B6F0`|EF:33797|
|4|`AddArchers_4BA10` (PORTED)|EF:33878|
|5-8, 11|stubs, dword_10=0, no EV case — non-spawnable/reserved (5's addr = `sub_4CB60` EF:34420, model 22's segment HELPER)|OPEN|
|9|`sub_4BBB0`|EF:33912|
|10|`sub_4BD00`|EF:33965|
|12|`sub_4BDF0`|EF:33999|
|13|`AddVilliger_4BF40` (PORTED)|EF:34037|
|14|`AddTrader_4C0B0`|EF:34094|
|15|`sub_4C1E0`|EF:34129|
|16|`sub_4C310`|EF:34163|
|17|`sub_4C460`|EF:34201|
|18|`sub_4C590`|EF:34236|
|19|`sub_4C6B0`|EF:34271|
|20|`sub_4C7F0`|EF:34307|
|21|`sub_4C8F0`|EF:34340|
|22|`sub_4CA00`|EF:34377|
|23|`sub_4CBF0`|EF:34454|
|24|`sub_4CCF0`|EF:34487|
|25|`sub_4CE00`|EF:34523|
|26|`sub_4CF00`|EF:34557|
|27|`sub_4D000`|EF:34591|
|28|`sub_4D1D0`|EF:34695|

### Action-table handler map (state base → 8 handler subs; `sub_` addr = table addr − 0x1E1000)

| Model | States | Handlers (+0..+7) |
|---|---|---|
|0|0x00-07|1EF20,1EF40,1EF70,1EFD0,1F000,1F020,1F2B0,1F300|
|1|0x08-0F|1F340,1F3C0,1F440,1F470,1F4F0,1F510,1F530,1F5B0|
|2|0x10-17|1F630,1F660,1F6D0,1F800,1F830,1F850,1F870,1F8A0|
|3|0x18-1F|1F950,1F970,1F990,1F9E0,1FA00,1FA20,1FA40,1FA50|
|4|0x20-27|1FA70,1FAA0,1FF40,1FFE0,20010,20040,20130,20140|
|5-8, 11|0x28-47, 0x58-5F|tiny stubs|
|9|0x48-4F|20370,203D0,20C50,20E50,20E80,20EA0,20FB0,20FC0|
|10|0x50-57|21030,22530,22540,22550,22560,22580,225A0,225B0|
|12|0x60-67|22760,22C80,22E60,23020,231E0,23200,23260,232A0|
|13|0x68-6F|23320,23340,23640,23660,23680,236F0,23710,23750|
|14|0x70-77|23790,237B0,23AC0,23AE0,23B00,23B30,23B90,23BD0|
|15|0x78-7F|23C20,23C40,23E60,240A0,240C0,240E0,243F0,24400|
|16|0x80-87|24420,24440,24510,247D0,247F0,24810,24830,24840|
|17|0x88-8F|24860,248C0,24930,24D40,24DA0,24DC0,24DE0,24DF0|
|18|0x90-97|24E20,25050,250B0,25280,252A0,252C0,25540,25550|
|19|0x98-9F|25590,255C0,25610,25CD0,25D00,25D20,25D40,25D50|
|20|0xA0-A7|25D80,25DE0,25E40,25F70,25FD0,25FF0,26010,26020|
|21|0xA8-AF|26050,26070,26220,263C0,263E0,26400,26420,26470|
|22|0xB0-B7|26960,26990,26AA0,26BD0,26CA0(=0xB4 chain-kill),26CC0,27920,27930|
|23|0xB8-BF|27950,27B20,27E00,27C10,27FA0,27FC0,28460,28470|
|24|0xC0-C7|28490,28500,28570,285D0,285F0,28610,28630,28660|
|25|0xC8-CF|28860,28C30,28C60,28CC0,28CE0,28EC0,28F40,28F50|
|26|0xD0-D7|28F90,28FC0,28FF0,29300,29330,29350,29370,29380|
|27|0xD8-DF+E9/EA|29400,29670,29710,29890,298B0,298D0,29920,29930; 0xE9/0xEA NULL (special)|
|28|0xE0-E7|2B1D0,2B200,2B260,2B750,2B760,2B780,2B7A0,2B7B0|
|shared|0xE8|`sub_1B6B0` (EF:8696) — multipart segment tick|

### Per-model ctor inventory

| M | Name | initState | min/max spd | maxLife | sprite | row | RNG | ShiftRot | Notes |
|---|---|---|---|---|---|---|---|---|---|
|0|Multipart worm/hydra|1|80/16|4000|40|71|1|—|16 children state 0xE8, chain word_0x32/0x34 (EF:33691-712); mana 4500|
|1|Vulture ✔|9|54/18|600|238|98|0|—|sound 46|
|2|(5,2)|0x11|64/30|3000|3|73|1|128,128|DAY-ONLY (EF:33758), subSpell 200|
|3|Multipart flyer|0x19|64/16|9000|88|74|1|—|16 children 0xE8, sprites 89+i (EF:33836-71)|
|4|Archers ✔|0x21|30/0|1000|0|75|1|128,256|attack targets (10,45) buildings (EF:11828)|
|9|(5,9)|0x48|20/0|1000|220|80|1|128,128|despawn if blocked (EF:33955)|
|10|Mana structure|0x50|—|300000|341|107|0|1024,1280|static flag 0x48800001; cave-flag gated (EF:33969)|
|12|(5,12)|0x61|54/24|1000|221|101|1|128,128|subSpell 500|
|13|Villager ✔|0x69|54/18|1000|242/271/241/239|100|2|128,128|4-way sprite variant (EF:34066)|
|14|Trader|0x71|54/18|1000|219|100|1|128,128|same row as villager|
|15|(5,15)|0x79|30/0|1000|0|83|0|128,128|byte[2]|=2 (EF:34153)|
|16|(5,16) boss|0x81|60/20|60000|207|84|1|128,128||
|17|(5,17)|0x89|68/20|10000|285|85|1|128,128|subSpell 350|
|18|slow tank|0x93|10/6|36000|286|86|1|512,512||
|19|(5,19)|0x99|76/8|600|287|88|1|85,51|fast/fragile; class-9-in-CTOR claim REFUTED (EF:34271) — if real it lives in handlers 255C0/25610|
|20|(5,20)|0xA1|32/20|5500|288|89|1|384,512|subSpell 100|
|21|(5,21)|0xA9|96/?|1000|—|96|1|128,128|non-standard init sub_26500/268F0 (EF:34369)|
|22|Segmented flyer|0xB0|128/16|2000|—|90|1|—|tail via sub_4CB60→sub_274C0; chain-kill 0xB4 sub_26CA0 (EF:17420-27); spawns alt+384|
|23|(5,23)|0xB8|24/14|10000|289|91|1|384,384|spawns z=0x2000 (high/flying); mana 100|
|24|CAVE-ONLY|0xC1|80/24|16000|335|102|1|256,640|(EF:34490), subSpell 1500|
|25|(5,25)|0xC9|60/20|7500|290|92|1|384,384|subSpell 300|
|26|(5,26)|0xD1|25/25|4400|318|99|1|256,384|post-init sub_293D0 (EF:34585)|
|27|**HYDRA** (a.k.a. "multipart tree/kraken" in older notes — player-identified 2026-07-16)|0xD9|—|—|—|—|0|—|body + 5 heads 0xE9 + 9 segs each 0xEA (EF:34621-67); heads fire bolts and retract-regrow on "death" (f71=6 → body gauge--, case 0xA gauge++); body attackable ONLY at gauge 0 = all heads down; NON-CAVE only; needs ≥51 free slots; special init sub_2AC50/2AD40/2AE30|
|28|(5,28) fastest|0xE1|120/64|8000|292|93|1|85,42|byte[3]|=8 (EF:34707); subSpell 2000|

### Dependency summary (class 5)

- **A. Shared-primitives-only (cheap):** 2, 9, 12, 14, 15, 16, 17, 18, 19, 20, 25, 26, 28 (+ ported 1/4/13). Per-model attack logic in base+1/+2 may need reading, but the movement/prekill/kill core is done.
- **B. Multipart subsystem needed:** 0, 3, 22, 27 (chain links word_0x32/word_0x34, shared segment tick sub_1B6B0 @0xE8, chain-kill walker sub_26CC0 @0xB4, model-27's 3-tier tree with NULL-dispatch states 0xE9/0xEA).
- **C. Class-9 launches:** which model spawns which projectile is NOT in the ctors — resolve per attack handler during the sweep (archer's sub_1FAA0 pattern is the ported template).
- **D. Misc:** map-type gates (2 day, 24 cave, 27 non-cave, 10 cave-flag); flying spawns (22, 23); non-standard inits (21, 26); mana-on-death baked into shared KillEntity.

### OPEN (class 5)

1. Models 5/6/7/8/11 = non-spawnable stubs — confirm reserved.
2. Per-model class-9 projectile linkage (trace each base+1/+2 during port).
3. Per-model sound ids beyond model 1 (46).
4. No water-locomotion flag found in ctors (behavior-row flags carry it — our BEHAVIOR extraction already has v_20/bit flags).
5. (5,19) flyer claim refuted at ctor level; the earlier ":34882" note pointed into the class-9 creator block — re-derive during the sweep.

---

## CONSOLIDATED SWEEP PLAN (synthesis)

1. **Class-5 wave A** (13 models on shared primitives) + class-2 models 3-8 + tree burn ticks + class-11 slot switches (5..0x2C incl. 12/16/17/31 + class-14 map objects they arm) — pure grind against this doc.
2. **Multipart subsystem** → class-5 models 0, 3, 22, 27 (level-000's (5,3) unblocks here).
3. **Class-9 flight states** (str_D7BD6 fly-path table first) → resolves the creature-launched projectiles + the (5,19) question; unblocks level-000's final wave.
4. **Class-10 wave 1** (sprite fx + smoke family + invisible drivers) — closes most level-data misfits ((10,60) smoke, (10,29), (10,0x0F/1D/1E/1F/20/32/33/50/51/53/54/55/58...)).
5. **Mana-sphere economy** → wizard tick/death scatter → castle machine (dependency order) — the MC2 win loop.
6. **Class-15 spell entities** (ctors first = collectible tokens; effect ticks per-spell as the campaign needs them) — the MC2 spell column.
7. THEN the LEVEL GRIND (4.3b): misfit ledger + fallback telemetry per level as the deviation worklist.

---

## SECTION 4: AUTHORED-RECORD CENSUS (2026-07-09, examples/rostercensus.rs over all baked mc2 levels)

What the campaign actually authors — the sweep-priority signal. Count = THING records across all 213 baked levels; classes 0 (conditional spawns), 3 (wizards/starts) shown for completeness.

```
( 0,  0) x 1116  37 levels
( 0,  1) x  277  26 levels
( 0,  2) x  428  22 levels
( 0,  3) x   36  12 levels
( 0,  4) x  143  15 levels
( 0,  5) x  580  22 levels
( 0,  6) x  373  20 levels
( 0,  7) x   39  level-001,level-056,level-122,level-134,level-139,level-141
( 0,  8) x   20  level-001,level-005,level-122,level-141,level-169
( 0,  9) x  937  18 levels
( 0, 10) x   12  level-056,level-122,level-134,level-180
( 0, 11) x  241  15 levels
( 0, 12) x    2  level-030,level-141
( 0, 13) x  151  13 levels
( 0, 14) x   41  9 levels
( 0, 16) x   35  8 levels
( 0, 17) x  136  10 levels
( 0, 18) x   79  9 levels
( 0, 19) x  283  12 levels
( 0, 20) x  364  17 levels
( 0, 21) x  129  7 levels
( 0, 22) x   35  9 levels
( 0, 23) x   27  8 levels
( 0, 24) x   69  level-030,level-116,level-140
( 0, 25) x  115  11 levels
( 0, 26) x   64  8 levels
( 0, 27) x   18  level-050,level-060,level-112,level-168
( 0, 28) x   14  level-038,level-039,level-112,level-159,level-168
( 0, 31) x   13  level-011,level-077,level-127
( 0, 32) x   22  9 levels
( 0, 34) x   38  9 levels
( 0, 35) x    3  level-038,level-039,level-101
( 0, 36) x    1  level-005
( 0, 39) x  158  15 levels
( 0, 42) x    1  level-168
( 0, 43) x    2  level-112,level-168
( 0, 45) x  138  24 levels
( 0, 50) x    4  level-046
( 0, 54) x    6  level-011,level-077,level-098,level-141
( 0, 58) x   45  level-056,level-066,level-077,level-112,level-122,level-134
( 0, 59) x   30  level-011,level-013,level-056,level-122,level-134,level-147
( 0, 60) x   16  level-001,level-005,level-088,level-098,level-141,level-162
( 0, 64) x   20  level-030
( 0, 71) x    2  level-027
( 0, 76) x   92  7 levels
( 0, 80) x   21  level-077,level-094,level-111,level-164
( 0, 82) x    1  level-077
( 0, 83) x  127  7 levels
( 0, 84) x   10  level-030,level-094
( 0, 85) x    6  level-030,level-094,level-147
( 2,  0) x 3953  41 levels
( 2,  1) x  912  78 levels
( 2,  2) x  203  55 levels
( 2,  3) x  307  39 levels
( 2,  6) x 1382  25 levels
( 2,  7) x  339  19 levels
( 2,  8) x  247  20 levels
( 3,  4) x  155  153 levels
( 3,  5) x   67  67 levels
( 3,  6) x   56  56 levels
( 3,  7) x   54  54 levels
( 3,  8) x   48  48 levels
( 3,  9) x   46  46 levels
( 3, 10) x   45  45 levels
( 3, 11) x   39  39 levels
( 5,  0) x  668  77 levels
( 5,  1) x 1491  39 levels
( 5,  2) x 1967  58 levels
( 5,  3) x  519  65 levels
( 5,  4) x 1550  53 levels
( 5,  9) x 4577  80 levels
( 5, 10) x   41  level-024,level-027,level-046,level-062,level-122,level-180
( 5, 12) x   67  9 levels
( 5, 13) x  945  31 levels
( 5, 14) x  240  23 levels
( 5, 16) x  261  49 levels
( 5, 17) x 1704  59 levels
( 5, 18) x  678  78 levels
( 5, 19) x 2538  68 levels
( 5, 20) x 1982  98 levels
( 5, 21) x 2804  69 levels
( 5, 22) x  191  44 levels
( 5, 23) x  224  37 levels
( 5, 24) x  604  16 levels
( 5, 25) x 1738  74 levels
( 5, 26) x 1205  61 levels
( 5, 27) x   43  26 levels
( 5, 28) x  373  46 levels
(10,  0) x  355  29 levels
(10,  1) x  774  43 levels
(10,  5) x  202  24 levels
(10,  6) x  973  43 levels
(10,  8) x   11  level-010,level-054
(10,  9) x  286  47 levels
(10, 11) x 1265  61 levels
(10, 13) x  976  35 levels
(10, 14) x  287  28 levels
(10, 15) x   52  14 levels
(10, 17) x   69  15 levels
(10, 22) x  102  16 levels
(10, 23) x   10  level-062,level-189
(10, 25) x  104  level-009,level-024,level-030
(10, 28) x 1424  35 levels
(10, 29) x 2145  34 levels
(10, 31) x 1073  28 levels
(10, 34) x  312  45 levels
(10, 39) x 3822  81 levels
(10, 45) x 3495  116 levels
(10, 50) x  695  15 levels
(10, 52) x    6  level-063,level-189
(10, 54) x  157  37 levels
(10, 57) x   89  11 levels
(10, 58) x 1509  66 levels
(10, 59) x  137  43 levels
(10, 60) x  176  31 levels
(10, 63) x  311  44 levels
(10, 64) x  413  33 levels
(10, 67) x   49  9 levels
(10, 71) x   21  level-012,level-018,level-024,level-027,level-046,level-139
(10, 76) x  172  25 levels
(10, 80) x 3033  45 levels
(10, 82) x  333  42 levels
(10, 83) x 1696  36 levels
(10, 84) x 1000  38 levels
(10, 85) x  854  35 levels
(10, 86) x    7  level-003
(11,  0) x 1556  132 levels
(11,  1) x   97  57 levels
(11,  2) x  238  57 levels
(11,  3) x   68  21 levels
(11,  4) x   79  79 levels
(11, 12) x   84  83 levels
(11, 13) x   23  22 levels
(11, 14) x   12  12 levels
(11, 15) x    9  9 levels
(11, 16) x   16  15 levels
(11, 17) x   14  14 levels
(11, 22) x   27  24 levels
(11, 29) x   27  15 levels
(11, 30) x   10  level-024,level-027,level-065,level-130,level-178
(11, 31) x    7  7 levels
(11, 32) x  218  77 levels
(11, 33) x   22  18 levels
(11, 34) x   33  24 levels
(11, 35) x   22  14 levels
(11, 36) x   39  27 levels
(11, 37) x   29  23 levels
(11, 38) x    3  level-059,level-099,level-168
(11, 39) x   11  11 levels
(11, 40) x    1  level-115
(11, 41) x   13  12 levels
(11, 42) x   23  23 levels
(11, 43) x    5  level-019,level-046,level-054,level-060,level-165
(11, 44) x    9  9 levels
(14,  1) x  158  25 levels
(14,  2) x  244  23 levels
(14,  3) x   82  79 levels
(14,  4) x    7  7 levels
(14,  5) x 2577  52 levels
(15,  0) x   30  28 levels
(15,  1) x   46  29 levels
(15,  2) x   71  36 levels
(15,  3) x   19  19 levels
(15,  4) x   41  37 levels
(15,  5) x   47  46 levels
(15,  6) x   87  50 levels
(15,  7) x   40  38 levels
(15,  8) x   39  37 levels
(15,  9) x   54  48 levels
(15, 10) x   43  38 levels
(15, 11) x   44  43 levels
(15, 12) x   40  37 levels
(15, 13) x   24  24 levels
(15, 14) x   21  20 levels
(15, 15) x   32  30 levels
(15, 16) x   38  31 levels
(15, 17) x   19  19 levels
(15, 18) x   20  20 levels
(15, 19) x   38  35 levels
(15, 20) x   27  24 levels
(15, 21) x   36  33 levels
(15, 22) x   28  24 levels
(15, 23) x   29  29 levels
(15, 24) x   39  22 levels
(15, 25) x    8  8 levels
```

Reading: class-5 heavy hitters m9 x4577, m21 x2804, m19 x2538, m20 x1982, m2 x1967, m25/m17 ~1700 — wave A + m21 covers the bulk. m5/6/7/8/11/15 NEVER authored (m15 code-spawn only?). Class-2: m4/m5 never authored. Class-11 authored set ⊂ 0..=44. Class-14 models 1-5 only (m5 x2577!). Class-15: all 26 models authored. Class-10 authored beyond ported: 5,6,8,9,11,13,14,15,17,22,23,25,28,29,31,39,50,52,54,57,58,59,60,63,64,67,71,76,80,82,83,84,85,86 — (10,39) x3822 (mana spheres!), (10,58) x1509 (the returns-0 creator discrepancy — heavily authored!), (10,29) x2145, (10,28) x1424. Class-0 models span 0..=85 (the conditional-spawn machinery mirrors the class-10 model space).
