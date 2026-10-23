"""Item and spell names for the hud_orbs spellbook: Elden Ring's English FMGs as a tab-separated list.

Usage: python er_item_names.py path\\to\\unpacked\\msg\\engus\\item_dlc02.msgbnd output\\er_item_names.tsv

Input is the uncompressed item msgbnd (blender_unpack.py / UXM + Smithbox; item_dlc02 holds the base game and both DLC
name tables). Each output line is `<category>\\t<id>\\t<name>` with category W weapon, P protector (armour),
A accessory (talisman), G goods, E gem (Ash of War) or M magic (sorcery / incantation), matching the category bits of
the game's item IDs. Later tables (the _dlc ones) override earlier ones.
"""
import sys
from pathlib import Path

from er_text import bnd4, read_fmg

TABLES = {"WeaponName": "W", "ProtectorName": "P", "AccessoryName": "A", "GoodsName": "G", "GemName": "E",
          "MagicName": "M"}


def main():
    source, target = Path(sys.argv[1]), Path(sys.argv[2])
    files = bnd4(source.read_bytes())
    names = {}
    for path in sorted(files, key=lambda p: ("_dlc" in p, p)):
        stem = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".fmg")
        category = TABLES.get(stem.split("_dlc")[0])
        if category is None:
            continue
        for text_id, text in read_fmg(files[path]).items():
            text = (text or "").strip()
            if text and not text.startswith("[ERROR]"):
                names[(category, text_id)] = text.replace("\t", " ").replace("\n", " ")
    target.parent.mkdir(parents=True, exist_ok=True)
    with open(target, "w", encoding="utf-8", newline="\n") as out:
        for (category, text_id), text in sorted(names.items()):
            out.write(f"{category}\t{text_id}\t{text}\n")
    print(f"wrote {len(names)} names to {target}")


if __name__ == "__main__":
    main()
