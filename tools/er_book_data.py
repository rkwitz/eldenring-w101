"""Elden Ring data for the hud_orbs spellbook (w101_orbs\\book\\er): item stats, short item texts, item icons and the
world map, all read from the game's own files (run it in the cloud container, which has zstandard, cryptography,
Pillow and pyooz; the game files are staged from the PC).

Usage:
  python er_book_data.py stats <regulation.bin> <Smithbox PARAM\\ER\\Defs> <item_dlc02.msgbnd.dcx> <out>
      -> out/er_item_stats.tsv, out/er_item_text.tsv
  python er_book_data.py icons <menu\\hi\\01_common.tpf.dcx> <menu\\hi\\01_common.sblytbnd.dcx> <out> [size]
      -> out/icons/<icon id>.png (every MENU_ItemIcon sprite, size px, default 128) and out/icons/map_*.png
  python er_book_data.py map-ranges <71_maptile.tpfbhd> <ranges.txt>
      -> "offset size name" of the Lands Between's map tiles at zoom levels L0 (41x41 tiles) and L2 (11x11), each in
         its fully revealed, unburnt variant (the name ends in a bitmask of map fragments); cut
         those ranges out of 71_maptile.tpfbdt on the PC into one slices file, in order
  python er_book_data.py map <ranges.txt> <slices.bin> <out>
      -> out/map/<L0|L2>_<column>_<row>.png

er_item_stats.tsv, one row per param row, `<cat>\\t<id>\\t<key>=<value>...` (cat as er_item_names.tsv: W weapon base
row (item id - upgrade level), P armour, A talisman, G goods, M spell (goods id), E Ash of War, R ReinforceParamWeapon):
  W: icon, wt, atk = physical,magic,fire,lightning,holy base attack, sc = str,dex,int,fai,arc scaling, req = the
     same requirements, rf = reinforce type (R row rf + level), kind = weapon class
  R: atk = attack rates, sc = scaling rates
  P: icon, wt, slot (0 head .. 3 legs), cut = physical,strike,slash,pierce,magic,fire,lightning,holy absorption %,
     res = immunity,robustness,focus,vitality, poise
  A: icon, wt;  E: icon;  G: icon, type (goodsType), max
  M: fp, slots, req = int,fai,arc, school (the W101 school tab: Fire Ice Storm Myth Life Death Balance Astral),
     erschool (Elden Ring's own school)
er_item_text.tsv: `<cat>\\t<id>\\t<one-line Info text>`.
"""

import io
import re
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from dcx_krak import decompress  # noqa: E402
from er_params import Regulation  # noqa: E402
from er_text import bnd4, read_fmg  # noqa: E402

WEAPON_KINDS = {
    1: "Dagger", 3: "Straight Sword", 5: "Greatsword", 7: "Colossal Sword", 9: "Curved Sword", 11: "Curved Greatsword",
    13: "Katana", 14: "Twinblade", 15: "Thrusting Sword", 16: "Heavy Thrusting Sword", 17: "Axe", 19: "Greataxe",
    21: "Hammer", 23: "Great Hammer", 24: "Flail", 25: "Spear", 28: "Great Spear", 29: "Halberd", 31: "Reaper",
    35: "Fist", 37: "Claw", 39: "Whip", 41: "Colossal Weapon", 50: "Light Bow", 51: "Bow", 53: "Greatbow",
    55: "Crossbow", 56: "Ballista", 57: "Glintstone Staff", 61: "Sacred Seal", 65: "Small Shield", 67: "Medium Shield",
    69: "Greatshield", 81: "Arrow", 83: "Greatarrow", 85: "Bolt", 86: "Greatbolt", 87: "Torch", 88: "Hand-to-Hand",
    89: "Perfume Bottle", 90: "Thrusting Shield", 91: "Throwing Blade", 92: "Backhand Blade", 93: "Light Greatsword",
    94: "Great Katana", 95: "Beast Claw",
}

# Spell schools: (goodsType, sortGroupId) -> (Elden Ring school, Wizard101 school tab). goodsType 5 / 17 =
# sorcery (attack / support), 16 / 18 = incantation; sortGroupId is the in-game sort group, one per school.
SCHOOLS = {
    ("S", 10): ("Glintstone", "Myth"), ("S", 20): ("Carian", "Myth"), ("S", 30): ("Night and Magma", "Astral"),
    ("S", 40): ("Crystal and Cold", "Ice"), ("S", 50): ("Gravity and Primeval", "Balance"), ("S", 60): ("Death", "Death"),
    ("I", 10): ("Two Fingers", "Life"), ("I", 20): ("Golden Order", "Life"), ("I", 25): ("Miquella", "Life"),
    ("I", 30): ("Dragon Cult", "Storm"), ("I", 35): ("Erdtree", "Life"), ("I", 40): ("Fire Monk and Giants", "Fire"),
    ("I", 45): ("Messmer's Flame", "Fire"), ("I", 50): ("Godskin and Bestial", "Death"),
    ("I", 60): ("Blood and Rot", "Death"), ("I", 70): ("Frenzied Flame", "Astral"),
    ("I", 80): ("Dragon Communion", "Balance"),
}

