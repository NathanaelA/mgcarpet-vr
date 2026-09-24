import sys, json, base64, re, struct
POOL=0x6E8E; STR=168; PL=0x2BDE
pre=re.compile(rb'^\{"t":(\d+)')
last=None
for line in sys.stdin.buffer:
    m=pre.match(line)
    if not m: continue
    t=int(m.group(1))
    s=json.loads(line).get('state',{}).get('struct_b64')
    if not s: continue
    d=base64.b64decode(s)
    reg=struct.unpack_from('<h',d,PL+998+58)[0]
    cs=[]
    for sl in range(1,1000):
        o=POOL+sl*STR
        if d[o+0x3F]==3 and d[o+0x40]==2 and struct.unpack_from('<H',d,o+0x1A)[0]==114 and not struct.unpack_from('<I',d,o+0xC)[0]&0x400:
            cs.append((sl,struct.unpack_from('<i',d,o+0x10)[0]))
    k=(tuple(cs),reg)
    if k!=last: print(t,'reg',reg,cs,flush=True)
    last=k
