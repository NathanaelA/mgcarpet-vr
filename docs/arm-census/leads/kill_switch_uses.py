"""Map every MGC_NO_* kill switch in mgc-sim to the port fns that consult it.
A law consulted in arm A of a retail routine but not in its twin arm B is the
strongest drift lead for the arm census (docs/ARM-CENSUS.md). Writes killswitches.json."""
import re,os,collections,json
root=os.path.join(os.path.dirname(os.path.abspath(__file__)),'../../../crates/mgc-sim/src')
fnre=re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?fn\s+([A-Za-z0-9_]+)')
files={}
for dp,_,fs in os.walk(root):
  for f in fs:
    if f.endswith('.rs'):
      p=os.path.join(dp,f); files[os.path.relpath(p,root)]=open(p).read().split('\n')
defs={}
for rel,lines in files.items():
  cur=None
  for i,l in enumerate(lines):
    m=fnre.match(l)
    if m: cur=(m.group(1),i)
    e=re.search(r'var_os\("(MGC_NO_[A-Z0-9_]+)"',l)
    if e and cur: defs[cur[0]]=(e.group(1),rel,cur[1]+1)
uses=collections.defaultdict(set)
pat=re.compile(r'\b('+'|'.join(sorted(defs,key=len,reverse=True))+r')\s*\(')
for rel,lines in files.items():
  cur='<top>'; intest=False
  for l in lines:
    if re.match(r'\s*#\[cfg\(test\)\]',l): intest=True
    m=fnre.match(l)
    if m: cur=m.group(1)
    if intest: continue
    for n in pat.findall(l):
      if cur!=n: uses[n].add(f'{rel}:{cur}')
rows=[dict(switch=n,env=e,defined=f'{r}:{ln}',uses=sorted(uses[n])) for n,(e,r,ln) in sorted(defs.items())]
json.dump(rows,open('killswitches.json','w'),indent=1)
print(len(rows),'switches;',collections.Counter(len(r['uses']) for r in rows))
