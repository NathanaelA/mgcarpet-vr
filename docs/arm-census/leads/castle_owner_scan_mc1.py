import sys, json, base64, re, struct
from collections import Counter
STEP=int(sys.argv[1])
POOL=29795; STR=164; WIZ=13323; WS=2049; T160=1103
pre=re.compile(rb'^\{"t":(\d+)')
last=None
for line in sys.stdin.buffer:
    m=pre.match(line)
    if not m: continue
    t=int(m.group(1))
    if t%STEP: continue
    d=json.loads(line).get('state',{}).get('struct_b64')
    if not d: continue
    d=base64.b64decode(d)
    local=struct.unpack_from('<H',d,8)[0]
    carpets=[struct.unpack_from('<H',d,WIZ+i*WS+10)[0] for i in range(8)]
    regs=[struct.unpack_from('<H',d,WIZ+i*WS+T160+50)[0] for i in range(8)]
    cs=[]
    for sl in range(1,1000):
        o=POOL+sl*STR
        if d[o+64]==3 and d[o+65]==2:
            cs.append((sl,struct.unpack_from('<H',d,o+24)[0],struct.unpack_from('<h',d,o+26)[0],hex(struct.unpack_from('<I',d,o+160)[0])))
    c=Counter((x[1],x[3]) for x in cs)
    multi=[k for k,v in c.items() if v>=2]
    key=(tuple(cs),)
    if key!=last:
        orph=[x for x in cs if x[1] not in carpets]
        print(t,'local',local,'hc',carpets[local],'ORPH' if orph else '',orph,'carpets',carpets,'reg',regs[local],'MULTI' if multi else '', multi, cs, flush=True)
    last=key
