"""unicorn_way_quests.json (the quest line with its goals, ER event flags and entities, read by w101_book_quests.py)
from the quest structure written out below (from the wikis and the client dialogue) and the client text dump uw_lang.json
(python -I w101_lang.py dump Root.wad uw_lang.json). Round 18 data script, moved into the repo in round 35 (R-DOC).
The output holds Wizard101 quest titles: personal use, never commit it.

Usage: python -I w101_quests_json.py uw_lang.json unicorn_way_quests.json"""
import json, sys, math

LANG = sys.argv[1]
OUT = sys.argv[2]
D = json.load(open(LANG, encoding="utf-8"))
TITLE = {k: v for k, _, v in D["QuestTitle"]}

FLAG_BASE = 1042366000  # quest n: base + 10*n (+0 active, +1..+6 goal i done, +8 ready to turn in, +9 complete)
COUNTER_BASE = 1042366500
TALK_BASE = 1042366600
ENTITY = {  # proposed entity ids (MSB), check against the map before use
    "lost_soul": [*range(1042360860, 1042360864), *range(1042360870, 1042360886)],  # round 6: 5 per path
    "skeletal_pirate": [*range(1042360864, 1042360868), *range(1042360886, 1042360902)],
    "dark_fairy": [*range(1042360868, 1042360870), *range(1042360902, 1042360910)],
    "rattlebones": [1042360850],
    "lady_blackhope": [1042360851],
    "fear_of_fire": [1042360852],
}
GROUP = {"lost_soul": 1042365860, "skeletal_pirate": 1042365864, "dark_fairy": 1042365868}  # MSB EntityGroupIDs

th = math.radians(-27.01); C, S = math.cos(th), math.sin(th)
def street_to_er(x, y, z):  # W101 street coords -> m60_42_36_00 local (street part at (-8.56, 94.0, -75.91), rot Y -27.01)
    fx, fy, fz = x * 0.01, z * 0.01, y * 0.01
    return [round(-8.56 + fx * C + fz * S, 2), round(94.0 + fy, 2), round(-75.91 - fx * S + fz * C, 2)]

NPCS = {
    "merle_ambrose": dict(name="Merle Ambrose", template="WC_StandIn_MerleAmbrose", where="Ravenwood (Headmaster's office); not on Unicorn Way"),
    "private_connelly": dict(name="Private Connelly", template="WC-ST01-NPC04", model="Human_WCGuard_A.nif", portrait="GUI/NpcPortraits/Human_WCGuard_A.dds", where="street, by the Unicorn Way entrance", w101=[2654.3, 2220.0, -1.6]),
    "ceren_nightchant": dict(name="Ceren Nightchant", template="WC-ST01-NPC01", model="P_B_WC_LifeBoy_s.nif", portrait="GUI/NpcPortraits/Art_Portrait_Boy_Life02.dds", where="street, middle", w101=[-4185.7, 8583.7, 28.9]),
    "olivia_dawnwillow": dict(name="Olivia Dawnwillow", template="WC-ST01-NPC03", model="Human_WCShopKeepF_A.nif", portrait="GUI/NpcPortraits/Human_WCShopKeepF_A_NPC.dds", where="street, next to Ceren", w101=[-2019.8, 8655.0, -137.4]),
    "lady_oriel": dict(name="Lady Oriel", template="WC-ST01-NPC02", model="Seraph_Aerial_B.nif", portrait="GUI/NpcPortraits/Portrait_Seraph_Aerial_B_NPC.dds", where="Hedge Maze (zone WC_Unicorn_HedgeMaze), floats ~4 m up", w101_zone="WC_Unicorn_HedgeMaze", w101=[597.4, 94.0, 404.5]),
    "private_oryan": dict(name="Private O'Ryan", template="WC-ST01-NPC05 (in house H1) / WC-ST01-NPC05-B (street, after the quest)", model="Human_WCGuard_B.nif", portrait="GUI/NpcPortraits/Human_WCGuard_B.dds", where="house H1 (WC_Unicorn_H1), later the street near the entrance", w101_zone="WC_Unicorn_H1", w101=[172.2, -66.3, 2.0], w101_street=[880.3, 894.5, 0.1]),
    "dorothy_gale": dict(name="Dorothy Gale", template="WC-ST01-NPC09-new", where="house H2 (WC_Unicorn_H2)", w101_zone="WC_Unicorn_H2"),
    "tin_man": dict(name="Tin Man", template="WC-ST01-NPC11 (+ Mr. Toto WC-ST01-NPC10)", where="house H3 (WC_Unicorn_H3)", w101_zone="WC_Unicorn_H3"),
    "harold_argleston": dict(name="Harold Argleston", where="Wizard City Library, The Commons; not on Unicorn Way"),
    "diego": dict(name="Diego the Duelmaster", template="WC-ST01-NPC06", where="street near the entrance (duel tutorials)", w101=[-1253.4, 2211.9, 30.3]),
    "gamma": dict(name="Gamma", template="WC-ST01-NPC07", where="tutorial owl (popup hints in the quest text)"),
}
for n in NPCS.values():
    if "w101" in n and "w101_zone" not in n:
        n["er_local"] = street_to_er(*n["w101"])
    if "w101_street" in n:
        n["er_local_street"] = street_to_er(*n["w101_street"])

