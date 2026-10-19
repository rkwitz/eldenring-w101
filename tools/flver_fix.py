"""Fix gaps in Elden Ring FLVERs (version 0x2001A) written by Soulstruct for Blender 3.2.1.

1. Skeleton set: Elden Ring FLVERs carry a small table right after the texture headers. Soulstruct leaves it out, so
   SoulsFormats-based tools (Smithbox) fail with "Read Int32: 0xC | Expected: 0x0" in SkeletonSet. We insert one base
   and one full skeleton entry per bone and shift every absolute offset after it.
2. Mesh bounds: each mesh has a 36-byte box stored as (half extents, unknown zero vector, center). Soulstruct writes
   only the older 24-byte "unset" placeholder (FLT_MAX, -FLT_MAX), which the game could cull. We make room for the
   full box and compute it from the vertex positions.
3. Vertex layout (optional): Soulstruct writes its own vertex layout (bone indices, two tangents, vertex color 0), while
   the game's asset for the same material uses a different one, and the game may not draw such a mesh, so
   convert_layout repacks the vertices to the vanilla FLVER's layout.
4. Texture paths (optional): Soulstruct writes bare texture file names where the vanilla FLVER leaves paths empty (the
   material supplies them); copy_texture_paths copies the vanilla paths.
5. Normal W (with a vanilla FLVER): the game reads the normal's fourth byte as the bone a vertex follows. Soulstruct
   writes 127, vanilla assets 0; set_normal_w writes 0.

Usage: python flver_fix.py input.flver [output.flver] [vanilla.flver to copy the vertex layout and texture paths from]
In Blender (Soulstruct installed), fix_binder(path) fixes every FLVER inside a compressed GEOMBND in place.
"""

import struct
import sys

HEADER_SIZE = 0x80
SECTIONS = (("dummy", 0x40), ("material", 0x20), ("bone", 0x80), ("mesh", 0x30), ("face_set", 0x20),
            ("vertex_buffer", 0x20), ("layout", 0x10), ("texture", 0x20))
SIZES = dict(SECTIONS)
POSITION_SEMANTIC, NORMAL_SEMANTIC, FLOAT3_TYPE = 0, 3, 2
BYTE4_TYPES = {0x10, 0x11, 0x13}  # four normalized bytes; SoulsFormats decodes normals and tangents the same in each


def _counts(data):
    version, _, _, dummies, materials, bones, meshes, vertex_buffers = struct.unpack_from("<8i", data, 8)
    face_sets, layouts, textures = struct.unpack_from("<3i", data, 0x50)
    return version, {"dummy": dummies, "material": materials, "bone": bones, "mesh": meshes, "face_set": face_sets,
                     "vertex_buffer": vertex_buffers, "layout": layouts, "texture": textures}


def _section_starts(counts):
    starts, pos = {}, HEADER_SIZE
    for name, size in SECTIONS:
        starts[name] = pos
        pos += counts[name] * size
    return starts, pos


def has_skeleton_set(data):
    _, counts = _counts(data)
    _, end = _section_starts(counts)
    count1, count2, offset1, _ = struct.unpack_from("<hhII", data, end)
    padding = struct.unpack_from("<5i", data, end + 12)
    return (count1, count2) == (counts["bone"], counts["bone"]) and offset1 == end + 0x20 and not any(padding)


def _absolute_offset_fields(counts, starts):
    """Positions of every int32 holding an absolute file offset. Face set and vertex buffer data offsets are relative
    to the vertex data offset, which is shifted itself."""
    yield 0x0C  # vertex data offset
    for i in range(counts["material"]):
        base = starts["material"] + i * SIZES["material"]
        yield from (base, base + 4, base + 0x14)  # name, MTD, GX list
    for i in range(counts["bone"]):
        yield starts["bone"] + i * SIZES["bone"] + 0x0C  # name
    for i in range(counts["mesh"]):
        base = starts["mesh"] + i * SIZES["mesh"]
        yield from (base + 0x18, base + 0x1C, base + 0x24, base + 0x2C)  # bounds, bones, face sets, vertex buffers
    for i in range(counts["layout"]):
        yield starts["layout"] + i * SIZES["layout"] + 0x0C  # members
    for i in range(counts["texture"]):
        base = starts["texture"] + i * SIZES["texture"]
        yield from (base, base + 4)  # path, type


def _insert(data, at, block):
    """Insert `block` at `at`, shifting every absolute offset that points at or past it."""
    _, counts = _counts(data)
    starts, _ = _section_starts(counts)
    out = bytearray(data)
    for pos in _absolute_offset_fields(counts, starts):
        value = struct.unpack_from("<i", data, pos)[0]
        if value >= at:
            struct.pack_into("<i", out, pos, value + len(block))
    return bytes(out[:at] + block + out[at:])


