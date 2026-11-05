"""Build a single-mesh Elden Ring FLVER straight from a Wizard101 OBJ, without Blender (stdlib only).

The result has the same layout as our Soulstruct export of the HD street (copied from that FLVER, `template`):
36-byte vertices with a float3 position, byte4 bone indices (all 0), a byte4 normal whose fourth byte is the bone
(0, see flver_fix.set_normal_w), a byte4 tangent, an empty second tangent, a white byte4 colour and a short2 UV
(units of 1/2048, each triangle moved by whole tiles to fit). Every triangle has its own three vertices; its face
normal is -(b - a) x (c - a) of the FLVER corners, and FLVER positions are the OBJ's with z negated (same winding, V
flipped). Without `normals` it is flat shaded like that export (each triangle lit as its own facet); `corner_normals`
gives smooth per-corner normals, W101's own where nif_normals.py wrote them.
flver_textures.split_materials then turns it into one mesh per texture.
"""

import math
import struct

import flver_fix as ff

UV_SCALE = 2048
POSITION, BONE_INDICES, NORMAL, UV, TANGENT, COLOR = 0, 2, 3, 5, 6, 10


def parse_obj(path):
    """(positions, uvs, [(material, ((v, vt), (v, vt), (v, vt)))]) of an OBJ, fans triangulated, 0-based indices."""
    positions, uvs, faces, material = [], [], [], None
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                positions.append(tuple(float(x) for x in line.split()[1:4]))
            elif line.startswith("vt "):
                uvs.append(tuple(float(x) for x in line.split()[1:3]))
            elif line.startswith("usemtl"):
                material = line.split(None, 1)[1].strip()
            elif line.startswith("f "):
                corners = []
                for c in line.split()[1:]:
                    v, vt = (c.split("/") + [""])[:2]
                    corners.append((int(v) - 1, int(vt) - 1 if vt else None))
                for k in range(1, len(corners) - 1):
                    faces.append((material, (corners[0], corners[k], corners[k + 1])))
    return positions, uvs, faces


def _byte4(x, y, z, w):
    return bytes(max(0, min(254, round(c * 127 + 127))) for c in (x, y, z)) + bytes([w])


def _short(value):
    return max(-32768, min(32767, round(value * UV_SCALE)))


def _normalize(v):
    length = math.sqrt(sum(c * c for c in v))
    return tuple(c / length for c in v) if length > 1e-12 else None


def _members(template):
    _, counts = ff._counts(template)
    starts, _ = ff._section_starts(counts)
    count, _, _, offset = struct.unpack_from("<4i", template, starts["layout"])
    return [struct.unpack_from("<5i", template, offset + 20 * k)[1:4] for k in range(count)]


def _encoder(template):
    """Function (position, normal, tangent, uv, flip) -> vertex bytes in the template's layout."""
    members = _members(template)
    size = max(offset for offset, _, _ in members) + 4
    tangents = [offset for offset, _, semantic in members if semantic == TANGENT]
    expected = {POSITION: (2,), BONE_INDICES: (0x10, 0x11, 0x13), NORMAL: (0x10, 0x11, 0x13),
                TANGENT: (0x10, 0x11, 0x13), COLOR: (0x10, 0x11, 0x13), UV: (0x15,)}
    for offset, data_type, semantic in members:
        assert data_type in expected.get(semantic, ()), f"unsupported layout member {data_type:#x}/{semantic}"

    def encode(position, normal, tangent, uv, flip):
        vertex = bytearray(size)
        for offset, _, semantic in members:
            if semantic == POSITION:
                struct.pack_into("<3f", vertex, offset, *position)
            elif semantic == NORMAL:
                vertex[offset:offset + 4] = _byte4(*normal, 0)
            elif semantic == TANGENT:
                vertex[offset:offset + 4] = _byte4(*tangent, 254 if flip else 0) if offset == tangents[0] else b"\x7f" * 4
            elif semantic == COLOR:
                vertex[offset:offset + 4] = b"\xff" * 4
            elif semantic == UV:
                struct.pack_into("<2h", vertex, offset, _short(uv[0]), _short(uv[1]))
        return bytes(vertex)

    return encode, size


