# MC2 MUSIC.DAT ground truth + XMI→SMF conversion law + danger-music verdict

> **⚠️ CORRECTION 2026-07-14 (decompile-verified, player-confirmed by ear): GAMEPLAY = GM driver BANK 0 (the "C2"/Magic Carpet 2 set), NOT bank 1.** This trace's §(A) and Port-recommendation originally said "bank 1 (C1 set)" — that was WRONG and shipped MC1's music as MC2 gameplay music (unfamiliar tracks; the aggressive 182bpm cave). The default gameplay bank is `musicChannel_E3814 = 0` (Sound.cpp:49, never reassigned) → `InitMusic_8D970`→`InitMusicBank(0)` (Sound.cpp:801). `InitMusicBank(1)` (EF:43023) runs ONLY under the hidden `-music2` command-line flag (`x_BYTE_355238_music2`, EF:39191; sets `setting_byte4_25 & 0x40`, default clear) — bank 1 = the hidden `-music2` alternate set. (The "C1 = Magic Carpet 1 music" reading of the prefix is UNCONFIRMED — the player did not recognize the bank-1 tracks as MC1 either; provenance OPEN. Verified fact = default is bank 0, `-music2` swaps to bank 1.) Empirical: bank-0 C2GAME3 (cave) = 80bpm/3 notes-per-sec (quiet/sparse) vs bank-1 C1GAME3 = 182bpm/26 notes-per-sec (the wrong aggressive track). Fix landed BAKE_EPOCH 11 (`parse_gm_bank(&music_dat, 0)`). Everything else below (XMI→SMF rules, ±1 skew, danger ramp) stands — only the bank index changes 1→0. The ±1 skew is inside AIL: `SOUND_start_sequence(track-1)` (Sound.cpp:4974).

Companion to `docs/traces/mc2-music-law.md` (which established the selection law:
MapType→track, redbook = speech). This trace nails the **container bytes**, the **XMI→SMF
conversion rules**, and settles the **peace/danger ("war") music** question.

Citations: `reference/remc2/remc2/<file>:<line>`. `EF` = `engine/EventsFunctions.cpp`.
Ground-truth byte dumps produced by `crates/mgc-import/examples/mc2_music_probe.rs`,
`mc2_evnt_probe.rs`, `mc2_loop_probe.rs` against the pristine GOG MC2 `SOUND/MUSIC.DAT`
(read out of `game.gog` via the importer). Status tags: **VERIFIED** (bytes or decompile
prove it), **INFERRED**, **OPEN**.

---

## (A) MUSIC.DAT ground truth — VERIFIED against the real file

File: `SOUND/MUSIC.DAT`, **643 564 bytes**, read from the CD image (`game.gog`) — it is NOT in
the NETHERW hard-disk overlay (that dir holds only the AIL `.MDI`/`.DIG` drivers). Read it via
`GameSource::read("SOUND/MUSIC.DAT")`.

### Container law (per `InitMusicBank_8EAD0`, `Sound.cpp:5499`) — CORRECTED/EXPANDED

```
seek(EOF-4); read u32 datapos            # = 643424 (0x9d160) in the real file
seek(datapos); read int16 driverarray[4] # per-driver COUNT (here: [2,2,2,2])
```

**`driverarray[i]` is a SONG-BANK count, not a track count.** Real value = **{G:2, R:2, F:2, W:2}** —
i.e. each of the 4 hardware drivers has **2 banks** (the code calls the bank a "channel").

Each bank is described by a **64-byte directory record** = **4 × `type_v8` (16 B each)**, one slot
per driver. `InitMusicBank_8EAD0(channel)` seeks `datapos + 8 + (channel)*64`, then reads
`type_v8 headerx[4]` (64 B) and picks `headerx[finaldrivernumber]` (`Sound.cpp:5546,5579`).
`type_v8` (`port_sdl_sound.h:90-96`):

| field | meaning (VERIFIED) |
|-------|--------------------|
| `dword_0`    | file offset of the **header block** (224 B, maybe RNC) |
| `dword_4`    | file offset of the **XMI data blob** |
| `sizeBytes_8`| header-block size (used as `size/32` = track count → 6) |
| `dword_12`   | XMI-data-blob size |