def add_skeleton_set(data):
    version, counts = _counts(data)
    if version < 0x2001A or has_skeleton_set(data):
        return data
    starts, insert_at = _section_starts(counts)
    bones = counts["bone"]

    # Copy each FLVER bone's hierarchy into both skeleton lists.
    entries = bytearray()
    for i in range(bones):
        base = starts["bone"] + i * SIZES["bone"]
        parent, first_child = struct.unpack_from("<hh", data, base + 0x1C)
        next_sibling, previous_sibling = struct.unpack_from("<hh", data, base + 0x2C)
        entries += struct.pack("<4hi4x", parent, first_child, next_sibling, previous_sibling, i)
    table = struct.pack("<hhII20x", bones, bones, insert_at + 0x20, insert_at + 0x20 + len(entries))
    return _insert(data, insert_at, table + entries + entries)


def _position_offset(data, starts, layout_index):
    member_count, _, _, members = struct.unpack_from("<4i", data, starts["layout"] + layout_index * SIZES["layout"])
    for k in range(member_count):
        _, struct_offset, data_type, semantic, _ = struct.unpack_from("<5i", data, members + 20 * k)
        if semantic == POSITION_SEMANTIC and data_type == FLOAT3_TYPE:
            return struct_offset
    return None


def fix_mesh_bounds(data):
    version, counts = _counts(data)
    if version < 0x2001A:
        return data
    starts, _ = _section_starts(counts)
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * SIZES["mesh"]
        bounds_offset = struct.unpack_from("<i", data, mesh + 0x18)[0]
        if not bounds_offset or struct.unpack_from("<f", data, bounds_offset)[0] < 1e30:
            continue  # no bounds, or already set
        # Grow the 24-byte placeholder to 36 bytes (16 inserted to keep the vertex data aligned).
        data = _insert(data, bounds_offset + 24, bytes(16))
        vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
        buffer_count, buffer_indices = struct.unpack_from("<2i", data, mesh + 0x28)
        lo, hi = [float("inf")] * 3, [float("-inf")] * 3
        for buffer_index in struct.unpack_from(f"<{buffer_count}i", data, buffer_indices):
            vb = starts["vertex_buffer"] + buffer_index * SIZES["vertex_buffer"]
            _, layout, vertex_size, vertex_count, _, _, length, offset = struct.unpack_from("<8i", data, vb)
            position = _position_offset(data, starts, layout)
            if position is None:
                continue
            start = vertex_data + offset + position
            view = memoryview(data)[start:start + vertex_size * (vertex_count - 1) + 12]
            fmt = f"<3f{vertex_size - 12}x"
            for xyz in struct.iter_unpack(fmt, bytes(view) + bytes(vertex_size - 12)):
                for axis in range(3):
                    lo[axis] = min(lo[axis], xyz[axis])
                    hi[axis] = max(hi[axis], xyz[axis])
        if lo[0] == float("inf"):
            continue
        half = [(h - l) / 2 for l, h in zip(lo, hi)]
        center = [(h + l) / 2 for l, h in zip(lo, hi)]
        out = bytearray(data)
        struct.pack_into("<9f", out, bounds_offset, *half, 0.0, 0.0, 0.0, *center)
        data = bytes(out)
    return data


def material_layout(data, material_index=0):
    """(type, semantic, index) of each vertex member used by the first mesh with this material, e.g. to copy the
    layout of the vanilla FLVER our export replaces."""
    _, counts = _counts(data)
    starts, _ = _section_starts(counts)
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * SIZES["mesh"]
        if struct.unpack_from("<i", data, mesh + 4)[0] == material_index:
            buffer_index = struct.unpack_from("<i", data, struct.unpack_from("<i", data, mesh + 0x2C)[0])[0]
            vb = starts["vertex_buffer"] + buffer_index * SIZES["vertex_buffer"]
            layout_index = struct.unpack_from("<i", data, vb + 4)[0]
            break
    else:
        raise ValueError(f"No mesh uses material {material_index}.")
    member_count, _, _, members = struct.unpack_from("<4i", data, starts["layout"] + layout_index * SIZES["layout"])
    return [struct.unpack_from("<5i", data, members + 20 * k)[2:] for k in range(member_count)]


