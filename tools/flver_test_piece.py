"""Cut a small test piece out of a single-mesh FLVER, shaped like the game's own asset FLVERs.

Our exported street is one mesh of ~470k vertices, which forces 32-bit face indices; vanilla asset FLVERs use 16-bit
indices. The piece keeps the triangles nearest a point (by default the middle of the road) up to 65535 vertices, so it
can use 16-bit indices, and copies two small fields from the vanilla file (face set Unk06 = 1, bone flags = 0). If the
game draws the piece but not the full street, the mesh needs splitting into 16-bit chunks.

Usage: python flver_test_piece.py street.flver piece.flver
"""

import struct
import sys

import flver_fix as ff

MAX_VERTICES = 0xFFFF
ROAD_CENTER = (-126.4, 0.0, 116.2)  # middle of Unicorn Way's road in the asset's own coordinates


def small_piece(data, center=ROAD_CENTER, max_vertices=MAX_VERTICES):
    _, counts = ff._counts(data)
    if counts["mesh"] != 1 or counts["face_set"] != 1 or counts["vertex_buffer"] != 1:
        raise ValueError(f"Expected one mesh, face set and vertex buffer, got {counts}.")
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    fs = starts["face_set"]
    index_count, index_offset, _, _, index_size = struct.unpack_from("<5i", data, fs + 0x08)
    indices = struct.unpack_from(f"<{index_count}{'I' if index_size == 32 else 'H'}", data, vertex_data + index_offset)
    vb = starts["vertex_buffer"]
    _, layout, vertex_size, vertex_count, _, _, length, offset = struct.unpack_from("<8i", data, vb)
    position = ff._position_offset(data, starts, layout)
    vertices = data[vertex_data + offset:vertex_data + offset + length]

    def pos(i):
        return struct.unpack_from("<3f", vertices, i * vertex_size + position)

    # Nearest triangles first, until the vertex budget is spent.
    triangles = [indices[k:k + 3] for k in range(0, index_count - 2, 3)]
    def distance(tri):
        c = [sum(pos(i)[a] for i in tri) / 3 for a in range(3)]
        return sum((c[a] - center[a]) ** 2 for a in range(3))
    remap, kept = {}, []
    for tri in sorted(triangles, key=distance):
        new = [i for i in tri if i not in remap]
        if len(remap) + len(set(new)) > max_vertices:
            break
        for i in tri:
            remap.setdefault(i, len(remap))
        kept.append(tri)

    order = sorted(remap, key=remap.get)
    new_vertices = b"".join(vertices[i * vertex_size:(i + 1) * vertex_size] for i in order)
    new_indices = struct.pack(f"<{3 * len(kept)}H", *(remap[i] for tri in kept for i in tri))
    padded = new_indices + bytes(-len(new_indices) % 16)
    region = padded + new_vertices
    points = [pos(i) for i in order]
    lo = [min(p[a] for p in points) for a in range(3)]
    hi = [max(p[a] for p in points) for a in range(3)]

    out = bytearray(data[:vertex_data] + region)
    struct.pack_into("<i", out, 0x10, len(region))
    struct.pack_into("<6f", out, 0x28, *lo, *hi)  # header bounding box
    struct.pack_into("<2i", out, 0x40, len(kept), len(kept))  # true and total face counts
    out[fs + 0x06] = 1  # Unk06, set on every vanilla face set
    struct.pack_into("<5i", out, fs + 0x08, len(kept) * 3, 0, len(new_indices), 0, 16)
    struct.pack_into("<i", out, vb + 0x0C, len(order))
    struct.pack_into("<2i", out, vb + 0x18, len(new_vertices), len(padded))
    bone = starts["bone"]
    struct.pack_into("<3f", out, bone + 0x30, *lo)
    struct.pack_into("<i", out, bone + 0x3C, 0)  # flags; 0 in vanilla
    struct.pack_into("<3f", out, bone + 0x40, *hi)
    bounds = struct.unpack_from("<i", data, starts["mesh"] + 0x18)[0]
    half = [(h - l) / 2 for l, h in zip(lo, hi)]
    middle = [(h + l) / 2 for l, h in zip(lo, hi)]
    struct.pack_into("<9f", out, bounds, *half, 0.0, 0.0, 0.0, *middle)
    return bytes(out)


def main():
    piece = small_piece(open(sys.argv[1], "rb").read())
    open(sys.argv[2], "wb").write(piece)
    print(f"Wrote {len(piece)} bytes to {sys.argv[2]}")


if __name__ == "__main__":
    main()
