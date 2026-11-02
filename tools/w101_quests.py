"""The Unicorn Way quest line as game logic: quests, goals, event flags, entity ids (no Wizard101 text; the client's
quest text is merged in by slug outside the repo, see /mnt/project-files/unicorn_way_quests).

Event flags (round 21: saved quest state in 1042360xxx, see QUEST_BASE; the rest in the 1042362xxx group; 1042362900-999 is reserved for the door /
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

# Round 21 (Q1): the saved quest state moved to the tile's 1042360xxx block. Ryan's round 20 log: every flag of the
# 1042362xxx block reads off after each map load (warp, death, loading the save; the save file holds them, the map
# load clears them), so the reset re-ran and Unicorn Way paid again. 1042360xxx is saved and kept: the First Step
# grace (1042360000/1) and this tile's item pickups (ItemLotParam_map 1042360010-060) live there. Free in vanilla
# (51 EMEVDs + regulation scanned): 1042360300-459, 500-543, 689-692. Dialogue requests, dialog answers, crafting
# and door flags stay in 1042362xxx: they only live for a moment and a load clearing them is right.
QUEST_BASE = 1042360300
COUNTER_BASE = 1042360500
CAGE_FLAG = 1042360540
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
    ("lady_oriel", "garden", (5.975, -0.7, 3.137), 180.0, GIVER_MODEL, 1042361873),  # NPC02, on the sunken centre floor
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
# Round 24 (N1, Ryan's picks): the stand-ins are now these vanilla NPCs, their own rows (vanilla regulation, Smithbox
# row names; the base row of each: team 26 = friendly NPC, no item lot, no extra SpEffect): (Think, NpcParam,
# CharaInitParam). c0000 humans wear their CharaInit row's look; c2170 / c5330 are their own models (CharaInit -1).
#   Private Connelly = Knight Bernahl (Beast Champion armour, row 23260), Ceren Nightchant = Brother Corhyn (Scribe
#   Corhyn's rows; Corhyn's Robe, 23510), Private O'Ryan = Castellan Edgar (23110, named "Edgar the Revenger" by
#   Smithbox: the only Edgar look row), Olivia Dawnwillow = Finger Reader Enia (c2170), Lady Oriel = St. Trina (c5330).
# Rounds 15-23: Varre's rows with the 6194 / 6164 / 23201 looks and Melina (c2180 21800000).
BERNAHL = (523260000, 523260010, 23260)
CORHYN = (523510000, 523510034, 23510)
# Round 27 (A4, Ryan: "Connelly is missing when I load in; he appears after a Roundtable Hold trip"): his round 24-26
# NpcParam 523260000 has disableInitializeDead 0 (like every NPC's base row x0000; Varre's 523010010 has 1), so once
# Bernahl had been killed (before round 26 made the NPCs immortal) the save loaded him dead until a rest / warp
# respawned him (Ryan's 22:59:42 log: "markers: dead street NPCs now [1042360820]" right after loading; 23:03:32
# after the Roundtable trip: "[]"). Rows 523260010 / 523510034 are the same NPCs with disableInitializeDead 1
# (differences: an item lot on death, NG+ scaling SpEffects 19351/19501 and 9630 status rates; they are immortal).
# Round 27 (A7, Ryan: "Olivia: if Enia has no standing idle, use Rya the Scout standing"): c2170 has none (her rig is
# seated even in its bind pose: pelvis 1.28 m, thighs level, feet 0.68 m up; no animation moves the pelvis), so
# Olivia = Rya the Scout: c0000 with her CharaInit 23130 and her own rows (NpcParam 523130000: SpEffect 9829,
# animIdOffset 329000000 -> 90100 = a329_090100, standing: pelvis 0.97, feet 0.11, head 1.40 m). Her base row has
# disableInitializeDead 0 too (no team-26 row with 1 and a Think); she is immortal from the idle event on.
RYA = (523130000, 523130000, 23130)
EDGAR = (523110000, 523110000, 23110)
ENIA = (21700000, 21700000, -1)
TRINA = (53300000, 53300000, -1)
# Round 26 (N1, Ryan: "Connelly stands where Ugo should, Ceren where Olivia should, Olivia where Connelly should, it's
# like you're just guessing"): rounds 15-25 read every spot with w101_lang.py `pos`, which paired each object's name
# with the NEXT object's location (the serializer writes m_location before m_name), so each NPC stood on its
# neighbour's spot with its neighbour's yaw, and round 25 (M7) then "fixed" Connelly by guessing the templates were
# swapped. Proof now (tools/w101_npc_spots.py on WizardCity-WC_Streets-WC_Unicorn.wad + Root.wad): instance ->
# m_templateID -> TemplateManifest -> ObjectData/WC/WC-ST01-NPCnn.xml -> display name key WC-NPCs_000000nn -> .lang
# text, with each object's own position/yaw from one property record; and the zone's "Player Stand In instance -
# <name>" cutscene markers stand 2.75 / 3.47 / 2.96 m straight in front of Connelly / Ceren / Diego (facing cos
# 1.000 / 1.000 / 0.991). Local y = W101 z * 0.01 + SPAWN_LIFT (Enia: see SEATED).
#   NPC04 = Private Connelly (WC-NPCs_00000024)   (-20.198, 86.550, z -1.374, yaw -177.30)
#   NPC01 = Ceren Nightchant (WC-NPCs_00000027)   (-35.276, 88.797, z -0.499, yaw -45.69)
#   NPC03 = Olivia Dawnwillow (WC-NPCs_00000021)  (-41.857, 85.837, z 0.289, yaw -24.23)
#   NPC06 = Diego the Duelmaster (WC-NPCs_00000116) (26.543, 22.200, z -0.016, yaw 55.86), by the Arena exit
#   NPC08_Shop = Ugo Kalahad (WC-NPCs_00000598)   (-12.534, 22.119, z 0.303, yaw -55.16)
#   NPC05 = Private O'Ryan in house H1's own zone (WC_Unicorn_H1); the street's NPC05-B (-21.709, 94.374) is a second
#     template of him (its own persona) for after Saving Private O'Ryan, so he is not on the street (OFF_STREET).
#   NPC12 = Aegon Statz (the Gravulum event NPC, round 16 F1: not a starter NPC), not placed.
SPAWN_LIFT = 0.2
# Round 26 (N2): Diego the Duelmaster = D, Hunter of the Dead's look (CharaInit 23190) on White Mask Varre's rows (a
# standing idle a320_090100, head 1.54 m; D's own NpcParam 523190000 plays a335_090100, sitting, head 1.09 m), Ugo
# Kalahad = Merchant Kale (c3200, his vanilla m60_42_36_00 rows, idle a000_000000 standing, head 1.70 m, hat 1.79 m).
# Both non-quest: idle lines only (w101_npc_dialogue.py).
D_LOOK = (HUMAN_THINK, HUMAN_NPC, 23190)
KALE = (32000000, 32000010, -1)
CHARACTERS = [
    ("private_connelly", "Private Connelly", "street", (-20.198, -1.174, 86.550), -177.30, "c0000",
     *BERNAHL, 1042360820),
    ("ceren_nightchant", "Ceren Nightchant", "street", (-35.276, -0.299, 88.797), -45.69, "c0000",
     *CORHYN, 1042360821),
    # Round 27 (A7): Rya the Scout standing (rounds 24-26: Enia seated in her chair, SEATED).
    ("olivia_dawnwillow", "Olivia Dawnwillow", "street", (-41.857, 0.289 + SPAWN_LIFT, 85.837), -24.23, "c0000",
     *RYA, 1042360822),
    # Round 22 (P1): on the centre of the battle circle (FX_MinigameSigilGlow (597.5, 313.7, -70.0); the sunken floor's
    # bounding centre in the classic maze OBJ is (5.98, 3.09); W101's own NPC02 instance (6.010, 3.035), 0.1 m off),
    # facing the garden's exit door (6.074, -35.75): yaw atan2(-dx, -dy) = -0.15.
    ("lady_oriel", "Lady Oriel", "garden", (5.975, -0.500, 3.137), -0.15, "c5330",
     *TRINA, 1042360823),
    ("diego", "Diego the Duelmaster", "street", (26.543, 0.184, 22.200), 55.86, "c0000", *D_LOOK, 1042360825),
    ("ugo_kalahad", "Ugo Kalahad", "street", (-12.534, 0.503, 22.119), -55.16, "c3200", *KALE, 1042360826),
]
# Round 26 (N1/N2): characters that keep their dialogue keys (request flag numbers) but are not placed on the street;
# msb_add_npcs.py deletes their part (by entity id). (slug, title, entity)
OFF_STREET = [("private_oryan", "Private O'Ryan", 1042360824)]
# Round 26 (N2): characters added after round 23's IDLE_DONE keys; their keys go after those so that no older
# request flag changes number.
LATE_CHARACTERS = {"diego", "ugo_kalahad"}
# Round 26 (N3, Ryan: "Enia floats above her chair; the chair floats and isn't level"): characters that sit in a vanilla
# seat (msb_npc_seats.py) with gravity off (2004[10], emevd_quests idle event), so the engine doesn't stand them on
# the seat's collision: their MSB root is placed exactly where vanilla has it relative to the seat.
SEATED = set()  # round 27 (A7): nobody sits any more (rounds 26: {"olivia_dawnwillow"})
# Round 26 (N7, Ryan: street NPCs must never die; flinching is fine): 2004[12] immortality (HP stays >= 1, hits still
# land) on every character, in its idle event; talk events END while the character is dead.
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
#   c2170 Enia, c5330 Trina (round 24): 0, their own default idle, looped, as Melina.
#   c5330 Trina (round 25 M4, Ryan: "a purple glob on the ground; use her risen state"): c5330 has no a000_000000;
#     her lying idle a000_030001 keeps her head 0.06 m up, inside the flower mound, while a000_030000 (6 s, first and
#     last frame alike, so it loops) has her risen out of the flowers, head bone 1.22 m (as her bind pose). Looped.
IDLE = {"c0000": (90100, 0), "c2180": (0, 1), "c2170": (0, 1), "c5330": (30000, 1), "c3200": (0, 1)}
# Round 27 (A5/A7, Ryan: "every street NPC standing where the model can"): per-character idles over IDLE. Bernahl's
# 90100 (a345_090100) sits and Corhyn's (a361_090100) kneels; their 90101 (a345_090101 6 s, a361_090101 8 s: the
# Roundtable Hold's standing Corhyn) stand from first to last frame (head bone 1.54 m, pelvis 0.96, feet 0.10-0.11 at
# the first, middle and last spline control points; c0000_a9x.anibnd), looped. Diego (Varre's a320_090100), Ugo (Kale
# a000_000000), Olivia (Rya a329_090100) already stand; Trina's risen a000_030000 is her only upright pose (head 1.22).
IDLE_BY_SLUG = {"private_connelly": (90101, 1), "ceren_nightchant": (90101, 1)}
# Round 21 (Q7): characters whose AI is switched off so they stay on their MSB spot (Lady Oriel ran around the maze).
STAND_STILL = {"lady_oriel"}
CHARACTER_PART_ID = 9100  # part header 0x08 / name suffix of the first character (<model>_9100..)
TALK_NPC_BUTTON = 6000  # ActionButtonParam "Talk" (category 4, radius 2.5 m), as vanilla m60_51_36 1051360734 on a character
# Round 25 (M5, Ryan: talk to NPCs while on Torrent): 6000 is greyed out on Torrent (isGrayoutForRide 1); every
# category-4 Talk row is. Vanilla row 6001 is the only "Talk" (text 7000) usable while riding (category 11,
# isGrayoutForRide 0, radius 1.2 m, height -0.25..+0.75 m, front 90 deg). The NPC events take either button.
TALK_NPC_BUTTON_RIDING = 6001
# Round 26 (N4): the talk and cage events wait for ONE of the two buttons, picked by 4[32] IF Character Riding Mount,
# and set this transient flag (1 = on Torrent) so the HUD prompt measures the same ActionButtonParam row.
RIDING_FLAG = 1042362570
# Round 17: the HUD's answer to a dialogue request (DIALOGUE_FLAG + k): OK = the last page was read / the quest was
# accepted; CANCEL = closed early / the quest was declined (rounds 15-16: ER's 2007[10] OK / back buttons).
DIALOG_OK, DIALOG_CANCEL = 1042362620, 1042362621
# Round 19 (E1): a versioned one-shot reset of the whole quest state. Ryan's save kept quest flags from older rounds
# (round 17's intro assigned Unicorn Way unasked), so the round 18 flow (Ambrose's offer first) never started. The
# reset event clears every quest flag (QUEST_FLAG_RANGES) once and sets QUEST_RESET; the quest events wait for it.
# A later round that needs another reset takes the next flag (1042362691 + ...), never reuses this one.
QUEST_RESET_R19 = 1042362690
# Round 20 (Q2): version 2 of the reset. Ryan's round 19 save has Unicorn Way done and Ghost Hunters assigned through
# Ambrose's offer; round 20 starts the line at Private Connelly's "!" instead, so every quest flag is cleared once
# more (QUEST_RESET_R19 too) and QUEST_RESET (the flag the events and the DLL wait for) is this one.
QUEST_RESET_R20 = 1042362691
# Round 21 (Q1): version 3, in the saved block (QUEST_RESET_R20 sat in the cleared block, so it ran on every load).
QUEST_RESET = 1042360691
# Round 19 (E1): on when Ambrose's intro offer has been shown once. The first offer comes anywhere on the street
# (INTRO_ANCHOR within INTRO_RADIUS m: Ceren stands in the middle of the street, near where a load puts the player);
# after a "no" it is offered again on reaching Private Connelly (street_start), as in round 18.
INTRO_SEEN = 1042360689  # round 21: saved block (was 1042362689)
INTRO_ANCHOR, INTRO_RADIUS = 1042360821, 200.0
# Every flag the quests use: quests (n = 0..15), dialogue requests, counters and cages, the old talk flags, the
# dialog answers, INTRO_SEEN. (first, last) inclusive.
QUEST_FLAG_RANGES = [(QUEST_BASE, QUEST_BASE + 159), (1042362400, 1042362499), (COUNTER_BASE, CAGE_FLAG + 3),
                     (1042362600, 1042362621), (INTRO_SEEN, INTRO_SEEN)]
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
    "diego": "GUI/NpcPortraits/Art_Portrait_Diego_Duelmaster.png",  # WC-ST01-NPC06 (round 26)
    "ugo_kalahad": "GUI/NpcPortraits/Human_WCShopKeep_D_NPC.png",  # WC-ST01-NPC08_Shop (round 26)
    "merle_ambrose": "GUI/NpcPortraits/Art_Portrait_Ambrose.png",  # ObjectData/WC/WC-Ambrose-PreAV.xml
    "gamma": "GUI/NpcPortraits/Portrait_Owl_Gamma_A_NPC.png",
}
PORTRAITS["intro"] = PORTRAITS["merle_ambrose"]
# Names of the speakers who are not street characters (round 18: a page can have its own speaker, e.g. Gamma's tips
# or Olivia cutting into Ceren's Skeleton Crew talk, as the client's lines have it).
SPEAKERS = {"intro": "Merle Ambrose", "merle_ambrose": "Merle Ambrose", "gamma": "Gamma",
            "private_oryan": "Private O'Ryan"}  # round 26: off the street, keys kept


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


# Round 23 (Q1, Ryan): Ceren's idle line was her Skeleton Crew pitch ("Skeletal Pirates? ... NO THANK YOU"), so after
# finishing Skeleton Crew her talk sounded like the quest being offered again without a "!". Each character's idle now
# follows the line: once one of these quests is complete (the last listed first) it says that quest's "done" line
# (w101_npc_dialogue.py: client lines from after the quest) instead of "<npc>|idle", which is only for before.
IDLE_DONE = {
    "ceren_nightchant": ["skeleton_crew", "sinister_skeleton"],
    "lady_oriel": ["finding_out_why"],
}  # round 23 (Q3): Olivia's and O'Ryan's quests are gone, so they only have "<npc>|idle"


def dialogue_keys():
    """{dialogue key: request flag}, in a fixed order that doesn't depend on the dialogue text: the intros of quests
    handed out on arrival (the giver's offer, then what follows it: Gamma's pointer), then per character its talk
    goals, the quests it offers, the quests it is part of (underway) and its idle line."""
    keys = []
    for q in QUESTS:
        # Round 20: Unicorn Way's intro keys stay (unused) so every request flag keeps its number in dialogue.tsv.
        if q["assign"] and q["assign"][0] in ("intro", "greet"):
            keys +=[f"intro|{q['slug']}", f"intro|{q['slug']}|after"]
    def character_keys(slug):
        keys = []
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
        return keys
    early = [c[0] for c in CHARACTERS if c[0] not in LATE_CHARACTERS]
    early.insert(4, OFF_STREET[0][0])  # round 26: O'Ryan's keys keep their place (5th character until round 25)
    for slug in early:
        keys += character_keys(slug)
    # Round 23 (Q1): what a character says when it has nothing for the player once one of its quests is complete
    # (IDLE_DONE), appended so every older request flag keeps its number.
    keys += [f"{slug}|{quest}|done" for slug, quests in IDLE_DONE.items() for quest in quests]
    for slug in (c[0] for c in CHARACTERS if c[0] in LATE_CHARACTERS):  # round 26 (N2)
        keys += character_keys(slug)
    assert len(keys) <= 100 and len(set(keys)) == len(keys)
    return {key: DIALOGUE_FLAG + k for k, key in enumerate(keys)}

# The four quest Bone Cages (W101 WC-FairyCage03, 04, 05, 02: street FLVER = W101 (x, z, y) * 0.01, yaw = the object's
# z rotation in degrees), entity ids. Round 22 (P2): Wizard101's own cage (tools/w101_bone_cage.py: a street lamp with
# the bone cage hanging off its arm) built by blender_build_cage.py as CAGE_MODEL (closed) and CAGE_OPEN_MODEL (door
# open), both placed at every spot; the cage events show the closed one until the cage is opened during Finding Out
# Why, then the open one. Rounds 13-21 used vanilla AEG270_120 (OLD_CAGE_MODEL), hidden unless the quest was active,
# and had the 4th cage at (-22.8, 82.6) by Olivia instead of WC-FairyCage02.
OLD_CAGE_MODEL = "AEG270_120"  # "Round metal prison cage"
CAGE_MODEL, CAGE_OPEN_MODEL = "AEG030_558", "AEG030_559"
CAGES = [
    ((-237.463, -0.999, 141.741), 1042361880),  # WC-FairyCage03
    ((-219.187, -0.999, 142.762), 1042361881),  # WC-FairyCage04
    ((-259.552, 0.0, 187.903), 1042361882),  # WC-FairyCage05
    ((-259.663, 0.0, 202.845), 1042361883),  # WC-FairyCage02
]
CAGE_YAWS = [-80.21, 68.58, -121.18, -62.40]  # W101 z rotation -1.400, 1.197, -2.115, -1.089 rad
CAGE_OPEN_ENTITY = 4  # the open cage of cage c has entity CAGES[c] + 4 (1042361884-887)
# Round 24 (M2): the cage's Examine button. ActionButtonParam 9000 (TALK_BUTTON) reaches only 1 m from the lamp's
# base, and the post (0.36 m) plus the cage hanging off it leave a thin ring the player can stand in: Ryan stood
# ~0.7 m from cage 0 (log 02:50:55, MSB rounded to whole metres) and got nothing. Vanilla row 9260 is the same
# button (category 5 "Examine", text 1000, height -0.5..+1.0 m, any angle) with radius 1.8 m.
CAGE_BUTTON = 9260
# Round 25 (M5, Ryan: open Bone Cages while on Torrent): 9260 (and 9000) are greyed out on Torrent
# (isGrayoutForRide 1). Vanilla row 9300 is the same Examine (category 5, text 1000) usable while riding
# (isGrayoutForRide 0), radius 1.5 m, height -0.25..+1.25 m. The cage events take either button.
CAGE_BUTTON_RIDING = 9300
CAGE_OPEN_SFX = 1060  # oneshot SFX on the open cage (vanilla common 9005810 plays it as it enables an asset)

# Goal regions (spheres): (slug, frame, position, radius, entity id, region id). Frames as msb_add_interiors.py.
REGIONS = [
    ("street_start", "street", (26.543, 1.0, 22.2), 12.0, 1042362830, 9220),  # around Connelly at the entrance
    ("hedge_maze", "garden", (5.975, 0.3, 3.137), 14.0, 1042362831, 9221),  # around Lady Oriel (round 22: circle centre)
    ("T1", "AEG030_519_9001", (-0.085, 1.0, -8.86), 4.0, 1042362832, 9222),  # Blackhope room arrival
    ("H4", "AEG030_557_9000", (0.202, 1.0, 8.405), 4.0, 1042362833, 9223),  # Thief's House arrival
    # Round 26 (N1): on Ceren's real spot (NPC01, w101_npc_spots.py); rounds 19-25 had Olivia's (-41.857, 85.837).
    ("near_ceren", "street", (-35.276, 1.0, 88.797), 15.0, 1042362834, 9224),
    ("near_oryan", "street", (-125.714, 1.0, 88.655), 6.0, 1042362835, 9225),
]
REGION_ENTITY = {r[0]: r[4] for r in REGIONS}


def quest_assets():
    """[(part name, frame, position, facing, model, entity)] of the giver and cage assets msb_add_quests.py places.
    footprint_clear.py keeps these part names (it would otherwise sink them 1000 m with the vanilla clutter)."""
    out = [(f"{model}_{9000 + k}", frame, local, facing, model, entity)
           for k, (_slug, frame, local, facing, model, entity) in enumerate(NPCS)]
    # facing f turns an asset to (frame rotation + f); the model's FLVER is Wizard101's model frame (x, z_up, y), so
    # f = the W101 yaw turns it as Wizard101 does (the same r = yaw + a as msb_add_npcs.py's characters)
    for model, extra in ((CAGE_MODEL, 0), (CAGE_OPEN_MODEL, CAGE_OPEN_ENTITY)):
        out += [(f"{model}_{9100 + c}", "street", local, yaw, model, entity + extra)
                for c, ((local, entity), yaw) in enumerate(zip(CAGES, CAGE_YAWS, strict=True))]
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
    # Round 20 (Q2, Ryan): no Merle Ambrose offer; the line starts with a "!" over Private Connelly, and talking to
    # him takes Unicorn Way ("greet": assigned and its talk goal done at once; in Wizard101 the student arrives with
    # it) and plays its goal conversation, which ends in the Ghost Hunters card (follow_up).
    dict(n=0, slug="unicorn_way", w101="1625BF", giver="merle_ambrose", assign=("greet", "private_connelly", None),
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
]
# Round 23 (Q3, Ryan: "I only care about the quest line to get to and defeat Rattlebones, and then go talk to the guy
# after. You can remove all the other quest lines"): QUESTS is only Wizard101's main chain Unicorn Way > Ghost Hunters >
# Skeleton Crew > Monsters and Mazes > Finding Out Why > Who Could It Be Now? > Sinister Skeleton (1625CF: defeat
# Rattlebones in tower T2, then "You did it, didn't you?!? You beat Rattlebones!" with Ceren Nightchant, which ends
# the line: Grim Tales is not offered). The side quests below are no longer offered, tracked, marked or talked about;
# their saved flags (REMOVED_FLAG_RANGES) are cleared on every map load (emevd_quests.reset_event). Kept for reference.
REMOVED_QUESTS = [
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
REMOVED_FLAG_RANGES = [(1042360400, 1042360459), (1042360512, 1042360515)]  # quests n=10..15, Pesky Pirates' counter
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
    # Round 22 (H7): the Hedge Maze (goto): Wizard101's green place disc of a hedge garden (w101_quest_arrow.py).
    "hedge_maze": "GUI/QuestButtons/Art_Quest_AV_WalledGarden.png",
}
GOAL_PICTURES.update({slug: PORTRAITS[slug] for slug in PORTRAITS if slug != "intro"})
# Round 20 (Q4, Ryan: "the picture of the enemy should correspond to the actual Elden Ring enemy"): the street's
# Elden Ring stand-ins in Wizard101's portrait ring. Mobs: Elden Ring's own picture of that enemy, its spirit-ash icon
# (Spirit Jellyfish Ashes 236000 icon 111, Skeletal Militiaman Ashes 212000 icon 102, Winged Misbegotten Ashes 210000
# icon 120); NPCs: their round 18 ER face portraits. Art made on the PC, never in the repo; the DLL also swaps the
# old paths of an older quests.tsv (quests.rs ER_PICTURES). Rattlebones (Giant Skeleton Torso: Elden Ring has no
# picture of it), Lady Blackhope and Fear of Fire (not placed in Elden Ring) keep Wizard101's.
ER_PICTURES = "GUI/NpcPortraits/ER"
GOAL_PICTURES.update({slug: f"{ER_PICTURES}/{slug}.png" for slug in ("lost_soul", "skeletal_pirate", "dark_fairy")})
GOAL_PICTURES.update({c[0]: f"{ER_PICTURES}/{c[0]}.png" for c in CHARACTERS})

# Round 20 (Q3): each quest's reward in runes, shown on its card and given once on completion by the HUD DLL
# (quests.rs REWARDS, keep in step). Wizard101's quest rewards are server-side, not in the client's files, and its
# wiki could not be reached, so these are Wizard City-sized gold amounts given as the same number of runes.
REWARDS = {"unicorn_way": 25, "ghost_hunters": 35, "skeleton_crew": 45, "monsters_and_mazes": 55,
           "finding_out_why": 75, "who_could_it_be_now": 60, "sinister_skeleton": 150}  # round 23 (Q3): side quests removed
REWARD_OFFSET = 7  # quest base + 7: its reward was given (set by the DLL; the reset clears it with the rest)


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