TEXT_TABLES = {"WeaponInfo": "W", "ProtectorInfo": "P", "AccessoryInfo": "A", "GoodsInfo": "G", "GemInfo": "E",
               "MagicInfo": "M"}


def fmt(values):
    return ",".join(f"{v:g}" if isinstance(v, float) else str(v) for v in values)


def stats(regulation, defs, msgbnd, out):
    reg = Regulation(regulation, defs)
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    lines = []
    weapons = reg.param("EquipParamWeapon")
    for row_id, w in weapons.items():
        if row_id < 100 or w["iconId"] <= 0:
            continue
        lines.append(("W", row_id, {
            "icon": w["iconId"], "wt": round(w["weight"], 1),
            "atk": fmt([w["attackBasePhysics"], w["attackBaseMagic"], w["attackBaseFire"], w["attackBaseThunder"],
                        w["attackBaseDark"]]),
            "sc": fmt([round(w[k], 1) for k in ("correctStrength", "correctAgility", "correctMagic", "correctFaith",
                                                "correctLuck")]),
            "req": fmt([w[k] for k in ("properStrength", "properAgility", "properMagic", "properFaith", "properLuck")]),
            "rf": w["reinforceTypeId"], "kind": WEAPON_KINDS.get(w["wepType"], ""),
        }))
    for row_id, r in reg.param("ReinforceParamWeapon").items():
        lines.append(("R", row_id, {
            "atk": fmt([round(r[k], 3) for k in ("physicsAtkRate", "magicAtkRate", "fireAtkRate", "thunderAtkRate",
                                                 "darkAtkRate")]),
            "sc": fmt([round(r[k], 3) for k in ("correctStrengthRate", "correctAgilityRate", "correctMagicRate",
                                                "correctFaithRate", "correctLuckRate")]),
        }))
    for row_id, p in reg.param("EquipParamProtector").items():
        if p["iconIdM"] <= 0:
            continue
        cut = [p[k] for k in ("neutralDamageCutRate", "blowDamageCutRate", "slashDamageCutRate", "thrustDamageCutRate",
                              "magicDamageCutRate", "fireDamageCutRate", "thunderDamageCutRate", "darkDamageCutRate")]
        lines.append(("P", row_id, {
            "icon": p["iconIdM"], "wt": round(p["weight"], 1), "slot": p["protectorCategory"],
            "cut": fmt([round((1 - c) * 100, 1) for c in cut]),
            "res": fmt([p["resistPoison"] + p["resistDisease"], p["resistBlood"] + p["resistFreeze"],
                        p["resistSleep"] + p["resistMadness"], p["resistCurse"]]),
            "poise": round(p["toughnessCorrectRate"] * 1000, 1),
        }))
    for row_id, a in reg.param("EquipParamAccessory").items():
        if a["iconId"] > 0:
            lines.append(("A", row_id, {"icon": a["iconId"], "wt": round(a["weight"], 1)}))
    for row_id, e in reg.param("EquipParamGem").items():
        if e["iconId"] > 0:
            lines.append(("E", row_id, {"icon": e["iconId"]}))
    goods = reg.param("EquipParamGoods")
    magic = reg.param("Magic", "MagicParam")
    for row_id, g in goods.items():
        if g["iconId"] > 0:
            lines.append(("G", row_id, {"icon": g["iconId"], "type": g["goodsType"], "max": g["maxNum"]}))
        kind = {5: "S", 17: "S", 16: "I", 18: "I"}.get(g["goodsType"])
        m = magic.get(row_id)
        if kind and m:
            er_school, tab = SCHOOLS.get((kind, g["sortGroupId"]), ("Sorcery" if kind == "S" else "Incantation",
                                                                    "Balance"))
            lines.append(("M", row_id, {
                "fp": m["mp"], "slots": m["slotLength"],
                "req": fmt([m["requirementIntellect"], m["requirementFaith"], m["requirementLuck"]]),
                "school": tab, "erschool": er_school, "kind": "Sorcery" if kind == "S" else "Incantation",
            }))
    with open(out / "er_item_stats.tsv", "w", encoding="utf-8", newline="\n") as f:
        for cat, row_id, values in lines:
            f.write("\t".join([cat, str(row_id)] + [f"{k}={v}" for k, v in values.items()]) + "\n")
    print(f"er_item_stats.tsv: {len(lines)} rows")

    files = bnd4(decompress(Path(msgbnd).read_bytes()))
    texts = {}
    for path in sorted(files, key=lambda p: ("_dlc" in p, p)):
        stem = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".fmg")
        cat = TEXT_TABLES.get(stem.split("_dlc")[0])
        if cat is None:
            continue
        for text_id, text in read_fmg(files[path]).items():
            text = (text or "").strip()
            if text and not text.startswith("[ERROR]"):
                texts[(cat, text_id)] = re.sub(r"\s+", " ", text)
    with open(out / "er_item_text.tsv", "w", encoding="utf-8", newline="\n") as f:
        for (cat, text_id), text in sorted(texts.items()):
            f.write(f"{cat}\t{text_id}\t{text}\n")
    print(f"er_item_text.tsv: {len(texts)} texts")


