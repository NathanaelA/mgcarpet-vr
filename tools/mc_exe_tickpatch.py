#!/usr/bin/env python3
"""Detour-patch CARPET.EXE / HIDDEN.EXE to pace the sim tick loop and expose a
recorder mailbox -- and, optionally, to guard retail's stale volcano registers.

Two independent patches live here, and the difference between them matters:

  * THE PACING STUB (default) is BEHAVIOUR-NEUTRAL. It changes when the tick
    loop runs, never what it computes: "the recorded tick sequence is identical
    to retail's". Every take in the corpus is recorded through it.

  * THE VOLCANO WRITE GUARD (``--volcano-guard``, MC1/HW only) CHANGES
    SIMULATION. It is the binary half of the port's
    ``volcano_register_revalidate`` patch, suppressing two blind writes that
    retail makes through stale slot registers -- the `+26 = 250` kick that
    turns a re-minted CASTLE into a level-250 castle and HANGS the game on its
    next downgrade, and the plume soft-kill that kills whatever inherited the
    old column's slot. It exists because those two writes make some levels
    (notably the Hidden Worlds showcase maps) unwinnable or crash-prone through
    no fault of the player.

    A binary carrying the guard IS NOT THE SHIPPED BINARY. Recordings made with
    it witness the PATCHED arm and must never be graded as retail; the default
    output is named ``*_RECVG.EXE`` rather than ``*_REC.EXE`` so the two cannot
    be confused on disk.

Why
---
The DOSBox recorder (``tools/mc_dosbox_recorder.py``) needs to sample the
master world struct *between* sub-steps, while it is fully settled. Retail's
tick loop was never frame-capped: with DOSBox cycles cranked high, every
DOSBox host-park lands mid-entity-loop, so the recorder loses ticks
("saturation loss"). This tool installs a tiny wrapper stub around the
per-sub-step tick function (remc1 ``sub_41780_41AC0``) by REDIRECTING its
callers (the gameSpeed fan-out ``call``s) to the stub instead of detouring
the function entry -- the entry stays byte-for-byte pristine, so there are no
injected bytes there to be misdecoded. The stub paces, then ``call``s the
original tick fn and ``ret``s to the caller. It:

  1. paces ONE sub-step per rendered frame to a wall-clock deadline (the
     game's own ~120 Hz PIT counter): fps = 120 / period, so the default
     period 5 gives ~24 fps (the authentic Magic Carpet rate) regardless of
     how high DOSBox cycles are set. The excess cycles are burned in a
     wall-clock spin -- exactly the large *quiescent* window the recorder
     wants: the world struct is settled and untouched. Only the FIRST
     sub-step of a frame is paced, so the F3 game-speed feature (1x / 4x /
     16x sub-steps per frame) still speeds the SIM up while the frame rate
     stays put; at the default speed (1 sub-step/frame) every sub-step is the
     first, so pacing is bit-identical in effect to the old every-sub-step
     pacer.
  1b. holds that quiescent window open for a FLOOR of timer counts (``--floor``,
     default 2) even when the frame overran its deadline. Pacing to an absolute
     deadline is only as good as the compute fitting inside it: a heavy frame
     arrives with the deadline already passed, the spin falls through, and the
     window collapses to nothing -- a dropped frame and a torn delta. The floor
     is measured from where the frame settles, so its width does not depend on
     load, and it is charged only to the frames that already overran (raising
     ``--period`` instead would tax every frame). It applies to the MC2 arm too.
  2. maintains a mailbox in obj3's committed tail: a magic, a monotonic
     sub-step counter, an ``in_window`` flag raised only around a paced spin
     (never on a free-running sub-step, so the recorder never parks in a
     zero-width window), and the raw F3 gameSpeed (0/1/2) so the recorder can
     tell a legit speed-up from a capture loss. The recorder snipes on
     ``in_window==1`` keyed by the tick counter and gets one coherent
     snapshot per paced sub-step -- gap-free by construction, no +63
     heuristic.

The sim is unaffected: MC1's lockstep multiplayer proves per-tick logic is
wall-clock independent (the ~120 Hz counter feeds render/animation timing,
never sim state), so pacing changes only *when* ticks run, never *what* they
compute. The recorded tick sequence is identical to retail's.

Safety / provenance
-------------------
- Patches a COPY. gamedata/ stays pristine GOG; output is ``*_REC.EXE``.
- The stub lives in obj1's zero-filled code cave (read+exec); the mailbox in
  obj4's zero BSS tail (read+write). Neither overlaps game data.
- The ONLY bytes changed in the game's own code are the 4-byte rel32 of each
  redirected ``call`` -- the tick fn entry is untouched (an earlier version
  detoured the entry; the 10-byte overwrite decoded as a wild ``add eax,[eax]``
  when the dynamic recompiler picked the region up misaligned, so we redirect
  the call site instead).
- The stub WRITES only EAX/ECX/EDX (caller-clobber on a void, no-arg fn) and
  only READS EBX (the fan-out's live loop index, to pace just the first
  sub-step); the original tick fn saves/restores EBX/ESI/EDI/EBP, so the
  caller's callee-saved registers (its loop counter in EBX) survive unchanged.
- DOS/4GW relocates the image by one base and injected code gets no LE fixups,
  so the stub computes that load delta at runtime (call/pop) and addresses all
  globals and the original tick fn relatively (delta-invariant).
- A guard counter bounds the spin: if the timer ISR is ever masked (counter
  frozen) the stub releases after ~1 s of emulated time instead of hanging.
- The wall clock is NOT monotonic for the life of the process, but the mailbox
  IS (init is magic-gated, so the deadline survives a level exit). The game
  zeroes the clock in its fade/delay helper on the way back to the menu and
  restores an older value on quickload, so the pacer resyncs on a deadline that
  is too far AHEAD of the clock as well as too far behind -- otherwise the
  second level of a session spins out the guard every frame (~0.2 fps).

Usage
-----
    python3 tools/mc_exe_tickpatch.py CARPET.EXE          # -> CARPET_REC.EXE
    python3 tools/mc_exe_tickpatch.py HIDDEN.EXE -o HID_REC.EXE
    python3 tools/mc_exe_tickpatch.py CARPET.EXE --period 4   # ~30 fps
    python3 tools/mc_exe_tickpatch.py CARPET.EXE --verify-only CARPET_REC.EXE
"""
from __future__ import annotations

import argparse
import struct
import sys
from dataclasses import dataclass
from typing import Optional

# --------------------------------------------------------------------------
# Mailbox layout. The mailbox and the wall clock both live in obj3 (the data
# object), addressed OBJ3-RELATIVE: at runtime the stub derives obj3's real
# base (see build_stub) rather than assuming any load delta, so its writes
# always land in obj3 -- never in game memory. Offsets below are relative to
# the mailbox base; the mailbox itself sits in obj3's committed BSS tail.
# Kept in lockstep with tools/mc_dosbox_recorder.py's EXE_MB_* constants.
# --------------------------------------------------------------------------
OBJ3_BASE = 0x90000  # obj3 LINK base (vbase)
MB_OBJ3 = 0xA2C40  # obj3-relative mailbox base: past both builds' vsize
#                    (CARPET 0xa2c00 / HIDDEN 0xa2bf0), inside the committed
#                    page tail (< 0xa3000). Same offset works for both.
MB_MAGIC0 = MB_OBJ3 + 0x00  # 'MGCT'
MB_MAGIC1 = MB_OBJ3 + 0x04  # 'TIK1'
MB_TICK = MB_OBJ3 + 0x08  # u32 monotonic sub-step counter
MB_INWIN = MB_OBJ3 + 0x0C  # u32 1 while parked in the quiescent spin
MB_DEADLINE = MB_OBJ3 + 0x10  # u32 next release, in PIT counts
MB_SPEED = MB_OBJ3 + 0x14  # u32 raw F3 gameSpeed latch (0/1/2) -- lets the
#                            recorder tell a legit F3 speed-up from capture loss
MB_PERIOD = MB_OBJ3 + 0x18  # u32 sub-step period in PIT counts (default 1)
MB_PARK_ARM = MB_OBJ3 + 0x1C  # u32 nonzero = park at the next level start
MB_PARK = MB_OBJ3 + 0x20  # u32 the init-park handshake (PARK_* below)
MB_PARK_MAGIC = MB_OBJ3 + 0x24  # 'MGCP' once the process has parked once
MB_END = MB_OBJ3 + 0x28  # one past the last mailbox word
MB_GUEST = OBJ3_BASE + MB_OBJ3  # guest-LINK addr the recorder reads (0x132c40)

MAGIC0 = 0x5443474D  # "MGCT"
MAGIC1 = 0x314B4954  # "TIK1"

GUARD_ITERS = 0x04000000  # spin bail-out (~1 s emulated); never hit if ISR live

# --------------------------------------------------------------------------
# THE INIT PARK (both arms, 2026-09-27).
#
# The windows above are all POST-something: MC2's opens after the frame
# driver (so its first window is t=1, one frame into the level), MC1's opens
# before the tick fn but after the frame driver's pre-tick calls, and both
# are only `floor` counts wide on the first frame, which the recorder must
# hit while it is still pinning frames. What the remc2 V03 save, `init-check
# --settle 0` and a native-vs-retail init diff all want is the world as level
# initialisation LEFT it -- before the first frame -- captured without a race.
#
# So on the FIRST call of a level the stub parks BEFORE calling the game. The
# park hooks the FRAME DRIVER call in both games (MC2 inside the existing
# signal stub; MC1 in a small stub of its own on GameLoop's `call
# DrawAndEventsInGame_34530`, because frame 1's `sub_3C9D0` join prologue
# spawns every wizard's carpet, spell tokens and starting castle BEFORE the
# first tick call -- so MC1's tick-stub window 1 is post-spawn, not post-init):
#   PMAGIC 'MGCP', written by the park itself the first time: absent = a fresh
#         process, whose first level parks unconditionally (the park block
#         does not rely on the tick stub's magic, which MC1 writes only later);
#   ARM   may be set to 1 by the host to park again (a warm process, the next
#         level); the stub clears it as it parks, so each arming parks ONCE;
#   PARK  = 1 while parked (the world is post-init and untouched; the counter
#         and in_window are unchanged, so a legacy reader sees nothing new);
#         the host writes 2 to release; the stub leaves 3 (released by the
#         host) or 4 (timed out) behind, so the host can tell whether its
#         snapshot was taken inside the park.
# The timeout is in the game's own timer counts (--init-park, 0 = no park
# code at all), so an exe run without a recorder only pauses at level start.
# It is behaviour-neutral like the rest of the stub: a wait, no writes to
# game memory. The frozen-timer guard below only matters if the ISR is dead.
# --------------------------------------------------------------------------
PARK_IDLE, PARK_PARKED, PARK_RELEASE, PARK_RELEASED, PARK_TIMEOUT = 0, 1, 2, 3, 4
PARK_MAGIC = 0x5043474D  # "MGCP"
INIT_PARK_DEFAULT = 500  # counts: ~4.2 s on MC1's ~120 Hz, 5 s on MC2's 100 Hz
INIT_PARK_MAX = 12000  # ~2 min
PARK_GUARD_ITERS = 0xFFFFFFFF  # only if the timer ISR is dead
RESYNC_COUNTS = 30  # >250 ms behind schedule -> resync instead of catch-up burst

# --------------------------------------------------------------------------
# The capture-window FLOOR (both arms).
#
# Both arms pace to an ABSOLUTE deadline -- MC1's `deadline += period`, MC2's
# `esi = timer_before_frame + 5`. That is fine while compute fits the budget
# and useless when it does not: a frame heavy enough to overrun its period
# (deaths, meteor swarms) arrives with the deadline already passed, the spin
# falls straight through, and `in_window` is raised and cleared within a
# handful of instructions -- a zero-width window the recorder cannot land in,
# so the take loses that frame and the delta tears. MC1 makes it worse by
# design: up to RESYNC_COUNTS of backlog is burned off with NO wait at all, so
# one heavy frame is followed by a burst of free-running ones.
#
# The floor is a RELATIVE wait, measured from the moment the frame settles, so
# its width does not depend on load: whatever the deadline says, the stub holds
# the quiescent window open for at least `floor` counts. It costs time only on
# the frames that already blew their budget -- unlike raising --period/--pace,
# which taxes every frame including the cheap ones.
#
# GRANULARITY LAW. The only clock either stub can read is an INTEGER tick
# counter (MC1 ~120 Hz = 8.33 ms/count, MC2 100 Hz = 10 ms/count), so
# `target = now + 1` guarantees nothing: enter a hair before the counter ticks
# and it releases immediately. FLOOR_MIN_GUARANTEED = 2 is the smallest value
# that guarantees a FULL count of real wait (and costs at most two). Sub-count
# precision would mean latching the PIT on port 0x40 -- far more code, and
# 8-10 ms is already an order of magnitude past what the recorder needs (three
# stable 224 KB reads).
FLOOR_DEFAULT = 2
FLOOR_MAX = 60  # ~0.5 s; kept well inside GUARD_ITERS so the guard never wins

# --------------------------------------------------------------------------
# MC2 / NETHERW.EXE arm. MC2 already frame-limits itself (InGameLoop_47320's
# native ~24 fps spin), so its takes are gap-free -- but ~33% are TORN: DOSBox
# can park the guest between PlayerEvents (per-player Turn++) and the entity
# pass, a settled-looking-but-mid-frame state. So the MC2 arm is SIGNAL-ONLY:
# no pacer, no spin, no wall clock. It wraps the frame-driver call and raises
# an `in_window` flag for exactly the interval when the frame is fully settled
# (post-draw) and the NEXT frame's Turn++ has not begun -- i.e. across MC2's
# own native limiter spin. The recorder captures only while in_window==1, so
# the Turn++-park tear is unobservable by construction.
#
# The hook is InGameLoop_47320's sole `call DrawAndEventsInGame_47560`:
#     mov esi,[GameTimerTurn]      ; esi = timer before the frame
#     call DrawAndEventsInGame     ; <-- redirected to the stub
#     add esi,5                    ; frame period = 5 timer ticks
#   spin:
#     cmp esi,[GameTimerTurn]      ; native frame-limiter busy-wait
#     ja spin                      ; <-- in_window is raised across THIS spin
# The stub: derive obj3 base, clear in_window (frame about to mutate), call the
# real frame driver, then bump a monotonic frame counter and set in_window=1.
# Continuity is that counter's delta -- NOT the per-player Turn, which advances
# mid-frame (inside PlayerEvents) by design and so can't gate the tear.
MB2_OBJ3 = 0xB42C0  # obj3-relative mailbox base == obj3.vsize (page-align lifts
#                     the segment limit over it; see patch_mc2). Guest 0x1842c0.
MB2_MAGIC0 = MB2_OBJ3 + 0x00  # 'MGCT'
MB2_MAGIC1 = MB2_OBJ3 + 0x04  # 'TIK2'
MB2_TICK = MB2_OBJ3 + 0x08  # u32 monotonic per-FRAME counter (bumped once/frame)
MB2_INWIN = MB2_OBJ3 + 0x0C  # u32 1 while parked in the settled inter-frame gap
MB2_PARK_ARM = MB2_OBJ3 + 0x10  # u32 nonzero = park before the next level's frame 1
MB2_PARK = MB2_OBJ3 + 0x14  # u32 the init-park handshake (PARK_* above)
MB2_PARK_MAGIC = MB2_OBJ3 + 0x18  # 'MGCP' once the process has parked once
MB2_END = MB2_OBJ3 + 0x1C  # one past the last mailbox word
OBJ3_BASE_MC2 = 0xD0000  # obj3 LINK base
MB2_GUEST = OBJ3_BASE_MC2 + MB2_OBJ3  # 0x1842c0 -- the guest-LINK addr

MAGIC2_1 = 0x324B4954  # "TIK2" (MC2 magic tail; MAGIC0 'MGCT' is shared)

