"""The Unicorn Way quest line as game logic: quests, goals, event flags, entity ids (no Wizard101 text; the client's
quest text is merged in by slug outside the repo, see /mnt/project-files/unicorn_way_quests).

Event flags (all in the m60_42_36 tile's 1042362xxx flag group; 1042362900-999 is reserved for the door /
loading-screen flags). The group matters: Elden Ring keeps event flags in blocks of 1000 (flag id / 1000) and only
the blocks its own data declares exist - CSFD4VirtualMemoryFlag looks the block up in a map and silently does
nothing when it is missing (eldenring crate, cs/event_flag.rs). Across 51 vanilla EMEVDs the per-map flags only ever
use the x0xxx, x2xxx, x7xxx, x8xxx and x9xxx sub-blocks, never x3/x5/x6, and this tile's own flags are 1042360000/1,
1042362732-734, 1042368540 and 1042369205-9416. Rounds 6-9 put ours in 1042366xxx, a block that does not exist, so
setting them did nothing and reading them always returned off (round 9: the door's load-screen flag never came on).
They now live in 1042362xxx, clear of 1042362732-734:
  quest n:  QUEST_BASE + 10*n   +0 assigned (stays on), +1..+6 goal i done, +8 ready to turn in (every goal but the
                                last one done), +9 complete
  counters: kill counts as 4-bit event values (2003[31] / 3[12]), COUNTER_BASE + 4*k
  cages:    one flag per Bone Cage opened, CAGE_FLAG + c
  talk:     TALK_FLAG + npc index: rounds 10-14 set it on a talk press for the HUD's dialogue box; unused since round 15
            (ER's dialog box, DIALOG_OK / DIALOG_CANCEL = 1042362620 / 621, see emevd_quests.py)
No flag is on in a new save, so the player starts with no quests. Round 18: every quest is offered by its giver and
assigned only when accepted (yes): Unicorn Way by Merle Ambrose's message on first reaching Private Connelly at the
street entrance, every other quest by its giver in conversation (see QUESTS).

Goal types: talk ("Talk" on the NPC character, CHARACTERS), goto (player inside a sphere region), defeat (count kills of
the listed enemies while the goal is current, then the count reaches `count`; one kill counts for every current goal
that wants that enemy), boss (one kill of the boss while the goal is current), cages (all Bone Cages opened while the
quest is active). A goal is current when its quest is assigned, it is not done, and the goals in `after` (default:
the one before it) are done.
"""

QUEST_BASE = 1042362000
COUNTER_BASE = 1042362500
CAGE_FLAG = 1042362540
TALK_FLAG = 1042362600
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

