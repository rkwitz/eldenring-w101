"""Text files (FMG inside BND4 .msgbnd) for Elden Ring, stdlib only: read and write FMG v2, replace one file in a BND4.

Usage: python er_text.py item.msgbnd out.msgbnd
  adds PlaceName 610900 "Unicorn Way" (the name the street's MapNameOverride regions in m60_42_36_00 show). Input is an
  uncompressed msgbnd from blender_unpack.py; compress the output as DFLT DCX for the project's msg/engus folder.
"""
import struct
def bnd4(b):
    n=struct.unpack_from('<i',b,0x0C)[0]; hs=struct.unpack_from('<q',b,0x20)[0]
    unicode=b[0x30]; out={}
    for k in range(n):
        o=0x40+k*hs
        csize=struct.unpack_from('<q',b,o+8)[0]
        usize=struct.unpack_from('<q',b,o+16)[0] if hs>=0x24 else csize
        doff,fid,noff=struct.unpack_from('<IiI',b,o+(24 if hs>=0x24 else 16))
        e=noff
        if unicode:
            while b[e:e+2]!=b'\0\0': e+=2
            name=b[noff:e].decode('utf-16-le')
        else:
            e=b.index(b'\0',noff); name=b[noff:e].decode('shift-jis')
        out[name]=b[doff:doff+csize]
    return out

def replace_entry(b, suffix, data):
    """BND4 bytes with the file whose name ends in `suffix` replaced by `data` (appended at the end, 16-aligned)."""
    b=bytearray(b); n=struct.unpack_from('<i',b,0x0C)[0]; hs=struct.unpack_from('<q',b,0x20)[0]
    names=list(bnd4(bytes(b)).keys())
    k=next(i for i,nm in enumerate(names) if nm.endswith(suffix))
    o=0x40+k*hs
    b+=b'\0'*((-len(b))%16); at=len(b); b+=data
    struct.pack_into('<qq',b,o+8,len(data),len(data))
    struct.pack_into('<I',b,o+24,at)
    return bytes(b)

import struct
def read_fmg(b):
    gc,sc=struct.unpack_from('<ii',b,0x0C); so=struct.unpack_from('<q',b,0x18)[0]
    out={}
    for g in range(gc):
        idx,first,last,_=struct.unpack_from('<iiii',b,0x28+16*g)
        for k,i in enumerate(range(first,last+1)):
            off=struct.unpack_from('<q',b,so+8*(idx+k))[0]
            if off:
                e=off
                while b[e:e+2]!=b'\0\0': e+=2
                out[i]=b[off:e].decode('utf-16-le')
            else: out[i]=None
    return out

def write_fmg(entries, template):
    """FMG v2 bytes for {id: text or None}, header flags copied from `template`."""
    ids=sorted(entries); groups=[]
    for i in ids:
        if groups and groups[-1][2]==i-1: groups[-1][2]=i
        else: groups.append([None,i,i])
    k=0
    for g in groups: g[0]=k; k+=g[2]-g[1]+1
    so=0x28+16*len(groups); strings_at=so+8*len(ids)
    out=bytearray(template[:0x28]); body=bytearray(); offs=[]
    for i in ids:
        t=entries[i]
        if t is None: offs.append(0)
        else: offs.append(strings_at+len(body)); body+=(t+'\0').encode('utf-16-le')
    for g in groups: out+=struct.pack('<iiii',g[0],g[1],g[2],0)
    for o in offs: out+=struct.pack('<q',o)
    out+=body
    struct.pack_into('<i',out,4,len(out)); struct.pack_into('<ii',out,0x0C,len(groups),len(ids)); struct.pack_into('<q',out,0x18,so)
    return bytes(out)


PLACE_NAME_ID = 610900


def add_place_name(msgbnd, text_id=PLACE_NAME_ID, text="Unicorn Way"):
    name = next(k for k in bnd4(msgbnd) if k.endswith("\\PlaceName.fmg"))
    old = bnd4(msgbnd)[name]
    entries = read_fmg(old)
    entries[text_id] = text
    return replace_entry(msgbnd, "\\PlaceName.fmg", write_fmg(entries, old))


if __name__ == "__main__":
    import sys
    open(sys.argv[2], "wb").write(add_place_name(open(sys.argv[1], "rb").read()))
