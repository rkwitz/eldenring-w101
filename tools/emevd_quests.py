"""Event side of the Unicorn Way quests (tools/w101_quests.py; map side: msb_add_quests.py, msb_add_npcs.py): adds
the quest events to m60_42_36_00.emevd as separate events (ids w101_quests.EVENT_BASE..+99), each started from the
top of event 0 and restarting on rest. Running it again replaces them, and it leaves every other event alone, so it
can be applied on top of other helpers' changes.

Events, in id order (all flags in w101_quests' docstring):
  assign  quests offered on arrival (Unicorn Way: reaching Private Connelly's sphere): the giver's offer
          ("intro|<quest>": Merle Ambrose's lines and the quest card with yes / no), the quest assigned only on yes,
          then "intro|<quest>|after" (Gamma pointing the player to Connelly). Round 18; rounds 10-17 assigned it
          before showing anything. Round 20: no quest uses it any more (Unicorn Way is "greet": npc block 0).
  goal    one per goal that is not a talk goal: END if done; WAIT for the quest's assigned flag and the goals before
          it; then wait for the goal's own condition (player in the sphere / kill counter >= N / every cage flag /
          the boss alive and then dead) and set the goal flag; the second-to-last goal also sets "ready", the last
          one sets "complete" and assigns the next quests. One-goal quests are ready as soon as they are assigned.
  kill    one per defeat goal, with the enemy's entity id as its parameter, started once per enemy of the goal's
          kind (2000[0] slot k, as vanilla's per-enemy events): END if the goal is done; wait for the goal to be
          current and the enemy alive, then for it to die; count it (2003[31], 4 bits) unless the goal finished
          meanwhile; END (the rest restart watches the respawned enemy again).
  cage    one per Bone Cage (round 22): closed cage shown, open twin hidden; if already opened, swap them and END;
          else wait for Finding Out Why, wait for the "Examine" press on the closed cage, set the cage flag, SFX on
          the open twin, swap them.
  npc     (round 15) one per street character (w101_quests.CHARACTERS, placed by msb_add_npcs.py): wait for ER's
          own "Talk" press (ActionButtonParam 6000) on the character, then the first block whose flags match, each
          ending in RESTART so the next press is handled the same way:
            0. (round 20) a "greet" quest of this NPC not assigned yet: assign it and go on to block 1, whose talk
               goal it now is (Connelly: Unicorn Way's conversation, ending in the Ghost Hunters card)
            1. a talk goal of any quest that is current and targets this NPC: set the goal flag (and ready /
               complete / the next quests' assignment, as a goal event would), then that goal's conversation
            2. a side quest this NPC hands out whose prerequisite is on and which is not assigned: the offer
               conversation; the quest is assigned only when it is accepted (round 17)
            3. a quest of this NPC's that is assigned and not complete and has "underway" lines: that conversation
            4. the NPC's idle conversation (round 23: its "<quest>|done" line once that quest is complete)
  idle    (round 16) one per street character: one frame after every load, ForceAnimation its idle
          (w101_quests.IDLE), then END.
Conversations (round 17, F2/F3) are shown by the HUD DLL in Wizard101's own dialogue box, not ER's 2007[10]: a
conversation is one request, `talk(key, entity)`: clear DIALOG_OK / DIALOG_CANCEL, set the key's request flag
(w101_quests.dialogue_keys(), 1042362400+), wait for OK, CANCEL or the player walking LEAVE_DISTANCE away, clear the
request flag again, and RESTART unless OK was answered. The DLL pages through the key's lines (NEXT / BACK / DONE)
and answers OK after the last one, CANCEL when closed early. For a quest offer its last page is the quest card with
ACCEPT / DECLINE: OK = accept, and only then does the event assign the quest; DECLINE, closing or walking away leave
it offered (the "!" stays). Talk goals still set their flags before the conversation, so leaving early never loses
progress. A dialogue file (--dialogue npc_dialogue.json: {"pages": {key: [[id, text], ...]}}; Wizard101 text, outside
the repo) only decides which "underway" blocks exist; the text itself never enters the EMEVD.

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
LEAVE_DISTANCE = 6.0  # walking this far away from the NPC ends the conversation
PAGE_GAP = 0.2  # seconds after a conversation before the next talk press is taken


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


def if_riding(group, entity, mounted):
    """4[32] IF Character Riding Mount."""
    return _i(4, 32, "<bxxxIBxxx", group, entity, mounted)


def buttons(on_foot, riding, entity, alive=False):
    """Wait for the Action Button on `entity` that fits the player's state: `on_foot` (greyed out on Torrent) while on
    foot, `riding` (usable on Torrent) while mounted, never both at once.
    Round 25 (M5) waited for either in one OR group, so Elden Ring had two action buttons up at the same spot and
    showed its "Switch actions" prompt (Ryan, round 26 N4). Now 4[32] IF Character Riding Mount picks the branch; a
    branch waits for its button OR the player (un)mounting, and on (un)mounting the event restarts into the other
    branch. Each branch sets RIDING_FLAG (w101_quests) for the HUD prompt (prompt.rs), which then measures the same
    row. With `alive` (round 26 N7) the button counts only while the character is alive."""
    def branch(button, mounted):
        press = [if_button(5, button, entity)] + ([if_dead(5, entity, 0)] if alive else [])
        return [set_flag(Q.RIDING_FLAG, mounted), *press, if_riding(6, PLAYER, 1 - mounted),
                if_group(-5, 1, 5), if_group(-5, 1, 6), if_group(0, 1, -5),
                _i(1000, 8, "<BBbx", 1, 1, 6)]  # (un)mounted instead of a press: RESTART
    foot, ride = branch(on_foot, 0), branch(riding, 1)
    return [if_riding(7, PLAYER, 1), _i(1000, 1, "<BBbx", len(foot) + 1, 1, 7),  # on Torrent now: the riding branch
            *foot, _i(1000, 3, "<Bxxx", len(ride)), *ride]


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


def oneshot_sfx(asset, sfx, dummy_poly=-1):
    """2006[03] SpawnOneshotSFX on an asset (target type 0), as vanilla common 9005810."""
    return _i(2006, 3, "<iIii", 0, asset, dummy_poly, sfx)


def force_animation(entity, animation, loop=0):
    """2003[18] ForceAnimationPlayback, packed as vanilla m60_42_36 event 1042363700 plays Varre's 90100."""
    return _i(2003, 18, "<IiBBBbf", entity, animation, loop, 0, 0, 0, 1.0)


