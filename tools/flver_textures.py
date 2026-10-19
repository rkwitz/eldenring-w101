"""Give the exported Unicorn Way FLVER one material per Wizard101 texture (stdlib only).

Our export puts the whole street on one grass material. `split_materials` rebuilds the FLVER with one mesh and one
material per texture, copying a plain vanilla aeg030_518 material for each: m60_fort_220 (shader M_AMSN_V, an albedo and a
normal map) for opaque textures, m60_fort_215 (M_AMSN_V__Edge, alpha-tested) for ones with transparency. Their albedo
paths point at our own textures, which tpf.py packs into asset/aet/aet030/aet030_518.tpf.dcx. Both vanilla materials
use the same vertex layout as our converted export, so vertices are copied as they are.

`obj_face_materials` recovers which texture each FLVER triangle had from the OBJ the street was imported from: the
export keeps every triangle, with the OBJ's z axis negated.
"""

import struct
from pathlib import PureWindowsPath

import flver_fix as ff

OPAQUE_MATERIAL, ALPHA_MATERIAL = 4, 3  # vanilla aeg030_518 materials m60_fort_220 and m60_fort_215
AET = "AET030_518"  # texture names must start with the TPF's stem: asset/aet/aet030/aet030_518.tpf.dcx
TEXTURE_DIR = r"N:\GR\data\Asset\Environment\texture\AET030"
FLAT_NORMAL = f"{AET}_n"


def texture_name(i):
    return f"{AET}_{i:03d}_a"


def texture_path(name):
    return f"{TEXTURE_DIR}\\{name}.tif"


def parse_obj(path):
    """(positions, [(material, (i, j, k))]) of a triangulated OBJ, with 0-based vertex indices."""
    positions, faces, material = [], [], None
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                positions.append(tuple(float(x) for x in line.split()[1:4]))
            elif line.startswith("usemtl"):
                material = line.split(None, 1)[1].strip()
            elif line.startswith("f "):
                corners = [int(c.split("/")[0]) for c in line.split()[1:]]
                corners = [c - 1 if c > 0 else len(positions) + c for c in corners]
                for k in range(1, len(corners) - 1):
                    faces.append((material, (corners[0], corners[k], corners[k + 1])))
    return positions, faces


def parse_mtl(path):
    """{material: texture file stem} from map_Kd lines."""
    textures, material = {}, None
    with open(path) as f:
        for line in f:
            if line.startswith("newmtl"):
                material = line.split(None, 1)[1].strip()
            elif line.startswith("map_Kd") and material:
                textures[material] = PureWindowsPath(line.split(None, 1)[1].strip()).stem
    return textures


def _key(points, digits):
    return tuple(sorted(tuple(round(c, digits) for c in p) for p in points))


def _mesh_triangles(data):
    """(vertex buffer header, vertex size, indices) of the single mesh of our export."""
    _, counts = ff._counts(data)
    assert counts["mesh"] == 1 and counts["face_set"] == 1 and counts["vertex_buffer"] == 1, counts
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    index_count, index_offset, _, _, index_size = struct.unpack_from("<5i", data, starts["face_set"] + 0x08)
    indices = struct.unpack_from(f"<{index_count}{'I' if index_size == 32 else 'H'}", data,
                                 vertex_data + index_offset)
    return indices


def obj_face_materials(data, obj_positions, obj_faces):
    """Material name of every triangle of our single-mesh FLVER, matched to the OBJ by corner positions."""
    lookups = {}
    for digits in (3, 2):
        table = {}
        for material, corners in obj_faces:
            points = [(p[0], p[1], -p[2]) for p in (obj_positions[c] for c in corners)]
            table.setdefault(_key(points, digits), []).append(material)
        lookups[digits] = table
    positions = list(_vertex_positions(data))
    indices = _mesh_triangles(data)
    materials, missing = [], 0
    for t in range(0, len(indices), 3):
        points = [positions[i] for i in indices[t:t + 3]]
        for digits in (3, 2):
            candidates = lookups[digits].get(_key(points, digits))
            if candidates:
                materials.append(candidates.pop() if len(candidates) > 1 else candidates[0])
                break
        else:
            # Corners moved too far in the export to match (a few dozen of 158k): the export keeps each Blender object's
            # triangles together, so take the material of the triangle before.
            materials.append(materials[-1] if materials else None)
            missing += 1
    return materials, missing


