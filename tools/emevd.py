"""Read Elden Ring event scripts (EMEVD, uncompressed) and insert instructions into them (stdlib only).

Usage: python emevd.py m18_00_00_00.emevd out.emevd
  closes the Stranded Graveyard exit door on every map load and every rest (see close_graveyard_door). The output still
  needs DCX compression (DFLT, as Smithbox saves) to go in the project's event folder.
python emevd.py --no-guidance m60_42_36_00.emevd out.emevd
  removes the Guidance of Grace light wisps at The First Step and Church of Elleh (see no_guidance_wisps).
python emevd.py --no-leaf-gusts common.emevd out.emevd
  removes the overworld's gusts of glowing orange leaves (see no_leaf_gusts).
python emevd.py --garden m60_42_36_00.emevd out.emevd
  the hedge-maze garden's door warps (see garden_doors; regions from msb_add_garden.py).
python emevd.py --rattlebones m60_42_36_00.emevd out.emevd
  Rattlebones' room door warps; the room's door only lets you out once he is dead (see rattlebones_doors).
python emevd.py --rattlebones-boss m60_42_36_00.emevd out.emevd
  Rattlebones held on his sigil until you walk in, and his boss health bar (see boss_bar; msb_add_boss.py).
python emevd.py --interiors m60_42_36_00.emevd out.emevd
  door warps of Lady Blackhope's tower and Fear of Fire's house (see interior_doors; msb_add_interiors.py).
python emevd.py --bosses m60_42_36_00.emevd out.emevd
  all door warps (rideable up to the door, load-screen flag, camera turned) and all four bosses (see all_bosses).
python emevd.py --no-leaves common.emevd|m60.emevd out.emevd
  every falling-leaf source of those events off, in place (see no_leaves).
python emevd.py --midday m60_42_36_00.emevd out.emevd [first_id second_id]
  always 12:00 (see midday; m60.emevd takes ids 1060002990 1060002991).
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


RATTLEBONES = 1042360850  # msb_add_boss.py: Giant Skeleton Torso c4960_9000 on the room's sigil
RATTLEBONES_DOORS = (  # (event id, trigger region, arrival region[, character that must be dead]): msb_add_room.py
    (1042362810, 1042362800, 1042362801),  # street tower door -> into Rattlebones' room
    (1042362811, 1042362802, 1042362803, RATTLEBONES),  # the room's door -> back on Unicorn Way, once he is dead
)


LOAD_SCREEN_FLAG = 1042366900  # set right before every door warp; the HUD overlay shows its load screen and clears it
STREET_YAW = -27.011917  # AEG030_518_9000 (the street) and every room part are turned by this about y
ARRIVAL_YAW = {  # arrival region -> its rotation y in m60_42_36_00 (msb_add_room / _garden / _interiors facings):
    # in game the player faces a region's rotation, and the camera yaw of 2008[4] uses the same convention (vanilla
    # common event 950 turns the camera to about its Melina regions' rotation); every arrival faces away from its door
    1042362801: STREET_YAW + 180.0, 1042362803: STREET_YAW,  # Rattlebones in (into the room) / out (onto the street)
    1042362805: STREET_YAW + 180.0, 1042362807: STREET_YAW - 91.1,  # garden in / out
    1042362817: STREET_YAW + 180.0, 1042362819: STREET_YAW + 180.0,  # Lady Blackhope in / out
    1042362821: STREET_YAW, 1042362823: STREET_YAW,  # Fear of Fire in / out
}
CAMERA_PITCH = 5.0  # as common event 950 after its warp
LOAD_SCREEN_LEAD = 0.75  # seconds between LOAD_SCREEN_FLAG on and the warp, so the HUD's load screen is up first
ARRIVAL_SETTLE = 1.0  # seconds after the warp before the door listens again


def rattlebones_doors(d, doors=RATTLEBONES_DOORS):
    """New events (one per door, started from the top of event 0) that wait for the player (10000) to be in the
    door's trigger region (3[2], as title_on_door_exit), mounted or not: Ryan rides up to a door and through it, and
    the no-riding boxes inside the rooms (msb_add_indoor.py) dismount him on arrival; the street-side door boxes are
    gone (2026-10-25). Then set LOAD_SCREEN_FLAG (2003[66]) for the HUD's load screen, wait LOAD_SCREEN_LEAD seconds
    (1001[0]) so the screen is up before anything moves (the instant warp showed none, 2026-10-25), and move the
    player to the arrival region with 2004[74] "Warp Character and Copy Floor with Fade-Out" (warp type 1 = region,
    dummy poly -1, floor copied from the player, no bonfire effect, camera reset behind him; packed as vanilla common
    event 6901 does): unlike the short warp 2004[41] it also reaches the garden 117 m off. After the warp, 2008[4]
    SetCameraAngle (pitch, world yaw; as common event 950 does after its warp) turns the camera to look where the
    arrival region faces (ARRIVAL_YAW), away from the door just come through, so holding forward walks on into the
    room instead of back into the other side's trigger. Wait ARRIVAL_SETTLE seconds and restart (1000[4] with 1),
    like Wizard101's door activators. A door with a 4th entry (a boss's entity id) only works while that character is
    dead (4[0] dead in the same condition group 1): the player can't leave the boss room until the boss is beaten
    (he is back after a rest, and so is the lock). Existing events with these ids are replaced, so running it again
    is harmless."""
    e = Emevd(d)
    ids = {door[0] for door in doors}
    events = [ev for ev in events_of(e) if ev["id"] not in ids]
    ins0 = next(ev for ev in events if ev["id"] == 0)["ins"]
    ins0[:] = [i for i in ins0 if not ((i["bank"], i["id"]) == (2000, 0)
                                      and struct.unpack_from("<I", i["args"], 4)[0] in ids)]
    for slot, (event_id, trigger, arrival, *boss) in enumerate(doors):
        ins0.insert(slot, {"bank": 2000, "id": 0, "args": struct.pack("<iIi", 0, event_id, 0), "layer": -1})
        wait = [
            _i(3, 2, "<bBxxIIi", 1, 1, 10000, trigger, 1),  # IF player in the door's area (group 1)
        ]
        if boss:
            wait.append(_i(4, 0, "<bxxxIBBxxf", 1, boss[0], 1, 0, 1.0))  # IF the boss is dead (group 1)
        body = wait + [
            _i(0, 0, "<bBbx", 0, 1, 1),  # IF condition group 1
            _i(2003, 66, "<BxxxIBxxx", 0, LOAD_SCREEN_FLAG, 1),  # load screen flag on (the HUD clears it)
            _i(1001, 0, "<f", LOAD_SCREEN_LEAD),  # wait for the HUD's screen
            _i(2004, 74, "<IBxxxIiIBBxx", 10000, 1, arrival, -1, 10000, 0, 1),  # fade out, warp to region
        ]
        if arrival in ARRIVAL_YAW:  # camera looking where the arrival faces
            body.append(_i(2008, 4, "<ff", CAMERA_PITCH, (ARRIVAL_YAW[arrival] + 180.0) % 360.0 - 180.0))
        body.append(_i(1001, 0, "<f", ARRIVAL_SETTLE))
        body.append(_i(1000, 4, "<Bxxx", 1))  # End Unconditionally: restart
        events.append({"id": event_id, "rest": 0, "pars": [], "ins": body})
    return build(e, events)


def _i(bank, iid, fmt, *args):
    return {"bank": bank, "id": iid, "args": struct.pack(fmt, *args), "layer": -1}


GARDEN_DOORS = (  # msb_add_garden.py's regions: the hedge-maze double door on Unicorn Way and the garden's door
    (1042362812, 1042362804, 1042362805),  # street double door -> into the garden
    (1042362813, 1042362806, 1042362807),  # the garden's door -> back out on Unicorn Way
)


def garden_doors(d):
    """The hedge-maze garden's two door warps, events like rattlebones_doors (rerunning replaces them)."""
    return rattlebones_doors(d, GARDEN_DOORS)