# InGameLoop_47320 frame-limiter signature. Locates the hook without hardcoding
# any VA: the `mov esi,[obj3ref]` / `call rel32` / `add esi,5` / `cmp esi,[same
# obj3ref]` / `ja $-6` shape is unique. group 1 = the GameTimerTurn obj3-disp
# (read at runtime to derive obj3's real base), group 2 = the frame-driver
# rel32. The two disps must be identical (same GameTimerTurn global).
import re as _re

MC2_LIMITER_SIG = _re.compile(
    rb"\x8b\x35(....)"      # mov esi,[GameTimerTurn]   (obj3-rel disp32)
    rb"\xe8(....)"          # call DrawAndEventsInGame   (rel32 -> frame driver)
    rb"\x83\xc6."           # add esi,imm8               (frame period; --pace
    #                         rewrites the imm8, so wildcard it -- else a paced
    #                         exe stops being recognised as MC2)
    rb"\x3b\x35(....)"      # cmp esi,[GameTimerTurn]
    rb"\x77\xf8",           # ja $-6                     (native limiter spin)
    _re.S,
)

WALLCLOCK_FROM_STRUCTPTR = 0x1E2C  # wallclock obj3-offset = structptr_off - this
GAMESPEED_PTR_FROM_STRUCTPTR = 8  # obj3ptr global obj3-offset = structptr_off + this
#   (the gameSpeed fan-out reads its runtime struct through THIS pointer global,
#    a different global from the tick fn's struct ptr; +8 holds in both builds.)
GAMESPEED_BYTE_OFF = 0x96  # F3 gameSpeed byte within that runtime struct (0/1/2)

# sub_41780 head: push ebx/esi/edi/ebp ; sub esp,0x158 ; mov esi,[structptr] ;
# imul eax,[esi+4],0x24a1 ; add eax,0x24df  (the :52223 LCG draw). Used only to
# LOCATE the tick fn -- we redirect its callers, never overwrite the entry.
TICKFN_PROLOGUE = bytes.fromhex("5356575581ec58010000")  # 10 bytes


# --------------------------------------------------------------------------
# Minimal LE parser
# --------------------------------------------------------------------------
@dataclass
class Obj:
    vsize: int
    vbase: int
    flags: int
    pageidx: int
    npages: int


@dataclass
class LE:
    data: bytearray
    lx: int
    datapages: int
    objs: list


def parse_le(data: bytes) -> LE:
    lx = struct.unpack_from("<I", data, 0x3C)[0]
    if data[lx : lx + 2] != b"LE":
        raise ValueError("not an LE executable (no 'LE' at MZ+0x3C)")
    g = lambda off: struct.unpack_from("<I", data, lx + off)[0]
    objtab, nobj, datapages = g(0x40), g(0x44), g(0x80)
    objs = []
    for i in range(nobj):
        vsize, vbase, flags, pageidx, npages, _ = struct.unpack_from(
            "<6I", data, lx + objtab + i * 24
        )
        objs.append(Obj(vsize, vbase, flags, pageidx, npages))
    return LE(bytearray(data), lx, datapages, objs)


def obj_file_off(le: LE, obj: Obj) -> int:
    return le.datapages + (obj.pageidx - 1) * 0x1000


def le_fixup_sources(le: LE, obj_index: int = 0) -> set:
    """Every LE internal-fixup SOURCE offset within an object, as object-relative
    offsets.

    Needed because a fixup is applied by the loader BEFORE any code runs: if we
    overwrite an instruction whose operand carries one, the loader stamps a
    relocated absolute over our bytes and the patch becomes a wild branch. (That
    is exactly how the first MC2 headless hook crashed DOSBox at launch.)
    """
    d = le.data
    lx = le.lx
    npages = struct.unpack_from("<I", d, lx + 0x14)[0]
    fpt = lx + struct.unpack_from("<I", d, lx + 0x68)[0]  # fixup page table
    frt = lx + struct.unpack_from("<I", d, lx + 0x6C)[0]  # fixup record table
    obj = le.objs[obj_index]
    first_page = 0
    for o in le.objs[:obj_index]:
        first_page += o.npages
    out = set()
    for pg in range(obj.npages):
        gp = first_page + pg
        if gp + 1 > npages:
            break
        s = struct.unpack_from("<I", d, fpt + gp * 4)[0]
        e = struct.unpack_from("<I", d, fpt + (gp + 1) * 4)[0]
        p, end = frt + s, frt + e
        while p < end:
            src, flags = d[p], d[p + 1]
            srcoff = struct.unpack_from("<h", d, p + 2)[0]
            p += 4
            if (src & 0xF) != 7:  # only internal refs are decoded here
                break
            p += 2 if (flags & 0x40) else 1  # object number
            p += 4 if (flags & 0x10) else 2  # target offset
            if 0 <= srcoff < 0x1000:  # negative = spills from the previous page
                out.add(pg * 0x1000 + srcoff)
    return out


def assert_no_fixup(le: LE, va: int, length: int, what: str) -> None:
    """Refuse to overwrite `length` bytes at `va` if the loader would relocate
    any of them. See `le_fixup_sources`."""
    obj = le.objs[0]
    off = va - obj.vbase
    hits = sorted(f for f in le_fixup_sources(le, 0) if off <= f < off + length)
    if hits:
        raise ValueError(
            f"{what}: {va:#x}+{length} carries LE fixup(s) at object offset "
            f"{[hex(h) for h in hits]} -- the loader would overwrite the patch "
            f"before it runs. Pick a fixup-free site (a relative call/jmp)."
        )


def va_to_file(le: LE, va: int) -> int:
    o = le.objs[0]
    if not (o.vbase <= va < o.vbase + o.npages * 0x1000):
        raise ValueError(f"VA {va:#x} not in obj1 code pages")
    return obj_file_off(le, o) + (va - o.vbase)


# --------------------------------------------------------------------------
# Locate the tick fn / cave / wallclock in a given build
# --------------------------------------------------------------------------
@dataclass
class Build:
    name: str
    hook_va: int  # tick fn entry (called by the stub; NEVER overwritten)
    call_sites: tuple  # VAs of the `call hook` instructions we redirect
    cave_va: int
    wallclock: int  # runtime flat addr of the ~120 Hz PIT counter (link space)
    structptr_off: int  # obj3-relative offset of the struct-ptr global (its
    #                     runtime disp32 lives in `mov esi,[..]` at hook_va+0xC)
    frame_call_site: int = 0  # GameLoop's `call DrawAndEventsInGame` (init park)
    frame_fn: int = 0  # DrawAndEventsInGame_34530_348F0


# GameLoop_34610's call of the frame driver, the MC1 init park's hook. The
# bytes before it are the `+13325` word clear (`mov word [ebx+eax+0x340D],0`;
# remc1's :41766 `var_u8_13327 = 0` is a source-corruption of it) and a
# `jmp +5` over the call; the call's rel32 is wildcarded -- the patch rewrites
# it, and a signature must never pin the bytes its own patch rewrites.
MC1_FRAMECALL_SIG = _re.compile(
    rb"\x66\xc7\x84\x03\x0d\x34\x00\x00\x00\x00"  # mov word [ebx+eax+0x340D],0
    rb"\xeb\x05"                                  # jmp +5 (over the call)
    rb"\xe8(....)",                               # call DrawAndEventsInGame
    _re.S,
)


def find_mc1_frame_call(le: LE, code: bytes) -> tuple[int, int]:
    """(call site VA, frame driver VA) of GameLoop's `call DrawAndEvents`.
    Refuses unless the signature is unique AND the driver has exactly that
    one caller in the whole image (so the hook is THE frame-1 entry)."""
    o1 = le.objs[0]
    hits = list(MC1_FRAMECALL_SIG.finditer(code))
    if len(hits) != 1:
        raise SystemExit(f"MC1 frame-driver call signature: {len(hits)} hits "
                         "(expected 1) -- not a pristine CARPET/HIDDEN.EXE?")
    site = o1.vbase + hits[0].start() + 12
    fn = (site + 5 + struct.unpack("<i", hits[0].group(1))[0]) & 0xFFFFFFFF
    callers = [i for i in range(len(code) - 5) if code[i] == 0xE8
               and o1.vbase + i + 5 + struct.unpack_from("<i", code, i + 1)[0] == fn]
    if callers != [site - o1.vbase]:
        raise SystemExit(f"frame driver {fn:#x} has {len(callers)} callers "
                         "(expected exactly the GameLoop one)")
    return site, fn


def find_build(le: LE) -> Build:
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    # Anchor on the prologue immediately followed by the struct load + LCG draw.
    # mov esi,[imm32] ; imul eax,[esi+4],0x24a1 ; add eax,0x24df
    import re

    pat = re.compile(
        re.escape(TICKFN_PROLOGUE)
        + rb"\x8b\x35(....)\x69\x46\x04\xa1\x24\x00\x00\x05\xdf\x24\x00\x00",
        re.S,
    )
    hits = list(pat.finditer(code))
    if len(hits) == 0:
        raise SystemExit(
            "tick-fn signature not found (0 hits). This is not a pristine "
            "CARPET.EXE / HIDDEN.EXE -- already patched, or an unexpected build."
        )
    if len(hits) != 1:
        raise SystemExit(f"expected exactly 1 tick-fn signature, found {len(hits)}")
    m = hits[0]
    hook_va = o1.vbase + m.start()
    structptr_pre = struct.unpack("<I", m.group(1))[0]
    structptr_runtime = OBJ3_BASE + structptr_pre
    wallclock = structptr_runtime - WALLCLOCK_FROM_STRUCTPTR

    # Validate the wallclock is an incremented counter (ISR writer present).
    wc_pre = struct.pack("<I", wallclock - OBJ3_BASE)
    if (b"\xff\x05" + wc_pre) not in code:
        raise ValueError(
            f"wallclock {wallclock:#x}: no 'inc [wc]' writer -- derivation suspect"
        )

    # Validate the F3 gameSpeed fan-out is the shape the pacer stub relies on
    # (obj3ptr global @ structptr_off+8, then gameSpeed byte @ +0x96). Match the
    # FULL `mov ebx,[obj3ptr] ; mov bl,[ebx+0x96]` form -- NOT a bare
    # `96 00 00 00` (a decoy flat-global `mov dl,[0x96]` lives elsewhere in the
    # code). Fail loudly if absent so an unexpected build can't be mispatched.
    gs_ptr_pre = structptr_pre + GAMESPEED_PTR_FROM_STRUCTPTR
    fanout_sig = (b"\x8b\x1d" + struct.pack("<I", gs_ptr_pre)
                  + b"\x8a\x9b" + struct.pack("<I", GAMESPEED_BYTE_OFF))
    if fanout_sig not in code:
        raise SystemExit(
            f"gameSpeed fan-out signature not found "
            f"(`mov ebx,[obj3+{gs_ptr_pre:#x}] ; mov bl,[ebx+{GAMESPEED_BYTE_OFF:#x}]`"
            f" = {fanout_sig.hex()}). The pacer's gameSpeed derivation is not "
            f"valid on this build -- refusing to patch."
        )

    # The call sites: `E8 rel32` (5 bytes) whose target is the tick fn. These
    # are the gameSpeed fan-out (remc1 :41677/41683/41688) -- redirecting them
    # to the stub leaves the tick fn's entry completely untouched (so no
    # detour bytes to be misdecoded), which is the whole point.
    call_sites = []
    for i in range(len(code) - 5):
        if code[i] == 0xE8:
            tgt = o1.vbase + i + 5 + struct.unpack_from("<i", code, i + 1)[0]
            if tgt == hook_va:
                call_sites.append(o1.vbase + i)
    if not call_sites:
        raise SystemExit(f"no `call {hook_va:#x}` sites found to redirect")

    # Cave = obj1's zero tail past vsize.
    cave_va = o1.vbase + o1.vsize
    cave_off = code_off + o1.vsize
    cave_end = code_off + o1.npages * 0x1000
    if any(le.data[cave_off:cave_end]):
        raise ValueError("obj1 tail cave is not zero-filled")

    # The mailbox lives in obj3's committed BSS tail (past vsize, within the
    # last committed page). Verify MB_OBJ3 is beyond obj3's declared data and
    # still inside the page DOS/4GW commits.
    obj3 = le.objs[2]
    if obj3.vbase != OBJ3_BASE:
        raise ValueError(f"obj3 vbase {obj3.vbase:#x} != {OBJ3_BASE:#x}")
    committed = (obj3.vsize + 0xFFF) & ~0xFFF
    if not (obj3.vsize <= MB_OBJ3 and MB_END <= committed):
        raise ValueError(
            f"mailbox obj3-off {MB_OBJ3:#x} not in obj3 tail "
            f"[vsize {obj3.vsize:#x}, committed {committed:#x})")

    name = "CARPET" if wallclock == 0xAC5D4 else ("HIDDEN" if wallclock == 0xAC5C4 else "?")
    frame_call_site, frame_fn = find_mc1_frame_call(le, code)
    return Build(name, hook_va, tuple(call_sites), cave_va, wallclock, structptr_pre,
                 frame_call_site, frame_fn)


# --------------------------------------------------------------------------
# Tiny assembler: raw bytes + label-relative branches, two-pass resolve.
# --------------------------------------------------------------------------
class Asm:
    def __init__(self, base_va: int):
        self.base = base_va
        self.items = []  # ('raw', bytes) | ('label', name) | ('br', op, width, target)
        self.size = 0

    def raw(self, b: bytes):
        self.items.append(("raw", b))
        self.size += len(b)

    def label(self, name: str):
        self.items.append(("label", name))

    def br8(self, op: int, target: str):
        self.items.append(("br", bytes([op]), 1, target))
        self.size += 2

    def jmp32(self, target: str):
        self.items.append(("br", b"\xe9", 4, target))
        self.size += 5

    # Convenience encoders. DATA references are OBJ3-relative ([edx + off],
    # ModRM 0x82 = mod=10 rm=010/edx): the stub holds obj3's real runtime base
    # in EDX (derived from the game's own relocated struct-ptr), so writes land
    # in obj3, never in game memory. During the preamble EDX briefly holds the
    # obj1 load delta instead, used only to read one relocated code disp.
    def call_next(self):  # call $+5 (pushes EIP of the following instr)
        self.raw(b"\xe8\x00\x00\x00\x00")

    def pop_edx(self):
        self.raw(b"\x5a")

    def push_edx(self):
        self.raw(b"\x52")

    def sub_edx_imm(self, imm):
        self.raw(b"\x81\xea" + struct.pack("<I", imm & 0xFFFFFFFF))

    def sub_eax_imm(self, imm):  # sub eax, imm32
        self.raw(b"\x2d" + struct.pack("<I", imm & 0xFFFFFFFF))

    def add_eax_imm(self, imm):  # add eax, imm32
        self.raw(b"\x05" + struct.pack("<I", imm & 0xFFFFFFFF))

    def mov_edx_eax(self):  # mov edx, eax
        self.raw(b"\x89\xc2")

    def mov_eax_m(self, a):  # mov eax,[edx+a]
        self.raw(b"\x8b\x82" + struct.pack("<I", a))

    def mov_m_eax(self, a):  # mov [edx+a],eax
        self.raw(b"\x89\x82" + struct.pack("<I", a))

    def mov_m_imm(self, a, imm):  # mov dword [edx+a],imm
        self.raw(b"\xc7\x82" + struct.pack("<I", a) + struct.pack("<I", imm & 0xFFFFFFFF))

    def inc_m(self, a):  # inc dword [edx+a]
        self.raw(b"\xff\x82" + struct.pack("<I", a))

    def add_eax_m(self, a):  # add eax,[edx+a]
        self.raw(b"\x03\x82" + struct.pack("<I", a))

    def test_eax(self):
        self.raw(b"\x85\xc0")

    def movzx_eax_byte_eax(self, disp):  # movzx eax, byte [eax+disp32]  (base EAX)
        self.raw(b"\x0f\xb6\x80" + struct.pack("<I", disp))

    def cmp_ebx_imm8(self, imm):  # cmp ebx, imm8
        self.raw(b"\x83\xfb" + struct.pack("<b", imm))

    def sub_eax_m(self, a):  # sub eax,[edx+a]
        self.raw(b"\x2b\x82" + struct.pack("<I", a))

    def cmp_eax_m(self, a):  # cmp eax,[edx+a]
        self.raw(b"\x3b\x82" + struct.pack("<I", a))

    def cmp_eax_imm(self, imm):
        self.raw(b"\x3d" + struct.pack("<I", imm & 0xFFFFFFFF))

    def mov_ecx_imm(self, imm):
        self.raw(b"\xb9" + struct.pack("<I", imm & 0xFFFFFFFF))

    def push_ecx(self):
        self.raw(b"\x51")

    def pop_ecx(self):
        self.raw(b"\x59")

    def dec_ecx(self):
        self.raw(b"\x49")

    def cmp_m_imm8(self, a, imm):  # cmp dword [edx+a], imm8
        self.raw(b"\x83\xba" + struct.pack("<I", a) + struct.pack("<b", imm))

    def assemble(self) -> bytes:
        # pass 1: label offsets
        pos, labels = 0, {}
        for it in self.items:
            if it[0] == "raw":
                pos += len(it[1])
            elif it[0] == "label":
                labels[it[1]] = pos
            else:
                pos += 1 + it[2]
        # pass 2: emit
        out = bytearray()
        pos = 0
        for it in self.items:
            if it[0] == "raw":
                out += it[1]
                pos += len(it[1])
            elif it[0] == "label":
                pass
            else:
                _, opb, width, tgt = it
                nextpos = pos + 1 + width
                disp = labels[tgt] - nextpos
                out += opb
                if width == 1:
                    if not (-128 <= disp <= 127):
                        raise ValueError(f"rel8 to {tgt} out of range ({disp})")
                    out += struct.pack("<b", disp)
                else:
                    out += struct.pack("<i", disp)
                pos = nextpos
        return bytes(out)


