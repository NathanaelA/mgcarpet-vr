# MC2 music law (decompile trace)

> **⚠️ 2026-07-14: gameplay music = MUSIC.DAT GM driver BANK 0 (the "C2"/MC2 set), NOT bank 1.** The default `musicChannel_E3814=0` selects bank 0 at boot (Sound.cpp:49/801); bank 1 (the "C1"/MC1 set) loads only under the hidden `-music2` flag. §4(A) below said "pick the G section" without a bank — the companion trace `mc2-music-dat-xmi.md` (its correction banner) has the full bank story. Baking bank 1 shipped MC1's music as MC2 gameplay (fixed BAKE_EPOCH 11). Bank 1 = a future opt-in "MC1 classic soundtrack".

Scope: what MC2 plays during **gameplay** vs **briefing/speech**, and how to port it.
All citations are `reference/remc2/remc2/engine/<file>:<line>`. `EF` = `EventsFunctions.cpp`.

Player ground-truth (framing): MC2 has BOTH a per-level **CD speech track** carrying the
quest-goal voiceovers (one continuous redbook track, sub-divided by an INDEX into little
segments) AND **MIDI/XMI background music** for gameplay. The decompile confirms both are
two entirely separate subsystems, selected differently.

---

## 1. In-game (gameplay) music — the definitive law

**Source:** local file `SOUND/MUSIC.DAT` (NOT the CD/redbook), played through the AIL/Miles
XMI sequencer. It is **never** redbook audio.

**Selection formula — by MapType only, not per-level:**

```
switch (terrain.MapType) {         // EF:31441-31449
  case Day:   maptypeMusic_0x235 = 2;
  case Night: maptypeMusic_0x235 = 1;
  case Cave:  maptypeMusic_0x235 = 3;
}
```

- Field: `D41A0_0.maptypeMusic_0x235`, level-struct offset **0x235 (565)** — `LevelStructs.h:214`
  (`int32_t maptypeMusic_0x235;//act music`). NOTE: the roadmap's "+576" is **imprecise**; the
  real offset is 0x235. There is no music field at 0x240.
- The switch only runs while `musicAble_E37FC && musicActive_E37FD && m_iNumberOfTracks` (EF:31439).
  So the entire selection is gated on the XMI music bank having loaded successfully.

**Played by** `StartMusic_8E160(track, volume)` — `Sound.cpp:869`:
```
AilInitSequence_95C00(m_hMusicSequence, musicHeader_E3808->str_8.track_10[track].xmiData_0, 0, track);
AilStartSequence_95D50(...); songCurrentlyPlaying_E3802 = track;      // Sound.cpp:889,900
```
Gate at Sound.cpp:871: `musicAble && musicActive && track <= m_iNumberOfTracks && songCurrentlyPlaying != track`
(re-selecting the same track is a no-op → no restart on MapType re-eval).

**Play sites (all pass `maptypeMusic_0x235`, volume 0x7F):**
- Level start / turn 1: EF:31662-31664 (`StopMusic_8E020(); StartMusic_8E160(maptypeMusic, 0x7F)`).
- Enter game after menu: `PlayerInput.cpp:459` (guarded by `musicActive_E37FD`).
- Music-toggle ON: `PlayerInput.cpp:1211-1213`.
- Resume after options: `Sound.cpp:6555`.

**Loop behavior:** XMI sequences loop natively via embedded XMI FOR controllers
(cc 116 = FOR-start, cc 117 = FOR-next; see `XmiInfo.h` `XMI_FindLoopEvents`). One track plays
per MapType and loops forever until MapType changes or the level ends. It does **not** cycle
through a playlist. Track index maps: **1 = Night, 2 = Day, 3 = Cave** (menus use track 4,
`MenusAndIntros.cpp:832`).

