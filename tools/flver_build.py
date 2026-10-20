"""Build a single-mesh Elden Ring FLVER straight from a Wizard101 OBJ, without Blender (stdlib only).

The result has the same layout as our Soulstruct export of the HD street (copied from that FLVER, `template`):
36-byte vertices with a float3 position, byte4 bone indices (all 0), a byte4 normal whose fourth byte is the bone
(0, see flver_fix.set_normal_w), a byte4 tangent, an empty second tangent, a white byte4 colour and a short2 UV
(units of 1/2048, each triangle moved by whole tiles to fit). Like that export it is flat shaded: every triangle has its own three vertices, its normal is
-(b - a) x (c - a) of the FLVER corners, and FLVER positions are the OBJ's with z negated (same winding, V flipped).
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


def build_single_mesh(template, positions, uvs, faces, fixed_normal=None):
    """(FLVER bytes, [material of each triangle]) for OBJ data from parse_obj. `template` is a single-mesh FLVER
    in the layout above; its header, sections, material and bone are kept, its vertices and indices replaced.
    `fixed_normal` gives every vertex that normal instead of its face's (the sky dome is lit evenly that way)."""
    encode, vertex_size = _encoder(template)
    vertices, materials = bytearray(), []
    lo, hi = [math.inf] * 3, [-math.inf] * 3
    for material, corners in faces:
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
        for point, uv in zip(points, tex):
            vertices += encode(point, normal, tangent, uv, handedness > 0)
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