def emit_init_park(a: "Asm", arm_off: int, park_off: int, pmagic_off: int,
                   timer_off: int, timeout: int) -> None:
    """THE INIT PARK (see PARK_* above): on a fresh process (no PMAGIC) or
    when armed, disarm, raise PARK and spin until the host writes
    PARK_RELEASE or `timeout` timer counts pass; leave PARK_RELEASED /
    PARK_TIMEOUT behind. EDX = obj3 base (in and out), EAX is clobbered, ECX
    is saved around the guard. Emits nothing for timeout 0."""
    if not timeout:
        return
    a.mov_eax_m(pmagic_off)
    a.cmp_eax_imm(PARK_MAGIC)
    a.br8(0x74, "park_warm")  # je park_warm  (this process has parked before)
    a.mov_m_imm(pmagic_off, PARK_MAGIC)  # fresh process: park its first level
    a.br8(0xEB, "park_go")
    a.label("park_warm")
    a.cmp_m_imm8(arm_off, 0)
    a.br8(0x74, "park_skip")  # je park_skip  (not armed)
    a.label("park_go")
    a.mov_m_imm(arm_off, 0)  # one park per arming
    a.mov_m_imm(park_off, PARK_PARKED)
    a.mov_eax_m(timer_off)
    a.add_eax_imm(timeout)  # eax = release deadline (timer counts)
    a.push_ecx()
    a.mov_ecx_imm(PARK_GUARD_ITERS)
    a.label("park_spin")
    a.cmp_m_imm8(park_off, PARK_RELEASE)
    a.br8(0x74, "park_host")  # je park_host  (the host has its snapshot)
    a.cmp_eax_m(timer_off)  # deadline vs now
    a.br8(0x7E, "park_timeout")  # jle park_timeout
    a.dec_ecx()
    a.br8(0x75, "park_spin")  # jnz park_spin
    a.label("park_timeout")  # (the dead-ISR guard lands here too)
    a.mov_m_imm(park_off, PARK_TIMEOUT)
    a.br8(0xEB, "park_done")
    a.label("park_host")
    a.mov_m_imm(park_off, PARK_RELEASED)
    a.label("park_done")
    a.pop_ecx()
    a.label("park_skip")


def build_passthrough(b: Build) -> bytes:
    """A bare wrapper: `call <tick fn> ; ret`, nothing else. Wired in place of
    the full stub, it isolates whether merely calling the tick fn through a
    cave trampoline is the problem, independent of any pacing logic."""
    rel = b.hook_va - (b.cave_va + 5)  # E8 rel32 at cave_va+0, so +5
    return b"\xe8" + struct.pack("<i", rel) + b"\xc3"


def build_stub(b: Build, period: int, floor: int = FLOOR_DEFAULT) -> bytes:
    a = Asm(b.cave_va)
    wc_off = b.structptr_off - WALLCLOCK_FROM_STRUCTPTR  # wallclock obj3-offset
    gs_ptr_off = b.structptr_off + GAMESPEED_PTR_FROM_STRUCTPTR  # obj3ptr global

    # --- derive obj3's real runtime base into EDX ---
    # DOS/4GW relocates objects independently and injected code gets no LE
    # fixups, so we can't assume any load base. Instead read the game's OWN
    # relocated pointer: `mov esi,[structptr]` at hook_va+0xC holds the disp32
    # that the loader fixed up to (obj3_base + structptr_off). Step 1 gets the
    # obj1 load delta (call/pop) purely to locate that code disp; step 2 reads
    # it and subtracts structptr_off to recover obj3_base. From then on EDX is
    # obj3_base and every data ref is obj3-relative, so writes stay in obj3.
    a.call_next()  # push EIP of the pop below
    a.pop_edx()  # edx = runtime(pop)
    a.sub_edx_imm(b.cave_va + 5)  # edx = obj1 load delta (link of pop = cave+5)
    a.mov_eax_m(b.hook_va + 0xC)  # eax = [edx + disp_va] = obj3_base + structptr_off
    a.sub_eax_imm(b.structptr_off)  # eax = obj3_base (runtime)
    a.mov_edx_eax()  # edx = obj3_base for all data refs below

    # --- one-time init (gated on the magic, robust to a non-zero tail) ---
    a.mov_eax_m(MB_MAGIC0)
    a.cmp_eax_imm(MAGIC0)
    a.br8(0x74, "after_init")  # je after_init  (already initialised)
    a.mov_m_imm(MB_MAGIC1, MAGIC1)
    a.mov_m_imm(MB_PERIOD, period)
    a.mov_m_imm(MB_TICK, 0)
    a.mov_eax_m(wc_off)
    a.mov_m_eax(MB_DEADLINE)
    a.mov_m_imm(MB_MAGIC0, MAGIC0)  # write magic LAST -> mailbox is atomic-ish
    a.label("after_init")

    # --- bump the sub-step counter and publish gameSpeed EVERY sub-step ---
    a.inc_m(MB_TICK)
    a.mov_eax_m(gs_ptr_off)  # eax = *obj3ptr = the runtime struct pointer
    a.movzx_eax_byte_eax(GAMESPEED_BYTE_OFF)  # eax = gameSpeed (0/1/2)
    a.mov_m_eax(MB_SPEED)  # publish raw gameSpeed for the recorder

    # --- decide whether THIS sub-step paces (law A: one paced sub-step/frame) --
    # The F3 fan-out runs the tick fn 1/4/16x per frame (gameSpeed 0/1/2) with
    # EBX = the loop index (1 on the first sub-step, 2..N after). Pacing every
    # sub-step would nullify F3 (N steps x one per-step wait = same real-time
    # sim rate at 1/N the fps). Instead pace exactly ONE sub-step per frame:
    #   * gameSpeed 0 (the default) runs the tick fn once/frame -> always pace
    #     (bit-identical in effect to the old every-sub-step pacer), and we do
    #     this via `test eax; jz` WITHOUT reading EBX (which is loop garbage at
    #     speed 0, never 1);
    #   * gameSpeed 1/2 -> pace only the first sub-step (EBX==1); sub-steps
    #     2..N run FREE, so the SIM speeds up 4x/16x while fps stays put.
    a.test_eax()
    a.br8(0x74, "pace")  # jz pace   (speed 0 -> pace unconditionally)
    a.cmp_ebx_imm8(1)
    a.br8(0x75, "skip")  # jne skip  (a later sub-step of a fast frame runs free)

    a.label("pace")
    a.mov_m_imm(MB_INWIN, 1)  # window raised ONLY around a real spin

    # --- floor: deadline = max(deadline, now + floor) -------------------------
    # The clamp, not the spin, is where the floor lives: push the deadline far
    # enough ahead of NOW that the spin below cannot fall straight through, then
    # let the existing wait/guard/resync machinery do the waiting unchanged.
    #   * keeping up (deadline - now > floor)  -> no-op, healthy takes unaffected
    #   * overrunning (now >= deadline)        -> wait exactly `floor`, every
    #     frame, and the catch-up burst is gone with it (the deadline is rebuilt
    #     from NOW each overrun, so backlog cannot accumulate).
    # Signed throughout: `now` and `deadline` are both PIT counts and the
    # difference is small in either direction, so `jle` reads correctly whether
    # the deadline is ahead of or behind the clock.
    if floor:
        a.mov_eax_m(wc_off)  # eax = now
        a.add_eax_imm(floor)  # eax = now + floor
        a.sub_eax_m(MB_DEADLINE)  # eax = (now + floor) - deadline
        a.br8(0x7E, "no_floor")  # jle no_floor  (deadline already far enough out)
        a.add_eax_m(MB_DEADLINE)  # eax = now + floor
        a.mov_m_eax(MB_DEADLINE)
        a.label("no_floor")

    # --- spin until now >= deadline (or bail on a frozen counter) ---
    # diff = now - deadline as a SIGNED i32: negative => still waiting,
    # non-negative => the deadline passed. Signed handles both the normal
    # "deadline slightly ahead" wait and a post-pause "deadline far behind"
    # resync with the same subtraction (no unsigned underflow).
    #
    # The wall clock is NOT monotonic across a level: the game's delay helper
    # (remc1 sub_10300, reached from the screen-fade path on the way back to
    # the menu) spins until the clock reaches a target and then ZEROES it, and
    # ALT+L quickload restores the clock from the savegame. Either one leaves
    # `now` far BEHIND a deadline the mailbox carried over from the previous
    # level -- so on level 2 every paced sub-step would spin out the full guard
    # (~0.2 fps) until the clock climbed back to a stale value minutes ahead.
    # Bound the wait in the other direction too: a deadline more than one
    # period + the catch-up slack AHEAD of the clock cannot be schedule, only a
    # backwards clock step, so drop it and resync. Checked inside the loop so
    # it self-heals whenever the step lands, not just at stub entry.
    # `floor` too, not just `period`: the clamp above can legitimately leave the
    # deadline `floor` counts ahead of the clock, and a floor larger than
    # `period` would otherwise read as a backwards clock step and resync away
    # the very wait we just installed.
    back_limit = max(period, floor) + RESYNC_COUNTS
    a.mov_ecx_imm(GUARD_ITERS)
    a.label("spin")
    a.mov_eax_m(wc_off)  # eax = now
    a.sub_eax_m(MB_DEADLINE)  # eax = now - deadline (signed)
    a.br8(0x79, "passed")  # jns passed  (now >= deadline)
    a.cmp_eax_imm(-back_limit)  # still waiting: is the deadline absurdly far off?
    a.br8(0x7C, "resync")  # jl resync  (clock stepped backwards -> stale deadline)
    a.dec_ecx()
    a.br8(0x75, "spin")  # jnz spin  (keep waiting)
    a.br8(0xEB, "release")  # guard expired (counter frozen) -> release

    a.label("passed")
    a.cmp_eax_imm(RESYNC_COUNTS)  # eax >= 0 here
    a.br8(0x72, "release")  # jb release  (within one catch-up bound)
    a.label("resync")
    a.mov_eax_m(wc_off)  # clock jumped (long pause / level exit) -> drop backlog
    a.mov_m_eax(MB_DEADLINE)  # deadline = now

    a.label("release")
    a.mov_eax_m(MB_DEADLINE)
    a.add_eax_m(MB_PERIOD)
    a.mov_m_eax(MB_DEADLINE)  # deadline += period (fixed cadence, no drift)
    a.mov_m_imm(MB_INWIN, 0)
    a.label("skip")

    body = a.assemble()

    # --- call the ORIGINAL (untouched) tick fn, then return to the caller ---
    # A relative call: both the stub and the tick fn are in obj1, so the rel32
    # is position-independent (delta-invariant). The stub WRITES only
    # eax/ecx/edx and only READS ebx (the fan-out's loop index); the tick fn
    # saves/restores ebx/esi/edi/ebp itself, so the caller's callee-saved regs
    # (its loop counter in ebx) survive intact.
    call_pos = len(body)
    rel = b.hook_va - (b.cave_va + call_pos + 5)
    return body + b"\xe8" + struct.pack("<i", rel) + b"\xc3"  # call hook ; ret


def build_frame_stub_mc1(b: Build, va: int, init_park: int) -> bytes:
    """MC1's init park, wrapped around GameLoop's call of the frame driver:
    `pushad`, derive obj3 (exactly as the pacing stub does, off the tick
    fn's own fixed-up struct-ptr disp), park if this is a fresh process or
    the host armed it, `popad`, then TAIL-JUMP to the untouched driver --
    which returns straight to GameLoop. Every register and flag the caller
    had is handed to the driver unchanged (pushad/popad; the park's compares
    happen before popad, and the driver is a void() that reads no flags).
    Frame 1 has not run when this parks: no carpet, no spell tokens, the
    command slots still hold the join command -- the world as
    `sub_407A0_40AE0` left it plus GameLoop's two overwrites (+13325 = 0,
    already 0 after init; AE408 var_u8_23, not recorded)."""
    a = Asm(va)
    a.raw(b"\x60")  # pushad
    a.call_next()
    a.pop_edx()
    a.sub_edx_imm(va + 1 + 5)  # edx = obj1 load delta (link of pop = va+6)
    a.mov_eax_m(b.hook_va + 0xC)  # eax = obj3_base + structptr_off
    a.sub_eax_imm(b.structptr_off)
    a.mov_edx_eax()  # edx = obj3_base
    emit_init_park(a, MB_PARK_ARM, MB_PARK, MB_PARK_MAGIC,
                   b.structptr_off - WALLCLOCK_FROM_STRUCTPTR, init_park)
    a.raw(b"\x61")  # popad
    body = a.assemble()
    rel = b.frame_fn - (va + len(body) + 5)
    return body + b"\xe9" + struct.pack("<i", rel)  # jmp frame_fn