def convert_layout(data, target, material_index=0):
    """Repack the vertex buffers of meshes using `material_index` to the member list `target` [(type, semantic, index)], e.g. the vanilla layout
    for the same material. Soulstruct adds bone indices, a second tangent and vertex color 0 where the game's own
    asset uses color 1; the shader may not draw a mesh whose layout doesn't match. Members are copied byte for byte
    from the source member with the same semantic (same index preferred) and the same type, or another byte4 type."""
    _, counts = _counts(data)
    starts, _ = _section_starts(counts)
    target = [tuple(m) for m in target]
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    out = bytearray(data)
    selected = set()
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * SIZES["mesh"]
        if struct.unpack_from("<i", data, mesh + 4)[0] == material_index:
            buffer_count, buffer_indices = struct.unpack_from("<2i", data, mesh + 0x28)
            selected.update(struct.unpack_from(f"<{buffer_count}i", data, buffer_indices))
    buffers = []
    for i in range(counts["vertex_buffer"]):
        vb = starts["vertex_buffer"] + i * SIZES["vertex_buffer"]
        _, layout, vertex_size, vertex_count, _, _, length, offset = struct.unpack_from("<8i", data, vb)
        buffers.append((offset, vb, layout, vertex_size, vertex_count, length))
    converted_layouts = set()
    # Last buffer first, so splicing never moves a buffer that is still to be converted.
    for offset, vb, layout, vertex_size, vertex_count, length in sorted(buffers, reverse=True):
        if (vb - starts["vertex_buffer"]) // SIZES["vertex_buffer"] not in selected:
            continue
        header = starts["layout"] + layout * SIZES["layout"]
        member_count, _, _, members = struct.unpack_from("<4i", data, header)
        source = [struct.unpack_from("<5i", data, members + 20 * k)[1:] for k in range(member_count)]
        if [tuple(m[1:]) for m in source] == target:
            continue
        bounds = sorted(m[0] for m in source) + [vertex_size]
        size_at = {start: end - start for start, end in zip(bounds, bounds[1:])}
        picks, used = [], set()
        for data_type, semantic, index in target:
            candidates = [k for k, m in enumerate(source) if k not in used and m[2] == semantic and (
                m[1] == data_type or {m[1], data_type} <= BYTE4_TYPES)]
            if not candidates:
                raise ValueError(f"No member of type {data_type} / semantic {semantic} to fill the target layout.")
            k = next((k for k in candidates if source[k][3] == index), candidates[0])
            used.add(k)
            picks.append((source[k][0], size_at[source[k][0]]))
        new_size = sum(size for _, size in picks)
        start = vertex_data + offset
        old = memoryview(data)[start:start + length]
        new = bytearray(new_size * vertex_count)
        pos = 0
        for at, size in picks:
            for v in range(vertex_count):
                new[v * new_size + pos:v * new_size + pos + size] = old[v * vertex_size + at:v * vertex_size + at + size]
            pos += size
        if layout not in converted_layouts:
            converted_layouts.add(layout)
            struct.pack_into("<i", out, header, len(target))
            pos = 0
            for k, (data_type, semantic, index) in enumerate(target):
                struct.pack_into("<5i", out, members + 20 * k, 0, pos, data_type, semantic, index)
                pos += picks[k][1]
        struct.pack_into("<i", out, vb + 8, new_size)
        struct.pack_into("<i", out, vb + 0x18, len(new))
        shift = len(new) - length
        # Shift the relative offsets of anything stored after this buffer.
        for j in range(counts["face_set"]):
            fs = starts["face_set"] + j * SIZES["face_set"] + 0x0C  # indices offset
            if struct.unpack_from("<i", out, fs)[0] > offset:
                struct.pack_into("<i", out, fs, struct.unpack_from("<i", out, fs)[0] + shift)
        for other_offset, other_vb, *_ in buffers:
            if other_offset > offset:
                struct.pack_into("<i", out, other_vb + 0x1C, struct.unpack_from("<i", out, other_vb + 0x1C)[0] + shift)
        struct.pack_into("<i", out, 0x10, struct.unpack_from("<i", out, 0x10)[0] + shift)
        out[start:start + length] = new
        data = bytes(out)
        out = bytearray(data)
    return bytes(out)


def _utf16(data, offset):
    end = offset
    while data[end:end + 2] != b"\0\0":
        end += 2
    return data[offset:end].decode("utf-16-le")


def _material_textures(data, material_index=0):
    """{texture type: (header position, path)} for one material."""
    _, counts = _counts(data)
    starts, _ = _section_starts(counts)
    _, _, count, first = struct.unpack_from("<4i", data, starts["material"] + material_index * SIZES["material"])
    textures = {}
    for i in range(first, first + count):
        header = starts["texture"] + i * SIZES["texture"]
        path, kind = struct.unpack_from("<2i", data, header)
        textures[_utf16(data, kind)] = (header, _utf16(data, path))
    return textures


