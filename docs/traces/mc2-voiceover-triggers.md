# MC2 CD Redbook Voiceover / Speech Playback Law — Definitive Trace

VERBATIM trace of the MC2 CD-audio (spoken objective/briefing) subsystem from the
remc2 decompile, port-ready. Supersedes and expands §2 of
`docs/traces/mc2-music-law.md`. Cross-references the stage/objective engine in
`docs/traces/mc2-stage-engine-completion.md`.

All citations are `reference/remc2/remc2/engine/<file>:<line>` unless a
`portability/` prefix is given. `EF` = `EventsFunctions.cpp`,
`M&I` = `MenusAndIntros.cpp`, `GU` = `GameUI.cpp`.

VERIFIED = read the code. INFERRED = deduced from context. OPEN = unresolved.

---

## 0. The law in one paragraph (VERIFIED)

The redbook CD carries **one continuous audio track per level**, chopped into up to
**10 spoken segments** by the static compiled table `CdTracks_DB080[28]`
(`Type_DB080_CdTrack.h:19`). A segment is played by
`PlayCDTrackSegmentNumber_86EB0(trackIdx, segmentIdx, paletteFlash)` (EF:47987), which
looks up `(startPos, length)` from the table, multiplies each by **13.33333** to convert
the stored **CD-frame** units to **milliseconds**, and hands them to
`PlayCDTrackSegment_86FF0` (EF:48056). There are **exactly three call sites**, all UI —
never gameplay BGM:

1. **Map-screen level description** — `PresentLevelDescription_80C30` (M&I:3599):
   `PlayCDTrackSegmentNumber_86EB0(levelIdx, 0, 0)` plays **segment 0** (the level's intro
   line) when you hover a level portal on the world map.
2. **The objective box** — `PresentObjective_59820` (EF:41038), called **every game tick**
   (EF:31818): plays **segment `ObjectiveText_1 + 1`** for the current objective, or
   **segment 9** at level-end. This fires both at the level-start briefing AND **in-game
   every time an objective advances** — that is the "stage trigger → voiceover" the player
   observed. Gated behind a **staged delay ramp** (`byte_0x36E02` 1→8) and the
   speech-enabled option.
3. **Secret-level objective** — `PresentObjectiveForSecretLevel` path (EF:41012):
   `PlayCDTrackSegmentForSecretLevel_86F20(secretIdx)` on the beacon/switch objective path.

The trigger that starts the ramp is the single byte `D41A0_0.byte_0x36E02`, set to 1 when
the **current** objective row completes or the level ends (EF:40911, the stage-engine advance
pass), at level load (LevelInit.cpp:41), and by the type-31 beacon switch
(EF:37307). No speech is ever fired by gameplay BGM logic; the two audio subsystems are
fully separate (see mc2-music-law.md §1).

---

## 1. The verbatim `CdTracks_DB080` table (VERIFIED — DUMPED COMPLETE)

Definition: **`Type_DB080_CdTrack.h:19-48`** (the whole array is a compiled initializer in
the header — not a runtime file). Struct: `Type_DB080_CdTrack.h:5-16`.

```c
#pragma pack(push, 1)
typedef struct {              // length 4  — but note the fields are int32 in the decl,
    int32_t startPos_0;       //   yet the ORIGINAL packs them as int16 (see §1a).
    int32_t length_2;
} Type_DB080_TrackSegment;

typedef struct {              // length 42
    int8_t TrackIdx_0;        // physical CD track number (1..28)
    Type_DB080_TrackSegment TrackSegments_DB080[10];
} Type_DB080_CdTrack;
#pragma pack(pop)

Type_DB080_CdTrack CdTracks_DB080[28];
```

All 28 entries, verbatim (index = 0-based table row; TrackIdx = physical CD track;
each is 10 `{startPos, length}` pairs in **CD-frame units** — multiply by 13.33333 for ms):