def wait(seconds):
    return _i(1001, 0, "<f", seconds)


END = _i(1000, 4, "<Bxxx", 0)
WAIT_FRAME = _i(1001, 1, "<i", 1)  # WaitFixedNumberOfFrames(1)
RESTART = _i(1000, 4, "<Bxxx", 1)

DIALOGUE = {}  # dialogue keys that have lines (main: --dialogue); decides which "underway" blocks exist
REQUESTS = Q.dialogue_keys()  # dialogue key -> the flag that asks the HUD to show it


def talk(key, entity=0):
    """One conversation in the HUD's Wizard101 dialogue box: request `key`, wait for the answer (or the player walking
    away from `entity`), withdraw the request, and restart the event unless the answer was OK."""
    request = REQUESTS[key]
    out = [set_flag(Q.DIALOG_OK, 0), set_flag(Q.DIALOG_CANCEL, 0), set_flag(request),
           if_flag(-1, 1, Q.DIALOG_OK), if_flag(-1, 1, Q.DIALOG_CANCEL)]
    if entity:
        out.append(if_near(-1, 0, entity, LEAVE_DISTANCE))
    return out + [if_group(0, 1, -1), set_flag(request, 0), end_if_flag(Q.DIALOG_OK, 0, restart=True), wait(PAGE_GAP)]


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
    # Round 23: never once the quest is complete (as markers.rs's "?").
    return ([(Q.assigned(q["slug"]), 1), (Q.complete(q["slug"]), 0), (Q.goal_flag(q["slug"], i), 0)]
            + [(Q.goal_flag(q["slug"], j), 1) for j in Q.after_of(q, i)])


def after_yes(npc, quest, entity):
    """Round 18 (F1): what the giver (and Gamma) say once quest `quest` is accepted ("<npc>|<quest>|accepted"), when
    the dialogue file has lines for it. The quest is already assigned, so closing it early loses nothing."""
    key = f"{npc}|{quest}|accepted"
    return talk(key, entity) if DIALOGUE.get(key) else []


