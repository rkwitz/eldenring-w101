"""The conversations the HUD DLL shows in Wizard101's dialogue box (round 17, F2/F3): w101_quests.dialogue_keys()
plus the lines of the personal dialogue file to w101_orbs\\book\\dialogue.tsv on Ryan's PC.

The map's talk events (emevd_quests.py) set a key's request flag; the DLL (hud_orbs/src/talk.rs) shows that key's
pages and answers DIALOG_OK / DIALOG_CANCEL. The lines are Wizard101 client text: npc_dialogue.json and the .tsv
written from it are for Ryan's own use and are never committed. Without the dialogue file the .tsv still has every
request (R lines), and the DLL falls back to the quest's own text for offers.

Usage: python -I w101_book_dialogue.py [npc_dialogue.json] dialogue.tsv

Lines (tab-separated):
  R  request flag  key  speaker slug  speaker name  portrait (art path)  quest slug (or -)  kind (intro, goal<n>,
     offer, accepted, underway, idle)  card (round 18: the quest whose card and yes / no end the conversation, or -)
  P  request flag  text  speaker name  portrait   one page, in order; the name and portrait (round 18) are the page's
     own speaker's (Gamma's tips, Olivia cutting in), empty for the conversation's own speaker

npc_dialogue.json version 2 (w101_npc_dialogue.py) has pages [speaker slug, text]; version 1 (round 15) had
[message id, "<speaker>\\n<line>..."], whose speaker line is dropped.
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import w101_quests as Q  # noqa: E402


def clean(text):
    """One page of npc_dialogue.json ("<speaker>\\n<line>\\n<line>...", wrapped for ER's box) as running text."""
    body = text.split("\n", 1)[1] if "\n" in text else text
    return re.sub(r"\s+", " ", body.replace("\t", " ")).strip()


def main(argv):
    pages = {}
    if len(argv) > 2:
        pages = json.loads(Path(argv[1]).read_text(encoding="utf-8"))["pages"]
    names = {c[0]: c[1] for c in Q.CHARACTERS} | Q.SPEAKERS
    lines = []
    for key, flag in Q.dialogue_keys().items():
        parts = key.split("|")
        speaker = parts[0]
        quest, kind = (parts[1], parts[2]) if len(parts) == 3 else (parts[1], "intro") if speaker == "intro" else ("-", parts[1])
        own = Q.BY_SLUG[quest]["giver"] if speaker == "intro" else speaker
        lines.append(["R", flag, key, speaker, names.get(own, own), Q.PORTRAITS.get(own, ""), quest, kind,
                      Q.dialogue_card(key) or "-"])
        for who, text in pages.get(key, []):
            if isinstance(who, str):  # version 2: [speaker slug, text]
                other = who != own
                lines.append(["P", flag, text, names.get(who, who) if other else "",
                              Q.PORTRAITS.get(who, "") if other else ""])
            else:
                lines.append(["P", flag, clean(text), "", ""])
    Path(argv[-1]).write_text("".join("\t".join(map(str, l)) + "\n" for l in lines), encoding="utf-8")
    print(f"{argv[-1]}: {sum(l[0] == 'R' for l in lines)} conversations, {sum(l[0] == 'P' for l in lines)} pages")


if __name__ == "__main__":
    main(sys.argv)
