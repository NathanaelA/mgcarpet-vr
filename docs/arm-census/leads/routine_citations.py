import re,os,sys,json,collections
root=os.path.join(os.path.dirname(os.path.abspath(__file__)),'../../../crates/mgc-sim/src')
fnre=re.compile(r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+([A-Za-z0-9_]+)')
subre=re.compile(r'sub_[0-9A-F]{5}(?:_[0-9A-F]{5})?')
cites=collections.defaultdict(list)   # sub -> [(file,fn,line,kind)]
for dp,_,fs in os.walk(root):
  for f in fs:
    if not f.endswith('.rs'): continue
    p=os.path.join(dp,f); rel=os.path.relpath(p,root)
    lines=open(p).read().split('\n')
    intest=False; cur=None; curline=0
    # find test module start
    for i,l in enumerate(lines):
      if re.match(r'\s*#\[cfg\(test\)\]',l): intest=True
      m=fnre.match(l)
      if m: cur=m.group(1); curline=i
      for s in subre.findall(l):
        # implementation marker: citation in doc comment within 12 lines before an fn decl
        kind='body'
        for j in range(i+1,min(i+14,len(lines))):
          if fnre.match(lines[j]) and all(x.strip().startswith(('///','#[','//')) or x.strip()=='' for x in lines[i:j]):
            kind='doc:'+fnre.match(lines[j]).group(1); break
        fn = kind[4:] if kind.startswith("doc:") else (cur or "<top>")
        cites[s].append((rel,fn,i+1,'doc' if kind.startswith('doc') else 'body','test' if intest else 'prod'))
out=[]
for s,c in cites.items():
  prod=[x for x in c if x[4]=='prod']
  fns=sorted({(x[0],x[1]) for x in prod})
  docfns=sorted({(x[0],x[1]) for x in prod if x[3]=='doc'})
  files={x[0] for x in prod}
  out.append((len(docfns),len(fns),len(files),s,docfns,fns))
out.sort(key=lambda x:(-x[0],-x[1]))
json.dump([dict(sub=o[3],ndoc=o[0],nfn=o[1],nfiles=o[2],docfns=o[4],fns=o[5]) for o in out],open('census.json','w'),indent=1)
print(sum(1 for o in out if o[0]>=2),'routines with doc-comment on >=2 fns')
print(sum(1 for o in out if o[1]>=2 and o[2]>=2),'routines cited in >=2 fns across >=2 files')
for o in out[:60]: print(o[0],o[1],o[2],o[3],' | '.join(f'{a}:{b}' for a,b in o[4])[:300])
