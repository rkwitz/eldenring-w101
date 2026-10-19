"""Fix two gaps in Elden Ring FLVERs (version 0x2001A) written by Soulstruct for Blender 3.2.1.

1. Skeleton set: Elden Ring FLVERs carry a small table right after the texture headers. Soulstruct leaves it out, so
   SoulsFormats-based tools (Smithbox) fail with "Read Int32: 0xC | Expected: 0x0" in SkeletonSet. We insert one base
   and one full skeleton entry per bone and shift every absolute offset after it.
2. Mesh bounds: each mesh has a 36-byte box stored as (half extents, unknown zero vector, center). Soulstruct writes
   only the older 24-byte "unset" placeholder (FLT_MAX, -FLT_MAX), which the game could cull. We make room for the
   full box and compute it from the vertex positions.

Usage: python flver_fix.py input.flver [output.flver]
In Blender (Soulstruct installed), fix_binder(path) fixes every FLVER inside a compressed GEOMBND in place.
"""

import struct
import sys

HEADER_SIZE = 0x80
SECTIONS = (("dummy", 0x40), ("material", 0x20), ("bone", 0x80), ("mesh", 0x30), ("face_set", 0x20),
            ("vertex_buffer", 0x20), ("layout", 0x10), ("texture", 0x20))
SIZES = dict(SECTIONS)
POSITION_SEMANTIC, FLOAT3_TYPE = 0, 2


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


def fix_flver(data):
    return add_skeleton_set(fix_mesh_bounds(data))


def fix_binder(path):
    """Fix the FLVERs inside a (DCX-compressed) binder such as a GEOMBND. Needs Soulstruct for Oodle compression."""
    from soulstruct.containers import Binder

    binder = Binder.from_path(path)
    changed = False
    for entry in binder.entries:
        if entry.name.lower().endswith(".flver"):
            data = entry.get_uncompressed_data()
            fixed = fix_flver(data)
            if fixed != data:
                entry.set_uncompressed_data(fixed)
                changed = True
    if changed:
        binder.write()
    return changed


def main():
    src = sys.argv[1]
    dst = sys.argv[2] if len(sys.argv) > 2 else src
    data = open(src, "rb").read()
    fixed = fix_flver(data)
    if fixed == data:
        print("Nothing to fix.")
        return
    open(dst, "wb").write(fixed)
    print(f"Fixed FLVER: {len(data)} -> {len(fixed)} bytes, written to {dst}")


if __name__ == "__main__":
    main()