def npc_event(slug, entity):
    # Round 26 (N7): a dead character (immortal, so only if something kills it anyway) takes no talk until the map
    # loads again: END while it is dead, and the button counts only while it is alive.
    body = [if_dead(4, entity, 1), _i(1000, 2, "<BBbx", 0, 1, 4)]  # group 4 (buttons uses 5, 6, 7)
    body += buttons(Q.TALK_NPC_BUTTON, Q.TALK_NPC_BUTTON_RIDING, entity, alive=True)
    for q in Q.QUESTS:  # 0. (round 20) a quest this NPC starts by being talked to ("greet"): assigned, then block 1
        if q["assign"] and q["assign"][0] == "greet" and q["assign"][1] == slug:
            # Round 23 (Q2): never for a quest already assigned or complete.
            body += block([(Q.assigned(q["slug"]), 0), (Q.complete(q["slug"]), 0)],
                          assign(q["slug"]))  # no RESTART: falls into its talk goal
    for q in Q.QUESTS:  # 1. talk goals
        for i, g in enumerate(q["goals"], 1):
            if g["type"] == "talk" and g["target"] == slug:
                key = f"{slug}|{q['slug']}|goal{i}"
                # Round 18 (F2): a goal that completes its quest may end in the offer of the next one (w101_quests.
                # follow_up); it is assigned only on yes. On no, or the box closed, the quest stays offered (block 2).
                nxt = Q.follow_up(q, i)
                accepted = assign(nxt["slug"]) + after_yes(slug, nxt["slug"], entity) if nxt else []
                body += block(current(q, i), goal_done(q, i) + talk(key, entity) + accepted + [RESTART])
    for q in Q.QUESTS:  # 2. side quests offered by talking
        if q["assign"] and q["assign"][0] == "talk" and q["assign"][1] == slug:
            # Round 23 (Q2): an offer never comes for a quest the player has (assigned) or finished (complete).
            conditions = [(Q.assigned(q["slug"]), 0), (Q.complete(q["slug"]), 0)]
            if q["assign"][2]:
                conditions.append((prereq_flag(q["assign"][2]), 1))
            # Round 17 (F3): assigned only once the offer is accepted (talk() restarts on decline / walking off).
            body += block(conditions, talk(f"{slug}|{q['slug']}|offer", entity) + assign(q["slug"])
                            + after_yes(slug, q["slug"], entity) + [RESTART])
    for q in Q.QUESTS:  # 3. underway
        key = f"{slug}|{q['slug']}|underway"
        if DIALOGUE.get(key):
            conditions = [(Q.assigned(q["slug"]), 1), (Q.complete(q["slug"]), 0)]
            # Round 23 (Q1): only until this NPC's last talk goal of the quest is done (Connelly's "drive away some of
            # these Lost Souls" no longer comes after he was told they're beaten and sends the player to Ceren).
            mine = [i for i, g in enumerate(q["goals"], 1) if g["type"] == "talk" and g["target"] == slug]
            if mine:
                conditions.append((Q.goal_flag(q["slug"], mine[-1]), 0))
            body += block(conditions, talk(key, entity) + [RESTART])
    # 4. idle. Round 23 (Q1): once one of its IDLE_DONE quests is complete, the latest one's "done" line (so Ceren
    # no longer opens with her Skeleton Crew pitch after that quest); before, "<npc>|idle".
    for quest in reversed(Q.IDLE_DONE.get(slug, [])):
        key = f"{slug}|{quest}|done"
        if DIALOGUE.get(key):
            body += block([(Q.complete(quest), 1)], talk(key, entity) + [RESTART])
    return body + talk(f"{slug}|idle", entity) + [RESTART]


