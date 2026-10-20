"""Move the Unicorn Way street so its archway to the Commons sits in the Stranded Graveyard's exit door.

The door (AEG027_002_0504, m18) is converted to The First Step tile with WorldMapLegacyConvParam (rows 140/141:
m18 + (147, 16, -102) = m60_42_36). The street turns with the door, and the grace parts and spawn points Ryan placed on
the street keep their spot on it. Works on a DFLT (zlib) map file as Smithbox saves it.

Usage: python msb_move_street.py in.msb.dcx out.msb.dcx
"""
import math,struct,zlib,sys
from msb_floor import read_msb,write_msb,entry_name,transform,set_transform,_list
def R(v,deg):
    c,s=math.cos(math.radians(deg)),math.sin(math.radians(deg)); x,y,z=v
    return (x*c+z*s, y, -x*s+z*c)
def to_world(flv,P,deg): r=R(flv,deg); return tuple(P[i]+r[i] for i in range(3))
def to_flver(w,P,deg): return R(tuple(w[i]-P[i] for i in range(3)),-deg)
DOOR=(-141.72+147, 73.58+16, 4.28-102); DOOR_YAW=-176.0
NEW_DEG=DOOR_YAW+180.0
mouth_flver=(-0.08,-0.01,-10.91)  # arch opening centre, floor level (OBJ z negated)
out=R((0,0,1),NEW_DEG); gap=0.2
target=(DOOR[0]+out[0]*gap, DOOR[1], DOOR[2]+out[2]*gap)
r=R(mouth_flver,NEW_DEG); NEW_P=tuple(target[i]-r[i] for i in range(3))
src,dst=sys.argv[1:3]
d=open(src,'rb').read(); raw=zlib.decompress(d[0x4c:])
h,L=read_msb(raw); parts=_list(L,'PARTS_PARAM_ST').entries
idx={entry_name(e):i for i,e in enumerate(parts)}
(op,orot)=transform(parts[idx['AEG030_518_9000']]); od=orot[1]
print('old street',op,od,'new',NEW_P,NEW_DEG)
FOLLOW=['AEG099_090_9003','AEG099_090_9006','AEG099_090_9008','AEG099_090_9011','c1000_9001','c0000_9003']
for n in FOLLOW:
    p,rot=transform(parts[idx[n]]); loc=to_flver(p,op,od); w=to_world(loc,NEW_P,NEW_DEG)
    nr=(rot[0],(rot[1]+NEW_DEG-od+180)%360-180,rot[2])
    print(n,[round(x,2) for x in p],'->',[round(x,2) for x in w],round(nr[1],1))
    parts[idx[n]]=set_transform(parts[idx[n]],w,nr)
parts[idx['AEG030_518_9000']]=set_transform(parts[idx['AEG030_518_9000']],NEW_P,(orot[0],NEW_DEG,orot[2]))
new=write_msb(h,L)
assert write_msb(*read_msb(raw))==raw
c=zlib.compress(new,9)
hdr=bytearray(d[:0x4c]); struct.pack_into('>II',hdr,0x1c,len(new),len(c))
open(dst,'wb').write(bytes(hdr)+c)
# verify
d2=open(dst,'rb').read(); assert zlib.decompress(d2[0x4c:])==new
print('ok',len(d2))