Note the per-record 16-byte slots are **interleaved by driver within one 64-byte record**, so the
record table starts at `datapos+8` and slot address = `datapos + 8 + channel*64 + driver*16`.

### The header block (on-disk serialization) — VERIFIED

`LoadMusicTrack` (`Sound.cpp:5573`) reads the header at `dword_0`, RNC-decompresses if it begins
`"RNC"` (`Sound.cpp:5634`), into `sub1type_E3808_music_header` (216 B after an 8-byte prefix;
`port_sdl_sound.h:100-127`). On disk each of the **6 track slots is 32 bytes**, laid out as the
`shadow_sub2type_E3808_music_header` (`port_sdl_sound.h:143-150`) — **filename FIRST**:

```
offset  size  field
  +0     18   filename_14[18]   (the in-memory struct lists filename last; on disk it is FIRST —
                                 the "shadow" struct is the real serialized order)
 +18      4   xmiData_0         (stored as a file/relative OFFSET; fixed to a pointer at load,
                                 GetMusicSequenceCount Sound.cpp:5567)
 +22      4   stub_4[4]
 +24      4   xmiSize_8
 +28      2   word_12           (= 90 for the 6 real subsongs; a per-subsong hint, ~BPM)
```

My probe reads the in-memory order (`xmiData,stub,size,word12,filename`) and still recovers the
filenames correctly because the 224-B blocks are laid out as `{8-B prefix}{stub[10]}{6×32-B}`.
The 6 slot filenames are the ground truth (below). `word_12 = 90` on all 6 real subsongs;
slot[0] shows `word_12=0` because slot[0]'s size field mirrors the whole-blob length.

### The XMI blob = ONE IFF container holding 6 sub-songs — VERIFIED

`dword_4` points at a single `FORM XDIR` … `CAT XMID` container with **6 `FORM XMID` children**
(6 EVNT chunks). The header's 6 filenames name those 6 sub-songs, in order. **Not RNC-compressed
in this retail file** (probe reported 0 RNC blocks; blobs begin `FORM` directly). Each blob's first
bytes: `46 4F 52 4D 00 00 00 0E 58 44 49 52 49 4E 46 4F` = `FORM....XDIRINFO`, then `CAT ....XMID`,
then per-song `FORM XMID / TIMB … / EVNT …`. Probe confirmed `FORM@0 XDIR@8 INFO@12 CAT @22
XMID@30 TIMB@46 EVNT@~78`, **#FORM=12, #EVNT=6** per blob (12 FORMs = 1 XDIR + 1 per-song 'FORM
XMID' ×6 … plus the outer; EVNT count 6 is the reliable subsong count).

### Full section/track table (real bytes) — VERIFIED

`datapos=643424`. Record table at `643432`. Driver order in the record: **0=G, 1=R, 2=F, 3=W**
(`Sound.cpp:5526-5541`: g/G→0, r/R→1, f/F→2, w/W→3).

| driver | bank | hdr off | xmidata off | xmidata size | 6 sub-song filenames (=EVNT order 0..5) |
|--------|------|---------|-------------|--------------|------------------------------------------|
| **G (GM/MPU)** | 0 | 229616 | 132384 | 97232 | C2GAME1 C2GAME2 C2GAME3 C2SETUP C2INTRO C2CUTS `.GEN` |
| **G (GM/MPU)** | 1 | 285792 | 229840 | 55952 | C1GAME1 C1GAME2 C1GAME3 C2SETUP C2INTRO C2CUTS `.GEN` |
| R (MT-32)      | 0 | 548096 | 453216 | 94880 | C2GAME1..3 C2SETUP C2INTRO C2CUTS `.ROL` |
| R (MT-32)      | 1 | …      | …       | …            | C1GAME1..3 + shared `.ROL` |
| F (FM/AdLib)   | 0 | 80656  | 0       | 80656 | C2GAME1..3 C2SETUP C2INTRO C2CUTS `.XMI` |
| F (FM/AdLib)   | 1 | 132160 | 80880   | 51280 | C1GAME1..3 + shared `.XMI` |
| W (AWE32)      | 0 | 390032 | 286016 | 104016 | C2GAME1..3 C2SETUP C2INTRO C2CUTS `.WTB` |
| W (AWE32)      | 1 | 452992 | 390256 | 62736 | C1GAME1..3 + shared `.WTB` |

