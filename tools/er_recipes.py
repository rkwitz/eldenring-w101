"""Elden Ring's crafting recipes for the hud_orbs spellbook's crafting page (w101_orbs\\book\\er\\er_recipes.tsv), read
from the game's own regulation.bin (read only: never deploy a regulation.bin). Run it in the cloud container.

Usage: python er_recipes.py <regulation.bin> <Defs folder> <er_item_names.tsv> <er_item_text.tsv> <out.tsv>

Every row of ShopLineupParam_Recipe that makes something (Elden Ring's crafting menu): the result (equipType 0 weapon =
arrows and bolts, 3 goods), how many one craft makes (setNum), the cookbook flag that unlocks it (eventFlag_forRelease,
0 = known from the start), and its materials (EquipMtrlSetParam materialId01-06 / itemNum01-06, all goods).

er_recipes.tsv, one row per recipe in ShopLineupParam_Recipe order; the row's index is its number in the crafting
events (emevd.py --crafting reads the same file, so the two always agree):
  index, row, kind (W / G), result item id, set (how many one craft makes), max (most the player can hold: goods
  maxNum, arrows maxArrowQuantity), release flag, tab (crafting.rs TABS: 1 pots and aromatics, 2 greases, 3 arrows
  and bolts, 4 food and medicine, 5 tools and throwables), materials "id:count,id:count", cookbook (the cookbook whose
  caption lists it, "" when known from the start), result name
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from er_params import Regulation  # noqa: E402

FOOD = ("Boluses", "Meat", "Liver", "Flesh", "Turtle Neck", "Fowl Foot", "Raisin", "Nostrum", "Remedy")


def tab(kind, name):
    if kind == "W":
        return 3
    if "Grease" in name:
        return 2
    if any(k in name for k in ("Pot", "Aromatic", "Spraymist")):
        return 1
    if any(k in name for k in FOOD):
        return 4
    return 5


def main(regulation, defs, names_tsv, text_tsv, out):
    reg = Regulation(regulation, defs)
    shop = reg.param("ShopLineupParam_Recipe", "ShopLineupParam")
    sets = reg.param("EquipMtrlSetParam")
    goods = reg.param("EquipParamGoods")
    weapons = reg.param("EquipParamWeapon")
    names = {}
    for line in open(names_tsv, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 3:
            names[(f[0], int(f[1]))] = f[2]
    # Which cookbook teaches what: each cookbook's caption lists its recipes as "- Name" lines.
    taught = {}
    for line in open(text_tsv, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 3 and f[0] == "C":
            book = names.get(("G", int(f[1])), "")
            for item in f[2].split("\\n"):
                if item.strip().startswith("-"):
                    taught.setdefault(item.strip()[1:].strip(), book)
    rows = []
    for row_id, r in sorted(shop.items()):
        kind = {0: "W", 3: "G"}.get(r["equipType"])
        mats = sets.get(r["mtrlId"])
        if kind is None or mats is None:
            continue
        item = r["equipId"]
        name = names.get((kind, item))
        if not name:
            continue
        need = [(mats[f"materialId0{i}"], mats[f"itemNum0{i}"]) for i in range(1, 7)]
        need = [(m, n) for m, n in need if m > 0 and n > 0]
        if any(mats[f"materialCate0{i}"] != 4 for i in range(1, 7) if mats[f"materialId0{i}"] > 0):
            raise SystemExit(f"recipe {row_id}: a material that isn't goods")
        most = goods[item]["maxNum"] if kind == "G" else weapons[item]["maxArrowQuantity"]
        flag = r["eventFlag_forRelease"]
        rows.append([row_id, kind, item, r["setNum"], most, flag, tab(kind, name),
                     ",".join(f"{m}:{n}" for m, n in need), taught.get(name, ""), name])
    # One cookbook per flag (recipes the caption lists under another name share their flag's book).
    by_flag = {}
    for row in rows:
        if row[5] and row[8]:
            by_flag.setdefault(row[5], []).append(row[8])
    for row in rows:
        books = by_flag.get(row[5], [])
        row[8] = max(set(books), key=books.count) if row[5] and books else ""
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        for i, row in enumerate(rows):
            f.write("\t".join(str(x) for x in [i] + row) + "\n")
    print(f"{len(rows)} recipes -> {out}")


if __name__ == "__main__":
    main(*sys.argv[1:6])
