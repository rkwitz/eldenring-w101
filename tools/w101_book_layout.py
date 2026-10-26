"""Turn Wizard101's spellbook page layouts (.gui) into flat layout files the hud_orbs spellbook draws, with the art,
fonts and English strings they use, plus the Wizard City loading screen.

Each page's .gui is a tree of windows (Window, ControlSprite, ControlText, ControlButton, ControlWidget, ...) whose
m_Window rectangles are already laid out for the page's fixed size, so a node's place is its parent's place plus its
own m_Window. This writes, per page, `<Page>.lay`: one line per node in drawing order (the .gui's child order, painter's
style), tab-separated `key=value` fields:

  id, p (parent id, -1 for the root), c (class), n (name), v (1 = VISIBLE flag), r (x0,y0,x1,y1 in page pixels),
  a (alpha), st (m_Style), fl (m_Flags)
  img, uv (u0,v0,u1,v1), tint (RRGGBBAA)          sprite material (ControlSprite, EnergyGlobe, PreviewWindow)
  img, f0/f1/f2/f3/f4 (uv of the normal, highlighted, depressed, greyed and checked frames), grow (hover scale)
                                                   buttons and check boxes
  back, buv                                        a HAS_BACK window's background (repeated), ControlWidget centre
  tl/t/tr/l/r/bl/b/br (image)                      ControlWidget border pieces, drawn at their own pixel size
  bar, pos, vert                                   ControlProgress
  font, fs (font scale), col, hcol (RRGGBBAA), sh (drop shadow), ol (outline), ww (word wrap), txt, label
                                                   text of ControlText, and the label of buttons

Strings like `<string;GUI_00000338>` are looked up in Locale/en-US/<table>.lang. Images are written as
art/<WAD path>.png, fonts as <font>.xml plus page PNGs (white with the glyphs in the alpha channel).

Usage: python w101_book_layout.py path\\to\\Wizard101\\Data\\GameData output_dir [--preview]

--preview also writes preview/<Page>.png, the page drawn with each node's own visibility (texts in a stand-in font),
to check the conversion. Needs Pillow.
"""
import html
import re
import struct
import sys
import xml.etree.ElementTree as ET
import zlib
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).parent))
from w101_hud_art import a8_dds  # noqa: E402

WADS = ["Root.wad", "_Shared-WorldData.wad", "WizardCity-WorldData.wad"]
# Book page per spellbook tab (SpellBookPrefsWindow.gui's tab buttons), plus the book itself.
PAGES = [
    "SpellBookPrefsWindow", "CharStatsSpellbookPage", "InventorySpellbookPage", "PetStatsSpellbookPage",
    "DeckConfiguration", "QuestListWindow", "MapSpellbookPage", "RecipeWindow", "HelpSpellbookPage", "Settings", "QuestInfoWindow",
]
# The Wizard City loading screen, the loading bar, and the wand mouse cursor (GUI/Cursors.xml "Default"; its
# cursor.cur twin puts the hot spot at 5, 5 of 45).
EXTRA_ART = ["GUI/LoadScreen_W01_WC_01.dds", "GUI/Art/Art_Progress_Bar_Large.dds", "GUI/Art/Art_Progress_Bar01.dds",
             "GUI/cursor.dds"]
# Card frames (deck cards, reagent cards) and ToolTip.gui's bubble, drawn by the DLL's own code.
EXTRA_ART += [f"GUI/Cards/School_{s}_Blnk.dds" for s in
              ("Fire", "Ice", "Storm", "Myth", "Life", "Death", "Balance", "Shadow", "Moon", "Treasure")]