MOBS = {
    "lost_soul": dict(w101="Lost Soul (WizardMobs 00000116, template Ghost-Blue, Death)", er="Spirit Jellyfish c4180 x20", entities=ENTITY["lost_soul"], group=GROUP["lost_soul"]),
    "skeletal_pirate": dict(w101="Skeletal Pirate (WizardMobs 00000330, Death)", er="curved-sword skeleton c3500 x20", entities=ENTITY["skeletal_pirate"], group=GROUP["skeletal_pirate"]),
    "dark_fairy": dict(w101="Dark Fairy (WizardMobs 00000021; classic name Dark Sprite, template Sprite-Dark)", er="Winged Misbegotten c3450 x10", entities=ENTITY["dark_fairy"], group=GROUP["dark_fairy"]),
    "rattlebones": dict(w101="Rattlebones (WizardMobs 00000032, boss, zone WC_Unicorn_T2 'The Archives')", er="Giant Skeleton Torso c4960_9000", entities=ENTITY["rattlebones"]),
    "lady_blackhope": dict(w101="Lady Blackhope (WizardMobs 00000020, boss, zone WC_Unicorn_T1 'Blackhope Tower')", er="not placed (T1 room empty)", entities=ENTITY["lady_blackhope"]),
    "fear_of_fire": dict(w101="Fear of Fire (WizardMobs 00003854, scarecrow, zone WC_Unicorn_H4 'Thief's House')", er="not placed (H4 clubhouse empty)", entities=ENTITY["fear_of_fire"]),
}

PLACES = {
    "street": "Unicorn Way street (m60_42_36_00, AEG030_518_9000)",
    "hedge_maze": "Hedge Maze garden (aeg030_555, regions 1042362804-807)",
    "T1": "Blackhope Tower (AEG030_519_9001, regions 1042362816-819)",
    "T2": "The Archives = Rattlebones' tower (AEG030_519_9000, regions 1042362800-803, arena 1042362808)",
    "H1": "O'Ryan's house (not built; WC_NPC_House_01.nif, shared by H1-H3)",
    "H2": "Dorothy's house (not built)",
    "H3": "Tin Man's house (not built)",
    "H4": "Thief's House = clubhouse interior (aeg030_557, regions 1042362820-823)",
}

def g(type_, target, count=1, location="street", text=None, **kw):
    d = dict(type=type_, target=target, count=count, location=location)
    if text: d["display"] = text
    d.update(kw)
    return d

W = "https://walkthroughwizard.com/all-posts/mmorpgs/wizard101/wizard101-main-questline-guides/wizard101-all-main-quests-in-wizard-city/"
R = "https://raiderking.com/wizard101-wizard-city-main-quest-tree/"
F = "https://www.wizard101.com/forum/wizard-city/guide-for-new-players-3--8ad6a41453c6974a0153cc973a99776c"
U = "https://101universe.fandom.com/wiki/Unicorn_Way"
O = "https://101universe.fandom.com/wiki/Olivia_Dawnwillow"
CL = "client (Root.wad Locale/en-US WizQst<id>.lang, QuestTitle.lang)"

