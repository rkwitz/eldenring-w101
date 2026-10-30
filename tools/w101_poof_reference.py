"""Reference frames of Wizard101's teleport poof (_Shared-WorldData.wad Character/FX_Disappear.nif), simulated from
the .nif's own particle data, to compare with the HUD's version (hud_orbs/src/teleport.rs; preview shots 13_poof_*,
same camera and background).
Usage: python -I w101_poof_reference.py meshes.json <dir with the raw FX_*.png> out_dir W H times...
  meshes.json: {"Editable Mesh"/"Cylinder01"/"Cylinder02": {v, t, uv, vc}, "path": [[percent, x, y, z], ...]}
  dumped from the .nif with pyffi; the PNGs are the textures as extracted (additive ones on black). Needs numpy, Pillow.
  W101 data: never commit the inputs or frames.
Assumptions (Gamebryo behaviour not stored in the file): variations are symmetric +-var/2; path keys' times are
the path's percent; additive blending for sparks/tubes (alpha flags 4097 = ONE/ONE), alpha blend for smoke (4845);
vertex colours are the emissive source for sparks/tubes (NiVertexColorProperty 16 = SRC_EMISSIVE, lighting E),
the smoke is lit (flags 40: SRC_AMB_DIF, E_A_D): colour = texture x min(1, 0.5 emissive + key)."""
import json, math, sys, random
import numpy as np
from PIL import Image, ImageDraw

UNIT = 1.9 / 320.0
meshes = json.load(open(sys.argv[1]))
texdir, out = sys.argv[2], sys.argv[3]
W, H = int(sys.argv[4]), int(sys.argv[5])
times = [float(t) for t in sys.argv[6:]]
tex = {k: np.asarray(Image.open(f"{texdir}/{f}").convert("RGBA")).astype(np.float32) / 255 for k, f in
       [("blue", "FX_DisappearSparks._1.png"), ("gold", "FX_DisappearSparks._2.png"),
        ("tube", "FX_LeaveToob_01.png"), ("smoke", "FX_SmokeChimney.png")]}

def w2e(p):  # Wizard101 units (z up) -> metres, ER axes (y up)
    return np.array([p[0] * UNIT, p[2] * UNIT, p[1] * UNIT])

# Camera: same as the preview (hud_orbs/preview teleport shots): at (0, 1.55, -3.4) looking at (0, 0.95, 0), fov 48.
cam_pos = np.array([0.0, 1.55, -3.4]); look = np.array([0.0, 0.95, 0.0]); FOV = math.radians(48)
fwd = look - cam_pos; fwd /= np.linalg.norm(fwd)
right = np.cross([0, 1, 0], fwd); right /= np.linalg.norm(right)
up = np.cross(fwd, right)
def project(p):
    d = p - cam_pos
    x, y, z = d @ right, d @ up, d @ fwd
    if z < 0.3: return None
    t = math.tan(FOV / 2)
    return ((1 + x / (z * t * W / H)) / 2 * W, (1 - y / (z * t)) / 2 * H, z)
def pixels(z): return H / 2 / (z * math.tan(FOV / 2))

body = meshes["Editable Mesh"]
bv = np.array(body["v"]); btris = body["t"]
def body_point(rng):
    a, b, c = bv[list(btris[rng.randrange(len(btris))])]
    u, v = rng.random(), rng.random()
    if u + v > 1: u, v = 1 - u, 1 - v
    p = a + u * (b - a) + v * (c - a)
    n = np.cross(b - a, c - a); n /= (np.linalg.norm(n) or 1)
    # Outward: away from the body's axis.
    if n[0] * p[0] + n[1] * p[1] < 0: n = -n
    return p, n
path = np.array(meshes["path"])
def path_at(f):
    f = min(max(f, 0), 1)
    i = np.searchsorted(path[:, 0], f)
    if i <= 0: return path[0, 1:]
    if i >= len(path): return path[-1, 1:]
    a, b = path[i - 1], path[i]
    k = (f - a[0]) / max(b[0] - a[0], 1e-6)
    return a[1:] + k * (b[1:] - a[1:])
