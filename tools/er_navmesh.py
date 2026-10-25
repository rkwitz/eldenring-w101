"""Bake an Elden Ring navmesh (hkaiNavMesh) for The First Step from our assets' collision, so enemies on Unicorn Way
can walk their patrol routes and chase the player.

Elden Ring keeps each map tile's navmesh in map/m60/<tile>/<tile>.nvmhktbnd.dcx: a BND4 of Havok 2018 tagfiles
  n<tile>_<model>.hkx    the navmesh for normal-sized characters (clearance cache ceiling 1.53 m)
  o<tile>_<model>.hkx    a second navmesh for large characters (ceiling 5.1 m)
  q<tile>_<model>_N.hkx  empty placeholders
and <tile>.nva.dcx lists them (NVMA, section version 4, 0x90-byte entries: name id 1<model>NNN for n, 3<model>NNN
for o, model id, first global face index, face count). The project's nva had dropped the two real entries, so the
tile had no navmesh at all.

Each hkx holds an hkaiNavMesh (faces = convex polygons with up to ~10 edges, wound so cross(b-a, c-a).y > 0; edges
a->b with oppositeEdge/oppositeFace indices or 0xFFFFFFFF on the boundary, flags 4 = ORIGINAL; vertices in tile-local
metres, w 0), its clearance cache (per vertex: distance to the boundary; per edge: (interpolant, clearance) of the
best crossing point, (0, 0) on the boundary; per face: one value per pair of traversable edges, then 255; all as
bytes, 254 = at or above the ceiling) and an hkaiStaticTreeNavMeshQueryMediator whose hkcdStaticTree uses
Aabb6BytesCodec nodes (same box coding as the collision Aabb5Bytes nodes, 23-bit data: leaf = face index,
internal = right child offset / 2). All of this was read off vanilla m60_42_36_00 (see er-navmesh notes); the
vanilla file round-trips byte for byte through hkx_tag.

Pipeline: asset collision (fsnp compressed mesh, decoded by hkx_collision) -> placed by the map's part transforms
-> Recast (tools/recast_bake.cpp: voxelise, erode by the agent radius, regions, convex polygons; once for n, once
with a larger agent for o) -> keep the connected pieces that hold an enemy or the player spawn -> clip to the 256 m
tiles (no vanilla navmesh has a vertex outside its tile's +-128 m square; the street spans m60_41_36, 42_35 and
42_36) -> per tile, hkaiNavMesh + cache + tree written into that tile's vanilla n/o files, and its nva counts set.
Vanilla tiles keep open boundary edges on the tile border (no streaming sets), so cut points are shared exactly.
The first deploy (one mesh reaching 237 m outside m60_42_36) coincided with a load crash (2026-10-23).

Only --append output loads: every mesh that replaced vanilla's faces crashed the game on load, the same street
appended after vanilla's faces loads and enemies patrol it (2026-10-25).

Usage:
  python er_navmesh.py MAP.msb.dcx GAME/map/m60 OUT_DIR --append [--tiles 42,36 41,36] --asset AEG030_518_9000=aeg030_518_l.geomhkxbnd.dcx
         [--asset ...] [--recast recast_bake] [--preview out.png]
  writes OUT_DIR/m60_XX_ZZ_00/<tile>.nvmhktbnd.dcx and .nva.dcx for every tile the mesh touches.
DCX input may be DFLT or KRAK (read with pyooz block by block, dcx_krak.py). Output is DFLT DCX, which the game
loads. recast_bake is built from tools/recast_bake.cpp and Recast (github.com/recastnavigation, zlib
licence); the build line is at the top of that file.
"""

import argparse
import math
import struct
import subprocess
import sys
import tempfile
import zlib
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import dcx_krak  # noqa: E402
import hkx_collision  # noqa: E402
import hkx_tag  # noqa: E402
from hkx_tag import Array, Record  # noqa: E402
from msb_floor import _list, entry_name, read_msb, transform  # noqa: E402

BOUNDARY = 0xFFFFFFFF
EDGE_ORIGINAL = 4
SEED_MODELS = "c"  # every character part (player spawns c0000, enemies, bosses) seeds the walkable pieces
AGENT = dict(cell=0.3, cell_height=0.1, height=1.8, radius=0.4, climb=0.5, slope=50.0)  # n: normal characters
AGENT_LARGE = dict(AGENT, height=3.0, radius=0.9)  # o: large characters (vanilla o meshes are coarser)
TILE = 256.0
MIN_TILE_FACES = 8
DFLT_HEADER = bytes.fromhex("44435800000100000000001800000024000000440000004c44435300000000000000000044435000"
                            "44464c540000002009000000000000000000000000000000000101004443410000000008")