QUESTS = [
    dict(n=0, slug="unicorn_way", wid="1625BF", kind="main", giver="merle_ambrose", prereqs=[],
         goals=[g("talk", "private_connelly", text="Talk to Private Connelly")], turn_in="private_connelly", next="ghost_hunters",
         notes="Ambrose teleports the player straight to Unicorn Way. ER: start on first arrival through the graveyard door (flag 1042368540 / title event 1042362991).",
         sources=[W, R, CL]),
    dict(n=1, slug="ghost_hunters", wid="1625C2", alt_text="1625C0", kind="main", giver="private_connelly", prereqs=["unicorn_way"],
         goals=[g("defeat", "lost_soul", 2, text="Defeat Lost Souls in Unicorn Way"), g("talk", "private_connelly", text="Talk to Private Connelly"), g("talk", "ceren_nightchant", text="Talk to Ceren Nightchant")],
         turn_in="ceren_nightchant", next="skeleton_crew", notes="Count 2 per raiderking/walkthroughwizard (a 2016 forum guide says 3).", sources=[W, R, F, CL]),
    dict(n=2, slug="skeleton_crew", wid="1625C1", kind="main", giver="ceren_nightchant", prereqs=["ghost_hunters"],
         goals=[g("defeat", "skeletal_pirate", 2, text="Defeat Skeletal Pirates in Unicorn Way"), g("talk", "ceren_nightchant", text="Talk to Ceren Nightchant")],
         turn_in="ceren_nightchant", next="monsters_and_mazes", unlocks=["pesky_pirates"],
         notes="The pirates' parting line ('rattle in the Darkness') is the story hook.", sources=[W, R, CL]),
    dict(n=3, slug="monsters_and_mazes", wid="1625C5", kind="main", giver="ceren_nightchant", prereqs=["skeleton_crew"],
         goals=[g("goto", "hedge_maze", location="hedge_maze", text="Go to the Hedge Maze"), g("talk", "lady_oriel", location="hedge_maze", text="Talk to Lady Oriel in the Hedge Maze")],
         turn_in="lady_oriel", next="finding_out_why", rewards="temporary mount (1 day)", sources=[W, R, CL]),
    dict(n=4, slug="finding_out_why", wid="1625CA", kind="main", giver="lady_oriel", prereqs=["monsters_and_mazes"],
         goals=[g("use", "bone_cage", 4, text="Open Bone Cages"), g("defeat_collect", "dark_fairy", 2, item="Fairy Dust", text="Defeat Dark Fairies and Collect Fairy Dust"),
                g("goto", "hedge_maze", location="hedge_maze", text="Go to Lady Oriel"), g("talk", "lady_oriel", location="hedge_maze", text="Talk to Lady Oriel")],
         turn_in="lady_oriel", next="who_could_it_be_now", sources=[R, W, F, CL],
         notes="Bone Cages: client objects WC-FairyCage02..05 (4 quest cages) + 6 generic WC-FairyCage, interactable name 'Bone Cage'. Fairy Dust is a drop chance in W101; ER: 1 per Dark Fairy kill."),
    dict(n=5, slug="who_could_it_be_now", wid="1625CE", kind="main", giver="lady_oriel", prereqs=["finding_out_why"],
         goals=[g("talk", "ceren_nightchant", text="Talk to Ceren Nightchant")], turn_in="ceren_nightchant", next="sinister_skeleton", sources=[W, R, CL]),
    dict(n=6, slug="sinister_skeleton", wid="1625CF", kind="main", giver="ceren_nightchant", prereqs=["who_could_it_be_now"],
         goals=[g("goto", "T2", location="T2", text="Search for Rattlebones (The Archives)"), g("defeat", "rattlebones", 1, location="T2", text="Defeat Rattlebones"),
                g("goto", "street", text="Return to Ceren Nightchant"), g("talk", "ceren_nightchant", text="Talk to Ceren Nightchant")],
         turn_in="ceren_nightchant", next="grim_tales", rewards="the Tome (story item)", sources=[R, W, CL]),
    dict(n=7, slug="grim_tales", wid="1625CC", kind="main", giver="ceren_nightchant", prereqs=["sinister_skeleton"],
         goals=[g("talk", "harold_argleston", location="off_street", text="Talk to Harold Argleston in the Wizard City Library"), g("talk", "merle_ambrose", location="off_street", text="Talk to Merle Ambrose")],
         turn_in="merle_ambrose", next="To Ravenwood! (1625D7, off Unicorn Way)", rewards="Treasure Card (Argleston)",
         notes="Title from the wikis (QuestTitle has no entry for 1625CC). Both NPCs are off the street: in ER end the chain here or stand Argleston/Ambrose in at the grace.", sources=[R, W, CL]),
    dict(n=10, slug="pesky_pirates", wid="1625C9", kind="side", giver="olivia_dawnwillow", prereqs=["skeleton_crew (started)"],
         goals=[g("defeat", "skeletal_pirate", 2, text="Defeat Skeletal Pirates in Unicorn Way"), g("talk", "olivia_dawnwillow", text="Talk to Olivia Dawnwillow")],
         turn_in="olivia_dawnwillow", rewards="Magic Ring", notes="Count not on any wiki we could reach (101universe: 'defeat a Skeletal Pirate'); 2 chosen.", sources=[O, CL]),
    dict(n=11, slug="saving_private_oryan", wid="1625DA", kind="side", giver="private_connelly", prereqs=["ghost_hunters (assumed)"],
         goals=[g("goto", "H1", location="H1", text="Find Private O'Ryan (a house with a light on)"), g("talk", "private_oryan", location="H1", text="Talk to Private O'Ryan"),
                g("goto", "T1", location="T1", text="Go to Lady Blackhope's Tower"), g("defeat", "lady_blackhope", 1, location="T1", text="Defeat Lady Blackhope"),
                g("talk", "private_oryan", text="Talk to Private O'Ryan"), g("talk", "private_connelly", text="Talk to Private Connelly")],
         turn_in="private_connelly", notes="Goal order rebuilt from the client dialogue (no wiki page reachable). Classic version 94F3: defeat Skeletal Pirates outside his house instead.", sources=[CL]),
    dict(n=12, slug="release_the_fairies", wid="1625CB", kind="side", giver="lady_oriel", prereqs=["monsters_and_mazes"],
         goals=[g("use", "bone_cage", 4, text="Free the Fairies from the Bone Cages"), g("talk", "lady_oriel", location="hedge_maze", text="Talk to Lady Oriel")],
         turn_in="lady_oriel", rewards="healing spell", notes="Overlaps the cage goal of Finding Out Why; wikis list the cages under Finding Out Why. ER: optional, uses the 6 generic cages.", sources=[CL]),
    dict(n=13, slug="yellow_brick_road", wid="13D86", kind="side", giver="private_connelly", prereqs=[],
         goals=[g("goto", "H2", location="H2", text="Check on Dorothy (house before the Hedge Maze)"), g("talk", "dorothy_gale", location="H2", text="Talk to Dorothy Gale"), g("talk", "private_connelly", text="Talk to Private Connelly")],
         turn_in="private_connelly", next="not_in_kansas_anymore", notes="Order inferred from client dialogue; houses H2/H3 not built.", sources=[CL]),
    dict(n=14, slug="not_in_kansas_anymore", wid="13D82", kind="side", giver="dorothy_gale", prereqs=["yellow_brick_road"],
         goals=[g("goto", "H3", location="H3", text="Find Dorothy's Friends"), g("talk", "tin_man", location="H3", text="Talk to Tin Man"), g("talk", "dorothy_gale", location="H2", text="Talk to Dorothy Gale")],
         turn_in="dorothy_gale", sources=[CL]),
    dict(n=15, slug="fear_of_fire", wid="173198", kind="side", giver="olivia_dawnwillow", prereqs=["pesky_pirates"],
         goals=[g("goto", "H4", location="H4", text="Go to the Thief's House"), g("defeat", "fear_of_fire", 1, location="H4", text="Defeat Fear of Fire"),
                g("talk", "olivia_dawnwillow", text="Talk to Olivia Dawnwillow")],
         turn_in="olivia_dawnwillow", rewards="Ring of Fire",
         notes="Stand-alone side quest in the mod (Ryan, 2026-10-23), given by Olivia after Pesky Pirates. In W101 it is part of the late Then-Faire quest The Power is Ours; its text is kept here.", sources=[CL]),
]

