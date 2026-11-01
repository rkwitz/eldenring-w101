"""The conversations the HUD DLL shows in Wizard101's dialogue box (round 17, F2/F3): w101_quests.dialogue_keys()
plus the lines of the personal dialogue file to w101_orbs\\book\\dialogue.tsv on Ryan's PC.

The map's talk events (emevd_quests.py) set a key's request flag; the DLL (hud_orbs/src/talk.rs) shows that key's
pages and answers DIALOG_OK / DIALOG_CANCEL. The lines are Wizard101 client text: npc_dialogue.json and the .tsv
written from it are for Ryan's own use and are never committed. Without the dialogue file the .tsv still has every
request (R lines), and the DLL falls back to the quest's own text for offers.

Usage: python -I w101_book_dialogue.py [npc_dialogue.json] dialogue.tsv

Lines (tab-separated):
  R  request flag  key  speaker slug  speaker name  portrait (art path)  quest slug (or -)  kind (intro, goal<n>,
     offer, underway, idle)
  P  request flag  text                one page, in order (the speaker line of the file's pages is dropped)
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
        lines.append(["R", flag, key, speaker, names.get(speaker, speaker), Q.PORTRAITS.get(speaker, ""), quest, kind])
        for _message, text in pages.get(key, []):
            lines.append(["P", flag, clean(text)])
    Path(argv[-1]).write_text("".join("\t".join(map(str, l)) + "\n" for l in lines), encoding="utf-8")
    print(f"{argv[-1]}: {sum(l[0] == 'R' for l in lines)} conversations, {sum(l[0] == 'P' for l in lines)} pages")


if __name__ == "__main__":
    main(sys.argv)
