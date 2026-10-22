"""Decode how Wizard101 moves mobs around a zone, and draw it top-down.

Reads a zone's data straight out of its KIWAD archive(s) and renders two PNGs plus a JSON dump:

  <out>/unicorn_way_pathing.png      every path, spawn group, duel sigil, walk-graph node and volume on the street
  <out>/unicorn_way_paths_grid.png   each mob path on its own, zoomed, with numbered nodes
  <out>/unicorn_way_pathing.json     the decoded data (positions in W101 units, Z up)

Usage:
  python w101_pathing.py --wad ZoneHD.wad [--wad ClassicMode-Zone.wad] --obj street.obj --out DIR [--root DIR]

--root is a folder holding Root.wad's TemplateManifest.xml and ObjectData/<mob>.xml files (any layout, matched by
basename) so spawns can be named and each mob's PathBehavior / PathMovementBehavior / DuelistBehavior read. Without it
the template ids are printed instead.

Data files (all in the zone wad; ClassicMode wads only override the .nif, they carry no game data):
  pathData.xml      BINd ObjectProperty: list of paths {m_id gid, m_name, node id list}. A path is just an ordered
                    list of node ids; nodes carry no links, so there are no branches.
  pathNodeData.bin  same serializer without the BINd header (plain u32 lengths): list of nodes
                    {m_location Vector3D, m_id gid, m_direction float, 3 unknown ints}.
  spawnData.xml     list of spawn groups {name, m_id, active, max/min counts, respawn time, list of weighted spawn
                    entries {weight, SpawnLocation {template id, DYNAMIC_SERVER, node-pick mode SNT_RANDOM /
                    SNT_RANDOM_UNIQUE, m_pathID}}}. Groups have no position of their own: a mob appears on a node of
                    its path.
  gamedata.bin      zone info plus every client object (Combat Sigil NNN = DuelCircle template 560, radius 700) and
                    named locations (Start, teleport targets).
  zone.nav          u16 n, u16 count, u16 0; count * (3 floats, u16 index); u32 edge count; edges as (u16 a, u16 b),
                    stored in both directions. A 480-unit grid graph over the walkable street.
  volumes.xml       trigger volumes (spheres/boxes) for music, POIs, teleports.
Positions are W101 units (1 unit = 1 cm), Z up. The OBJ from nif2obj is (x, z, -y) * 0.01, so W101 (x, y) =
(obj.x, -obj.z) * 100.
"""

import argparse
import json
import math
import re
import struct
import sys
import zlib
from functools import lru_cache
from pathlib import Path

# ---------------------------------------------------------------- KIWAD + ObjectProperty binary


def wad_files(path):
    """{name: bytes} of a KingsIsle KIWAD archive."""
    d = Path(path).read_bytes()
    assert d[:5] == b"KIWAD", f"{path} is not a KIWAD"
    version, count = struct.unpack_from("<II", d, 5)
    p, out = 13 + (1 if version >= 2 else 0), {}
    for _ in range(count):
        offset, size, zsize, compressed, _crc, name_len = struct.unpack_from("<IIIBII", d, p)
        p += 21
        name = d[p:p + name_len - 1].decode()
        p += name_len
        raw = d[offset:offset + (zsize if compressed else size)]
        out[name] = zlib.decompress(raw) if compressed else raw
    return out


def u32(d, o):
    return struct.unpack_from("<I", d, o)[0]


