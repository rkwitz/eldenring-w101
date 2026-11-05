"""Wizard101 room lighting helpers for blender_build_room.py (numpy only, so they run in Blender's Python and here).

Round 25 B1 (Ryan: the Rattlebones room should look like Wizard101's: warmer, brighter wood floor, stained-glass
windows with coloured light, softer). What Wizard101 draws there (classic WC_Generic_Gauntlet_NoDoor, per-vertex
colours in the OBJ as "#vc" lines):
  - every mesh is texture x vertex colour (baked light). The wood floor's colours are a flat warm 0.91/0.88/0.77;
  - window_floor_05..12: one 5-triangle patch per window on the wood floor (3.8 to 7.2 m from the centre, 4-5 cm
    above it), drawn ADDITIVELY with the stained-glass part of the window texture times vertex colours 0.01..0.28:
    the coloured light spots on the floor;
  - Light_rays05..12: the beams from each window to its patch, additive the same way (0 at the floor, 0.28-0.32 at
    the window).
Elden Ring has no additive map material we can use for the beams (the vanilla light-shaft shader takes a 3-channel
mask and one colour, not a picture), so the beams stay out. The floor spots are baked here instead: each patch
becomes an opaque piece of floor with its own texture = the graded floor texture under it (the floor's UVs are planar,
fitted from the floor mesh) plus window colour x vertex colour, in one atlas, drawn with the room's emissive material
like the floor around it, so it matches the floor exactly outside the light.
"""

import struct

import numpy as np

LUMA = np.array([0.299, 0.587, 0.114])


# --- DXT1

def _565(c):
    return np.stack([((c >> 11) & 31) / 31, ((c >> 5) & 63) / 63, (c & 31) / 31], -1)


def dxt1_decode(dds):
    """(H, W, 3) float RGB 0..1 of mip 0 of a legacy DXT1 DDS."""
    assert dds[84:88] == b"DXT1", dds[84:88]
    height, width = struct.unpack_from("<II", dds, 12)
    bw, bh = max(1, width // 4), max(1, height // 4)
    blocks = np.frombuffer(dds, np.uint8, bw * bh * 8, 128).reshape(-1, 8)
    c0 = blocks[:, 0].astype(np.int32) | blocks[:, 1].astype(np.int32) << 8
    c1 = blocks[:, 2].astype(np.int32) | blocks[:, 3].astype(np.int32) << 8
    bits = (blocks[:, 4].astype(np.uint32) | blocks[:, 5].astype(np.uint32) << 8 |
            blocks[:, 6].astype(np.uint32) << 16 | blocks[:, 7].astype(np.uint32) << 24)
    a, b = _565(c0), _565(c1)
    four = (c0 > c1)[:, None]
    p2 = np.where(four, (2 * a + b) / 3, (a + b) / 2)
    p3 = np.where(four, (a + 2 * b) / 3, 0.0)
    palette = np.stack([a, b, p2, p3], 1)  # (n, 4, 3)
    idx = (bits[:, None] >> (2 * np.arange(16, dtype=np.uint32))[None]) & 3  # (n, 16)
    px = np.take_along_axis(palette, idx[:, :, None].astype(np.int64), 1)  # (n, 16, 3)
    return px.reshape(bh, bw, 4, 4, 3).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 3)


def _pack565(rgb):
    q = np.clip(np.rint(rgb * [31, 63, 31]), 0, [31, 63, 31]).astype(np.int32)
    return q[..., 0] << 11 | q[..., 1] << 5 | q[..., 2]


