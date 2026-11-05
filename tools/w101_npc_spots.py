"""Round 26 (N1): which Wizard101 NPC stands where on a zone, PROVED from the client's own data (stdlib only).

For every placed object of a zone's gamedata.bin whose template is an NPC, prints the chain
  instance name -> template id (the object's m_templateID) -> TemplateManifest.xml -> ObjectData/<file>.xml
  -> its display-name key (WC-NPCs_000000nn ...) -> the Locale/en-US .lang text
and the object's OWN position and yaw, read with w101_pathing's property decoder (each object's properties are one
record, so position and name cannot come from different objects). As an independent check, every
"Player Stand In instance - <NPC name>" (the zone's cinematic camera markers, ObjectData/EditorOnly/PlayerStandIn) is
matched to the NPC of that name: its distance from the NPC and the cosine between the NPC's facing (-sin yaw, -cos yaw)
and the direction to the marker (1.0 = the NPC looks straight at where the player stands in its cutscene).

Rounds 15-25 read positions with w101_lang.py `pos`, which took the first location AFTER an object's name. The
serializer writes m_location BEFORE m_name, so that was the next object's location: every NPC got its neighbour's
spot and yaw (fixed there in round 26).

Usage: python -I w101_npc_spots.py <zone .wad> <dir with TemplateManifest.xml, ObjectData/..., Locale/en-US/*.lang>
"""
import math
import re
import struct
import sys
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import w101_lang as L  # noqa: E402
import w101_pathing as P  # noqa: E402

CLIENT_OBJECTS, H_SCALE = 0x349E3284, 0x1DFD45A5


def lang_text(root):
    out = {}
    for f in (root / "Locale" / "en-US").glob("*.lang"):
        table, entries = L.parse_lang(f.read_bytes())
        out.update({f"{table}_{key}": value for key, _comment, value in entries})
    return out


def template_files(root):
    raw = (root / "TemplateManifest.xml").read_bytes()
    raw = zlib.decompress(raw[13:]) if P.u32(raw, 4) & 8 else raw

    def find(template):
        i = raw.find(b":\x8f5\x89" + struct.pack("<I", template))  # m_id hash, then the u32 id after the file name
        names = re.findall(rb"ObjectData/[ -~]+?\.xml", raw[max(0, i - 200):i]) if i > 0 else []
        return names[-1].decode() if names else None
    return find


def spots(wad, root):
    root = Path(root)
    text, find = lang_text(root), template_files(root)
    data = P.load(P.wad_files(wad)["gamedata.bin"])
    rows = []
    for o in data.root["props"][CLIENT_OBJECTS]:
        p = o["props"]
        if P.H_TEMPLATE not in p:
            continue
        template = P.i64(p[P.H_TEMPLATE])
        file = find(template)
        keys = []
        if file and (root / file).exists():
            raw = (root / file).read_bytes()
            keys = sorted(set(m.decode() for m in re.findall(rb"[A-Za-z0-9\-]+_[0-9A-F]{8}", raw)))
        rows.append({"instance": P.string(p.get(P.H_OBJ_NAME, b""), False), "template": template, "file": file,
                     "names": [(k, text[k]) for k in keys if k in text],
                     "pos": P.vec(p[P.H_LOCATION]), "yaw": P.vec(p[P.H_ORIENT])[2] if P.H_ORIENT in p else 0.0})
    return rows


def main(argv):
    rows = spots(argv[1], argv[2])
    npcs = [r for r in rows if r["names"] and "NPC" in (r["file"] or "")]
    print("instance\ttemplate\tObjectData\tname key = text\tW101 x, y, z (m)\tyaw (deg)")
    for r in npcs:
        x, y, z = (v / 100 for v in r["pos"])
        print(f"{r['instance']}\t{r['template']}\t{r['file']}\t{r['names'][0][0]} = {r['names'][0][1]}\t"
              f"{x:.3f}, {y:.3f}, {z:.3f}\t{math.degrees(r['yaw']):.2f}")
    print("\ncheck: Player Stand In markers (cutscene spot of the player) against the NPC of that name")
    for r in rows:
        m = re.match(r"Player Stand In instance - (.+)", r["instance"])
        if not m:
            continue
        npc = next((n for n in npcs if n["names"][0][1] == m.group(1)), None)
        if npc is None:
            print(f"{r['instance']}: no NPC of that name")
            continue
        dx, dy = r["pos"][0] - npc["pos"][0], r["pos"][1] - npc["pos"][1]
        d = math.hypot(dx, dy)
        cos = (-math.sin(npc["yaw"]) * dx - math.cos(npc["yaw"]) * dy) / d
        print(f"{r['instance']}: {d / 100:.2f} m from {npc['instance']}, facing cos {cos:.3f}")


if __name__ == "__main__":
    main(sys.argv)