### MUSIC.DAT container layout (`InitMusicBank_8EAD0`, Sound.cpp:5498)
```
sprintf(path,"%s/SOUND/MUSIC.DAT",cdDataPath);
seek(EOF-4); read u32 datapos;        // trailer points at the driver index
seek(datapos); read 8 bytes = int16 driverarray[4];   // per-driver track counts
```
**Driver sections** (`musicDriverType_180C84`, chosen from the AIL .MDI driver name at Sound.cpp:721-796):
| char | index | hardware | .MDI drivers that select it |
|------|-------|----------|------------------------------|
| G/g  | 0 | General MIDI / MPU-401 / MT32MPU-as-GM | MPU401, SNDSCAPE, (SBAWE32 w/ GM) |
| R/r  | 1 | Roland MT-32 | MT32MPU |
| F/f  | 2 | FM / AdLib / SoundBlaster | ADLIB, OPL3, SBLASTER, SBPRO1/2, ESFM, PAS… |
| W/w  | 3 | SB AWE32 wavetable | SBAWE32 |

Each driver has its own bank; `channellplus <= driverarray[finaldrivernumber]` bounds-checks
(Sound.cpp:5544). Per-track header struct `sub2type_E3808_music_header`
(`portability/port_sdl_sound.h:101-107`): `{uint8_t* xmiData_0; int8 stub_4[4]; int32 xmiSize_8;
int16 word_12; int8 filename_14[18];}`, 6 tracks per bank (`track_10[6]`, .h:111). Track blob may
be **RNC-compressed** (magic `RNC`, decompressed via `DataFileRNC::Decompress`, Sound.cpp:5614).
The blob at `xmiData_0` is **XMI** (IFF FORM/XDIR/XMID/EVNT), fed straight to `AilInitSequence`.

---

## 2. Briefing / speech law (CD redbook — do NOT confuse with §1)

The redbook CD tracks carry **spoken objective/briefing lines**, addressed as
`(trackIdx, segmentIdx)` via the `CdTracks_DB080` table.

**Segment index table** = `CdTracks_DB080[28]` — `Type_DB080_CdTrack.h:19`. This IS the
"unique-to-MC2 index" the player described. It is a **static compiled table** (not a runtime
file), one entry per CD track:
```c
typedef struct { int32 startPos_0; int32 length_2; } Type_DB080_TrackSegment; // seconds
typedef struct { int8 TrackIdx_0;                       // physical CD track number
                 Type_DB080_TrackSegment TrackSegments_DB080[10]; } Type_DB080_CdTrack;
Type_DB080_CdTrack CdTracks_DB080[28];
```
- 28 entries, `TrackIdx_0` = 1..28 (sequential). Each holds up to **10 (start, length)** segment
  pairs in CD-frame units. Entries 0-25 = campaign level speech; entries 26-27 (`(a1!=0)+25`) =
  the two secret-level tracks.

**Player** `PlayCDTrackSegmentNumber_86EB0(trackIdx, segmentIdx, paletteFlash)` — EF:47987:
```c
trackId = CdTracks_DB080[trackIdx].TrackIdx_0;
startPos = CdTracks_DB080[trackIdx].TrackSegments_DB080[segmentIdx].startPos_0 * 13.3333; // →ms
length   = CdTracks_DB080[trackIdx].TrackSegments_DB080[segmentIdx].length_2   * 13.3333;
if (trackId && length) PlayCDTrackSegment_86FF0(trackId, startPos, length);      // EF:48056
```
`13.33333` = 400/30: the stored units are CD **frames at 75fps scaled**, converted to ms.
`PlayCDTrackSegment_86FF0` gates on `cdSpeechEnabled_E2A28 && (musicAble || soundAble)`, applies a
runtime lead-in via `TrackOffsets_180084[]` (filled from the real drive's TOC in
`QueryCdTrack_86370`, EF:47921-47965), then plays that byte range only. `…ForSecretLevel_86F20`
(EF:48014) is the secret-level variant; `…WithPaletteFlash_86F70` (EF:48035) wraps it with a
palette-fade timer.

**Call sites (all briefing/objective UI, never gameplay BGM):**
- Objective box during briefing: **track = level number, segment = `ObjectiveText_1 + 1`**
  (EF:41034-41038). `ObjectiveText_1` is the objective index in the mission struct
  (`type_substr_3659C`, `LevelStructs.h:191-196`). segment 9 = level-end line, segment 0/etc for
  special levels 30-34.