```
idx  TrackIdx  segments (startPos,length) × 10   [values hex as in source]
 0   0x01  {0,2EE}{339,12C}{4B0,1C2}{6BD,1C2}{8CA,177}{A8C,177}{0,0}{0,0}{0,0}{C4E,20D}
 1   0x02  {0,465}{4B0,20D}{708,258}{9AB,12C}{B22,20D}{D7A,1C2}{0,0}{0,0}{0,0}{F87,258}
 2   0x03  {0,384}{3CF,2A3}{6BD,177}{87F,1C2}{A8C,177}{C4E,12C}{0,0}{0,0}{0,0}{DC5,177}
 3   0x04  {0,20D}{258,12C}{3CF,1C2}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{5DC,258}
 4   0x05  {0,2EE}{339,177}{4FB,12C}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{672,2A3}
 5   0x06  {0,384}{3CF,177}{591,177}{753,20D}{9AB,1C2}{0,0}{0,0}{0,0}{0,0}{BB8,177}
 6   0x07  {0,465}{4B0,177}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{672,20D}
 7   0x08  {0,384}{3CF,1C2}{5DC,177}{79E,12C}{915,12C}{A8C,258}{D2F,20D}{0,0}{0,0}{F87,12C}
 8   0x09  {0,3CF}{41A,177}{5DC,96}{6BD,E1}{7E9,20D}{A41,20D}{0,0}{0,0}{0,0}{C99,20D}
 9   0x0A  {0,2A3}{2EE,20D}{546,177}{708,258}{9AB,20D}{0,0}{0,0}{0,0}{0,0}{C03,12C}
10   0x0B  {0,258}{2A3,177}{465,20D}{6BD,177}{87F,1C2}{0,0}{0,0}{0,0}{0,0}{A8C,12C}
11   0x0C  {0,546}{591,258}{834,1C2}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{A41,12C}
12   0x0D  {0,3CF}{41A,1C2}{627,177}{7E9,177}{9AB,12C}{0,0}{0,0}{0,0}{0,0}{B22,1C2}
13   0x0E  {0,2EE}{339,2A3}{627,20D}{87F,1C2}{A8C,177}{0,0}{0,0}{0,0}{0,0}{C4E,20D}
14   0x0F  {0,465}{4B0,E1}{5DC,20D}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{834,20D}
15   0x10  {0,339}{384,20D}{5DC,E1}{708,E1}{834,177}{9F6,12C}{0,0}{0,0}{0,0}{B6D,177}
16   0x11  {0,465}{4B0,177}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{672,12C}
17   0x12  {0,5DC}{627,1C2}{834,12C}{9AB,1C2}{BB8,177}{0,0}{0,0}{0,0}{0,0}{D7A,12C}
18   0x13  {0,20D}{258,12C}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{3CF,E1}
19   0x14  {0,339}{384,20D}{5DC,20D}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{834,1C2}
20   0x15  {0,339}{384,1C2}{591,177}{753,20D}{0,0}{0,0}{0,0}{0,0}{0,0}{9AB,177}
21   0x16  {0,465}{4B0,E1}{5DC,20D}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{834,1C2}
22   0x17  {0,3CF}{41A,12C}{591,177}{753,20D}{9AB,12C}{B22,12C}{C99,1C2}{EA6,177}{0,0}{1068,12C}
23   0x18  {0,339}{384,177}{546,20D}{79E,12C}{915,12C}{A8C,12C}{0,0}{0,0}{0,0}{C03,177}
24   0x19  {0,4B0}{0,0}{4FB,2A3}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{7E9,627}
25   0x1A  {0,177}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}
26   0x1B  {0,177}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}{0,0}
27   0x1C  {5BD0,107}{5BE0,107}{5BF0,107}{5C00,107}{5C10,107}{5C24,107}{5C34,107}{5C44,107}{5C60,107}{5C70,107}
```

Notes on the table (VERIFIED unless flagged):

- **Row 27 (0x1C) is anomalous**: all-nonzero, 10 equal-length (0x107) clips at very high
  startPos (0x5BD0 = 23504 frames ≈ 5m13s in). This is the **secret-level 2 / end-game track**
  with 10 uniformly-cut lines. `PlayCDTrackSegmentForSecretLevel_86F20` only ever addresses
  rows 25 and 26 (`(a1!=0)+25`, §4), so row 27 is reached ONLY via the objective-box path
  `PlayCDTrackSegmentNumber_86EB0(v8, v9, …)` when `levelnumber_43w == 27` — INFERRED it is the
  final secret/ending level's per-objective track.
- Rows **25, 26** (0x1A, 0x1B) have a single seg-0 clip {0,0x177} — the two "secret level"
  intro one-liners addressed by `(a1!=0)+25`.
- The **segment-9 (10th) slot** is used as the **level-completion line** on most rows
  (it is populated on nearly every campaign row), distinct from segments 0-8.
- Segment slots that are `{0,0}` are **empty** → `length==0` → the play function no-ops
  (`if (trackId && length)`, EF:48003). A level with N objectives fills slots 0..N plus slot 9.

### 1a. int16-vs-int32 provenance note (IMPORTANT for the importer)