NORMALS_MAGIC = b"W101NRM1"  # written by nif_normals.py
NORMAL_MATCH = 0.002  # m: an OBJ corner takes W101's normals from NIF vertices this close (Blender's float32 export)
CREASE = 80  # degrees: without W101's normals, faces meeting at a sharper angle keep a hard edge


def read_normals(path):
    """[(position, unit normal)] in OBJ space from nif_normals.py's file (stdlib only, runs inside Blender)."""
    with open(path, "rb") as f:
        data = f.read()
    assert data[:8] == NORMALS_MAGIC, f"{path} is not a W101 normals file"
    count = struct.unpack_from("<I", data, 8)[0]
    return [(r[:3], r[3:]) for r in struct.iter_unpack("<6f", data[12:12 + 24 * count])]


def _face_normal(points):
    a, b, c = points
    e1 = [b[k] - a[k] for k in range(3)]
    e2 = [c[k] - a[k] for k in range(3)]
    # FLVER = OBJ with z negated (a mirror), hence the minus; the length is twice the area (area weighting)
    return (-(e1[1] * e2[2] - e1[2] * e2[1]), -(e1[2] * e2[0] - e1[0] * e2[2]), -(e1[0] * e2[1] - e1[1] * e2[0]))


def _dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def corner_normals(positions, faces, reference=None, crease=CREASE):
    """Per face of parse_obj's `faces`, its three corner normals in FLVER space (None for a degenerate face), so the
    FLVER shades smoothly like W101 instead of one flat normal per triangle.

    With `reference` (read_normals of the zone's NIF), each corner takes the W101 normal of the NIF vertex at its
    position (within NORMAL_MATCH); where W101 splits normals at a hard edge, several vertices share the position and
    the one facing most like this triangle wins. Corners W101 has nothing for (your own edits) get the area-weighted
    average of the triangles around that point whose normals are within `crease` degrees of this one's.
    Returns (normals, stats) with stats = {"w101": corners matched, "smoothed": corners averaged}."""
    flver = [(p[0], p[1], -p[2]) for p in positions]
    face_n = [_face_normal([flver[v] for v, _ in corners]) for _, corners in faces]
    unit = [_normalize(n) for n in face_n]

    grid = {}
    for p, n in reference or ():
        key = tuple(math.floor(c / NORMAL_MATCH) for c in p)
        grid.setdefault(key, []).append((p, (n[0], n[1], -n[2])))
    near = {}

    def w101(v):
        if v not in near:
            p = positions[v]
            kx, ky, kz = (math.floor(c / NORMAL_MATCH) for c in p)
            found = []
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    for dz in (-1, 0, 1):
                        for q, n in grid.get((kx + dx, ky + dy, kz + dz), ()):
                            if (q[0] - p[0]) ** 2 + (q[1] - p[1]) ** 2 + (q[2] - p[2]) ** 2 <= NORMAL_MATCH ** 2:
                                found.append(n)
            near[v] = found
        return near[v]

    around = {}
    for f, (_, corners) in enumerate(faces):
        if unit[f] is not None:
            for v, _ in corners:
                around.setdefault(tuple(round(c, 3) for c in positions[v]), []).append(f)
    cos_crease = math.cos(math.radians(crease))
    out, stats = [], {"w101": 0, "smoothed": 0}
    for f, (_, corners) in enumerate(faces):
        if unit[f] is None:
            out.append(None)
            continue
        normals = []
        for v, _ in corners:
            candidates = w101(v) if grid else ()
            if candidates:
                normals.append(max(candidates, key=lambda n: _dot(n, unit[f])))
                stats["w101"] += 1
                continue
            total = [0.0, 0.0, 0.0]
            for g in around[tuple(round(c, 3) for c in positions[v])]:
                if _dot(unit[g], unit[f]) >= cos_crease:
                    total = [total[k] + face_n[g][k] for k in range(3)]
            normals.append(_normalize(total) or unit[f])
            stats["smoothed"] += 1
        out.append(normals)
    return out, stats