EXTRA_ART += [f"GUI/Tooltip/bubble_{p}.png" for p in ("TL", "T", "TR", "L", "R", "BL", "B", "BR", "C")]
# Quest markers over the quest givers (markers.rs): "!" offers a quest, the "?" medallion is a goal to talk to them.
EXTRA_ART += ["GUI/QuestButtons/Quest_Start.dds", "GUI/QuestButtons/Art_Quest_Complete.dds"]
# Item tooltips (itemStatsInfo*.gui's frames and equip fist), the deck's slot frame, ReagentWindow's cards, tabs and
# recipe-book toggle.
EXTRA_ART += [f"GUI/Art/Art_Message_Box{c}_{p}.dds" for c in ("", "_Blue")
              for p in ("TL", "T", "TR", "L", "R", "BL", "B", "BR")]
EXTRA_ART += ["GUI/InventoryPage/Button_Equip.dds", "GUI/DeckConfig/Background_Slot.dds",
              "GUI/Backgrounds/Background_Reagent.dds", "GUI/Art/Art_CraftingTab.dds"]
EXTRA_ART += [f"GUI/Buttons/{b}.dds" for b in ("Button_Crafting_Reagents", "Button_Crafting_Recipes",
              "Button_Crafting_Harvest", "Button_Crafting_RareHarvest", "Button_Gold_Bag", "Button_Crafting_WorldDrop",
              "Button_Class_Project")]
# Tooltip stat rows (value, school icon, stat icon), the reagent card (School_Resource_Blnk) and its blank medallion,
# and the badge medals on the Character page's achievement rows.
EXTRA_ART += [f"GUI/Icons/{i}.dds" for i in ("Icon_All", "Type_Damage", "Type_Resist", "Type_Stun_Resist", "Type_Pip",
              "Type_PowerPips")]
EXTRA_ART += [f"GUI/Icons/Icon_SM_{s}.dds" for s in ("Myth", "Fire", "Storm", "Life", "Ice", "Death", "Balance")]
EXTRA_ART += ["GUI/Cards/School_Resource_Blnk.dds", "GUI/Reagents/Resource_Nothing.dds"]
EXTRA_ART += [f"GUI/Art/Art_Badge0{i}.dds" for i in range(1, 7)]
FONT_FILE = re.compile(r"([A-Za-z0-9_]+)\.xml$")


class Wads:
    """Lookup of entries across the WADs (by lower-case name), read on demand."""

    def __init__(self, game_data):
        self.entries = {}
        for name in WADS:
            path = Path(game_data) / name
            if not path.exists():
                continue
            with open(path, "rb") as f:
                head = f.read(14)
                assert head[:5] == b"KIWAD", f"{path} is not a Wizard101 WAD"
                version, count = struct.unpack_from("<II", head, 5)
                f.seek(13 + (1 if version >= 2 else 0))
                for _ in range(count):
                    offset, size, zsize, compressed, _crc, name_len = struct.unpack("<IIIBII", f.read(21))
                    entry = f.read(name_len)[:-1].decode(errors="replace")
                    self.entries.setdefault(entry.lower(), (path, offset, zsize if compressed else size, compressed))

    def read(self, name):
        hit = self.entries.get(name.lower())
        if not hit:
            return None
        path, offset, length, compressed = hit
        with open(path, "rb") as f:
            f.seek(offset)
            raw = f.read(length)
        return zlib.decompress(raw) if compressed else raw


class Strings:
    """Locale/en-US/<table>.lang files: a header line, then (key, comment, text) line triples."""

    def __init__(self, wads):
        self.wads, self.tables = wads, {}

    def lookup(self, text):
        def one(m):
            table, _, key = m.group(1).partition("_")
            if table not in self.tables:
                data = self.wads.read(f"Locale/en-US/{table}.lang")
                lines = data.decode("utf-16").splitlines() if data else []
                self.tables[table] = {lines[i]: lines[i + 2] for i in range(1, len(lines) - 2, 3)}
            return self.tables[table].get(key, "")
        text = html.unescape(text or "")
        return re.sub(r"<string;([^>]+)>", one, text)


