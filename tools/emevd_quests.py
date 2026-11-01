"""Event side of the Unicorn Way quests (tools/w101_quests.py; map side: msb_add_quests.py, msb_add_npcs.py): adds
the quest events to m60_42_36_00.emevd as separate events (ids w101_quests.EVENT_BASE..+99), each started from the
top of event 0 and restarting on rest. Running it again replaces them, and it leaves every other event alone, so it
can be applied on top of other helpers' changes.

Events, in id order (all flags in w101_quests' docstring):
  assign  quests assigned by walking somewhere (Unicorn Way: reaching Private Connelly's sphere), with the
          quest's intro page (dialogue key "intro|<quest>": Gamma pointing the player to Connelly).
  goal    one per goal that is not a talk goal: END if done; WAIT for the quest's assigned flag and the goals before
          it; then wait for the goal's own condition (player in the sphere / kill counter >= N / every cage flag /
          the boss alive and then dead) and set the goal flag; the second-to-last goal also sets "ready", the last
          one sets "complete" and assigns the next quests. One-goal quests are ready as soon as they are assigned.
  kill    one per defeat goal, with the enemy's entity id as its parameter, started once per enemy of the goal's
          kind (2000[0] slot k, as vanilla's per-enemy events): END if the goal is done; wait for the goal to be
          current and the enemy alive, then for it to die; count it (2003[31], 4 bits) unless the goal finished
          meanwhile; END (the rest restart watches the respawned enemy again).
  cage    one per Bone Cage: disabled (not drawn, no collision) at load; if already opened, END; else wait for
          Finding Out Why, enable it, wait for the "Examine" press, set the cage flag, disable it again.
  npc     (round 15) one per street character (w101_quests.CHARACTERS, placed by msb_add_npcs.py): wait for ER's
          own "Talk" press (ActionButtonParam 6000) on the character, then the first block whose flags match, each
          ending in RESTART so the next press is handled the same way:
            1. a talk goal of any quest that is current and targets this NPC: set the goal flag (and ready /
               complete / the next quests' assignment, as a goal event would), then that goal's pages
            2. a side quest this NPC hands out whose prerequisite is on and which is not assigned: assign it, pages
            3. a quest of this NPC's that is assigned and not complete and has "underway" pages: those pages
            4. the NPC's idle pages
          Flags are set before the pages, so leaving the dialog early never loses progress.
  idle    (round 16) one per street character: one frame after every load, ForceAnimation its idle
          (w101_quests.IDLE), then END.
Pages are ER's generic dialog box (2007[10] "Display Generic Dialog and Set Event Flags", OK only, as vanilla
common.emevd 4852 shows its messages) with EventTextForMap text from er_text.py --dialogue: OK shows the next page,
the back button or walking away from the NPC ends the conversation. Message ids come from a dialogue file
(--dialogue npc_dialogue.json: {"pages": {"<npc>|<quest>|goal<i>" / "|offer" / "|underway", "<npc>|idle",
"intro|<quest>": [[message id, text], ...]}}); that file holds Wizard101 text and lives outside the repo. Without
it the events still work, silently. The HUD's own dialogue box (talk flags 600-604) is no longer cued.

Usage: python emevd_quests.py [--dialogue npc_dialogue.json] in.emevd[.dcx] out.emevd[.dcx]   (DFLT DCX in -> out)
"""

import json
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from emevd import Emevd, _i, build, events_of  # noqa: E402
import w101_quests as Q  # noqa: E402

PLAYER = 10000
GE = 4  # comparison: GreaterOrEqual
DIALOG_DISTANCE = 5.0  # the dialog closes when the player is farther than this from the NPC
LEAVE_DISTANCE = 6.0  # walking this far away ends the conversation (in case the dialog closes without a flag)
PAGE_GAP = 0.2  # seconds between one page closing and the next one opening


def if_flag(group, state, flag):
    return _i(3, 0, "<bBBxI", group, state, 0, flag)


def if_area(group, state, region):
    return _i(3, 2, "<bBxxIIi", group, state, PLAYER, region, 1)


def if_near(group, state, entity, distance):
    return _i(3, 3, "<bBxxIIfi", group, state, PLAYER, entity, distance, 1)


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


def end_if_flag(flag, state=1, restart=False):
    return _i(1003, 2, "<BBBxI", int(restart), state, 0, flag)


def skip_if_flag(lines, flag, state=1):
    return _i(1003, 1, "<BBBxI", lines, state, 0, flag)


def set_flag(flag, state=1):
    return _i(2003, 66, "<BxxxIBxxx", 0, flag, state)


def increment(base, bits=4, maximum=15):
    return _i(2003, 31, "<III", base, bits, maximum)


def asset_enabled(entity, state):
    return _i(2005, 3, "<IBxxx", entity, state)


def force_animation(entity, animation, loop=0):
    """2003[18] ForceAnimationPlayback, packed as vanilla m60_42_36 event 1042363700 plays Varre's 90100."""
    return _i(2003, 18, "<IiBBBbf", entity, animation, loop, 0, 0, 0, 1.0)