# --------------------------------------------------------------------------
# Patch / verify
# --------------------------------------------------------------------------
def patch(le: LE, b: Build, period: int, wire: bool = True, passthrough: bool = False,
          extend: bool = True, floor: int = FLOOR_DEFAULT,
          volcano: bool = False, timer_init: bool = True,
          init_park: int = INIT_PARK_DEFAULT) -> bytes:
    o1 = le.objs[0]
    stub = build_passthrough(b) if passthrough else build_stub(b, period, floor)
    cave_off = va_to_file(le, b.cave_va)
    if cave_off + len(stub) > obj_file_off(le, o1) + o1.npages * 0x1000:
        raise ValueError("stub overflows the cave")
    le.data[cave_off : cave_off + len(stub)] = stub

    # The volcano write guard shares the cave, placed straight after the pacing
    # stub (16-aligned) so the two never overlap however either one grows.
    vol_end = b.cave_va + len(stub)
    if volcano:
        vol_va = (b.cave_va + len(stub) + 15) & ~15
        vol_blob = patch_volcano(le, vol_va)
        vol_end = vol_va + len(vol_blob)
        if va_to_file(le, vol_end) > obj_file_off(le, o1) + o1.npages * 0x1000:
            raise ValueError("volcano stubs overflow the cave")

    # The `-custom` timer install shares the cave too, 16-aligned after
    # whatever precedes it. Like the volcano guard it is applied even under
    # --inert: --inert is about not wiring the PACER, and this stub's site is
    # unreachable unless the game is launched with the custom/network bit.
    if timer_init:
        tmr_va = (vol_end + 15) & ~15
        tmr_blob = patch_timer_init(le, tmr_va)
        vol_end = tmr_va + len(tmr_blob)
        if va_to_file(le, vol_end) > obj_file_off(le, o1) + o1.npages * 0x1000:
            raise ValueError("timer-init stub overflows the cave")

    # The init park's frame-driver stub, 16-aligned after whatever precedes
    # it. Only wired together with the pacer (never under --inert /
    # --passthrough: those are diagnostics of the pacing stub alone).
    park_va = None
    if init_park and wire and not passthrough:
        park_va = (vol_end + 15) & ~15
        park_blob = build_frame_stub_mc1(b, park_va, init_park)
        poff = va_to_file(le, park_va)
        le.data[poff : poff + len(park_blob)] = park_blob
        vol_end = park_va + len(park_blob)
        if va_to_file(le, vol_end) > obj_file_off(le, o1) + o1.npages * 0x1000:
            raise ValueError("init-park stub overflows the cave")

    # Both the code cave (obj1 tail) and the mailbox (obj3 tail) sit PAST their
    # object's declared vsize, so at runtime those tails fall outside the
    # segment limit: jumping into obj1's tail faults, and WRITES into obj3's
    # tail don't persist (the magic never sticks -> init re-runs every call ->
    # the pacing deadline is reset to `now` every call -> no throttle).
    # Page-align both vsizes so the tails become declared, in-limit segment
    # space. The file already provides / commits these pages; only the declared
    # size was short of page-aligned.
    if extend:
        objtab = struct.unpack_from("<I", le.data, le.lx + 0x40)[0]
        new1 = (o1.vsize + 0xFFF) & ~0xFFF
        if vol_end > o1.vbase + new1:
            raise ValueError("stub crosses the page boundary; extend by another page")
        struct.pack_into("<I", le.data, le.lx + objtab + 0 * 24, new1)
        o1.vsize = new1

        o3 = le.objs[2]
        new3 = (o3.vsize + 0xFFF) & ~0xFFF
        if MB_END > new3:
            raise ValueError("mailbox past obj3's page-aligned vsize")
        struct.pack_into("<I", le.data, le.lx + objtab + 2 * 24, new3)
        o3.vsize = new3

    if not wire:
        return stub  # --inert: stub written, call sites untouched (never executed)

    # Redirect each `call hook` to `call stub` -- rewrite only the 4-byte rel32.
    # The tick fn's entry is left byte-for-byte untouched.
    for cs in b.call_sites:
        off = va_to_file(le, cs)
        if le.data[off] != 0xE8:
            raise ValueError(f"call site {cs:#x} is not an E8 call")
        rel = b.cave_va - (cs + 5)
        le.data[off + 1 : off + 5] = struct.pack("<i", rel)
    if park_va is not None:
        off = va_to_file(le, b.frame_call_site)
        if le.data[off] != 0xE8:
            raise ValueError(f"frame call site {b.frame_call_site:#x} is not an E8 call")
        assert_no_fixup(le, b.frame_call_site + 1, 4, "MC1 frame-driver call rel32")
        le.data[off + 1 : off + 5] = struct.pack("<i", park_va - (b.frame_call_site + 5))
    return stub


# --------------------------------------------------------------------------
# THE VOLCANO WRITE GUARD (round 162) -- optional, MC1/HW only.
#
# Retail's eruption start (`sub_25EC0`, VA 0x25FBC / 0x26030 in BOTH builds)
# makes TWO BLIND WRITES through global slot registers whose only gate is
# `slot != 0` (the `> pool base` pointer test) -- no class, model or life
# check -- so each stamps whatever record now occupies that slot:
#
#   KICK  VA 0x25FBE  `66 c7 42 1a fa 00`  movw $0xfa,0x1a(%edx)
#         edx = pool_base + erupting_reg*164. `+26 = 250` is meant for the
#         PREVIOUS volcano (10,18). Landing on a re-minted CASTLE sets its
#         LEVEL to 250; every later downgrade reads the build table out of
#         bounds and the collapse walker `sub_28FE0` run for row 245 reads
#         127 rows from a garbage pointer and never returns -- the
#         player-reported FREEZE (mc1l45-froze castle 966, mc1l26-froze
#         castle 984). A register naming the driver's own slot self-kicks it.
#
#   PLUME VA 0x26035  `e8 rel32`            call <soft-kill>(old plume)
#         eax = pool_base + plume_reg*164, pushed as the arg. Meant for the
#         previous (10,19) column; lands on whatever inherited the slot.
#
# The guard is EXACTLY the port's `volcano_register_revalidate` patch
# (crates/mgc-sim/src/patches.rs, crates/mgc-sim/src/mc1/combat.rs): the kick
# lands only on a (10,18) that is not the driver itself, the plume kill only
# on a (10,19). Nothing else changes -- this is a write guard, not a rewrite.
#
# ⚠ A binary carrying this guard is NO LONGER THE SHIPPED BINARY: unlike the
# pacing stub (whose recorded tick sequence is identical to retail's) this
# CHANGES SIMULATION. Takes made with it witness the PATCHED arm, never the
# faithful one, and must never be graded as retail.
#
# Class/model live at +64/+65 of a 164-byte record; a volcano is (10,18) and
# its plume (10,19).
VOL_KICK_SIG = bytes.fromhex("760666c7421afa00")   # jbe +6 ; movw $0xfa,0x1a(%edx)
VOL_PLUME_SIG = bytes.fromhex("39d0760950e8")      # cmp %edx,%eax ; jbe +9 ; push %eax ; call
VOL_CLASS_OFF, VOL_MODEL_OFF = 0x40, 0x41
VOL_CLASS, VOL_MODEL_VOLCANO, VOL_MODEL_PLUME = 10, 18, 19


def find_volcano_sites(le: LE) -> tuple[int, int]:
    """(kick_store_va, plume_call_va), located by unique byte signature."""
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    blob = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    def one(sig: bytes, what: str) -> int:
        hits = []
        i = blob.find(sig)
        while i >= 0:
            hits.append(i)
            i = blob.find(sig, i + 1)
        if len(hits) != 1:
            raise ValueError(f"volcano {what}: expected 1 signature hit, got {len(hits)}")
        return o1.vbase + hits[0]

    # The kick SIGNATURE starts at the `jbe`; the store is 2 bytes in. The
    # plume signature starts at the `cmp`; its `call` is 5 bytes in.
    return one(VOL_KICK_SIG, "kick") + 2, one(VOL_PLUME_SIG, "plume") + 5


def build_volcano_stubs(vol_va: int, killer_va: int) -> tuple[bytes, int, int]:
    """(blob, kick_stub_va, plume_stub_va) for the cave at `vol_va`."""
    # --- stub 1: the kick guard. Entered by `call`; edx = the register's
    # record, ebx = the erupting driver's own record. Flags are dead across
    # the original store (the next instruction reloads a global), but they are
    # preserved anyway so the stub is safe to re-point at any similar site.
    k = bytearray()
    k += b"\x9c"                                    # pushfd
    k += b"\x39\xda"                                # cmp %ebx,%edx      (prev == self?)
    k += b"\x74\x12"                                # je  skip
    k += bytes([0x80, 0x7A, VOL_CLASS_OFF, VOL_CLASS])       # cmpb $10,0x40(%edx)
    k += b"\x75\x0c"                                # jne skip
    k += bytes([0x80, 0x7A, VOL_MODEL_OFF, VOL_MODEL_VOLCANO])  # cmpb $18,0x41(%edx)
    k += b"\x75\x06"                                # jne skip
    k += b"\x66\xc7\x42\x1a\xfa\x00"                # movw $0xfa,0x1a(%edx)
    k += b"\x9d"                                    # skip: popfd
    k += b"\xc3"                                    # ret
    assert len(k) == 25, len(k)

    # --- stub 2: the plume guard. Entered by `call` with the record already
    # pushed, so the arg is at [esp+4]. On a pass it TAIL-JUMPS to the original
    # killer, whose own `ret` returns straight to the caller -- the stack frame
    # is exactly what it expects. eax is dead across the original cdecl call.
    plume_va = vol_va + len(k)
    p = bytearray()
    p += b"\x8b\x44\x24\x04"                        # mov 0x4(%esp),%eax
    p += bytes([0x80, 0x78, VOL_CLASS_OFF, VOL_CLASS])       # cmpb $10,0x40(%eax)
    p += b"\x75\x0b"                                # jne skip
    p += bytes([0x80, 0x78, VOL_MODEL_OFF, VOL_MODEL_PLUME]) # cmpb $19,0x41(%eax)
    p += b"\x75\x05"                                # jne skip
    jmp_va = plume_va + len(p)
    p += b"\xe9" + struct.pack("<i", killer_va - (jmp_va + 5))   # jmp <killer>
    p += b"\xc3"                                    # skip: ret
    assert len(p) == 22, len(p)
    return bytes(k + p), vol_va, plume_va


def patch_volcano(le: LE, vol_va: int) -> bytes:
    """Install the write guard, stubs at `vol_va`. Returns the stub blob."""
    kick_va, plume_call_va = find_volcano_sites(le)

    # Resolve the original soft-kill target from the existing rel32.
    pc_off = va_to_file(le, plume_call_va)
    if le.data[pc_off] != 0xE8:
        raise ValueError(f"plume site {plume_call_va:#x} is not an E8 call")
    killer_va = plume_call_va + 5 + struct.unpack_from("<i", le.data, pc_off + 1)[0]

    blob, kick_stub_va, plume_stub_va = build_volcano_stubs(vol_va, killer_va)
    off = va_to_file(le, vol_va)
    le.data[off : off + len(blob)] = blob

    # Site 1: the 6-byte store becomes `call <kick stub>` + one nop. The `jbe`
    # two bytes above still jumps +6, landing on the same next instruction.
    ks_off = va_to_file(le, kick_va)
    if bytes(le.data[ks_off : ks_off + 6]) != bytes.fromhex("66c7421afa00"):
        raise ValueError(f"kick site {kick_va:#x} is not the expected store")
    le.data[ks_off : ks_off + 5] = b"\xe8" + struct.pack("<i", kick_stub_va - (kick_va + 5))
    le.data[ks_off + 5] = 0x90

    # Site 2: re-point the existing call at the plume stub (same length).
    le.data[pc_off + 1 : pc_off + 5] = struct.pack("<i", plume_stub_va - (plume_call_va + 5))
    return blob


# --------------------------------------------------------------------------
# The `-custom` timer install (MC1 only)
# --------------------------------------------------------------------------
# `-custom` (and `-network` / `-demo N` / `-roll N`, which set the SAME bit --
# 0x01 of [struct+1], remc1 sub_main.cpp:42280-42315) makes `TopProcedure`
# skip the menu loop. That is the whole point of the flag: it is the only way
# to make `-level N` stick. But the menu loop is also the ONLY place the game
# installs a timer:
#
#     sub_357C0_35B80()        :58139   <- installs the ISR
#       <- sub_4AC70_4AFB0()   :57885   (the decompile's own note: "sound joystick")
#         <- sub_4AB20_4AE60() :41541   INSIDE `if ((var_u8_1 & 1) == 0)`
#           <- TopProcedure
#
# and it is that function -- not the sound driver -- that owns BOTH arms:
#
#     if (byte_939E4 || byte_939CC) { sub_5D97B(0x78, sub_357A0, ..); }  // sound IRQ
#     else                          { sub_5A459_5A969(); }               // "START_TIMER":
#                                                                        // PIT ch0 divisor
#                                                                        // 9903 = 120.5 Hz,
#                                                                        // vector 8 -> sub_5A3E3
#
# Both arms increment `wallclock` (the two `inc [wc]` sites in the image are
# sub_357A0 and sub_5A3E3), so sound being off is NOT the problem -- the
# no-sound arm is a complete timer installer. The problem is that under
# `-custom` the function containing both arms is never CALLED, so the counter
# the pacer spins on never advances, the stub's frozen-timer guard fires on
# every frame, and the game runs at under 1 fps. Measured 2026-09-22; the
# UNPATCHED binary under `-custom` is fine, because retail's main loop does
# not depend on that counter.
#
# The fix is one call. `sub_357C0_35B80` is idempotent (`byte_90AD4` once-guard),
# and we splice it into the custom branch itself, so the patched bytes are
# UNREACHABLE on the menu path -- every existing menu-path capture is byte-for-
# byte unaffected at runtime. It self-selects the correct arm, so we make no
# decision about sound. (Note it will usually take the sound arm, since the
# sound init inside it SETS byte_939E4/byte_939CC; `-custom -time` sets
# var_u8_0 |= 0x40, which takes the silent START_TIMER arm instead.)
#
# Anchors, both verified unique in CARPET.EXE and HIDDEN.EXE:
#   installer:  `call <rel32> ; or byte [disp32],0x80`  (remc1 :58139-58140)
#               -> 0x357C0 (CARPET) / 0x35B80 (HIDDEN)
#   custom arm: `xor %bl,%bl ; push imm32 ; (mov %bl,[disp32]) x4 ; call <rel32>`
#               -> call site 0x3413B (CARPET) / 0x344FB (HIDDEN), whose target
#                  is sub_40440_40780 (the text.dat load).
TIMER_INSTALLER_SIG = _re.compile(rb"\xe8(....)\x80\x0d....\x80", _re.S)
TIMER_CUSTOM_SIG = _re.compile(rb"\x30\xdb\x68....(?:\x88\x1d....){4}\xe8(....)", _re.S)
TIMER_CUSTOM_CALL_OFF = 2 + 5 + 4 * 6  # branch start -> the `call` opcode


def find_timer_sites(le: LE) -> tuple:
    """(installer_va, custom_call_va, orig_target_va) for the MC1 builds."""
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    hits = list(TIMER_INSTALLER_SIG.finditer(code))
    if len(hits) != 1:
        raise SystemExit(
            f"timer-installer signature: expected 1 hit, found {len(hits)} "
            f"(`call sub_357C0 ; or byte [..],0x80`, remc1 :58139-58140)"
        )
    m = hits[0]
    site = o1.vbase + m.start()
    installer_va = site + 5 + struct.unpack("<i", m.group(1))[0]

    hits = list(TIMER_CUSTOM_SIG.finditer(code))
    if len(hits) != 1:
        raise SystemExit(
            f"custom-branch signature: expected 1 hit, found {len(hits)} "
            f"(`xor %bl,%bl ; push imm32 ; 4x mov %bl,[..] ; call`, remc1 :41506-41512)"
        )
    m = hits[0]
    custom_call_va = o1.vbase + m.start() + TIMER_CUSTOM_CALL_OFF
    orig_target_va = custom_call_va + 5 + struct.unpack("<i", m.group(1))[0]
    return installer_va, custom_call_va, orig_target_va


def build_timer_stub(tmr_va: int, installer_va: int, orig_target_va: int) -> bytes:
    """Entered by `call` with sub_40440's single cdecl arg already at [esp+4].

    Installs the timer, then TAIL-JUMPS to the original target so its own `ret`
    returns straight to the custom branch with the stack exactly as it expects
    -- the same shape as the volcano plume guard above. Both branches are
    rel32 WITHIN obj1 (the cave is obj1's tail), so neither needs an LE fixup
    nor the obj3-base derivation the pacer stub has to do for its data refs.
    """
    t = bytearray()
    t += b"\x60"  # pushad   (the installer is void/no-arg; belt and braces)
    call_va = tmr_va + len(t)
    t += b"\xe8" + struct.pack("<i", installer_va - (call_va + 5))  # call <installer>
    t += b"\x61"  # popad
    jmp_va = tmr_va + len(t)
    t += b"\xe9" + struct.pack("<i", orig_target_va - (jmp_va + 5))  # jmp <orig>
    assert len(t) == 12, len(t)
    return bytes(t)


