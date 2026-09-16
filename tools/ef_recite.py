#!/usr/bin/env python3
"""Re-point stale `EF:<n>` citations at the current
reference/remc2/remc2/engine/EventsFunctions.cpp.

⚠⚠⚠ BUILT AND DRY-RUN IN ROUND 141, DELIBERATELY NOT APPLIED. Read this
before running it with --apply.

The debt is real and measured: of the 1,286 citations that name exactly
one `sub_`, only SIX still land on their target; 993 point below it and
the drift is not uniform (clusters at -13, -22, -51, plus a tail around
-300..-400, from citations written against different upstream commits).

The reason it was not applied is that THE TREE HAS NO SINGLE CONVENTION.
Some rounds cite the IDA address banner `//----- (00XXXXXX) ----`, others
cite the function signature on the NEXT line. This script normalises
everything onto the banner, so applying it would silently move every
citation of the other kind by one -- including round 141's own fresh,
correct ones (e.g. `sub_5C950` EF:43982, the signature; its banner is
43981). Fixing that properly means recording which convention each
citation used, which the text does not carry.

If you do apply it: pick the convention first, make the script honour it,
and do it in a round with NO other source edits in flight, because it
rewrites ~1,250 comment lines across ~50 files and will bury a real diff.

Resolution is by the `sub_<ADDR>` name on the SAME source line, via the
address banner — NOT by matching the symbol, because remc2 renames
functions in place (`sub_508E0` is now `sub_508E0_castle_defend_create`).

  ef_fix.py            dry run: print every rewrite it would make
  ef_fix.py --apply    rewrite in place
"""
import re, sys, collections, subprocess, os
ROOT='/home/rain/projects/mgcarpet'
EF=os.path.join(ROOT,'reference/remc2/remc2/engine/EventsFunctions.cpp')
lines=open(EF,encoding='utf-8',errors='replace').read().split('\n')
banner=re.compile(r'^//-----\s*\(([0-9A-Fa-f]{8})\)')
defn=re.compile(r'^[A-Za-z_].*\b(sub_[0-9A-Fa-f]{3,6})[0-9A-Za-z_]*\s*\(')
addr2line={}; def2line={}
for i,l in enumerate(lines,1):
    m=banner.match(l)
    if m: addr2line[m.group(1).upper().lstrip('0')]=i
    d=defn.match(l)
    if d and not l.rstrip().endswith(';'): def2line.setdefault(d.group(1).split('_')[1].upper().lstrip('0'),i)
sym=re.compile(r'\b(sub_[0-9A-Fa-f]{3,6})')
out=subprocess.run(['rg','-n','--no-heading','EF:[0-9]+',os.path.join(ROOT,'crates')],
                   capture_output=True,text=True).stdout.split('\n')
edits=collections.defaultdict(list); stats=collections.Counter()
for row in out:
    if not row: continue
    try: path,ln,txt=row.split(':',2)
    except ValueError: continue
    names=set(sym.findall(txt)); refs=set(int(x) for x in re.findall(r'EF:([0-9]+)',txt))
    if len(names)!=1 or len(refs)!=1:
        stats['skipped: not a clean 1 symbol / 1 EF line']+=1; continue
    name=names.pop(); ref=refs.pop()
    a=name.split('_')[1].upper().lstrip('0')
    L=addr2line.get(a); how='banner'
    if L is None: L=def2line.get(a); how='defline'
    if L is None: stats['UNRESOLVED']+=1; print(f"UNRESOLVED {path}:{ln} {name} EF:{ref}"); continue
    if L==ref: stats['already correct']+=1; continue
    stats[f'rewrite via {how}']+=1
    edits[path].append((int(ln),ref,L,name))
for k,v in sorted(stats.items()): print(f"{k}: {v}")
apply='--apply' in sys.argv
n=0
for path,es in sorted(edits.items()):
    src=open(path,encoding='utf-8').read().split('\n')
    for ln,ref,L,name in es:
        old=src[ln-1]; new=re.sub(rf'EF:{ref}\b',f'EF:{L}',old)
        if new==old: print(f"  !! no-op {path}:{ln} EF:{ref}"); continue
        src[ln-1]=new; n+=1
        if not apply and n<=12: print(f"  {path}:{ln}  {name}  EF:{ref} -> EF:{L}")
    if apply: open(path,'w',encoding='utf-8').write('\n'.join(src))
print(f"\n{'APPLIED' if apply else 'WOULD REWRITE'} {n} citations across {len(edits)} files")