# --- files

def load_dcx(path):
    """Raw bytes of a DFLT or KRAK DCX file (KRAK through pyooz, see dcx_krak.py)."""
    return dcx_krak.decompress(Path(path).read_bytes())


def dflt_dcx(raw):
    packed = zlib.compress(raw, 9)
    head = bytearray(DFLT_HEADER)
    struct.pack_into(">II", head, 0x1C, len(raw), len(packed))
    return bytes(head) + packed


def bnd4_entries(bnd):
    """[(header offset, name, data)] of a BND4 with 0x24-byte entry headers (sizes, offset, id, name offset)."""
    assert bnd[:4] == b"BND4"
    count, = struct.unpack_from("<i", bnd, 0x0C)
    stride = 0x24
    out = []
    for i in range(count):
        h = 0x40 + i * stride
        size, = struct.unpack_from("<q", bnd, h + 0x08)
        offset, entry_id, name_at = struct.unpack_from("<III", bnd, h + 0x18)
        end = name_at
        while bnd[end:end + 2] != b"\0\0":
            end += 2
        out.append((h, bnd[name_at:end].decode("utf-16-le"), bnd[offset:offset + size]))
    return out


def bnd4_replace(bnd, new_data):
    """BND4 bytes with entries' data replaced ({entry name suffix: bytes}); everything before the first entry's data
    is kept and every entry starts on a 16-byte boundary, as in the game's files (Havok needs 16-byte alignment)."""
    entries = bnd4_entries(bnd)
    first = min(struct.unpack_from("<I", bnd, h + 0x18)[0] for h, _, _ in entries)
    out = bytearray(bnd[:first])
    for h, name, data in entries:
        for suffix, replacement in new_data.items():
            if name.lower().endswith(suffix.lower()):
                data = replacement
        while len(out) % 16:
            out.append(0)
        struct.pack_into("<qq", out, h + 0x08, len(data), len(data))
        struct.pack_into("<I", out, h + 0x18, len(out))
        out += data
    return bytes(out)


# --- geometry in

def placed_triangles(map_raw, assets):
    """Tile-local triangles of each asset's collision, placed by its part transform (world = pos + R_y(rot) * p)."""
    _, lists = read_msb(map_raw)
    parts = {entry_name(e): e for e in _list(lists, "PARTS_PARAM_ST").entries}
    tris = []
    for part, binder in assets:
        (px, py, pz), (_, ry, _) = transform(parts[part])
        c, s = math.cos(math.radians(ry)), math.sin(math.radians(ry))
        _, _, hkx = bnd4_entries(load_dcx(binder))[0]
        _, root = hkx_tag.read(hkx)
        decoded = hkx_collision.decode(hkx_collision.body_of(root)["shape"], check=False)
        for tri in decoded.values():
            pts = [(px + x * c + z * s, py + y, pz - x * s + z * c) for x, y, z in (v[:3] for v in tri)]
            # Recast only walks up-facing triangles; W101 geometry mostly faces the other way after the FLVER flip
            if _normal(pts)[1] < 0:
                pts = [pts[0], pts[2], pts[1]]
            tris.append(pts)
        print(f"{part}: {len(decoded)} collision triangles at ({px:.2f}, {py:.2f}, {pz:.2f}) rot {ry:.1f}")
    return tris


def seeds(map_raw):
    _, lists = read_msb(map_raw)
    out = []
    for e in _list(lists, "PARTS_PARAM_ST").entries:
        name = entry_name(e)
        if name.startswith(SEED_MODELS):
            out.append((name, transform(e)[0]))
    return out