class KiBinary:
    """KingsIsle ObjectProperty binary. An object is (class hash u32, bit size u32, properties); a property is
    (bit size u32, property hash u32, value). Properties start on byte boundaries, but bools are counted as 1 bit and
    some ints as 7 bits too many, so byte lengths are found by trying ceil/floor of size/8 until the object closes."""

    def __init__(self, data):
        self.compact = data[:4] == b"BINd"  # flags 7: compact length prefixes, enums written as strings
        if self.compact and u32(data, 4) & 8:  # compressed: u32 size then zlib
            data = data[:8] + zlib.decompress(data[13:])
        self.d = data
        start = 8 if self.compact else 0
        self.root = self.obj(start, len(data))

    def props(self, a, b):
        d = self.d

        @lru_cache(None)
        def go(p):
            if p == b:
                return ()
            if p + 8 > b:
                return None
            size = u32(d, p)
            if size < 64 or size > (b - p) * 8 + 7:
                return None
            for n in sorted({(size + 7) // 8, size // 8}, reverse=True):
                if n >= 8 and p + n <= b:
                    rest = go(p + n)
                    if rest is not None:
                        return ((u32(d, p + 4), p + 8, p + n),) + rest
            return None

        return go(a)

    def obj(self, a, b):
        if a + 8 > b or u32(self.d, a + 4) < 32:
            return None
        props = self.props(a + 8, b)
        if props is None:
            return None
        return {"class": u32(self.d, a), "props": {h: self.value(pa, pb) for h, pa, pb in props}}

    def length(self, a):
        d = self.d
        if not self.compact:
            return u32(d, a), 4
        return (u32(d, a) >> 1, 4) if d[a] & 1 else (d[a] >> 1, 1)

    def objlist(self, a, b):
        d = self.d
        if a + (1 if self.compact else 4) > b:
            return None
        n, k = self.length(a)
        if not 0 < n <= 5000:
            return None

        def go(q, left):
            if left == 0:
                return [] if q == b else None
            if q + 8 > b or u32(d, q + 4) < 32:
                return None
            lo = q + 4 + u32(d, q + 4) // 8
            for e in range(lo, min(b, lo + max(16, u32(d, q + 4) // 32)) + 1):
                if (left == 1) != (e == b):
                    continue
                if left > 1 and (e + 8 > b or u32(d, e + 4) < 32 or u32(d, e + 4) // 8 > b - e):
                    continue
                o = self.obj(q, e)
                if o is not None:
                    rest = go(e, left - 1)
                    if rest is not None:
                        return [o] + rest
            return None

        return go(a + k, n)

    def value(self, a, b):
        v = None
        if b - a >= 16 and u32(self.d, a + 4) >= 64:
            v = self.obj(a, b)
        if v is None and b - a >= 9:
            v = self.objlist(a, b)
        return v if v is not None else self.d[a:b]


def load(data):
    sys.setrecursionlimit(max(sys.getrecursionlimit(), 20000))
    k = KiBinary(data)
    assert k.root is not None, "could not parse"
    return k


def string(v, compact=True):
    """A std::string value: compact 1-byte length (BINd) or u16 length (headerless files)."""
    if isinstance(v, bytes) and v:
        if compact and not v[0] & 1 and v[0] >> 1 == len(v) - 1:
            return v[1:].decode("latin1")
        if len(v) >= 2 and struct.unpack_from("<H", v)[0] == len(v) - 2:
            return v[2:].decode("latin1")
    return None


def f32(v):
    return struct.unpack("<f", v)[0]


def i32(v):
    return struct.unpack("<i", v)[0] if len(v) == 4 else v[0]


def i64(v):
    return struct.unpack("<q", v)[0]


def vec(v):
    return list(struct.unpack("<3f", v))


# property hashes (djb2(name) + KI string id(type), masked to 31 bits); names confirmed where noted
H_ID, H_NAME, H_LOCATION, H_ORIENT = 0x7C9B9045, 0x665CD49C, 0x857EDC1B, 0x8BB77F8E  # m_id gid, -, m_location, -
H_DIRECTION, H_PATH_ID, H_TEMPLATE, H_RADIUS = 0x5B15F2F8, 0x30F70232, 0x25C8A9AF, 0x3AF933DF  # m_direction, m_pathID, -, m_radius
H_PATH_NODES, H_OBJ_NAME = 0x5544AE1E, 0xCAFA03F3

# ---------------------------------------------------------------- decoding


def decode_paths(files):
    nodes = {}
    for o in load(files["pathNodeData.bin"]).root["props"][0x825D4A91]:
        p = o["props"]
        nodes[i64(p[H_ID])] = {"pos": vec(p[H_LOCATION]), "direction": f32(p[H_DIRECTION])}
    paths = []
    for o in load(files["pathData.xml"]).root["props"][0x960EABFA]:
        p = o["props"]
        raw = p[H_PATH_NODES]
        n = raw[0] >> 1
        ids = list(struct.unpack_from(f"<{n}q", raw, 1))
        paths.append({"id": i64(p[H_ID]), "name": string(p[H_NAME]), "nodes": ids})
    return nodes, paths


def decode_spawns(files):
    groups = []
    for o in load(files["spawnData.xml"]).root["props"][0xBD221102]:
        p = o["props"]
        entries = []
        for e in p[0x9C313256]:
            loc = e["props"][0x584C9987]["props"]
            entries.append({
                "weight": e["props"][0x3CFDBD03][0],
                "template": i64(loc[H_TEMPLATE]),
                "object_type": string(loc[0xD1F3E716]),
                "node_pick": string(loc[0x5C23425F]),  # SNT_RANDOM: any node, SNT_RANDOM_UNIQUE: no two on one node
                "start_node": i32(loc[0x5F4203E1]),  # -1 for mobs, 0 for treasure
                "path_id": i64(loc[H_PATH_ID]),
            })
        groups.append({
            "name": string(p[H_NAME]), "id": i64(p[H_ID]), "active": bool(p[0x0E43F82F][0]),
            "count_a": i32(p[0x263AD58D]),  # 2 or 4 for mob groups, 20 for wisps: most alive at once
            "count_b": i32(p[0x63E111B9]),  # 1 or 2
            "respawn_s": i32(p[0x2CE15B92]),  # 30 for mobs, 400 for chests, 10 for wisps
            "flag_3b265c3f": bool(p[0x3B265C3F][0]),
            "entries": entries,
        })
    return groups


def decode_gamedata(files):
    k = load(files["gamedata.bin"])
    root = k.root["props"]
    sigils, points = [], []
    for o in root[0x7E033ECF]:  # named locations
        p = o["props"]
        points.append({"name": string(p[0x94773FFA], False), "pos": vec(p[H_LOCATION]), "yaw": f32(p[H_DIRECTION])})
    for o in root[0x349E3284]:  # client objects
        p = o["props"]
        if H_TEMPLATE in p and i64(p[H_TEMPLATE]) == 560:  # ObjectData/DuelCircle.xml
            sigils.append({"name": string(p[H_OBJ_NAME], False), "pos": vec(p[H_LOCATION]),
                           "yaw": vec(p[H_ORIENT])[2], "radius": f32(p[H_RADIUS])})
    return sigils, points


def decode_nav(files):
    d = files["zone.nav"]
    _, count, _ = struct.unpack_from("<3H", d, 0)
    p, pts = 6, []
    for i in range(count):
        x, y, z, n = struct.unpack_from("<3fH", d, p)
        assert n == i, "zone.nav node index out of order"
        pts.append([x, y, z])
        p += 14
    (n_edges,) = struct.unpack_from("<I", d, p)
    edges = [struct.unpack_from("<2H", d, p + 4 + 4 * i) for i in range(n_edges)]
    return pts, sorted({tuple(sorted(e)) for e in edges})


def decode_volumes(files):
    out = []
    for o in load(files["volumes.xml"]).root["props"][0x884BFB48]:
        p = o["props"]
        out.append({"name": string(p.get(0xC6E6048B, b"")) or "",
                    "pos": [f32(p[0x7DB3F828]), f32(p[0x7DB3F829]), f32(p[0x7DB3F82A])],
                    "shape": string(p.get(0x8987B2CC, b"")) or "",
                    "radius": f32(p[H_RADIUS]) if H_RADIUS in p else 0.0,
                    "size": [f32(p[h]) if h in p else 0.0 for h in (0x2D481539, 0x35EBF597, 0x3492258C)]})
    return out


def decode_templates(root_dir, ids):
    """{template id: {name, path behavior...}} from TemplateManifest.xml and ObjectData xmls under root_dir."""
    if not root_dir:
        return {}
    root_dir = Path(root_dir)
    manifest = next(root_dir.rglob("TemplateManifest.xml"), None)
    if manifest is None:
        return {}
    d = manifest.read_bytes()
    raw = zlib.decompress(d[13:]) if u32(d, 4) & 8 else d
    files = {f.name: f for f in root_dir.rglob("*.xml")}
    out = {}
    for t in ids:
        i = raw.find(b":\x8f5\x89" + struct.pack("<I", t))  # m_id hash, then the u32 id after the file name
        names = re.findall(rb"ObjectData/[ -~]+?\.xml", raw[max(0, i - 200):i]) if i > 0 else []
        if not names:
            continue
        rel = names[-1].decode()
        info = {"file": rel}
        f = files.get(rel.split("/")[-1])
        if f is not None:
            info.update(mob_behaviors(f.read_bytes()))
        out[t] = info
    return out


def mob_behaviors(data):
    k = load(data)
    info = {}
    for b in k.root["props"].get(0x47651BD2, []):
        p = b["props"]
        kind = string(p.get(0xBA9B842C, b""))  # m_behaviorName
        if kind == "PathBehavior":
            info["path"] = {"kind": string(p[0xAC508C34]), "int_7c163556": i32(p[0x7C163556]),
                            "bool_9edb7a33": bool(p[0x9EDB7A33][0]), "int_30f4516d": i32(p[0x30F4516D]),
                            "float_1857e727": f32(p[0x1857E727])}
        elif kind == "PathMovementBehavior":
            info["speed"] = f32(p[0x2DD81273])  # m_movementSpeed, units/s
            info["speed_scale"] = f32(p[0x2DD0E16A])  # m_movementScale
        elif kind == "DuelistBehavior":
            info["duelist_radius"] = f32(p[0x4DE56BCD])
    name = string(k.root["props"].get(0x8071C6F3, b""))
    if name:
        info["name"] = name
    return info

# ---------------------------------------------------------------- geometry


def load_obj_triangles(path):
    """Top-down triangles (W101 units) of an OBJ written by nif2obj (x, z, -y)*0.01, plus each triangle's height and
    whether it faces up."""
    import numpy as np
    verts, faces = [], []
    with open(path, "rb") as f:
        for line in f:
            if line.startswith(b"v "):
                verts.append(line.split()[1:4])
            elif line.startswith(b"f "):
                idx = [int(t.split(b"/")[0]) for t in line.split()[1:]]
                for i in range(1, len(idx) - 1):
                    faces.append((idx[0], idx[i], idx[i + 1]))
    v = np.array(verts, dtype=np.float64)
    w = np.stack([v[:, 0] * 100, -v[:, 2] * 100, v[:, 1] * 100], axis=1)  # W101 x, y, z
    f = np.array(faces, dtype=np.int64)
    f = np.where(f > 0, f - 1, f + len(v))
    tri = w[f]
    n = np.cross(tri[:, 1] - tri[:, 0], tri[:, 2] - tri[:, 0])
    up = np.abs(n[:, 2]) > 0.7 * np.linalg.norm(n, axis=1).clip(1e-9)
    return tri, up

# ---------------------------------------------------------------- rendering


MOB_COLORS = ["#2a78d6", "#008a7a", "#e8890c", "#d6336c", "#7048c4", "#3d9a2b", "#c5452a", "#1f9fd1",
              "#b8860b", "#8c4fa0", "#d1477e", "#4a6fb5", "#2e8b57", "#cc5a00", "#6a5acd"]


def footprint_image(tri, up, extent, px_per_unit, nav_z):
    """Top-down grey raster of the street mesh, painted low to high: the street (near nav height) light grey,
    walls, roofs and trees darker the higher they are. Skybox-height geometry is dropped."""
    from PIL import Image, ImageDraw
    import numpy as np
    x0, x1, y0, y1 = extent
    w, h = int((x1 - x0) * px_per_unit), int((y1 - y0) * px_per_unit)
    img = Image.new("RGB", (w, h), (255, 255, 255))
    draw = ImageDraw.Draw(img)
    top = tri[:, :, 2].max(axis=1)
    px = (tri[:, :, 0] - x0) * px_per_unit
    py = (y1 - tri[:, :, 1]) * px_per_unit
    keep = (px.max(1) >= 0) & (px.min(1) <= w) & (py.max(1) >= 0) & (py.min(1) <= h) & (top < nav_z[1] + 3000)
    for t in np.argsort(top):
        if keep[t]:
            g = int(np.clip(228 - (top[t] - nav_z[1] - 150) / 2500 * 80, 160, 228))
            draw.polygon([(px[t, 0], py[t, 0]), (px[t, 1], py[t, 1]), (px[t, 2], py[t, 2])], fill=(g, g, g))
    return img


def classify(path_name):
    n = path_name.lower()
    if any(k in n for k in ("ghost", "skeleton", "fair", "wisp")):
        return "mob"
    return "resource"


def arrow(ax, a, b, color, size=9):
    import matplotlib.patches as mp
    mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
    dx, dy = b[0] - a[0], b[1] - a[1]
    L = math.hypot(dx, dy)
    if L < 1e-6:
        return
    s = min(L * 0.3, 1.2)
    ax.add_patch(mp.FancyArrowPatch((mx - dx / L * s / 2, my - dy / L * s / 2), (mx + dx / L * s / 2, my + dy / L * s / 2),
                                    arrowstyle="-|>", mutation_scale=size, color=color, lw=0, zorder=6))


def render(data, tri, up, out_dir):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.lines import Line2D
    import matplotlib.patches as mp

    nodes, paths, groups = data["nodes"], data["paths"], data["spawn_groups"]
    nav_pts, nav_edges = data["nav"]["nodes"], data["nav"]["edges"]
    templates = data["templates"]
    pts = [n["pos"] for n in nodes.values()] + nav_pts + [s["pos"] for s in data["sigils"]]
    xs, ys = [p[0] for p in pts], [p[1] for p in pts]
    pad = 2500
    extent = (min(xs) - pad, max(xs) + pad, min(ys) - pad, max(ys) + pad)
    navz = sorted(p[2] for p in nav_pts)
    nav_z = (navz[int(len(navz) * 0.02)], navz[int(len(navz) * 0.98)])
    M = 0.01  # W101 units -> metres for plotting
    ext_m = [e * M for e in extent]

    by_path = {}
    for g in groups:
        for e in g["entries"]:
            by_path.setdefault(e["path_id"], []).append((g, e))

    def mob_label(t):
        info = templates.get(t, {})
        return info.get("name") or Path(info.get("file", str(t))).stem

    mob_paths = [p for p in paths if classify(p["name"]) == "mob"]
    res_paths = [p for p in paths if classify(p["name"]) != "mob"]
    color = {p["id"]: MOB_COLORS[i % len(MOB_COLORS)] for i, p in enumerate(mob_paths)}

    # ------------------------------------------------ overview
    fig_w = 28
    fig_h = fig_w * (ext_m[3] - ext_m[2]) / (ext_m[1] - ext_m[0]) + 2.5
    img = footprint_image(tri, up, extent, 0.1, nav_z)
    fig, ax = plt.subplots(figsize=(fig_w, fig_h), dpi=110)
    ax.imshow(img, extent=ext_m, origin="upper", zorder=0, interpolation="bilinear")

    for a, b in nav_edges:
        p, q = nav_pts[a], nav_pts[b]
        ax.plot([p[0] * M, q[0] * M], [p[1] * M, q[1] * M], color="#9cc3e6", lw=0.5, zorder=1)
    ax.scatter([p[0] * M for p in nav_pts], [p[1] * M for p in nav_pts], s=2, color="#6fa8dc", zorder=1)

    for v in data["volumes"]:
        if v["shape"] == "Sphere" and v["radius"] > 0:
            ax.add_patch(mp.Circle((v["pos"][0] * M, v["pos"][1] * M), v["radius"] * M, fill=False, ls=(0, (2, 3)),
                                   ec="#b07cc6", lw=0.9, zorder=2))
            ax.text(v["pos"][0] * M, v["pos"][1] * M, v["name"], fontsize=6.5, color="#8a5aa0", ha="center", zorder=2)

    for p in res_paths:
        q = [nodes[i]["pos"] for i in p["nodes"]]
        ax.scatter([a[0] * M for a in q], [a[1] * M for a in q], s=14, marker="s", color="#a0703c", ec="white",
                   lw=0.4, zorder=3)
        ax.text(q[0][0] * M, q[0][1] * M + 1.2, p["name"], fontsize=6.5, color="#7a5226", zorder=3)

    for p in mob_paths:
        c = color[p["id"]]
        q = [nodes[i]["pos"] for i in p["nodes"]]
        loop = q + [q[0]]  # PT_LOOP: after the last node the mob walks back to the first
        ax.plot([a[0] * M for a in loop], [a[1] * M for a in loop], color=c, lw=2.2, zorder=4,
                solid_capstyle="round")
        for a, b in zip(loop, loop[1:]):
            arrow(ax, (a[0] * M, a[1] * M), (b[0] * M, b[1] * M), c, 14)
        ax.scatter([a[0] * M for a in q], [a[1] * M for a in q], s=26, color=c, ec="white", lw=0.8, zorder=5)
        ax.scatter([q[0][0] * M], [q[0][1] * M], s=80, color="white", ec=c, lw=2, zorder=5)
        spawns = by_path.get(p["id"], [])
        sub = "; ".join(f"{g['name']}: {mob_label(e['template'])} x{g['count_a']}" for g, e in spawns[:2])
        cx, cy = sum(a[0] for a in q) / len(q) * M, max(a[1] for a in q) * M
        ax.text(cx, cy + 2.0, p["name"], fontsize=9, fontweight="bold", color=c, ha="center", zorder=7,
                bbox=dict(boxstyle="round,pad=0.2", fc="white", ec=c, lw=0.6, alpha=0.9))
        if sub:
            ax.text(cx, cy + 0.6, sub, fontsize=6.5, color="#333", ha="center", zorder=7)

    for s in data["sigils"]:
        x, y = s["pos"][0] * M, s["pos"][1] * M
        ax.add_patch(mp.Circle((x, y), s["radius"] * M, fill=True, fc="#f4c43033", ec="#c99a00", lw=1.2, zorder=3))
        ax.scatter([x], [y], marker="*", s=110, color="#d9a400", ec="black", lw=0.4, zorder=8)
        ax.text(x, y - 1.6, s["name"].replace("Combat Sigil ", "S"), fontsize=7, ha="center", va="top", zorder=8)

    for pnt in data["points"]:
        x, y = pnt["pos"][0] * M, pnt["pos"][1] * M
        ax.scatter([x], [y], marker="v", s=40, color="#222", zorder=8)
        ax.text(x + 0.8, y + 0.8, pnt["name"], fontsize=6, color="#222", zorder=8)

    ax.set_xlim(ext_m[0], ext_m[1])
    ax.set_ylim(ext_m[2], ext_m[3])
    ax.set_aspect("equal")
    ax.set_xlabel("W101 X (m)")
    ax.set_ylabel("W101 Y (m)")
    ax.set_title("Wizard101 Unicorn Way (WC_Z06): mob paths, spawns, duel sigils and walk graph (top-down, Z up)",
                 fontsize=15, loc="left")

    # scale bar and north arrow (W101 +Y; the game's own map orientation is not stored with the zone)
    sx, sy = ext_m[0] + 6, ext_m[2] + 6
    for i in range(5):
        ax.add_patch(mp.Rectangle((sx + 10 * i, sy), 10, 0.8, fc="black" if i % 2 == 0 else "white", ec="black",
                                  lw=0.8, zorder=9))
    for i in range(0, 51, 10):
        ax.text(sx + i, sy + 1.4, f"{i}", fontsize=8, ha="center", zorder=9)
    ax.text(sx + 25, sy - 2.6, "metres", fontsize=8, ha="center", zorder=9)
    nx, ny = ext_m[1] - 8, ext_m[3] - 16
    ax.annotate("", xy=(nx, ny + 9), xytext=(nx, ny), arrowprops=dict(arrowstyle="-|>", lw=2, color="black"), zorder=9)
    ax.text(nx, ny + 10, "N (+Y)", ha="center", fontsize=10, fontweight="bold", zorder=9)

    handles = [Line2D([], [], color=color[p["id"]], lw=2.5, marker="o", label=p["name"]) for p in mob_paths]
    handles += [
        Line2D([], [], color="white", marker="o", mec="black", mew=1.5, ms=8, lw=0, label="first node of a path"),
        Line2D([], [], color="#a0703c", marker="s", lw=0, label="chest / reagent spawn nodes (no walking)"),
        Line2D([], [], color="#d9a400", marker="*", mec="black", ms=12, lw=0, label="combat sigil (radius 7 m shaded)"),
        Line2D([], [], color="#9cc3e6", lw=1, marker=".", mfc="#6fa8dc", label="zone.nav walk graph (4.8 m grid)"),
        Line2D([], [], color="#b07cc6", lw=1, ls=":", label="trigger volume (sphere)"),
        Line2D([], [], color="#222", marker="v", lw=0, label="named location (Start, teleports)"),
        mp.Patch(color="#e4e4e4", label="street surface (classic OBJ, top-down)"),
        mp.Patch(color="#a8a8a8", label="buildings, walls, trees (darker = higher)"),
    ]
    ax.legend(handles=handles, loc="lower right", fontsize=8.5, framealpha=0.95, title="Legend (arrows: walk direction)")
    fig.tight_layout()
    fig.savefig(out_dir / "unicorn_way_pathing.png")
    plt.close(fig)

    # ------------------------------------------------ small multiples
    cols = 4
    rows = math.ceil(len(mob_paths) / cols)
    fig, axes = plt.subplots(rows, cols, figsize=(cols * 6.5, rows * 6.5), dpi=110)
    big = footprint_image(tri, up, extent, 0.2, nav_z)
    for ax, p in zip(axes.flat, mob_paths):
        c = color[p["id"]]
        q = [nodes[i]["pos"] for i in p["nodes"]]
        loop = q + [q[0]]
        x = [a[0] * M for a in q]
        y = [a[1] * M for a in q]
        cx, cy = (min(x) + max(x)) / 2, (min(y) + max(y)) / 2
        half = max(max(x) - min(x), max(y) - min(y)) / 2 + 8
        ax.imshow(big, extent=ext_m, origin="upper", interpolation="bilinear")
        for a, b in nav_edges:
            pa, pb = nav_pts[a], nav_pts[b]
            ax.plot([pa[0] * M, pb[0] * M], [pa[1] * M, pb[1] * M], color="#9cc3e6", lw=0.4, zorder=1)
        for s in data["sigils"]:
            ax.add_patch(mp.Circle((s["pos"][0] * M, s["pos"][1] * M), s["radius"] * M, fc="#f4c43033", ec="#c99a00"))
            ax.scatter([s["pos"][0] * M], [s["pos"][1] * M], marker="*", s=90, color="#d9a400", ec="black", lw=0.4,
                       zorder=8)
        ax.plot([a[0] * M for a in loop], [a[1] * M for a in loop], color=c, lw=2.2, zorder=4)
        for a, b in zip(loop, loop[1:]):
            arrow(ax, (a[0] * M, a[1] * M), (b[0] * M, b[1] * M), c, 15)
        ax.scatter(x, y, s=40, color=c, ec="white", lw=0.8, zorder=5)
        for k, (xx, yy) in enumerate(zip(x, y)):
            ax.text(xx + 0.5, yy + 0.5, str(k + 1), fontsize=8, fontweight="bold", color="#111", zorder=6)
        length = sum(math.dist(a[:2], b[:2]) for a, b in zip(loop, loop[1:])) * M
        spawns = by_path.get(p["id"], [])
        lines = [f"{p['name']}  ({len(q)} nodes, loop {length:.0f} m)"]
        for g, e in spawns:
            info = templates.get(e["template"], {})
            sp = f", {info['speed'] / 100:.2f} m/s" if "speed" in info else ""
            lines.append(f"{g['name']}: {mob_label(e['template'])} x{g['count_a']}, "
                         f"respawn {g['respawn_s']} s{sp}")
        ax.set_title("\n".join(lines), fontsize=8.5, loc="left", color=c if not spawns else "black")
        ax.set_xlim(cx - half, cx + half)
        ax.set_ylim(cy - half, cy + half)
        ax.set_aspect("equal")
        ax.tick_params(labelsize=7)
        bar = 10 if half > 12 else 5
        ax.add_patch(mp.Rectangle((cx - half + 2, cy - half + 2), bar, 0.4, color="black"))
        ax.text(cx - half + 2 + bar / 2, cy - half + 3, f"{bar} m", fontsize=7, ha="center")
        ax.annotate("", xy=(cx + half - 3, cy + half - 2), xytext=(cx + half - 3, cy + half - 6),
                    arrowprops=dict(arrowstyle="-|>", color="black"))
        ax.text(cx + half - 3, cy + half - 1.6, "N", ha="center", fontsize=8, fontweight="bold")
    for ax in list(axes.flat)[len(mob_paths):]:
        ax.axis("off")
    fig.suptitle("Unicorn Way mob paths one by one (node numbers = walk order, PT_LOOP returns from the last node to 1)",
                 fontsize=14, x=0.01, y=0.995, ha="left")
    fig.tight_layout(rect=(0, 0, 1, 0.975))
    fig.savefig(out_dir / "unicorn_way_paths_grid.png")
    plt.close(fig)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--wad", action="append", required=True, help="zone wad(s); later wads override earlier files")
    ap.add_argument("--obj", required=True, help="street OBJ from nif2obj (x, z, -y) * 0.01")
    ap.add_argument("--out", required=True)
    ap.add_argument("--root", help="folder with TemplateManifest.xml and ObjectData xmls from Root.wad")
    a = ap.parse_args()
    files, owners = {}, {}
    for w in a.wad:
        for name, raw in wad_files(w).items():
            files[name] = raw
            owners.setdefault(name, []).append(Path(w).name)
    for w in a.wad:
        print(f"{Path(w).name}: {sorted(n for n, o in owners.items() if Path(w).name in o and '/' not in n)}")
    nodes, paths = decode_paths(files)
    groups = decode_spawns(files)
    sigils, points = decode_gamedata(files)
    nav_pts, nav_edges = decode_nav(files)
    volumes = decode_volumes(files)
    templates = decode_templates(a.root, sorted({e["template"] for g in groups for e in g["entries"]}))
    data = {"nodes": nodes, "paths": paths, "spawn_groups": groups, "sigils": sigils, "points": points,
            "nav": {"nodes": nav_pts, "edges": nav_edges}, "volumes": volumes, "templates": templates}
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "unicorn_way_pathing.json").write_text(json.dumps(data, indent=1, default=str))

    path_by_id = {p["id"]: p for p in paths}
    print(f"{len(paths)} paths, {len(nodes)} nodes, {len(groups)} spawn groups, {len(sigils)} sigils, "
          f"{len(nav_pts)} nav nodes / {len(nav_edges)} edges, {len(volumes)} volumes")
    for g in groups:
        for e in g["entries"]:
            t = templates.get(e["template"], {})
            p = path_by_id.get(e["path_id"], {"name": "?", "nodes": []})
            print(f"  {g['name']:<26} -> {p['name']:<18} ({len(p['nodes'])} nodes) {t.get('name', e['template'])} "
                  f"x{g['count_a']}/{g['count_b']} w{e['weight']} {e['node_pick']} respawn {g['respawn_s']}s "
                  f"speed {t.get('speed', '?')} {t.get('path', '')}")
    tri, up = load_obj_triangles(a.obj)
    render(data, tri, up, out)
    print("wrote", out / "unicorn_way_pathing.png", out / "unicorn_way_paths_grid.png")


if __name__ == "__main__":
    main()