# Round 15: the street's Wizard101 NPCs as Elden Ring characters (msb_add_npcs.py), talked to with ER's own "Talk"
# button and answered in ER's own dialog box (emevd_quests.py; text in the map's EventTextForMap entries, er_text.py
# --dialogue). The invisible giver assets above stay in the map unused, so nothing else's part indices move.
# Rows are vanilla tuples (model, Think, NpcParam as some vanilla map places them; no regulation edits):
#   c0000 humans: Think 523011000 / NpcParam 523010010 = White Mask Varre as vanilla m60_42_36_00 places him at The
#     First Step (friendly NPC team 26, Limgrave), the look from the CharaInitParam row (equipment, face, sex)
#   c3200 Merchant Kale (vanilla m60_42_36_00), c2010 Blaidd (m60_44_34_00), c4110 Boc the Seamster (m60_39_40_00),
#   c2180 Melina (base row 21800000, team 26; vanilla places only her Leyndell row 21801034)
# (slug, W101 name, frame, FLVER-frame position, W101 yaw degrees, model, Think, NpcParam, CharaInit, entity)
# Positions/yaws: the zone gamedata.bin (WC_Unicorn, WC_Unicorn_HedgeMaze), W101 (x, z, y) * 0.01, y lifted 0.2 m so
# the character drops onto the collision. O'Ryan stands at house H1's door (the house is not built), Lady Oriel on the
# maze's sunken floor (W101 floats her 4 m up).
# Round 16 (F1): only the NPCs a new level 1-2 wizard meets on the street and gets quests from. Removed (REMOVED):
# Diego (NPC06: PvP trainer; his only quest, WizQst162711, is a talk-to handed out by someone else), Ugo Kalahad
# (NPC08_Shop: a shop, ShoppingBehavior, no quest) and Aegon Statz (NPC12: the Gravulum Order / Pekuliarity event;
# the client's own badge and Crown Shop lines send you to him in Ravenwood, no Unicorn Way starter quest names him).
# Round 16 (F2): W101 yaw = rotation about z, applied clockwise (Gamebryo's NiMatrix3), and a W101 character's face
# is its model's -y (Human_WCGuard_A.nif: the eye bones sit at -y of the head), so it faces (-sin yaw, -cos yaw) in
# W101 (x, y). Checked on the zone itself: the Abby stand-in faces Private Connelly exactly (cos 1.000), Connelly
# faces the Hub arrival point (0.98); round 15's (-sin yaw, +cos yaw) mirrored every facing across the x axis.
# O'Ryan's yaw: facing out of house H1's door, towards the "Street 1 House 1 Exit" location (-12463.1, 8646.2).
HUMAN_THINK, HUMAN_NPC = 523011000, 523010010
CHARACTERS = [
    ("private_connelly", "Private Connelly", "street", (26.543, 0.184, 22.200), 55.86, "c0000",
     HUMAN_THINK, HUMAN_NPC, 6194, 1042360820),
    ("ceren_nightchant", "Ceren Nightchant", "street", (-41.857, 0.489, 85.837), -24.23, "c0000",
     HUMAN_THINK, HUMAN_NPC, 6164, 1042360821),
    ("olivia_dawnwillow", "Olivia Dawnwillow", "street", (-20.198, -1.174, 86.550), -177.30, "c0000",
     HUMAN_THINK, HUMAN_NPC, 23201, 1042360822),
    ("lady_oriel", "Lady Oriel", "garden", (5.974, -0.500, 0.940), 179.62, "c2180",
     21800000, 21800000, -1, 1042360823),
    ("private_oryan", "Private O'Ryan", "street", (-125.714, -0.511, 88.655), -26.27, "c0000",
     HUMAN_THINK, HUMAN_NPC, 6194, 1042360824),
]
# Round 15 characters taken out in round 16 (F1): (slug, part name, model). msb_add_npcs.py deletes these parts.
REMOVED = [
    ("diego", "c2010_9105", "c2010"),  # entity 1042360825
    ("ugo_kalahad", "c3200_9106", "c3200"),  # 1042360826
    ("aegon_statz", "c4110_9107", "c4110"),  # 1042360827
]
# Round 16 (F3): the idle each character is put in on every load (EMEVD 2003[18], as vanilla does it). W101 has every
# street NPC standing (their ObjectData: FidgetBehavior Idle / IdleFidget, no seat), so all of them stand.
#   c0000: 90100, the event idle vanilla m60_42_36 event 1042363700 plays on White Mask Varre at The First Step. With
#     NpcParam 523010010 (Varre's row: SpEffect 9820, animIdOffset 320000000) it resolves to Varre's own a320_090100.
#   c2180 Melina: 0, her standing idle (her row has no animation offset), looped.
# (animation, loop) by model
IDLE = {"c0000": (90100, 0), "c2180": (0, 1)}
CHARACTER_PART_ID = 9100  # part header 0x08 / name suffix of the first character (<model>_9100..)
TALK_NPC_BUTTON = 6000  # ActionButtonParam "Talk" (category 4, radius 2.5 m), as vanilla m60_51_36 1051360734 on a character
# Round 17: the HUD's answer to a dialogue request (DIALOGUE_FLAG + k): OK = the last page was read / the quest was
# accepted; CANCEL = closed early / the quest was declined (rounds 15-16: ER's 2007[10] OK / back buttons).
DIALOG_OK, DIALOG_CANCEL = 1042362620, 1042362621
CHARACTER = {c[0]: c for c in CHARACTERS}