def copy_texture_paths(data, vanilla, material_index=0):
    """Give each texture of the material the path the vanilla FLVER uses for the same slot. Vanilla asset FLVERs leave
    most paths empty and let the material supply the texture; Soulstruct writes bare file names instead."""
    wanted = {kind: path for kind, (_, path) in _material_textures(vanilla, material_index).items()}
    out = bytearray(data)
    appended = []
    for kind, (header, path) in _material_textures(data, material_index).items():
        if kind not in wanted or path == wanted[kind]:
            continue
        if wanted[kind]:
            appended.append((header, wanted[kind]))
        else:
            # Point at the terminator of the type name: an empty string.
            kind_offset = struct.unpack_from("<i", data, header + 4)[0]
            struct.pack_into("<i", out, header, kind_offset + 2 * len(kind))
    data = bytes(out)
    if appended:
        # New strings go right before the vertex data, padded to keep it 16-byte aligned.
        at = struct.unpack_from("<i", data, 0x0C)[0]
        block, positions = bytearray(), []
        for header, path in appended:
            positions.append((header, at + len(block)))
            block += path.encode("utf-16-le") + b"\0\0"
        block += bytes(-len(block) % 16)
        data = bytearray(_insert(data, at, bytes(block)))
        for header, offset in positions:
            struct.pack_into("<i", data, header, offset)
        data = bytes(data)
    return data


def set_normal_w(data, value=0, material_index=0):
    """Set the fourth byte of every byte4 normal in the material's meshes. Elden Ring reads it as the bone a rigid
    vertex follows: vanilla assets store 0 (their only bone), Soulstruct stores 127 (no bone), and the game then
    doesn't draw the mesh."""
    _, counts = _counts(data)
    starts, _ = _section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    out = bytearray(data)
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * SIZES["mesh"]
        if struct.unpack_from("<i", data, mesh + 4)[0] != material_index:
            continue
        buffer_count, buffer_indices = struct.unpack_from("<2i", data, mesh + 0x28)
        for buffer_index in struct.unpack_from(f"<{buffer_count}i", data, buffer_indices):
            vb = starts["vertex_buffer"] + buffer_index * SIZES["vertex_buffer"]
            _, layout, vertex_size, vertex_count, _, _, _, offset = struct.unpack_from("<8i", data, vb)
            member_count, _, _, members = struct.unpack_from("<4i", data, starts["layout"] + layout * SIZES["layout"])
            for k in range(member_count):
                _, struct_offset, data_type, semantic, _ = struct.unpack_from("<5i", data, members + 20 * k)
                if semantic == NORMAL_SEMANTIC and data_type in BYTE4_TYPES:
                    first = vertex_data + offset + struct_offset + 3
                    out[first:first + vertex_size * vertex_count:vertex_size] = bytes([value]) * vertex_count
    return bytes(out)


def fix_flver(data, vanilla=None):
    """Fix a Soulstruct export; with `vanilla` (the game's FLVER it replaces), also match its vertex layout and
    texture paths for material 0."""
    data = add_skeleton_set(fix_mesh_bounds(data))
    if vanilla:
        data = copy_texture_paths(convert_layout(data, material_layout(vanilla)), vanilla)
        data = set_normal_w(data)
    return data


def fix_binder(path, vanilla=None):
    """Fix the FLVERs inside a (DCX-compressed) binder such as a GEOMBND, optionally matching them to `vanilla` (see
    fix_flver). Needs Soulstruct for Oodle compression."""
    from soulstruct.containers import Binder

    binder = Binder.from_path(path)
    changed = False
    for entry in binder.entries:
        if entry.name.lower().endswith(".flver"):
            data = entry.get_uncompressed_data()
            fixed = fix_flver(data, vanilla)
            if fixed != data:
                entry.set_uncompressed_data(fixed)
                changed = True
    if changed:
        binder.write()
    return changed


def flver_in_binder(path):
    """The first FLVER inside a (DCX-compressed) binder, uncompressed. Needs Soulstruct."""
    from soulstruct.containers import Binder

    return next(e.get_uncompressed_data() for e in Binder.from_path(path).entries if e.name.lower().endswith(".flver"))


def main():
    src = sys.argv[1]
    dst = sys.argv[2] if len(sys.argv) > 2 else src
    data = open(src, "rb").read()
    vanilla = open(sys.argv[3], "rb").read() if len(sys.argv) > 3 else None
    fixed = fix_flver(data, vanilla)
    if fixed == data:
        print("Nothing to fix.")
        return
    open(dst, "wb").write(fixed)
    print(f"Fixed FLVER: {len(data)} -> {len(fixed)} bytes, written to {dst}")


if __name__ == "__main__":
    main()