def _normal(p):
    a = [p[1][k] - p[0][k] for k in range(3)]
    b = [p[2][k] - p[0][k] for k in range(3)]
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def bake(tris, recast, agent=AGENT):
    with tempfile.TemporaryDirectory() as tmp:
        soup, out = Path(tmp, "soup.bin"), Path(tmp, "polys.txt")
        soup.write_bytes(b"".join(struct.pack("<9f", *a, *b, *c) for a, b, c in tris))
        args = [agent[k] for k in ("cell", "cell_height", "height", "radius", "climb", "slope")]
        subprocess.run([recast, str(soup), str(out)] + [str(a) for a in args], check=True)
        verts, polys, heights = [], [], defaultdict(list)
        for line in out.read_text().splitlines():
            t = line.split()
            if t[0] == "v":
                verts.append([float(x) for x in t[1:]])
            elif t[0] == "p":
                polys.append([int(x) for x in t[2:]])
            elif t[0] == "d":  # detail vertices: the polygon's own vertices come first, with sampled heights
                dv = [float(x) for x in t[2:]]
                for k, vi in enumerate(polys[-1]):
                    heights[vi].append(dv[3 * k + 1])
    for vi, hs in heights.items():
        verts[vi][1] = sum(hs) / len(hs)
    return verts, polys


# --- mesh

def _edge_map(polys):
    owner = {}
    for f, p in enumerate(polys):
        for k in range(len(p)):
            owner[(p[k], p[(k + 1) % len(p)])] = (f, k)
    return owner


def _inside(px, pz, pts):
    inside = False
    for i in range(len(pts)):
        (x1, _, z1), (x2, _, z2) = pts[i], pts[i - 1]
        if (z1 > pz) != (z2 > pz) and px < (x2 - x1) * (pz - z1) / (z2 - z1) + x1:
            inside = not inside
    return inside


def keep_connected(verts, polys, seed_points, max_dy=3.0):
    """Polygons connected to any seed (a polygon under the seed within max_dy, else the nearest within 5 m)."""
    owner = _edge_map(polys)
    neighbours = [set() for _ in polys]
    for (a, b), (f, _) in owner.items():
        if (b, a) in owner:
            neighbours[f].add(owner[(b, a)][0])
    starts = set()
    for name, (x, y, z) in seed_points:
        best = None
        for f, p in enumerate(polys):
            pts = [verts[i] for i in p]
            cy = sum(q[1] for q in pts) / len(pts)
            if abs(cy - y) > max_dy and not (min(q[1] for q in pts) - max_dy <= y <= max(q[1] for q in pts) + max_dy):
                continue
            if _inside(x, z, pts):
                best = (0.0, f)
                break
            d = min(math.hypot(q[0] - x, q[2] - z) for q in pts)
            if d < 5.0 and (best is None or d < best[0]):
                best = (d, f)
        print(f"  seed {name} ({x:.1f}, {y:.1f}, {z:.1f}): " + (f"polygon {best[1]} ({best[0]:.1f} m)" if best else "NO NAVMESH"))
        if best:
            starts.add(best[1])
    seen, todo = set(starts), list(starts)
    while todo:
        f = todo.pop()
        for g in neighbours[f] - seen:
            seen.add(g)
            todo.append(g)
    kept = sorted(seen)
    used = sorted({i for f in kept for i in polys[f]})
    remap = {old: new for new, old in enumerate(used)}
    return [verts[i] for i in used], [[remap[i] for i in polys[f]] for f in kept]


def orient(verts, polys):
    """Wind every polygon so cross(b - a, c - a).y > 0 (vanilla's winding), by its signed area in x/z."""
    out = []
    for p in polys:
        area = sum(verts[p[k]][2] * verts[p[k - 1]][0] - verts[p[k]][0] * verts[p[k - 1]][2] for k in range(len(p)))
        out.append(p if area < 0 else p[::-1])
    return out