box = [np.array([x, y, z]) for x in (-8, 8) for y in (-8, 8) for z in (0, 15)]

def keys3(x):  # sparks' colour keys (rgb and alpha): 1, 0.5 at 0.75, 0
    return 1 - 0.5 * x / 0.75 if x < 0.75 else 0.5 * (1 - x) / 0.25
def smoke_key(x):
    return 0.18 + 0.42 * x / 0.2 if x < 0.2 else 0.6 - 0.42 * (x - 0.2) / 0.8

# name, texture, rate, start, stop, radius, radius var, life, life var, speed, speed var, accel z, spin, spin var, grow, fade
SYSTEMS = [
    ("spark", "gold", 272.7, 0.0, 0.367, 15 * 1.95, 4.5 * 1.95, 0.533, 0.2, 0, 0, 90 - 180, 5.236, 1.745, 0, 0.167),
    ("spark", "blue", 272.7, 0.0, 0.367, 15 * 1.95, 4.5 * 1.95, 0.533, 0.2, 0, 0, 90 - 180, 5.236, 1.745, 0, 0.167),
    ("twinkle", "blue", 409.1, 0.0, 0.367, 20, 0, 0.1, 0, 300, 300, 0, 3.491, 0.611, 0.0333, 0),
    ("twinkle", "gold", 272.7, 0.0, 0.367, 15, 0, 0.1, 0, 300, 300, 0, 3.491, 0.611, 0.0333, 0),
    ("smoke", "smoke", 200.0, 0.267, 0.367, 100, 0, 1.333, 0, 0, 0, 180, 3.491, 0.611, 0, 0),
]

def simulate(until, seed=7):
    rng = random.Random(seed); dt = 1 / 240; t = 0; owed = [0.0] * len(SYSTEMS); parts = []
    sym = lambda v: (rng.random() - 0.5) * v
    while t < until - 1e-9:
        for s, (kind, tx, rate, start, stop, rad, radv, life, lifev, spd, spdv, acc, spin, spinv, grow, fade) in enumerate(SYSTEMS):
            if start <= t < stop:
                owed[s] += rate * dt
            while owed[s] >= 1:
                owed[s] -= 1
                if kind == "spark":
                    p = path_at(t / 0.367) + box[rng.randrange(8)] - np.array([0, 0, 7.5]); v = np.zeros(3)
                else:
                    p, n = body_point(rng)
                    v = n * (spd + sym(spdv))
                parts.append(dict(s=s, p=p.astype(float), v=v.astype(float), age=0.0, life=life + sym(lifev),
                                  r=rad + sym(radv), spin=(spin + sym(spinv)) * rng.choice((-1, 1)), ang=0.0))
        for q in parts:
            acc = SYSTEMS[q["s"]][11]
            turb = 6 if SYSTEMS[q["s"]][0] == "spark" else (2 if SYSTEMS[q["s"]][0] == "smoke" else 0)
            q["v"][2] += acc * dt
            if turb: q["v"] += np.array([rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1)]) * turb * dt
            q["p"] += q["v"] * dt; q["age"] += dt; q["ang"] += q["spin"] * dt
        parts = [q for q in parts if q["age"] < q["life"]]
        t += dt
    return parts

def sprite(canvas, texname, centre, half, ang, colour, alpha, additive):
    n = max(2, int(half * 2))
    img = Image.fromarray((tex[texname] * 255).astype(np.uint8)).resize((n, n), Image.BILINEAR).rotate(math.degrees(ang), Image.BILINEAR)
    a = np.asarray(img).astype(np.float32) / 255
    x0, y0 = int(centre[0] - n / 2), int(centre[1] - n / 2)
    xs, ys = max(0, x0), max(0, y0); xe, ye = min(W, x0 + n), min(H, y0 + n)
    if xe <= xs or ye <= ys: return
    sub = a[ys - y0:ye - y0, xs - x0:xe - x0]
    if additive:
        canvas[ys:ye, xs:xe] += sub[..., :3] * colour
    else:
        al = sub[..., 3:4] * alpha
        canvas[ys:ye, xs:xe] = canvas[ys:ye, xs:xe] * (1 - al) + sub[..., :3] * colour * al