def colour(abgr, default="FFFFFFFF"):
    """Wizard101's AABBGGRR as RRGGBBAA."""
    v = (abgr or default).strip()
    if len(v) != 8:
        v = default
    return v[6:8] + v[4:6] + v[2:4] + v[0:2]


def numbers(text, n):
    try:
        values = [float(x) for x in (text or "").split(",")]
    except ValueError:
        values = []
    return values if len(values) == n else None


def child(el, tag):
    """The Class inside <tag>, or None."""
    holder = el.find(tag)
    if holder is None or not len(holder):
        return None
    return holder[0]


def texture(material):
    if material is None:
        return None
    tex = child(material, "m_pDiffuseMap")
    return tex.findtext("m_sFilename") if tex is not None else None


def frames(material):
    out = [numbers(f.text, 4) for f in material.findall("m_Frames")]
    out = [f for f in out if f]
    if not out:
        tx, ty = int(material.findtext("m_TilesX") or 1), int(material.findtext("m_TilesY") or 1)
        out = [[x / tx, y / ty, (x + 1) / tx, (y + 1) / ty] for y in range(ty) for x in range(tx)]
    return out


def uv_text(uv):
    return ",".join(f"{v:.6g}" for v in uv)


class Converter:
    def __init__(self, wads, strings):
        self.wads, self.strings = wads, strings
        self.art, self.fonts = set(), set()

    def image(self, path):
        if not path:
            return None
        path = path if path.lower().startswith("gui/") else f"GUI/{path}"
        self.art.add(path)
        return str(Path(path).with_suffix(".png")).replace("\\", "/")

    def font(self, style):
        font = child(style, "m_pFont") if style is not None else None
        if font is None:
            return None
        m = FONT_FILE.search(font.findtext("m_sFilename") or "")
        if not m:
            return None
        self.fonts.add(m.group(1))
        return m.group(1), float(font.findtext("m_fScale") or 1), int(font.findtext("m_nKerningAdjust") or 0)

    def page(self, root):
        lines = []
        top = next(iter(root), None)
        if top is None:
            return lines
        self.node(top, -1, (0.0, 0.0), None, None, lines, top_level=True)
        return lines

    def node(self, el, parent, origin, inherited_style, inherited_font, lines, top_level=False):
        cls = (el.get("Name") or "").replace("class ", "")
        window = numbers(el.findtext("m_Window"), 4) or [0, 0, 0, 0]
        offset = numbers(el.findtext("m_Offset"), 2) or [0, 0]
        if top_level:
            window = [0, 0, window[2] - window[0], window[3] - window[1]]
        x0, y0 = origin[0] + window[0] + offset[0], origin[1] + window[1] + offset[1]
        rect = [x0, y0, x0 + window[2] - window[0], y0 + window[3] - window[1]]
        flags, style_flags = el.findtext("m_Flags") or "", el.findtext("m_Style") or ""
        own_style = child(el, "m_pWindowStyle")
        style = own_style if own_style is not None else inherited_style
        font = self.font(style) or inherited_font
        me = len(lines)
        fields = {
            "id": me, "p": parent, "c": cls, "n": el.findtext("m_sName") or "",
            "v": int("VISIBLE" in flags.split("|")), "r": uv_text(rect),
            "a": f"{float(el.findtext('m_fAlpha') or 1):.3g}", "st": style_flags, "fl": flags,
        }
        lines.append(fields)

        material = child(el, "m_pMaterial")
        if material is not None and cls not in ("ControlButton",):
            img = self.image(texture(material))
            if img:
                f = frames(material)
                current = int(material.findtext("m_nCurrentFrame") or 0)
                fields["img"] = img
                fields["uv"] = uv_text(f[min(current, len(f) - 1)])
                fields["tint"] = colour(el.findtext("m_Color"))
        button = child(el, "m_pButton")
        if button is not None:
            img = self.image(texture(button))
            if img:
                f = frames(button)
                fields["img"] = img
                for key, state in [("f0", "m_pNormalState"), ("f1", "m_pHighlightedState"),
                                   ("f2", "m_pDepressedState"), ("f3", "m_pGreyedState"),
                                   ("f4", "m_pCheckedState")]:
                    s = child(el, state)
                    if s is not None:
                        i = int(s.findtext("m_nMaterialFrame") or 0)
                        fields[key] = uv_text(f[min(i, len(f) - 1)])
                fields["tint"] = colour(el.findtext("m_Color"))
            if "EFFECT_SCALE" in style_flags:
                fields["grow"] = f"{float(el.findtext('m_fMaxScale') or 1.1):.3g}"
        if style is not None and "HAS_BACK" in style_flags:
            back = child(style, "m_pBackMaterial")
            img = self.image(texture(back)) if back is not None else None
            if img:
                fields["back"] = img
                fields["buv"] = uv_text(frames(back)[0])
        if cls == "ControlWidget":
            for key, tag in [("tl", "m_pTopLeftCornerMaterial"), ("tr", "m_pTopRightCornerMaterial"),
                             ("bl", "m_pBottomLeftCornerMaterial"), ("br", "m_pBottomRightCornerMaterial"),
                             ("t", "m_pTopEdgeMaterial"), ("b", "m_pBottomEdgeMaterial"),
                             ("l", "m_pLeftEdgeMaterial"), ("rt", "m_pRightEdgeMaterial"),
                             ("cm", "m_pCenterMaterial")]:
                img = self.image(texture(child(el, tag)))
                if img:
                    fields[key] = img
        if cls == "ControlProgress":
            bar = child(el, "m_pBarMaterial")
            img = self.image(texture(bar)) if bar is not None else None
            if img:
                fields["bar"] = img
                fields["pos"] = f"{float(el.findtext('m_fPosition') or 0):.3g}"
                fields["vert"] = el.findtext("m_bDisplayVertical") or "0"

        for tag, key in [("m_cardSize", "card"), ("m_spacing", "gap")]:
            if el.findtext(tag):
                fields[key] = el.findtext(tag)
        text = el.find("m_sText")
        label = el.findtext("m_sLabel")
        if text is not None or label:
            if font:
                fields["font"], fields["fs"] = font[0], f"{font[1]:.4g}"
                if font[2]:
                    fields["ka"] = str(font[2])  # extra pen advance per glyph, font pixels
            if style is not None:
                fields["col"] = colour(style.findtext("m_Color"), "FF000000")
                fields["hcol"] = colour(style.findtext("m_HighColor"))
            tc = child(el, "m_textComponent")
            shadow = el.findtext("m_bUseDropShadow") or (tc.findtext("m_useDropShadow") if tc is not None else "0")
            outline = el.findtext("m_bUseOutline") or (tc.findtext("m_useOutline") if tc is not None else "0")
            fields["sh"], fields["ol"] = shadow or "0", outline or "0"
            fields["ww"] = el.findtext("m_bEnableWordWrap") or "0"
            if text is not None:
                fields["txt"] = self.strings.lookup(text.text).replace("\t", " ").replace("\n", "\\n")
            if label:
                fields["label"] = self.strings.lookup(label).replace("\t", " ").replace("\n", "\\n")

        for holder in el.findall("m_Children"):
            for sub in holder:
                self.node(sub, me, (rect[0], rect[1]), style, font, lines)