def split_tiles(verts, polys, home):
    """{(x, z) tile index: (tile-local vertices, polygons)}: every polygon clipped to the 256 m tiles it crosses, so
    each tile's navmesh stays inside its own +-128 m square like vanilla's (no vanilla navmesh has a vertex outside
    it). Cut points are computed from each edge in a fixed direction, so neighbours share them exactly."""
    hx, hz = home
    out = defaultdict(lambda: ([], [], {}))

    def cut(a, b, axis, value):
        a, b = sorted((a, b))
        t = (value - a[axis]) / (b[axis] - a[axis])
        return tuple(a[k] + t * (b[k] - a[k]) if k != axis else value for k in range(3))

    def clip(points, axis, value, keep_below):
        res = []
        for i in range(len(points)):
            cur, prev = points[i], points[i - 1]
            cin = cur[axis] <= value if keep_below else cur[axis] >= value
            pin = prev[axis] <= value if keep_below else prev[axis] >= value
            if cin != pin:
                res.append(cut(prev, cur, axis, value))
            if cin:
                res.append(cur)
        return res

    for p in polys:
        pts = [tuple(verts[i]) for i in p]
        txs = range(int(math.floor((min(q[0] for q in pts) + 128) / TILE)), int(math.floor((max(q[0] for q in pts) + 128) / TILE)) + 1)
        tzs = range(int(math.floor((min(q[2] for q in pts) + 128) / TILE)), int(math.floor((max(q[2] for q in pts) + 128) / TILE)) + 1)
        for dx in txs:
            for dz in tzs:
                piece = pts
                for axis, d in ((0, dx), (2, dz)):
                    piece = clip(piece, axis, d * TILE - 128, False) if piece else piece
                    piece = clip(piece, axis, d * TILE + 128, True) if piece else piece
                local = []
                for q in piece:
                    q = (q[0] - dx * TILE, q[1], q[2] - dz * TILE)
                    if not local or max(abs(q[k] - local[-1][k]) for k in range(3)) > 1e-4:
                        local.append(q)
                while len(local) > 1 and max(abs(local[0][k] - local[-1][k]) for k in range(3)) <= 1e-4:
                    local.pop()
                if len(local) < 3:
                    continue
                area = abs(sum(local[k][2] * local[k - 1][0] - local[k][0] * local[k - 1][2] for k in range(len(local)))) / 2
                if area < 1e-3:
                    continue
                tv, tp, index = out[(hx + dx, hz + dz)]
                face = []
                for q in local:
                    key = tuple(round(c, 4) for c in q)
                    if key not in index:
                        index[key] = len(tv)
                        tv.append(list(q))
                    face.append(index[key])
                tp.append(face)
    return {tile: (tv, tp) for tile, (tv, tp, _) in out.items()}


class Boundary:
    """Boundary segments bucketed on a 4 m x/z grid, for clearance queries."""

    def __init__(self, segments, cell=4.0):
        self.cell, self.grid = cell, defaultdict(list)
        for a, b in segments:
            for gx in range(int(math.floor(min(a[0], b[0]) / cell)), int(math.floor(max(a[0], b[0]) / cell)) + 1):
                for gz in range(int(math.floor(min(a[2], b[2]) / cell)), int(math.floor(max(a[2], b[2]) / cell)) + 1):
                    self.grid[(gx, gz)].append((a, b))

    def distance(self, p, cap):
        r = int(math.ceil(cap / self.cell))
        gx, gz = int(math.floor(p[0] / self.cell)), int(math.floor(p[2] / self.cell))
        best = cap
        for dx in range(-r, r + 1):
            for dz in range(-r, r + 1):
                for a, b in self.grid.get((gx + dx, gz + dz), ()):
                    if abs((a[1] + b[1]) / 2 - p[1]) > 2.5:
                        continue
                    ex, ez = b[0] - a[0], b[2] - a[2]
                    t = ((p[0] - a[0]) * ex + (p[2] - a[2]) * ez) / ((ex * ex + ez * ez) or 1.0)
                    t = min(1.0, max(0.0, t))
                    best = min(best, math.hypot(a[0] + t * ex - p[0], a[2] + t * ez - p[2]))
        return best


def clearance_cache(template, verts, polys, edges):
    """A fully calculated hkaiNavMeshClearanceCache record for the mesh, from `template` (a vanilla one)."""
    ceiling, to_int = template["clearanceCeiling"], template["clearanceRealToIntMultiplier"]
    q = lambda d: min(254, int(d * to_int))  # noqa: E731
    segments = [(verts[e["a"]], verts[e["b"]]) for e in edges if e["oppositeEdge"] == BOUNDARY]
    boundary = Boundary(segments)
    on_boundary = {i for e in edges if e["oppositeEdge"] == BOUNDARY for i in (e["a"], e["b"])}
    vertex = [0 if i in on_boundary else q(boundary.distance(v, ceiling)) for i, v in enumerate(verts)]
    mcp = []
    for e in edges:
        if e["oppositeEdge"] == BOUNDARY:
            mcp.append((0, 0))
            continue
        a, b = verts[e["a"]], verts[e["b"]]
        best = (-1.0, 0.0)
        for s in range(9):
            t = s / 8
            p = [a[k] + t * (b[k] - a[k]) for k in range(3)]
            d = boundary.distance(p, ceiling)
            if d > best[0] + 1e-6:
                best = (d, t)
        mcp.append((round(best[1] * 253), q(best[0])))
    offsets, pairs, start = [], [], 0
    for p in polys:
        offsets.append(len(pairs))
        trav = [start + k for k in range(len(p)) if edges[start + k]["oppositeEdge"] != BOUNDARY]
        for i in range(len(trav)):
            for j in range(i + 1, len(trav)):
                pairs.append(min(mcp[trav[i]][1], mcp[trav[j]][1]))
        pairs.append(255)
        start += len(p)
    cache = Record(template.type, template)
    mcp_type = template["mcpData"].type
    u8, u32 = template["vertexClearances"].type, template["faceOffsets"].type
    cache["faceOffsets"] = Array(u32, offsets)
    cache["edgePairClearances"] = Array(u8, pairs)
    cache["unusedEdgePairElements"] = 0
    cache["mcpData"] = Array(mcp_type, [Record(mcp_type, interpolant=i, clearance=c) for i, c in mcp])
    cache["vertexClearances"] = Array(u8, vertex)
    cache["uncalculatedFacesLowerBound"] = len(polys)
    return cache


