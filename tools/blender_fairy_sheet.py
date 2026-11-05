"""Round 30 (FA1): the hedge maze fairies' sprite sheet for hud_orbs/src/fairies.rs, rendered by Blender from
Wizard101's own model, textures and animation. Writes w101_orbs\\fairies\\fairy_sheet.png (never committed).

Source (Mob-WorldData.wad, the maze's NPCSprite template 82309): Character/DarkSprite/MC_Spirit_Model.nif (two
skinned NiTriShapes: SC_Spirit:0 body with Textures/SC_Spirit_Body__26a9720d.nif, SC_Spirit:1 wings with
SC_Spirit_Wing__972b01ee.nif, alpha-tested at 150 by its NiAlphaProperty), posed by MC_Dark_Spirit_Walk.kf
(MC_Spirit_Anims.kfm's walk clip, 2.33 s; NiBSplineCompTransformInterpolators evaluated as open uniform cubic
B-splines, NiTransformInterpolators with XYZ / quaternion keys) and skinned as pyffi's get_skin_deformation does.

Render: Cycles, transparent, orthographic, 10 degrees above, VIEWS rows (row 0 from the front, W101 models face -y,
then the camera steps to the fairy's right) x FRAMES columns of CELL px. The square covers 2 x radius of every
frame's bounds round their centre; fairies.rs SIZE / CENTRE_UP take the numbers this prints.

Usage (bpy module: pip install bpy==5.1.2 pyffi setuptools pillow, Python 3.13; or Blender's own Python):
  python blender_fairy_sheet.py <Wizard101 Data\\GameData> <out folder>
Env: VIEWS (8), FRAMES (48), CELL (224), SAMPLES (24), ELEV (10), PITCH (50).
"""
import math
import os
import struct
import sys
import tempfile
import time
import zlib
from pathlib import Path

time.clock = time.perf_counter  # pyffi 2.2.3
sys.path.insert(0, str(Path(__file__).parent))
import numpy as np  # noqa: E402
from pyffi.formats.nif import NifFormat  # noqa: E402

WANT = ["Character/DarkSprite/MC_Spirit_Model.nif", "Character/DarkSprite/MC_Dark_Spirit_Walk.kf",
        "Textures/SC_Spirit_Body__26a9720d.nif", "Textures/SC_Spirit_Wing__972b01ee.nif"]


def extract(gamedata, dest):
    with open(Path(gamedata) / "Mob-WorldData.wad", "rb") as f:
        head = f.read(14)
        version, count = struct.unpack_from("<II", head, 5)
        f.seek(13 + (1 if version >= 2 else 0))
        entries = []
        for _ in range(count):
            o, s, z, iz, _crc, n = struct.unpack("<IIIBII", f.read(21))
            entries.append((f.read(n).rstrip(b"\0").decode("latin-1"), o, s, z, iz))
        for name, o, s, z, iz in entries:
            if name in WANT:
                f.seek(o)
                raw = f.read(z if iz else s)
                (Path(dest) / Path(name).name).write_bytes(zlib.decompress(raw) if iz else raw)


BIG = 1e30

def load(path):
    d = NifFormat.Data(); s = open(path, "rb"); d.inspect(s); d.read(s); return d

def m33(r):
    return np.array([[r.m_11, r.m_12, r.m_13], [r.m_21, r.m_22, r.m_23], [r.m_31, r.m_32, r.m_33]], float)

def quat_rows(w, x, y, z):
    # matrix in the same (row-vector) convention as pyffi's node.rotation: transpose of column-vector rotation
    n = (w*w + x*x + y*y + z*z) ** 0.5; w, x, y, z = w/n, x/n, y/n, z/n
    c = np.array([[1-2*(y*y+z*z), 2*(x*y-w*z), 2*(x*z+w*y)],
                  [2*(x*y+w*z), 1-2*(x*x+z*z), 2*(y*z-w*x)],
                  [2*(x*z-w*y), 2*(y*z+w*x), 1-2*(x*x+y*y)]])
    return c.T

def euler_rows(rx, ry, rz):
    cx, sx, cy, sy, cz, sz = np.cos(rx), np.sin(rx), np.cos(ry), np.sin(ry), np.cos(rz), np.sin(rz)
    X = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]]); Y = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]])
    Z = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]])
    return (Z @ Y @ X).T

def mat4(s, r, t):
    m = np.eye(4); m[:3, :3] = r * s; m[3, :3] = t; return m