INTERIOR_DOORS = (  # msb_add_interiors.py's regions: the other boss interiors, empty for now
    (1042362816, 1042362816, 1042362817),  # Lady Blackhope's tower door (T1) -> into her room
    (1042362817, 1042362818, 1042362819),  # her room's door -> back on Unicorn Way
    (1042362818, 1042362820, 1042362821),  # Fear of Fire's house door (H4) -> into the clubhouse
    (1042362819, 1042362822, 1042362823),  # the clubhouse door -> back on Unicorn Way
)


def interior_doors(d):
    """Door warps of Lady Blackhope's tower and Fear of Fire's house, events like rattlebones_doors (no boss lock yet;
    add the boss's entity id as a 4th entry once one is placed). Rerunning replaces them."""
    return rattlebones_doors(d, INTERIOR_DOORS)


BOSS_BAR_EVENT = 1042362815
RATTLEBONES_ARENA = 1042362808  # msb_add_boss.py: sphere over the whole room
RATTLEBONES_SIGIL = 1042362809  # msb_add_boss.py: point on the room's sigil, facing the door
RATTLEBONES_NAME = 904969000  # NpcName "Rattlebones" (er_text.py --npc-names)
SERAPH = 1042360853
# (bar event, boss entity, arena region, sigil region, NpcName id, animation on arrival or None, DispMask bits on):
# msb_add_boss.BOSSES; names from er_text.NPC_NAMES. Rattlebones (Giant Skeleton Torso, Mohgwyn rows) gets the
# Mohgwyn torsos' wake animation 20010 (m12_05 event 12052250 plays it on c4960 after standby 30010); Seraph (Malenia)
# gets what m15 event 15002810 does when her fight starts: DispMask bits 0, 12 and 13 on and animation 3006.
BOSSES = (
    (BOSS_BAR_EVENT, RATTLEBONES, RATTLEBONES_ARENA, RATTLEBONES_SIGIL, RATTLEBONES_NAME, 20010, ()),
    (1042362820, 1042360851, 1042362824, 1042362825, 904969001, None, ()),  # Lady Blackhope (Nox Swordstress)
    (1042362821, 1042360852, 1042362826, 1042362827, 904969002, None, ()),  # Fear of Fire (Mad Pumpkin Head)
    (1042362822, SERAPH, 1042362828, 1042362829, 904969003, 3006, (0, 12, 13)),  # Seraph (Malenia)
)
BOSS_SETUP_EVENT = 1042362814
PHASE_ONE_EVENT = 1042362823