CLASSIC = [  # retired pre-2023 chain, text still in the client
    ("955B", "Unicorn's Folly", "Ambrose -> letter to Private Stillson at the gate -> Private Connelly"),
    ("94F2", "Ghost Hunters", "Connelly: defeat Lost Souls -> Ceren"),
    ("94E5", "Pesky Pirates", "Olivia: drive the Skeletal Pirates out (looting houses)"),
    ("94F3", "Saving Private O'Ryan", "Connelly: O'Ryan stuck in his house, defeat Skeletal Pirates outside"),
    ("94EC", "Fairies Wear Boots", "check whether the Fairies are good or evil (Dark Fairies)"),
    ("94ED", "They're Not Bad . . .", "speak to Lady Oriel in the Hedge Maze"),
    ("94EE", "Finding Out Why", "battle Dark Sprites for clues (bone key)"),
    ("94EF", "(Fairy Cages)", "open a cage with the bone key"),
    ("9AE8", "Finding Out Why", "defeat Dark Fairies, collect Fairy Dust"),
    ("9AEA", "Freedom!", "free the Fairies from the Bone Cages (healing spell)"),
    ("9AEB", "Sinister Skeleton", "Oriel -> Ceren: who clacks? Rattlebones"),
    ("298E2", "Stop the Maker", "Ceren: defeat Rattlebones in Delia's tower (94F0 = older copy)"),
    ("94F6", "Rattlebones Report", "report to Headmaster Ambrose"),
    ("957B", "Olde News", "Sergeant Muldoon (Olde Town): defeat Lady Blackhope in her tower"),
    ("AC51", "No Hope", "bounty hunter: defeat Lady Blackhope"),
]

