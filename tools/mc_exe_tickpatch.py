#!/usr/bin/env python3
"""Detour-patch CARPET.EXE / HIDDEN.EXE to pace the sim tick loop and expose a
recorder mailbox.

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

  1. paces every sub-step to a wall-clock deadline (the game's own PIT
     counter, measured live at ~480 Hz). At the default game speed the caller
     runs the tick fn 4x per rendered frame, so fps = 480 / (4 * period): the
     default period 5 gives ~24 fps (the authentic Magic Carpet rate),
     regardless of how high DOSBox cycles are set. The excess cycles are
     burned in a wall-clock spin, which is exactly the large *quiescent*
     window the recorder wants: the world struct is settled and untouched.
  2. maintains a mailbox in obj3's committed tail: a magic, a monotonic tick
     counter, and an ``in_window`` flag raised for the whole spin. The
     recorder snipes on ``in_window==1`` keyed by the tick counter and gets
     one coherent snapshot per sub-step -- gap-free by construction, no more
     +63 heuristic.

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
- The stub touches only EAX/ECX/EDX (caller-clobber on a void, no-arg fn); the
  original tick fn saves/restores EBX/ESI/EDI/EBP, so the caller's callee-saved
  registers (its loop counter in EBX) survive the wrapper unchanged.
- DOS/4GW relocates the image by one base and injected code gets no LE fixups,
  so the stub computes that load delta at runtime (call/pop) and addresses all
  globals and the original tick fn relatively (delta-invariant).
- A guard counter bounds the spin: if the timer ISR is ever masked (counter
  frozen) the stub releases after ~1 s of emulated time instead of hanging.

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
MB_PERIOD = MB_OBJ3 + 0x18  # u32 sub-step period in PIT counts (default 1)
MB_GUEST = OBJ3_BASE + MB_OBJ3  # guest-LINK addr the recorder reads (0x132c40)

MAGIC0 = 0x5443474D  # "MGCT"
MAGIC1 = 0x314B4954  # "TIK1"

GUARD_ITERS = 0x04000000  # spin bail-out (~1 s emulated); never hit if ISR live
RESYNC_COUNTS = 30  # >250 ms behind schedule -> resync instead of catch-up burst

WALLCLOCK_FROM_STRUCTPTR = 0x1E2C  # wallclock obj3-offset = structptr_off - this

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
    if not (obj3.vsize <= MB_OBJ3 and MB_OBJ3 + 0x20 <= committed):
        raise ValueError(
            f"mailbox obj3-off {MB_OBJ3:#x} not in obj3 tail "
            f"[vsize {obj3.vsize:#x}, committed {committed:#x})")

    name = "CARPET" if wallclock == 0xAC5D4 else ("HIDDEN" if wallclock == 0xAC5C4 else "?")
    return Build(name, hook_va, tuple(call_sites), cave_va, wallclock, structptr_pre)


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

    def sub_edx_imm(self, imm):
        self.raw(b"\x81\xea" + struct.pack("<I", imm & 0xFFFFFFFF))

    def sub_eax_imm(self, imm):  # sub eax, imm32
        self.raw(b"\x2d" + struct.pack("<I", imm & 0xFFFFFFFF))

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

    def sub_eax_m(self, a):  # sub eax,[edx+a]
        self.raw(b"\x2b\x82" + struct.pack("<I", a))

    def cmp_eax_imm(self, imm):
        self.raw(b"\x3d" + struct.pack("<I", imm & 0xFFFFFFFF))

    def mov_ecx_imm(self, imm):
        self.raw(b"\xb9" + struct.pack("<I", imm & 0xFFFFFFFF))

    def dec_ecx(self):
        self.raw(b"\x49")

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


def build_passthrough(b: Build) -> bytes:
    """A bare wrapper: `call <tick fn> ; ret`, nothing else. Wired in place of
    the full stub, it isolates whether merely calling the tick fn through a
    cave trampoline is the problem, independent of any pacing logic."""
    rel = b.hook_va - (b.cave_va + 5)  # E8 rel32 at cave_va+0, so +5
    return b"\xe8" + struct.pack("<i", rel) + b"\xc3"


def build_stub(b: Build, period: int) -> bytes:
    a = Asm(b.cave_va)
    wc_off = b.structptr_off - WALLCLOCK_FROM_STRUCTPTR  # wallclock obj3-offset

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

    # --- open the quiescent window for this sub-step ---
    a.inc_m(MB_TICK)
    a.mov_m_imm(MB_INWIN, 1)

    # --- spin until now >= deadline (or bail on a frozen counter) ---
    # diff = now - deadline as a SIGNED i32: negative => still waiting,
    # non-negative => the deadline passed. Signed handles both the normal
    # "deadline slightly ahead" wait and a post-pause "deadline far behind"
    # resync with the same subtraction (no unsigned underflow).
    a.mov_ecx_imm(GUARD_ITERS)
    a.label("spin")
    a.mov_eax_m(wc_off)  # eax = now
    a.sub_eax_m(MB_DEADLINE)  # eax = now - deadline (signed)
    a.br8(0x79, "passed")  # jns passed  (now >= deadline)
    a.dec_ecx()
    a.br8(0x75, "spin")  # jnz spin  (keep waiting)
    a.br8(0xEB, "release")  # guard expired (counter frozen) -> release

    a.label("passed")
    a.cmp_eax_imm(RESYNC_COUNTS)  # eax >= 0 here
    a.br8(0x72, "release")  # jb release  (within one catch-up bound)
    a.mov_eax_m(wc_off)  # too far behind (long pause) -> drop the backlog
    a.mov_m_eax(MB_DEADLINE)  # deadline = now

    a.label("release")
    a.mov_eax_m(MB_DEADLINE)
    a.add_eax_m(MB_PERIOD)
    a.mov_m_eax(MB_DEADLINE)  # deadline += period (fixed cadence, no drift)
    a.mov_m_imm(MB_INWIN, 0)

    body = a.assemble()

    # --- call the ORIGINAL (untouched) tick fn, then return to the caller ---
    # A relative call: both the stub and the tick fn are in obj1, so the rel32
    # is position-independent (delta-invariant). The stub only touched
    # eax/ecx/edx; the tick fn saves/restores ebx/esi/edi/ebp itself, so the
    # caller's callee-saved regs (its loop counter in ebx) survive intact.
    call_pos = len(body)
    rel = b.hook_va - (b.cave_va + call_pos + 5)
    return body + b"\xe8" + struct.pack("<i", rel) + b"\xc3"  # call hook ; ret


# --------------------------------------------------------------------------
# Patch / verify
# --------------------------------------------------------------------------
def patch(le: LE, b: Build, period: int, wire: bool = True, passthrough: bool = False,
          extend: bool = True) -> bytes:
    o1 = le.objs[0]
    stub = build_passthrough(b) if passthrough else build_stub(b, period)
    cave_off = va_to_file(le, b.cave_va)
    if cave_off + len(stub) > obj_file_off(le, o1) + o1.npages * 0x1000:
        raise ValueError("stub overflows the cave")
    le.data[cave_off : cave_off + len(stub)] = stub

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
        if b.cave_va + len(stub) > o1.vbase + new1:
            raise ValueError("stub crosses the page boundary; extend by another page")
        struct.pack_into("<I", le.data, le.lx + objtab + 0 * 24, new1)
        o1.vsize = new1

        o3 = le.objs[2]
        new3 = (o3.vsize + 0xFFF) & ~0xFFF
        if o3.vbase + MB_OBJ3 + 0x20 > o3.vbase + new3:
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
    return stub


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

    if inert:
        print(f"VERIFY {path}: OK (INERT)")
        print(f"  stub present @ {cave_va:#x} ({stub_len} bytes) but NO call site "
              f"targets it -- never executed; obj1.vsize {aligned}")
        return

    print(f"VERIFY {path}: OK")
    print(f"  {redirected} call site(s) -> stub @ {cave_va:#x}; stub -> original "
          f"tick fn @ {hook_va:#x}; {stub_len} bytes; obj1.vsize {aligned}; entry untouched")
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


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe", help="CARPET.EXE or HIDDEN.EXE (a pristine copy)")
    ap.add_argument("-o", "--out", help="output path (default: <NAME>_REC.EXE)")
    ap.add_argument(
        "--period",
        type=int,
        default=5,
        help="sub-step period in ~480 Hz PIT counts. fps = 480 / (4 substeps * "
             "period): default 5 -> ~24 fps; 4 -> ~30 fps; 6 -> ~20 fps "
             "(measured live: period 30 gave ~4 fps).",
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

    if args.verify_only:
        verify(args.verify_only, args.period, inert=args.inert, passthrough=args.passthrough)
        return 0

    data = open(args.exe, "rb").read()
    le = parse_le(data)
    b_ = find_build(le)
    mode = ("  [INERT: stub written, NOT wired]" if args.inert
            else "  [PASSTHROUGH: bare call/ret trampoline]" if args.passthrough else "")
    mode += "  [--no-extend: vsize NOT page-aligned]" if args.no_extend else ""
    print(f"build={b_.name}  hook={b_.hook_va:#x}  cave={b_.cave_va:#x}  "
          f"wallclock={b_.wallclock:#x}{mode}")
    stub = patch(le, b_, args.period, wire=not args.inert, passthrough=args.passthrough,
                 extend=not args.no_extend)

    out = args.out
    if not out:
        import os

        base = os.path.basename(args.exe)
        stem, ext = os.path.splitext(base)
        out = os.path.join(os.path.dirname(args.exe) or ".", f"{stem}_REC{ext or '.EXE'}")
    with open(out, "wb") as f:
        f.write(le.data)
    tag = ", INERT" if args.inert else ", PASSTHROUGH" if args.passthrough else ""
    print(f"wrote {out}  (stub {len(stub)} B, period={args.period}{tag})")
    verify(out, args.period, inert=args.inert, passthrough=args.passthrough)
    return 0


if __name__ == "__main__":
    sys.exit(main())
