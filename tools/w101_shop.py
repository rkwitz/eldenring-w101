"""Round 27 (B2): Ugo Kalahad's shop (Kalahad's Adventure Wear) for the HUD's Wizard101 shop window (hud_orbs
shop.rs) and its events (emevd.py --shop): book/er/shop.tsv.

Wizard101 stock (wizard101central wiki, NPC:Ugo_Kalahad, read 2026-11-02; Ugo's ObjectData template
WC-ST01-NPC08_Shop has a ShoppingBehavior "WizardShops_00000002" but the item list is server side): hat Soft Hood
24-25 gold, robe Novice's Robe 32-33, boots Senior Novice's Shoes 222-230, wands (40 each) Scepter of Lost Lore,
Wand of Effervescence, Frosted Wand, Scepter of Ignition, Reaper's Rod, Wand of Equivalence, Baton of the Clouds.
Each is sold as an Elden Ring equivalent (STOCK) for RUNES_PER_GOLD runes per gold (the top of the wiki's range).

Selling (Wizard101's SELL tab, like an Elden Ring merchant): the Elden Ring pieces of the shop's four categories at
their EquipParam sellValue: Hats = head armour, Robes = chest armour, Boots = leg armour, Wands = staves and sacred
seals (unupgraded, wepType 57 / 61); rows with sellValue 0 (unsellable in Elden Ring) are left out.

Lines (tab-separated):  B  n  category  W101 name  gold  runes  item id (with category bits)
                        S  n  category  item id  runes
n: the event index (buy 0..15, sell from SELL_BASE); category: hat, robe, boots, wand.

Usage: python w101_shop.py regulation.bin <Smithbox PARAM\\ER\\Defs> <w101_orbs\\er_item_names.tsv> out.tsv
Elden Ring data: never commit the output.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from er_params import Regulation  # noqa: E402

RUNES_PER_GOLD = 25
SELL_BASE = 16
PROTECTOR, WEAPON = 0x10000000, 0
# (category, Wizard101 name, gold, Elden Ring item id with category bits, why)
STOCK = [
    ("hat", "Soft Hood", 25, PROTECTOR | 630000, "Astrologer Hood: a soft cloth hood for a beginning caster"),
    ("robe", "Novice's Robe", 33, PROTECTOR | 630100, "Astrologer Robe: the starting sorcerer's robe"),
    ("boots", "Senior Novice's Shoes", 230, PROTECTOR | 630300, "Astrologer Trousers: legs go with boots"),
    ("wand", "Scepter of Lost Lore", 40, WEAPON | 33130000, "Myth (lore): Astrologer's Staff"),
    ("wand", "Wand of Effervescence", 40, WEAPON | 34000000, "Life: Finger Seal (healing incantations)"),
    ("wand", "Frosted Wand", 40, WEAPON | 33040000, "Ice: Crystal Staff"),
    ("wand", "Scepter of Ignition", 40, WEAPON | 34020000, "Fire: Giant's Seal (fire incantations)"),
    ("wand", "Reaper's Rod", 40, WEAPON | 33180000, "Death: Prince of Death's Staff"),
    ("wand", "Wand of Equivalence", 40, WEAPON | 34060000, "Balance: Golden Order Seal"),
    ("wand", "Baton of the Clouds", 40, WEAPON | 34080000, "Storm: Dragon Communion Seal (lightning)"),
]


def names(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) == 3 and f[1].isdigit():
            out[(f[0], int(f[1]))] = f[2]
    return out


def main():
    reg_path, defs, names_path, out = sys.argv[1:5]
    reg = Regulation(reg_path, defs)
    nm = names(names_path)
    pr, wp = reg.param("EquipParamProtector"), reg.param("EquipParamWeapon")
    lines = []
    for n, (cat, w101, gold, item, _) in enumerate(STOCK):
        lines.append(f"B\t{n}\t{cat}\t{w101}\t{gold}\t{gold * RUNES_PER_GOLD}\t{item}")
    sells = []
    for pid, row in sorted(pr.items()):
        cat = "hat" if row["headEquip"] else "robe" if row["bodyEquip"] else "boots" if row["legEquip"] else None
        name = nm.get(("P", pid), "")
        if cat and row["sellValue"] > 0 and name and not name.startswith("["):
            sells.append((cat, PROTECTOR | pid, row["sellValue"]))
    for wid, row in sorted(wp.items()):
        name = nm.get(("W", wid), "")
        if row["wepType"] in (57, 61) and wid % 100 == 0 and row["sellValue"] > 0 and name and not name.startswith("["):
            sells.append(("wand", WEAPON | wid, row["sellValue"]))
    for k, (cat, item, runes) in enumerate(sells):
        lines.append(f"S\t{SELL_BASE + k}\t{cat}\t{item}\t{runes}")
    Path(out).write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{out}: {len(STOCK)} to buy, {len(sells)} to sell "
          f"({', '.join(f'{c} {sum(1 for s in sells if s[0] == c)}' for c in ('hat', 'robe', 'boots', 'wand'))})")


if __name__ == "__main__":
    main()
