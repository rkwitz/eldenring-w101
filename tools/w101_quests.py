"""The Unicorn Way quest line as game logic: quests, goals, event flags, entity ids (no Wizard101 text; the client's
quest text is merged in by slug outside the repo, see /mnt/project-files/unicorn_way_quests).

Event flags (all in the m60_42_36 tile block; 1042366700+ is left alone: the map uses 1042366710, and 1042366900-999
is reserved for the door / loading-screen flags):
  quest n:  QUEST_BASE + 10*n   +0 assigned (stays on), +1..+6 goal i done, +8 ready to turn in (every goal but the
                                last one done), +9 complete
  counters: kill counts as 4-bit event values (2003[31] / 3[12]), COUNTER_BASE + 4*k
  cages:    one flag per Bone Cage opened, CAGE_FLAG + c
  talk:     TALK_FLAG + npc index, set by the game each time the player talks to that quest giver (action button);
            the HUD shows the dialogue and clears it (EMEVD never clears it)
No flag is on in a new save, so the player starts with no quests: Unicorn Way is assigned on reaching Private
Connelly at the street entrance, every other quest by the quest before it or by talking to its giver.

Goal types: talk (action button on the giver's asset), goto (player inside a sphere region), defeat (count kills of
the listed enemies while the goal is current, then the count reaches `count`; one kill counts for every current goal
that wants that enemy), boss (one kill of the boss while the goal is current), cages (all Bone Cages opened while the
quest is active). A goal is current when its quest is assigned, it is not done, and the goals in `after` (default:
the one before it) are done.
"""

QUEST_BASE = 1042366000
COUNTER_BASE = 1042366500
CAGE_FLAG = 1042366540
TALK_FLAG = 1042366600
EVENT_BASE = 1042363800  # our EMEVD events 1042363800..1042363899 (free in this map)

# Characters (MSB entity ids): 5 street mobs per patrol path (round 6: the 10 originals got 4 copies each, msb_more_mobs.py).
# MOB_PARTS lists the part names in the same order as MOBS; msb_add_quests.py sets the ids on the 10 originals
# (c....._9000..) and checks the copies, which msb_more_mobs.py already numbered.
def _mob_ids(*ranges):
    return [i for a, b in ranges for i in range(a, b + 1)]


MOBS = {
    "lost_soul": _mob_ids((1042360860, 1042360863), (1042360870, 1042360885)),  # c4180 Spirit Jellyfish
    "skeletal_pirate": _mob_ids((1042360864, 1042360867), (1042360886, 1042360901)),  # c3500
    "dark_fairy": _mob_ids((1042360868, 1042360869), (1042360902, 1042360909)),  # c3450 Winged Misbegotten
}
MOB_MODELS = {"lost_soul": "c4180", "skeletal_pirate": "c3500", "dark_fairy": "c3450"}
MOB_PARTS = {kind: [f"{MOB_MODELS[kind]}_{9000 + k}" for k in range(len(ids))] for kind, ids in MOBS.items()}
BOSSES = {
    "rattlebones": 1042360850,  # msb_add_boss.py
    "lady_blackhope": 1042360851,  # bosses helper (T1); change here if it uses another id
    "fear_of_fire": 1042360852,  # bosses helper (H4)
}

# Quest givers = invisible assets with an action button, in this order (npc index for TALK_FLAG). Positions are in
# the frame's own FLVER coordinates (= W101 (x, z, y) * 0.01): street = AEG030_518_9000, garden = AEG030_555_9000.
# GIVER_MODEL AEG099_990 is vanilla's empty asset (FLVER with no mesh and no vertices, no collision file); vanilla
# puts action buttons on it, e.g. common_func event 90005750 (item pick-up, m11_10 AEG099_990_9000). The visible
# vanilla placeholders of rounds 3-5 (OLD_GIVER_MODELS: statues, an armour stand) are renamed to it in place by
# msb_add_quests.py. Swap GIVER_MODEL per NPC for converted W101 NPC meshes later.
TALK_BUTTON = 9000  # ActionButtonParam "Examine": centred on the entity (dummyPoly -1), radius 1 m, height -0.5..+1 m
GIVER_MODEL = "AEG099_990"
NPCS = [  # (slug, frame, position, facing degrees, asset model, entity id)
    ("private_connelly", "street", (26.543, -0.016, 22.2), 180.0, GIVER_MODEL, 1042361870),  # W101 NPC04
    ("ceren_nightchant", "street", (-41.857, 0.289, 85.837), 180.0, GIVER_MODEL, 1042361871),  # NPC01
    ("olivia_dawnwillow", "street", (-20.198, -1.374, 86.55), 180.0, GIVER_MODEL, 1042361872),  # NPC03
    ("lady_oriel", "garden", (5.974, -0.7, 0.94), 180.0, GIVER_MODEL, 1042361873),  # NPC02, on the sunken centre floor
    ("private_oryan", "street", (-125.714, -0.711, 88.655), 90.0, GIVER_MODEL, 1042361874),  # at house H1's door
]
OLD_GIVER_MODELS = ["AEG217_220", "AEG110_324", "AEG024_169", "AEG099_010", "AEG217_220"]  # same order as NPCS
NPC_INDEX = {slug: k for k, (slug, *_rest) in enumerate(NPCS)}

