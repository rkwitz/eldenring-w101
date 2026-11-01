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

Usage: python -I w101_npc_dialogue.py uw_lang.json npc_dialogue.json
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
CONVERSATIONS = {
    # Merle Ambrose in his office (here: on reaching Unicorn Way), then Gamma.
    "intro|unicorn_way": lines(UW, 0, 1, 2, speaker="merle_ambrose"),
    "intro|unicorn_way|after": lines(UW, 4, speaker="merle_ambrose") + lines(UW, 5, **GAMMA),
    # Private Connelly
    "private_connelly|unicorn_way|goal1": lines(UW, 7, 10, 15, 13) + lines(GH, 0, 1),
    "private_connelly|ghost_hunters|offer": lines(GH, 0, 1),
    "private_connelly|ghost_hunters|accepted": lines(GH, 2, 12, **GAMMA),
    "private_connelly|ghost_hunters|underway": lines(GH, 14),
    "private_connelly|ghost_hunters|goal2": lines(GH, 5, 15, 6),
    "private_connelly|saving_private_oryan|offer": lines(SPO, 1, 14),
    "private_connelly|saving_private_oryan|underway": lines(SPO, 0),
    "private_connelly|saving_private_oryan|goal6": lines(SPO, 2, 3),
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
    "ceren_nightchant|idle": lines(SC, 15, 16),
    # Olivia Dawnwillow (her Then-Faire stands in for The Power is Ours)
    "olivia_dawnwillow|pesky_pirates|offer": lines(PP, 4, 6),
    "olivia_dawnwillow|pesky_pirates|accepted": lines(PP, 1, 3, **GAMMA),
    "olivia_dawnwillow|pesky_pirates|underway": lines(PP, 0),
    "olivia_dawnwillow|pesky_pirates|goal2": lines(PP, 9, 14) + lines(FTW, 1, 2),
    "olivia_dawnwillow|fear_of_fire|offer": lines(FTW, 1, 2),
    "olivia_dawnwillow|fear_of_fire|goal3": lines(FOF, 31, 32, 33),
    "olivia_dawnwillow|idle": lines(PP, 0),
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
    # Private O'Ryan, hiding in house 1
    "private_oryan|saving_private_oryan|goal2": lines(SPO, 15, 16, 17, 18),
    "private_oryan|saving_private_oryan|underway": lines(SPO, 18),
    "private_oryan|saving_private_oryan|goal5": lines(SPO, 4, 5),
    "private_oryan|idle": lines(SPO, 17),
}


def tidy(text):
    """A .lang line as the HUD's text: inline pictures to the PNGs under book\\art, the player's name as 'Wizard'."""
    text = re.sub(r"<image;([^;>]+)\.dds[^>]*>", r"<image;\1.png> ", text, flags=re.I)
    text = text.replace("$PLAYER_FIRSTNAME$", "Wizard")
    return re.sub(r"\s+", " ", text).strip()


def main(argv):
    lang = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
    keys = Q.dialogue_keys()
    unknown = sorted(set(CONVERSATIONS) - set(keys))
    assert not unknown, f"not dialogue keys: {unknown}"
    tables = {}
    pages = {}
    for key, refs in CONVERSATIONS.items():
        own = key.split("|")[0]
        out = []
        for speaker, table, n in refs:
            rows = tables.setdefault(table, {int(k): v for k, _c, v in lang[f"WizQst{table}"]})
            out.append([speaker or (Q.BY_SLUG[key.split("|")[1]]["giver"] if own == "intro" else own), tidy(rows[n])])
        pages[key] = out
    data = {"version": 2, "personal_use_only": True, "pages": pages}
    Path(argv[2]).write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding="utf-8")
    print(f"{argv[2]}: {len(pages)} conversations, {sum(map(len, pages.values()))} pages "
          f"({len(keys) - len(pages)} keys without lines: {', '.join(k for k in keys if k not in pages)})")


if __name__ == "__main__":
    main(sys.argv)