# Round 17 (F2/F3): the conversations are shown by the HUD DLL in Wizard101's own dialogue box (DialogWindow.gui),
# not ER's 2007[10]. A talk event sets the request flag of a dialogue key and waits for DIALOG_OK / DIALOG_CANCEL (or
# the player walking off); the DLL reads which flag is on, shows that key's pages (the text is read at runtime from
# w101_orbs/book/dialogue.tsv on Ryan's PC, tools/w101_book_dialogue.py) and answers. 1042362400-499 is free (quests
# 000-159, crafting 160-182 / 350-399, counters 500-515, cages 540-543, talk 600-604, dialog 620/621, vanilla
# 732-734, doors 900-999).
DIALOGUE_FLAG = 1042362400
# Wizard101's portrait of each speaker (DialogWindow's imgPortrait): the NpcPortraits file the speaker's ObjectData
# template names (Root.wad ObjectData/WC/WC-ST01-NPC0n.xml; the intro is Gamma's).
PORTRAITS = {
    "private_connelly": "GUI/NpcPortraits/Human_WCGuard_A.png",  # WC-ST01-NPC04
    "ceren_nightchant": "GUI/NpcPortraits/Art_Portrait_Boy_Life02.png",  # WC-ST01-NPC01
    "olivia_dawnwillow": "GUI/NpcPortraits/Human_WCShopKeepF_A_NPC.png",  # WC-ST01-NPC03
    "lady_oriel": "GUI/NpcPortraits/Portrait_Seraph_Aerial_B_NPC.png",  # WC-ST01-NPC02
    "private_oryan": "GUI/NpcPortraits/Human_WCGuard_B.png",  # WC-ST01-NPC05
    "merle_ambrose": "GUI/NpcPortraits/Art_Portrait_Ambrose.png",  # ObjectData/WC/WC-Ambrose-PreAV.xml
    "gamma": "GUI/NpcPortraits/Portrait_Owl_Gamma_A_NPC.png",
}
PORTRAITS["intro"] = PORTRAITS["merle_ambrose"]
# Names of the speakers who are not street characters (round 18: a page can have its own speaker, e.g. Gamma's tips
# or Olivia cutting into Ceren's Skeleton Crew talk, as the client's lines have it).
SPEAKERS = {"intro": "Merle Ambrose", "merle_ambrose": "Merle Ambrose", "gamma": "Gamma"}


def follow_up(q, i):
    """The quest the npc of talk goal i of quest q offers at the end of that same conversation, or None: goal i
    completes q and that npc hands out a quest once q is complete (round 18 F2: Connelly closes Unicorn Way and offers
    Ghost Hunters in one conversation, as in Wizard101)."""
    g = q["goals"][i - 1]
    if g["type"] != "talk" or i != len(q["goals"]):
        return None
    return next((r for r in QUESTS if r["assign"] and r["assign"][0] == "talk" and r["assign"][1] == g["target"]
                 and r["assign"][2] == f"{q['slug']}:complete"), None)


def dialogue_card(key):
    """The quest whose card ends conversation `key` with Wizard101's yes / no (accept / decline), or None: the
    intro's offer, an npc's offer, and a talk goal that hands out the next quest (follow_up)."""
    parts = key.split("|")
    if parts[0] == "intro":
        return parts[1] if len(parts) == 2 else None
    if len(parts) == 3 and parts[2] == "offer":
        return parts[1]
    if len(parts) == 3 and parts[2].startswith("goal"):
        nxt = follow_up(BY_SLUG[parts[1]], int(parts[2][4:]))
        return nxt["slug"] if nxt else None
    return None