- Secret-level objective: EF:41012.
- Map-screen level description (`PresentLevelDescription_80C30`): `MenusAndIntros.cpp:3599`,
  `PlayCDTrackSegmentNumber_86EB0(levelIdx, 0, 0)` — plays segment 0 (the level's intro line),
  gated on `OptionsSettingFlag & 0x40` (speech-enabled) and `!IsPlayingCDTrack`.

So the redbook CD is **one track per level, chopped into ≤10 voiceover clips by a static
(start,length) index**, triggered by the briefing/map UI. It is never the background score.

---

## 3. Script cases 12 / 25 — what the roadmap note actually points at

**Finding: there is no music/speech "script opcode 12/25".** The roadmap phrasing does not
resolve to a real music case. Every `case 12`/`case 0xC` and `case 25`/`case 0x19` in the engine
is unrelated to music:
- `MenusAndIntros.cpp:1861` `case 12/13` → keyboard-glyph name lookup (`"- "`).
- `EF:37711 case 0x12 / 37751 case 0x19` → player-input/menu event handler (name entry, exit).
- `EF:54862 case 0xC / 54983 case 0x19` → mob AI target-acquisition switch.
- The mission/stage script (`StageVars_0x3647A`, decoded in `InitStageVars_11EE0`, EF:4629) uses
  **low-nibble opcodes 1-9** (spatial/entity triggers), not 12/25, and does not touch music.

The only per-level "song command" that exists is the **objective-segment index**
`ObjectiveText_1 + 1` from §2 (the closest match to "per-level song command"), and the
**MapType→track** map from §1. Treat the "+576 / cases 12,25" note as superseded by this trace.

---

## 4. Port recommendation for our bundle

Two independent tracks, mirroring retail:

**(A) Gameplay music = MapType XMI (replace the interim redbook loop).**
Our interim pick `track-{2 + level%27}` (`crates/mgc-app/src/main.rs:519`) is wrong on both
counts — wrong source (redbook, which is speech) and wrong selection (per-level cycling). Retail
plays exactly ONE looping XMI chosen by MapType. Recommended:
- Import `SOUND/MUSIC.DAT`: parse trailer u32 → driver index (`int16[4]`), pick the **G (GM)**
  section (index 0) to match our HMP→GM fluidsynth path, RNC-decompress each track blob if `RNC`,
  and treat each blob as **XMI**. Our pipeline currently ingests **HMP** (`smf.rs`,
  `MUSIC<bank>-2`) → SMF → fluidsynth; MC2 needs an **XMI→SMF** converter step (parse
  FORM/XDIR/XMID, EVNT delta-times are XMI variable interval + fixed-length note-off; XMI FOR
  loops via cc116/117 — `XmiInfo.h` already locates them). After that the existing SMF→fluidsynth
  →FLAC bake and the danger/GM variants apply unchanged.
- Bake 3 gameplay tracks: **track 1 = Night, 2 = Day, 3 = Cave** (+ track 4 = menu). Name them by
  role in `mc2-audio/music.json`, e.g. `mc2-night/day/cave`.
- App: select by the level's MapType (Day/Night/Cave), start once at level load, loop; restart
  only on MapType change. No `% 27`.

**(B) Objective speech = redbook segments (new, optional).**
Keep tracks 02..28 (they are the speech tracks, correctly ripped). Port `CdTracks_DB080` verbatim
(28×[trackIdx + 10×(startPos,length)]) into the bundle, converting `startPos/length` to ms via
`×13.33333`. Play `segment = ObjectiveText+1` (objective box) / `segment 0` (map description) as
one-shot clips, gated behind a speech-enabled option. This is a separate feature from music and
can land later.

**Minimum viable step:** implement (A) — MapType XMI via MUSIC.DAT/GM — and drop the redbook
music loop. That alone makes gameplay music faithful. (B) restores the voiceovers.