The **modernized header** declares the segment fields as `int32_t` (`Type_DB080_CdTrack.h:6-7`),
and `#pragma pack(1)` makes `Type_DB080_CdTrack` = 1 + 10×8 = 81 bytes. **But the ORIGINAL
IDA layout packed them as `int16`** — struct comment `//lenght 4` (the whole SEGMENT is 4
bytes = 2×int16), and `//lenght 42` (the whole ROW = 1 + 10×4 = 41, padded/commented 42).
The dead original indexing code proves it: `EF:47996-48000` computes `42 * a1` for the row
stride and reads `*(int16*)` for startPos/length; `PlayCDTrackSegmentForSecretLevel` dead code
`EF:48021-48024` uses `21 * ((a1!=0)+25)` (21 = 42/2 words) and `int16_t`. **The authored
on-disk table is `int8 TrackIdx + int16[2]×10` = 41 bytes/row.** All the values above fit in
int16 (max 0x5C70 = 23664), so **the numbers are identical either way** — but when porting the
raw table bytes, treat each cell as **int16 little-endian**, row stride 41 (not 81).

### 1b. The unit-conversion law (VERIFIED)

`PlayCDTrackSegmentNumber_86EB0` (EF:48001-48002):
```c
startPos_v6 = CdTracks_DB080[trackIdx].TrackSegments_DB080[segmentIdx].startPos_0 * 13.33333333333;
length_v7   = CdTracks_DB080[trackIdx].TrackSegments_DB080[segmentIdx].length_2   * 13.33333333333;
```
- **13.33333 = 400/30 = 1000/75.** The stored unit is **CD frames at 75 frames/second**;
  `frames × (1000/75)` → **milliseconds**. (A CD "frame"/sector = 1/75 s.) So e.g.
  row 0 seg 0 = {0, 0x2EE=750 frames} → 0 ms .. 750×13.333 = **10000 ms = 10.0 s** clip.
- The result is stored into `int32` (truncated), so sub-ms rounding is discarded.
- **Secret-level variant does NOT convert** (EF:48027-48028): it passes the raw frame values
  straight through as if ms. That is a decompile quirk of the dead path; the LIVE secret path
  (§4) calls `PlayCDTrackSegmentForSecretLevel_86F20` which reads
  `TrackSegments_DB080[0]` **without** the ×13.333 (EF:48027-48028) then calls
  `…WithPaletteFlash_86F70(trackIdx_v2, startPos, length)`. **OPEN/CAUTION:** for the two
  secret rows (25,26) the only segment is `{0, 0x177}` and startPos is 0, so the missing
  conversion only mis-scales the *length* (0x177=375 frames → should be 5000 ms; unconverted =
  375 ms). This looks like a **latent bug in the decompile's secret path** — for the port,
  apply ×13.33333 uniformly to keep clips their true length. FLAG as OPEN for a playtest check.

### 1c. `TrackOffsets_180084` and per-track rips (VERIFIED — no offset needed for our rip)

`TrackOffsets_180084[track]` is filled at CD-init `QueryCdTrack_86370` scan (EF:47921-47965):
- For each physical track, `sub_85F60(x_DWORD_180486)` converts the drive-TOC **MSF address**
  to absolute CD **frames**: `sub_85F60(a1) = 75*minutes + 4500*... + frames` — actually
  `75*BYTE1 + 4500*BYTE2 + BYTE0` (sub_main_old.cpp:539-542), i.e. `75*sec + 4500*min + frame`
  = absolute frame position of that track's start on the physical disc.
- **Track-1 lead-in is subtracted from all** (`TrackOffsets[i] -= TrackOffsets[1]`,
  EF:47940-47948) so offsets are relative to track 1.
- At play, the **legacy MSCDEX path** adds it: `sub_86780(…, TrackOffsets[track] + startPosSec, …)`
  (EF:48067) — because a real drive addresses the whole disc, not a track.

**BUT the reimplemented backend does NOT use TrackOffsets at all.** `PlayCdTrackSegment`
(portability/port_sdl_sound.cpp:871-896) opens a **per-track WAV file** and seeks by the
per-track startPos only:
```c
double startPosSec = startPosMs / 1000;
m_ptrSpeechBytesOffSet = (44100 * startPosSec * 16 * 2) / 8;   // byte offset, 16-bit stereo 44.1k
sprintf(speechPath, "%s/TRACK%02d.WAV", speechFolder, trackIdx);   // TRACK01.WAV .. TRACK28.WAV
chunk = Mix_LoadWAV(speechPath);
chunk->abuf += m_ptrSpeechBytesOffSet;                          // seek into the file
chunk->alen -= m_ptrSpeechBytesOffSet;
Mix_PlayChannelTimed(cdChannel, chunk, 0, lengthMs);           // play for lengthMs
```
Note it passes `startPosSec` (the raw table startPos, NOT +TrackOffsets) at EF:48068
(`PlayCdTrackSegment(trackIdx, startPosSec, lengthMs)`), while only the dead legacy
`sub_86780` gets the `+TrackOffsets` sum. **→ PORT RULE for our FLAC-per-track rip: ignore
`TrackOffsets_180084` entirely.** Each ripped track starts at 0; seek `startPos_frames ×
13.33333 ms` into the file and play `length_frames × 13.33333 ms`. This is exactly what
remc2's own SDL port does. VERIFIED.

