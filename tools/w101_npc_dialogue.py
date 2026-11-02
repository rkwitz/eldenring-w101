"""The street NPCs' conversations (round 18, F1): which Wizard101 quest lines each dialogue key
(w101_quests.dialogue_keys) shows, page by page and with who says each page, as Wizard101 plays them.

This file holds only references (the quest's WizQst table and the line's number), never the text. The text comes from
Ryan's personal dump of the client's .lang files (reference\\w101\\extracted\\quests\\uw_lang.json, tools/w101_lang.py)
into npc_dialogue.json, which stays on his PC / in the staging folder with dialogue.tsv (w101_book_dialogue.py).

Each page is "<table>:<line>" (said by the key's own character) or "<speaker>=<table>:<line>" (another one: gamma
for Gamma's tips, olivia_dawnwillow when she cuts in on Ceren, lady_oriel, merle_ambrose). A key whose last page ends
in the quest card (w101_quests.dialogue_card) gets yes / no on that page.

Left out: Gamma's tips about things Elden Ring has no counterpart for (the sidewalk's safety, Health / Mana wisps,
mounts, the backpack and equipping), the Grim Tales errand to the Library, and Release the Fairies (1625CB), whose
Bone Cages are part of Finding Out Why here.

Round 23b: Ryan played the line in Classic Mode, so the client only has the classic voice files. CONVERSATIONS now
holds the CLASSIC quest text (the classic tables in Root.wad, which classic mode shows and the classic voices read);
the round 18-23 text from the updated tables (WizQst1625xx) is UPDATED_CONVERSATIONS (--updated). The classic quests
are split differently (Fairies Wear Boots, They're Not Bad..., Finding Out Why + Freedom!, Sinister Skeleton, Stop the
Maker) but the mod keeps its quest structure (w101_quests.QUESTS), so each conversation takes the closest classic lines.
Classic Mode shows nothing after "yes", so the "accepted" keys have no lines (except Oriel's Finding Out Why, which
has one), and the intro keys (unused since round 20) none. VOICE maps the classic voice files (w101_orbs\voice) to
their lines; the optional third argument writes book\voice.tsv (key, 0-based page, file) for every voiced page.

Usage: python -I w101_npc_dialogue.py [--updated] wizqst.json npc_dialogue.json [voice.tsv]
  wizqst.json: {table: [[key, comment, value], ...]} with the tables used (w101_lang.py dump <Root.wad> out 'WizQst')
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import w101_quests as Q  # noqa: E402

UW, GH, SC, MM = "1625BF", "1625C2", "1625C1", "1625C5"
FOW, WC, SS, GT = "1625CA", "1625CE", "1625CF", "1625CC"
PP, SPO, FOF, FTW = "1625C9", "1625DA", "173198", "173197"


def lines(table, *numbers, speaker=None):
    return [(speaker, table, n) for n in numbers]


GAMMA = {"speaker": "gamma"}
UPDATED_CONVERSATIONS = {
    # Merle Ambrose in his office (here: on reaching Unicorn Way), then Gamma.
    "intro|unicorn_way": lines(UW, 0, 1, 2, speaker="merle_ambrose"),
    "intro|unicorn_way|after": lines(UW, 4, speaker="merle_ambrose") + lines(UW, 5, **GAMMA),
    # Private Connelly
    "private_connelly|unicorn_way|goal1": lines(UW, 7, 10, 15, 13) + lines(GH, 0, 1),
    "private_connelly|ghost_hunters|offer": lines(GH, 0, 1),
    "private_connelly|ghost_hunters|accepted": lines(GH, 2, 12, **GAMMA),
    "private_connelly|ghost_hunters|underway": lines(GH, 14),
    "private_connelly|ghost_hunters|goal2": lines(GH, 5, 15, 6),
    "private_connelly|idle": lines(UW, 13),
    # Ceren Nightchant (Olivia overhears him in Skeleton Crew)
    "ceren_nightchant|ghost_hunters|goal3": (lines(GH, 7) + lines(SC, 0, 15, 16)
                                             + lines(SC, 1, speaker="olivia_dawnwillow") + lines(SC, 2)),
    "ceren_nightchant|skeleton_crew|offer": lines(SC, 15, 16, 2),
    "ceren_nightchant|skeleton_crew|accepted": lines(SC, 3) + lines(SC, 4, **GAMMA),
    "ceren_nightchant|skeleton_crew|underway": lines(SC, 5),
    "ceren_nightchant|skeleton_crew|goal2": lines(SC, 8, 9, 12, 13) + lines(MM, 1, 2, 4),
    "ceren_nightchant|monsters_and_mazes|offer": lines(MM, 1, 2, 4),
    "ceren_nightchant|monsters_and_mazes|underway": lines(MM, 0),
    "ceren_nightchant|who_could_it_be_now|goal1": lines(WC, 6) + lines(SS, 1, 2),
    "ceren_nightchant|sinister_skeleton|offer": lines(SS, 1, 2),
    "ceren_nightchant|sinister_skeleton|accepted": lines(SS, 10),
    "ceren_nightchant|sinister_skeleton|underway": lines(SS, 0),
    "ceren_nightchant|sinister_skeleton|goal4": lines(SS, 7) + lines(GT, 1, 2, 3),
    # Round 23 (Q1): her idle was SC 15-16, the first pages of the Skeleton Crew offer, so after Skeleton Crew it read
    # as that offer again; before the quest she greets the player (GH 7), after it her lines from after each quest.
    "ceren_nightchant|idle": lines(GH, 7),
    "ceren_nightchant|skeleton_crew|done": lines(SC, 12, 13),
    "ceren_nightchant|sinister_skeleton|done": lines(GT, 1, 2),
    # Olivia Dawnwillow: round 23 (Q3) no quests (Pesky Pirates / The Power is Ours removed); her idle is her remark on
    # the Skeletal Pirates (PP 4) without the request (PP 0 "Please help drive these Skeletal Pirates out" read as a
    # quest offer).
    "olivia_dawnwillow|idle": lines(PP, 4),
    # Lady Oriel in the Hedge Maze
    "lady_oriel|monsters_and_mazes|goal2": lines(MM, 5, 7) + lines(FOW, 1) + lines(MM, 3) + lines(FOW, 3, 4),
    "lady_oriel|finding_out_why|offer": lines(FOW, 1, 3, 4),
    "lady_oriel|finding_out_why|accepted": lines(FOW, 9),
    "lady_oriel|finding_out_why|underway": lines(FOW, 0),
    "lady_oriel|finding_out_why|goal4": lines(FOW, 10, 11) + lines(WC, 1, 12, 8),
    "lady_oriel|who_could_it_be_now|offer": lines(WC, 1, 12, 8),
    "lady_oriel|who_could_it_be_now|accepted": lines(WC, 10),
    "lady_oriel|who_could_it_be_now|underway": lines(WC, 0),
    "lady_oriel|idle": lines(MM, 5),
    "lady_oriel|finding_out_why|done": lines(FOW, 10),
    # Private O'Ryan, hiding in house 1: round 23 (Q3) no quest (Saving Private O'Ryan removed), idle line only.
    "private_oryan|idle": lines(SPO, 17),
}


# Classic quest tables (QuestTitle): Ghost Hunters, Fairies Wear Boots (Ceren: find the Fairies), They're Not Bad...
# (Ceren -> Oriel), Finding Out Why (Oriel: Fairy Dust), Freedom! (Oriel: Bone Cages), Sinister Skeleton (Oriel ->
# Ceren), Stop the Maker (Ceren: Rattlebones), Rattlebones Report (Ceren -> Ambrose), Pesky Pirates, Saving Private
# O'Ryan.
C_GH, C_FWB, C_TNB, C_FOW, C_FREE = "94F2", "94EC", "94ED", "9AE8", "9AEA"
C_SS, C_STM, C_RR, C_PP, C_SPO = "9AEB", "298E2", "94F6", "94E5", "94F3"
CONVERSATIONS = {
    # Private Connelly: classic Ghost Hunters (his greeting is its offer).
    "private_connelly|unicorn_way|goal1": lines(C_GH, 18, 19),
    "private_connelly|ghost_hunters|offer": lines(C_GH, 18, 19),
    "private_connelly|ghost_hunters|underway": lines(C_GH, 17),
    "private_connelly|ghost_hunters|goal2": lines(C_GH, 20, 31),
    "private_connelly|idle": lines(C_GH, 32),
    # Ceren Nightchant: Skeleton Crew takes Fairies Wear Boots' lines (classic Ceren's first quest), Monsters and Mazes
    # They're Not Bad..., Who Could It Be Now? the end of classic Sinister Skeleton, Sinister Skeleton Stop the Maker.
    "ceren_nightchant|ghost_hunters|goal3": lines(C_FWB, 32, 33),
    "ceren_nightchant|skeleton_crew|offer": lines(C_FWB, 32, 33),
    "ceren_nightchant|skeleton_crew|underway": lines(C_FWB, 31),
    "ceren_nightchant|skeleton_crew|goal2": lines(C_FWB, 35, 36) + lines(C_TNB, 16, 17, 18),
    "ceren_nightchant|monsters_and_mazes|offer": lines(C_TNB, 16, 17, 18),
    "ceren_nightchant|monsters_and_mazes|underway": lines(C_TNB, 15),
    "ceren_nightchant|who_could_it_be_now|goal1": lines(C_SS, 9, 10, 11) + lines(C_STM, 2, 3),
    "ceren_nightchant|sinister_skeleton|offer": lines(C_STM, 2, 3),
    "ceren_nightchant|sinister_skeleton|underway": lines(C_STM, 0),
    "ceren_nightchant|sinister_skeleton|goal4": lines(C_STM, 5) + lines(C_RR, 19),  # not RR 18/20: no Ambrose here
    "ceren_nightchant|idle": lines(C_STM, 1),
    "ceren_nightchant|skeleton_crew|done": lines(C_FWB, 36),
    "ceren_nightchant|sinister_skeleton|done": lines(C_RR, 19),
    # Olivia Dawnwillow: Pesky Pirates' thanks (her other lines ask for the removed quest).
    "olivia_dawnwillow|idle": lines(C_PP, 29),
    # Lady Oriel: They're Not Bad...'s end, then Finding Out Why + Freedom! as one quest (Fairy Dust, Bone Cages);
    # FREE 15 (a healing spell reward) left out: the reward here is runes.
    "lady_oriel|monsters_and_mazes|goal2": lines(C_TNB, 19) + lines(C_FOW, 11, 12) + lines(C_FREE, 9, 8),
    "lady_oriel|finding_out_why|offer": lines(C_FOW, 11, 12) + lines(C_FREE, 9, 8),
    "lady_oriel|finding_out_why|accepted": lines(C_FOW, 17),
    "lady_oriel|finding_out_why|underway": lines(C_FOW, 10) + lines(C_FREE, 13),
    "lady_oriel|finding_out_why|goal4": lines(C_FREE, 11) + lines(C_FOW, 13, 15, 16) + lines(C_SS, 7, 8),
    "lady_oriel|who_could_it_be_now|offer": lines(C_SS, 7, 8),
    "lady_oriel|who_could_it_be_now|underway": lines(C_SS, 6),
    "lady_oriel|idle": lines(C_FOW, 9),
    "lady_oriel|finding_out_why|done": lines(C_FREE, 11),
    # Private O'Ryan in house 1: Saving Private O'Ryan's greeting and his remark (no request).
    "private_oryan|idle": lines(C_SPO, 30, 31),
}

# Classic voice files (W101 *_QST_<n>, in w101_orbs\voice) -> (table, line). Matched in round 23 (S1, pocketsphinx +
# text match, round23/voice/classic_voice_map.tsv) and round 23b (Ceren 11, Oriel 09/11/18, O'Ryan 01; same method,
# the files are numbered in table order per speaker).
VOICE = {
    "PrivateConnelly_QST_05.mp3": (C_GH, 17), "PrivateConnelly_QST_06.mp3": (C_GH, 18),
    "PrivateConnelly_QST_07.mp3": (C_GH, 19), "PrivateConnelly_QST_08.mp3": (C_GH, 20),
    "PrivateConnelly_QST_18.mp3": (C_GH, 31),
    "PrivateConnelly_QST_09.mp3": (C_SPO, 26), "PrivateConnelly_QST_10.mp3": (C_SPO, 27),
    "PrivateConnelly_QST_11.mp3": (C_SPO, 28), "PrivateConnelly_QST_12.mp3": (C_SPO, 29),
    "PrivateConnelly_QST_16.mp3": (C_SPO, 47),
    "PrivateORyan_QST_01.mp3": (C_SPO, 30), "PrivateORyan_QST_02.mp3": (C_SPO, 31),
    "PrivateORyan_QST_03.mp3": (C_SPO, 32), "PrivateORyan_QST_04.mp3": (C_SPO, 33),
    "PrivateORyan_QST_05.mp3": (C_SPO, 48),
    "OliviaDawnwillow_QST_04.mp3": (C_PP, 29),
    "CerenNightchant_QST_01.mp3": (C_FWB, 31), "CerenNightchant_QST_02.mp3": (C_FWB, 32),
    "CerenNightchant_QST_03.mp3": (C_FWB, 33), "CerenNightchant_QST_04.mp3": (C_FWB, 35),
    "CerenNightchant_QST_05.mp3": (C_FWB, 36),
    "CerenNightchant_QST_06.mp3": (C_TNB, 15), "CerenNightchant_QST_07.mp3": (C_TNB, 16),
    "CerenNightchant_QST_08.mp3": (C_TNB, 17), "CerenNightchant_QST_09.mp3": (C_TNB, 18),
    "CerenNightchant_QST_10.mp3": (C_STM, 0), "CerenNightchant_QST_11.mp3": (C_STM, 1),
    "CerenNightchant_QST_12.mp3": (C_STM, 2), "CerenNightchant_QST_13.mp3": (C_STM, 3),
    "CerenNightchant_QST_14.mp3": (C_STM, 5),
    "CerenNightchant_QST_15.mp3": (C_RR, 18), "CerenNightchant_QST_16.mp3": (C_RR, 19),
    "CerenNightchant_QST_17.mp3": (C_RR, 20),
    "CerenNightchant_QST_18.mp3": (C_SS, 9), "CerenNightchant_QST_19.mp3": (C_SS, 10),
    "CerenNightchant_QST_20.mp3": (C_SS, 11),
    "LadyOriel_QST_06.mp3": (C_TNB, 19), "LadyOriel_QST_07.mp3": (C_FOW, 10), "LadyOriel_QST_09.mp3": (C_FOW, 11),
    "LadyOriel_QST_10.mp3": (C_FOW, 12), "LadyOriel_QST_11.mp3": (C_FOW, 17), "LadyOriel_QST_12.mp3": (C_FOW, 13),
    "LadyOriel_QST_13.mp3": (C_FOW, 15),
    "LadyOriel_QST_14.mp3": (C_FREE, 8), "LadyOriel_QST_15.mp3": (C_FREE, 9), "LadyOriel_QST_16.mp3": (C_FREE, 13),
    "LadyOriel_QST_17.mp3": (C_FREE, 11), "LadyOriel_QST_18.mp3": (C_FREE, 15),
    "LadyOriel_QST_19.mp3": (C_SS, 6), "LadyOriel_QST_20.mp3": (C_SS, 7), "LadyOriel_QST_21.mp3": (C_SS, 8),
    "MerleAmbrose_QST_90.mp3": (C_RR, 25), "MerleAmbrose_QST_91.mp3": (C_RR, 22),
}


def tidy(text):
    """A .lang line as the HUD's text: inline pictures to the PNGs under book\\art, the player's name as 'Wizard'."""
    text = re.sub(r"<image;([^;>]+)\.dds[^>]*>", r"<image;\1.png> ", text, flags=re.I)
    text = text.replace("$PLAYER_FIRSTNAME$", "Wizard")
    return re.sub(r"\s+", " ", text).strip()


def main(argv):
    conversations = CONVERSATIONS
    if argv[1] == "--updated":
        conversations, argv = UPDATED_CONVERSATIONS, argv[1:]
    lang = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
    keys = Q.dialogue_keys()
    unknown = sorted(set(conversations) - set(keys))
    assert not unknown, f"not dialogue keys: {unknown}"
    by_line = {line: f for f, line in VOICE.items()}
    tables = {}
    pages = {}
    voiced = []
    for key in keys:  # dialogue_keys() order, so voice.tsv follows dialogue.tsv
        if key not in conversations:
            continue
        own = key.split("|")[0]
        out = []
        for speaker, table, n in conversations[key]:
            rows = tables.setdefault(table, {int(k): v for k, _c, v in lang[f"WizQst{table}"]})
            if (table, n) in by_line and not speaker:
                voiced.append((key, len(out), by_line[table, n]))
            out.append([speaker or (Q.BY_SLUG[key.split("|")[1]]["giver"] if own == "intro" else own), tidy(rows[n])])
        pages[key] = out
    data = {"version": 2, "personal_use_only": True, "pages": pages}
    Path(argv[2]).write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding="utf-8")
    total = sum(map(len, pages.values()))
    print(f"{argv[2]}: {len(pages)} conversations, {total} pages "
          f"({len(keys) - len(pages)} keys without lines: {', '.join(k for k in keys if k not in pages)})")
    if len(argv) > 3:
        head = "# dialogue key<TAB>page (0-based)<TAB>file in w101_orbs/voice/ (tools/w101_npc_dialogue.py VOICE)\n"
        Path(argv[3]).write_text(head + "".join(f"{k}\t{p}\t{f}\n" for k, p, f in voiced), encoding="utf-8")
        print(f"{argv[3]}: {len(voiced)} of {total} pages voiced, {len({f for *_, f in voiced})} files")


if __name__ == "__main__":
    main(sys.argv)
