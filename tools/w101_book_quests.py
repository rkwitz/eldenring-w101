"""The Unicorn Way quest list for the hud_orbs spellbook's Quests page and NPC dialogue: unicorn_way_quests.json (the
quest line with its ER event flags, built from Ryan's W101 client text; personal use, never in the repo) to
w101_orbs\\book\\quests.tsv on the PC.

Usage: python w101_book_quests.py unicorn_way_quests.json quests.tsv

Lines (tab-separated; text has tabs and line breaks replaced by spaces, markup tags removed):
  Q  slug  title  kind (main/side)  assigned flag  ready flag  complete flag  giver name  turn-in name
  G  slug  goal n  flag  after (goal numbers, comma-separated)  count  counter (0 = none; a flag number = a
     4-bit event value on that flag and the next three, most significant first; list:<flags> = how many are on)  text  place
  N  npc slug  talk flag  name
  X  slug  key  text            (the quest's dialogue lines in client order)
  D  slug  event  text          (optional: what an NPC says on a talk press, by event: offer, goal<n>, turnin,
                                 underway; the HUD falls back to the quest state without them)
Only quests with flags (built in the map events) are written. Quest n: flags 1042362000 + 10n (+0 assigned, +1..+6
goals, +8 ready, +9 complete); talk flags 1042362600.. are set on every talk press (see the quests README).
"""

import json
import re
import sys

TALK_FLAGS = {"private_connelly": 1042362600, "ceren_nightchant": 1042362601, "olivia_dawnwillow": 1042362602,
              "lady_oriel": 1042362603, "private_oryan": 1042362604}
CAGE_FLAGS = [1042362540, 1042362541, 1042362542, 1042362543]
PLACES = {"street": "Unicorn Way", "hedge_maze": "The Hedge Maze", "T1": "Blackhope Tower", "T2": "The Archives",
          "H1": "Unicorn Way", "H2": "Unicorn Way", "H3": "Unicorn Way", "H4": "The Thief's House"}


def clean(text):
    text = re.sub(r"<[^>]*>", "", text or "")
    text = text.replace("$PLAYER_FIRSTNAME$", "Tarnished").replace("$PLAYER_NAME$", "Tarnished")
    return re.sub(r"\s+", " ", text).strip()


def main():
    source, target = sys.argv[1], sys.argv[2]
    data = json.load(open(source, encoding="utf-8"))
    npcs = data.get("npcs", {})
    name = lambda slug: npcs.get(slug, {}).get("name", slug.replace("_", " ").title())  # noqa: E731
    lines = []
    for q in data["quests"]:
        flags = q.get("flags")
        if not flags:
            continue
        slug = q["id"]
        lines.append(["Q", slug, clean(q["title"]), q.get("kind", "main"), flags["assigned"], flags["ready"],
                      flags["complete"], name(q.get("giver", "")), name(q.get("turn_in", ""))])
        for n, g in enumerate(q["goals"], 1):
            counter = (g.get("counter") or {}).get("base_flag", 0)
            if g.get("target") == "bone_cage":
                counter = "list:" + ",".join(str(f) for f in CAGE_FLAGS)
            lines.append(["G", slug, n, g.get("flag") or 0, ",".join(str(a) for a in g.get("after") or []),
                          g.get("count", 1), counter, clean(g["display"]), PLACES.get(g.get("location"), "Unicorn Way")])
        for key, text in sorted(q.get("text", {}).items()):
            text = clean(text)
            if text:
                lines.append(["X", slug, key, text])
        # Optional "dialogue": {event: text keys or lines} (offer, goal<n>, turnin, underway).
        for event, entries in (q.get("dialogue") or {}).items():
            for entry in [entries] if isinstance(entries, str) else entries:
                text = clean(q.get("text", {}).get(entry, entry))
                if text:
                    lines.append(["D", slug, event, text])
    for slug, flag in TALK_FLAGS.items():
        lines.append(["N", slug, flag, name(slug)])
    with open(target, "w", encoding="utf-8", newline="\n") as f:
        for line in lines:
            f.write("\t".join(str(v).replace("\t", " ") for v in line) + "\n")
    print(f"{target}: {sum(l[0] == 'Q' for l in lines)} quests, {len(lines)} lines")


if __name__ == "__main__":
    main()