def bspline(cps, u):
    """Open uniform cubic B-spline (Gamebryo NiBSplineBasis degree 3) at u in [0, 1]."""
    cps = np.asarray(cps, float); n = len(cps)
    if n < 4:
        return cps[min(int(u * (n - 1) + 0.5), n - 1)]
    k = 3; knots = np.concatenate([np.zeros(k), np.linspace(0, 1, n - k + 1), np.ones(k)])
    u = min(max(u, 0.0), 1.0 - 1e-9)
    i = np.searchsorted(knots, u, side="right") - 1
    d = [cps[j + i - k].copy() for j in range(k + 1)]
    for r in range(1, k + 1):
        for j in range(k, r - 1, -1):
            a = (u - knots[j + i - k]) / (knots[j + 1 + i - r] - knots[j + i - k])
            d[j] = (1 - a) * d[j - 1] + a * d[j]
    return d[k]

def keys_at(keys, t):
    ts = [k.time for k in keys]
    if t <= ts[0]: return keys[0].value
    if t >= ts[-1]: return keys[-1].value
    i = np.searchsorted(ts, t) - 1
    a = (t - ts[i]) / (ts[i + 1] - ts[i]); return (keys[i], keys[i + 1], a)

class Anim:
    def __init__(self, kf):
        seq = load(kf).roots[0]
        self.start, self.stop = seq.start_time, seq.stop_time
        self.blocks = {}
        for b in seq.controlled_blocks:
            self.blocks[b.get_node_name().decode()] = b.interpolator

    def local(self, name, rest, t):
        """(scale, rot rows, trans) of node `name` at time t, rest = (s, r, t) defaults."""
        s, r, tr = rest
        ip = self.blocks.get(name)
        if ip is None: return rest
        if isinstance(ip, NifFormat.NiBSplineCompTransformInterpolator):
            u = (t - ip.start_time) / max(ip.stop_time - ip.start_time, 1e-6)
            tv = ip
            T = list(ip.get_translations()); R = list(ip.get_rotations()); S = list(ip.get_scales())
            tr = bspline(T, u) if T else (np.array([tv.translation.x, tv.translation.y, tv.translation.z]) if abs(tv.translation.x) < BIG else tr)
            if R:
                q = bspline(R, u); r = quat_rows(*q)
            elif abs(tv.rotation.w) < BIG:
                r = quat_rows(tv.rotation.w, tv.rotation.x, tv.rotation.y, tv.rotation.z)
            if S: s = float(bspline(np.array(S)[:, None], u)[0])
            elif abs(tv.scale) < BIG: s = tv.scale
            return s, r, np.asarray(tr, float)
        if isinstance(ip, NifFormat.NiTransformInterpolator):
            tv = ip; d = ip.data
            if abs(tv.translation.x) < BIG: tr = np.array([tv.translation.x, tv.translation.y, tv.translation.z])
            if abs(tv.rotation.w) < BIG: r = quat_rows(tv.rotation.w, tv.rotation.x, tv.rotation.y, tv.rotation.z)
            if abs(tv.scale) < BIG: s = tv.scale
            if d is not None:
                if d.translations.num_keys:
                    v = keys_at(d.translations.keys, t)
                    if isinstance(v, tuple):
                        k0, k1, a = v; tr = np.array([k0.value.x, k0.value.y, k0.value.z]) * (1 - a) + np.array([k1.value.x, k1.value.y, k1.value.z]) * a
                    else: tr = np.array([v.x, v.y, v.z])
                if d.rotation_type == 4:
                    ang = []
                    for g in d.xyz_rotations:
                        if g.num_keys == 0: ang.append(0.0); continue
                        v = keys_at(g.keys, t)
                        ang.append(v[0].value * (1 - v[2]) + v[1].value * v[2] if isinstance(v, tuple) else v)
                    if any(g.num_keys for g in d.xyz_rotations): r = euler_rows(*ang)
                elif d.num_rotation_keys:
                    v = keys_at(d.quaternion_keys, t)
                    if isinstance(v, tuple):
                        k0, k1, a = v; q0 = np.array([k0.value.w, k0.value.x, k0.value.y, k0.value.z]); q1 = np.array([k1.value.w, k1.value.x, k1.value.y, k1.value.z])
                        if q0 @ q1 < 0: q1 = -q1
                        r = quat_rows(*(q0 * (1 - a) + q1 * a))
                    else: r = quat_rows(v.w, v.x, v.y, v.z)
                if d.scales.num_keys:
                    v = keys_at(d.scales.keys, t)
                    s = v[0].value * (1 - v[2]) + v[1].value * v[2] if isinstance(v, tuple) else v
            return s, r, tr
        return rest