def aabb_tree(template_tree, verts, polys):
    """hkcdStaticTree::Aabb6BytesTree over the faces: binary BVH, node, left subtree, right subtree."""
    boxes = []
    for p in polys:
        pts = [verts[i] for i in p]
        boxes.append(([min(q[k] for q in pts) for k in range(3)], [max(q[k] for q in pts) for k in range(3)]))
    centre = [[(b[0][k] + b[1][k]) / 2 for k in range(3)] for b in boxes]
    node_type = template_tree["nodes"].type
    nodes = []

    def build(items, box, parent_lo, parent_hi, is_root=False):
        index = len(nodes)
        xyz = [0, 0, 0] if is_root else [hkx_collision._compress_dim(box[0][k], box[1][k], parent_lo[k], parent_hi[k])
                                          for k in range(3)]
        rec = Record(node_type, xyz=xyz, hiData=0, loData=0)
        nodes.append(rec)
        lo, hi = hkx_collision._child_box(xyz, parent_lo, parent_hi)
        if len(items) == 1:
            rec["hiData"], rec["loData"] = (items[0] >> 16) & 0x7F, items[0] & 0xFFFF
            return
        left, right = hkx_collision._split(items, centre)
        build(left, hkx_collision._union([boxes[i] for i in left]), lo, hi)
        offset = (len(nodes) - index) // 2
        rec["hiData"], rec["loData"] = 0x80 | (offset >> 16) & 0x7F, offset & 0xFFFF
        build(right, hkx_collision._union([boxes[i] for i in right]), lo, hi)

    domain = hkx_collision._union(boxes)
    sys.setrecursionlimit(max(10000, sys.getrecursionlimit()))
    build(list(range(len(polys))), domain, domain[0], domain[1], is_root=True)
    tree = Record(template_tree.type, template_tree)
    tree["nodes"] = Array(node_type, nodes)
    tree["domain"] = Record(template_tree["domain"].type, min=domain[0] + [0.0], max=domain[1] + [0.0])
    return tree, boxes


def _topology(polys, face_t, edge_t, f0=0, e0=0, v0=0):
    """Face and edge records for the polygons, numbered from face f0, edge e0, vertex v0."""
    owner = _edge_map(polys)
    starts, n = [], 0
    for p in polys:
        starts.append(n)
        n += len(p)
    faces, edges = [], []
    for f, p in enumerate(polys):
        faces.append(Record(face_t, startEdgeIndex=e0 + starts[f], startUserEdgeIndex=-1, numEdges=len(p),
                            numUserEdges=0, clusterIndex=-1, padding=0xCDCD))
        for k in range(len(p)):
            a, b = p[k], p[(k + 1) % len(p)]
            opp = owner.get((b, a))
            edges.append(Record(edge_t, a=v0 + a, b=v0 + b,
                                oppositeEdge=e0 + starts[opp[0]] + opp[1] if opp else BOUNDARY,
                                oppositeFace=f0 + opp[0] if opp else BOUNDARY, flags=EDGE_ORIGINAL, paddingByte=0,
                                userEdgeCost=0.0))
    return faces, edges