def patch_timer_init(le: LE, tmr_va: int) -> bytes:
    """Install the `-custom` timer fix, stub at `tmr_va`. Returns the blob."""
    installer_va, custom_call_va, orig_target_va = find_timer_sites(le)
    blob = build_timer_stub(tmr_va, installer_va, orig_target_va)
    off = va_to_file(le, tmr_va)
    le.data[off : off + len(blob)] = blob

    # Re-point the existing call at the stub (same length, rel32 only).
    cs_off = va_to_file(le, custom_call_va)
    if le.data[cs_off] != 0xE8:
        raise ValueError(f"custom-branch site {custom_call_va:#x} is not an E8 call")
    # A relative call is never relocated, so this is belt-and-braces -- but the
    # MC2 arm learned the hard way what overwriting a fixed-up operand costs.
    assert_no_fixup(le, custom_call_va, 5, "MC1 -custom timer hook")
    le.data[cs_off + 1 : cs_off + 5] = struct.pack("<i", tmr_va - (custom_call_va + 5))
    return blob


def read_timer_init(le: LE):
    """(stub_va, installer_va, orig_target_va) read back THROUGH the installed
    stub, or None if this binary carries no timer install.

    It must not compare the custom-branch call against `find_timer_sites`'s
    `orig_target_va`: that is derived from the very rel32 the patch overwrites,
    so post-patch the two are equal by construction and the test always says
    "absent". Identify the stub by its SHAPE instead (`60 E8 .. 61 E9 ..`) and
    resolve both of its displacements -- which round-trips the patch rather
    than trusting either signature.
    """
    try:
        _, custom_call_va, _ = find_timer_sites(le)
        off = va_to_file(le, custom_call_va)
        stub_va = custom_call_va + 5 + struct.unpack_from("<i", le.data, off + 1)[0]
        s = va_to_file(le, stub_va)
        if not (le.data[s] == 0x60 and le.data[s + 1] == 0xE8
                and le.data[s + 6] == 0x61 and le.data[s + 7] == 0xE9):
            return None
        installer_va = (stub_va + 1) + 5 + struct.unpack_from("<i", le.data, s + 2)[0]
        orig_va = (stub_va + 7) + 5 + struct.unpack_from("<i", le.data, s + 8)[0]
    except (SystemExit, Exception):  # SystemExit is a BaseException -- name it
        return None
    return stub_va, installer_va, orig_va


def has_timer_init(le: LE) -> bool:
    """True if the custom-branch call has been re-pointed at our stub."""
    return read_timer_init(le) is not None


# --- MC2 / NETHERW twin -----------------------------------------------------
# `sub_32A70`, the ground-vortex controller, has the SAME two blind writes
# through `word_0x31` / `word_0x33`. Three things differ from MC1 and all three
# matter: the record is 168 B with class at +0x3F and model at +0x40 (MC1:
# 164 B, +0x40 / +0x41); the slot registers index a POINTER TABLE
# (`mov 0x1a3e4(,%eax,4),%eax`) rather than base + slot*stride; and the kick is
# a 32-bit `movl $0xfa,0x10(%eax)` (MC1's is a 16-bit `+26` store). The plume
# kill is `call <flags |= 0x400>`, and unlike MC1's the port's guard also
# rejects the NEW column itself -- which is live in `esi` at the call.
VOL2_KICK_SIG = bytes.fromhex("39d07607c74010fa000000")  # cmp %edx,%eax ; jbe +7 ; movl $0xfa,0x10(%eax)
VOL2_PLUME_SIG = bytes.fromhex("39c8760950e8")           # cmp %ecx,%eax ; jbe +9 ; push %eax ; call
VOL2_CLASS_OFF, VOL2_MODEL_OFF = 0x3F, 0x40


def find_volcano_sites_mc2(le: LE) -> tuple[int, int]:
    """(kick_store_va, plume_call_va) in NETHERW.EXE, by unique signature."""
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    blob = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    def one(sig: bytes, what: str) -> int:
        hits = []
        i = blob.find(sig)
        while i >= 0:
            hits.append(i)
            i = blob.find(sig, i + 1)
        if len(hits) != 1:
            raise ValueError(f"volcano(mc2) {what}: expected 1 signature hit, got {len(hits)}")
        return o1.vbase + hits[0]

    # Kick signature starts at the `cmp`; its store is 4 bytes in. Plume
    # signature starts at the `cmp`; its `call` is 5 bytes in.
    return one(VOL2_KICK_SIG, "kick") + 4, one(VOL2_PLUME_SIG, "plume") + 5


def build_volcano_stubs_mc2(vol_va: int, killer_va: int) -> tuple[bytes, int, int]:
    """(blob, kick_stub_va, plume_stub_va) for NETHERW's cave at `vol_va`."""
    # stub 1 -- eax = the register's record, ebx = the driver's own record.
    k = bytearray()
    k += b"\x9c"                                            # pushfd
    k += b"\x39\xd8"                                        # cmp %ebx,%eax
    k += b"\x74\x13"                                        # je  skip
    k += bytes([0x80, 0x78, VOL2_CLASS_OFF, VOL_CLASS])     # cmpb $10,0x3f(%eax)
    k += b"\x75\x0d"                                        # jne skip
    k += bytes([0x80, 0x78, VOL2_MODEL_OFF, VOL_MODEL_VOLCANO])  # cmpb $18,0x40(%eax)
    k += b"\x75\x07"                                        # jne skip
    k += b"\xc7\x40\x10\xfa\x00\x00\x00"                    # movl $0xfa,0x10(%eax)
    k += b"\x9d\xc3"                                        # skip: popfd ; ret
    assert len(k) == 26, len(k)

    # stub 2 -- arg at [esp+4] (already pushed); esi = the NEW column, which
    # `call 0x7c710` (cdecl) preserves, so the self-test is free here.
    plume_va = vol_va + len(k)
    p = bytearray()
    p += b"\x8b\x44\x24\x04"                                # mov 0x4(%esp),%eax
    p += b"\x39\xf0"                                        # cmp %esi,%eax  (old == col?)
    p += b"\x74\x11"                                        # je  skip
    p += bytes([0x80, 0x78, VOL2_CLASS_OFF, VOL_CLASS])     # cmpb $10,0x3f(%eax)
    p += b"\x75\x0b"                                        # jne skip
    p += bytes([0x80, 0x78, VOL2_MODEL_OFF, VOL_MODEL_PLUME])    # cmpb $19,0x40(%eax)
    p += b"\x75\x05"                                        # jne skip
    jmp_va = plume_va + len(p)
    p += b"\xe9" + struct.pack("<i", killer_va - (jmp_va + 5))   # jmp <killer>
    p += b"\xc3"                                            # skip: ret
    assert len(p) == 26, len(p)
    return bytes(k + p), vol_va, plume_va


def patch_volcano_mc2(le: LE, vol_va: int) -> bytes:
    kick_va, plume_call_va = find_volcano_sites_mc2(le)
    pc_off = va_to_file(le, plume_call_va)
    if le.data[pc_off] != 0xE8:
        raise ValueError(f"volcano(mc2) plume site {plume_call_va:#x} is not an E8 call")
    killer_va = plume_call_va + 5 + struct.unpack_from("<i", le.data, pc_off + 1)[0]

    blob, kick_stub_va, plume_stub_va = build_volcano_stubs_mc2(vol_va, killer_va)
    off = va_to_file(le, vol_va)
    le.data[off : off + len(blob)] = blob

    # The 7-byte store becomes `call <stub>` + two nops; the `jbe` above still
    # jumps +7 onto the same next instruction.
    ks_off = va_to_file(le, kick_va)
    if bytes(le.data[ks_off : ks_off + 7]) != bytes.fromhex("c74010fa000000"):
        raise ValueError(f"volcano(mc2) kick site {kick_va:#x} is not the expected store")
    le.data[ks_off : ks_off + 5] = b"\xe8" + struct.pack("<i", kick_stub_va - (kick_va + 5))
    le.data[ks_off + 5 : ks_off + 7] = b"\x90\x90"

    le.data[pc_off + 1 : pc_off + 5] = struct.pack("<i", plume_stub_va - (plume_call_va + 5))
    return blob


def has_volcano_guard_mc2(le: LE) -> bool:
    try:
        kick_va, _ = find_volcano_sites_mc2(le)
    except ValueError:
        # Signature gone == already patched (the store is a call now).
        try:
            off = va_to_file(le, 0x32B79)
        except Exception:
            return False
        return le.data[off] == 0xE8 and le.data[off + 5 : off + 7] == b"\x90\x90"
    return False


def verify_volcano_mc2(le: LE) -> str:
    o1 = le.objs[0]
    kick_va, plume_va = 0x32B79, 0x32BDE
    ko = va_to_file(le, kick_va)
    if le.data[ko] != 0xE8 or le.data[ko + 5 : ko + 7] != b"\x90\x90":
        raise ValueError("volcano(mc2): kick site is not `call rel32` + 2 nops")
    kick_stub = kick_va + 5 + struct.unpack_from("<i", le.data, ko + 1)[0]
    po = va_to_file(le, plume_va)
    if le.data[po] != 0xE8:
        raise ValueError("volcano(mc2): plume site is not an E8 call")
    plume_stub = plume_va + 5 + struct.unpack_from("<i", le.data, po + 1)[0]
    lo, hi = o1.vbase, o1.vbase + o1.npages * 0x1000
    for name, va in (("kick", kick_stub), ("plume", plume_stub)):
        if not (lo <= va < hi):
            raise ValueError(f"volcano(mc2): {name} stub {va:#x} outside obj1")
    js = va_to_file(le, plume_stub) + 20
    if le.data[js] != 0xE9:
        raise ValueError("volcano(mc2): plume stub does not end in a tail jmp")
    killer = plume_stub + 20 + 5 + struct.unpack_from("<i", le.data, js + 1)[0]
    want, _, want_plume = build_volcano_stubs_mc2(kick_stub, killer)
    if want_plume != plume_stub:
        raise ValueError("volcano(mc2): stub layout disagrees")
    got = bytes(le.data[va_to_file(le, kick_stub) : va_to_file(le, kick_stub) + len(want)])
    if got != want:
        raise ValueError("volcano(mc2): stub bytes differ from the canonical build")
    return (f"volcano guard (mc2): kick {kick_va:#x} -> {kick_stub:#x}, "
            f"plume {plume_va:#x} -> {plume_stub:#x} -> killer {killer:#x}; "
            f"{len(want)} B, canonical")


def verify_volcano(le: LE) -> str:
    """Re-derive the guard from the patched image and check it end to end.

    Returns a one-line report. Raises if anything does not add up. The kick
    SIGNATURE is gone by construction once patched (the store became a call),
    which is also what stops the patch being applied twice.
    """
    o1 = le.objs[0]
    kick_va, plume_va = 0x25FBE, 0x26035  # same VAs in both MC1 builds
    lo, hi = o1.vbase, o1.vbase + o1.npages * 0x1000

    ko = va_to_file(le, kick_va)
    if le.data[ko] != 0xE8 or le.data[ko + 5] != 0x90:
        raise ValueError("volcano: kick site is not `call rel32` + nop")
    kick_stub = kick_va + 5 + struct.unpack_from("<i", le.data, ko + 1)[0]

    po = va_to_file(le, plume_va)
    if le.data[po] != 0xE8:
        raise ValueError("volcano: plume site is not an E8 call")
    plume_stub = plume_va + 5 + struct.unpack_from("<i", le.data, po + 1)[0]

    for name, va in (("kick", kick_stub), ("plume", plume_stub)):
        if not (lo <= va < hi):
            raise ValueError(f"volcano: {name} stub {va:#x} is outside obj1")
        if va < o1.vbase + o1.vsize - 0x1000:
            raise ValueError(f"volcano: {name} stub {va:#x} is not in the cave tail")

    # The plume stub must tail-jump to a real routine, and the blob must be
    # byte-identical to what this build would emit for that target.
    js = va_to_file(le, plume_stub) + 16
    if le.data[js] != 0xE9:
        raise ValueError("volcano: plume stub does not end in a tail jmp")
    killer = plume_stub + 16 + 5 + struct.unpack_from("<i", le.data, js + 1)[0]
    want, _, want_plume = build_volcano_stubs(kick_stub, killer)
    if want_plume != plume_stub:
        raise ValueError("volcano: stub layout disagrees (kick/plume not adjacent)")
    got = bytes(le.data[va_to_file(le, kick_stub) : va_to_file(le, kick_stub) + len(want)])
    if got != want:
        raise ValueError("volcano: stub bytes differ from the canonical build")
    return (f"volcano guard: kick {kick_va:#x} -> {kick_stub:#x}, "
            f"plume {plume_va:#x} -> {plume_stub:#x} -> killer {killer:#x}; "
            f"{len(want)} B, canonical")


def has_volcano_guard(le: LE) -> bool:
    """True if the kick store has been replaced by a call (i.e. patched)."""
    try:
        off = va_to_file(le, 0x25FBE)
    except Exception:
        return False
    return le.data[off] == 0xE8 and le.data[off + 5] == 0x90


def verify(path: str, period: int, inert: bool = False, passthrough: bool = False) -> None:
    import shutil

    from collections import Counter

    data = open(path, "rb").read()
    le = parse_le(data)
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = data[code_off : code_off + o1.npages * 0x1000]

    # obj1's vsize is page-aligned by the patch so the cave is in-limit; locate
    # the stub independently of vsize (it is NOT at vbase+vsize any more).
    redirected = 0
    if inert:  # no redirects -- find the full stub's distinctive preamble
        idx = code.find(b"\xe8\x00\x00\x00\x00\x5a\x81\xea")
        if idx < 0:
            raise SystemExit("VERIFY FAIL: stub preamble not found in obj1")
        cave_va = o1.vbase + idx
    else:  # the redirected calls all target the stub -- that is cave_va
        cnt = Counter()
        for i in range(len(code) - 5):
            if code[i] == 0xE8:
                t = o1.vbase + i + 5 + struct.unpack_from("<i", code, i + 1)[0]
                cnt[t] += 1
        cave_va = next((t for t in sorted(cnt, reverse=True)
                        if cnt[t] >= 3 and code[t - o1.vbase] == 0xE8), None)
        if cave_va is None:
            raise SystemExit("VERIFY FAIL: no 3-way redirected call target (stub)")
        redirected = cnt[cave_va]

    # From cave_va, the stub's `call <hook> ; ret` is the first `E8 rel32 C3`
    # (the call/pop preamble is `E8 00000000 5A`, whose +5 byte is 5A not C3).
    rel = cave_va - o1.vbase
    end_j = next((j for j in range(0, 400)
                  if code[rel + j] == 0xE8 and code[rel + j + 5] == 0xC3), None)
    if end_j is None:
        raise SystemExit("VERIFY FAIL: no `call hook ; ret` in the stub")
    hook_va = cave_va + end_j + 5 + struct.unpack_from("<i", code, rel + end_j + 1)[0]
    stub_len = end_j + 6
    aligned = "page-aligned" if o1.vsize % 0x1000 == 0 else f"NOT page-aligned ({o1.vsize:#x})"

    # Read the floor back OUT of the patched image rather than trusting the
    # argument: `add eax,imm32 ; sub eax,[edx+MB_DEADLINE]` is the clamp's
    # signature and occurs nowhere else (the spin's own `sub eax,[deadline]` is
    # preceded by a disp32 tail byte, never by an `05` opcode).
    fm = _re.search(
        rb"\x05(....)\x2b\x82" + _re.escape(struct.pack("<I", MB_DEADLINE)),
        code[rel : rel + stub_len],
        _re.S,
    )
    floor_val = struct.unpack("<I", fm.group(1))[0] if fm else 0
    fl = (f"floor {floor_val} counts (>={(floor_val - 1) * 1000 / 120:.1f} ms window)"
          if floor_val else "floor OFF")

    if inert:
        print(f"VERIFY {path}: OK (INERT)")
        print(f"  stub present @ {cave_va:#x} ({stub_len} bytes) but NO call site "
              f"targets it -- never executed; obj1.vsize {aligned}; {fl}")
        return

    print(f"VERIFY {path}: OK")
    print(f"  {redirected} call site(s) -> stub @ {cave_va:#x}; stub -> original "
          f"tick fn @ {hook_va:#x}; {stub_len} bytes; obj1.vsize {aligned}; "
          f"entry untouched; {fl}")
    if has_volcano_guard(le):
        print(f"  {verify_volcano(le)}")
        print("  \u26a0 SIMULATION IS PATCHED -- recordings from this binary "
              "witness the PATCHED arm, never retail.")

    print(f"  {read_mc1_frame_park(le, code)}")

    # The `-custom` timer install. Report it either way: its ABSENCE is the
    # thing that cost a session to diagnose (a `-custom` launch pacing against
    # a clock nothing starts runs at under 1 fps), so say so out loud.
    tmr = read_timer_init(le)
    if tmr:
        stub_va, installer_va, orig_va = tmr
        _, custom_call_va, _ = find_timer_sites(le)
        print(f"  -custom timer install: call @ {custom_call_va:#x} -> stub @ "
              f"{stub_va:#x} -> installer @ {installer_va:#x}, tail-jmp @ "
              f"{orig_va:#x}; unreachable unless launched "
              f"-custom/-network/-demo/-roll, so menu-path captures are unaffected")
    else:
        print("  \u26a0 NO -custom timer install: a -custom/-network/-demo/-roll "
              "launch will run at <1 fps (nothing starts the clock the pacer "
              "spins on). Re-patch without --no-timer-init.")
    if shutil.which("ndisasm"):
        import subprocess
        import tempfile

        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as tf:
            tf.write(code[rel : rel + stub_len])
            tmp = tf.name
        out = subprocess.run(
            ["ndisasm", "-b", "32", "-o", hex(cave_va), tmp], capture_output=True, text=True
        ).stdout
        print("  --- stub disassembly ---")
        for ln in out.strip().splitlines():
            print("   ", ln)


