"""Round 27 (B1): dialogue portraits of the street NPCs' Elden Ring stand-ins, rendered by Blender (Cycles) from the
game's own FLVER models and textures, then set in Wizard101's blue portrait inside its gold ring, as
orbs\\w101_orbs\\book\\art\\GUI\\NpcPortraits\\ER\\<slug>.png (hud_orbs markers::er_portraits shows them in the
dialogue box and the talk prompt). Replaces the book's software-renderer portraits (tools/w101_npc_portraits.py).

Run by blender_build_all.py (step "portraits") or alone in Blender's Scripting tab. Reads the game's chr and parts
folders (read only) and book\\er\\idle_pose.tsv; a file it replaces is kept once as <name>.before_round27.

What it does per NPC (tools/er_portrait_scene.py builds the meshes):
- c0000 people: face, eyes, hair, eyebrows and beard from FaceParam, worn armour from CharaInitParam (pieces hidden by
  the armour's invisibleFlag_SexVer bits, as the game does), posed in the player's idle animation's first frame.
  Hair, brows and beard are strand cards: their colour is the FaceParam hair colour and their alpha the shared
  strand mask (common_body.tpf AAT500_PCHair_n alpha / the part's own _n alpha), so hair has real strands.
- chr models (Enia, Trina, Kale, Ranni, Nox, the Giant Skeleton Torso): the chrbnd FLVER in its rest pose with its
  _h textures.
- Materials: albedo (alpha-clipped), tangent normal map (DirectX green flipped), rougher cloth and skin, shinier metal.
  Eye whites get the iris painted from the _3m mask (model.rs eye()).
- Light: a warm key from the front left, a cool fill from the right, a rim light from behind; head-and-shoulders
  camera turned a little off the face (VIEWS), Cycles with denoising, transparent background, 512 px.
- Composite: Wizard101's NpcPortraits ring (per-pixel median of six portraits) and their blue (quadratic fit), 256 px,
  exactly as w101_npc_portraits.py did.

Settings for a run outside Ryan's PC (the cloud previews use them): PORTRAIT_GAME (Game folder), PORTRAIT_DEC (folder
of already decompressed copies), PORTRAIT_BOOK, PORTRAIT_OUT, PORTRAIT_ONLY (comma list of slugs), PORTRAIT_SAMPLES.
"""
from __future__ import annotations

import importlib
import math
import os
import shutil
import sys
import tempfile
from pathlib import Path

import bpy
import numpy as np

ROOT = Path.home() / "Documents" / "game_modding"
TOOLS = Path(os.environ.get("PORTRAIT_TOOLS", ROOT / "eldenring-w101" / "tools"))
sys.path.insert(0, str(TOOLS))
import er_portrait_scene as scene  # noqa: E402

importlib.reload(scene)

GAME = Path(os.environ.get("PORTRAIT_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game"))
DEC = os.environ.get("PORTRAIT_DEC")
BOOK = Path(os.environ.get("PORTRAIT_BOOK", ROOT / "reference" / "eldenring" / "orbs" / "w101_orbs" / "book"))
OUT = Path(os.environ.get("PORTRAIT_OUT", BOOK / "art" / "GUI" / "NpcPortraits" / "ER"))
SAMPLES = int(os.environ.get("PORTRAIT_SAMPLES", "96"))
# Olivia Dawnwillow's stand-in (round 27: Rya the Scout, since Enia's c2170 has no standing idle; street README): both are rendered (olivia_dawnwillow.enia.png / .rya.png) and this one is also written as
# olivia_dawnwillow.png, the file the DLL shows.
OLIVIA = os.environ.get("PORTRAIT_OLIVIA", "rya")
RENDER = 512
SIDE = 256
BACKUP = ".before_round27"

# slug -> (view width m, centre above the head bone m, camera yaw rad (+ = from the character's left), file name
#          [, centre shift right m, camera height x width])
VIEWS = {
    "private_connelly": (0.46, 0.03, 0.35, "private_connelly"),
    "private_oryan": (0.46, 0.03, 0.35, "private_oryan"),
    "ceren_nightchant": (0.46, 0.03, 0.35, "ceren_nightchant"),
    "diego": (0.48, 0.03, 0.35, "diego"),
    "olivia_rya": (0.46, 0.03, 0.35, "olivia_dawnwillow.rya"),
    "olivia_dawnwillow": (0.75, 0.05, 0.2, "olivia_dawnwillow.enia"),
    "ugo_kalahad": (0.75, 0.06, 0.3, "ugo_kalahad"),
    "lady_oriel": (0.6, 0.06, -0.5, "lady_oriel", -0.07),
    "merle_ambrose": (0.55, -0.02, 0.25, "merle_ambrose", 0.0, -0.45),
    "lady_blackhope": (0.8, 0.06, 0.3, "lady_blackhope"),
    "rattlebones": (4.2, 0.1, 0.25, "rattlebones"),
}
# Bosses sit on the red enemy disc (round 26 W8, w101_portrait_red.py), fitted to Wizard101's own enemy pictures.
RED = {"rattlebones", "lady_blackhope"}
RED_REFS = ["Fairy_DarkMinion_MOB", "Portrait_Skeleton_Pirate_A_MOB", "Portrait_Ghost_LostSoul_MOB_A",
            "Portrait_Skeleton_Armored_A_MOB"]