The `.GEN/.ROL/.XMI/.WTB` extension = the patch-set label per driver (General-MIDI, Roland,
adlib-FM, aWe32-waveTableBank). **All four are XMI containers** — the W blobs also begin
`FORM XDIR/CAT XMID` (probe confirmed). So the G (GM) section is the right pick for our
fluidsynth/GM bake, index 0, bank chosen at runtime (below).

### Which of the 6 sub-songs plays, and which bank — VERIFIED (mechanism) / role-labels INFERRED

The 6 sub-songs are **GAME1, GAME2, GAME3, SETUP, INTRO, CUTS** (0..5).
- **Bank selection**: `InitMusicBank_8EAD0(channel)` with `channel = musicChannel_E3814` (default 0)
  or an explicit arg. The **gameplay** bank is loaded by `InitMusicBank_8EAD0(1)` at boot
  (`EF:43023`, guarded by `setting_byte4_25 & 0x40`) → **bank 1 = the "C1" set**
  (C1GAME1/2/3 + shared SETUP/INTRO/CUTS). Bank 0 ("C2" set) is loaded on demand by the
  animation/cutscene sound-script (`Animation.cpp:106`, key `'B'`, explicit index).
- **Sub-song selection during gameplay**: `StartMusic_8E160(track,0x7F)` with
  `track = D41A0_0.maptypeMusic_0x235 ∈ {Night:1, Day:2, Cave:3}` (`EF:31441-31449`, per the
  companion trace). `StartMusic` indexes `track_10[track]` (`Sound.cpp:889`) and the load loop
  arms sequences `1..m_iNumberOfTracks` from `track_10[i-1]` (`Sound.cpp:5676-5677`) — note the
  ±1 skew between load (0-based `i-1`) and play (`track_10[track]`). Net: MapType 1/2/3 select
  **GAMEn**; **track 4 = SETUP = the menu song** (`MenusAndIntros.cpp:832,874` call
  `StartMusic_8E160(4,…)`). INTRO/CUTS (slots 4/5) are for the intro/cutscene animations.
- **Role labels** GAME1=Night, GAME2=Day, GAME3=Cave are INFERRED from the companion trace's
  MapType map + slot order; confirm empirically at bake time (the three GAMEn tempos differ:
  bank-1 GAME1=120bpm, GAME2=128bpm, GAME3=182bpm).

**m_iNumberOfTracks** = `sizeBytes_8 / sizeof(sub2type)=32` = **6** (`Sound.cpp:5611`). The music
switch is gated on `musicAble && musicActive && m_iNumberOfTracks` (`EF:31438`).

---

## (B) XMI → SMF conversion law

Two independent ground-truth references in the decompile (NOT opaque AIL blobs):
1. `engine/XmiInfo.cpp` — a clean hand-written loop-scanner + container walker.
2. `engine/AIL_stub.cpp` — the decompiled Miles/AIL runtime interpreter (`sub_A6530` etc.); only
   the sound-card I/O is stubbed, the **parse/timing/loop logic is real C**.
3. `portability/xmi2mid.cpp` — Corsix's `TranscodeXmiToMid`, the **actual working converter remc2
   ships**. Use this as the byte-exact algorithmic reference.

### Container walk — VERIFIED (`XmiInfo.cpp:30-90`, `AIL_stub.cpp:4790-4823`)
- Big-endian IFF sizes; word-align every chunk: `next = p + 8 + size + (size&1)`.
- Accept `FORM XMID` (single song), `CAT XMID` (multi), or `FORM XDIR`(+`INFO`) **followed in the
  file** by `CAT XMID` — compute `cat = p + 8 + form_size + (form_size&1)` (`XmiInfo.cpp:63-70`).
  MC2's blobs are the `FORM XDIR … CAT XMID` shape with 6 `FORM XMID` children.
- Song *n* = the *n*-th `FORM XMID` inside the `CAT` (linear index, no name lookup;
  `AIL_stub.cpp:4805-4816`). Inside each `FORM XMID`: `TIMB`, (`RBRN`), `EVNT`.
