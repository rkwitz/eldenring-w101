"""Add two MapNameOverride regions (type 51) over the Unicorn Way plaza in m60_42_36_00: TextID 610900 (shown on map load)
and -610900 (on entering), as boxes copied from region hitset_h423601. Edit the paths below before running.
"""
import sys,struct,zlib,math
sys.path.insert(0,'/home/claude/eldenring-w101/tools')
from msb_floor import read_msb,write_msb,entry_name,_list
TEXT_ID=610900
d=open('/tmp/claude-0/t.dcx','rb').read(); raw=zlib.decompress(d[0x4c:])
h,L=read_msb(raw); R=_list(L,'POINT_PARAM_ST')
tmpl=next(e for e in R.entries if entry_name(e)=='hitset_h423601')
ids=[struct.unpack_from('<i',e,0x2C)[0] for e in R.entries]; print('region ids',min(ids),max(ids))
def R4(v,deg):
    c,s=math.cos(math.radians(deg)),math.sin(math.radians(deg)); x,y,z=v; return (x*c+z*s,y,-x*s+z*c)
P=(6.1348,89.59,-86.6426); DEG=4.0
off=R4((0,0,17.0),DEG); centre=(P[0]+off[0],P[1]-5,P[2]+off[2])
def region(name,text_id,type_index,region_id):
    e=bytearray(tmpl)
    nm=(name+'\0').encode('utf-16-le'); nm+=b'\0'*((-(0x68+len(nm)))%8)
    head=bytearray(e[:0x68]); after_name=e[0x88:0xb0]  # base data, shape, entity data (relative layout kept)
    shift=0x68+len(nm)-0x88
    body=bytearray(head)+nm+after_name
    td=len(body); body+=struct.pack('<ii',text_id,0)
    u4=len(body); body+=tmpl[0xb4:]
    struct.pack_into('<I',body,0x08,51); struct.pack_into('<i',body,0x0C,type_index)
    struct.pack_into('<3f',body,0x14,*centre); struct.pack_into('<3f',body,0x20,0.0,DEG,0.0)
    struct.pack_into('<i',body,0x2C,region_id)
    for f in (0x30,0x38,0x48,0x50):
        struct.pack_into('<q',body,f,struct.unpack_from('<q',tmpl,f)[0]+shift)
    struct.pack_into('<q',body,0x58,td); struct.pack_into('<q',body,0x60,u4)
    so=struct.unpack_from('<q',body,0x48)[0]; struct.pack_into('<3f',body,so,90.0,90.0,40.0)
    body+=b"\0"*((-len(body))%8)
    return bytes(body)
new=[region('UnicornWay_name_load',TEXT_ID,0,max(ids)+1), region('UnicornWay_name_enter',-TEXT_ID,1,max(ids)+2)]
R.entries+=new
out=write_msb(h,L)
for e in new:
    print(entry_name(e), struct.unpack_from('<qIiI3f3fiqqiIqqqq',e,0), e[struct.unpack_from('<q',e,0x58)[0]:][:8].hex())
c=zlib.compress(out,9); hd=bytearray(d[:0x4c]); struct.pack_into('>II',hd,0x1c,len(out),len(c))
open('/mnt/project-files/tmp_msg/m60_42_36_00.msb.dcx','wb').write(bytes(hd)+c)
h2,L2=read_msb(zlib.decompress((bytes(hd)+c)[0x4c:])); print(len(_list(L2,'POINT_PARAM_ST').entries))
