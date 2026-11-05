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

Round 23 (hud H1, Ryan: "the actual arrow design is not accurate. It should be a flat arrow, and then it's just
extended downwards. The one you have is too fancy"): every 3D arrow in the client files is bevelled
(WC_QuestArrow.nif and WizardCity-WorldData StateObjects/Quest_Arrows/WC_QuestArrows_01/_09.nif: 92-vertex bevelled
arrows; Root.wad Character/Arrow/Quest_Arrow_JT.nif = the flat quad plus a 54-vertex blob). So the flat arrow itself,
FX_Quest_Arrow_JT (book/art/Quest/Quest_Arrow.png, 128x256, tail at the top, tip at the bottom) on Quest_Arrow_JT.nif's
42.46 x 70 unit quad, is extruded into a slab SLAB_THICK units deep: Quest/Quest_Arrow_Slab.tsv (same "V"/"T" format;
top = the texture's outline, ear-clipped, UVs = the texture; sides straight down, UV = the arrow's yellow; bottom).

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


HALF_W, HALF_L = 21.23, 35.0  # Quest_Arrow_JT.nif's quad (questhelper.rs HALF_W / HALF_L)
SLAB_THICK = 9.0
SIDE_UV = (0.5, 0.35)  # a texel in the middle of the arrow's yellow


def outline(png, step=2, tolerance=1.2):
    """The arrow's outline (u, v) from the texture's alpha: every row is one span, so its left ends top to bottom
    and right ends bottom to top, simplified (Douglas-Peucker, `tolerance` texels)."""
    im = Image.open(png).convert("RGBA")
    w, h = im.size
    a = im.getchannel("A").load()
    left, right = [], []
    for y in range(0, h, 1):
        xs = [x for x in range(w) if a[x, y] > 128]
        if xs:
            left.append((xs[0], y + 0.5))
            right.append((xs[-1] + 1, y + 0.5))
    def dp(pts):
        if len(pts) < 3:
            return pts
        (x0, y0), (x1, y1) = pts[0], pts[-1]
        L = max(((x1 - x0) ** 2 + (y1 - y0) ** 2) ** 0.5, 1e-6)
        d = [abs((x1 - x0) * (y0 - y) - (x0 - x) * (y1 - y0)) / L for x, y in pts]
        k = max(range(len(d)), key=d.__getitem__)
        if d[k] <= tolerance:
            return [pts[0], pts[-1]]
        return dp(pts[:k + 1])[:-1] + dp(pts[k:])
    ring = dp(left) + dp(right[::-1])
    return [(x / w, y / h) for x, y in ring]


def ear_clip(poly):
    """Triangles (index triples, counter-clockwise like `poly`) of a simple polygon."""
    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    idx = list(range(len(poly)))
    out = []
    while len(idx) > 3:
        for k in range(len(idx)):
            i, j, l = idx[k - 1], idx[k], idx[(k + 1) % len(idx)]
            if cross(poly[i], poly[j], poly[l]) <= 1e-9:
                continue
            tri = (poly[i], poly[j], poly[l])
            if any(m not in (i, j, l) and all(cross(tri[q], tri[(q + 1) % 3], poly[m]) >= 0 for q in range(3))
                   for m in idx):
                continue
            out.append((i, j, l))
            idx.pop(k)
            break
        else:
            break
    if len(idx) == 3:
        out.append(tuple(idx))
    return out


def slab(png):
    """(vertices [(x, y, z, u, v)], triangles) of the extruded flat arrow (x along the arrow, tip +x; z across; y up)."""
    ring = outline(png)
    area = sum(a[0] * b[1] - b[0] * a[1] for a, b in zip(ring, ring[1:] + ring[:1]))
    if area < 0:
        ring = ring[::-1]
    pos = [(-HALF_L + v * 2 * HALF_L, -HALF_W + u * 2 * HALF_W) for u, v in ring]  # (x, z)
    n = len(ring)
    top, bottom = SLAB_THICK / 2, -SLAB_THICK / 2
    verts = [(x, top, z, u, v) for (x, z), (u, v) in zip(pos, ring)]
    verts += [(x, bottom, z, u, v) for (x, z), (u, v) in zip(pos, ring)]
    tris = []
    for a, b, c in ear_clip(ring):
        tris.append((a, b, c))
        tris.append((n + a, n + c, n + b))
    for i in range(n):  # sides: separate vertices on the yellow
        j = (i + 1) % n
        k = len(verts)
        for (x, z) in (pos[i], pos[j]):
            verts.append((x, top, z) + SIDE_UV)
        for (x, z) in (pos[i], pos[j]):
            verts.append((x, bottom, z) + SIDE_UV)
        tris.append((k, k + 3, k + 1))
        tris.append((k, k + 2, k + 3))
    return verts, tris


def main(argv):
    gamedata, book = Path(argv[1]), Path(argv[2])
    verts, uvs, tris = mesh(wad_read(gamedata / SHARED, "Character/Arrow/WC_QuestArrow.nif"))
    lines = [f"V\t{x:.4f}\t{y:.4f}\t{z:.4f}\t{u:.5f}\t{v:.5f}" for (x, y, z), (u, v) in zip(verts, uvs)]
    lines += [f"T\t{a}\t{b}\t{c}" for a, b, c in tris]
    out = book / "Quest" / "WC_QuestArrow.tsv"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(lines) + "\n", newline="\n")
    print(f"{out}: {len(verts)} vertices, {len(tris)} triangles")
    verts, tris = slab(book / "art" / "Quest" / "Quest_Arrow.png")
    lines = [f"V\t{x:.4f}\t{y:.4f}\t{z:.4f}\t{u:.5f}\t{v:.5f}" for x, y, z, u, v in verts]
    lines += [f"T\t{a}\t{b}\t{c}" for a, b, c in tris]
    out = book / "Quest" / "Quest_Arrow_Slab.tsv"
    out.write_text("\n".join(lines) + "\n", newline="\n")
    print(f"{out}: {len(verts)} vertices, {len(tris)} triangles (flat arrow extruded {SLAB_THICK} units)")
    picture = book / "art" / "GUI" / "QuestButtons" / "Art_Quest_AV_WalledGarden.png"
    picture.parent.mkdir(parents=True, exist_ok=True)
    Image.open(BytesIO(wad_read(gamedata / SHARED, "GUI/QuestButtons/Art_Quest_AV_WalledGarden.dds"))).convert("RGBA").save(picture)
    print(picture)


if __name__ == "__main__":
    main(sys.argv)