def tpf_textures(data):
    """{name: DDS bytes} of a PC TPF."""
    count = struct.unpack_from("<i", data, 8)[0]
    out = {}
    for i in range(count):
        offset, size = struct.unpack_from("<Ii", data, 0x10 + 20 * i)
        name_offset = struct.unpack_from("<I", data, 0x10 + 20 * i + 12)[0]
        end = name_offset
        while data[end:end + 2] != b"\0\0":
            end += 2
        out[data[name_offset:end].decode("utf-16-le")] = data[offset:offset + size]
    return out


def icons(tpf, sblyt, out, size=128):
    from PIL import Image
    out = Path(out) / "icons"
    out.mkdir(parents=True, exist_ok=True)
    textures = tpf_textures(decompress(Path(tpf).read_bytes()))
    layouts = bnd4(decompress(Path(sblyt).read_bytes()))
    count = 0
    for path, xml in layouts.items():
        atlas = path.replace("\\", "/").rsplit("/", 1)[-1].removesuffix(".layout")
        if not (atlas.startswith("SB_Icon") or atlas.startswith("SB_MapCursor")) or atlas not in textures:
            continue
        image = Image.open(io.BytesIO(textures[atlas]))
        image.load()
        for m in re.finditer(rb'<SubTexture name="([^"]+)" x="(\d+)" y="(\d+)" width="(\d+)" height="(\d+)"', xml):
            name = m.group(1).decode()
            x, y, w, h = (int(v) for v in m.groups()[1:])
            sprite = image.crop((x, y, x + w, y + h))
            icon = re.match(r"MENU_ItemIcon_(\d+)\.png", name)
            if icon:
                sprite.resize((size, size), Image.LANCZOS).save(out / f"{int(icon.group(1))}.png", optimize=True)
            else:
                sprite.save(out / ("map_" + name.removeprefix("MENU_MAP_").removesuffix(".png") + ".png"))
            count += 1
    print(f"{count} icons in {out}")


def bhf4(data):
    """[(name, offset, size)] of a BHF4 (.tpfbhd) header."""
    assert data[:4] == b"BHF4", data[:4]
    count = struct.unpack_from("<i", data, 0x0C)[0]
    entry_size = struct.unpack_from("<q", data, 0x20)[0]
    unicode = data[0x30]
    out = []
    for i in range(count):
        at = 0x40 + i * entry_size
        # 0x24-byte entries: flags, -1, size (q), uncompressed size (q), offset (u32), id, name offset
        size = struct.unpack_from("<q", data, at + 8)[0]
        offset, _, name_offset = struct.unpack_from("<IiI", data, at + 24)
        end = name_offset
        if unicode:
            while data[end:end + 2] != b"\0\0":
                end += 2
            name = data[name_offset:end].decode("utf-16-le")
        else:
            end = data.index(b"\0", name_offset)
            name = data[name_offset:end].decode("shift-jis")
        out.append((name, offset, size))
    return out


def map_ranges(bhd, ranges):
    best = {}
    for name, offset, size in bhf4(Path(bhd).read_bytes()):
        m = re.search(r"M00_(L[02])_(\d+)_(\d+)_([0-9a-fA-F]+)\.tpf", name)
        if not m:
            continue
        mask = int(m.group(4), 16)
        key = (m.group(1), int(m.group(2)), int(m.group(3)))
        # Bits 0-14 = the map fragments (WorldMapPieceParam rows 0-14) revealed; bits 15-20 are later states of the
        # world (the ashen capital and others), left out.
        score = (mask < 1 << 15, bin(mask).count("1"), -mask)
        if key not in best or score > best[key][0]:
            best[key] = (score, offset, size, name.replace("\\", "/").rsplit("/", 1)[-1])
    with open(ranges, "w") as f:
        for _, offset, size, name in (best[k] for k in sorted(best)):
            f.write(f"{offset} {size} {name}\n")
    print(f"{len(best)} tiles, {sum(v[2] for v in best.values())} bytes")


def map_tiles(ranges, slices, out):
    from PIL import Image
    out = Path(out) / "map"
    out.mkdir(parents=True, exist_ok=True)
    data = Path(slices).read_bytes()
    at = 0
    for line in open(ranges):
        _, size, name = line.split()
        size = int(size)
        m = re.search(r"M00_(L[02])_(\d+)_(\d+)_", name)
        tpf = decompress(data[at:at + size])
        at += size
        (dds,) = tpf_textures(tpf).values()
        image = Image.open(io.BytesIO(dds))
        image.load()
        image.convert("RGB").save(out / f"{m.group(1)}_{int(m.group(2))}_{int(m.group(3))}.jpg", quality=88)
    print(f"map tiles in {out}")


def main():
    command = sys.argv[1]
    if command == "stats":
        stats(*sys.argv[2:6])
    elif command == "icons":
        icons(sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 128)
    elif command == "map-ranges":
        map_ranges(sys.argv[2], sys.argv[3])
    elif command == "map":
        map_tiles(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
