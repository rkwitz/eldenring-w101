"""Read Elden Ring event scripts (EMEVD, uncompressed) and insert instructions into them (stdlib only).

Usage: python emevd.py m18_00_00_00.emevd out.emevd
  closes the Stranded Graveyard exit door (AEG027_002_0504, entity 18001511) on every map load: event 0 starts the
  door's common event 90005501 with open-flag 18000510, so a SetEventFlag(18000510, OFF) (2003[66]) goes right
  before it. The output still needs DCX compression (DFLT, as Smithbox saves) to go in the project's event folder.
"""
import struct
class Emevd:
    def __init__(s,d):
        s.d=d; h=struct.unpack_from('<16q',d,0x10)
        s.ne,s.eo,s.ni,s.io=h[0],h[1],h[2],h[3]; s.npar,s.paro=h[8],h[9]; s.argo=h[13]
        s.events=[]
        for k in range(s.ne):
            eid,ic,io,pc,po,rest,_=struct.unpack_from('<qqqqqII',d,s.eo+k*48)
            ins=[]
            for j in range(ic):
                o=s.io+io+j*32 if io<s.io else io+j*32
                bank,iid,al,ao,lo=struct.unpack_from('<iiqqq',d,s.io+(io//1)+j*32) if False else struct.unpack_from('<iiqqq',d,o)
                ins.append(dict(off=o,bank=bank,id=iid,args=d[s.argo+ao:s.argo+ao+al],argoff=s.argo+ao,layer=lo))
            pars=[]
            for j in range(pc):
                pars.append(struct.unpack_from('<qqqii',d,po+j*32 if po>=s.paro else s.paro+po+j*32))
            s.events.append(dict(id=eid,ins=ins,pars=pars,rest=rest,off=s.eo+k*48))


def insert_instruction(d, event_index, position, bank, iid, args):
    """EMEVD bytes with one instruction inserted into event `event_index` at `position`."""
    h=list(struct.unpack_from('<16q',d,0x10))
    ne,eo,ni,io=h[0],h[1],h[2],h[3]; argo,argl=h[13],h[12]; paro=h[9]
    assert argl%4==0
    events=bytearray(d[eo:eo+48*ne])
    eid,ic,eio,pc,po,rest,pad=struct.unpack_from('<qqqqqII',events,48*event_index)
    abs_index=eio//32+position
    struct.pack_into('<qqqqqII',events,48*event_index,eid,ic+1,eio,pc,po,rest,pad)
    for k in range(event_index+1,ne):
        f=list(struct.unpack_from('<qqqqqII',events,48*k)); f[2]+=32; struct.pack_into('<qqqqqII',events,48*k,*f)
    instr=bytearray(d[io:io+32*ni])
    new=struct.pack('<iiqqq',bank,iid,len(args),argl,-1)
    instr[abs_index*32:abs_index*32]=new
    args_block=d[argo:argo+argl]+args
    pad=(-len(args_block))%16
    tail_start=argo+argl  # params, linked files, strings follow
    tail_start=paro
    assert d[io+32*ni:argo]==b'' , 'unexpected gap between instructions and args'
    out=bytearray(d[:eo])+events+d[eo+48*ne:io]+instr
    new_argo=len(out); out+=args_block+b'\0'*pad
    shift=len(out)-paro
    tail=d[paro:]
    out+=tail
    h[2]=ni+1; h[3]=io
    for i in (5,7): h[i]=new_argo if h[i]==argo else h[i]
    h[12]=argl+len(args); h[13]=new_argo
    for i in (9,11,15): h[i]+=shift
    struct.pack_into('<16q',out,0x10,*h)
    struct.pack_into('<I',out,0x0C,len(out))
    return bytes(out)


DOOR_FLAG = 18000510


def close_graveyard_door(d):
    e = Emevd(d)
    event0 = e.events[0]
    at = next(k for k, i in enumerate(event0["ins"])
              if (i["bank"], i["id"]) == (2000, 6) and struct.unpack_from("<i", i["args"], 8)[0] == DOOR_FLAG)
    return insert_instruction(d, 0, at, 2003, 66, struct.pack("<BxxxIBxxx", 0, DOOR_FLAG, 0))


if __name__ == "__main__":
    import sys
    data = open(sys.argv[1], "rb").read()
    open(sys.argv[2], "wb").write(close_graveyard_door(data))