W101 = ["Human_WCGuard_A", "Human_WCGuard_B", "Art_Portrait_Boy_Life02", "Human_WCShopKeepF_A_NPC",
        "Portrait_Seraph_Aerial_B_NPC", "Portrait_Owl_Gamma_A_NPC"]


# ---------------------------------------------------------------- images
class Images:
    def __init__(self, tex: dict, work: Path):
        self.tex, self.work, self.cache = tex, work, {}

    def get(self, stem: str | None):
        if not stem or stem not in self.tex:
            return None
        if stem not in self.cache:
            p = self.work / f"{stem}.dds"
            p.write_bytes(self.tex[stem])
            im = bpy.data.images.load(str(p))
            im.colorspace_settings.name = "sRGB" if stem.endswith(("_a", "_a_l")) else "Non-Color"
            im.alpha_mode = "CHANNEL_PACKED"
            self.cache[stem] = im
        return self.cache[stem]

    def eye(self, white: str, mask: str):
        key = f"eye:{white}"
        if key not in self.cache:
            w, m = self.get(white), self.get(mask)
            if w is None or m is None:
                return w
            wp = np.array(w.pixels[:], np.float32).reshape(w.size[1], w.size[0], 4)
            mp = np.array(m.pixels[:], np.float32).reshape(m.size[1], m.size[0], 4)
            ys = (np.arange(w.size[1]) * m.size[1] // w.size[1])
            xs = (np.arange(w.size[0]) * m.size[0] // w.size[0])
            mp = mp[ys][:, xs]
            im = bpy.data.images.new("eye_baked", w.size[0], w.size[1], alpha=True)
            im.pixels[:] = scene.eye_pixels(wp, mp).ravel()
            im.pack()
            self.cache[key] = im
        return self.cache[key]


def srgb_to_linear(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name: str, mesh: scene.Mesh, images: Images):
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    nodes, links = nt.nodes, nt.links
    bsdf = nodes.get("Principled BSDF")
    bsdf.inputs["Roughness"].default_value = 0.6
    uv = nodes.new("ShaderNodeUVMap")
    uv.uv_map = "UV"
    colour = (*[srgb_to_linear(c) for c in mesh.colour], 1.0)
    alpha_socket = None
    if mesh.kind == "eye":
        im = images.eye(mesh.albedo, mesh.alpha)
        if im:
            t = nodes.new("ShaderNodeTexImage")
            t.image = im
            links.new(uv.outputs[0], t.inputs[0])
            links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
        bsdf.inputs["Roughness"].default_value = 0.15
    elif mesh.kind == "hair":
        bsdf.inputs["Base Color"].default_value = colour
        bsdf.inputs["Roughness"].default_value = 0.45
        a = images.get(mesh.alpha)
        if a:
            t = nodes.new("ShaderNodeTexImage")
            t.image = a
            links.new(uv.outputs[0], t.inputs[0])
            alpha_socket = t.outputs["Alpha"]
    else:
        im = images.get(mesh.albedo)
        if im:
            t = nodes.new("ShaderNodeTexImage")
            t.image = im
            links.new(uv.outputs[0], t.inputs[0])
            links.new(t.outputs["Color"], bsdf.inputs["Base Color"])
            if mesh.kind in ("solid", "metal", "solid_alpha"):
                alpha_socket = t.outputs["Alpha"]
        else:
            bsdf.inputs["Base Color"].default_value = colour
        if mesh.kind == "metal":
            bsdf.inputs["Metallic"].default_value = 0.55
            bsdf.inputs["Roughness"].default_value = 0.38
        if mesh.kind == "skin":
            bsdf.inputs["Roughness"].default_value = 0.5
            bsdf.inputs["Subsurface Weight"].default_value = 0.12
            bsdf.inputs["Subsurface Radius"].default_value = (1.0, 0.35, 0.2)
            bsdf.inputs["Subsurface Scale"].default_value = 0.01
        n = images.get(mesh.normal)
        if n:
            tn = nodes.new("ShaderNodeTexImage")
            tn.image = n
            links.new(uv.outputs[0], tn.inputs[0])
            sep = nodes.new("ShaderNodeSeparateColor")
            links.new(tn.outputs["Color"], sep.inputs[0])
            inv = nodes.new("ShaderNodeMath")
            inv.operation = "SUBTRACT"
            inv.inputs[0].default_value = 1.0
            links.new(sep.outputs[1], inv.inputs[1])
            comb = nodes.new("ShaderNodeCombineColor")
            links.new(sep.outputs[0], comb.inputs[0])
            links.new(inv.outputs[0], comb.inputs[1])
            comb.inputs[2].default_value = 1.0
            nm = nodes.new("ShaderNodeNormalMap")
            nm.uv_map = "UV"
            nm.inputs["Strength"].default_value = 0.7
            links.new(comb.outputs[0], nm.inputs["Color"])
            links.new(nm.outputs[0], bsdf.inputs["Normal"])
    if alpha_socket is not None:
        if mesh.kind == "hair":
            links.new(alpha_socket, bsdf.inputs["Alpha"])
        else:
            clip = nodes.new("ShaderNodeMath")
            clip.operation = "GREATER_THAN"
            clip.inputs[1].default_value = 0.5
            links.new(alpha_socket, clip.inputs[0])
            links.new(clip.outputs[0], bsdf.inputs["Alpha"])
        if hasattr(mat, "surface_render_method"):
            mat.surface_render_method = "DITHERED"
    return mat


def add_mesh(m: scene.Mesh, images: Images, collection):
    me = bpy.data.meshes.new(m.name)
    me.vertices.add(len(m.pos))
    me.vertices.foreach_set("co", m.pos.astype(np.float32).ravel())
    n = len(m.tris)
    me.loops.add(3 * n)
    me.polygons.add(n)
    me.loops.foreach_set("vertex_index", m.tris.astype(np.int32).ravel())
    me.polygons.foreach_set("loop_start", np.arange(0, 3 * n, 3, dtype=np.int32))
    me.polygons.foreach_set("loop_total", np.full(n, 3, np.int32))
    uv = me.uv_layers.new(name="UV")
    loop_uv = m.uv[m.tris.ravel()]
    uv.data.foreach_set("uv", np.stack([loop_uv[:, 0], 1.0 - loop_uv[:, 1]], 1).astype(np.float32).ravel())
    me.update(calc_edges=True)
    me.validate(clean_customdata=False)
    nrm = m.nrm / np.maximum(np.linalg.norm(m.nrm, axis=1, keepdims=True), 1e-6)
    # Keep the game's winding facing its normals (flip the triangles if most face away).
    fn = np.array([p.normal for p in me.polygons])
    vn = nrm[m.tris].mean(1) if len(fn) == len(m.tris) else None
    if vn is not None and len(fn) and (np.einsum("ij,ij->i", fn, vn) < 0).mean() > 0.5:
        me.flip_normals()
    me.normals_split_custom_set_from_vertices(nrm.astype(np.float32).tolist())
    me.polygons.foreach_set("use_smooth", np.ones(len(me.polygons), bool))
    me.materials.append(material(m.name, m, images))
    ob = bpy.data.objects.new(m.name, me)
    collection.objects.link(ob)
    return ob


def light(sc, name, kind, loc, target, energy, colour, size=1.0):
    data = bpy.data.lights.new(name, kind)
    data.energy = energy
    data.color = colour
    if kind == "AREA":
        data.size = size
    ob = bpy.data.objects.new(name, data)
    sc.collection.objects.link(ob)
    ob.location = loc
    look_at(ob, target)
    return ob


def look_at(ob, target):
    from mathutils import Vector
    d = Vector(target) - ob.location
    ob.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def render_one(slug: str, files: scene.Files, idle: dict, work: Path) -> Path | None:
    meshes, tex, head = scene.build(files, slug, idle)
    if not meshes:
        print(f"portrait {slug}: no meshes")
        return None
    # A scene of its own (Ryan's open Blender file is left as it is); removed with everything in it afterwards.
    before = {k: {b.name for b in getattr(bpy.data, k)} for k in CLEAN}
    sc = bpy.data.scenes.new(f"portrait_{slug}")
    images = Images(tex, work)
    for m in meshes:
        add_mesh(m, images, sc.collection)
    if head is None:
        allp = np.concatenate([m.pos for m in meshes])
        head = np.array([allp[:, 0].mean(), allp[:, 1].mean(), allp[:, 2].max() - 0.15])
    v = VIEWS[slug]
    width, up, yaw, _ = v[:4]
    side, elev = (tuple(v[4:]) + (0.0, 0.04)[len(v) - 4:])[:2]
    # The character faces Blender -Y; yaw turns the camera round it (+ = toward the character's left, image right);
    # side moves the picture's centre along the image's right; elev raises the camera (x width).
    target = (float(head[0]) + side * math.cos(yaw), float(head[1]) - side * math.sin(yaw), float(head[2] + up))
    dist = width * 3.2
    cam_loc = (target[0] - dist * math.sin(yaw), target[1] - dist * math.cos(yaw), target[2] + elev * width)
    cam_data = bpy.data.cameras.new("cam")
    cam_data.angle = 2 * math.atan(width / 2 / dist)
    cam_data.clip_start, cam_data.clip_end = 0.05, 50
    cam = bpy.data.objects.new("cam", cam_data)
    sc.collection.objects.link(cam)
    cam.location = cam_loc
    look_at(cam, target)
    sc.camera = cam
    # Lights relative to the face (scaled with the view for the giant).
    k = width / 0.62

    def around(a, h, r):
        return (target[0] - r * k * math.sin(a), target[1] - r * k * math.cos(a), target[2] + h * k)

    light(sc, "key", "AREA", around(0.75, 0.55, 1.3), target, 90 * k * k, (1.0, 0.93, 0.85), 0.9 * k)
    light(sc, "fill", "AREA", around(-0.9, 0.1, 1.4), target, 35 * k * k, (0.8, 0.88, 1.0), 1.2 * k)
    light(sc, "rim", "AREA", around(math.pi - 0.5, 0.7, 1.2), target, 120 * k * k, (0.85, 0.92, 1.0), 0.6 * k)
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.3, 0.36, 0.45, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.35
    sc.world = world
    sc.render.engine = "CYCLES"
    cy = sc.cycles
    cy.samples = SAMPLES
    cy.use_denoising = True
    cy.seed = 1
    try:
        prefs = bpy.context.preferences.addons["cycles"].preferences
        for backend in ("OPTIX", "CUDA", "HIP", "ONEAPI", "METAL"):
            try:
                prefs.compute_device_type = backend
                prefs.get_devices()
                if any(d.type != "CPU" for d in prefs.devices):
                    for d in prefs.devices:
                        d.use = True
                    cy.device = "GPU"
                    break
            except TypeError:
                continue
    except Exception:  # no GPU: CPU
        pass
    sc.render.film_transparent = True
    sc.render.resolution_x = sc.render.resolution_y = RENDER
    sc.render.resolution_percentage = 100
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_mode = "RGBA"
    sc.view_settings.view_transform = "Standard"
    sc.view_settings.look = "None"
    sc.view_settings.exposure = 0.0
    out = work / f"{slug}.raw.png"
    sc.render.filepath = str(out)
    try:
        bpy.ops.render.render(write_still=True, scene=sc.name)
    finally:
        for ob in list(sc.collection.objects):
            bpy.data.objects.remove(ob)
        bpy.data.scenes.remove(sc)
        purge(before)
    return out


# Data the portraits make (removed after each one, so a run leaves Ryan's Blender file as it was).
CLEAN = ("meshes", "materials", "images", "lights", "cameras", "worlds")


def purge(before):
    for kind in CLEAN:
        coll = getattr(bpy.data, kind)
        for block in [b for b in coll if b.name not in before[kind]]:
            coll.remove(block)


# ---------------------------------------------------------------- Wizard101 frame (w101_npc_portraits.py)
def read_png(path: Path) -> np.ndarray:
    """RGBA 0..255 float, top row first."""
    im = bpy.data.images.load(str(path))
    px = np.array(im.pixels[:], np.float32).reshape(im.size[1], im.size[0], 4)[::-1] * 255.0
    bpy.data.images.remove(im)
    return px


def resize(img: np.ndarray, side: int) -> np.ndarray:
    """Area/bilinear resample to side x side (numpy)."""
    h, w = img.shape[:2]
    if h % side == 0 and w % side == 0 and h >= side:
        f = h // side
        return img.reshape(side, f, side, f, -1).mean((1, 3))
    ys = (np.arange(side) + 0.5) * h / side - 0.5
    xs = (np.arange(side) + 0.5) * w / side - 0.5
    y0, x0 = np.clip(np.floor(ys).astype(int), 0, h - 1), np.clip(np.floor(xs).astype(int), 0, w - 1)
    y1, x1 = np.clip(y0 + 1, 0, h - 1), np.clip(x0 + 1, 0, w - 1)
    fy, fx = (ys - y0)[:, None, None], (xs - x0)[None, :, None]
    a = img[y0][:, x0] * (1 - fx) + img[y0][:, x1] * fx
    b = img[y1][:, x0] * (1 - fx) + img[y1][:, x1] * fx
    return a * (1 - fy) + b * fy


def background(portraits: Path, names=None, red=False):
    """(ring median, quadratic fit of the painted disc colour) of Wizard101's portraits: the NPCs' blue disc, or
    (red) the enemies' red disc (round 26 W8: boss pictures on the red disc)."""
    ims = np.stack([read_png(portraits / f"{n}.png") for n in (names or W101)])
    yy, xx = np.mgrid[0:128, 0:128]
    r = np.hypot(xx - 63.5, yy - 63.5)
    pts, cols = [], []
    for im in ims:
        R, G, B = im[..., 0], im[..., 1], im[..., 2]
        blue = ((R > G + 60) & (R > B + 40) if red else (B > R + 60) & (B > G + 25)) & (r < 51)
        pts.append(np.stack([xx[blue], yy[blue]], 1))
        cols.append(im[blue][:, :3])
    pts, cols = np.concatenate(pts).astype(float), np.concatenate(cols)
    X = np.stack([np.ones(len(pts)), pts[:, 0], pts[:, 1], pts[:, 0] ** 2, pts[:, 1] ** 2, pts[:, 0] * pts[:, 1]], 1)
    coef = np.linalg.lstsq(X, cols, rcond=None)[0]
    return np.median(ims, 0), coef


def compose(ring, coef, render: np.ndarray) -> np.ndarray:
    k = SIDE / 128.0
    yy, xx = np.mgrid[0:SIDE, 0:SIDE]
    x, y = (xx + 0.5) / k - 0.5, (yy + 0.5) / k - 0.5
    X = np.stack([np.ones_like(x), x, y, x * x, y * y, x * y], -1)
    canvas = np.zeros((SIDE, SIDE, 4))
    canvas[..., :3] = np.clip(X @ coef, 0, 255)
    r = np.hypot(x - 63.5, y - 63.5)
    face = resize(render, SIDE)
    a = face[..., 3:4] / 255.0 * np.clip(52.0 - r, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - a) + face[..., :3] * a
    ring_px = resize(ring, SIDE)
    m = np.clip(r - 51.0, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - m) + ring_px[..., :3] * m
    canvas[..., 3] = np.where(r >= 51.0, ring_px[..., 3], 255)
    return np.clip(canvas, 0, 255)


def write_png(px: np.ndarray, path: Path):
    h, w = px.shape[:2]
    im = bpy.data.images.new(path.stem, w, h, alpha=True)
    im.pixels[:] = (px[::-1] / 255.0).astype(np.float32).ravel()
    im.filepath_raw = str(path)
    im.file_format = "PNG"
    im.save()
    bpy.data.images.remove(im)


def main():
    only = [s for s in os.environ.get("PORTRAIT_ONLY", "").split(",") if s]
    slugs = only or list(VIEWS)
    files = scene.Files(GAME, Path(DEC) if DEC else None)
    idle = scene.load_idle(BOOK / "er" / "idle_pose.tsv")
    portraits = BOOK / "art" / "GUI" / "NpcPortraits"
    OUT.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="npc_portraits_"))
    raws = {}
    try:
        for slug in slugs:
            raw = render_one(slug, files, idle, work)
            if raw:
                raws[slug] = read_png(raw)
                if os.environ.get("KEEP"):
                    shutil.copyfile(raw, OUT / f"{VIEWS[slug][3]}.raw.png")
        ring, coef = background(portraits)
        _, red_coef = background(portraits, RED_REFS, red=True)
        for slug, raw in raws.items():
            target = OUT / f"{VIEWS[slug][3]}.png"
            backup = target.with_name(target.name + BACKUP)
            if target.exists() and not backup.exists():
                shutil.copyfile(target, backup)
            px = compose(ring, red_coef if slug in RED else coef, raw)
            write_png(px, target)
            print(f"portrait: {target}")
            if target.name == f"olivia_dawnwillow.{OLIVIA}.png":
                shown = OUT / "olivia_dawnwillow.png"
                backup = shown.with_name(shown.name + BACKUP)
                if shown.exists() and not backup.exists():
                    shutil.copyfile(shown, backup)
                write_png(px, shown)
                print(f"portrait: {shown} (Olivia = {OLIVIA})")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