def wait(seconds):
    return _i(1001, 0, "<f", seconds)


def dialog(message, entity, distance):
    """ER's generic dialog, OK button only; OK sets DIALOG_OK, back sets DIALOG_CANCEL."""
    return _i(2007, 10, "<ihhIfIII", message, 1, 1, entity, distance, Q.DIALOG_OK, Q.DIALOG_CANCEL, Q.DIALOG_CANCEL)


END = _i(1000, 4, "<Bxxx", 0)
WAIT_FRAME = _i(1001, 1, "<i", 1)  # WaitFixedNumberOfFrames(1)
RESTART = _i(1000, 4, "<Bxxx", 1)

DIALOGUE = {}  # dialogue key -> [message id, ...] (main: --dialogue)


def pages(key, entity=0):
    """One dialog per page of `key`; after each, the event restarts unless OK was pressed (back, or walked off)."""
    out = []
    for message in DIALOGUE.get(key, []):
        out += [set_flag(Q.DIALOG_OK, 0), set_flag(Q.DIALOG_CANCEL, 0),
                dialog(message, entity, DIALOG_DISTANCE if entity else 100.0),
                if_flag(-1, 1, Q.DIALOG_OK), if_flag(-1, 1, Q.DIALOG_CANCEL)]
        if entity:
            out.append(if_near(-1, 0, entity, LEAVE_DISTANCE))
        out += [if_group(0, 1, -1), end_if_flag(Q.DIALOG_OK, 0, restart=True), wait(PAGE_GAP)]
    return out


def block(conditions, body):
    """Instructions that run `body` only when every (flag, state) of `conditions` holds (skips past it otherwise)."""
    out = []
    assert len(conditions) + len(body) <= 256, "a skip counts at most 255 lines"
    for k, (flag, state) in enumerate(conditions):
        out.append(skip_if_flag(len(conditions) - 1 - k + len(body), flag, 1 - state))
    return out + body


def assign(slug):
    """Instructions that assign quest `slug` (and make a one-goal quest ready at once)."""
    out = [set_flag(Q.assigned(slug))]
    if len(Q.BY_SLUG[slug]["goals"]) == 1:
        out.append(set_flag(Q.ready(slug)))
    return out


def goal_done(q, i):
    """Instructions for goal i of quest q being done: its flag, ready / complete, the next quests."""
    out = [set_flag(Q.goal_flag(q["slug"], i))]
    if i == len(q["goals"]) - 1:
        out.append(set_flag(Q.ready(q["slug"])))
    if i == len(q["goals"]):
        out.append(set_flag(Q.complete(q["slug"])))
        for nxt in q["then"]:
            out += assign(nxt)
    return out


def prereq_flag(spec):
    slug, _, state = spec.partition(":")
    return Q.complete(slug) if state == "complete" else Q.assigned(slug)


def current(q, i):
    """(flag, state) conditions of goal i of quest q being current."""
    return ([(Q.assigned(q["slug"]), 1), (Q.goal_flag(q["slug"], i), 0)]
            + [(Q.goal_flag(q["slug"], j), 1) for j in Q.after_of(q, i)])


def npc_event(slug, entity):
    body = [if_button(0, Q.TALK_NPC_BUTTON, entity)]
    for q in Q.QUESTS:  # 1. talk goals
        for i, g in enumerate(q["goals"], 1):
            if g["type"] == "talk" and g["target"] == slug:
                key = f"{slug}|{q['slug']}|goal{i}"
                body += block(current(q, i), goal_done(q, i) + pages(key, entity) + [RESTART])
    for q in Q.QUESTS:  # 2. side quests offered by talking
        if q["assign"] and q["assign"][0] == "talk" and q["assign"][1] == slug:
            conditions = [(Q.assigned(q["slug"]), 0)]
            if q["assign"][2]:
                conditions.append((prereq_flag(q["assign"][2]), 1))
            body += block(conditions, assign(q["slug"]) + pages(f"{slug}|{q['slug']}|offer", entity) + [RESTART])
    for q in Q.QUESTS:  # 3. underway
        key = f"{slug}|{q['slug']}|underway"
        if DIALOGUE.get(key):
            body += block([(Q.assigned(q["slug"]), 1), (Q.complete(q["slug"]), 0)], pages(key, entity) + [RESTART])
    return body + pages(f"{slug}|idle", entity) + [RESTART]  # 4. idle


