"""Event side of the Unicorn Way quests (tools/w101_quests.py; map side: msb_add_quests.py): adds the quest events
to m60_42_36_00.emevd as separate events (ids w101_quests.EVENT_BASE..+99), each started from the top of event 0 and
restarting on rest. Running it again replaces them, and it leaves every other event alone, so it can be applied on
top of other helpers' changes.

Events, in id order (all flags in w101_quests' docstring):
  talk    one per quest giver: action button TALK_BUTTON on the giver's asset -> set its talk flag, loop. This is
          what keeps the prompt up; the HUD shows the dialogue for whatever the press changed and clears the flag.
  assign  quests not handed on by the quest before them: Unicorn Way when the player reaches Connelly's sphere,
          side quests by talking to their giver once their prerequisite flag is on.
  goal    one per goal: END if done; WAIT for the quest's assigned flag and the goals before it; then wait for the
          goal's own condition (talk press / player in the sphere / kill counter >= N / every cage flag / the boss
          alive and then dead) and set the goal flag; the second-to-last goal also sets "ready", the last one sets
          "complete" and assigns the next quests. One-goal quests are ready as soon as they are assigned.
  kill    one per (defeat goal, enemy): END if the goal is done; wait for the goal to be current and the enemy alive,
          then for it to die; count it (2003[31], 4 bits) unless the goal finished meanwhile; END (the rest restart
          watches the respawned enemy again).
  cage    one per Bone Cage: if already opened, disable the asset and END; else wait for Finding Out Why, the press,
          set the cage flag and disable the asset (the fairy is free).
Waiting on the flags first and on the button press last means a giver's quest prompt only exists while that event
can use it; the talk event's prompt is always there.

Usage: python emevd_quests.py in.emevd[.dcx] out.emevd[.dcx]   (DFLT DCX in -> DFLT DCX out)
"""

import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from emevd import Emevd, _i, build, events_of  # noqa: E402
import w101_quests as Q  # noqa: E402

PLAYER = 10000
GE = 4  # comparison: GreaterOrEqual


def if_flag(group, state, flag):
    return _i(3, 0, "<bBBxI", group, state, 0, flag)


def if_area(group, state, region):
    return _i(3, 2, "<bBxxIIi", group, state, PLAYER, region, 1)


def if_value(group, base, bits, comparison, value):
    return _i(3, 12, "<bxxxIBBxxI", group, base, bits, comparison, value)


def if_button(group, button, entity):
    return _i(3, 24, "<bxxxiI", group, button, entity)


def if_dead(group, entity, dead):
    return _i(4, 0, "<bxxxIBBxxf", group, entity, dead, 0, 1.0)


def if_group(group, state, target):
    return _i(0, 0, "<bBbx", group, state, target)


def wait_flag(flag, state=1):
    return _i(1003, 0, "<BBxxI", state, 0, flag)


def end_if_flag(flag, state=1):
    return _i(1003, 2, "<BBBxI", 0, state, 0, flag)


def skip_if_flag(lines, flag, state=1):
    return _i(1003, 1, "<BBBxI", lines, state, 0, flag)


def set_flag(flag, state=1):
    return _i(2003, 66, "<BxxxIBxxx", 0, flag, state)


def increment(base, bits=4, maximum=15):
    return _i(2003, 31, "<III", base, bits, maximum)


def asset_enabled(entity, state):
    return _i(2005, 3, "<IBxxx", entity, state)


END, RESTART = _i(1000, 4, "<Bxxx", 0), _i(1000, 4, "<Bxxx", 1)


def assign(slug):
    """Instructions that assign quest `slug` (and make a one-goal quest ready at once)."""
    out = [set_flag(Q.assigned(slug))]
    if len(Q.BY_SLUG[slug]["goals"]) == 1:
        out.append(set_flag(Q.ready(slug)))
    return out


def prereq_flag(spec):
    slug, _, state = spec.partition(":")
    return Q.complete(slug) if state == "complete" else Q.assigned(slug)


def npc_entity(slug):
    return next(e for s, _f, _p, _r, _m, e in Q.NPCS if s == slug)