def build_single_mesh(template, positions, uvs, faces, fixed_normal=None, vertex_normal=None, normals=None):
    """(FLVER bytes, [material of each triangle]) for OBJ data from parse_obj. `template` is a single-mesh FLVER
    in the layout above; its header, sections, material and bone are kept, its vertices and indices replaced.
    `fixed_normal` gives every vertex that normal instead of its face's; `vertex_normal(point)` (FLVER space, unit
    length) gives each vertex its own, so a curved surface shades smoothly instead of showing its triangles;
    `normals` (from `corner_normals`, one entry per face) gives each corner its own."""
    encode, vertex_size = _encoder(template)
    vertices, materials = bytearray(), []
    lo, hi = [math.inf] * 3, [-math.inf] * 3
    for f, (material, corners) in enumerate(faces):
        points = [(positions[v][0], positions[v][1], -positions[v][2]) for v, _ in corners]
        a, b, c = points
        e1 = [b[k] - a[k] for k in range(3)]
        e2 = [c[k] - a[k] for k in range(3)]
        cross = (e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0])
        normal = _normalize(tuple(-x for x in cross))
        if normal is None:
            continue  # degenerate triangle
        normal = fixed_normal or normal
        tex = [(uvs[t][0], 1.0 - uvs[t][1]) if t is not None else (0.0, 0.0) for _, t in corners]
        # Shorts only reach +-16: move each triangle's UVs by whole tiles, which repeating textures don't notice.
        shift = [math.floor(sum(uv[k] for uv in tex) / 3) for k in range(2)]
        tex = [(u - shift[0], v - shift[1]) for u, v in tex]
        du1, dv1 = tex[1][0] - tex[0][0], tex[1][1] - tex[0][1]
        du2, dv2 = tex[2][0] - tex[0][0], tex[2][1] - tex[0][1]
        det = du1 * dv2 - du2 * dv1
        # Soulstruct's export stores dP/dv here (checked against the HD street), so do the same.
        tangent = _normalize(tuple((du1 * e2[k] - du2 * e1[k]) / det for k in range(3))) if abs(det) > 1e-12 else None
        tangent = tangent or _normalize((normal[2], 0.0, -normal[0])) or (1.0, 0.0, 0.0)
        along_u = tuple((dv2 * e1[k] - dv1 * e2[k]) for k in range(3)) if abs(det) > 1e-12 else (0.0, 0.0, 0.0)
        side = (normal[1] * tangent[2] - normal[2] * tangent[1], normal[2] * tangent[0] - normal[0] * tangent[2],
                normal[0] * tangent[1] - normal[1] * tangent[0])
        handedness = sum(a * b for a, b in zip(side, along_u)) * (1 if det > 0 else -1)
        corner = normals[f] if normals and normals[f] else [vertex_normal(p) if vertex_normal else normal
                                                                for p in points]
        for point, uv, n in zip(points, tex, corner):
            # keep the tangent at right angles to a corner normal that isn't the face's
            t = _normalize(tuple(tangent[k] - n[k] * _dot(n, tangent) for k in range(3))) or tangent
            vertices += encode(point, n, t, uv, handedness > 0)
            lo = [min(lo[k], point[k]) for k in range(3)]
            hi = [max(hi[k], point[k]) for k in range(3)]
        materials.append(material)

    _, counts = ff._counts(template)
    assert counts["mesh"] == 1 and counts["face_set"] == 1 and counts["vertex_buffer"] == 1, counts
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", template, 0x0C)[0]
    vertex_count = len(vertices) // vertex_size
    indices = struct.pack(f"<{vertex_count}I", *range(vertex_count))
    out = bytearray(template[:vertex_data])
    struct.pack_into("<3f3f", out, 0x28, *lo, *hi)
    struct.pack_into("<2i", out, 0x40, vertex_count // 3, vertex_count // 3)
    face_set = starts["face_set"]
    struct.pack_into("<3i", out, face_set + 0x08, vertex_count, 0, len(indices))
    struct.pack_into("<i", out, face_set + 0x18, 32)
    index_end = len(indices) + (-len(indices) % 16)
    struct.pack_into("<8i", out, starts["vertex_buffer"], 0, 0, vertex_size, vertex_count, 0, 0, len(vertices),
                     index_end)
    data = indices + bytes(index_end - len(indices)) + bytes(vertices)
    struct.pack_into("<i", out, 0x10, len(data))
    return bytes(out) + data, materials
