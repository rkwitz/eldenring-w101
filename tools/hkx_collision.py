"""Build Elden Ring asset collision (an fsnpCustomParamCompressedMeshShape in a Havok 2018 tagfile) from triangles.

Elden Ring's asset collision HKX holds one static body whose shape is FromSoftware's fsnpCustomParamCompressedMeshShape:
a Havok hknpCompressedMeshShape plus a little per-triangle data. `build_hkx` takes a vanilla asset HKX as the template
(its TYPE section, material, motion and body settings are kept) and replaces the shape with one made from the given
triangles. `decode` reads a shape back into triangles and checks its trees, for validation.

Compressed mesh layout (as read from vanilla files, and Smithbox's DS3 hknpCollisionMeshBuilder):
- The mesh is split into sections of at most 127 primitives (triangles) and 255 vertices. A primitive's four byte
  indices point into its section's vertex list: first the section's packed vertices (11/11/10 bits, decoded with the
  section's codecParms offset and scale), then its shared vertices (21/21/22 bits over the whole mesh domain, through
  sharedVerticesIndex). A triangle repeats its third index.
- The mesh tree (Aabb5BytesCodec nodes) is a BVH over sections; each section has its own BVH (Aabb4BytesCodec) over its
  primitives. Child boxes are coded relative to the parent box in 16 square-root steps per side.
- A shape key is section << 8 | primitive << 1 (| 1 for a quad's second triangle); bitsPerKey is the bit length of the
  largest key. hknpShape-style keys store the key in the top bits with the rest set (`_full_key`).
- The SIMD tree is a 4-wide BVH over primitives whose leaves hold shape keys; node 0 is an empty sentinel.
"""

import math
import struct

import hkx_tag

FLT_MAX = 3.402820018375656e38
EMPTY_KEY = 0xFFFFFFFF
MAX_SECTION_PRIMITIVES = 127
MAX_SECTION_VERTICES = 255
CONVEX_RADIUS = 0.015  # vanilla aeg030_518_l
PACKED_STEP = 0.004  # largest packed-vertex quantization step (m); vertices of wider sections become shared


# --- decoding

def _shared_vertex(value, domain):
    lo, hi = domain["min"], domain["max"]
    parts = (value & 0x1FFFFF, (value >> 21) & 0x1FFFFF, (value >> 42) & 0x3FFFFF)
    scales = ((hi[0] - lo[0]) / 0x1FFFFF, (hi[1] - lo[1]) / 0x1FFFFF, (hi[2] - lo[2]) / 0x3FFFFF)
    return tuple(parts[k] * scales[k] + lo[k] for k in range(3))


def _packed_vertex(value, parms):
    parts = (value & 0x7FF, (value >> 11) & 0x7FF, (value >> 22) & 0x3FF)
    return tuple(parts[k] * parms[3 + k] + parms[k] for k in range(3))


def _child_box(xyz, lo, hi):
    """Decode one compressed AABB node against its parent box."""
    out_lo, out_hi = [], []
    for k in range(3):
        span = hi[k] - lo[k]
        out_lo.append(lo[k] + (xyz[k] >> 4) ** 2 / 226.0 * span)
        out_hi.append(hi[k] - (xyz[k] & 0x0F) ** 2 / 226.0 * span)
    return out_lo, out_hi


def _walk5(nodes, index, lo, hi, leaves):
    node = nodes[index]
    lo, hi = _child_box(node["xyz"], lo, hi)
    data = (node["hiData"] & 0x7F) << 8 | node["loData"]
    if node["hiData"] & 0x80:
        _walk5(nodes, index + 1, lo, hi, leaves)
        _walk5(nodes, index + data * 2, lo, hi, leaves)
    else:
        leaves.append((data, lo, hi))


def _walk4(nodes, index, lo, hi, leaves):
    node = nodes[index]
    lo, hi = _child_box(node["xyz"], lo, hi)
    if node["data"] & 1:
        _walk4(nodes, index + 1, lo, hi, leaves)
        _walk4(nodes, index + (node["data"] & 0xFE), lo, hi, leaves)
    else:
        leaves.append((node["data"] >> 1, lo, hi))


def _contains(lo, hi, points, slack):
    return all(lo[k] - slack <= p[k] <= hi[k] + slack for p in points for k in range(3))