class Model:
    def __init__(self, nif):
        self.data = load(nif); self.root = self.data.roots[0]
        self.nodes = {}; self.parent = {}
        def walk(n, p):
            self.nodes[n.name.decode()] = n; self.parent[n.name.decode()] = p
            for c in getattr(n, "children", []) or []:
                if c is not None: walk(c, n.name.decode())
        walk(self.root, None)
        self.shapes = [n for n in self.nodes.values() if isinstance(n, NifFormat.NiTriShape)]

    def rest(self, name):
        n = self.nodes[name]; return n.scale, m33(n.rotation), np.array([n.translation.x, n.translation.y, n.translation.z])

    def world(self, anim=None, t=0.0, fixed=("MoveNode",)):
        W = {}
        def get(name):
            if name in W: return W[name]
            rest = self.rest(name)
            loc = anim.local(name, rest, t) if (anim and name not in fixed) else rest
            m = mat4(*loc); p = self.parent[name]
            W[name] = m if p is None else m @ get(p)
            return W[name]
        for k in self.nodes: get(k)
        return W

    def skinned(self, W):
        out = []
        for sh in self.shapes:
            d = sh.data; si = sh.skin_instance; sd = si.data
            V = np.array([[v.x, v.y, v.z, 1.0] for v in d.vertices])
            skin_off = np.array(sd.get_transform().as_list())
            root_w = W[si.skeleton_root.name.decode()]; inv_root = np.linalg.inv(root_w)
            acc = np.zeros((len(V), 4))
            for i, bone in enumerate(si.bones):
                bd = sd.bone_list[i]
                M = np.array(bd.get_transform().as_list()) @ (W[bone.name.decode()] @ inv_root) @ skin_off
                for w in bd.vertex_weights:
                    acc[w.index] += w.weight * (V[w.index] @ M)
            # into model-root space: skinned verts are relative to the skeleton root
            acc = acc @ root_w
            uv = np.array([[c.u, c.v] for c in d.uv_sets[0]]) if len(d.uv_sets) else None
            tris = np.array([list(t) for t in d.get_triangles()])
            out.append((sh.name.decode(), acc[:, :3], uv, tris))
        return out


def textures(src, dest):
    import w101_teleport_art as T
    T.DXT.setdefault(5, (b"DXT3", 16))  # the wing texture is DXT3
    for n in ("SC_Spirit_Body__26a9720d", "SC_Spirit_Wing__972b01ee"):
        T.pixel_image((Path(src) / f"{n}.nif").read_bytes()).save(Path(dest) / f"{n}.png")