def _encode_level(img):
    h, w, _ = img.shape
    bh, bw = max(1, h // 4), max(1, w // 4)
    if h < 4 or w < 4:
        img = np.pad(img, ((0, max(0, 4 - h)), (0, max(0, 4 - w)), (0, 0)), mode="edge")
    px = img[:bh * 4, :bw * 4].reshape(bh, 4, bw, 4, 3).transpose(0, 2, 1, 3, 4).reshape(-1, 16, 3)
    mean = px.mean(1, keepdims=True)
    centred = px - mean
    cov = np.einsum("nki,nkj->nij", centred, centred)
    axis = np.ones((len(px), 3))
    for _ in range(4):  # power iteration for the main colour axis
        axis = np.einsum("nij,nj->ni", cov, axis)
        axis /= np.maximum(np.linalg.norm(axis, axis=1, keepdims=True), 1e-9)
    t = np.einsum("nki,ni->nk", centred, axis)
    hi = np.take_along_axis(px, t.argmax(1)[:, None, None].repeat(3, 2), 1)[:, 0]
    lo = np.take_along_axis(px, t.argmin(1)[:, None, None].repeat(3, 2), 1)[:, 0]
    c0, c1 = _pack565(hi), _pack565(lo)
    swap = c0 < c1
    c0, c1 = np.where(swap, c1, c0), np.where(swap, c0, c1)
    a, b = _565(c0), _565(c1)
    palette = np.stack([a, b, (2 * a + b) / 3, (a + 2 * b) / 3], 1)
    idx = ((px[:, :, None, :] - palette[:, None, :, :]) ** 2).sum(-1).argmin(-1).astype(np.uint32)
    idx[c0 == c1] = 0  # one colour (3-colour mode, index 0 = c0: opaque)
    bits = (idx << (2 * np.arange(16, dtype=np.uint32))[None]).sum(1).astype(np.uint32)
    out = np.zeros((len(px), 2), np.uint32)
    out[:, 0] = c0.astype(np.uint32) | c1.astype(np.uint32) << 16
    out[:, 1] = bits
    return out.astype("<u4").tobytes()


def dxt1_dds(img):
    """Legacy DXT1 DDS bytes with a full mip chain (box filtered) of an (H, W, 3) float image (powers of two)."""
    height, width, _ = img.shape
    levels, data = 0, bytearray()
    h, w = height, width
    while True:
        data += _encode_level(img)
        levels += 1
        if h == 1 and w == 1:
            break
        nh, nw = max(1, h // 2), max(1, w // 2)
        img = img.reshape(nh, h // nh, nw, w // nw, 3).mean((1, 3))
        h, w = nh, nw
    linear_size = max(1, width // 4) * max(1, height // 4) * 8
    pixel_format = struct.pack("<II4s5I", 32, 0x4, b"DXT1", 0, 0, 0, 0, 0)
    header = struct.pack("<4s7I44x", b"DDS ", 124, 0xA1007, height, width, linear_size, 0, levels) + pixel_format
    header += struct.pack("<5I", 0x401008, 0, 0, 0, 0)
    return header + bytes(data)


# --- colour

def grade(img, saturation=1.0, gains=(1.0, 1.0, 1.0)):
    """Saturation about the luma, then per-channel gains, clamped (as dds_color.boost with gains does per block)."""
    luma = (img * LUMA).sum(-1, keepdims=True)
    return np.clip((luma + (img - luma) * saturation) * np.asarray(gains), 0.0, 1.0)


def sample(img, u, v):
    """Bilinear, wrapping sample of `img` at OBJ texture coordinates (row = (1 - v) * H, as flver_build flips v)."""
    h, w, _ = img.shape
    x = np.asarray(u) * w - 0.5
    y = (1.0 - np.asarray(v)) * h - 0.5
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    fx, fy = (x - x0)[..., None], (y - y0)[..., None]
    p = lambda yy, xx: img[yy % h, xx % w]  # noqa: E731
    return ((p(y0, x0) * (1 - fx) + p(y0, x0 + 1) * fx) * (1 - fy) +
            (p(y0 + 1, x0) * (1 - fx) + p(y0 + 1, x0 + 1) * fx) * fy)


# --- floor light spots

def floor_uv_map(positions, uvs, faces, floor_name):
    """Least-squares affine map (x, z) -> (u, v) of the floor mesh `floor_name` (its UVs are planar)."""
    xz, uv = [], []
    for (_, name), corners in faces:
        if name == floor_name:
            for v, t in corners:
                xz.append((positions[v][0], positions[v][2], 1.0))
                uv.append(uvs[t])
    a, b = np.array(xz), np.array(uv)
    m, *_ = np.linalg.lstsq(a, b, rcond=None)
    assert np.abs(a @ m - b).max() < 1e-3, "floor UVs are not planar"
    return m  # (3, 2)


def bake_floor_spots(positions, uvs, colours, faces, spot_names, floor_name, floor_img, window_img, gain=1.0,
                     cell=256, px_per_m=64.0, margin=0.25):
    """The atlas image and new texture coordinates for the floor light spots.

    `faces` = [((material, object name), ((v, vt), ...))] (flver_build.parse_obj order with face_objects names),
    `colours[v]` = the vertex's #vc (r, g, b), `spot_names` = objects to bake (window_floor_*), `floor_img` = the
    graded floor texture, `window_img` = the window texture (both float RGB). Returns (atlas (H, W, 3), new_uvs list,
    {vertex index: new uv index offset}): append new_uvs to the OBJ's uvs and point each spot corner at
    len(uvs) + offset[v]."""
    floor_map = floor_uv_map(positions, uvs, faces, floor_name)
    spots = {}
    for (_, name), corners in faces:
        if name in spot_names:
            spots.setdefault(name, []).append(corners)
    names = sorted(spots)
    cols = 4
    rows = -(-len(names) // cols)
    atlas_h = 1 << int(np.ceil(np.log2(rows * cell)))
    atlas_w = cols * cell
    atlas = np.zeros((atlas_h, atlas_w, 3))
    new_uvs, offset = [], {}
    for k, name in enumerate(names):
        tris = spots[name]
        verts = sorted({v for c in tris for v, _ in c})
        xz = np.array([(positions[v][0], positions[v][2]) for v in verts])
        lo, hi = xz.min(0) - margin, xz.max(0) + margin
        scale = min(px_per_m, (cell - 8) / float((hi - lo).max()))
        ox, oy = (k % cols) * cell + 4, (k // cols) * cell + 4  # cell's pixel origin (top left)
        for v, p in zip(verts, xz):
            px, py = ox + (p[0] - lo[0]) * scale, oy + (p[1] - lo[1]) * scale
            offset[v] = len(new_uvs)
            new_uvs.append((float(px / atlas_w), float(1.0 - py / atlas_h)))
        # every pixel of the cell back to the floor plane
        yy, xx = np.mgrid[oy - 4:oy - 4 + cell, ox - 4:ox - 4 + cell]
        x = lo[0] + (xx + 0.5 - ox) / scale
        z = lo[1] + (yy + 0.5 - oy) / scale
        # barycentrics in every triangle; the one each pixel is most inside wins (clamped outside: soft padding)
        best, best_t = np.full(x.shape, -np.inf), np.zeros(x.shape, int)
        weights_all = []
        for t, corners in enumerate(tris):
            (ax, az), (bx, bz), (cx, cz) = [(positions[v][0], positions[v][2]) for v, _ in corners]
            det = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz)
            w0 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / det
            w1 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / det
            w = np.stack([w0, w1, 1 - w0 - w1], -1)
            weights_all.append(w)
            m = w.min(-1)
            take = m > best
            best = np.where(take, m, best)
            best_t = np.where(take, t, best_t)
        weights = np.stack([np.choose(best_t, [ww[..., i] for ww in weights_all]) for i in range(3)], -1)
        weights = np.clip(weights, 0.0, None)
        weights /= weights.sum(-1, keepdims=True)
        uv_c = np.zeros(x.shape + (2,))
        vc_c = np.zeros(x.shape + (3,))
        for t, corners in enumerate(tris):
            sel = best_t == t
            for i, (v, vt) in enumerate(corners):
                uv_c[sel] += weights[sel][:, i:i + 1] * np.asarray(uvs[vt])
                vc_c[sel] += weights[sel][:, i:i + 1] * np.asarray(colours[v][:3])
        fu = floor_map[0, 0] * x + floor_map[1, 0] * z + floor_map[2, 0]
        fv = floor_map[0, 1] * x + floor_map[1, 1] * z + floor_map[2, 1]
        floor = sample(floor_img, fu, fv)
        light = sample(window_img, uv_c[..., 0], uv_c[..., 1]) * vc_c * gain
        atlas[oy - 4:oy - 4 + cell, ox - 4:ox - 4 + cell] = np.clip(floor + light, 0.0, 1.0)
    return atlas, new_uvs, offset


def obj_vertex_colours(path):
    """[(r, g, b)] per OBJ vertex from the '#vc' line after each 'v' line (white where a vertex has none)."""
    colours = []
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                colours.append((1.0, 1.0, 1.0))
            elif line.startswith("#vc") and colours:
                colours[-1] = tuple(float(x) for x in line.split()[1:4])
    return colours