def read_mc1_frame_park(le: LE, code: bytes) -> str:
    """Describe the MC1 init park off the patched image: GameLoop's frame
    call either targets a `pushad ; call $+5 ; pop edx` stub that ends in a
    tail `jmp` (decode its timeout and the jmp target), or is pristine."""
    o1 = le.objs[0]
    hits = list(MC1_FRAMECALL_SIG.finditer(code))
    if len(hits) != 1:
        return f"⚠ init park: frame-call signature {len(hits)} hits (!)"
    site = o1.vbase + hits[0].start() + 12
    tgt = (site + 5 + struct.unpack("<i", hits[0].group(1))[0]) & 0xFFFFFFFF
    s = tgt - o1.vbase
    if code[s : s + 7] != b"\x60\xe8\x00\x00\x00\x00\x5a":
        return _park_note(None, 120.0)
    e = code.find(b"\x61\xe9", s)  # popad ; jmp rel32
    if e < 0 or e - s > 400:
        raise SystemExit("VERIFY FAIL: init-park stub has no `popad ; jmp`")
    fn = o1.vbase + e + 6 + struct.unpack_from("<i", code, e + 2)[0]
    t = read_init_park(code[s:e], MB_PARK_ARM, MB_PARK)
    if t is None:
        raise SystemExit("VERIFY FAIL: init-park stub body unrecognised")
    return (f"{_park_note(t, 120.0)}; GameLoop call @ {site:#x} -> park stub @ "
            f"{tgt:#x} -> tail-jmp frame driver @ {fn:#x}")


# --------------------------------------------------------------------------
# MC2 / NETHERW.EXE: locate the frame-limiter hook, build the signal stub,
# patch, verify. Signal-only (no pacer), so the stub is tiny and the MC1 path
# stays untouched.
# --------------------------------------------------------------------------
@dataclass
class BuildMC2:
    name: str
    call_site: int  # VA of `call DrawAndEventsInGame_47560` we redirect
    frame_fn: int  # VA of the frame driver (the stub calls the original)
    cave_va: int
    obj3ref_va: int  # VA of a fixed-up obj3 disp32 (the GameTimerTurn ref) whose
    #                  runtime value = obj3_base + obj3ref_off -- read to recover
    #                  obj3's real load base, exactly as the MC1 stub does.
    obj3ref_off: int  # obj3-relative offset that disp holds
    period_va: int  # VA of the `add esi,N` immediate byte (the native frame
    #                 period in 100 Hz ticks; default 5 = ~20 fps). Widening it
    #                 with --pace guarantees a wide capture window on heavy
    #                 frames whose compute would otherwise eat the native spin.
    period_now: int  # the current period byte (5 on a pristine NETHERW)


def find_build_mc2(le: LE) -> BuildMC2:
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    # Double-patch guard: the limiter signature wildcards the call rel32, so it
    # still matches an already-patched exe -- but the stub's preamble is unique
    # and only present once we've patched. Refuse cleanly.
    if b"\xe8\x00\x00\x00\x00\x5a\x81\xea" in code:
        raise SystemExit("already patched (stub preamble present) -- refusing")

    hits = list(MC2_LIMITER_SIG.finditer(code))
    if len(hits) == 0:
        raise SystemExit(
            "MC2 frame-limiter signature not found (0 hits). Not a pristine "
            "NETHERW.EXE -- already patched, or an unexpected build."
        )
    if len(hits) != 1:
        raise SystemExit(f"expected exactly 1 MC2 limiter signature, found {len(hits)}")
    m = hits[0]
    ref1 = struct.unpack("<I", m.group(1))[0]
    ref2 = struct.unpack("<I", m.group(3))[0]
    if ref1 != ref2:
        raise SystemExit("MC2 limiter: the two GameTimerTurn disps differ")
    # Layout inside the match: mov esi,[disp](6) | call rel32(5) |
    #                          add esi,imm8(3: 83 c6 NN) | cmp(6) | ja(2).
    call_site = o1.vbase + m.start() + 6
    rel = struct.unpack("<I", m.group(2))[0]
    frame_fn = (call_site + 5 + rel) & 0xFFFFFFFF
    obj3ref_va = o1.vbase + m.start() + 2  # the disp32 field of `mov esi,[..]`
    obj3ref_off = ref1
    period_va = o1.vbase + m.start() + 13  # the imm8 of `add esi,N`
    period_now = code[m.start() + 13]

    # obj3 must hold GameTimerTurn (the disp is an obj3-relative offset).
    obj3 = le.objs[2]
    if obj3.vbase != OBJ3_BASE_MC2:
        raise ValueError(f"obj3 vbase {obj3.vbase:#x} != {OBJ3_BASE_MC2:#x}")
    if not (0 <= obj3ref_off < obj3.vsize):
        raise ValueError(f"GameTimerTurn obj3-off {obj3ref_off:#x} outside obj3")

    # Cave = obj1's zero tail past vsize (in-limit after patch_mc2 page-aligns).
    cave_va = o1.vbase + o1.vsize
    cave_off = code_off + o1.vsize
    cave_end = code_off + o1.npages * 0x1000
    if any(le.data[cave_off:cave_end]):
        raise ValueError("obj1 tail cave is not zero-filled")

    # Mailbox in obj3's committed BSS tail (page-align lifts the DS limit over
    # it). MB2_OBJ3 sits at obj3.vsize, so page-aligning vsize covers it.
    committed = (obj3.vsize + 0xFFF) & ~0xFFF
    if not (obj3.vsize <= MB2_OBJ3 and MB2_END <= committed):
        raise ValueError(
            f"mailbox obj3-off {MB2_OBJ3:#x} not in obj3 tail "
            f"[vsize {obj3.vsize:#x}, page {committed:#x})"
        )
    return BuildMC2("NETHERW", call_site, frame_fn, cave_va, obj3ref_va,
                    obj3ref_off, period_va, period_now)


def build_stub_mc2(b: BuildMC2, floor: int = FLOOR_DEFAULT,
                   init_park: int = INIT_PARK_DEFAULT) -> bytes:
    """Signal-only wrapper. On each frame:
      1. derive obj3's real runtime base (read the game's own fixed-up
         GameTimerTurn disp, minus its obj3 offset -- delta-safe like MC1);
      1b. once per arming (the first frame of a fresh process, or whenever
         the host re-arms), THE INIT PARK: hold the world as
         LevelInitGame left it until the host releases it (see PARK_*).
         Nothing in the game runs between LevelInitGame's tail and this
         call except idempotent overwrites (LoadSpr's paths, the music
         index from MapType, InGameLoop's paletteMod_51 = 0 and
         dw_w_b_0_2BDE_11230.word[1] = 0), so this IS remc2's
         RecordingLevelSave instant for the save's purposes;
      2. clear in_window (the frame driver is about to mutate the world);
      3. call the ORIGINAL frame driver (Turn++, entity pass, draw);
      4. bump a monotonic per-frame counter and raise in_window;
      5. hold that window open for at least `floor` timer counts.
    in_window is therefore up from just after the draw, across MC2's native
    limiter spin, until the next frame's mutation -- a settled window keyed by
    the counter. Step 5 is what makes the width load-independent: the native
    limiter's budget is absolute (`turn_before_frame + N`), so a frame whose
    compute eats it leaves no spin at all, and the floor is then the entire
    window. It sits in the TAIL, after the counter bump, so a window the
    recorder sees announced as fresh is the same one being held open.
    Touches only eax/edx (caller-clobber; esi=turn and ebx=loop counter, which
    InGameLoop reads after the call, are preserved) plus ecx, which the floor
    spin's guard borrows under a push/pop so it survives too; the frame driver
    saves/restores its own callee-saved regs."""
    a = Asm(b.cave_va)
    # --- derive obj3 base into edx ---
    a.call_next()  # push EIP of pop
    a.pop_edx()  # edx = runtime(pop)
    a.sub_edx_imm(b.cave_va + 5)  # edx = obj1 load delta (link of pop = cave+5)
    a.mov_eax_m(b.obj3ref_va)  # eax = [delta + refVA] = obj3_base + obj3ref_off
    a.sub_eax_imm(b.obj3ref_off)  # eax = obj3_base (runtime)
    a.mov_edx_eax()  # edx = obj3_base for all mailbox refs

    # --- one-time init (magic-gated; robust to a non-zero tail) ---
    a.mov_eax_m(MB2_MAGIC0)
    a.cmp_eax_imm(MAGIC0)
    a.br8(0x74, "after_init")  # je after_init
    a.mov_m_imm(MB2_MAGIC1, MAGIC2_1)
    a.mov_m_imm(MB2_TICK, 0)
    a.mov_m_imm(MB2_INWIN, 0)
    a.mov_m_imm(MB2_MAGIC0, MAGIC0)  # magic LAST -> mailbox is atomic-ish
    a.label("after_init")

    # --- the init park: before frame 1 touches anything ---
    emit_init_park(a, MB2_PARK_ARM, MB2_PARK, MB2_PARK_MAGIC, b.obj3ref_off,
                   init_park)

    # --- close the window: the frame is about to mutate the world ---
    a.mov_m_imm(MB2_INWIN, 0)
    head = a.assemble()

    # --- call the ORIGINAL frame driver, preserving obj3 base across it ---
    # push edx ; call frame_fn ; pop edx. The original call site pushed no
    # argument (turn is passed in esi), so the extra push is invisible to the
    # callee (it reads no stack arg) and the stack stays balanced.
    push_pos = len(head)  # `push edx` (1 byte) then `call` (5 bytes)
    call_pos = push_pos + 1
    rel = b.frame_fn - (b.cave_va + call_pos + 5)
    mid = b"\x52" + b"\xe8" + struct.pack("<i", rel) + b"\x5a"  # push edx;call;pop edx

    # --- open the window: frame settled; native limiter spin follows ---
    t = Asm(b.cave_va + len(head) + len(mid))
    t.inc_m(MB2_TICK)
    t.mov_m_imm(MB2_INWIN, 1)

    # --- floor: hold the window open at least `floor` timer counts -----------
    # Same counter the native limiter spins on, so this composes with it rather
    # than replacing it: the total window is floor + max(0, native spin). ECX is
    # borrowed for the frozen-timer guard and restored, because InGameLoop's
    # live registers across the call are not fully known -- the stub's standing
    # rule is to hand back everything but eax/edx.
    if floor:
        t.mov_eax_m(b.obj3ref_off)  # eax = GameTimerTurn (now)
        t.add_eax_imm(floor)  # eax = release target
        t.push_ecx()
        t.mov_ecx_imm(GUARD_ITERS)
        t.label("fspin")
        t.cmp_eax_m(b.obj3ref_off)  # target vs now
        t.br8(0x7E, "fdone")  # jle fdone  (target reached -> release)
        t.dec_ecx()
        t.br8(0x75, "fspin")  # jnz fspin  (keep waiting)
        t.label("fdone")  # guard expired (ISR masked) falls through here too
        t.pop_ecx()
    t.raw(b"\xc3")  # ret
    return head + mid + t.assemble()


def patch_mc2(le: LE, b: BuildMC2, wire: bool = True, extend: bool = True,
              pace: Optional[int] = None, floor: int = FLOOR_DEFAULT,
              volcano: bool = False, headless: bool = True,
              init_park: int = INIT_PARK_DEFAULT) -> bytes:
    o1 = le.objs[0]

    # Optional: widen the native frame period so a heavy frame's compute can't
    # eat the whole limiter spin (the capture window). One byte -- the imm8 of
    # `add esi,N`. Purely a real-time pacing change: the sim still runs one
    # PlayerEvents (Turn++) + one entity pass per frame, so the recorded frame
    # sequence is byte-identical, just held longer. N in 1..127 (higher = lower
    # fps, wider window). This is the definitive zero-gap fix for graphics-heavy
    # levels; without it, signal-only relies on compute fitting the ~50 ms budget.
    if pace is not None:
        if not (1 <= pace <= 127):
            raise ValueError("--pace must be in 1..127 (imm8 frame period)")
        poff = va_to_file(le, b.period_va)
        if le.data[poff - 2 : poff] != b"\x83\xc6":  # `add esi,` guard
            raise ValueError(f"period byte @ {b.period_va:#x} is not an `add esi,imm8`")
        le.data[poff] = pace

    stub = build_stub_mc2(b, floor, init_park)
    cave_off = va_to_file(le, b.cave_va)
    if cave_off + len(stub) > obj_file_off(le, o1) + o1.npages * 0x1000:
        raise ValueError("stub overflows the cave")
    le.data[cave_off : cave_off + len(stub)] = stub

    # The volcano write guard shares the cave, 16-aligned after the signal stub.
    vol_end = b.cave_va + len(stub)
    if volcano:
        vol_va = (b.cave_va + len(stub) + 15) & ~15
        vol_blob = patch_volcano_mc2(le, vol_va)
        vol_end = vol_va + len(vol_blob)
        if va_to_file(le, vol_end) > obj_file_off(le, o1) + o1.npages * 0x1000:
            raise ValueError("volcano stubs overflow the cave")

    # The headless `-level` fix shares the cave too, 16-aligned after whatever
    # precedes it. Applied even under --inert: --inert is about not wiring the
    # SIGNAL stub, and this one's site never executes unless `-level` is given.
    if headless:
        hl_va = (vol_end + 15) & ~15
        hl_blob = patch_mc2_headless(le, b, hl_va)
        vol_end = hl_va + len(hl_blob)
        if va_to_file(le, vol_end) > obj_file_off(le, o1) + o1.npages * 0x1000:
            raise ValueError("headless -level stub overflows the cave")

    # Page-align obj1.vsize (so the code cave is inside the CS limit and will
    # execute) and obj3.vsize (so the mailbox is inside the DS limit and its
    # writes persist) -- the same two lifts the MC1 arm needs.
    if extend:
        objtab = struct.unpack_from("<I", le.data, le.lx + 0x40)[0]
        new1 = (o1.vsize + 0xFFF) & ~0xFFF
        if vol_end > o1.vbase + new1:
            raise ValueError("stub crosses the page boundary; extend by another page")
        struct.pack_into("<I", le.data, le.lx + objtab + 0 * 24, new1)
        o1.vsize = new1

        o3 = le.objs[2]
        new3 = (o3.vsize + 0xFFF) & ~0xFFF
        if MB2_END > new3:
            raise ValueError("mailbox past obj3's page-aligned vsize")
        struct.pack_into("<I", le.data, le.lx + objtab + 2 * 24, new3)
        o3.vsize = new3

    if not wire:
        return stub  # --inert

    off = va_to_file(le, b.call_site)
    if le.data[off] != 0xE8:
        raise ValueError(f"call site {b.call_site:#x} is not an E8 call")
    rel = b.cave_va - (b.call_site + 5)
    le.data[off + 1 : off + 5] = struct.pack("<i", rel)
    return stub