def quest_events():
    """[(event id, instructions)] of every quest event, in id order."""
    events = []
    for slug, *_rest in Q.NPCS:  # talk
        events.append([if_button(0, Q.TALK_BUTTON, npc_entity(slug)), set_flag(Q.TALK_FLAG + Q.NPC_INDEX[slug]),
                       RESTART])
    for q in Q.QUESTS:  # assign
        if not q["assign"]:
            continue
        how, target, prereq = q["assign"]
        body = [end_if_flag(Q.assigned(q["slug"]))]
        if prereq:
            body.append(wait_flag(prereq_flag(prereq)))
        body.append(if_area(0, 1, Q.REGION_ENTITY[target]) if how == "goto" else
                    if_button(0, Q.TALK_BUTTON, npc_entity(target)))
        events.append(body + assign(q["slug"]) + [END])
    counters = Q.counters()
    kills = []
    for q in Q.QUESTS:  # goals
        slug, goals = q["slug"], q["goals"]
        for i, g in enumerate(goals, 1):
            flag = Q.goal_flag(slug, i)
            current = [wait_flag(Q.assigned(slug))] + [wait_flag(Q.goal_flag(slug, j)) for j in Q.after_of(q, i)]
            body = [end_if_flag(flag)] + current
            if g["type"] == "talk":
                body.append(if_button(0, Q.TALK_BUTTON, npc_entity(g["target"])))
            elif g["type"] == "goto":
                body.append(if_area(0, 1, Q.REGION_ENTITY[g["target"]]))
            elif g["type"] == "defeat":
                counter = counters[(slug, i)]
                body.append(if_value(0, counter, 4, GE, g["count"]))
                for mob in Q.MOBS[g["target"]]:
                    kills.append([end_if_flag(flag)] + current + [
                        if_dead(0, mob, 0), if_dead(0, mob, 1), skip_if_flag(1, flag), increment(counter), END])
            elif g["type"] == "boss":
                boss = Q.BOSSES[g["target"]]
                body += [if_dead(0, boss, 0), if_dead(0, boss, 1)]
            elif g["type"] == "cages":
                body += [if_flag(1, 1, Q.CAGE_FLAG + c) for c in range(len(Q.CAGES))] + [if_group(0, 1, 1)]
            else:
                raise ValueError(g["type"])
            body.append(set_flag(flag))
            if i == len(goals) - 1:
                body.append(set_flag(Q.ready(slug)))
            if i == len(goals):
                body.append(set_flag(Q.complete(slug)))
                for nxt in q["then"]:
                    body += assign(nxt)
            events.append(body + [END])
    events += kills
    for c, (_local, entity) in enumerate(Q.CAGES):  # cages
        flag = Q.CAGE_FLAG + c
        events.append([skip_if_flag(2, flag, 0), asset_enabled(entity, 0), END,
                       wait_flag(Q.assigned("finding_out_why")), if_button(0, Q.TALK_BUTTON, entity),
                       set_flag(flag), asset_enabled(entity, 0), END])
    assert len(events) <= 100, f"{len(events)} events do not fit in {Q.EVENT_BASE}..+99"
    return [(Q.EVENT_BASE + k, body) for k, body in enumerate(events)]


def add_quests(d):
    e = Emevd(d)
    ours = range(Q.EVENT_BASE, Q.EVENT_BASE + 100)
    events = [ev for ev in events_of(e) if ev["id"] not in ours]
    ins0 = next(ev for ev in events if ev["id"] == 0)["ins"]
    ins0[:] = [i for i in ins0 if not ((i["bank"], i["id"]) == (2000, 0)
                                      and struct.unpack_from("<I", i["args"], 4)[0] in ours)]
    new = quest_events()
    for slot, (event_id, _body) in enumerate(new):
        ins0.insert(slot, _i(2000, 0, "<iIi", 0, event_id, 0))
    for event_id, body in new:
        events.append({"id": event_id, "rest": 1, "pars": [], "ins": body})
    print(f"{len(new)} quest events {new[0][0]}..{new[-1][0]}")
    return build(e, events)


def main():
    data = Path(sys.argv[1]).read_bytes()
    if data[:4] == b"DCX\0":
        assert data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed file"
        out = add_quests(zlib.decompress(data[0x4C:]))
        packed = zlib.compress(out, 9)
        head = bytearray(data[:0x4C])
        struct.pack_into(">II", head, 0x1C, len(out), len(packed))
        Path(sys.argv[2]).write_bytes(bytes(head) + packed)
    else:
        Path(sys.argv[2]).write_bytes(add_quests(data))


if __name__ == "__main__":
    main()