def _start_once(events, event_id):
    """Drop `event_id` from `events` and from event 0's starts, then start it from the top of event 0."""
    events[:] = [ev for ev in events if ev["id"] != event_id]
    ins0 = next(ev for ev in events if ev["id"] == 0)["ins"]
    ins0[:] = [i for i in ins0 if not ((i["bank"], i["id"]) == (2000, 0)
                                      and struct.unpack_from("<I", i["args"], 4)[0] == event_id)]
    ins0.insert(0, _i(2000, 0, "<iIi", 0, event_id, 0))


def boss_bar(d, event_id=BOSS_BAR_EVENT, boss=RATTLEBONES, arena=RATTLEBONES_ARENA, sigil=RATTLEBONES_SIGIL,
             name=RATTLEBONES_NAME, animation=None, disp_masks=()):
    """A new event `event_id` (restarts on rest, started from the top of event 0) for the boss in his room (held
    disabled and without gravity by boss_setup until then):
      wait until the boss is alive AND the player (10000) is in the arena (condition group 1)
      2004[5] enable him, 2004[41] short warp onto region `sigil` (as Margit's event moves him), 2004[10] gravity on
      2004[26] DispMask bits `disp_masks` on, 2003[18] ForceAnimation `animation` (packed as m15 15002810's 3006)
      2003[11] DisplayBossHealthBar(1, boss, slot 0, name), packed as vanilla (m10_00 event 10002810, Margit)
      wait until the player has left the arena OR the boss is dead (condition group -1)
      2003[11] DisplayBossHealthBar(0, ...) and restart (walking back in puts a living boss back on the sigil)
    4[0] IfCharacterDeadAlive(group, entity, 0 alive / 1 dead, comparison 0 "=", 1.0) and 3[2] are packed as in
    vanilla events. The boss is an ordinary enemy (no defeat flag), so it is back after a rest like any other and the
    bar works again. Rerunning replaces the event."""
    e = Emevd(d)
    events = events_of(e)
    _start_once(events, event_id)
    body = [
        _i(4, 0, "<bxxxIBBxxf", 1, boss, 0, 0, 1.0),  # IF boss alive (group 1)
        _i(3, 2, "<bBxxIIi", 1, 1, 10000, arena, 1),  # IF player inside the arena (group 1)
        _i(0, 0, "<bBbx", 0, 1, 1),  # IF condition group 1
        _i(2004, 5, "<IBxxx", boss, 1),  # enable him
        _i(2004, 41, "<IIIi", boss, 1, sigil, -1),  # warp him onto the sigil
        _i(2004, 10, "<IBxxx", boss, 1),  # gravity on
    ]
    body += [_i(2004, 26, "<IBBxx", boss, bit, 1) for bit in disp_masks]
    if animation is not None:
        body.append(_i(2003, 18, "<IiBBBbf", boss, animation, 0, 0, 0, 0, 1.0))
    body += [
        _i(2003, 11, "<BxxxIhxxi", 1, boss, 0, name),  # show the boss bar
        _i(3, 2, "<bBxxIIi", -1, 0, 10000, arena, 1),  # IF player outside the arena (OR group -1)
        _i(4, 0, "<bxxxIBBxxf", -1, boss, 1, 0, 1.0),  # IF boss dead (OR group -1)
        _i(0, 0, "<bBbx", 0, 1, -1),  # IF condition group -1
        _i(2003, 11, "<BxxxIhxxi", 0, boss, 0, name),  # hide it
        _i(1000, 4, "<Bxxx", 1),  # restart
    ]
    events.append({"id": event_id, "rest": 1, "pars": [], "ins": body})
    return build(e, events)