# --------------------------------------------------------------------------
# The MC2 headless `-level` launch
# --------------------------------------------------------------------------
# `-level N` does NOT boot straight in on a pristine NETHERW: the game always
# lands in the main menu, and the switch is only honoured on the SECOND pass
# through it (player-observed 2026-09-23; re-derived from the shipped bytes
# after an earlier dig got this wrong). The gate, at VA 0x77063:
#
#     mov   dl,[E29E1]        ; MenusAndIntros.cpp:463  `char x_BYTE_E29E1 = 1;`
#     test  dl,dl
#     jne   <draw the main menu>          ; <-- FIRST PASS ALWAYS TAKES THIS
#     mov   eax,[..] ; testb $0x10,0x16(%eax)   ; MULTIPLAYER_MODE
#     jne   <draw the main menu>
#     push  0 ; call NewGameDialog_77350  ; <-- the only reader of -level
#     mov   di,[m_ExitMenuLoop] ; test di,di ; je <draw the main menu>
#
# In C that is `if (E29E1 || MULTIPLAYER_MODE || (NewGameDialog(0), !ExitMenuLoop))`
# -- and `||` SHORT-CIRCUITS, so while E29E1 is 1 the dialog is never called at
# all. E29E1 is cleared only at the END of the menu loop (:649-650), which is
# exactly why entering the campaign once makes `-level` work thereafter. It is
# not in config.dat, so it is back to 1 on every launch: there is no
# configuration route to a headless boot.
#
# The fix is one byte, written only when `-level` was actually given: clear
# E29E1 in the parser's `-level` handler, so the first menu pass falls through
# to NewGameDialog. A launch WITHOUT `-level` never executes the patched
# instruction, so the menu behaves exactly as shipped.
#
# NOT fixed (player-ruled 2026-09-23, "the whole dosbox can be killed"): once
# in the level there is no way back out -- `LoadLevelNumber_D419C` has four
# references in the whole image (init to -1, the parser write, two reads in
# NewGameDialog) and is NEVER cleared after use, so every return to the dialog
# relaunches the same level forever.
#
# Anchors, both verified unique in NETHERW.EXE:
#   handler: `mov $1,%cl ; add $4,%esp ; mov %al,0x42(%ebp) ; mov %al,[D419C]
#             ; mov %cl,0x72(%ebp)` -- the store is the 5-byte hook @ VA 0x5630C.
#   gate:    the whole NewGameDialog guard above; its `call` target is checked
#            against NewGameDialog, so the match verifies itself.
# ⚠⚠ WHICH INSTRUCTION WE OVERWRITE IS LOAD-BEARING. The first cut hooked the
# 5-byte `mov %al,[D419C]` store -- and DOSBox died at launch with
# "Illegal read from 1000000 / DYNX86:Can't run code in this page". That store's
# operand is an LE INTERNAL FIXUP (page 70, page-off 0x30D -> obj 3 + 0x419C),
# so the loader writes `obj3_base + 0x419C` straight over the `call`'s rel32
# before a single instruction runs. The tree already knew the outbound half of
# this rule -- "injected code gets no LE fixups", hence the obj3-base
# derivation -- and this is its mirror image: **code we overwrite may still HAVE
# one.** `assert_no_fixup` below now enforces it at every overwrite site.
#
# So we hook the handler's FINAL instruction instead, the `jmp` back to the
# parse loop (VA 0x56314): relative, never fixed up, and a jmp->jmp tail chain
# needs no stack handling at all. The `mov %al,[D419C]` store is left pristine,
# fixup and all -- retail still writes LoadLevelNumber itself.
MC2_LEVEL_SIG = _re.compile(
    rb"\xb1\x01\x83\xc4\x04\x88\x45\x42\xa2(....)\x88\x4d\x72\xe9....", _re.S
)
MC2_LEVEL_LL_OFF = 9  # match start -> the moffs32 naming LoadLevelNumber
# ⚠⚠⚠ A SIGNATURE MUST NOT PIN THE BYTES ITS OWN PATCH REWRITES. Three separate
# detectors in this file got that wrong before it stuck: the MC1 timer one
# compared against a target the patch had already moved, the first MC2 one
# pinned the `\xa2` opcode it turned into `\xe8`, and this one pinned the whole
# `add esp,12 ; test dl,dl ; jne` window it replaces -- each time reporting a
# perfectly good patch as absent. The window below is therefore a WILDCARD
# group that callers decode: `\x83...` pristine, `\xe9...` once hooked.
MC2_MENUGATE_SIG = _re.compile(
    rb"\x8a\x15(....)"                 # +0  mov dl,[E29E1]   (obj3-rel disp32)
    rb"(.......)"                      # +6  THE HOOK WINDOW -- never pinned
    rb"\xa1....\xf6\x40\x16\x10\x75."  # +13 mov eax,[..] ; testb $0x10,.. ; jne
    rb"\x6a\x00\xe8(....)",            # +24 push 0 ; call NewGameDialog
    _re.S,
)
MC2_GATE_HOOK_OFF = 6   # match start -> `add esp,12`, the 7-byte hook window
MC2_GATE_WINDOW = 7     # add esp,12 (3) + test dl,dl (2) + jne rel8 (2)
MC2_GATE_PRISTINE = b"\x83\xc4\x0c\x84\xd2\x75"  # the window minus its rel8


def find_mc2_headless_sites(le: LE) -> tuple:
    """(hook_va, first_byte, loadlevel_off, menu_va, fallthrough_va)."""
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = bytes(le.data[code_off : code_off + o1.npages * 0x1000])

    hits = list(MC2_LEVEL_SIG.finditer(code))
    if len(hits) != 1:
        raise SystemExit(
            f"MC2 -level handler signature: expected 1 hit, found {len(hits)}"
        )
    loadlevel_off = struct.unpack("<I", hits[0].group(1))[0]

    hits = list(MC2_MENUGATE_SIG.finditer(code))
    if len(hits) != 1:
        raise SystemExit(
            f"MC2 menu-gate signature: expected 1 hit, found {len(hits)}"
        )
    m = hits[0]
    gate_off = struct.unpack("<I", m.group(1))[0]
    window = m.group(2)
    hook_va = o1.vbase + m.start() + MC2_GATE_HOOK_OFF
    fallthrough_va = hook_va + MC2_GATE_WINDOW
    # Only a PRISTINE window still carries the jne rel8; on a patched image the
    # menu target is recovered from the stub instead (see read_mc2_headless).
    menu_va = (fallthrough_va + struct.unpack("<b", window[6:7])[0]
               if window.startswith(MC2_GATE_PRISTINE) else None)
    # Self-check: the guarded call must really be NewGameDialog.
    call_va = o1.vbase + m.end() - 5
    tgt = call_va + 5 + struct.unpack("<i", m.group(3))[0]
    if not (0 <= gate_off < le.objs[2].vsize):
        raise ValueError(f"menu gate obj3-off {gate_off:#x} outside obj3")
    for name, v in (("guarded call", tgt), ("menu", menu_va)):
        if v is not None and not (o1.vbase <= v < o1.vbase + o1.npages * 0x1000):
            raise ValueError(f"menu gate: {name} target {v:#x} outside obj1")
    return hook_va, window, loadlevel_off, menu_va, fallthrough_va


def build_mc2_headless_stub(b: BuildMC2, va: int, loadlevel_off: int,
                            menu_va: int, fallthrough_va: int) -> bytes:
    """Replaces the gate's `add esp,12 ; test dl,dl ; jne <menu>`.

    DL arrives holding `x_BYTE_E29E1`, freshly loaded by the instruction just
    above (which we must NOT touch -- its operand carries an LE fixup). We keep
    that value, and only when `-level` was actually given (LoadLevelNumber, a
    SIGNED byte, is >= 0) do we substitute 0 so control falls through to
    NewGameDialog.

    ⚠ The first cut cleared E29E1 itself, back in the command-line parser. That
    hung the game on `-skipscreens`: E29E1 has a SECOND reader,
    `PlayInGameFmv_82670` (MenusAndIntros.cpp:4073), whose `if (!E29E1)` branch
    then ran on a first boot it was never meant to -- black screen. Deciding at
    the gate leaves that reader, and the variable, completely untouched.
    """
    a = Asm(va)
    a.raw(b"\x83\xc4\x0c")  # add esp,12        (the displaced instruction)
    a.raw(b"\x50")  # push eax
    a.raw(b"\x52")  # push edx                  (parks DL = the gate value)
    a.call_next()  # push EIP of the pop
    a.pop_edx()
    a.sub_edx_imm(va + 10)  # edx = obj1 load delta (link of pop = va+3+1+1+5)
    a.mov_eax_m(b.obj3ref_va)  # eax = obj3_base + obj3ref_off
    a.sub_eax_imm(b.obj3ref_off)  # eax = obj3_base
    a.mov_edx_eax()  # edx = obj3_base
    a.raw(b"\x80\xba" + struct.pack("<I", loadlevel_off) + b"\x00")  # cmpb $0,[edx+D419C]
    a.raw(b"\x5a")  # pop edx   -- POP does not disturb the flags
    a.raw(b"\x58")  # pop eax
    a.raw(b"\x7c\x02")  # jl +2   (LoadLevelNumber < 0 -> no -level, honour DL)
    a.raw(b"\x30\xd2")  # xor dl,dl  (-level given -> force the fall-through)
    a.raw(b"\x84\xd2")  # test dl,dl
    body = a.assemble()
    jne_va = va + len(body)
    body += b"\x0f\x85" + struct.pack("<i", menu_va - (jne_va + 6))  # jne <menu>
    jmp_va = va + len(body)
    body += b"\xe9" + struct.pack("<i", fallthrough_va - (jmp_va + 5))
    return body


def patch_mc2_headless(le: LE, b: BuildMC2, va: int) -> bytes:
    """Install the headless `-level` fix, stub at `va`. Returns the blob."""
    hook_va, window, loadlevel_off, menu_va, fallthrough_va = find_mc2_headless_sites(le)
    if not window.startswith(MC2_GATE_PRISTINE):
        raise ValueError(
            f"gate hook {hook_va:#x} window is {window.hex()}, not the pristine "
            f"`add esp,12 ; test dl,dl ; jne` -- already patched?")
    # The crash that taught us this: never overwrite a relocated operand.
    assert_no_fixup(le, hook_va, MC2_GATE_WINDOW, "MC2 headless -level hook")

    blob = build_mc2_headless_stub(b, va, loadlevel_off, menu_va, fallthrough_va)
    off = va_to_file(le, va)
    le.data[off : off + len(blob)] = blob

    hoff = va_to_file(le, hook_va)
    le.data[hoff] = 0xE9
    le.data[hoff + 1 : hoff + 5] = struct.pack("<i", va - (hook_va + 5))
    le.data[hoff + 5 : hoff + MC2_GATE_WINDOW] = b"\x90" * (MC2_GATE_WINDOW - 5)
    return blob


def read_mc2_headless(le: LE):
    """(stub_va, loadlevel_off, gate_off) read back through the installed stub,
    or None. Identified by SHAPE (`60 e8 00000000 5a`), never by comparing the
    store's target against a signature that the patch itself rewrote."""
    try:
        hook_va, window, _, _, _ = find_mc2_headless_sites(le)
        if window[0] != 0xE9:  # still the pristine `add esp,12`
            return None
        hoff = va_to_file(le, hook_va)
        stub_va = hook_va + 5 + struct.unpack_from("<i", le.data, hoff + 1)[0]
        s = va_to_file(le, stub_va)
        if bytes(le.data[s : s + 5]) != b"\x83\xc4\x0c\x50\x52":
            return None
        ll = struct.unpack_from("<I", le.data, s + 32)[0]  # cmpb $0,[edx+D419C]
        jne_va = stub_va + 45
        menu = jne_va + 6 + struct.unpack_from("<i", le.data, s + 47)[0]
        jmp_va = stub_va + 51
        fall = jmp_va + 5 + struct.unpack_from("<i", le.data, s + 52)[0]
    except (SystemExit, Exception):
        return None
    return stub_va, ll, menu, fall


def has_mc2_headless(le: LE) -> bool:
    return read_mc2_headless(le) is not None


def read_init_park(stub: bytes, arm_off: int, park_off: int) -> Optional[int]:
    """The init park's timeout (timer counts) decoded from a stub's bytes, or
    None when the stub carries no park (patched with --init-park 0, or
    before 2026-09-27). Matches the exact prologue `emit_init_park` writes."""
    m = _re.search(
        b"\\x83\\xba" + _re.escape(struct.pack("<I", arm_off)) + b"\\x00\\x74."
        + b"\\xc7\\x82" + _re.escape(struct.pack("<II", arm_off, 0))
        + b"\\xc7\\x82" + _re.escape(struct.pack("<II", park_off, PARK_PARKED))
        + b"\\x8b\\x82....\\x05(....)",
        stub, _re.S)
    return struct.unpack("<I", m.group(1))[0] if m else None


def _park_note(timeout: Optional[int], hz: float) -> str:
    if timeout is None:
        return "init park ABSENT (the first window is the first capture)"
    return (f"init park: first level of a process holds until the host releases "
            f"it, or {timeout} counts (~{timeout / hz:.1f} s)")