def render(D, TEX, OUT):
    import bpy
    import mathutils
    VIEWS = int(os.environ.get("VIEWS", 8)); FRAMES = int(os.environ.get("FRAMES", 48)); CELL = int(os.environ.get("CELL", 224))
    ELEV = math.radians(float(os.environ.get("ELEV", 10)))
    m = Model(D + "/MC_Spirit_Model.nif"); a = Anim(D + "/MC_Dark_Spirit_Walk.kf")
    times = [a.start + (a.stop - a.start) * i / FRAMES for i in range(FRAMES)]
    poses = [m.skinned(m.world(a, t)) for t in times]
    # Round 31 (FA1, Ryan: "they look tilted up at the sky, not facing forward"): Walk.kf on its own holds the
    # body upright with the face up; W101's maze sprites (ref 6_hedge_maze_fairies.png) fly pitched forward, the
    # body axis about 47 degrees off vertical towards the flight, legs trailing. PITCH (degrees, ref-measured)
    # turns the posed mesh forward (head towards -y, the model's front) about the first frame's middle.
    pitch = math.radians(float(os.environ.get("PITCH", 50)))
    first = np.concatenate([s[1] for s in poses[0]])
    pivot = (first.min(0) + first.max(0)) / 2
    c, s_ = math.cos(pitch), math.sin(pitch)
    turn = np.array([[1, 0, 0], [0, c, s_], [0, -s_, c]])  # rows: v @ turn turns +z towards -y
    poses = [[(n, (v - pivot) @ turn + pivot, uv, tris) for n, v, uv, tris in p] for p in poses]
    allv = np.concatenate([s[1] for p in poses for s in p])
    lo, hi = allv.min(0), allv.max(0); centre = (lo + hi) / 2; radius = float(np.linalg.norm(hi - lo) / 2) * 1.02
    bpy.ops.wm.read_factory_settings(use_empty=True)
    sc = bpy.context.scene
    sc.render.engine = "CYCLES"; sc.cycles.samples = int(os.environ.get("SAMPLES", 24)); sc.cycles.use_denoising = False
    sc.cycles.device = "CPU"; sc.render.film_transparent = True
    sc.render.resolution_x = sc.render.resolution_y = CELL; sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGBA"; sc.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("w"); sc.world = world; world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.9, 0.9, 0.95, 1); world.node_tree.nodes["Background"].inputs[1].default_value = 0.55
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN")); sun.data.energy = 2.2
    sun.rotation_euler = (math.radians(40), 0, math.radians(30)); sc.collection.objects.link(sun)
    def mat(name, img, clip):
        mt = bpy.data.materials.new(name); mt.use_nodes = True; nt = mt.node_tree; b = nt.nodes["Principled BSDF"]
        t = nt.nodes.new("ShaderNodeTexImage"); t.image = bpy.data.images.load(img); t.interpolation = "Linear"
        nt.links.new(t.outputs["Color"], b.inputs["Base Color"]); b.inputs["Roughness"].default_value = 0.8
        if clip:
            g = nt.nodes.new("ShaderNodeMath"); g.operation = "GREATER_THAN"; g.inputs[1].default_value = 150 / 255
            nt.links.new(t.outputs["Alpha"], g.inputs[0]); nt.links.new(g.outputs[0], b.inputs["Alpha"])
        return mt
    mats = {"SC_Spirit:0": mat("body", TEX + "/SC_Spirit_Body__26a9720d.png", False),
            "SC_Spirit:1": mat("wing", TEX + "/SC_Spirit_Wing__972b01ee.png", True)}
    objs = []
    for name, v, uv, tris in poses[0]:
        me = bpy.data.meshes.new(name); me.from_pydata((v - centre).tolist(), [], tris.tolist())
        uvl = me.uv_layers.new()
        for poly in me.polygons:
            for li in poly.loop_indices:
                vi = me.loops[li].vertex_index; uvl.data[li].uv = (uv[vi][0], 1 - uv[vi][1])
        me.materials.append(mats[name]); o = bpy.data.objects.new(name, me); sc.collection.objects.link(o); objs.append(o)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam")); cam.data.type = "ORTHO"; cam.data.ortho_scale = 2 * radius
    sc.collection.objects.link(cam); sc.camera = cam
    os.makedirs(OUT, exist_ok=True)
    for vi in range(VIEWS):
        # view yaw: 0 = camera in front of the fairy (W101 models face -y), counter-clockwise seen from above
        yaw = 2 * math.pi * vi / VIEWS
        d = np.array([math.sin(yaw) * math.cos(ELEV) * -1, -math.cos(yaw) * math.cos(ELEV), math.sin(ELEV)])
        pos = d * radius * 4; cam.location = pos.tolist()
        z = -d; z = z / np.linalg.norm(z)
        cam.rotation_euler = (mathutils.Vector((-d).tolist())).to_track_quat("-Z", "Y").to_euler()
        for fi, pose in enumerate(poses):
            for o, (name, v, uv, tris) in zip(objs, pose):
                co = (v - centre).ravel().tolist(); o.data.vertices.foreach_set("co", co); o.data.update()
            sc.render.filepath = f"{OUT}/v{vi}_f{fi:02d}.png"; bpy.ops.render.render(write_still=True)
    open(f"{OUT}/meta.txt", "w", newline="\n").write(f"centre {centre.tolist()}\nradius {radius}\nlo {lo.tolist()}\nhi {hi.tolist()}\nseconds {a.stop - a.start}\n")
    print("done", lo, hi, radius)
    return VIEWS, FRAMES, CELL, centre, radius


def main(argv):
    from PIL import Image
    out = Path(argv[2])
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="fairy_"))
    extract(argv[1], work)
    textures(work, work)
    views, frames, cell, centre, radius = render(str(work), str(work), str(work / "cells"))
    sheet = Image.new("RGBA", (frames * cell, views * cell), (0, 0, 0, 0))
    for v in range(views):
        for f in range(frames):
            sheet.paste(Image.open(work / "cells" / f"v{v}_f{f:02d}.png"), (f * cell, v * cell))
    target = out / "fairy_sheet.png"
    if target.exists() and not target.with_suffix(".png.before_round30").exists():
        target.rename(target.with_suffix(".png.before_round30"))
    sheet.save(target, optimize=True)
    print(f"wrote {target} {sheet.size}: VIEWS {views} FRAMES {frames}; fairies.rs SIZE = 2.0 * {radius / 100:.7f}, "
          f"CENTRE_UP = {centre[2] / 100:.6f}")


if __name__ == "__main__":
    main(sys.argv)