def _vertex_positions(data):
    _, counts = ff._counts(data)
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    _, layout, vertex_size, vertex_count, _, _, _, offset = struct.unpack_from("<8i", data, starts["vertex_buffer"])
    base = vertex_data + offset + ff._position_offset(data, starts, layout)
    for v in range(vertex_count):
        yield struct.unpack_from("<3f", data, base + v * vertex_size)


def _gx_block(data, offset):
    """Bytes of the GX item list at `offset`, up to and including its terminator item (id 0x7FFFFFFF)."""
    pos = offset
    while True:
        item_id, _, length = struct.unpack_from("<iii", data, pos)
        pos += length
        if item_id == 0x7FFFFFFF:
            return data[offset:pos]


def _material_template(vanilla, index):
    _, counts = ff._counts(vanilla)
    starts, _ = ff._section_starts(counts)
    name, mtd, count, first, flags, gx, _, _ = struct.unpack_from("<8i", vanilla, starts["material"] + 32 * index)
    textures = []
    for i in range(first, first + count):
        header = starts["texture"] + 32 * i
        path, kind = struct.unpack_from("<2i", vanilla, header)
        textures.append((ff._utf16(vanilla, kind), vanilla[header + 8:header + 32]))
    return {"mtd": ff._utf16(vanilla, mtd), "flags": flags, "gx": _gx_block(vanilla, gx), "textures": textures}


class _Blob:
    """Bytes with deferred int32 offsets: `ref` reserves a field, `fill` writes it once the target is placed."""

    def __init__(self):
        self.data = bytearray()

    def align(self, n):
        self.data += bytes(-len(self.data) % n)

    def add(self, block, align=1):
        self.align(align)
        at = len(self.data)
        self.data += block
        return at

    def utf16(self, text):
        return self.add(text.encode("utf-16-le") + b"\0\0", 2)

    def set(self, at, fmt, *values):
        struct.pack_into(fmt, self.data, at, *values)