def quest_events():
    """[(event id, instructions)] of every quest event, in id order."""
    events = []
    for q in Q.QUESTS:  # assign by walking somewhere (talk assigns are in the NPC events)
        if not q["assign"] or q["assign"][0] != "goto":
            continue
        _how, target, prereq = q["assign"]
        body = [end_if_flag(Q.assigned(q["slug"]))]
        if prereq:
            body.append(wait_flag(prereq_flag(prereq)))
        body.append(if_area(0, 1, Q.REGION_ENTITY[target]))
        events.append(body + assign(q["slug"]) + pages(f"intro|{q['slug']}") + [END])
    counters = Q.counters()
    kills = []  # (instructions, parameters, enemies)
    for q in Q.QUESTS:  # goals (talk goals are in the NPC events)
        slug, goals = q["slug"], q["goals"]
        for i, g in enumerate(goals, 1):
            if g["type"] == "talk":
                continue
            flag = Q.goal_flag(slug, i)
            waits = [wait_flag(Q.assigned(slug))] + [wait_flag(Q.goal_flag(slug, j)) for j in Q.after_of(q, i)]
            body = [end_if_flag(flag)] + waits
            if g["type"] == "goto":
                body.append(if_area(0, 1, Q.REGION_ENTITY[g["target"]]))
            elif g["type"] == "defeat":
                counter = counters[(slug, i)]
                body.append(if_value(0, counter, 4, GE, g["count"]))
                kill = [end_if_flag(flag)] + waits + [
                    if_dead(0, 0, 0), if_dead(0, 0, 1), skip_if_flag(1, flag), increment(counter), END]
                alive = len(waits) + 1  # the two if_dead lines take the enemy (parameter 0 -> args offset 4)
                kills.append((kill, [(alive, 4, 0, 4, 1), (alive + 1, 4, 0, 4, 1)], Q.MOBS[g["target"]]))
            elif g["type"] == "boss":
                boss = Q.BOSSES[g["target"]]
                body += [if_dead(0, boss, 0), if_dead(0, boss, 1)]
            elif g["type"] == "cages":
                body += [if_flag(1, 1, Q.CAGE_FLAG + c) for c in range(len(Q.CAGES))] + [if_group(0, 1, 1)]
            else:
                raise ValueError(g["type"])
            events.append(body + goal_done(q, i) + [END])
    events += kills
    for c, (_local, entity) in enumerate(Q.CAGES):  # cages
        flag = Q.CAGE_FLAG + c
        events.append([asset_enabled(entity, 0), end_if_flag(flag), wait_flag(Q.assigned("finding_out_why")),
                       asset_enabled(entity, 1), if_button(0, Q.TALK_BUTTON, entity), set_flag(flag),
                       asset_enabled(entity, 0), END])
    for slug, *_rest, entity in Q.CHARACTERS:  # npcs
        events.append(npc_event(slug, entity))
    for _slug, _title, _frame, _local, _yaw, model, *_rows, entity in Q.CHARACTERS:  # idles (round 16 F3)
        animation, loop = Q.IDLE[model]
        events.append([WAIT_FRAME, force_animation(entity, animation, loop), END])  # vanilla waits a frame too
    assert len(events) <= 100, f"{len(events)} events do not fit in {Q.EVENT_BASE}..+99"
    out = []
    for k, ev in enumerate(events):
        body, pars, instances = ev if isinstance(ev, tuple) else (ev, [], [None])
        out.append((Q.EVENT_BASE + k, body, pars, instances))
    return out


def add_quests(d):
    e = Emevd(d)
    ours = range(Q.EVENT_BASE, Q.EVENT_BASE + 100)
    events = [ev for ev in events_of(e) if ev["id"] not in ours]
    ins0 = next(ev for ev in events if ev["id"] == 0)["ins"]
    ins0[:] = [i for i in ins0 if not ((i["bank"], i["id"]) == (2000, 0)
                                      and struct.unpack_from("<I", i["args"], 4)[0] in ours)]
    new = quest_events()
    inits = []
    for event_id, _body, _pars, instances in new:
        for slot, arg in enumerate(instances):  # one parameter (an entity id) or none (0, as before)
            inits.append(_i(2000, 0, "<iIi" if arg is None else "<iII", slot if arg is not None else 0, event_id,
                            0 if arg is None else arg))
    ins0[0:0] = inits
    for event_id, body, pars, _instances in new:
        events.append({"id": event_id, "rest": 1, "pars": pars, "ins": body})
    print(f"{len(new)} quest events {new[0][0]}..{new[-1][0]}, {len(inits)} starts, "
          f"{sum(len(v) for v in DIALOGUE.values())} dialogue pages")
    return build(e, events)


def load_dialogue(path):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    return {key: [message for message, _text in entries] for key, entries in data["pages"].items()}


def main(argv):
    if argv[0] == "--dialogue":
        DIALOGUE.update(load_dialogue(argv[1]))
        argv = argv[2:]
    data = Path(argv[0]).read_bytes()
    if data[:4] == b"DCX\0":
        assert data[0x28:0x2C] == b"DFLT", "expected a DFLT-compressed file"
        out = add_quests(zlib.decompress(data[0x4C:]))
        packed = zlib.compress(out, 9)
        head = bytearray(data[:0x4C])
        struct.pack_into(">II", head, 0x1C, len(out), len(packed))
        Path(argv[1]).write_bytes(bytes(head) + packed)
    else:
        Path(argv[1]).write_bytes(add_quests(data))


if __name__ == "__main__":
    main(sys.argv[1:])
