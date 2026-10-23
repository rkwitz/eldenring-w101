"""Check Elden Ring navmesh files (<tile>.nvmhktbnd.dcx + <tile>.nva.dcx) for anything that breaks vanilla's rules,
and compare them with the vanilla files field by field. Written separately from er_navmesh.py so it does not share
its mistakes: faces, edges, vertices and tree nodes are decoded here straight from the tagfile DATA bytes (struct
layouts from the TYPE section's member offsets); hkx_tag is only used for the type table and item list.

Checks: DCX/BND4 layout, entry names/ids/flags vs vanilla, TAG0 sections (SDKV, TYPE identical to vanilla), mesh
topology (edge chains, mutual opposite edge/face links, no repeated directed edge, indices in range), geometry
(convex, wound like vanilla, no degenerate face or edge, vertices inside the tile), AABB, clearance cache sizes and
per-face block rule, query tree (every face once, boxes contain faces, offsets in range), NVA counts.

Usage: python navmesh_check.py OUR_TILE_FOLDER VANILLA_TILE_FOLDER      (folders holding <tile>.nvmhktbnd.dcx/.nva.dcx)
Prints problems; exit code 1 if any.
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import dcx_krak  # noqa: E402
import hkx_tag  # noqa: E402

NONE = 0xFFFFFFFF
problems = []


def bad(msg):
    problems.append(msg)
    print("  PROBLEM:", msg)


def bnd4(raw):
    count, = struct.unpack_from("<i", raw, 0x0C)
    entries = []
    for i in range(count):
        h = 0x40 + i * 0x24
        flags, minus1, csize, usize, offset, eid, name_at = struct.unpack_from("<IiqqIII", raw, h)
        end = name_at
        while raw[end:end + 2] != b"\0\0":
            end += 2
        entries.append(dict(flags=flags, minus1=minus1, csize=csize, usize=usize, offset=offset, id=eid,
                            name=raw[name_at:end].decode("utf-16-le"), data=raw[offset:offset + csize]))
    return entries


def members(tag, type_name):
    t = tag.by_name[type_name]
    return {m.name: m.offset for m in t.all_members()}


def items(tag, type_name):
    return [(off, cnt) for t, fl, off, cnt in tag.items if t is not None and t.name == type_name]


def mesh(data):
    """Faces, edges, vertices and tree nodes of the first hkaiNavMesh, from raw DATA bytes."""
    tag = hkx_tag.TagFile(data)
    d = tag.data
    navs = items(tag, "hkaiNavMesh")
    assert len(navs) == 1, "expected one hkaiNavMesh object"
    nav_off = navs[0][0]
    mem = members(tag, "hkaiNavMesh")

    def array(field, elem_size, fmt=None):
        index, = struct.unpack_from("<Q", d, nav_off + mem[field])
        if index == 0:
            return []
        _, _, off, cnt = tag.items[index]
        return [struct.unpack_from(fmt, d, off + i * elem_size) for i in range(cnt)] if fmt else (off, cnt)

    fm, em = members(tag, "hkaiNavMesh::Face"), members(tag, "hkaiNavMesh::Edge")
    assert (fm["startEdgeIndex"], fm["numEdges"], em["a"], em["oppositeEdge"]) == (0, 8, 0, 8)
    faces = array("faces", 16, "<iihhhH")
    edges = array("edges", 20, "<IIIIBBH")
    verts = array("vertices", 16, "<4f")
    aabb = struct.unpack_from("<8f", d, nav_off + mem["aabb"])
    node_items = items(tag, "hkcdCompressedAabbCodecs::Aabb6BytesCodec")
    nodes = []
    for off, cnt in node_items:
        nodes = [struct.unpack_from("<3BBH", d, off + 6 * i) for i in range(cnt)]
    dom = None
    for t, fl, off, cnt in tag.items:
        if t is not None and t.name == "hkcdStaticAabbTree::Impl":
            tm = members(tag, "hkcdStaticAabbTree::Impl")
            tree_off = off + tm["tree"]
            sub = members(tag, "hkcdStaticTree::Aabb6BytesTree")
            dom = struct.unpack_from("<8f", d, tree_off + sub["domain"])
    _, root = hkx_tag.read(data)
    nav = root["namedVariants"][0]["variant"]
    caches = [c["initialCache"] for c in nav["clearanceCacheSeedingDataSet"]["cacheDatas"]]
    return tag, faces, edges, verts, aabb, nodes, dom, caches, nav


def child_box(xyz, lo, hi):
    out_lo, out_hi = [], []
    for k in range(3):
        span = hi[k] - lo[k]
        out_lo.append(lo[k] + (xyz[k] >> 4) ** 2 / 226.0 * span)
        out_hi.append(hi[k] - (xyz[k] & 15) ** 2 / 226.0 * span)
    return out_lo, out_hi


def check_mesh(name, data, vdata):
    tag, faces, edges, verts, aabb, nodes, dom, caches, nav = mesh(data)
    vtag = hkx_tag.TagFile(vdata)
    if tag.sdk != vtag.sdk or tag.type_section != vtag.type_section:
        bad(f"{name}: SDK or TYPE section differs from vanilla")
    F, E, V = len(faces), len(edges), len(verts)
    print(f"  {name}: {F} faces, {E} edges, {V} vertices, {len(nodes)} tree nodes")
    if F == 0:
        return 0
    # topology
    seen_directed = {}
    end = 0
    for f, (start, ustart, n, nu, cluster, pad) in enumerate(faces):
        if start != end and f > 0 and nu == 0:
            pass
        if n < 3 or start < 0 or start + n > E:
            bad(f"{name}: face {f} edge range {start}+{n}")
            continue
        end = start + n
        ids = [edges[k][0] for k in range(start, start + n)]
        for i, k in enumerate(range(start, start + n)):
            a, b, oe, of, fl, _, _ = edges[k]
            if a >= V or b >= V:
                bad(f"{name}: edge {k} vertex out of range")
            if b != edges[start + (i + 1) % n][0]:
                bad(f"{name}: face {f} edge chain broken at {k}")
            if a == b:
                bad(f"{name}: edge {k} degenerate")
            if (a, b) in seen_directed:
                bad(f"{name}: directed edge {a}->{b} used twice (faces {seen_directed[(a, b)]}, {f})")
            seen_directed[(a, b)] = f
            if (oe == NONE) != (of == NONE):
                bad(f"{name}: edge {k} half-open opposite")
            if oe != NONE:
                if oe >= E or of >= F:
                    bad(f"{name}: edge {k} opposite out of range")
                    continue
                o = edges[oe]
                os_, _, on = faces[of][0], 0, faces[of][2]
                if not os_ <= oe < os_ + on:
                    bad(f"{name}: edge {k} opposite edge {oe} not in face {of}")
                if (o[0], o[1]) != (b, a) or o[2] != k or o[3] != f:
                    bad(f"{name}: edge {k} opposite link not mutual")
        if len(set(ids)) != len(ids):
            bad(f"{name}: face {f} repeats a vertex")
        # geometry: convex, wound with cross(b-a, c-a).y > 0, not degenerate
        pts = [verts[i] for i in ids]
        area2 = sum(pts[i - 1][2] * pts[i][0] - pts[i - 1][0] * pts[i][2] for i in range(n))  # 2x area, > 0 when wound
        for i in range(n):
            p, q, r = pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
            cross = (q[2] - p[2]) * (r[0] - p[0]) - (q[0] - p[0]) * (r[2] - p[2])
            if cross < -1e-4:
                bad(f"{name}: face {f} not convex/wound (corner {i}, {cross:.5f})")
                break
            if sum((q[k] - p[k]) ** 2 for k in range(3)) ** 0.5 < 1e-3:
                bad(f"{name}: face {f} edge shorter than 1 mm")
        if area2 < 1e-4:
            bad(f"{name}: face {f} area {area2 / 2:.6f}")
    # vertices / aabb
    for i, v in enumerate(verts):
        if abs(v[0]) > 129 or abs(v[2]) > 129:
            bad(f"{name}: vertex {i} outside the tile ({v[0]:.1f}, {v[2]:.1f})")
            break
        if not all(aabb[k] <= v[k] <= aabb[4 + k] for k in range(3)):
            bad(f"{name}: vertex {i} outside the mesh aabb")
            break
    # clearance caches
    for c in caches:
        if (len(c["faceOffsets"]), len(c["mcpData"]), len(c["vertexClearances"]), c["uncalculatedFacesLowerBound"]) != (F, E, V, F):
            bad(f"{name}: clearance cache sizes {len(c['faceOffsets'])}/{len(c['mcpData'])}/{len(c['vertexClearances'])}")
        offs = list(c["faceOffsets"]) + [len(c["edgePairClearances"])]
        for f, (start, ustart, n, nu, _, _) in enumerate(faces):
            k = sum(1 for e in range(start, start + n) if edges[e][2] != NONE) + nu
            if offs[f + 1] - offs[f] != k * (k - 1) // 2 + 1 or c["edgePairClearances"][offs[f + 1] - 1] != 255:
                bad(f"{name}: clearance block of face {f}")
                break
    # tree
    if len(nodes) != 2 * F - 1:
        bad(f"{name}: {len(nodes)} tree nodes for {F} faces")
    leaves = []
    stack = [(0, dom[:3], dom[4:7])]
    while stack:
        i, lo, hi = stack.pop()
        if i >= len(nodes):
            bad(f"{name}: tree node {i} out of range")
            break
        x, y, z, hid, lod = nodes[i]
        lo, hi = child_box((x, y, z), lo, hi)
        data_ = (hid & 0x7F) << 16 | lod
        if hid & 0x80:
            stack += [(i + 1, lo, hi), (i + 2 * data_, lo, hi)]
        else:
            leaves.append((data_, lo, hi))
    if sorted(x[0] for x in leaves) != list(range(F)):
        bad(f"{name}: tree leaves are not each face once")
    for f, lo, hi in leaves:
        if f >= F:
            continue
        start, _, n = faces[f][:3]
        for e in range(start, start + n):
            v = verts[edges[e][0]]
            if not all(lo[k] - 1e-3 <= v[k] <= hi[k] + 1e-3 for k in range(3)):
                bad(f"{name}: tree box of face {f} misses a vertex")
                break
    return F


def nva_entries(raw):
    version, size, count = struct.unpack_from("<III", raw, 0x34)
    return [struct.unpack_from("<12f9I", raw, 0x40 + i * 0x90)[12:] for i in range(count)], raw[0x34 + size:]


def main():
    ours, vanilla = Path(sys.argv[1]), Path(sys.argv[2])
    tile = vanilla.name
    print(f"== {tile}")
    o_raw = (ours / f"{tile}.nvmhktbnd.dcx").read_bytes() if (ours / f"{tile}.nvmhktbnd.dcx").exists() else None
    v_raw = (vanilla / f"{tile}.nvmhktbnd.dcx").read_bytes()
    counts = {}
    if o_raw is None:
        print("  no nvmhktbnd (vanilla one is used)")
        counts = None
    else:
        print(f"  DCX {o_raw[0x28:0x2C].decode()} (vanilla {v_raw[0x28:0x2C].decode()})")
        ob, vb = dcx_krak.decompress(o_raw), dcx_krak.decompress(v_raw)
        if ob[:0x40] != vb[:0x40]:
            bad("BND4 header differs from vanilla")
        oe, ve = bnd4(ob), bnd4(vb)
        if [(e["flags"], e["minus1"], e["id"], e["name"]) for e in oe] != [(e["flags"], e["minus1"], e["id"], e["name"]) for e in ve]:
            bad("BND4 entry flags/ids/names differ from vanilla")
        first = min(e["offset"] for e in oe)
        if ob[:first][0x40 + 0x24 * len(oe):] != vb[:first][0x40 + 0x24 * len(ve):]:
            bad("BND4 name/hash area differs from vanilla")
        for e, v in zip(oe, ve):
            if e["offset"] % 16 or e["csize"] != e["usize"] or e["offset"] + e["csize"] > len(ob):
                bad(f"entry {e['name']}: offset/size")
            base = e["name"].split("\\")[-1]
            if e["data"] == v["data"]:
                print(f"  {base}: identical to vanilla")
                tag = hkx_tag.TagFile(e["data"])
                cnt = items(tag, "hkaiNavMesh::Face")
                counts[base[0]] = cnt[0][1] if cnt else 0
                continue
            counts[base[0]] = counts.get(base[0], 0) + check_mesh(base, e["data"], v["data"]) if base[0] in "no" else 0
            if base[0] == "q":
                check_mesh(base, e["data"], v["data"])
    on = (ours / f"{tile}.nva.dcx")
    nva = dcx_krak.decompress(on.read_bytes()) if on.exists() else None
    vnva = dcx_krak.decompress((vanilla / f"{tile}.nva.dcx").read_bytes())
    if nva is None:
        print("  no nva (vanilla one is used)")
        return
    oent, otail = nva_entries(nva)
    vent, vtail = nva_entries(vnva)
    if nva[:0x34] != vnva[:0x34][:0x8] + nva[0x8:0xC] + vnva[0xC:0x34] or otail != vtail:
        bad("NVA header/other sections differ from vanilla")
    print(f"  NVA {len(oent)} entries (vanilla {len(vent)}); DCX {on.read_bytes()[0x28:0x2C].decode()}")
    running = 0
    for o, v in zip(oent, vent):
        name, model, first, zero, count = o[:5]
        if (name, model, zero, o[5:]) != (v[0], v[1], v[3], v[5:]):
            bad(f"NVA entry {name}: fields other than first/count differ from vanilla")
        if first != running:
            bad(f"NVA entry {name}: first face {first}, expected {running}")
        running += count
        if counts is not None and model == int(tile[4:6] + tile[7:9] + "00") and str(name)[0] in "13":
            want = counts.get("n" if str(name)[0] == "1" else "o")
            if want is not None and count != want:
                bad(f"NVA entry {name}: count {count}, mesh has {want}")
    if counts is None:
        for o, v in zip(oent, vent):
            if o[4] != v[4]:
                bad(f"NVA entry {o[0]}: count {o[4]} but the vanilla mesh (in use) has {v[4]}")


if __name__ == "__main__":
    main()
    print("OK" if not problems else f"{len(problems)} problem(s)")
    sys.exit(1 if problems else 0)