def build_hkx(template_bytes, verts, polys):
    """Navmesh hkx bytes: the vanilla file's objects with the mesh, cache and tree replaced. Every replaced mesh
    crashed the game on load (2026-10-23/24); use append_hkx."""
    tag, root = hkx_tag.read(template_bytes)
    nm = root["namedVariants"][0]["variant"]
    face_t, edge_t, vec_t = nm["faces"].type, nm["edges"].type, nm["vertices"].type
    faces, edges = _topology(polys, face_t, edge_t)
    nm["faces"] = Array(face_t, faces)
    nm["edges"] = Array(edge_t, edges)
    nm["vertices"] = Array(vec_t, [[v[0], v[1], v[2], 0.0] for v in verts])
    nm["streamingSets"] = Array(nm["streamingSets"].type)
    nm["faceData"] = Array(nm["faceData"].type, [0] * len(faces))
    nm["edgeData"] = Array(nm["edgeData"].type, [0] * len(edges))
    # vanilla: x/z = +-(384 + r) (the tile and its neighbours), y = vertex range +- r, w = -+r; r = 2.4 (n), 8 (o)
    r = -nm["aabb"]["min"][3] or 2.4
    ys = [v[1] for v in verts]
    nm["aabb"] = Record(nm["aabb"].type, min=[-384 - r, min(ys) - r, -384 - r, -r], max=[384 + r, max(ys) + r, 384 + r, r])
    seeding = nm["clearanceCacheSeedingDataSet"]
    for data in seeding["cacheDatas"]:
        data["initialCache"] = clearance_cache(data["initialCache"], verts, polys, edges)
    mediator = root["namedVariants"][1]["variant"]
    impl = mediator["tree"]["treePtr"]
    impl["tree"], _ = aabb_tree(impl["tree"], verts, polys)
    mediator["navMesh"] = nm
    return hkx_tag.write(tag, root, share_strings=True)


def append_hkx(template_bytes, verts, polys):
    """(hkx bytes, vanilla face count): the vanilla navmesh kept record for record, with our polygons appended as
    faces F0.., edges E0.., vertices V0.. Something outside the hkx refers to vanilla face indices: a mesh that
    renumbers them crashes the game on load, this one loads and enemies walk it (L6, 2026-10-25). Only the counts,
    the clearance cache (vanilla's + ours), the y extent of the aabb and the search tree (rebuilt over all faces)
    change. faceData/edgeData of the new faces are 0."""
    tag, root = hkx_tag.read(template_bytes)
    nm = root["namedVariants"][0]["variant"]
    f0, e0, v0 = len(nm["faces"]), len(nm["edges"]), len(nm["vertices"])
    face_t, edge_t, vec_t = nm["faces"].type, nm["edges"].type, nm["vertices"].type
    faces, edges = _topology(polys, face_t, edge_t, f0, e0, v0)
    local = [dict(a=e["a"] - v0, b=e["b"] - v0, oppositeEdge=e["oppositeEdge"] - e0 if e["oppositeEdge"] != BOUNDARY
                  else BOUNDARY) for e in edges]
    nm["faces"] = Array(face_t, list(nm["faces"]) + faces)
    nm["edges"] = Array(edge_t, list(nm["edges"]) + edges)
    nm["vertices"] = Array(vec_t, list(nm["vertices"]) + [[v[0], v[1], v[2], 0.0] for v in verts])
    nm["faceData"] = Array(nm["faceData"].type, list(nm["faceData"]) + [0] * len(faces))
    nm["edgeData"] = Array(nm["edgeData"].type, list(nm["edgeData"]) + [0] * len(edges))
    lo, hi, ys = nm["aabb"]["min"], nm["aabb"]["max"], [v[1] for v in verts]
    r = abs(lo[3]) or 2.4
    nm["aabb"] = Record(nm["aabb"].type, min=[lo[0], min(lo[1], min(ys) - r), lo[2], lo[3]],
                        max=[hi[0], max(hi[1], max(ys) + r), hi[2], hi[3]])
    for data in nm["clearanceCacheSeedingDataSet"]["cacheDatas"]:
        old = data["initialCache"]
        mine = clearance_cache(old, verts, polys, local)
        cache = Record(old.type, old)
        base = len(old["edgePairClearances"])
        for key, extra in (("faceOffsets", [base + o for o in mine["faceOffsets"]]),
                           ("edgePairClearances", list(mine["edgePairClearances"])),
                           ("mcpData", list(mine["mcpData"])), ("vertexClearances", list(mine["vertexClearances"]))):
            cache[key] = Array(old[key].type, list(old[key]) + extra)
        cache["uncalculatedFacesLowerBound"] = f0 + len(polys)
        data["initialCache"] = cache
    all_e = nm["edges"]
    all_v = [list(v[:3]) for v in nm["vertices"]]
    all_p = [[all_e[k]["a"] for k in range(f["startEdgeIndex"], f["startEdgeIndex"] + f["numEdges"])] for f in nm["faces"]]
    mediator = root["namedVariants"][1]["variant"]
    impl = mediator["tree"]["treePtr"]
    impl["tree"], _ = aabb_tree(impl["tree"], all_v, all_p)
    mediator["navMesh"] = nm
    return hkx_tag.write(tag, root, share_strings=True), f0