def line_text(fields):
    return "\t".join(f"{k}={v}" for k, v in fields.items())


def preview(lines, out_dir, images):
    """A rough drawing of the page: images, and text boxes outlined with their text."""
    width = max(800, int(max(float(l["r"].split(",")[2]) for l in lines)))
    height = max(600, int(max(float(l["r"].split(",")[3]) for l in lines)))
    canvas = Image.new("RGBA", (width, height), (40, 40, 40, 255))
    draw = ImageDraw.Draw(canvas)
    shown = {}
    for l in lines:
        shown[l["id"]] = l["v"] == 1 and (l["p"] < 0 or shown[l["p"]])
        if not shown[l["id"]]:
            continue
        x0, y0, x1, y1 = [int(round(float(v))) for v in l["r"].split(",")]
        if x1 <= x0 or y1 <= y0:
            continue
        for key, uv_key in [("back", "buv"), ("img", "uv" if "uv" in l else "f0"), ("bar", None)]:
            if key not in l or l[key] not in images:
                continue
            src = images[l[key]]
            uv = [float(v) for v in l.get(uv_key, "0,0,1,1").split(",")] if uv_key else [0, 0, 1, 1]
            if max(uv) > 1.0:
                uv = [0, 0, 1, 1]  # repeated backgrounds: one stretched copy is enough for a preview
            box = (int(uv[0] * src.width), int(uv[1] * src.height), int(uv[2] * src.width), int(uv[3] * src.height))
            if box[2] <= box[0] or box[3] <= box[1]:
                continue
            piece = src.crop(box).resize((x1 - x0, y1 - y0))
            canvas.alpha_composite(piece, (x0, y0)) if x0 >= 0 and y0 >= 0 else None
        if "txt" in l or "label" in l:
            text = re.sub(r"<[^>]*>", "", l.get("txt") or l.get("label") or "")
            c = l.get("col", "000000FF")
            draw.text((x0 + 2, y0 + 1), text[:60], fill=(int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), 255))
    (out_dir / "preview").mkdir(exist_ok=True)
    return canvas