def verify_mc2(path: str, inert: bool = False) -> None:
    import shutil

    data = open(path, "rb").read()
    le = parse_le(data)
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = data[code_off : code_off + o1.npages * 0x1000]

    # The stub preamble is distinctive: E8 00000000 5A 81 EA (call/pop/sub).
    idx = code.find(b"\xe8\x00\x00\x00\x00\x5a\x81\xea")
    if idx < 0:
        raise SystemExit("VERIFY FAIL: MC2 stub preamble not found in obj1")
    cave_va = o1.vbase + idx

    # The stub's `push edx ; call frame_fn ; pop edx` is the only `52 E8.. 5A`.
    rel = cave_va - o1.vbase
    j = next(
        (k for k in range(0, 400)
         if code[rel + k] == 0x52 and code[rel + k + 1] == 0xE8 and code[rel + k + 6] == 0x5A),
        None,
    )
    if j is None:
        raise SystemExit("VERIFY FAIL: no `push edx ; call frame_fn ; pop edx`")
    frame_fn = cave_va + j + 6 + struct.unpack_from("<i", code, rel + j + 2)[0]

    # Tail after `pop edx`: inc(6) + mov dword(10), then either `ret` outright
    # (floor OFF) or the 30-byte floor block ending in `ret`. Decode it rather
    # than assuming a length -- and fail loudly on a shape we did not emit, so
    # the reported stub_len can never silently under-run the real stub.
    p = rel + j + 7 + 6 + 10
    floor_val = 0
    if code[p] != 0xC3:
        fm = _re.match(
            rb"\x8b\x82(....)\x05(....)\x51\xb9....\x3b\x82(....)\x7e.\x49\x75.\x59\xc3",
            code[p:], _re.S,
        )
        if fm is None:
            raise SystemExit("VERIFY FAIL: unrecognised MC2 stub tail (floor block)")
        if fm.group(1) != fm.group(3):
            raise SystemExit("VERIFY FAIL: floor spin samples two different timers")
        floor_val = struct.unpack("<I", fm.group(2))[0]
        p += fm.end() - 1  # land on the `ret`
    stub_len = p + 1 - rel
    fl = (f"floor {floor_val} counts (>={(floor_val - 1) * 10:.0f} ms window)"
          if floor_val else "floor OFF")
    park = _park_note(read_init_park(code[rel : rel + stub_len], MB2_PARK_ARM, MB2_PARK), 100.0)
    aligned = "page-aligned" if o1.vsize % 0x1000 == 0 else f"NOT page-aligned ({o1.vsize:#x})"

    redirected = 0
    call_site = None
    if not inert:
        for i in range(len(code) - 5):
            if code[i] == 0xE8:
                t = o1.vbase + i + 5 + struct.unpack_from("<i", code, i + 1)[0]
                if t == cave_va:
                    redirected += 1
                    call_site = i
        if redirected != 1:
            raise SystemExit(f"VERIFY FAIL: expected 1 redirected call, found {redirected}")

    if inert:
        print(f"VERIFY {path}: OK (INERT)")
        print(f"  MC2 stub @ {cave_va:#x} ({stub_len} bytes) but NO call site targets "
              f"it; obj1.vsize {aligned}; {fl}")
        # --inert suppresses the SIGNAL stub's wiring, not the headless fix --
        # say so, rather than leaving a patch present but unreported.
        hl = read_mc2_headless(le)
        print(f"  headless -level: {'present, stub @ %#x' % hl[0] if hl else 'ABSENT'} "
              f"(--inert does not disable it)")
        return
    # The native frame period is the `add esi,N` imm8 right after the call.
    period = code[call_site + 7] if code[call_site + 5 : call_site + 7] == b"\x83\xc6" else None
    per = (f"; frame period {period} (~{100 / period:.1f} fps @ 100 Hz)"
           if period else "")
    print(f"VERIFY {path}: OK")
    print(f"  1 call site -> stub @ {cave_va:#x}; stub -> frame driver @ {frame_fn:#x}; "
          f"{stub_len} bytes; mailbox guest {MB2_GUEST:#x} (MGCTTIK2); obj1.vsize "
          f"{aligned}; entry untouched; {fl}{per}")
    print(f"  {park}")
    if has_volcano_guard_mc2(le):
        print(f"  {verify_volcano_mc2(le)}")
        print("  \u26a0 SIMULATION IS PATCHED -- recordings from this binary "
              "witness the PATCHED arm, never retail.")

    # The headless `-level` fix. Report it either way -- its ABSENCE means
    # `-level N` lands in the main menu and is only honoured on the second pass.
    hl = read_mc2_headless(le)
    if hl:
        stub_va, ll_off, menu, fall = hl
        print(f"  headless -level: menu gate -> stub @ {stub_va:#x}; falls "
              f"through to NewGameDialog @ {fall:#x} only when LoadLevelNumber "
              f"obj3+{ll_off:#x} >= 0, else jne @ {menu:#x} as shipped; "
              f"x_BYTE_E29E1 itself is never written")
    else:
        print("  \u26a0 NO headless -level fix: `-level N` will land in the main "
              "menu and only take effect after you enter the campaign once. "
              "Re-patch without --no-headless-level.")
    if shutil.which("ndisasm"):
        import subprocess
        import tempfile

        with tempfile.NamedTemporaryFile(delete=False, suffix=".bin") as tf:
            tf.write(code[rel : rel + stub_len])
            tmp = tf.name
        out = subprocess.run(
            ["ndisasm", "-b", "32", "-o", hex(cave_va), tmp], capture_output=True, text=True
        ).stdout
        print("  --- stub disassembly ---")
        for ln in out.strip().splitlines():
            print("   ", ln)


def is_mc2(le: LE) -> bool:
    """A NETHERW.EXE has the MC2 limiter signature; CARPET/HIDDEN have the MC1
    tick-fn prologue. Peek for the former."""
    o1 = le.objs[0]
    code_off = obj_file_off(le, o1)
    code = bytes(le.data[code_off : code_off + o1.npages * 0x1000])
    return MC2_LIMITER_SIG.search(code) is not None


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe", help="CARPET.EXE / HIDDEN.EXE (MC1) or NETHERW.EXE (MC2), a pristine copy")
    ap.add_argument("-o", "--out", help="output path (default: <NAME>_REC.EXE)")
    ap.add_argument(
        "--period",
        type=int,
        default=5,
        help="frame period in ~120 Hz PIT counts. fps = 120 / period: "
             "default 5 -> ~24 fps; 4 -> ~30 fps; 6 -> ~20 fps "
             "(measured live: period 30 gave ~4 fps).",
    )
    ap.add_argument(
        "--pace",
        type=int,
        default=None,
        metavar="N",
        help="MC2 only: widen the native frame period to N 100 Hz ticks "
             "(default 5 = ~20 fps). Higher N = lower fps but a WIDER capture "
             "window, guaranteeing a quiescent spin even on graphics-heavy "
             "frames whose compute would otherwise eat the window (the cause of "
             "sporadic missed frames). Sim-neutral: the recorded frame sequence "
             "is byte-identical, just paced slower. Try 10-15 for heavy levels.",
    )
    ap.add_argument(
        "--floor",
        type=int,
        default=FLOOR_DEFAULT,
        metavar="N",
        help=f"BOTH arms: minimum capture window, in timer counts, held open on "
             f"every paced frame even when the frame overran its budget (default "
             f"{FLOOR_DEFAULT}; 0 disables). Both stubs pace to an ABSOLUTE "
             f"deadline, so a frame heavy enough to overrun it (deaths, meteor "
             f"swarms) leaves a zero-width window and the recorder drops the "
             f"frame -- the floor makes the width load-independent. The counter "
             f"is integral (MC1 ~120 Hz, MC2 100 Hz), so N=1 guarantees nothing "
             f"and N=2 is the smallest value that guarantees a full count "
             f"(8.3 / 10 ms). Costs time ONLY on frames that already blew their "
             f"budget, unlike --period / --pace. Max {FLOOR_MAX}.",
    )
    ap.add_argument(
        "--volcano-guard",
        action="store_true",
        help="ALL THREE BUILDS: also install the VOLCANO WRITE GUARD -- the binary "
             "half of the port's `volcano_register_revalidate` patch. Retail's "
             "eruption start makes two blind writes through stale slot "
             "registers (gated only on `slot != 0`): the `+26 = 250` kick, "
             "which turns a re-minted CASTLE into a level-250 castle and HANGS "
             "the game on its next downgrade, and the plume soft-kill, which "
             "kills whatever inherited the old column's slot. The guard lets "
             "the kick land only on a (10,18) other than the driver itself and "
             "the kill only on a (10,19). "
             "WARNING: this CHANGES SIMULATION, unlike the pacing stub -- a "
             "binary carrying it is no longer the shipped one, and recordings "
             "made with it witness the PATCHED arm, never retail.",
    )
    ap.add_argument(
        "--no-timer-init",
        action="store_true",
        help="MC1 ONLY: do NOT splice the timer install into the `-custom` "
             "branch. `-custom` (and -network / -demo / -roll, same flag bit) "
             "skips TopProcedure's menu loop, which is the ONLY caller of "
             "sub_357C0_35B80 -- the function that installs the timer ISR, on "
             "BOTH its arms (sound IRQ 0x78, or the no-sound START_TIMER that "
             "programs PIT ch0 to 120.5 Hz). Nothing then increments the "
             "counter the pacer spins on, its frozen-timer guard fires every "
             "frame, and the game runs at <1 fps. The default splices one "
             "idempotent call into that branch; the patched bytes are "
             "UNREACHABLE on the menu path, so menu-path captures are "
             "unaffected. Sim-neutral in the sense that matters here -- it "
             "starts a clock, it does not change what any sub-step computes -- "
             "but it does mean a `-custom` launch now initialises sound as "
             "every normal launch does (add `-time` to take the silent arm).",
    )
    ap.add_argument(
        "--no-headless-level",
        action="store_true",
        help="MC2 ONLY: do NOT make `-level N` skip the main menu. On a "
             "pristine NETHERW, `-level N` does NOT boot straight in -- "
             "`x_BYTE_E29E1` starts at 1 and the menu condition SHORT-CIRCUITS "
             "on it, so NewGameDialog (the only reader of -level) is never "
             "called on the first pass; it is cleared only at the end of the "
             "menu loop, which is why entering the campaign once makes -level "
             "work thereafter. It is not in config.dat, so there is no "
             "configuration route to a headless boot. The default clears that "
             "byte from inside the `-level` handler, so a launch WITHOUT "
             "-level never executes the patched instruction and the menu "
             "behaves exactly as shipped. (Not fixed either way: once in the "
             "level there is no way back out -- LoadLevelNumber is never "
             "cleared, so quitting relaunches it forever. Kill DOSBox.)",
    )
    ap.add_argument(
        "--init-park",
        type=int,
        default=INIT_PARK_DEFAULT,
        metavar="N",
        help=f"BOTH arms: THE INIT PARK -- on the first call of a level (the "
             f"first level of a fresh process, or whenever the recorder re-arms "
             f"it) hold the world exactly as level initialisation left it, "
             f"before frame 1, until the recorder releases it, or N timer counts "
             f"pass (default {INIT_PARK_DEFAULT}: ~4 s on MC1, 5 s on MC2; 0 = "
             f"no park code at all). This is the snapshot the recorder writes as "
             f"the `init` record. Sim-neutral: a wait, no writes to game memory. "
             f"Max {INIT_PARK_MAX}.",
    )
    ap.add_argument("--verify-only", metavar="PATCHED", help="just re-verify an already-patched exe")
    ap.add_argument(
        "--inert",
        action="store_true",
        help="DIAGNOSTIC: write the stub into the cave but do NOT redirect any "
             "call site, so the stub is never executed. If the game still "
             "crashes, the cave write itself (not the stub logic) is the problem.",
    )
    ap.add_argument(
        "--passthrough",
        action="store_true",
        help="DIAGNOSTIC: wire the call sites to a bare `call <tick fn> ; ret` "
             "trampoline (no pacing, no delta, no mailbox). Isolates whether "
             "calling the tick fn through the cave is itself the problem.",
    )
    ap.add_argument(
        "--no-extend",
        action="store_true",
        help="DIAGNOSTIC: do NOT page-align obj1's vsize. The cave stays past "
             "the declared code size (unloaded / outside the CS limit), so this "
             "reproduces the crash -- use it to A/B against the default fix.",
    )
    args = ap.parse_args(argv)

    if not (0 <= args.floor <= FLOOR_MAX):
        raise SystemExit(f"--floor must be in 0..{FLOOR_MAX} (0 = off)")
    if args.floor == 1:
        raise SystemExit(
            "--floor 1 guarantees no wait at all: the timer is an integer "
            "counter, so entering a hair before it ticks releases immediately. "
            "Use 2 (the smallest value that guarantees a full count) or 0 to "
            "disable the floor."
        )

    if not (0 <= args.init_park <= INIT_PARK_MAX):
        raise SystemExit(f"--init-park must be in 0..{INIT_PARK_MAX} (0 = off)")

    if args.verify_only:
        vdata = open(args.verify_only, "rb").read()
        if is_mc2(parse_le(vdata)):
            verify_mc2(args.verify_only, inert=args.inert)
        else:
            verify(args.verify_only, args.period, inert=args.inert, passthrough=args.passthrough)
        return 0

    data = open(args.exe, "rb").read()
    le = parse_le(data)

    def _out_path():
        if args.out:
            return args.out
        import os

        base = os.path.basename(args.exe)
        stem, ext = os.path.splitext(base)
        # A volcano-guarded binary gets its OWN name: it is not the shipped
        # simulation, so it must never be mistaken for the recording EXE on
        # disk. Takes made with it witness the PATCHED arm.
        tag = "_RECVG" if getattr(args, "volcano_guard", False) else "_REC"
        return os.path.join(os.path.dirname(args.exe) or ".", f"{stem}{tag}{ext or '.EXE'}")

    # --- MC2 / NETHERW: signal-only (no pacer), optional frame-period widen. ---
    if is_mc2(le):
        if args.passthrough:
            raise SystemExit("--passthrough is an MC1-only diagnostic")
        if args.no_timer_init:
            raise SystemExit(
                "--no-timer-init is MC1-only: MC2 has no `-custom` (its `-level N` "
                "boots straight in on its own) and its arm adds no pacing, so "
                "there is no deadline to starve."
            )
        b2 = find_build_mc2(le)
        mode = "  [INERT: stub written, NOT wired]" if args.inert else ""
        mode += "  [--no-extend: vsize NOT page-aligned]" if args.no_extend else ""
        pace_note = (f"  [--pace {args.pace}: period {b2.period_now}->{args.pace}, "
                     f"~{100 / max(args.pace, 1):.1f} fps]" if args.pace is not None else "")
        print(f"build={b2.name}  hook(call)={b2.call_site:#x}  frame_fn={b2.frame_fn:#x}  "
              f"cave={b2.cave_va:#x}  mailbox={MB2_GUEST:#x}  "
              f"timer=obj3+{b2.obj3ref_off:#x}{mode}{pace_note}")
        stub = patch_mc2(le, b2, wire=not args.inert, extend=not args.no_extend,
                         pace=args.pace, floor=args.floor, volcano=args.volcano_guard,
                         headless=not args.no_headless_level, init_park=args.init_park)
        out = _out_path()
        with open(out, "wb") as f:
            f.write(le.data)
        tag = ", INERT" if args.inert else ""
        pace_tag = f", pace={args.pace}" if args.pace is not None else ", signal-only"
        floor_tag = f", floor={args.floor}" if args.floor else ", floor=OFF"
        vg = ", VOLCANO-GUARD (simulation patched, NOT retail)" if args.volcano_guard else ""
        hl = ", NO headless -level" if args.no_headless_level else ", headless -level"
        pk = f", init-park={args.init_park}" if args.init_park else ", init-park=OFF"
        print(f"wrote {out}  (stub {len(stub)} B{pace_tag}{floor_tag}{pk}{hl}{tag}{vg})")
        verify_mc2(out, inert=args.inert)
        return 0

    # --- MC1 / CARPET / HIDDEN: pacer + mailbox. ---
    if args.no_headless_level:
        raise SystemExit(
            "--no-headless-level is MC2-only: MC1's `-level N` is honoured "
            "immediately, it is just overwritten by the campaign map screen "
            "unless you also pass -custom (see --no-timer-init)."
        )
    b_ = find_build(le)
    mode = ("  [INERT: stub written, NOT wired]" if args.inert
            else "  [PASSTHROUGH: bare call/ret trampoline]" if args.passthrough else "")
    mode += "  [--no-extend: vsize NOT page-aligned]" if args.no_extend else ""
    print(f"build={b_.name}  hook={b_.hook_va:#x}  cave={b_.cave_va:#x}  "
          f"wallclock={b_.wallclock:#x}{mode}")
    stub = patch(le, b_, args.period, wire=not args.inert, passthrough=args.passthrough,
                 extend=not args.no_extend, floor=args.floor, volcano=args.volcano_guard,
                 timer_init=not args.no_timer_init, init_park=args.init_park)

    out = _out_path()
    with open(out, "wb") as f:
        f.write(le.data)
    tag = ", INERT" if args.inert else ", PASSTHROUGH" if args.passthrough else ""
    floor_tag = f", floor={args.floor}" if args.floor else ", floor=OFF"
    vol_tag = ", VOLCANO-GUARD (simulation patched, NOT retail)" if args.volcano_guard else ""
    tmr_tag = ", NO -custom timer init" if args.no_timer_init else ", -custom timer init"
    pk = f", init-park={args.init_park}" if args.init_park else ", init-park=OFF"
    print(f"wrote {out}  (stub {len(stub)} B, period={args.period}"
          f"{floor_tag}{pk}{tmr_tag}{tag}{vol_tag})")
    verify(out, args.period, inert=args.inert, passthrough=args.passthrough)
    return 0


if __name__ == "__main__":
    sys.exit(main())