def boss_setup(d, bosses=BOSSES, event_id=BOSS_SETUP_EVENT):
    """A new event `event_id` (restarts on rest, no loop) that disables every boss (2004[5] 0) and turns its gravity
    off (2004[10] 0) when the map loads: they wait unseen until boss_bar enables them on their sigil as the player
    walks in (spawning with the map, before the room's asset collision is there, a boss can fall through the floor;
    and the Seraph's alcove is within sight of the garden's centre). Rerunning replaces it."""
    e = Emevd(d)
    events = events_of(e)
    _start_once(events, event_id)
    body = []
    for b in bosses:
        body += [_i(2004, 5, "<IBxxx", b[1], 0), _i(2004, 10, "<IBxxx", b[1], 0)]
    events.append({"id": event_id, "rest": 1, "pars": [], "ins": body})
    return build(e, events)


def phase_one_only(d, boss=SERAPH, event_id=PHASE_ONE_EVENT):
    """Seraph is Malenia's phase 1 only. Vanilla switches to phase 2 from m15 event 15002811: 15002810 makes her
    immortal (2004[12]) at the start of the fight, and 15002811 waits for her HP <= 1 (4[14], packed as there) to run
    the Scarlet Aeonia cutscene and switch her DispMasks and SpEffects. Here nothing makes her immortal, so she dies at
    0 HP in phase 1; in case her row keeps her alive at 1 HP, this event (restarts on rest) waits for HP <= 1 and
    kills her (2004[4] ForceCharacterDeath, runes 1). Rerunning replaces it."""
    e = Emevd(d)
    events = events_of(e)
    _start_once(events, event_id)
    body = [
        _i(4, 14, "<bxxxIbxxxibxxxf", 0, boss, 5, 1, 0, 1.0),  # IF HP <= 1
        _i(2004, 4, "<IBxxx", boss, 1),  # force death, award runes
    ]
    events.append({"id": event_id, "rest": 1, "pars": [], "ins": body})
    return build(e, events)


def all_bosses(d):
    """Every door warp (load-screen flag, fade-out warp) and every boss of Unicorn Way: msb_add_boss.py's four."""
    for doors in (RATTLEBONES_DOORS, GARDEN_DOORS, INTERIOR_DOORS):
        d = rattlebones_doors(d, doors)
    for event_id, boss, arena, sigil, name, animation, masks in BOSSES:
        d = boss_bar(d, event_id, boss, arena, sigil, name, animation, masks)
    return phase_one_only(boss_setup(d))


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