def quest_events():
    """[(event id, instructions)] of every quest event, in id order."""
    events = []
    for q in Q.QUESTS:  # offered on arrival (talk offers are in the NPC events)
        if not q["assign"] or q["assign"][0] != "intro":
            continue
        _how, target, prereq = q["assign"]
        region = Q.REGION_ENTITY[target]
        body = [end_if_flag(Q.assigned(q["slug"]))]
        if prereq:
            body.append(wait_flag(prereq_flag(prereq)))
        # Round 19 (E1): the first offer comes anywhere on the street (Ryan never got within the 12 m of Connelly's
        # sphere, so round 18's offer never came); once shown (INTRO_SEEN), a "no" waits for the region again.
        body += [skip_if_flag(2, Q.INTRO_SEEN, 1), if_near(0, 1, Q.INTRO_ANCHOR, Q.INTRO_RADIUS),
                 skip_if_flag(1, Q.INTRO_SEEN, 0), if_area(0, 1, region), set_flag(Q.INTRO_SEEN)]
        # Round 18 (F2): the giver's offer (Merle Ambrose's lines and the quest card with yes / no); the quest is
        # the player's only on yes. On no, or the box closed, it is offered again on the next arrival (after leaving
        # the region), never assigned behind the player's back as in rounds 10-17.
        request = REQUESTS[f"intro|{q['slug']}"]
        body += [set_flag(Q.DIALOG_OK, 0), set_flag(Q.DIALOG_CANCEL, 0), set_flag(request),
                 if_flag(-1, 1, Q.DIALOG_OK), if_flag(-1, 1, Q.DIALOG_CANCEL), if_group(0, 1, -1), set_flag(request, 0),
                 skip_if_flag(2, Q.DIALOG_OK, 1), if_area(0, 0, region), RESTART]
        events.append(body + assign(q["slug"]) + talk(f"intro|{q['slug']}|after") + [END])
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
        # Round 22 (P2): the cage stands on the street from the start (Ryan saw none: rounds 13-21 hid it until Finding
        # Out Why); opened = its open twin (door swung, entity + CAGE_OPEN_ENTITY) instead, with a burst of SFX.
        flag, opened = Q.CAGE_FLAG + c, entity + Q.CAGE_OPEN_ENTITY
        events.append([asset_enabled(entity, 1), asset_enabled(opened, 0),
                       skip_if_flag(3, flag, 0),  # not opened yet: on to the wait
                       asset_enabled(entity, 0), asset_enabled(opened, 1), END,
                       wait_flag(Q.assigned("finding_out_why")),  # the Examine prompt only while the quest is on
                       *buttons(Q.CAGE_BUTTON, Q.CAGE_BUTTON_RIDING, entity), set_flag(flag),
                       oneshot_sfx(opened, Q.CAGE_OPEN_SFX), asset_enabled(entity, 0), asset_enabled(opened, 1), END])
    for slug, *_rest, entity in Q.CHARACTERS:  # npcs
        events.append(npc_event(slug, entity))
    for _slug, _title, _frame, _local, _yaw, model, *_rows, entity in Q.CHARACTERS:  # idles (round 16 F3)
        animation, loop = Q.IDLE[model]
        body = [WAIT_FRAME, force_animation(entity, animation, loop)]  # vanilla waits a frame too
        # Round 26 (N7): never dies (HP stays >= 1; hits and flinches still happen). N3: a seated character keeps the
        # MSB root under its seat (gravity off), as vanilla Enia in her chair.
        body.insert(1, _i(2004, 12, "<IBxxx", entity, 1))
        if _slug in Q.SEATED:
            body.insert(1, _i(2004, 10, "<IBxxx", entity, 0))
        if _slug in Q.STAND_STILL:
            # Round 21 (Q7): Ryan: Lady Oriel ran around the hedge maze pathfinding. AI off (2004[1], as the bosses'
            # setup in emevd.py) keeps her on her MSB spot in the middle; her Talk button is the event's, not the AI's.
            body.insert(1, _i(2004, 1, "<IBxxx", entity, 0))
        events.append(body + [END])
    assert len(events) <= 100, f"{len(events)} events do not fit in {Q.EVENT_BASE}..+99"
    events.append(reset_event())
    out = []
    for k, ev in enumerate(events):
        body, pars, instances = ev if isinstance(ev, tuple) else (ev, [], [None])
        if k < len(events) - 1:
            # Round 19 (E1): nothing runs before the one-shot reset has cleared the old quest state (parameters
            # point at instruction lines, so they move down by the added line).
            body = [wait_flag(Q.QUEST_RESET)] + body
            pars = [(line + 1, *rest) for line, *rest in pars]
        out.append((Q.EVENT_BASE + k, body, pars, instances))
    return out


def reset_event():
    """Round 19 (E1): once per save (QUEST_RESET, versioned), every quest flag off, then QUEST_RESET on. Ryan's save
    kept Unicorn Way's state from round 17, so the quest line could not start at Merle Ambrose's offer."""
    # Round 23 (Q3): the removed side quests' saved flags go off on every map load (Ryan's save had Pesky Pirates
    # assigned), so nothing in the book, markers or helper can show them again.
    body = [set_flag(f, 0) for first, last in Q.REMOVED_FLAG_RANGES for f in range(first, last + 1)]
    body += [end_if_flag(Q.QUEST_RESET)]
    for first, last in Q.QUEST_FLAG_RANGES:
        body += [set_flag(f, 0) for f in range(first, last + 1)]
    return body + [set_flag(Q.QUEST_RESET), END]


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
          f"{len(REQUESTS)} dialogue keys, {len(DIALOGUE)} with lines")
    return build(e, events)


def load_dialogue(path):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    return {key: len(entries) for key, entries in data["pages"].items() if entries}


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
