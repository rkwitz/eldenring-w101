"""Work out where to lay flat invisible floor pieces under an asset's walkable surfaces (stdlib only).

Elden Ring asset collision has to be an fsnpCustomParamCompressedMeshShape, which no public tool writes yet, so until
we can build real collision we cover the street with the game's own invisible wall piece aeg099_002 laid flat.
`floor_tiles` grids the mesh's upward-facing triangles into tiles one piece wide and returns one height per tile:
the most common walkable height (by area) in that tile.
"""

import collections
import struct

import flver_fix as ff

PIECE_WIDTH = 14.92  # aeg099_002 collision: x -7.466..7.459, y 0..8.343, z +-0.065
PIECE_LENGTH = 8.343
PIECE_HALF_THICKNESS = 0.065


def _triangles(data):
    """Yield the corner positions of every triangle in the first face set of each mesh."""
    _, counts = ff._counts(data)
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * ff.SIZES["mesh"]
        face_set_index = struct.unpack_from("<i", data, struct.unpack_from("<i", data, mesh + 0x24)[0])[0]
        buffer_index = struct.unpack_from("<i", data, struct.unpack_from("<i", data, mesh + 0x2C)[0])[0]
        fs = starts["face_set"] + face_set_index * ff.SIZES["face_set"]
        index_count, index_offset, _, _, index_size = struct.unpack_from("<5i", data, fs + 0x08)
        indices = struct.unpack_from(f"<{index_count}{'I' if index_size == 32 else 'H'}", data,
                                     vertex_data + index_offset)
        vb = starts["vertex_buffer"] + buffer_index * ff.SIZES["vertex_buffer"]
        _, layout, vertex_size, _, _, _, _, offset = struct.unpack_from("<8i", data, vb)
        base = vertex_data + offset + ff._position_offset(data, starts, layout)
        for k in range(0, index_count - 2, 3):
            yield [struct.unpack_from("<3f", data, base + indices[k + c] * vertex_size) for c in range(3)]


def floor_tiles(data, tile=(PIECE_WIDTH, 2 * PIECE_LENGTH), heights=(-8.0, 6.0), min_area=40.0, step=0.25):
    """[(x, height, z)] tile centres in the model's coordinates, for tiles with at least `min_area` square metres of
    walkable surface between `heights`. A walkable triangle faces up within ~25 degrees."""
    areas = collections.defaultdict(lambda: collections.Counter())
    for a, b, c in _triangles(data):
        u = [b[i] - a[i] for i in range(3)]
        v = [c[i] - a[i] for i in range(3)]
        n = (u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0])
        length = (n[0] ** 2 + n[1] ** 2 + n[2] ** 2) ** 0.5
        if not length or abs(n[1]) < 0.9 * length:
            continue
        x, y, z = ((a[i] + b[i] + c[i]) / 3 for i in range(3))
        if heights[0] <= y <= heights[1]:
            areas[(int(x // tile[0]), int(z // tile[1]))][round(y / step) * step] += length / 2
    tiles = []
    for (i, j), counter in sorted(areas.items()):
        if sum(counter.values()) >= min_area:
            tiles.append(((i + 0.5) * tile[0], counter.most_common(1)[0][0], (j + 0.5) * tile[1]))
    return tiles