- **Corsix shortcut**: `TranscodeXmiToMid` just `scanTo("EVNT")` + `skip(8)` and converts ONE song
  (`xmi2mid.cpp:267`). For a 6-song container you must locate each EVNT yourself (walk the CAT) and
  convert each subsong separately — my `mc2_loop_probe.rs` already walks successive EVNTs.

### The 4 trickiest rules

**1. XMI delta-time = summed run of `<0x80` bytes — NOT the MIDI VLQ continuation scheme.**
`XmiInfo.cpp:19-23`: `while(!(*p & 0x80)) d += *p++;`. A delay is a run of bytes each ≤0x7F, summed
(127+127+… for long delays). The first byte with the high bit **set** is the next event's status.
Corsix scales each such byte by 3 as it accumulates (`iTokenTime += iTokenType*3`,
`xmi2mid.cpp:288`) — that ×3 is folded into its output timebase (see rule 4). A from-scratch
converter can instead keep ticks 1:1 and choose the SMF division to match (rule 4). **This dual
decoder (summed-run delay vs continuation-VLQ elsewhere) is the #1 gotcha.** VERIFIED by hand-decode
(`mc2_evnt_probe.rs`): first GAME1 event stream shows `+1` accumulated delays and correct events.

**2. Note-On carries an embedded duration VLQ; there are NO note-offs — synthesize them.**
An XMI note-on is `9n note vel <dur:VLQ>` where `<dur>` uses the **standard** continuation-bit VLQ
(`XmiInfo.cpp:150-157`, `AIL_stub.cpp:5570`). No `8n` events exist in the stream. The converter must
push each note into a priority queue keyed by `now + dur` and emit a note-off (`8n note 0` or
`9n note 0`) when the timeline reaches that end-time — exactly what Corsix does: it `append`s a
second token at `iTokenTime + readUIntVar()*3` with an empty buffer, then `std::sort`s all tokens by
time before writing (`xmi2mid.cpp:309-319,365`). VERIFIED by hand-decode (dur=27990, dur=21 read
cleanly as VLQ).

**3. FOR-loop controllers cc116/cc117 are AIL-private loop control — strip or unroll, never emit.**
(`XmiInfo.cpp:161-199`, `AIL_stub.cpp:5187-5235`.) **cc116 = FOR start**, controller *value* =
repeat count, **value 0 = infinite loop** (sentinel). **cc117 = NEXT**: acts only when value ≥ 64
(jump back to the matching cc116 pointer), value < 64 = no-op fall-through. Nesting is a **4-deep**
stack (`FOR_NEST=4`). Every MC2 gameplay sub-song wraps its whole body in `cc116 val=0` …
`cc117 val=127` (probe: **all 12 GAMEn/SETUP/… tracks carry exactly `[(116,0),(117,127)]`**) — i.e.
**one infinite full-song loop**. Corsix's `scanTo("EVNT")` converter passes cc116/117 through as raw
`Bn 74/75` CC bytes (it does not special-case them) — that is a **bug to avoid**: a GM synth would
receive junk controllers. **Our bake law**: cut the loopable region as `loop-start(cc116) →
loop-end(cc117)` and loop the baked FLAC at runtime (mirrors our MC1 one-pass-bake + runtime-loop
pipeline). For MC2 the cc116 sits at song start and cc117 at song end, so the loopable region ≈ the
whole song — bake one pass, drop the two CC events, loop the FLAC. (If a track ever had a non-loop
intro before cc116, bake intro-then-loop; none of the 12 MC2 tracks do.)