LEAF_GUST_EVENT, LEAF_GUST_ASSET, LEAF_GUST_SFX = 1040, 1060001500, 806800


def no_leaf_gusts(d, dummy=NO_ENTITY):
    """common.emevd with the overworld leaf gusts gone. Common event 1040 (started from event 0, restarts on rest)
    loops: random wait, switch the weather wind to 808004 (strong), then 2006[04] attaches effect 806800 to dummy
    poly 200 of asset 1060001500 (AEG094_900_9900 in m60_00_00_99) for ~14 s. 806800 is a flipbook of tumbling
    orange leaves (texture s42071_a) with HDR gold colour keys, so the leaves read as falling embers. Both 2006[04]
    calls get the unused entity `dummy` instead, in place (same file size); wind, SpEffects 4200/4201 and the rest of
    the event are untouched. Applies to the whole overworld."""
    e = Emevd(d)
    old = struct.pack("<iii", LEAF_GUST_ASSET, 200, LEAF_GUST_SFX)
    out, hits = bytearray(d), 0
    for ev in e.events:
        for i in ev["ins"]:
            if (i["bank"], i["id"]) == (2006, 4) and i["args"] == old:
                assert ev["id"] == LEAF_GUST_EVENT
                struct.pack_into("<I", out, i["argoff"], dummy)
                hits += 1
    assert hits == 2, f"found {hits} leaf gust SFX calls, expected 2"
    return bytes(out)


LEAF_SFX = (806800, 806810)  # falling-leaf flipbooks AEG094_900_9900 (entity 1060001500) carries
GUST_WIND_SFX = 808004  # the strong wind of the gusts (2006[06] Set Wind SFX; -1 = none)
AMBIENT_SPEFFECTS = (4200, 4201, 4202)  # player SpEffects of the gusts (4200/4201) and of Limgrave north by day (4202)


def no_leaves(d, dummy=NO_ENTITY):
    """common.emevd or m60.emevd with every falling-leaf source of their events gone, in place (same file size; skip
    counts untouched):
      2006[04] CreateAssetFollowingSFX of LEAF_SFX on 1060001500 -> entity `dummy` (no SFX)
      2006[06] SetWindSFX 808004 -> -1
      2004[08] SetSpEffect AMBIENT_SPEFFECTS on the player (10000 / 20000) -> entity `dummy`
    Sources: common event 1040 (the gusts: between 0:00 and 3:30 in m60, wind 808004, leaves 806800, SpEffects
    4200/4201; no_leaf_gusts only did its SFX) and m60.emevd, which repeats that event as 1060002500 (not started),
    keeps leaves 806810 on 1060001500 whenever flag 9988 is on (1060002510, started from its event 0) and gives the
    player SpEffect 4202 in region group 1060002410 "緑北" (Limgrave north) from 6:00 to 18:00 in most weathers
    (1060002410; its siblings 1060002420 / 2430 are Liurnia's fireflies and the mountains' particles, left alone).
    Returns (bytes, number of patched instructions)."""
    e = Emevd(d)
    out, hits = bytearray(d), 0
    for ev in e.events:
        for i in ev["ins"]:
            key, a = (i["bank"], i["id"]), i["args"]
            if key == (2006, 4) and struct.unpack_from("<i", a, 8)[0] in LEAF_SFX:
                struct.pack_into("<I", out, i["argoff"], dummy)
            elif key == (2006, 6) and struct.unpack_from("<i", a)[0] == GUST_WIND_SFX:
                struct.pack_into("<i", out, i["argoff"], -1)
            elif key == (2004, 8) and struct.unpack_from("<Ii", a) in {(p, s) for p in (10000, 20000)
                                                                          for s in AMBIENT_SPEFFECTS}:
                struct.pack_into("<I", out, i["argoff"], dummy)
            else:
                continue
            hits += 1
    return bytes(out), hits


MIDDAY_EVENT, REFREEZE_EVENT = 1042362992, 1042362993  # in m60_42_36_00; m60.emevd gets 1060002990 / 1060002991
MIDDAY = (12, 0, 0)


