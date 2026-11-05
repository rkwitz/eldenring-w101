"""Read Wizard101 locale text (.lang tables) straight from a KIWAD archive (stdlib only).

Format of a .lang file (Root.wad Locale/<lang>/<Table>.lang): UTF-16LE with BOM, CRLF lines. Line 1 is
"1:<Table>"; then one entry per three lines: key, comment (often empty), value. Most files end with an empty line.
Values are single lines (line breaks inside text are written as a literal backslash-n). Other files refer to an entry
as "<Table>_<key>" (e.g. ObjectData NPC names "WC-NPCs_00000024", quest text "WizQst1625C2_00000013").

Quest text: each quest has its own table WizQst<hex id> (dialogue, goal targets, location names "Zone|World"), and
QuestTitle.lang maps the same hex id to the quest's title (QuestTitle[1625C2] = "Ghost Hunters"). Goal verbs ("Talk
to", "Collect", ...) are in WizardQuestGoals.lang and goal/popup templates in Quest.lang. Quest logic (goal order,
counts, prerequisites, rewards) is server-side and not in the client.

Usage:
  python -I w101_lang.py dump <Root.wad> <out.json> [table-regex]   all tables {table: [[key, comment, value], ...]}
  python -I w101_lang.py grep <Root.wad> <value-regex> [table-regex]
  python -I w101_lang.py quest <Root.wad> <hex id> ...               title + every entry of WizQst<id>
  python -I w101_lang.py pos <gamedata.bin> [name-regex]              placed objects of a zone and their W101 position
"""
import json
import re
import struct
import sys
import zlib

LOCALE = "Locale/en-US/"


def wad_index(path):
    """[(name, offset, size, zsize, is_zip)] of a KIWAD archive."""
    with open(path, "rb") as f:
        head = f.read(14)
        assert head[:5] == b"KIWAD", "not a KIWAD archive"
        version, count = struct.unpack_from("<II", head, 5)
        f.seek(13 + (1 if version >= 2 else 0))
        out = []
        for _ in range(count):
            offset, size, zsize, is_zip, _crc, name_len = struct.unpack("<IIIBII", f.read(21))
            name = f.read(name_len).rstrip(b"\0").decode("utf-8", "replace")
            out.append((name, offset, size, zsize, is_zip))
        return out


def wad_read(path, entries):
    """{name: bytes} for the given index entries."""
    out = {}
    with open(path, "rb") as f:
        for name, offset, size, zsize, is_zip in entries:
            f.seek(offset)
            raw = f.read(zsize if is_zip else size)
            out[name] = zlib.decompress(raw) if is_zip else raw
    return out


def parse_lang(data):
    """(table, [(key, comment, value)]) of one .lang file."""
    text = data.decode("utf-16")  # strips the BOM
    lines = text.split("\r\n")
    table = lines[0].split(":", 1)[-1]
    body = lines[1:]
    if body and body[-1] == "" and len(body) % 3 == 1:
        body = body[:-1]
    return table, [tuple(body[i:i + 3]) for i in range(0, len(body) - 2, 3)]


def lang_tables(wad, table_pattern=None, locale=LOCALE):
    """{table: [(key, comment, value)]} of every .lang in `locale`, optionally only tables matching `table_pattern`."""
    pat = re.compile(table_pattern) if table_pattern else None
    entries = [e for e in wad_index(wad) if e[0].startswith(locale) and e[0].endswith(".lang")
               and (pat is None or pat.search(e[0][len(locale):-5]))]
    tables = {}
    for name, data in wad_read(wad, entries).items():
        table, rows = parse_lang(data)
        tables[table] = rows
    return tables


def quest_text(tables, quest_id):
    """{"id", "title", "entries": {key: value}} of quest `quest_id` (hex string as in WizQst<id>)."""
    titles = {k: v for k, _, v in tables.get("QuestTitle", [])}
    rows = tables.get("WizQst" + quest_id, [])
    return {"id": quest_id, "title": titles.get(quest_id), "entries": {k: v for k, _, v in rows}}


NAME_HASH, POS_HASH = bytes.fromhex("f303faca"), bytes.fromhex("1bdc7e85")


def placed_objects(gamedata):
    """[(name, x, y, z)] of a zone's gamedata.bin: each object name (hash f303faca, u16 length, text) and the
    position of the same object (hash 1bdc7e85 = m_location, 3 floats, W101 units, Z up), which the serializer writes
    BEFORE the name: the last position between the previous name and this one.
    Round 26 (N1): rounds 15-25 took the first position AFTER the name, which is the NEXT object's location, so every
    NPC got its neighbour's spot (Connelly stood on Ugo's spot, Ceren on Olivia's, Olivia on Connelly's). Checked
    against w101_pathing's property decoder (tools/w101_npc_spots.py) and the zone's "Player Stand In instance - <NPC>"
    cinematic markers, which stand 3 m in front of the right NPC."""
    out, i, prev = [], 0, 0
    while True:
        i = gamedata.find(NAME_HASH, i)
        if i < 0:
            return out
        n = struct.unpack_from("<H", gamedata, i + 4)[0]
        name = gamedata[i + 6:i + 6 + n].decode("latin-1")
        j = gamedata.rfind(POS_HASH, prev, i)
        if j >= 0:
            out.append((name,) + struct.unpack_from("<3f", gamedata, j + 4))
        prev = i = i + 6 + n


def main(argv):
    cmd = argv[1]
    if cmd == "dump":
        tables = lang_tables(argv[2], argv[4] if len(argv) > 4 else None)
        with open(argv[3], "w", encoding="utf-8") as f:
            json.dump(tables, f, ensure_ascii=False)
        print(len(tables), "tables,", sum(len(r) for r in tables.values()), "entries")
    elif cmd == "grep":
        pat = re.compile(argv[3], re.I)
        for table, rows in sorted(lang_tables(argv[2], argv[4] if len(argv) > 4 else None).items()):
            for key, _, value in rows:
                if pat.search(value):
                    print(f"{table}|{key}|{value}")
    elif cmd == "quest":
        ids = argv[3:]
        tables = lang_tables(argv[2], "^(QuestTitle|" + "|".join("WizQst" + q for q in ids) + ")$")
        for q in ids:
            qt = quest_text(tables, q)
            print(f"== {q} {qt['title']}")
            for key, value in qt["entries"].items():
                print(f"  {key}: {value}")
    elif cmd == "pos":
        pat = re.compile(argv[3] if len(argv) > 3 else ".", re.I)
        with open(argv[2], "rb") as f:
            for name, x, y, z in placed_objects(f.read()):
                if pat.search(name):
                    print(f"{name}\t{x:.1f}\t{y:.1f}\t{z:.1f}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