def dialogue_keys():
    """{dialogue key: request flag}, in a fixed order that doesn't depend on the dialogue text: the intros of quests
    handed out on arrival (the giver's offer, then what follows it: Gamma's pointer), then per character its talk
    goals, the quests it offers, the quests it is part of (underway) and its idle line."""
    keys = []
    for q in QUESTS:
        if q["assign"] and q["assign"][0] == "intro":
            keys += [f"intro|{q['slug']}", f"intro|{q['slug']}|after"]
    for slug, *_rest in CHARACTERS:
        talks = [(q, i) for q in QUESTS for i, g in enumerate(q["goals"], 1)
                 if g["type"] == "talk" and g["target"] == slug]
        keys += [f"{slug}|{q['slug']}|goal{i}" for q, i in talks]
        offers = [q for q in QUESTS if q["assign"] and q["assign"][0] == "talk" and q["assign"][1] == slug]
        keys += [f"{slug}|{q['slug']}|offer" for q in offers]
        # Round 18: what the giver says once the quest is accepted (and Gamma's tips that come with it), as
        # Ambrose's "Splendid!" after Unicorn Way.
        keys += [f"{slug}|{q['slug']}|accepted" for q in offers]
        keys += [f"{slug}|{q['slug']}|underway" for q in QUESTS
                 if q["giver"] == slug or any(t[0] is q for t in talks)]
        keys.append(f"{slug}|idle")
    assert len(keys) <= 100 and len(set(keys)) == len(keys)
    return {key: DIALOGUE_FLAG + k for k, key in enumerate(keys)}

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


# n, slug, W101 quest id, giver, how it is assigned, goals, quests assigned on completion.
# Round 18 (F2): every quest is handed out by its giver and only becomes the player's once accepted, as in Wizard101
# (the client's own lines: Ambrose "Care to put your talents to the test?" / "Splendid!", Gamma "Olivia is offering
# you a Side Quest. Once you accept it..."). assign:
#   ("intro", region, None): Merle Ambrose's offer, shown as his message on first reaching `region` (in Wizard101 he
#       gives Unicorn Way in his office before teleporting the student to the street; Gamma's pointer follows)
#   ("talk", npc, prereq): the npc offers it once prereq ("quest" = assigned, "quest:complete") holds; when the talk
#       goal that completes the prereq quest is with the same npc, the offer is the end of that same conversation
#       (Connelly: Unicorn Way done -> Ghost Hunters), else the npc shows "!" and offers it on the next talk.
# `then` is no longer used to hand out quests (rounds 10-17 assigned the next main quest without asking).
QUESTS = [
    dict(n=0, slug="unicorn_way", w101="1625BF", giver="merle_ambrose", assign=("intro", "street_start", None),
         goals=[goal("talk", "private_connelly")], then=[]),
    dict(n=1, slug="ghost_hunters", w101="1625C2", giver="private_connelly",
         assign=("talk", "private_connelly", "unicorn_way:complete"),
         goals=[goal("defeat", "lost_soul", 2), goal("talk", "private_connelly"), goal("talk", "ceren_nightchant")],
         then=[]),
    dict(n=2, slug="skeleton_crew", w101="1625C1", giver="ceren_nightchant",
         assign=("talk", "ceren_nightchant", "ghost_hunters:complete"),
         goals=[goal("defeat", "skeletal_pirate", 2), goal("talk", "ceren_nightchant")], then=[]),
    dict(n=3, slug="monsters_and_mazes", w101="1625C5", giver="ceren_nightchant",
         assign=("talk", "ceren_nightchant", "skeleton_crew:complete"),
         goals=[goal("goto", "hedge_maze"), goal("talk", "lady_oriel")], then=[]),
    dict(n=4, slug="finding_out_why", w101="1625CA", giver="lady_oriel",
         assign=("talk", "lady_oriel", "monsters_and_mazes:complete"),
         goals=[goal("cages", "bone_cage", 4, after=[]), goal("defeat", "dark_fairy", 2, after=[]),
                goal("goto", "hedge_maze", after=[1, 2]), goal("talk", "lady_oriel")], then=[]),
    dict(n=5, slug="who_could_it_be_now", w101="1625CE", giver="lady_oriel",
         assign=("talk", "lady_oriel", "finding_out_why:complete"),
         goals=[goal("talk", "ceren_nightchant")], then=[]),
    dict(n=6, slug="sinister_skeleton", w101="1625CF", giver="ceren_nightchant",
         assign=("talk", "ceren_nightchant", "who_could_it_be_now:complete"),
         goals=[goal("goto", "T2"), goal("boss", "rattlebones"), goal("goto", "near_ceren"),
                goal("talk", "ceren_nightchant")], then=[]),  # Grim Tales (n=7) is off the street: not offered
    dict(n=10, slug="pesky_pirates", w101="1625C9", giver="olivia_dawnwillow",
         assign=("talk", "olivia_dawnwillow", "skeleton_crew"),
         goals=[goal("defeat", "skeletal_pirate", 2), goal("talk", "olivia_dawnwillow")], then=[]),
    dict(n=11, slug="saving_private_oryan", w101="1625DA", giver="private_connelly",
         assign=("talk", "private_connelly", "ghost_hunters:complete"),
         goals=[goal("goto", "near_oryan"), goal("talk", "private_oryan"), goal("goto", "T1"),
                goal("boss", "lady_blackhope"), goal("talk", "private_oryan"), goal("talk", "private_connelly")],
         then=[]),
    # Wizard101 gives The Power is Ours at the Then-Faire (off the street); Olivia stands in as its giver here.
    dict(n=15, slug="fear_of_fire", w101="173198", giver="olivia_dawnwillow",
         assign=("talk", "olivia_dawnwillow", "pesky_pirates:complete"),
         goals=[goal("goto", "H4"), goal("boss", "fear_of_fire"), goal("talk", "olivia_dawnwillow")], then=[]),
]
BY_SLUG = {q["slug"]: q for q in QUESTS}