def main():
    game_data, out = Path(sys.argv[1]), Path(sys.argv[2])
    want_preview = "--preview" in sys.argv
    out.mkdir(parents=True, exist_ok=True)
    wads = Wads(game_data)
    strings = Strings(wads)
    conv = Converter(wads, strings)
    pages = {}
    for page in PAGES:
        data = wads.read(f"GUI/{page}.gui")
        if data is None:
            print(f"{page}.gui not found")
            continue
        root = ET.fromstring(data.decode("utf-8", "replace"))
        lines = conv.page(root)
        pages[page] = lines
        (out / f"{page}.lay").write_text("\n".join(line_text(l) for l in lines) + "\n", encoding="utf-8")
        print(f"{page}.lay: {len(lines)} nodes")
    for path in EXTRA_ART:
        conv.image(path)

    images, missing = {}, []
    for path in sorted(conv.art):
        data = wads.read(path) or wads.read(str(Path(path).with_suffix(".png"))) or wads.read(
            str(Path(path).with_suffix(".jpg")))
        target_name = str(Path(path).with_suffix(".png")).replace("\\", "/")
        if data is None:
            missing.append(path)
            continue
        try:
            image = Image.open(BytesIO(data)).convert("RGBA")
        except Exception as e:  # noqa: BLE001 - DDS variants Pillow can't read
            missing.append(f"{path} ({e})")
            continue
        target = out / "art" / target_name
        target.parent.mkdir(parents=True, exist_ok=True)
        image.save(target)
        images[target_name] = image
    for font in sorted(conv.fonts):
        xml = wads.read(f"Fonts/{font}.xml")
        if xml is None:
            missing.append(f"Fonts/{font}.xml")
            continue
        (out / f"{font}.xml").write_bytes(xml)
        for page in re.findall(r'FileName="([^"]+)"', xml.decode()):
            data = wads.read(f"Fonts/{page}")
            if data is None:
                missing.append(f"Fonts/{page}")
                continue
            alpha = a8_dds(data)
            image = Image.new("L", alpha.size, 255).convert("RGBA")
            image.putalpha(alpha)
            image.save(out / f"{Path(page).stem}.png")
    print(f"{len(images)} images, fonts {sorted(conv.fonts)}, missing: {missing}")
    if want_preview:
        for page, lines in pages.items():
            preview(lines, out, images).save(out / "preview" / f"{page}.png")


if __name__ == "__main__":
    main()