def check_hkx(data):
    """Re-read a written navmesh and check edges, opposites and that the tree boxes hold every face once."""
    _, root = hkx_tag.read(data)
    nm = root["namedVariants"][0]["variant"]
    F, E, V = nm["faces"], nm["edges"], nm["vertices"]
    for f, face in enumerate(F):
        for k in range(face["startEdgeIndex"], face["startEdgeIndex"] + face["numEdges"]):
            e = E[k]
            if e["oppositeEdge"] != BOUNDARY:
                o = E[e["oppositeEdge"]]
                assert (o["a"], o["b"]) == (e["b"], e["a"]) and E[o["oppositeEdge"]] is e, f"face {f} edge {k}"
                g = F[e["oppositeFace"]]
                assert g["startEdgeIndex"] <= e["oppositeEdge"] < g["startEdgeIndex"] + g["numEdges"]
        a, b, c = (V[E[face["startEdgeIndex"] + i]["a"]] for i in range(3))
        assert _normal([a, b, c])[1] > -1e-6, f"face {f} winding"
    tree = root["namedVariants"][1]["variant"]["tree"]["treePtr"]["tree"]
    leaves = []

    def walk(i, lo, hi):
        n = tree["nodes"][i]
        lo, hi = hkx_collision._child_box(n["xyz"], lo, hi)
        data_ = (n["hiData"] & 0x7F) << 16 | n["loData"]
        if n["hiData"] & 0x80:
            walk(i + 1, lo, hi)
            walk(i + data_ * 2, lo, hi)
        else:
            leaves.append((data_, lo, hi))

    walk(0, tree["domain"]["min"], tree["domain"]["max"])
    assert sorted(x[0] for x in leaves) == list(range(len(F))), "tree leaves"
    for f, lo, hi in leaves:
        face = F[f]
        pts = [V[E[k]["a"]] for k in range(face["startEdgeIndex"], face["startEdgeIndex"] + face["numEdges"])]
        assert hkx_collision._contains(lo, hi, pts, 1e-3), f"tree box of face {f}"
    return len(F), len(E), len(V)


def patch_nva(nva, tile_model, counts):
    """NVA bytes with the tile's n/o navmesh entries' face counts set (`counts` = {1: n faces, 3: o faces}) and the
    running face offsets redone. Entries: 0x90 bytes from section offset +12; +0x30 name id, +0x34 model id,
    +0x38 first face, +0x40 face count."""
    out = bytearray(nva)
    assert out[:4] == b"NVMA"
    version, size, count = struct.unpack_from("<III", out, 0x34)
    assert version == 4
    running = 0
    for i in range(count):
        e = 0x34 + 12 + i * 0x90
        name, model = struct.unpack_from("<II", out, e + 0x30)
        kind = int(str(name)[0])
        faces = counts.get(kind, 0) if model == tile_model else struct.unpack_from("<I", out, e + 0x40)[0]
        struct.pack_into("<I", out, e + 0x38, running)
        struct.pack_into("<I", out, e + 0x40, faces)
        running += faces
    return bytes(out)


