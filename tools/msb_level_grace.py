"""One-off (2026-10-20): level the First Step grace parts on the street floor, put the rest spot beside the grace and
stretch the Unicorn Way name regions over the whole street, using the street part's current position.
"""
import sys,struct,zlib,math
sys.path.insert(0,'/home/claude/eldenring-w101/tools')
from msb_floor import read_msb,write_msb,entry_name,transform,set_transform,_list
def R(v,deg):
    c,s=math.cos(math.radians(deg)),math.sin(math.radians(deg)); x,y,z=v; return (x*c+z*s,y,-x*s+z*c)
src='/mnt/user-data/uploads/game_modding/reference/eldenring/project/map/mapstudio/m60_42_36_00.msb.dcx'
d=open(src,'rb').read(); raw=zlib.decompress(d[0x4c:]); h,L=read_msb(raw)
P=_list(L,'PARTS_PARAM_ST').entries; idx={entry_name(e):i for i,e in enumerate(P)}
(sp,srot)=transform(P[idx['AEG030_518_9000']]); deg=srot[1]
w=lambda flv:tuple(sp[i]+R(flv,deg)[i] for i in range(3))
# grace: on the floor where Ryan put it (floor = street height -0.01 there)
gp,grot=transform(P[idx['AEG099_090_9011']]); floor=sp[1]-0.01
for n in ('AEG099_090_9003','AEG099_090_9006','AEG099_090_9008','AEG099_090_9011'):
    p,r=transform(P[idx[n]]); P[idx[n]]=set_transform(P[idx[n]],(p[0],floor+0.1,p[2]),None)
p,r=transform(P[idx['c1000_9001']]); P[idx['c1000_9001']]=set_transform(P[idx['c1000_9001']],(p[0],floor-0.4,p[2]),None)
# rest spot: vanilla offset from the grace (1.33,-0.1,-2.2 at grace yaw 166), turned with the grace
turn=grot[1]-166.0; off=R((1.33,0,-2.2),turn)
P[idx['c0000_9003']]=set_transform(P[idx['c0000_9003']],(gp[0]+off[0],floor,gp[2]+off[2]),(0.0,(172.0+turn+180)%360-180,0.0))
for n in ('AEG099_090_9011','c1000_9001','c0000_9003'): print(n,[round(x,2) for x in transform(P[idx[n]])[0]],round(transform(P[idx[n]])[1][1],1))
# name regions: one box over the whole street (OBJ floor spans x -353..70, z -231..21)
Rg=_list(L,'POINT_PARAM_ST').entries
centre_obj=(-141.0,-105.0); c=w((centre_obj[0],-20.0,-centre_obj[1]))
for i,e in enumerate(Rg):
    if struct.unpack_from('<I',e,8)[0]==51:
        e=bytearray(e); struct.pack_into('<3f',e,0x14,*c); struct.pack_into('<3f',e,0x20,0.0,deg,0.0)
        so=struct.unpack_from('<q',e,0x48)[0]; struct.pack_into('<3f',e,so,450.0,280.0,90.0)
        Rg[i]=bytes(e); print(entry_name(e),[round(x,1) for x in c])
out=write_msb(h,L); cz=zlib.compress(out,9); hd=bytearray(d[:0x4c]); struct.pack_into('>II',hd,0x1c,len(out),len(cz))
open('/mnt/project-files/tmp_ev/m60_42_36_00.msb.dcx','wb').write(bytes(hd)+cz)