**4. Tempo / PPQN so playback speed is correct.**
XMI's clock is **fixed at 120 Hz** (AIL preference #11 = 120; `interval_time = 1e6/120 ≈ 8333 µs`
per tick; `AIL_stub.cpp:609,5814`). Tempo (`FF 51 03`) and time-sig (`FF 58`) metas are honored and
**present inside the EVNT** (my hand-decode shows `FF 51 03 08 8e 6c` = 561260 µs/qn at song start;
the 12 GAMEn subsongs range 80–182 BPM). Two consistent output recipes:
  - **Recommended (from-scratch Rust):** SMF **division = 60 PPQN**, keep delta ticks 1:1, pass the
    `FF 51` tempo metas through verbatim. With 60 PPQN and the song's own tempo, wall-clock is
    correct because XMI ticks run at 120/sec and a quarter = 60 ticks (60 PPQN × 2 quarters/sec at
    120 BPM = 120 ticks/sec). Tempo changes still mean µs/quarter and Just Work.
  - **Corsix (shipped reference):** scales every delta & duration by 3, sets header
    division = `(iTempo*3)/25000` where `iTempo = firstTempo_µs*3` (`xmi2mid.cpp:334,360`), and only
    the FIRST tempo is emitted (later tempos are dropped, `xmi2mid.cpp:338-344`). Self-consistent but
    loses mid-song tempo changes — the 60-PPQN pass-through recipe is cleaner for us.

### TIMB / RBRN / GM patches — VERIFIED
- **TIMB** = `[u16 count][ (patch:u8, bank:u8) × count ]` (`AIL_stub.cpp:6223-6284`). Used ONLY to
  preload the synth's patch set; **the program selection that matters is the `Cn` Program-Change
  events in EVNT**. The G-section EVNT program-changes are **direct GM patch numbers** (hand-decode:
  `C1 2A`=prog 42 Cello, `C5 32`=50 Synth-Strings, `C1 4A`=74 Flute, `C1 0C`=12 Marimba) — so the
  GM bake needs no patch remap. **Ignore TIMB for the SMF.**
- **RBRN** (branch points) — recognized, pointer stored, never parsed for linear playback
  (`AIL_stub.cpp:6199`). Interactive-jump table only; **drop it for the SMF.** Record layout is OPEN.

### AIL-private controllers to STRIP (never forward to a GM synth) — VERIFIED
cc110-115 = AIL channel-lock / patch-bank / ICA control; **cc116/117 = FOR loops** (rule 3);
cc118 = beat callback; **cc119 (0x77) = XMI trigger callback** (`AIL_stub.cpp:5246-5253`,
`5000-5253`). MC2's gameplay streams emit `Bn 74 00` (cc116) and `Bn 77 00` (**cc119**) at song
start (hand-decode: the header block sends `b6/b7/b8 77 00`). The Corsix path leaks these raw; a
correct converter drops the whole 110-119 controller band — **except** it must PASS real GM
controllers 0x00-0x6D (07=volume, 0x0A=pan, 0x5B=reverb, which the streams use).

**cc119 is load-bearing for MC2's danger music (see C):** in the original engine the XMI trigger
tag fires `sub_8E0D0` (registered by StartMusic, `Sound.cpp:890`), which sets
`UpdateMusicTimer_E3819 = true` — i.e. the war/danger ramp only arms once the sequence hits its
cc119 trigger. Our FLAC bake discards MIDI events, so we implement the danger ramp in the app
directly (part C port note); but keep this in mind if we ever run live MIDI.

---

## (C) Peace/danger ("war") music — VERDICT: **MC2 HAS a danger ramp** (a controller-11 volume ramp on the ONE MapType track, NOT a track switch). VERIFIED.

> Correction to a first-pass hypothesis: I initially read this as "no danger music" because the
> `StartMusic`/`AilSetSequenceVolume`/`SetMusicVolume_98790` paths carry no combat trigger. That is
> true but incomplete — the danger ramp runs on a **separate path** (`WarMusicSetVolume` /
> controller-11 expression), which the earlier greps missed. The mechanism is real and combat-driven.

### The ramp state machine (`Sound.cpp`) — VERIFIED
- `x_BYTE_E3816` = ramp value 0..127 (`Sound.cpp:51`). **0 = peace (music ducked quiet), 127 = danger (full).**
- `x_BYTE_E3817` = state (1 = heading to peace, 2 = heading to danger) (`Sound.cpp:52`).
- `x_BYTE_E3818` = ramp-active flag; `x_BYTE_E381A` = step (±1, sign flips on state change).
- `UpdateMusicTimer_E3819` = master enable, set **true** by the XMI **trigger callback** `sub_8E0D0`
  (`Sound.cpp:850-857`), which also sends `cc11=0` on the channel; set false by `StopMusic`
  (`Sound.cpp:837`). **So the war system only arms after the running XMI emits its trigger tag
  (cc119) and StartMusic registers `sub_8E0D0` (`Sound.cpp:890`)** — see (B): cc119 is load-bearing.