def split_materials(data, vanilla, face_materials, textures, alpha):
    """New FLVER bytes: one mesh/material per texture. `face_materials[t]` names triangle t's texture (an index into
    `textures`, or None to drop it); `alpha` holds the indices of textures needing the alpha-tested material."""
    version, counts = ff._counts(data)
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    vb = struct.unpack_from("<8i", data, starts["vertex_buffer"])
    _, layout_index, vertex_size, vertex_count, _, _, _, vb_offset = vb
    source = data[vertex_data + vb_offset:vertex_data + vb_offset + vertex_size * vertex_count]
    indices = _mesh_triangles(data)
    face_set_template = data[starts["face_set"]:starts["face_set"] + 32]
    mesh_template = data[starts["mesh"]:starts["mesh"] + 48]
    bone = data[starts["bone"]:starts["bone"] + 128]
    bone_name = ff._utf16(data, struct.unpack_from("<i", bone, 0x0C)[0])
    layout = data[starts["layout"]:starts["layout"] + 16]
    member_count, _, _, members_offset = struct.unpack_from("<4i", layout)
    members = data[members_offset:members_offset + 20 * member_count]
    templates = {k: _material_template(vanilla, k) for k in (OPAQUE_MATERIAL, ALPHA_MATERIAL)}

    groups = {}
    for t, texture in enumerate(face_materials):
        if texture is not None:
            groups.setdefault(texture, []).append(t)
    used = sorted(groups)

    # Per mesh: its own vertices (in first-use order) and indices into them.
    meshes = []
    for texture in used:
        remap, vertices, local = {}, bytearray(), []
        for t in groups[texture]:
            for v in indices[3 * t:3 * t + 3]:
                if v not in remap:
                    remap[v] = len(remap)
                    vertices += source[v * vertex_size:(v + 1) * vertex_size]
                local.append(remap[v])
        meshes.append((texture, bytes(vertices), local))

    material_count = len(meshes)
    texture_count = sum(len(templates[ALPHA_MATERIAL if t in alpha else OPAQUE_MATERIAL]["textures"])
                        for t, _, _ in meshes)
    out_counts = {"dummy": 0, "material": material_count, "bone": 1, "mesh": material_count,
                  "face_set": material_count, "vertex_buffer": material_count, "layout": 1, "texture": texture_count}
    out_starts, sections_end = ff._section_starts(out_counts)

    blob = _Blob()
    blob.data += data[:ff.HEADER_SIZE]
    blob.data += bytes(sections_end - ff.HEADER_SIZE)
    blob.set(0x14, "<5i", 0, material_count, 1, material_count, material_count)
    blob.set(0x50, "<3i", material_count, 1, texture_count)
    blob.set(0x40, "<2i", len(indices) // 3, len(indices) // 3)

    # Bone, layout and the shared (empty) bone index list.
    blob.data[out_starts["bone"]:out_starts["bone"] + 128] = bone
    blob.set(out_starts["bone"] + 0x0C, "<i", blob.utf16(bone_name))
    blob.data[out_starts["layout"]:out_starts["layout"] + 16] = layout
    blob.set(out_starts["layout"] + 0x0C, "<i", blob.add(members, 4))
    bone_indices = blob.add(bytes(4), 4)

    texture_index = 0
    gx_offsets = {}
    for m, (texture, vertices, local) in enumerate(meshes):
        kind = ALPHA_MATERIAL if texture in alpha else OPAQUE_MATERIAL
        template = templates[kind]
        material = out_starts["material"] + 32 * m
        if kind not in gx_offsets:
            gx_offsets[kind] = blob.add(template["gx"], 4)
        blob.set(material, "<8i", blob.utf16(texture_name(texture)), blob.utf16(template["mtd"]),
                 len(template["textures"]), texture_index, template["flags"], gx_offsets[kind], m, 0)
        for sampler, rest in template["textures"]:
            header = out_starts["texture"] + 32 * texture_index
            path = texture_path(texture_name(texture)) if "AlbedoMap" in sampler else texture_path(FLAT_NORMAL)
            blob.data[header + 8:header + 32] = rest
            blob.set(header, "<2i", blob.utf16(path), blob.utf16(sampler))
            texture_index += 1

        mesh = out_starts["mesh"] + 48 * m
        blob.data[mesh:mesh + 48] = mesh_template
        bounds = blob.add(bytes(36), 4)
        face_set_index = blob.add(struct.pack("<i", m), 4)
        buffer_index = blob.add(struct.pack("<i", m), 4)
        blob.set(mesh + 4, "<i", m)
        blob.set(mesh + 0x18, "<2i", bounds, bone_indices)
        blob.set(mesh + 0x20, "<2i", 1, face_set_index)
        blob.set(mesh + 0x28, "<2i", 1, buffer_index)
        xyz = [struct.unpack_from("<3f", vertices, v * vertex_size + ff._position_offset(data, starts, layout_index))
               for v in range(len(vertices) // vertex_size)]
        lo = [min(p[k] for p in xyz) for k in range(3)]
        hi = [max(p[k] for p in xyz) for k in range(3)]
        blob.set(bounds, "<9f", *[(h - l) / 2 for l, h in zip(lo, hi)], 0.0, 0.0, 0.0,
                 *[(h + l) / 2 for l, h in zip(lo, hi)])

    # Vertex data: each mesh's indices, then its vertices.
    blob.align(16)
    vertex_start = len(blob.data)
    blob.set(0x0C, "<i", vertex_start)
    for m, (texture, vertices, local) in enumerate(meshes):
        wide = len(vertices) // vertex_size > 0xFFFF
        packed = struct.pack(f"<{len(local)}{'I' if wide else 'H'}", *local)
        index_at = blob.add(packed, 16) - vertex_start
        vertex_at = blob.add(vertices, 16) - vertex_start
        face_set = out_starts["face_set"] + 32 * m
        blob.data[face_set:face_set + 32] = face_set_template
        blob.set(face_set + 0x08, "<3i", len(local), index_at, len(packed))
        blob.set(face_set + 0x18, "<i", 32 if wide else 16)
        blob.set(out_starts["vertex_buffer"] + 32 * m, "<8i", 0, 0, vertex_size, len(vertices) // vertex_size, 0,
                 0, len(vertices), vertex_at)
    blob.align(16)
    blob.set(0x10, "<i", len(blob.data) - vertex_start)
    return ff.add_skeleton_set(bytes(blob.data))
