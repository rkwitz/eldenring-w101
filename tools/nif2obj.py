"""Convert a Wizard101 (Gamebryo 20.6, NiMesh) .nif into a Wavefront OBJ + MTL.

Usage: python nif2obj.py input.nif output.obj [scale]

scale defaults to 0.01 (Wizard101 units to meters). Output is Y-up, matching
Blender's default OBJ import settings.

Needs pyffi (pip install pyffi). Only static triangle meshes are exported;
particles, animation and skinning are skipped.
"""
import struct
import sys
import time
from pathlib import Path

if not hasattr(time, "clock"):  # pyffi 2.2.3 still calls time.clock
    time.clock = time.perf_counter

from pyffi.formats.nif import NifFormat

# Gamebryo component formats -> (struct code, count, normalize divisor)
FORMATS = {
    0x10215: ("H", 1, None),  # UINT16_1 (indices)
    0x10219: ("I", 1, None),  # UINT32_1 (indices)
    0x30437: ("f", 3, None),  # FLOAT32_3
    0x20436: ("f", 2, None),  # FLOAT32_2
    0x40438: ("f", 4, None),  # FLOAT32_4
    0x4013C: ("B", 4, 255.0),  # NORMUINT8_4 (colors)
}


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def local_matrix(node):
    r, t, s = node.rotation, node.translation, node.scale
    rows = [[r.m_11, r.m_12, r.m_13], [r.m_21, r.m_22, r.m_23], [r.m_31, r.m_32, r.m_33]]
    # Gamebryo stores row vectors: v' = v * (R*s) + t
    m = [[rows[i][j] * s for j in range(3)] + [0.0] for i in range(3)]
    m.append([t.x, t.y, t.z, 1.0])
    return m


def apply(m, v, w=1.0):
    return tuple(v[0] * m[0][j] + v[1] * m[1][j] + v[2] * m[2][j] + w * m[3][j] for j in range(3))


def read_stream(ref):
    stream = ref.stream
    fmt = int(stream.component_formats[0])
    if fmt not in FORMATS:
        return None
    code, count, norm = FORMATS[fmt]
    raw = bytes(stream.data)
    size = struct.calcsize(code) * count
    values = [struct.unpack_from("<" + code * count, raw, i) for i in range(0, len(raw) - size + 1, size)]
    if norm:
        values = [tuple(c / norm for c in v) for v in values]
    return values


def texture_name(mesh):
    for prop in list(getattr(mesh, "properties", [])):
        if isinstance(prop, NifFormat.NiTexturingProperty) and prop.has_base_texture:
            src = prop.base_texture.source
            if src is not None:
                return src.file_name.decode(errors="replace")
    return None


def walk(node, parent_matrix, out):
    matrix = mat_mul(local_matrix(node), parent_matrix)
    if isinstance(node, NifFormat.NiMesh):
        out.append((node, matrix))
    for child in getattr(node, "children", []) or []:
        if child is not None:
            walk(child, matrix, out)


def convert(src, dst, scale=0.01):
    data = NifFormat.Data()
    with open(src, "rb") as f:
        data.read(f)
    identity = [[1.0 if i == j else 0.0 for j in range(4)] for i in range(4)]
    meshes = []
    for root in data.roots:
        walk(root, identity, meshes)

    dst = Path(dst)
    mtl_path = dst.with_suffix(".mtl")
    materials = {}
    exported = skipped = 0
    with open(dst, "w") as obj:
        obj.write(f"mtllib {mtl_path.name}\n")
        v_base = vt_base = 1
        for mesh, matrix in meshes:
            if int(mesh.primitive_type) != 0:  # triangles only
                skipped += 1
                continue
            streams = {}
            for ref in mesh.datas:
                sem = ref.component_semantics[0].name.decode()
                streams.setdefault(sem, read_stream(ref))
            pos, idx = streams.get("POSITION"), streams.get("INDEX")
            if not pos or not idx:
                skipped += 1
                continue
            uv = streams.get("TEXCOORD")
            name = mesh.name.decode(errors="replace") or f"mesh{exported}"
            obj.write(f"o {name}\n")
            for p in pos:
                x, y, z = (c * scale for c in apply(matrix, p))
                obj.write(f"v {x:.5f} {z:.5f} {-y:.5f}\n")  # Gamebryo Z-up -> OBJ Y-up
            if uv:
                for u, v in uv:
                    obj.write(f"vt {u:.5f} {1.0 - v:.5f}\n")
            tex = texture_name(mesh)
            if tex:
                mat = Path(tex.replace("\\", "/")).stem
                materials[mat] = tex
                obj.write(f"usemtl {mat}\n")
            flat = [i[0] for i in idx]
            for a, b, c in zip(flat[0::3], flat[1::3], flat[2::3]):
                if uv:
                    obj.write(f"f {a+v_base}/{a+vt_base} {b+v_base}/{b+vt_base} {c+v_base}/{c+vt_base}\n")
                else:
                    obj.write(f"f {a+v_base} {b+v_base} {c+v_base}\n")
            v_base += len(pos)
            if uv:
                vt_base += len(uv)
            exported += 1
    with open(mtl_path, "w") as mtl:
        for mat, tex in materials.items():
            mtl.write(f"newmtl {mat}\nmap_Kd {Path(tex.replace(chr(92), '/')).name}\n\n")
    print(f"exported {exported} meshes, skipped {skipped}, {len(materials)} materials -> {dst}")


if __name__ == "__main__":
    convert(sys.argv[1], sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 0.01)