def midday(d, event_ids=(MIDDAY_EVENT, REFREEZE_EVENT)):
    """Always midday (Wizard101 has no day and night). Vanilla holds the time in the Stranded Graveyard this way (m18
    event 18000021: 2001[04] SetCurrentTime 10:30 then 2001[05] FreezeTime until the First Step door opens). Two new
    events, restarting on rest (so a rest at the grace sets it again), started from the top of event 0:
      first:  unfreeze, SetCurrentTime(12:00, no fade, no clock), freeze; then wait until the time is outside
              11:30-12:30 (1[05] TimeOfDayInRange 0:00-11:29:59 OR 12:30-23:59:59, e.g. after "pass time" at a grace
              or something unfreezing it) and restart
      second: freeze again every 10 s (common events 702 / 3040 unfreeze time in their own situations)
    Rerunning replaces them."""
    e = Emevd(d)
    events = events_of(e)
    first, second = event_ids
    for event_id in (second, first):
        _start_once(events, event_id)
    h, m, s = MIDDAY
    events.append({"id": first, "rest": 1, "pars": [], "ins": [
        _i(2001, 5, "<Bxxx", 0),
        _i(2001, 4, "<BBBBBBxxfff", h, m, s, 0, 0, 0, 0.0, 0.0, 0.0),
        _i(2001, 5, "<Bxxx", 1),
        _i(1, 5, "<bBBBBBBx", -1, 0, 0, 0, 11, 29, 59),
        _i(1, 5, "<bBBBBBBx", -1, 12, 30, 0, 23, 59, 59),
        _i(0, 0, "<bBbx", 0, 1, -1),
        _i(1000, 4, "<Bxxx", 1),
    ]})
    events.append({"id": second, "rest": 1, "pars": [], "ins": [
        _i(2001, 5, "<Bxxx", 1),
        _i(1001, 0, "<f", 10.0),
        _i(1000, 4, "<Bxxx", 1),
    ]})
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
    if sys.argv[1] == "--no-leaf-gusts":  # emevd.py --no-leaf-gusts common.emevd out.emevd
        open(sys.argv[3], "wb").write(no_leaf_gusts(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--rattlebones":  # emevd.py --rattlebones m60_42_36_00.emevd out.emevd (doors of msb_add_room.py)
        open(sys.argv[3], "wb").write(rattlebones_doors(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--garden":  # emevd.py --garden m60_42_36_00.emevd out.emevd (doors of msb_add_garden.py)
        open(sys.argv[3], "wb").write(garden_doors(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--interiors":  # emevd.py --interiors in.emevd out.emevd (doors of msb_add_interiors.py)
        open(sys.argv[3], "wb").write(interior_doors(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--rattlebones-boss":  # emevd.py --rattlebones-boss in.emevd out.emevd (msb_add_boss.py)
        open(sys.argv[3], "wb").write(boss_bar(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--no-leaves":  # emevd.py --no-leaves common.emevd|m60.emevd out.emevd
        out, hits = no_leaves(open(sys.argv[2], "rb").read())
        print(f"{hits} instructions patched")
        open(sys.argv[3], "wb").write(out)
        sys.exit()
    if sys.argv[1] == "--midday":  # emevd.py --midday m60_42_36_00.emevd out.emevd [first_id second_id]
        ids = tuple(int(x) for x in sys.argv[4:6]) or (MIDDAY_EVENT, REFREEZE_EVENT)
        open(sys.argv[3], "wb").write(midday(open(sys.argv[2], "rb").read(), ids))
        sys.exit()
    if sys.argv[1] == "--bosses":  # emevd.py --bosses in.emevd out.emevd (msb_add_boss.py, msb_add_indoor.py)
        open(sys.argv[3], "wb").write(all_bosses(open(sys.argv[2], "rb").read()))
        sys.exit()
    if sys.argv[1] == "--no-guidance":  # emevd.py --no-guidance m60_42_36_00.emevd out.emevd
        open(sys.argv[3], "wb").write(no_guidance_wisps(open(sys.argv[2], "rb").read()))
        sys.exit()
    data = open(sys.argv[1], "rb").read()
    open(sys.argv[2], "wb").write(close_graveyard_door(data))