# The four quest Bone Cages (W101 WC-FairyCage02..05), placeholder cage asset, entity ids. The cage events keep them
# disabled (not drawn, no collision) except while Finding Out Why's cage goal is current.
CAGE_MODEL = "AEG270_120"  # "Round metal prison cage"
CAGES = [
    ((-237.463, -0.999, 141.741), 1042361880),
    ((-219.187, -0.999, 142.762), 1042361881),
    ((-259.552, 0.0, 187.903), 1042361882),
    ((-22.839, -0.828, 82.569), 1042361883),
]

# Goal regions (spheres): (slug, frame, position, radius, entity id, region id). Frames as msb_add_interiors.py.
REGIONS = [
    ("street_start", "street", (26.543, 1.0, 22.2), 12.0, 1042362830, 9220),  # around Connelly at the entrance
    ("hedge_maze", "garden", (5.974, 0.3, 0.94), 14.0, 1042362831, 9221),  # around Lady Oriel
    ("T1", "AEG030_519_9001", (-0.085, 1.0, -8.86), 4.0, 1042362832, 9222),  # Blackhope room arrival
    ("H4", "AEG030_557_9000", (0.202, 1.0, 8.405), 4.0, 1042362833, 9223),  # Thief's House arrival
    ("near_ceren", "street", (-41.857, 1.0, 85.837), 15.0, 1042362834, 9224),
    ("near_oryan", "street", (-125.714, 1.0, 88.655), 6.0, 1042362835, 9225),
]
REGION_ENTITY = {r[0]: r[4] for r in REGIONS}


def quest_assets():
    """[(part name, frame, position, facing, model, entity)] of the giver and cage assets msb_add_quests.py places.
    footprint_clear.py keeps these part names (it would otherwise sink them 1000 m with the vanilla clutter)."""
    out = [(f"{model}_{9000 + k}", frame, local, facing, model, entity)
           for k, (_slug, frame, local, facing, model, entity) in enumerate(NPCS)]
    out += [(f"{CAGE_MODEL}_{9100 + c}", "street", local, 0.0, CAGE_MODEL, entity)
            for c, (local, entity) in enumerate(CAGES)]
    return out
REGION_ENTITY["T2"] = 1042362808  # msb_add_boss.py: W101_Rattlebones_arena, sphere over the whole room


def goal(type_, target, count=1, after=None):
    return {"type": type_, "target": target, "count": count, "after": after}


