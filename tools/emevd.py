"""Read Elden Ring event scripts (EMEVD, uncompressed) and insert instructions into them (stdlib only).

Usage: python emevd.py m18_00_00_00.emevd out.emevd
  closes the Stranded Graveyard exit door on every map load and every rest (see close_graveyard_door). The output still
  needs DCX compression (DFLT, as Smithbox saves) to go in the project's event folder.
python emevd.py --no-guidance m60_42_36_00.emevd out.emevd
  removes the Guidance of Grace light wisps at The First Step and Church of Elleh (see no_guidance_wisps).
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
                o=s.io+io+j*32  # instruction and parameter offsets are relative to their tables
                bank,iid,al,ao,lo=struct.unpack_from('<iiqqq',d,o)
                ins.append(dict(off=o,bank=bank,id=iid,args=d[s.argo+ao:s.argo+ao+al],argoff=s.argo+ao,layer=lo))
            pars=[]
            for j in range(pc):
                pars.append(struct.unpack_from('<qqqii',d,s.paro+po+j*32))
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


DOOR_FLAGS = (18000510, 18000511)  # the door's open state, and "opened" (set when it was pulled open)
DOOR_EVENT = 18002990  # new map event, free in m18


def _flag_off(flag):
    return {"bank": 2003, "id": 66, "args": struct.pack("<BxxxIBxxx", 0, flag, 0), "layer": -1}


def close_graveyard_door(d):
    """The Stranded Graveyard exit door (asset 18001510) closed on every load and every rest at a grace.

    Vanilla starts its two common events from event 0 (90005501: shows the saved state) and event 18002510 (90005500:
    the pull-open interaction). Both move into a new event that restarts on rest and first clears the door flags; the
    old calls are replaced in place (no lines removed, so no skip counts change)."""
    e = Emevd(d)
    events = events_of(e)
    by_id = {ev["id"]: ev for ev in events}

    def take(event_id, common_event, replacement):
        ins = by_id[event_id]["ins"]
        k = next(k for k, i in enumerate(ins) if (i["bank"], i["id"]) == (2000, 6)
                 and struct.unpack_from("<iI", i["args"], 4) == (common_event, DOOR_FLAGS[0]))
        original, ins[k] = ins[k], replacement
        return original

    start_ours = {"bank": 2000, "id": 0, "args": struct.pack("<iIi", 0, DOOR_EVENT, 0), "layer": -1}
    show_state = take(0, 90005501, start_ours)
    interact = take(18002510, 90005500, _flag_off(DOOR_FLAGS[0]))
    body = [_flag_off(f) for f in DOOR_FLAGS] + [show_state, interact]
    events.append({"id": DOOR_EVENT, "rest": 1, "pars": [], "ins": body})  # rest 1 = restart when resting
    return build(e, events)


def reset_flags_on_load(d, event_id, flags):
    """A new event `event_id` that turns `flags` off on every map load and every rest at a grace (restart on rest),
    started from the top of event 0 so no skip counts change. Used for the Stranded Graveyard exit door: the lift-up
    door AEG110_064_1000 in m60_42_36_00 is an ObjAct whose opened state is event flag 1042368540."""
    e = Emevd(d)
    events = events_of(e)
    start = {"bank": 2000, "id": 0, "args": struct.pack("<iIi", 0, event_id, 0), "layer": -1}
    next(ev for ev in events if ev["id"] == 0)["ins"].insert(0, start)
    events.append({"id": event_id, "rest": 1, "pars": [], "ins": [_flag_off(f) for f in flags]})
    return build(e, events)


def title_on_door_exit(d, event_id=1042362991, door_flag=1042368540, area=1042362700, place_name=610900):
    """A new event `event_id` (restart on rest) that waits until the door's open flag is on while the player (10000) is
    inside region `area` (both at once, condition group 1, so a flag left on from the save before the reset event clears
    it can't count), then shows PlaceName `place_name` as the area welcome card (2007[14], as m11_05 does for Leyndell).
    Started from event 0 right after the door reset event 1042362990 (or at the top if that is absent). For the Unicorn Way card on walking out of the First Step lift-up door: the
    MapNameOverride box alone showed it only after a warp/load."""
    e = Emevd(d)
    events = events_of(e)
    assert all(ev["id"] != event_id for ev in events), f"event {event_id} already exists"
    start = {"bank": 2000, "id": 0, "args": struct.pack("<iIi", 0, event_id, 0), "layer": -1}
    ins0 = next(ev for ev in events if ev["id"] == 0)["ins"]
    reset = [k for k, i in enumerate(ins0) if (i["bank"], i["id"]) == (2000, 0)
             and struct.unpack_from("<I", i["args"], 4)[0] == 1042362990]
    ins0.insert(reset[0] + 1 if reset else 0, start)
    body = [
        {"bank": 3, "id": 0, "args": struct.pack("<bBBxI", 1, 1, 0, door_flag), "layer": -1},  # IF Event Flag ON
        {"bank": 3, "id": 2, "args": struct.pack("<bBxxIIi", 1, 1, 10000, area, 1), "layer": -1},  # IF player in area
        {"bank": 0, "id": 0, "args": struct.pack("<bBbx", 0, 1, 1), "layer": -1},  # IF Condition Group 1
        {"bank": 2007, "id": 14, "args": struct.pack("<i", place_name), "layer": -1},  # Display Area Welcome Message
    ]
    events.append({"id": event_id, "rest": 1, "pars": [], "ins": body})
    return build(e, events)


GUIDANCE_GRACES = (1042361981, 1042361980)  # AEG099_090 grace markers of The First Step (76101) and Church of Elleh (76100)
NO_ENTITY = 1042361989  # unused entity id in this tile (not in m60_42_36_00 or m60_10_09_02)


def no_guidance_wisps(d, markers=GUIDANCE_GRACES, dummy=NO_ENTITY):
    """Event 0's calls of common event 90005100 (grace with Guidance of Grace) for the grace `markers` get entity
    `dummy` instead. 90005100 uses that argument (X8) only for the golden guidance-light SFX 6400, which it attaches to
    the marker's dummy poly 100 when the grace is lit (2006[04]) and removes with 2006[05]; the guidance flags 78100..
    (WorldMapPointParam 95100/95101: the golden line on the world map), the grace itself and the first-rest tutorial
    keep working. SFX are not saved, so this needs no flag reset."""
    e = Emevd(d)
    events = events_of(e)
    hits = 0
    for i in next(ev for ev in events if ev["id"] == 0)["ins"]:
        if (i["bank"], i["id"]) == (2000, 6) and struct.unpack_from("<i", i["args"], 4)[0] == 90005100:
            if struct.unpack_from("<I", i["args"], 16)[0] in markers:
                a = bytearray(i["args"]); struct.pack_into("<I", a, 16, dummy); i["args"] = bytes(a); hits += 1
    assert hits == len(markers), f"found {hits} of {len(markers)} 90005100 calls"
    return build(e, events)


def build(e, events):
    """EMEVD bytes from `events` [{id, rest, ins: [{bank, id, args, layer}], pars: [(index, target, source, size, unk)]}]
    with the header flags, linked files and strings of the parsed file `e` (no layers)."""
    d = e.d
    h = list(struct.unpack_from("<16q", d, 0x10))
    linked, strings = d[h[11]:h[11] + 8 * h[10]], d[h[15]:h[15] + h[14]]
    n_ins = sum(len(ev["ins"]) for ev in events)
    events_at = 0x90
    ins_at = events_at + 48 * len(events)
    args_at = ins_at + 32 * n_ins
    table, instr, args, params = bytearray(), bytearray(), bytearray(), bytearray()
    for ev in events:
        table += struct.pack("<qqqqqII", ev["id"], len(ev["ins"]), len(instr), len(ev["pars"]),
                             len(params) if ev["pars"] else -1, ev["rest"], 0)
        for i in ev["ins"]:
            instr += struct.pack("<iiqqq", i["bank"], i["id"], len(i["args"]), len(args) if i["args"] else -1, i["layer"])
            args += i["args"] + b"\0" * ((-len(i["args"])) % 4)
        for p in ev["pars"]:
            params += struct.pack("<qqqii", *p)
    args += b"\0" * ((-len(args)) % 16)
    args_len = len(args)  # the game's own files count the padding
    params_at = args_at + len(args)
    linked_at = params_at + len(params)
    strings_at = linked_at + len(linked)
    out = bytearray(d[:0x90]) + table + instr + args + params + linked + strings
    h[0:4] = [len(events), events_at, n_ins, ins_at]
    h[5] = h[7] = h[13] = args_at
    h[8], h[9], h[11], h[12], h[15] = sum(len(ev["pars"]) for ev in events), params_at, linked_at, args_len, strings_at
    struct.pack_into("<16q", out, 0x10, *h)
    struct.pack_into("<I", out, 0x0C, len(out))
    return bytes(out)


def events_of(e):
    return [{"id": ev["id"], "rest": ev["rest"], "pars": [tuple(p) for p in ev["pars"]],
             "ins": [{k: i[k] for k in ("bank", "id", "args", "layer")} for i in ev["ins"]]} for ev in e.events]


if __name__ == "__main__":
    import sys
    if sys.argv[1] == "--no-guidance":  # emevd.py --no-guidance m60_42_36_00.emevd out.emevd
        open(sys.argv[3], "wb").write(no_guidance_wisps(open(sys.argv[2], "rb").read()))
        sys.exit()
    data = open(sys.argv[1], "rb").read()
    open(sys.argv[2], "wb").write(close_graveyard_door(data))