def decode(shape, check=True):
    """{shape key: (a, b, c)} triangles of a compressed mesh shape. With `check`, assert that every tree box holds what
    it points at (allowing for the codecs' rounding) and that the SIMD tree lists every key."""
    tree = shape["data"]["meshTree"]
    domain = tree["domain"]
    shared = [_shared_vertex(v, domain) for v in tree["sharedVertices"]]
    triangles = {}
    for s, section in enumerate(tree["sections"]):
        parms = section["codecParms"]
        packed = [_packed_vertex(v, parms) for v in
                  tree["packedVertices"][section["firstPackedVertexIndex"]:][:section["numPackedVertices"]]]
        local = packed
        if s + 1 < len(tree["sections"]):
            shared_count = tree["sections"][s + 1]["firstSharedVertexIndex"] - section["firstSharedVertexIndex"]
        else:
            shared_count = len(tree["sharedVerticesIndex"]) - section["firstSharedVertexIndex"]
        local = packed + [shared[i] for i in
                          tree["sharedVerticesIndex"][section["firstSharedVertexIndex"]:][:shared_count]]
        primitives = tree["primitives"][section["firstPrimitiveIndex"]:][:section["numPrimitives"]]
        for p, primitive in enumerate(primitives):
            a, b, c, d = primitive["indices"]
            triangles[s << 8 | p << 1] = (local[a], local[b], local[c])
            if d != c:
                triangles[s << 8 | p << 1 | 1] = (local[a], local[c], local[d])
        if check:
            leaves = []
            _walk4(section["nodes"], 0, section["domain"]["min"], section["domain"]["max"], leaves)
            assert sorted(leaf[0] for leaf in leaves) == list(range(len(primitives))), f"section {s} tree"
            slack = 1e-3 + 1e-5 * max(section["domain"]["max"][k] - section["domain"]["min"][k] for k in range(3))
            for p, lo, hi in leaves:
                assert _contains(lo, hi, triangles[s << 8 | p << 1], slack), f"section {s} primitive {p} box"
    if check:
        leaves = []
        _walk5(tree["nodes"], 0, domain["min"], domain["max"], leaves)
        assert sorted(leaf[0] for leaf in leaves) == list(range(len(tree["sections"]))), "mesh tree"
        for s, lo, hi in leaves:
            section = tree["sections"][s]
            assert section["leafIndex"] >= 0
            assert _contains(lo, hi, [section["domain"]["min"][:3], section["domain"]["max"][:3]], 1e-3), f"section {s}"
        keys = []
        for node in shape["data"]["simdTree"]["nodes"][1:]:
            if node["isLeaf"]:
                for i, key in enumerate(node["data"]):
                    if key != EMPTY_KEY:
                        keys.append(key)
                        box = ([node["lx"][i], node["ly"][i], node["lz"][i]],
                               [node["hx"][i], node["hy"][i], node["hz"][i]])
                        assert _contains(*box, triangles[key], 1e-4), f"simd key {key}"
        assert sorted(keys) == sorted(k for k in triangles if not k & 1), "simd tree keys"
    return triangles


# --- building

def _bounds(points):
    return [min(p[k] for p in points) for k in range(3)], [max(p[k] for p in points) for k in range(3)]


def _grow(box, by):
    return [x - by for x in box[0]], [x + by for x in box[1]]


def _union(boxes):
    return [min(b[0][k] for b in boxes) for k in range(3)], [max(b[1][k] for b in boxes) for k in range(3)]


class _Node:
    """Binary BVH node: `items` (indices into the caller's list) at a leaf, `left`/`right` otherwise."""

    def __init__(self, box, items=None, left=None, right=None):
        self.box, self.items, self.left, self.right = box, items, left, right


def _split(items, centre):
    """Halve `items` at the median of the longest axis of their centres."""
    lo, hi = _bounds([centre[i] for i in items])
    axis = max(range(3), key=lambda k: hi[k] - lo[k])
    items = sorted(items, key=lambda i: centre[i][axis])
    half = len(items) // 2
    return items[:half], items[half:]


def _binary_tree(items, boxes, centre, is_leaf):
    if is_leaf(items):
        return _Node(_union([boxes[i] for i in items]), items=items)
    left, right = _split(items, centre)
    left, right = _binary_tree(left, boxes, centre, is_leaf), _binary_tree(right, boxes, centre, is_leaf)
    return _Node(_union([left.box, right.box]), left=left, right=right)