---

## 2. Every CD-audio function + call site (VERIFIED)

| function | file:line | role |
|---|---|---|
| `PlayCDTrackSegmentNumber_86EB0(trackIdx, segIdx, paletteFlash)` | EF:47987 | table lookup + ms convert → dispatch |
| `PlayCDTrackSegmentForSecretLevel_86F20(a1)` | EF:48014 | secret-level: row `(a1!=0)+25`, seg 0, palette-flash always |
| `PlayCDTrackSegmentWithPaletteFlash_86F70(track, startMs, lenMs)` | EF:48039 | registers a 50Hz palette-fade timer around the play |
| `PlayCDTrackSegment_86FF0(track, startMs, lenMs)` | EF:48056 | gate + `StopCdPlayback` + backend `PlayCdTrackSegment` |
| `PlayCdTrackSegment(track, startMs, lenMs)` (backend) | port_sdl_sound.cpp:871 | opens `TRACK%02d.WAV`, seeks, plays timed |
| `StopCdPlayback_86860(a1)` | Sound.cpp:1019 | halt current speech clip (`EndPlayingCdTrackSegment`) |
| `GetCdTrackStatus_86180(a1)` | Sound.cpp:1186 | 1 = playing, 256 = idle (`IsCdTrackPlaying`) |
| `IsCdTrackPlaying()` (backend) | port_sdl_sound.cpp:898 | `Mix_Playing(cdChannel)==1` |
| `StopCdPlayBackAndFadeUp_59AF0()` | EF:41088 | stop clip + fade music/sfx back up |
| `FadeDownSoundVolume_59A50()` | EF:41069 | duck music+sfx to 1/3 while speech plays |
| `QueryCdTrack_86370` / `QueryCdTracks_86270` | Sound.cpp:1231 / 1052 | TOC scan at init, sets `cdSpeechEnabled_E2A28` |
| `AreCdTracksAvailable` / `GetCdTrackCount` (backend) | port_sdl_sound.cpp:929/934 | probe `TRACK%02d.WAV` files present |

### The THREE live play call sites (VERIFIED — grep exhaustive):

**(A) Map screen — segment 0 intro.** `PresentLevelDescription_80C30`, M&I:3595-3600:
```c
if (DisplayLevelDescriptionText_17DE34 != 3
    && OptionsSettingFlag_24 & 0x40           // SPEECH_ENABLED
    && !IsPlayingCDTrack_17E09D) {
    IsPlayingCDTrack_17E09D = 1;
    if ((int16)levelIdx_v3 != -1)
        PlayCDTrackSegmentNumber_86EB0(levelIdx_v3, 0, 0);   // M&I:3599, NO palette flash
}
```
- `levelIdx_v3` = the map-portal index of the currently-highlighted level
  (`mapScreenPortals_E17CC[i].activated_18 == 2`, M&I:3572-3578). **0-based**, 0..25.
- Text shown alongside: `langindexbuffer[23 + levelIdx_v3]` (M&I:3592).
- Gated by `!IsPlayingCDTrack_17E09D` — **won't re-fire while a description clip is still
  playing** (interference guard, §6). `IsPlayingCDTrack_17E09D` is reset to 0 at
  M&I:953/1414/3301 (leaving the map / description-toggle) and set to 1 at M&I:3293/3597.

**(B) Objective box (briefing + in-game) — the heart.** `PresentObjective_59820`,
EF:40957-41066, called each tick from the main loop (EF:31818). See §3 for the full state
machine. The play call is EF:41038:
```c
PlayCDTrackSegmentNumber_86EB0(v8, v9, 1);   // WITH palette flash
```
with `v8 = levelnumber_43w` (0-based), `v9 = ObjectiveText_1 + 1` (current objective) OR
`v9 = 9` at level-end OR the special-level `{v8=0, v9=4}` case (§3, §4).

**(C) Secret-level objective.** Same `PresentObjective_59820`, on the beacon path
(`byte_0x36E0B & 1`), EF:41012:
```c
PlayCDTrackSegmentForSecretLevel_86F20(array_0x2BDE[LevelIndex_0xc].byte_0x3E4_2BE4_12226);
```
`byte_0x3E4_2BE4_12226` is a per-player 0/1 flag (the secret-level "which secret" selector,
GU:551) → row `(0? :1)+25` = **row 25 or 26**.