def tubes(canvas, t):
    if t >= 0.367: return
    shift = t / 0.367
    tt = tex["tube"]; th, tw = tt.shape[:2]
    for name in ("Cylinder01", "Cylinder02"):
        m = meshes[name]
        zs = sorted(set(round(v[2]) for v in m["v"]))
        r = max(math.hypot(v[0], v[1]) for v in m["v"])
        rows = sorted({(round(v[2]), round(uv[1], 3), round(c[0], 3)) for v, uv, c in zip(m["v"], m["uv"], m["vc"])})
        ring = {}
        for z, v, c in rows: ring.setdefault(z, (v, c))
        zk = sorted(ring)
        def at(z):
            for a, b in zip(zk, zk[1:]):
                if a <= z <= b:
                    k = (z - a) / (b - a)
                    return ring[a][0] + k * (ring[b][0] - ring[a][0]), ring[a][1] + k * (ring[b][1] - ring[a][1])
            return ring[zk[-1]]
        layer = Image.new("RGB", (W, H)); dr = ImageDraw.Draw(layer)
        ROWS, AROUND = 60, 48; top = zk[-1]
        for i in range(ROWS):
            z0, z1 = top * i / ROWS, top * (i + 1) / ROWS
            v, c = at((z0 + z1) / 2)
            vt = min(max(v + shift, 0), 0.999)
            for k in range(AROUND):
                u = (k + 0.5) / AROUND
                col = tt[int(vt * th), int(u * tw), :3] * c
                if col.max() < 0.004: continue
                a0, a1 = 2 * math.pi * k / AROUND, 2 * math.pi * (k + 1) / AROUND
                pts = [project(w2e((r * math.cos(a), r * math.sin(a), z))) for a, z in ((a0, z0), (a1, z0), (a1, z1), (a0, z1))]
                if any(p is None for p in pts): continue
                dr.polygon([(p[0], p[1]) for p in pts], fill=tuple(int(x * 255) for x in col))
        canvas += np.asarray(layer).astype(np.float32) / 255  # additive (two sides overlap only at the silhouette)

def render(t, bg):
    canvas = bg.copy()
    parts = simulate(t)
    # smoke first (alpha), then tubes and sparks (additive)
    for q in parts:
        kind, tx = SYSTEMS[q["s"]][0], SYSTEMS[q["s"]][1]
        if kind != "smoke": continue
        pr = project(w2e(q["p"]))
        if not pr: continue
        x = q["age"] / q["life"]; k = smoke_key(x)
        light = min(1.0, 0.5 + k)
        sprite(canvas, tx, pr, q["r"] * UNIT * pixels(pr[2]), q["ang"], light, k, False)
    tubes(canvas, t)
    for q in parts:
        kind, tx = SYSTEMS[q["s"]][0], SYSTEMS[q["s"]][1]
        if kind == "smoke": continue
        pr = project(w2e(q["p"]))
        if not pr: continue
        x = q["age"] / q["life"]; k = keys3(x)
        scale = 1.0
        fade, grow = SYSTEMS[q["s"]][15], SYSTEMS[q["s"]][14]
        if fade: scale = min(1.0, (q["life"] - q["age"]) / fade)
        if grow: scale = min(1.0, q["age"] / grow)
        sprite(canvas, tx, pr, q["r"] * UNIT * pixels(pr[2]) * scale, q["ang"], k, 1, True)
    return np.clip(canvas, 0, 1)

if __name__ == "__main__":
    bg_path = sys.argv[7] if False else None
    bg = np.zeros((H, W, 3), np.float32); bg[:] = np.array([70, 90, 80]) / 255
    for t in times:
        img = render(t, bg)
        Image.fromarray((img * 255).astype(np.uint8)).save(f"{out}/w101_{t:.2f}.png")
        print(t)