# n, slug, W101 quest id, giver, how it is assigned, goals, quests assigned on completion
QUESTS = [
    dict(n=0, slug="unicorn_way", w101="1625BF", giver="merle_ambrose", assign=("goto", "street_start", None),
         goals=[goal("talk", "private_connelly")], then=["ghost_hunters"]),
    dict(n=1, slug="ghost_hunters", w101="1625C2", giver="private_connelly", assign=None,
         goals=[goal("defeat", "lost_soul", 2), goal("talk", "private_connelly"), goal("talk", "ceren_nightchant")],
         then=["skeleton_crew"]),
    dict(n=2, slug="skeleton_crew", w101="1625C1", giver="ceren_nightchant", assign=None,
         goals=[goal("defeat", "skeletal_pirate", 2), goal("talk", "ceren_nightchant")], then=["monsters_and_mazes"]),
    dict(n=3, slug="monsters_and_mazes", w101="1625C5", giver="ceren_nightchant", assign=None,
         goals=[goal("goto", "hedge_maze"), goal("talk", "lady_oriel")], then=["finding_out_why"]),
    dict(n=4, slug="finding_out_why", w101="1625CA", giver="lady_oriel", assign=None,
         goals=[goal("cages", "bone_cage", 4, after=[]), goal("defeat", "dark_fairy", 2, after=[]),
                goal("goto", "hedge_maze", after=[1, 2]), goal("talk", "lady_oriel")], then=["who_could_it_be_now"]),
    dict(n=5, slug="who_could_it_be_now", w101="1625CE", giver="lady_oriel", assign=None,
         goals=[goal("talk", "ceren_nightchant")], then=["sinister_skeleton"]),
    dict(n=6, slug="sinister_skeleton", w101="1625CF", giver="ceren_nightchant", assign=None,
         goals=[goal("goto", "T2"), goal("boss", "rattlebones"), goal("goto", "near_ceren"),
                goal("talk", "ceren_nightchant")], then=[]),  # Grim Tales (n=7) is off the street: not assigned
    dict(n=10, slug="pesky_pirates", w101="1625C9", giver="olivia_dawnwillow",
         assign=("talk", "olivia_dawnwillow", "skeleton_crew"),
         goals=[goal("defeat", "skeletal_pirate", 2), goal("talk", "olivia_dawnwillow")], then=[]),
    dict(n=11, slug="saving_private_oryan", w101="1625DA", giver="private_connelly",
         assign=("talk", "private_connelly", "ghost_hunters:complete"),
         goals=[goal("goto", "near_oryan"), goal("talk", "private_oryan"), goal("goto", "T1"),
                goal("boss", "lady_blackhope"), goal("talk", "private_oryan"), goal("talk", "private_connelly")],
         then=[]),
    dict(n=15, slug="fear_of_fire", w101="173198", giver="olivia_dawnwillow",
         assign=("talk", "olivia_dawnwillow", "pesky_pirates:complete"),
         goals=[goal("goto", "H4"), goal("boss", "fear_of_fire"), goal("talk", "olivia_dawnwillow")], then=[]),
]
BY_SLUG = {q["slug"]: q for q in QUESTS}


def base(slug):
    return QUEST_BASE + 10 * BY_SLUG[slug]["n"]


def assigned(slug):
    return base(slug)


def goal_flag(slug, i):  # i counts from 1
    return base(slug) + i


def ready(slug):
    return base(slug) + 8


def complete(slug):
    return base(slug) + 9


def after_of(q, i):
    """Goal numbers (from 1) that must be done before goal i of quest q."""
    a = q["goals"][i - 1]["after"]
    return ([i - 1] if i > 1 else []) if a is None else a


def counters():
    """{(slug, goal number): counter base flag} for every defeat goal, in quest order."""
    out, k = {}, 0
    for q in QUESTS:
        for i, g in enumerate(q["goals"], 1):
            if g["type"] == "defeat":
                out[(q["slug"], i)] = COUNTER_BASE + 4 * k
                k += 1
    return out


def flag_table():
    """The flag list for the HUD (JSON-able, no W101 text)."""
    cnt = counters()
    quests = []
    for q in QUESTS:
        goals = []
        for i, g in enumerate(q["goals"], 1):
            d = dict(n=i, type=g["type"], target=g["target"], count=g["count"], flag=goal_flag(q["slug"], i),
                     after=after_of(q, i))
            if (q["slug"], i) in cnt:
                d["counter"] = {"base_flag": cnt[(q["slug"], i)], "bits": 4}
            if g["type"] == "cages":
                d["cage_flags"] = [CAGE_FLAG + c for c in range(len(CAGES))]
            if g["type"] == "talk":
                d["talk_flag"] = TALK_FLAG + NPC_INDEX[g["target"]]
            goals.append(d)
        quests.append(dict(n=q["n"], slug=q["slug"], w101_quest_id=q["w101"], giver=q["giver"],
                           assigned=assigned(q["slug"]), ready=ready(q["slug"]), complete=complete(q["slug"]),
                           goals=goals, then=q["then"]))
    return dict(
        quests=quests,
        talk_flags={slug: TALK_FLAG + k for k, (slug, *_r) in enumerate(NPCS)},
        cage_flags=[CAGE_FLAG + c for c in range(len(CAGES))],
        reserved={"grim_tales": QUEST_BASE + 70, "door_loadscreen": [1042366900, 1042366999]},
        entities=dict(mobs=MOBS, bosses=BOSSES, givers={s: e for s, _f, _p, _r, _m, e in NPCS},
                      cages=[e for _p, e in CAGES], regions=REGION_ENTITY),
        notes="assigned stays on after completion; a quest is in progress when assigned is on and complete is off; "
              "its current goal is the first goal whose flag is off and whose 'after' goals are on. Counters are "
              "4-bit event values (bit order: base flag = most significant bit, to be checked in game).")


if __name__ == "__main__":
    import json
    import sys
    json.dump(flag_table(), open(sys.argv[1], "w") if len(sys.argv) > 1 else sys.stdout, indent=1)
