"""Write Wizard101's own vertex normals of a .nif next to its converted OBJ, so our FLVER shades like W101.

Usage: python nif_normals.py input.nif output.normals [scale]

nif2obj.py writes no normals, so the street FLVER used to get one flat normal per triangle (every triangle of a tree
trunk lit as its own facet, one near white and the next much darker). W101's meshes carry per-vertex normals, smooth
across most edges and split only at the hard ones, and many are hand-set (tree canopies, foliage cards), so they
can't be recomputed from the geometry alone (an area-weighted average matches W101 only ~85% of the time).

The output is in the OBJ's frame (same transform as nif2obj.py: node matrices, scale, Gamebryo Z-up -> OBJ Y-up), one
record per NIF vertex: b"W101NRM1", uint32 count, then count x (float32 x, y, z position, float32 x, y, z unit
normal). flver_build.read_normals reads it (stdlib, inside Blender) and flver_build.corner_normals looks the OBJ's
corners up in it by position. Needs pyffi.
"""

import struct
import sys

import nif2obj
from nif2obj import NifFormat, apply, walk

MAGIC = b"W101NRM1"  # flver_build.NORMALS_MAGIC


def _unit(v):
    length = sum(c * c for c in v) ** 0.5
    return tuple(c / length for c in v) if length > 1e-12 else None


def nif_normals(src, scale=0.01):
    """[(position, normal)] in OBJ space for every vertex of every NiTriShape/NiMesh with normals."""
    data = NifFormat.Data()
    with open(src, "rb") as f:
        data.read(f)
    identity = [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]
    meshes = []
    for root in data.roots:
        walk(root, identity, meshes)
    out = []
    for mesh, matrix in meshes:
        if isinstance(mesh, NifFormat.NiTriShape):
            shape = mesh.data
            if shape is None or not shape.has_vertices or not shape.has_normals:
                continue
            pos = [(v.x, v.y, v.z) for v in shape.vertices]
            nrm = [(n.x, n.y, n.z) for n in shape.normals]
        else:
            streams = {}
            for ref in mesh.datas:
                streams.setdefault(ref.component_semantics[0].name.decode(), nif2obj.read_stream(ref))
            pos, nrm = streams.get("POSITION"), streams.get("NORMAL")
            if not pos or not nrm:
                continue
        for p, n in zip(pos, nrm):
            x, y, z = (round(c * scale, 5) for c in apply(matrix, p))  # nif2obj writes positions with 5 decimals
            normal = _unit(apply(matrix, n, w=0.0))
            if normal:
                nx, ny, nz = normal
                out.append(((x, z, -y), (nx, nz, -ny)))  # Gamebryo Z-up -> OBJ Y-up, as nif2obj
    return out


def write(records, path):
    with open(path, "wb") as f:
        f.write(MAGIC + struct.pack("<I", len(records)))
        for p, n in records:
            f.write(struct.pack("<6f", *p, *n))


if __name__ == "__main__":
    records = nif_normals(sys.argv[1], float(sys.argv[3]) if len(sys.argv) > 3 else 0.01)
    write(records, sys.argv[2])
    print(f"{len(records)} vertex normals -> {sys.argv[2]}")