# Round 18 (F3): the picture Wizard101's quest card shows for a goal (QuestInfoWindow's imgGoalObjective1): the
# portrait of the one to talk to or defeat (GUI/NpcPortraits, the files the mobs' and NPCs' ObjectData name), the Bone
# Cage's quest art; none for a place (Wizard101 shows a location picture there, and Unicorn Way's places have none).
GOAL_PICTURES = {  # Root.wad ObjectData/WC/<template>.xml of each mob
    "lost_soul": "GUI/NpcPortraits/Portrait_Ghost_LostSoul_MOB_A.png",  # Ghost-Blue-L01
    "skeletal_pirate": "GUI/NpcPortraits/Portrait_Skeleton_Pirate_A_MOB.png",  # WC-UW-Skeleton-Pirate-L01
    "dark_fairy": "GUI/NpcPortraits/Fairy_DarkMinion_MOB.png",  # WC-UW-Sprite-Dark-L01
    "rattlebones": "GUI/NpcPortraits/Portrait_Skeleton_Armored_A_NPC.png",  # WC-StandIn-Rattlebones
    "lady_blackhope": "GUI/NpcPortraits/Art_Portrait_GhostLady_Green.png",  # Heroic_WC_Blackhope
    "fear_of_fire": "GUI/NpcPortraits/Art_Portrait_Scarecrow.png",  # WC_StandIn_FearOfFire
    "bone_cage": "GUI/QuestButtons/Art_HD_WC_BoneCage.png",
}
GOAL_PICTURES.update({slug: PORTRAITS[slug] for slug in PORTRAITS if slug != "intro"})


def goal_picture(g):
    """The quest card's picture of goal g (an art path under w101_orbs/book/art), or ""."""
    return GOAL_PICTURES.get(g["target"], "")


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
        reserved={"grim_tales": QUEST_BASE + 70, "door_loadscreen": [1042362900, 1042362999]},
        entities=dict(mobs=MOBS, bosses=BOSSES, givers={s: e for s, _f, _p, _r, _m, e in NPCS},
                      cages=[e for _p, e in CAGES], regions=REGION_ENTITY),
        notes="assigned stays on after completion; a quest is in progress when assigned is on and complete is off; "
              "its current goal is the first goal whose flag is off and whose 'after' goals are on. Counters are "
              "4-bit event values (bit order: base flag = most significant bit, to be checked in game).")


if __name__ == "__main__":
    import json
    import sys
    json.dump(flag_table(), open(sys.argv[1], "w") if len(sys.argv) > 1 else sys.stdout, indent=1)