def text_of(wid):
    return {k: v for k, _, v in D.get("WizQst" + wid, [])}

out = dict(
    about="Unicorn Way quest line for the Elden Ring W101 mod. Structure from wikis + client dialogue; text from Ryan's own W101 client. Personal use only: do not commit to GitHub.",
    flags=dict(scheme="quest n: %d + 10*n; +0 active, +1..+6 goal i done, +8 ready to turn in, +9 complete" % FLAG_BASE,
               counters="event values from %d (4 bits each)" % COUNTER_BASE, talk="talk requests from %d (one per NPC)" % TALK_BASE),
    npcs=NPCS, mobs=MOBS, places=PLACES, quests=[], classic_retired=[],
    bone_cages=[dict(template=t, w101=list(w), er_local=street_to_er(*w)) for t, w in [
        ("WC-FairyCage02", (-23746.3, 14174.1, -99.9)), ("WC-FairyCage03", (-21918.7, 14276.2, -99.9)),
        ("WC-FairyCage04", (-25955.2, 18790.3, 0.0)), ("WC-FairyCage05", (-2283.9, 8256.9, -82.8)),
        ("WC-FairyCage", (-25905.6, 18703.9, 0.1)), ("WC-FairyCage", (-23742.5, 14279.2, -100.8)), ("WC-FairyCage", (-21915.1, 14399.3, -99.9)),
        ("WC-FairyCage", (-15885.2, 14149.2, 8.2)), ("WC-FairyCage", (-18203.6, 12619.2, 0.0)), ("WC-FairyCage", (-25966.3, 20284.5, 0.0))]])