def preview(path, verts, polys, seed_points):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.collections import PolyCollection
    fig, ax = plt.subplots(figsize=(12, 12))
    pc = PolyCollection([[(verts[i][0], verts[i][2]) for i in p] for p in polys],
                        array=[sum(verts[i][1] for i in p) / len(p) for p in polys], cmap="viridis",
                        edgecolor="k", linewidth=0.2)
    ax.add_collection(pc)
    plt.colorbar(pc, label="height (m)")
    for name, (x, _, z) in seed_points:
        ax.plot(x, z, "r*", ms=10)
        ax.annotate(name, (x, z), fontsize=6)
    ax.autoscale()
    ax.set_aspect("equal")
    ax.set_xlabel("x (tile-local m)")
    ax.set_ylabel("z")
    plt.savefig(path, dpi=80)


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("map", help="our m60_42_36_00.msb.dcx")
    ap.add_argument("vanilla_m60", help="the game's map/m60 folder (vanilla <tile>.nvmhktbnd.dcx and .nva.dcx)")
    ap.add_argument("out_dir", help="gets m60_XX_ZZ_00/<tile>.nvmhktbnd.dcx + .nva.dcx for every tile the mesh touches")
    ap.add_argument("--asset", action="append", required=True, help="PART_NAME=collision.geomhkxbnd.dcx")
    ap.add_argument("--recast", default=str(Path(__file__).parent / "recast_bake"))
    ap.add_argument("--preview")
    ap.add_argument("--home", default="42,36", help="tile the map and enemies belong to")
    ap.add_argument("--append", action="store_true",
                    help="keep each vanilla navmesh and append our faces (the only variant that loads in game)")
    ap.add_argument("--tiles", nargs="+", help="only write these tiles, e.g. 42,36 41,36")
    args = ap.parse_args()
    home = tuple(int(x) for x in args.home.split(","))
    only = {tuple(int(x) for x in t.split(",")) for t in args.tiles} if args.tiles else None

    map_raw = load_dcx(args.map)
    tris = placed_triangles(map_raw, [a.split("=", 1) for a in args.asset])
    seed_points = seeds(map_raw)
    meshes = {}
    for kind, agent in (("n", AGENT), ("o", AGENT_LARGE)):
        verts, polys = bake(tris, args.recast, agent)
        print(f"{kind}: Recast {len(verts)} vertices, {len(polys)} polygons")
        verts, polys = keep_connected(verts, polys, seed_points)
        polys = orient(verts, polys)
        print(f"{kind}: kept {len(polys)} polygons connected to the enemies/spawns")
        if args.preview and kind == "n":
            preview(args.preview, verts, polys, seed_points)
        meshes[kind] = split_tiles(verts, polys, home)

    for kind in meshes:  # drop slivers (a few faces cut off at a tile corner) rather than edit a tile for them
        for tile in [t for t, (_, tp) in meshes[kind].items() if len(tp) < MIN_TILE_FACES]:
            print(f"{kind}: dropping {len(meshes[kind][tile][1])} sliver face(s) in tile {tile}")
            del meshes[kind][tile]
    for tile in sorted(set(meshes["n"]) | set(meshes["o"])):
        if only is not None and tile not in only:
            print(f"skipping tile {tile} (n {len(meshes['n'].get(tile, ([], []))[1])} faces)")
            continue
        name = f"m60_{tile[0]:02d}_{tile[1]:02d}_00"
        model = int(f"{tile[0]:02d}{tile[1]:02d}00")
        bnd = load_dcx(Path(args.vanilla_m60, name, f"{name}.nvmhktbnd.dcx"))
        new, counts = {}, {}
        for _, entry, data in bnd4_entries(bnd):
            base = entry.split("\\")[-1]
            kind = base[0]
            if kind in "no" and base.endswith(f"_{model}.hkx"):
                verts, polys = meshes[kind].get(tile, ([], []))
                assert polys, f"{name}: no {kind} faces (tiny sliver?); not handled"
                if args.append:
                    out, f0 = append_hkx(data, verts, polys)
                    print(f"{name} {base}: {f0} vanilla + {len(polys)} faces, {len(out)} bytes")
                else:
                    out, f0 = build_hkx(data, verts, polys), 0
                    print(f"{name} {base}: {check_hkx(out)} faces/edges/vertices, {len(out)} bytes")
                new[base] = out
                counts[1 if kind == "n" else 3] = f0 + len(polys)
        assert len(new) == 2, f"{name}: expected the n and o navmeshes, got {list(new)}"
        folder = Path(args.out_dir, name)
        folder.mkdir(parents=True, exist_ok=True)
        (folder / f"{name}.nvmhktbnd.dcx").write_bytes(dflt_dcx(bnd4_replace(bnd, new)))
        nva = load_dcx(Path(args.vanilla_m60, name, f"{name}.nva.dcx"))
        (folder / f"{name}.nva.dcx").write_bytes(dflt_dcx(patch_nva(nva, model, counts)))
        print(f"Wrote {folder}")


if __name__ == "__main__":
    main()
