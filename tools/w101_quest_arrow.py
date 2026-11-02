"""Round 22 (hud H2, H7): Wizard101's 3D quest arrow mesh and the hedge maze's goal picture for the hud_orbs DLL, as
files under w101_orbs\\book on Ryan's PC (never committed: Wizard101 data).

H2: _Shared-WorldData.wad Character/Arrow/WC_QuestArrow.nif is Wizard101's solid quest arrow (one NiTriStrips, 92
vertices, 104 triangles: a bevelled arrow 62 units long, tip at +x, 36 wide along z, 11 thick along y), textured with
FX_Quest_Arrow_JT (the same yellow, black-edged texture as Quest/Quest_Arrow.png). Written as
Quest/WC_QuestArrow.tsv: "V x y z u v" per vertex, "T a b c" per triangle (strip unrolled, degenerate ones dropped).
(The flat Character/Arrow/Quest_Arrow_JT.nif that round 18 drew is the ground pointer the street's ObjectData arrows
use, OMNI-POINTER / WC-ARROW-NPC01.)
H7: the Hedge Maze goal's picture: _Shared-WorldData.wad GUI/QuestButtons/Art_Quest_AV_WalledGarden.dds (Wizard101's
green "place" disc, a hedge garden) as GUI/QuestButtons/Art_Quest_AV_WalledGarden.png.

Usage: python -I w101_quest_arrow.py path/to/Wizard101/Data/GameData w101_orbs/book   (stdlib + Pillow)
"""
import struct
import sys
from io import BytesIO
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from w101_quest_art import wad_read  # noqa: E402

SHARED = "_Shared-WorldData.wad"


def blocks(d):
    """(type, bytes) of every block of a Gamebryo 20.3 file."""
    i = d.index(b"\n") + 1
    _ver, _endian, _user, n = struct.unpack_from("<IBII", d, i)
    i += 13
    ntypes, = struct.unpack_from("<H", d, i)
    i += 2
    types = []
    for _ in range(ntypes):
        k, = struct.unpack_from("<I", d, i)
        types.append(d[i + 4:i + 4 + k].decode())
        i += 4 + k
    index = struct.unpack_from(f"<{n}H", d, i)
    i += 2 * n
    sizes = struct.unpack_from(f"<{n}I", d, i)
    i += 4 * n
    nstr, _ = struct.unpack_from("<II", d, i)
    i += 8
    for _ in range(nstr):
        k, = struct.unpack_from("<I", d, i)
        i += 4 + k
    ngroups, = struct.unpack_from("<I", d, i)
    i += 4 + 4 * ngroups
    out = []
    for b in range(n):
        out.append((types[index[b]], d[i:i + sizes[b]]))
        i += sizes[b]
    return out


def mesh(nif):
    blk = next(b for t, b in blocks(nif) if t == "NiTriStripsData")
    j = 4
    nv, = struct.unpack_from("<H", blk, j)
    j += 5
    verts = [struct.unpack_from("<3f", blk, j + 12 * k) for k in range(nv)]
    j += 12 * nv
    flags, = struct.unpack_from("<H", blk, j)
    j += 2
    assert flags & 0x3F == 1, "one UV set expected"
    if blk[j]:
        j += 12 * nv  # normals (the DLL lights each face from its own plane)
    j += 1 + 16  # bounding sphere
    if blk[j]:
        j += 16 * nv
    j += 1
    uvs = [struct.unpack_from("<2f", blk, j + 8 * k) for k in range(nv)]
    j += 8 * nv + 6
    _ntri, nstrips = struct.unpack_from("<HH", blk, j)
    j += 4
    lengths = struct.unpack_from(f"<{nstrips}H", blk, j)
    j += 2 * nstrips + 1
    tris = []
    for length in lengths:
        s = struct.unpack_from(f"<{length}H", blk, j)
        j += 2 * length
        for k in range(length - 2):
            a, b, c = s[k], s[k + 1], s[k + 2]
            if k % 2:
                a, b = b, a
            if len({a, b, c}) == 3:
                tris.append((a, b, c))
    return verts, uvs, tris


def main(argv):
    gamedata, book = Path(argv[1]), Path(argv[2])
    verts, uvs, tris = mesh(wad_read(gamedata / SHARED, "Character/Arrow/WC_QuestArrow.nif"))
    lines = [f"V\t{x:.4f}\t{y:.4f}\t{z:.4f}\t{u:.5f}\t{v:.5f}" for (x, y, z), (u, v) in zip(verts, uvs)]
    lines += [f"T\t{a}\t{b}\t{c}" for a, b, c in tris]
    out = book / "Quest" / "WC_QuestArrow.tsv"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n")
    print(f"{out}: {len(verts)} vertices, {len(tris)} triangles")
    picture = book / "art" / "GUI" / "QuestButtons" / "Art_Quest_AV_WalledGarden.png"
    picture.parent.mkdir(parents=True, exist_ok=True)
    Image.open(BytesIO(wad_read(gamedata / SHARED, "GUI/QuestButtons/Art_Quest_AV_WalledGarden.dds"))).convert("RGBA").save(picture)
    print(picture)


if __name__ == "__main__":
    main(sys.argv)