counter = COUNTER_BASE
npc_index = {k: i for i, k in enumerate(NPCS)}
for q in QUESTS:
    base = FLAG_BASE + 10 * q["n"]
    goals = []
    for i, gl in enumerate(q["goals"], 1):
        gl = dict(gl, flag=base + i)
        if gl["type"] in ("defeat", "defeat_collect", "use") and gl["count"] > 1:
            gl["counter"] = dict(base_flag=counter, bits=4); counter += 4
        if gl["type"] == "talk" and gl["target"] in npc_index:
            gl["talk_flag"] = TALK_BASE + npc_index[gl["target"]]
        goals.append(gl)
    entry = dict(id=q["slug"], w101_quest_id=q["wid"], title=TITLE.get(q["wid"]) or {"1625CC": "Grim Tales"}.get(q["wid"]),
                 kind=q["kind"], giver=q["giver"], prereqs=q["prereqs"], goals=goals, turn_in=q.get("turn_in"), next=q.get("next"),
                 unlocks=q.get("unlocks", []), rewards=q.get("rewards"), notes=q.get("notes"), sources=q["sources"],
                 flags=dict(active=base, ready=base + 8, complete=base + 9),
                 text=text_of(q["wid"]))
    if q.get("alt_text"):
        entry["alt_text"] = {"w101_quest_id": q["alt_text"], "title": TITLE.get(q["alt_text"]), "text": text_of(q["alt_text"])}
    out["quests"].append(entry)
for wid, title, summary in CLASSIC:
    out["classic_retired"].append(dict(w101_quest_id=wid, title=TITLE.get(wid) or title, summary=summary, text=text_of(wid)))
out["names"] = dict(npc={k: v for k, _, v in D["WC-NPCs"] if k in ("00000024", "00000027", "00000030", "00000021", "00000058", "00000735", "00000736", "00000739")},
                    mobs={k: v for k, _, v in D["WizardMobs"] if k in ("00000020", "00000021", "00000032", "00000116", "00000330", "00003854")},
                    zone={k: v for k, _, v in D["WizardZone"] if k in ("00000067", "00000077", "00000384")},
                    interactable={k: v for k, _, v in D["Interactables"] if k in ("FairyCage", "00000008")},
                    goal_verbs=sorted({v for k, _, v in D["WizardQuestGoals"] if v in ("Defeat and Collect", "Talk to", "Go to", "Open", "Free", "Find", "Explore", "Return to", "Collect", "Search", "Locate", "Rescue")}),
                    goal_templates={k: v for k, _, v in D["Quest"] if k in ("GoalCounter", "BountyNewGoalPopupText", "BountyGoalReward", "PersonaNewGoalPopupText", "UsageGoalReward", "NewQuestPopup", "QuestCompletePopup", "BountyCollectGoalReward", "WaypointNewGoalPopupText")})
sys.path.insert(0, "/home/claude/eldenring-w101/tools")
import w101_quests as WQ
FT = {q["slug"]: q for q in WQ.flag_table()["quests"]}
for q in out["quests"]:
    t = FT.get(q["id"])
    q["implemented"] = t is not None
    if t:
        assert len(t["goals"]) == len(q["goals"]), q["id"]
        q["flags"] = dict(assigned=t["assigned"], ready=t["ready"], complete=t["complete"])
        q["next"] = t["then"] or q["next"]
        for g, tg in zip(q["goals"], t["goals"]):
            for k in ("flag", "counter", "talk_flag"):
                g.pop(k, None)
            g.update({k: tg[k] for k in ("flag", "after", "counter", "talk_flag", "cage_flags") if k in tg})
    else:
        q["flags"] = None
        for g in q["goals"]:
            for k in ("flag", "counter", "talk_flag"):
                g.pop(k, None)
out["flags"] = {k: v for k, v in WQ.flag_table().items() if k != "quests"}
json.dump(out, open(OUT, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print("quests", len(out["quests"]), "classic", len(out["classic_retired"]), "counter flags up to", counter - 1)
for q in out["quests"]:
    print(q["flags"], q["w101_quest_id"], q["title"], len(q["text"]), [ (x["type"], x["target"], x["count"]) for x in q["goals"]])