**Ramp timer `FadeWarMusic_99830` (`Sound.cpp:5857`):** each tick, if active,
`x_BYTE_E3816 += x_BYTE_E381A; WarMusicSetVolume(x_BYTE_E3816)`. Self-terminates at the rails
(`==127 && state 2`, or `==0 && state 1`, releasing the timer). **In the original build this drove
`AilSendChannelVoiceMessage(... i|0xB0, 11, x_BYTE_E3816)` per active channel** (the loop at
`Sound.cpp:5880-5884`, now commented out) — i.e. MIDI **controller-11 (expression)** attenuation of
the one MapType sequence. remc2 substitutes `WarMusicSetVolume` (`port_sdl_sound.cpp:165`) which
scales a parallel `danger.ogg` mix channel instead — a remc2 enhancement (`GAME_music_war` /
`warMusicOn` / `*danger.ogg`, `port_sdl_sound.cpp:37-197`), **not** original-game data.

**Arm/disarm `UpdateMusic_99970(state, rate)` (`Sound.cpp:6076`):** acts only when state changes
(`x_BYTE_E3817 != state`); flips the step sign; registers `FadeWarMusic_99830` at timer frequency
`30*rate` Hz (rate clamped 1..4, else 30). Gated on `UpdateMusicTimer_E3819 && musicAble &&
musicActive && songCurrentlyPlaying && seq-not-done`.

### The combat trigger — VERIFIED
`UpdateMusic_99970` has exactly two callers, both in the per-frame local-player status update
`sub_5D530` (`EF:59839-59849`):
```c
if (a1x->dword_0xA4_164x->playerColorIndex_0x38_56 == D41A0_0.LevelIndex_0xc) {  // local player
    if (a1x->dword_0xA4_164x->word_0x36_54 <= 0)  UpdateMusic_99970(1, 3);       // -> PEACE
    else { a1x->dword_0xA4_164x->word_0x36_54--;  UpdateMusic_99970(2, 3); }     // -> DANGER
}
```
The countdown `word_0x36_54` (on `type_str_164`, `global_types.h:238`) is **stamped to 100** in
`sub_5EF70` (`EF:60598-60606`) whenever a **class-3, model-0 entity** (a spell/projectile
manifestation — the class-3 list per project memory) contacts the player. `sub_5EF70` is called from
~16 projectile/spell spawn+impact sites (e.g. `EF:9707/9736/9766/9841/9877/9908`, `13505/15165`,
`54851/54887/54981`, `60664/60724/62225`, `Events.cpp:2904`).

**Net law (VERIFIED):** each frame a projectile/spell touches the player, `word_0x36_54 ← 100`.
While >0 it decrements one per frame and the music ramps **toward danger** (state 2, +1/tick at
`30*3 = 90` Hz, up to 127). When it hits 0, the music ramps **back to peace** (state 1, −1/tick,
down to 0). Full traverse ≈ 127 ticks (~1.4 s at 90 Hz). One hit sustains danger ~100 frames,
refreshed by continued fire. **Rate = 90 Hz, step ±1, arm-constant = 100** — the same constant and
role as **MC1's `v_46=100`** danger law (there is no literal `v_46` symbol in the MC2 tree; the MC2
field is `word_0x36_54`).

### Track switch on combat? NO. VERIFIED negative.
`StartMusic_8E160` is NEVER called with a combat-derived argument. Its ONLY track arguments across
the whole tree are: `D41A0_0.maptypeMusic_0x235` (MapType 1/2/3), the literal **4** (menu = SETUP),
and `pSoundEvent[].index` (cutscene sound-script). Exhaustive start/stop table:

| site | call | trigger |
|------|------|---------|
| `EF:31662-64` | `StopMusic; StartMusic(maptypeMusic,0x7F)` | level start / turn 1 |
| `PlayerInput.cpp:459` | `StartMusic(maptypeMusic,0x7F)` | un-pause / enter game |
| `PlayerInput.cpp:1205/1213` | `StopMusic` / `StartMusic(maptypeMusic,0x7F)` | **M key** toggle |
| `Sound.cpp:6555` | `StartMusic(maptypeMusic,0x7F)` | resume after options |
| `MenusAndIntros.cpp:832,874` | `StartMusic(4,0x7F)` | **menu = SETUP (track 4)** |
| `Animation.cpp:110/136/142` | `StartMusic(script.index,0x64/0x7F)` | intro/cutscene sound-script (bank 0) |
| `Animation.cpp:105,193` `PlayerInput.cpp:442,1584` `MenusAndIntros.cpp:802,831,872,4145` `EF:8663,31479` `Sound.cpp:5510` | `StopMusic` | menu/pause/teardown |

`EF:8663` = `StopMusic` under `GAME_PAUSED` — pause halts music. No victory/defeat jingle is routed
through the sequencer (level-end music is stopped, not swapped; win/lose presentation = CD-audio +
FMV). Non-combat volume: `SetMusicVolume_98790(500,0)` (`EF PaletteFadeIn_480A0`, only caller) =
level-transition fade; `FadeDownSoundVolume_59A50`/`FadeUpSoundVolume_59B50` duck ALL audio around
CD-voiceover playback (`EF:41069/41103`). None combat-keyed.

### Port recommendation for the danger ramp
Faithful behavior = ONE MapType track whose **expression/volume is smoothly ducked toward full
("danger") when the local player is under projectile fire and back down ("peace") when combat
lapses**, at ±1 per 90 Hz tick over a 0..127 range, armed by a 100-frame countdown per hit.
For our FLAC-bake pipeline (no live MIDI), the clean port: keep the peace loop at reduced volume and
crossfade toward full/an intenser mix on the same 100-frame countdown — OR (simplest faithful-enough)
duck the single baked track's playback gain between a peace level and full on the countdown. Do NOT
ship a separate `danger.ogg` (that's remc2's non-canonical enhancement) unless we author one
deliberately as an opt-in enhanced mode. **OPEN**: the original controller-11 loop is commented out
in remc2, so the exact per-channel dynamics (which channels, whether it scales all or a subset) are
inferred from intent; the value range (0..127) and trigger law are VERIFIED.

---

## Port recommendation (supersedes companion trace §4A specifics)

1. Import `SOUND/MUSIC.DAT`: trailer u32 → `driverarray[4]`; pick driver **G (index 0)**, **bank 0**
   (the default `musicChannel_E3814=0` = gameplay; bank 1 = the `-music2` MC1-classic opt-in — see
   the correction banner), read its `type_v8` slot (record @ `datapos+8 + 0*64 + 0*16`),
   take the XMI blob at `dword_4`/`dword_12` (no RNC in retail, but keep the `RNC` check for safety).
2. Walk the `FORM XDIR / CAT XMID`; extract sub-songs **0,1,2 = C2GAME1/2/3** (the gameplay set).
   Convert each EVNT→SMF with the rules above (summed-run delta; note-on dur→synth note-off;
   division 60 PPQN + tempo pass-through; strip cc110-119; TIMB/RBRN ignored; loop = whole song).
3. Bake 3 gameplay FLACs (Night=GAME1, Day=GAME2, Cave=GAME3 — EAR-CONFIRMED for bank 0) + optionally
   track 3 = SETUP as the **menu** song. Loop the FLAC at runtime (cut the cc116→cc117 region).
4. Selection at runtime = level MapType (Day/Night/Cave), start once on level load, restart only on
   MapType change. **No danger/war crossfade** (part C).

## Probes (left in tree for re-runs)
- `crates/mgc-import/examples/mc2_music_probe.rs` — full container dump (sections, filenames, sizes,
  IFF chunk scan). `cargo run -p mgc-import --release --example mc2_music_probe -- "<gamedata-root>"`.
- `crates/mgc-import/examples/mc2_evnt_probe.rs` — hand-decode of the G/bank0 GAME1 EVNT stream
  (proves delta accumulation + note-on embedded-duration VLQ).
- `crates/mgc-import/examples/mc2_loop_probe.rs` — per-subsong loop-controller + tempo scan for both
  G banks (proves every track = one `[(116,0),(117,127)]` infinite loop).