def _compress_dim(lo, hi, parent_lo, parent_hi):
    span = parent_hi - parent_lo
    if span <= 0:
        return 0
    scale = 226.0 / span
    eps = span * 1e-6
    a = math.sqrt(max((lo - parent_lo - eps) * scale, 0))
    b = math.sqrt(max((parent_hi - hi - eps) * scale, 0))
    return min(15, int(math.floor(a))) << 4 | min(15, int(math.floor(b)))


def _compressed_tree(root, leaf_data, axis5):
    """Flatten a binary tree into Aabb5BytesCodec (axis5) or Aabb4BytesCodec node dicts, coding each box against its
    decoded parent box. `leaf_data(node)` is a leaf's index. Returns (nodes, {leaf data: node index})."""
    nodes, where = [], {}

    def add(node, parent_lo, parent_hi, is_root=False):
        index = len(nodes)
        xyz = [0, 0, 0] if is_root else [_compress_dim(node.box[0][k], node.box[1][k], parent_lo[k], parent_hi[k])
                                          for k in range(3)]
        entry = {"xyz": xyz}
        nodes.append(entry)
        lo, hi = _child_box(xyz, parent_lo, parent_hi)
        if node.items is not None:
            data = leaf_data(node)
            where[data] = index
            if axis5:
                entry["hiData"], entry["loData"] = (data >> 8) & 0x7F, data & 0xFF
            else:
                entry["data"] = data * 2
        else:
            add(node.left, lo, hi)
            offset = len(nodes) - index
            if axis5:
                entry["hiData"], entry["loData"] = 0x80 | ((offset // 2) >> 8) & 0x7F, (offset // 2) & 0xFF
            else:
                assert offset < 256
                entry["data"] = offset | 1
            add(node.right, lo, hi)

    add(root, root.box[0], root.box[1], is_root=True)
    return nodes, where


def _simd_nodes(keys, boxes):
    """4-wide BVH over shape keys: list of (is_leaf, [(data, box)]) with node 0 the empty sentinel. Groups are cut in
    multiples of the child subtree's capacity so leaves stay full."""
    nodes = [(False, [])]
    centre = {k: [(boxes[k][0][a] + boxes[k][1][a]) / 2 for a in range(3)] for k in keys}

    def cut(group, size):
        """`group` sorted along its longest axis and cut after `size` (a multiple of the capacity)."""
        lo, hi = _bounds([centre[k] for k in group])
        axis = max(range(3), key=lambda a: hi[a] - lo[a])
        group = sorted(group, key=lambda k: centre[k][axis])
        return group[:size], group[size:]

    def build(group):
        index = len(nodes)
        if len(group) <= 4:
            nodes.append((True, [(k, boxes[k]) for k in group]))
            return index, _union([boxes[k] for k in group])
        nodes.append(None)
        cap = 4
        while cap * 4 < len(group):
            cap *= 4
        chunks = -(-len(group) // cap)  # 2 to 4 children
        halves = cut(group, -(-chunks // 2) * cap) if chunks > 2 else (group,)
        quarters = []
        for half in halves:
            quarters += cut(half, cap) if len(half) > cap else (half,)
        children = [build(q) for q in quarters if q]
        nodes[index] = (False, [(child, box) for child, box in children])
        return index, _union([box for _, box in children])

    build(sorted(keys))
    out = []
    for is_leaf, slots in nodes:
        empty = EMPTY_KEY if is_leaf else 0
        slots = slots + [(empty, ([FLT_MAX] * 3, [-FLT_MAX] * 3))] * (4 - len(slots))
        out.append((is_leaf, slots))
    return out


def _vec(xyz, w=0.0):
    return list(xyz) + [w]


def build_shape(template, triangles, vertex_indices=None, radius=CONVEX_RADIUS, step=PACKED_STEP):
    """Replace the compressed mesh in vanilla shape `template` (a decoded fsnpCustomParamCompressedMeshShape) with one
    made of `triangles` [(a, b, c)], keeping its types and other settings. `vertex_indices` [(i, j, k)] go into the
    per-triangle data (defaults to positions' order). Returns the shape and the decoded positions by shape key."""
    tag_types = {k: v.type for k, v in template["data"]["meshTree"].items() if isinstance(v, hkx_tag.Array)}
    # positions -> vertex ids
    ids, points = {}, []
    tris = []
    for tri in triangles:
        key = []
        for p in tri:
            p = tuple(round(x, 5) for x in p)
            if p not in ids:
                ids[p] = len(points)
                points.append(p)
            key.append(ids[p])
        tris.append(tuple(key))
    if vertex_indices is None:
        vertex_indices = tris
    # drop degenerate triangles (repeated or collinear corners)
    keep = []
    for i, (a, b, c) in enumerate(tris):
        u = [points[b][k] - points[a][k] for k in range(3)]
        v = [points[c][k] - points[a][k] for k in range(3)]
        n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
        if len({a, b, c}) == 3 and sum(x * x for x in n) > 1e-12:
            keep.append(i)
    tris = [tris[i] for i in keep]
    vertex_indices = [vertex_indices[i] for i in keep]

    # sections: binary BVH over triangles, cut where a subtree fits a section
    boxes = [_grow(_bounds([points[i] for i in t]), radius) for t in tris]
    centre = [[sum(points[i][k] for i in t) / 3 for k in range(3)] for t in tris]

    def fits(items):
        return len(items) <= MAX_SECTION_PRIMITIVES and len({v for i in items for v in tris[i]}) <= MAX_SECTION_VERTICES

    root = _binary_tree(list(range(len(tris))), boxes, centre, fits)
    leaves = []

    def collect(node):
        if node.items is not None:
            leaves.append(node)
        else:
            collect(node.left)
            collect(node.right)

    collect(root)
    owners = {}
    for s, leaf in enumerate(leaves):
        for v in {v for i in leaf.items for v in tris[i]}:
            owners.setdefault(v, []).append(s)
    shared = {v for v, o in owners.items() if len(o) > 1}
    # a section's own vertices are packed in 11/11/10 bits over their bounds; share the outliers of wide sections
    tops = (0x7FF, 0x7FF, 0x3FF)
    for leaf in leaves:
        own = [v for v in {v for i in leaf.items for v in tris[i]} if v not in shared]
        while own:
            lo, hi = _bounds([points[v] for v in own])
            steps = [(hi[k] - lo[k]) / tops[k] for k in range(3)]
            axis = max(range(3), key=lambda k: steps[k])
            if steps[axis] <= step:
                break
            middle = sorted(points[v][axis] for v in own)[len(own) // 2]
            far = max(own, key=lambda v: abs(points[v][axis] - middle))
            own.remove(far)
            shared.add(far)
    shared_ids = sorted(shared)
    assert len(shared_ids) < 0x10000, f"{len(shared_ids)} shared vertices; sharedVerticesIndex is 16-bit"
    shared_index = {v: i for i, v in enumerate(shared_ids)}

    tree = template["data"]["meshTree"]
    lo, hi = _bounds(points)
    domain = _grow((lo, hi), radius)
    domain_rec = hkx_tag.Record(tree["domain"].type, min=_vec(domain[0], 1.0), max=_vec(domain[1], 1.0))
    shared_values = []
    for v in shared_ids:
        p, value = points[v], 0
        for k, (bits, shift) in enumerate(((21, 0), (21, 21), (22, 42))):
            top = (1 << bits) - 1
            span = domain[1][k] - domain[0][k]
            value |= min(top, max(0, round((p[k] - domain[0][k]) / span * top))) << shift
        shared_values.append(value)
    decoded_shared = [_shared_vertex(x, domain_rec) for x in shared_values]

    section_type = tree["sections"].type
    section_template = tree["sections"][0]
    sections, primitives, packed_values, shared_list, runs = [], [], [], [], []
    decoded = {}
    for s, leaf in enumerate(leaves):
        used = sorted({v for i in leaf.items for v in tris[i]})
        own = [v for v in used if v not in shared_index]
        common = [v for v in used if v in shared_index]
        plo, phi = _bounds([points[v] for v in own]) if own else ([0.0] * 3, [0.0] * 3)
        scale = [(phi[k] - plo[k]) / top if phi[k] > plo[k] else 0.0 for k, top in enumerate((0x7FF, 0x7FF, 0x3FF))]
        parms = plo + scale
        local, positions = {}, []
        first_packed = len(packed_values)
        for v in own:
            value = 0
            for k, (top, shift) in enumerate(((0x7FF, 0), (0x7FF, 11), (0x3FF, 22))):
                q = round((points[v][k] - plo[k]) / scale[k]) if scale[k] else 0
                value |= min(top, max(0, q)) << shift
            packed_values.append(value)
            local[v] = len(positions)
            positions.append(_packed_vertex(value, parms))
        first_shared = len(shared_list)
        for v in common:
            shared_list.append(shared_index[v])
            local[v] = len(positions)
            positions.append(decoded_shared[shared_index[v]])
        # section BVH over its triangles (by decoded positions), primitives numbered in tree order
        items = leaf.items
        sboxes = {i: _grow(_bounds([positions[local[v]] for v in tris[i]]), radius) for i in items}
        scentre = {i: [sum(positions[local[v]][k] for v in tris[i]) / 3 for k in range(3)] for i in items}
        sroot = _binary_tree(items, sboxes, scentre, lambda g: len(g) == 1)
        order = []
        snodes, _ = _compressed_tree(sroot, lambda n: order.append(n.items[0]) or len(order) - 1, axis5=False)
        first_primitive = len(primitives)
        for p, i in enumerate(order):
            a, b, c = (local[v] for v in tris[i])
            primitives.append([a, b, c, c])
            decoded[i] = (s << 8 | p << 1, tuple(positions[local[v]] for v in tris[i]))
        leaf.box = sroot.box
        rec = hkx_tag.Record(section_type, section_template)
        rec.update(
            nodes=hkx_tag.Array(section_template["nodes"].type,
                                [hkx_tag.Record(section_template["nodes"].type, n) for n in snodes]),
            domain=hkx_tag.Record(section_template["domain"].type, min=_vec(sroot.box[0], 1.0),
                                  max=_vec(sroot.box[1], 1.0)),
            codecParms=parms, firstPackedVertexIndex=first_packed, firstSharedVertexIndex=first_shared,
            firstPrimitiveIndex=first_primitive, firstDataRunIndex=s, numPackedVertices=len(own),
            numPrimitives=len(order), numDataRuns=1, page=0, layerData=0, flags=0)
        sections.append(rec)
        runs.append(hkx_tag.Record(tree["primitiveDataRuns"].type, value=0xFFFF, index=0, count=len(order)))

    # mesh tree over sections (boxes from the section trees, so they hold the decoded triangles)
    def refit(node):
        if node.items is None:
            refit(node.left)
            refit(node.right)
            node.box = _union([node.left.box, node.right.box])

    refit(root)
    root.box = domain  # the game decodes the root against the mesh domain
    leaf_number = {id(leaf): s for s, leaf in enumerate(leaves)}
    mnodes, where = _compressed_tree(root, lambda n: leaf_number[id(n)], axis5=True)
    for s, rec in enumerate(sections):
        rec["leafIndex"] = where[s]
    node_type = tree["nodes"].type

    keys = [decoded[i][0] for i in range(len(tris))]
    max_key = max(keys)
    bits = max_key.bit_length()
    tree_rec = hkx_tag.Record(tree.type, tree)
    tree_rec.update(
        nodes=hkx_tag.Array(node_type, [hkx_tag.Record(node_type, n) for n in mnodes]),
        domain=domain_rec, numPrimitiveKeys=len(primitives), bitsPerKey=bits, maxKeyValue=max_key,
        primitiveStoresIsFlatConvex=0,
        sections=hkx_tag.Array(section_type, sections),
        primitives=hkx_tag.Array(tree["primitives"].type,
                                 [hkx_tag.Record(tree["primitives"].type, indices=p) for p in primitives]),
        sharedVerticesIndex=hkx_tag.Array(tag_types["sharedVerticesIndex"], shared_list),
        packedVertices=hkx_tag.Array(tag_types["packedVertices"], packed_values),
        sharedVertices=hkx_tag.Array(tag_types["sharedVertices"], shared_values),
        primitiveDataRuns=hkx_tag.Array(tree["primitiveDataRuns"].type, runs))
    assert tree_rec.keys() == tree.keys()

    # SIMD tree over shape keys with tight boxes
    simd = template["data"]["simdTree"]
    simd_type = simd["nodes"].type
    key_boxes = {key: _bounds(tri) for key, tri in decoded.values()}
    simd_nodes = []
    for is_leaf, slots in _simd_nodes(list(key_boxes), key_boxes):
        node = hkx_tag.Record(simd_type, isLeaf=is_leaf, isActive=False)
        for k, axis in enumerate("xyz"):
            node["l" + axis] = [box[0][k] for _, box in slots]
            node["h" + axis] = [box[1][k] for _, box in slots]
        node["data"] = [d for d, _ in slots]
        simd_nodes.append(hkx_tag.Record(simd_type, {m.name: node[m.name] for m in simd_type.all_members()}))
    simd_rec = hkx_tag.Record(simd.type, simd)
    simd_rec["nodes"] = hkx_tag.Array(simd_type, simd_nodes)

    data = hkx_tag.Record(template["data"].type, template["data"])
    data.update(meshTree=tree_rec, simdTree=simd_rec)
    data["hasSimdTree"] = True

    shape = hkx_tag.Record(template.type, template)
    interior = hkx_tag.Record(template["triangleIsInterior"].type, template["triangleIsInterior"])
    storage = hkx_tag.Record(interior["storage"].type, interior["storage"])
    storage["words"] = hkx_tag.Array(storage["words"].type, [0] * ((max_key + 32) // 32))
    storage["numBits"] = max_key + 1
    interior["storage"] = storage
    param = hkx_tag.Record(template["pParam"].type, template["pParam"])
    tri_type = param["triangleDataArray"].type
    param["triangleDataArray"] = hkx_tag.Array(tri_type, [
        hkx_tag.Record(tri_type, primitiveDataIndex=0, triangleDataIndex=i, vertexIndexA=a, vertexIndexB=b,
                       vertexIndexC=c) for i, (a, b, c) in enumerate(vertex_indices)])
    shape.update(numShapeKeyBits=bits, convexRadius=radius, data=data, triangleIsInterior=interior,
                 numTriangles=0, numConvexShapes=0, pParam=param,
                 triangleIndexToShapeKey=hkx_tag.Array(template["triangleIndexToShapeKey"].type,
                                                       [_full_key(k, bits) for k in keys]))
    return shape, {key: tri for key, tri in decoded.values()}


def _full_key(key, bits):
    return (key << (32 - bits)) | ((1 << (32 - bits)) - 1)


def body_of(root):
    return root["namedVariants"][0]["variant"]["systemDatas"][0]["bodyCinfos"][0]


def build_hkx(template_bytes, triangles, name=None):
    """HKX bytes of the template file with its body's shape replaced by `triangles`, the body at the origin."""
    tag, root = hkx_tag.read(template_bytes)
    body = body_of(root)
    shape, decoded = build_shape(body["shape"], triangles)
    body["shape"] = shape
    body["position"] = [0.0, 0.0, 0.0, 1.0]
    body["orientation"] = [0.0, 0.0, 0.0, 1.0]
    body["dragProperties"] = None
    if name:
        body["name"] = name
    return hkx_tag.write(tag, root), decoded


def replace_binder_entry(bnd, data):
    """BND4 bytes with its only entry's data replaced. Soulstruct's binder writer starts entry data at an 8-byte
    boundary, but Havok needs the tagfile 16-byte aligned (the game crashes on load otherwise), so we keep the game's
    own header, which already places the data at a 16-byte offset, and only change the sizes."""
    assert bnd[:4] == b"BND4" and struct.unpack_from("<i", bnd, 0x0C)[0] == 1, "expected a BND4 with one entry"
    header = 0x40  # the entry header follows the 0x40-byte binder header
    offset = struct.unpack_from("<I", bnd, header + 0x18)[0]
    assert offset % 16 == 0
    out = bytearray(bnd[:offset])
    struct.pack_into("<qq", out, header + 0x08, len(data), len(data))
    return bytes(out) + data


def main():
    """python hkx_collision.py template.hkx model.flver out.hkx: collision for every triangle of the FLVER."""
    import sys
    from pathlib import Path

    import flver_floor

    template, flver, target = (Path(a) for a in sys.argv[1:4])
    triangles = list(flver_floor._triangles(flver.read_bytes()))
    data, _ = build_hkx(template.read_bytes(), triangles)
    _, root = hkx_tag.read(data)
    decode(body_of(root)["shape"])
    target.write_bytes(data)
    print(f"Wrote {target}: {len(triangles)} triangles, {len(data)} bytes")


if __name__ == "__main__":
    main()