**No CD-speech play calls exist in intro / victory / defeat / cutscene code** (grep for
all `PlayCDTrack*` returns only the three above). The intros use bitmap+sample sequences
(`DrawBitmapAndPlaySound_7E320`), not redbook. VERIFIED.

---

## 3. The in-game stage-trigger → voiceover chain (VERIFIED — the core)

The chain has three stages: (i) the stage engine sets a trigger byte, (ii) a delay ramp,
(iii) the actual speech + music-duck. It runs **during live gameplay** because
`PresentObjective_59820` is ticked every frame (EF:31818, right after
`sub_58F00_game_objectives` at 31817).

### 3a. The trigger: `byte_0x36E02` (VERIFIED)

`D41A0_0.byte_0x36E02` is the **objective-message trigger** (stage-engine trace §0/§4a).
Set to 1 at:
- **Level load** — `LevelInit.cpp:41` (`byte_0x36E02 = 1`, `byte_0x36E0B &= 0xFC`). This is
  the **briefing** speech: the first objective's line plays when the level fades in.
- **Objective advance / level-end** — EF:40911 (`sub_58F00_game_objectives`). Fires **when
  the CURRENT row completes OR the level ends** (`if (v23 || v14)`, EF:40899; v23 = "current
  row just completed", v14 = "no active row remains" = level end). NOT when a background row
  completes out of turn. This is the **in-game stage trigger**.
- **Beacon/switch spawn** — EF:37307 in `AddSwitch31atyp_50FF0` (also sets `byte_0x36E0B |= 1`),
  the type-31 beacon: arms the objective message AND marks it a "beacon" objective → sound 41
  and the secret-track path.

### 3b. The delay ramp — `PresentObjective_59820` when SPEECH enabled (VERIFIED)

Guarded by `paletteMod_51 >= 3` (EF:40973) = **the level fade-in has finished** (paletteMod
counts 0→1→2→3 during the load fade, EF:31878-31890; ≥3 = fully live). So speech never fires
mid-fade. Then, with `byte_0x36E02 = v3` nonzero and `OptionsSettingFlag_24 & SPEECH_ENABLED`
(EF:40984):

```
v3 == 1        → byte_0x36E02++ (=2); return;          (EF:40988-91, ramp step)
v3 in 2..6     → LABEL_32: byte_0x36E02++; return;     (EF:40993, 41001-03, ramp steps)
v3 == 7        → byte_0x36E02 = 8; paletteSubMod_180=8; goto LABEL_36
                 → PrepareEventSound(level,-1,41)      (EF:41048-41058, the sound-41 pre-cue)
v3 == 8        → THE ACTUAL SPEECH PLAY; byte_0x36E02 = 9 (EF:41007-41046)
v3 in 9..0xC7  → LABEL_32: byte_0x36E02++; return;     (EF:40997-41003, quiet tail)
v3 == 0xC8(-56)→ LABEL_38: byte_0x36E02 = 0; return;   (EF:40999,41005,40981, done latch)
```
(Branch precedence: `v3<7` → ramp; `v3>7` split into `>8` [tail/reset] vs `==8` [play];
the fall-through `v3==7` case reaches EF:41048.)

So `byte_0x36E02` walks **1 → … → 7 → 8** across ~7 ticks. **Step 7** fires
`PrepareEventSound(level,-1,41)` (LABEL_36, EF:41058) as a **pre-cue chime**; **step 8**
fires the ACTUAL CD speech (§3c) and sets `byte_0x36E02 = 9`, which on subsequent ticks keeps
incrementing (LABEL_32) until it hits 0xC8/-56, then resets to 0 (a long quiet tail so it
won't re-fire). The ramp is a **deliberate ~7-tick delay** between the objective completing
and the voice starting (lets the "objective complete" UI/chime land first). VERIFIED.

Note: the **sound-41 pre-cue at step 7 always fires** here regardless of `byte_0x36E0B`
(LABEL_36 is the unconditional-41 target), whereas the SPEECH-DISABLED path (§3d) chooses
41-vs-61 by `byte_0x36E0B & 1`. So with speech ON you get: step-7 sound 41 → step-8 speech
(+ its own 61 chime on the normal path, EF:41019).

### 3c. Step-8: the actual play (VERIFIED)

At `v3 == 8` (EF:40997-41046), with `byte_0x36E02 = 9`, `paletteSubMod_180 = 8`:

- **Beacon path** `byte_0x36E0B & 1` (EF:41009-41017):
  `PlayCDTrackSegmentForSecretLevel_86F20(secretIdx)` (row 25/26 seg 0) + `FadeDownSoundVolume_59A50()`.
- **Normal path** (EF:41018-41046):
  - If `ObjectiveText_1 != 0`: `PrepareEventSound_6E450(level, -1, 61)` — sound **61
    (Success2)**, the objective-advance chime (EF:41019). *(Suppressed on objective 0 — the
    briefing.)*
  - **Special levels 30..34** (`levelnumber_43w in 30..34`, EF:41020-41029): if NOT level-end
    → `{v8=0, v9=4}` (track row 0, segment 4); if level-end → `v8=10, v9=9` (row 10 seg 9).
  - **Normal levels** (EF:41031-41044): `v8 = levelnumber_43w`; if NOT level-end →
    `v9 = ObjectiveText_1 + 1`; else → `v9 = 9`.
  - Then `LABEL_30` (EF:41036-41042): `paletteSubMod_180 = 8`;
    `PlayCDTrackSegmentNumber_86EB0(v8, v9, 1)` **(palette flash ON)**;
    `FadeDownSoundVolume_59A50()` (duck music+sfx);
    if `autoShowObjectivesForForeignLanguages && lang != 2` → show text box 200 ticks.

**Segment map (VERIFIED):**
- **segment 0** = level intro line (map screen only, path A).
- **segment `ObjectiveText_1 + 1`** = the current objective's spoken line (so objective row 0
  → segment 1, row 1 → segment 2, …). ObjectiveText_1 is the stage cursor (stage-engine §0).
- **segment 9** = the level-COMPLETE line (fires when `IsLevelEnd_0` is set).
- **special levels 30-34**: segment 4 for the in-progress line, segment 9 for completion,
  addressed against **track row 0** (not the level number) — INFERRED these share row-0's
  clips as generic "special level" narration.
- **level-FAILED**: no dedicated segment. There is **no CD-speech call on defeat** anywhere
  (grep exhaustive). VERIFIED — defeat handling is silent w.r.t. redbook.

### 3d. When SPEECH is disabled — the chime-only path (VERIFIED)

EF:41052-41063 (the `else` of `SPEECH_ENABLED`): no ramp, immediate:
```c
byte_0x36E02 = 0;
byte_counter_current_objective_box_0x36E04 = 200;      // show text box 200 ticks
if (byte_0x36E0B & 1)  PrepareEventSound_6E450(level, -1, 41);   // 41 = Switch (beacon)
else if (ObjectiveText_1 != 0) PrepareEventSound_6E450(level, -1, 61);   // 61 = Success2
```
So even without speech, an objective advance shows the text box + plays the advance chime
(**61 Success2**, or **41 Switch** for beacon objectives), suppressed on objective 0.
(This is the PORT CORRECTION already noted in the stage-engine trace §4b.)

### 3e. Does it fire on trigger or on opening the box? (VERIFIED)

**On trigger (automatically), not on the player opening a box.** The whole ramp runs off
`byte_0x36E02` which is set by the stage engine / level load / beacon — the player does not
open the objective box to get speech; it plays ~7 ticks after the objective state changes.
The 200-tick `byte_...0x36E04` counter is only the on-screen TEXT box's dwell time, and only
when speech is off or foreign-language auto-show is on. The map-screen line (path A) DOES fire
on hover (highlighting a portal). VERIFIED.

---

## 4. Track ↔ level mapping (VERIFIED)

- **Table is 0-based**; `TrackIdx_0` field is the 1-based physical CD track (row i →
  `TrackIdx_0 = i+1`, uniformly). The BACKEND opens `TRACK%02d.WAV` by `TrackIdx_0`
  (1..28), so **physical file = row index + 1**.
- **Campaign levels = 26 portals** (`mapScreenPortals_E17CC[26]`,
  `Type_MapScreenPortals_E17CC.cpp:3`), map-portal index 0..25 → table rows 0..25.
- **`levelnumber_43w` is 0-based** (default 0 — M&I:1386, EF:39271 from
  `x_BYTE_355210_level` which defaults 0; set to portal index `i` at M&I:1542/3346,
  EF:31383). Used directly as `trackIdx` at EF:41032. So **row = level number, 0-based**.
- **Secret levels** = `PlayCDTrackSegmentForSecretLevel_86F20`, row `(a1 != 0) + 25`
  (EF:48025), where `a1 = byte_0x3E4_2BE4_12226` (0 or 1) → **row 25 or 26**. The two secret
  intro tracks. (`secretMapScreenPortals_E2970[6]` — 6 secret portals but only two secret CD
  rows; the secret portals reuse levelNumber for the objective-box path.)
- **Row 27 (0x1C)** — the 10-uniform-clip high-offset track — is reached only via the
  objective-box path with `levelnumber_43w == 27` (INFERRED: the final/ending secret level).
- **Special levels 30-34** — do NOT index by level number for speech; they hardcode
  **row 0** (segment 4 in-progress / segment 9 complete via `v8=10`), EF:41020-41029. So the
  CD table is only 28 rows even though level numbers reach 34.

Summary formula:
```
map-screen intro:   row = portalIdx (0..25),  segment = 0
in-game objective:  row = levelnumber_43w,    segment = ObjectiveText_1 + 1
level complete:     row = levelnumber_43w,    segment = 9
special lvl 30-34:  row = 0 (or 10 at end),   segment = 4 (or 9)
secret level:       row = 25 + (secretFlag),  segment = 0
```

---

## 5. Speech-enabled option gate (VERIFIED)

Two distinct flags:
- **`cdSpeechEnabled_E2A28`** (Sound.cpp:20) = **hardware capability**: set to 1 only when the
  CD TOC scan succeeds and at least one track is readable (EF:47965;
  backend `AreCdTracksAvailable` → `GetCdTrackCount>0`). If no CD/tracks, it stays 0 and every
  low-level play/stop bails (EF:48041, 48058; Sound.cpp:1024,1116). Cleared if sound+music both
  off (EF:43042).
- **`OptionsSettingFlag_24 & SPEECH_ENABLED` (0x40)** (`global_types.h:169`) = **user
  preference**, toggled in-game (PlayerInput.cpp:1221-1224, "Speech On/Off"), initialized on
  at startup if `!nocd && cdSpeechEnabled` (EF:39257). `SPEECH_DISABLED = 0xBF` (mask to
  clear). The objective ramp branches on this (EF:40984); the map path checks
  `OptionsSettingFlag_24 & 0x40` (M&I:3595).
- **`byte_0x36E0B & 1`** = "this objective is a beacon/switch objective" (set by the type-31
  beacon, EF:37308) → uses secret-track path + sound 41 instead of 61.

**Port gate:** play speech only when (our-CD-tracks-present) AND (user speech option on).

---

## 6. Behavioral details to reproduce (VERIFIED unless flagged)

### 6a. Palette flash (VERIFIED)
`PlayCDTrackSegmentWithPaletteFlash_86F70` (EF:48039-48048) wraps the play in an AIL timer:
```c
TimerIdx = AilRegisterTimer(FadePalettes_86EA0);   // FadePalettes → PaletteChanges_47760
AilSetTimerFrequency(TimerIdx, 50);                // 50 Hz
AilStartTimer(TimerIdx);
PlayCDTrackSegment_86FF0(...);                      // blocks? no — starts the clip
AilReleaseTimer(TimerIdx);
```
`FadePalettes_86EA0` (EF:47980) just calls `PaletteChanges_47760()` — the level's palette
fade/pulse routine — at 50 Hz for the duration of the setup. Combined with
`paletteSubMod_180 = 8` (set just before every objective-box play, EF:41011/41037/41049),
this drives a **screen palette flash/pulse synced to the objective-voiceover start**. The map
path (A) calls WITHOUT palette flash (paletteFlash=0). The objective path (B/C) always uses
flash=1. VERIFIED. (Exact visual = the `paletteSubMod_180 == 8` case in the palette state
machine, EF:31937+ — INFERRED a brief white/flash pulse; not fully traced.)

### 6b. Interference / interruption rules (VERIFIED)
- **Map screen (path A):** gated by `!IsPlayingCDTrack_17E09D` (M&I:3595) — a NEW hover is
  **skipped** while a description line is still playing (no interrupt).
- **Objective box (paths B/C):** `PlayCDTrackSegment_86FF0` (EF:48061) calls
  `StopCdPlayback_86860` FIRST, so a new objective line **interrupts** whatever is playing.
  Also `GetCdTrackStatus_86180 == 256` (idle) is what triggers the fade-up (EF:40971) — the
  next line simply halts the previous.
- `GetCdTrackStatus`: **1 = playing, 256 = not playing** (Sound.cpp:1226-1228).

### 6c. Music/SFX ducking during speech (VERIFIED — YES it ducks)
- On play (paths B/C): `FadeDownSoundVolume_59A50()` (EF:41069) sets **sfx and music volume
  to 1/3** (`soundVolume/3`, `musicVolume/3`) and sets the latch `setting_38545 |= 0x40`.
- Each tick, `PresentObjective_59820` head (EF:40971-40972): if `setting_38545 & 0x40`
  (ducked) AND `GetCdTrackStatus == 256` (speech finished) →
  `StopCdPlayBackAndFadeUp_59AF0()` (EF:41088): halt clip + register a 120 Hz
  `FadeUpSoundVolume` timer that ramps volumes back, and clear the latch (`&= 0xBF`).
- So: **music+sfx duck to 1/3 while a voiceover plays, then fade back up when it ends.**
  The map path (A) does NOT duck. VERIFIED.

### 6d. Backend playback specifics (VERIFIED — for the port)
`PlayCdTrackSegment` (port_sdl_sound.cpp:871): opens `TRACK%02d.WAV`, computes byte offset
`(44100 * startPosSec * 16 * 2)/8` assuming **16-bit stereo 44.1 kHz** PCM, seeks by adjusting
`chunk->abuf`/`alen`, plays `Mix_PlayChannelTimed(cdChannel, chunk, loops=0, ms=lengthMs)` —
so `lengthMs` is a HARD stop time (SDL_mixer timed play), a single non-looping clip on a
dedicated channel (`maxSimultaniousSounds`). Our rips are FLAC per-track — same law: seek
`startFrames×13.333 ms`, play for `lengthFrames×13.333 ms`, one-shot, on a dedicated channel.

---

## 7. Port recommendation (delta vs mc2-music-law.md §4B)

Confirmed additions for the voiceover feature:
1. **Ignore `TrackOffsets_180084`** — per-track rips start at 0 (§1c). Bake tracks
   `TRACK01..TRACK28` (or FLAC equivalents) = table rows 0..27, physical index = row+1.
2. **Port `CdTracks_DB080` verbatim** (§1) as int16 pairs, ×13.33333 → ms at bake or play.
   Apply the conversion **uniformly** including the two secret rows (fix the decompile's
   unconverted secret-path length, §1b — FLAG for playtest).
3. **Trigger law** = drive segment playback off our objective/stage engine
   (`objective_mc2` / `Mc2Stage`): on level load play `(level, segment 1)`; on current-row
   advance play `(level, ObjectiveText+1)`; on level complete play `(level, 9)`; special
   levels 30-34 use row 0 seg 4 / row 0(→"10") seg 9; secret levels row 25+flag seg 0; map
   hover plays `(portal, 0)`. Add the **~7-tick delay ramp** and the **advance chime**
   (61 Success2 / 41 beacon) as separate one-shots.
4. **Duck** music+sfx to 1/3 during a voiceover, fade back up on completion (§6c).
5. **Gate** behind (tracks-present) AND (speech option on); **interrupt** on objective lines,
   **skip** on map-hover lines while one is playing.

---

## POST-TRACE CORRECTION (2026-07-12, duration-fit proof): `TrackIdx_0` counts AUDIO tracks, not physical tracks

The GOG image has 27 audio tracks at cue positions 2..28 (track 1 = data), yet the table
holds 28 rows with TrackIdx 1..28. Fitting every row's minimum required duration
(`max(startPos+length)/75 s`) against the actual cue track durations settles it:

- **row r → cue/rip track r+2** fits **27/27** rows (row 27's implied track 29 does not
  exist — its 0x5BD0+ offsets are DEAD DATA, consistent with its anomalous shape);
- row r → cue track r+1 (TrackIdx read as physical) violates 11 rows.

So `TrackIdx_0` is a 1-based index over the AUDIO tracks (the original pressing evidently
also had its data track outside that numbering), and remc2's `TRACK01.WAV` = the FIRST
AUDIO track = our `track-02` rip. **PORT RULE: table row r slices rip track r+2**
(`track-{r+2:02}.flac`); row 27 is skipped. This also retires OPEN item 2 below (row 27
is unreachable dead data — `levelnumber == 27` has no CD track behind it even on retail).

## OPEN items

1. **Secret-path unit conversion (§1b).** The live `PlayCDTrackSegmentForSecretLevel_86F20`
   passes raw frame values (no ×13.333) — a probable decompile/original bug that would make
   secret intro clips 13× too short. Confirm against retail secret-level playback; the port
   should convert uniformly. OPEN.
2. **Row 27 (0x1C) usage.** Reached only via `levelnumber_43w == 27` in the objective path;
   INFERRED to be the final/ending secret level's 10-clip track. Which in-game level maps to
   27 is not pinned here. OPEN.
3. **Special-level (30-34) row-0 sharing.** They address CD row 0 (segment 4 / via `v8=10`
   segment 9). Whether physical track 1 truly holds generic "special level" narration, or this
   is dead/placeholder, is INFERRED. OPEN — verify if any special level ships a voiceover.
4. **`paletteSubMod_180 == 8` visual.** The exact palette-flash appearance (color/duration)
   is not fully traced (state machine EF:31937+). INFERRED a brief pulse synced to speech
   onset. Trace if the visual flash is ported.
5. **`v8 = 10` at special-level end (EF:41028).** Row 10 (0x0A, TrackIdx 0x0B) is a normal
   campaign row; special-level-END reusing it looks suspicious (possibly a decompile artifact
   / off-by pattern). OPEN — low priority (special-level speech is edge-case).
