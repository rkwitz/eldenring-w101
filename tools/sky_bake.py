"""Bake Wizard101's classic Wizard City sky into one texture and build the dome that shows it in Elden Ring (numpy).

How Wizard101 draws it (zone gamedata.bin lists Skyboxes/HD_WC_Hub_Skybox_01_Layer1..6_NK.nif; 3-6 are empty):
- Layer1 is a closed 12-sided prism around the camera (r ~20 m, band from 5.8 m below to 13.9 m above the camera, a
  fan cap above and below). The band wraps WC_Skybox01_KH (1024x256 swirl sky) once around, V over the full height;
  the top cap's UVs all sit in a tiny patch at the top edge of the texture, so it is one flat purple. Texturing is
  APPLY_REPLACE (unlit), wrap S/T, trilinear (packed into the TexDesc flags 0x3200).
- Layer2 is a slightly smaller 12-sided band of WC_Skybox_Clouds_KH (256x256 white with alpha) tiled 4 times around,
  alpha blended (NiAlphaProperty 0x12ED) with vertex alpha fading its top and bottom rows to 0. A
  NiTextureTransformController scrolls its U by 1 every 33.3 s (one full turn every 133 s).
- Both are drawn camera-attached (Z buffer test and write on), so the camera always sits at their centre.

Elden Ring can't attach an asset to the camera, and our lit asset material can't scroll UVs, so `bake` renders what
the Wizard101 camera sees in every direction (layer 1, then layer 2's clouds over it, frozen at time 0) into one opaque
texture indexed by direction (U = azimuth, V = elevation), and `dome` builds a closed surface (shaped by `hull` to wrap the zone tightly) whose UVs are its
own directions from its centre. From the centre it looks exactly like Wizard101's sky, and the 12 flat panels are gone.
"""

import math
import struct

import numpy as np

# Dome centre in the street OBJ's coordinates (metres, y up): the middle of the zone (5-95 percentile box of the classic
# OBJ's vertices) at eye height. The dome's shape comes from `hull`: in each direction it sits just beyond the farthest
# Wizard101 geometry, so it hides Elden Ring's land outside the zone but keeps the backdrop trees and towers.
CENTRE = (-115.9, 2.0, -105.8)
SEGMENTS, RINGS = 120, 72  # dome grid: 3 degrees around, 2.5 degrees up
MINIMUM = (150.0, 450.0)  # dome radius where no Wizard101 geometry is near: at/below 5 degrees up, at/above 35 degrees

WIDTH, ROWS = 1024, 512  # baked texture: 1024 = the Wizard101 sky texture's own width for the full turn
ELEV_TOP, ELEV_BOTTOM = 90.0, -30.0  # elevations covered by the texture's rows; the dome below uses the last row
# Wizard101's swirl band only reaches ~35 degrees up; above it the cap is one flat purple, which on a big dome reads as a
# plain disc cut out of the sky. The bake stretches the band's upper part over the whole dome instead: a dome direction
# at elevation STRETCH[0][k] shows what Wizard101 shows at STRETCH[1][k] (unchanged up to 5 degrees, the band's top
# reaching just short of the zenith, which fades into the cap's purple).
STRETCH = ((-90.0, 5.0, 86.0, 90.0), (-90.0, 5.0, 33.0, 40.0))


def read_dds(dds):
    """RGBA float array (rows, columns, 4) in 0..1 of a DDS's first mip: DXT1 or 32-bit uncompressed (tex2dds.py's
    output for the Wizard101 sky textures)."""
    height, width = struct.unpack_from("<II", dds, 12)
    fourcc = dds[84:88]
    if fourcc == b"DXT1":
        bw, bh = max(1, width // 4), max(1, height // 4)
        raw = np.frombuffer(dds, np.uint8, bw * bh * 8, 128).reshape(bh * bw, 8)
        c = raw[:, 0:4].copy().view("<u2").astype(np.int64)  # (n, 2) endpoints
        bits = raw[:, 4:8].copy().view("<u4")[:, 0].astype(np.int64)
        rgb = np.stack([(c >> 11 & 31) * 255 / 31, (c >> 5 & 63) * 255 / 63, (c & 31) * 255 / 31], -1)  # (n, 2, 3)
        c0, c1 = rgb[:, 0], rgb[:, 1]
        four = (c[:, 0] > c[:, 1])[:, None]
        palette = np.stack([c0, c1, np.where(four, (2 * c0 + c1) / 3, (c0 + c1) / 2),
                            np.where(four, (c0 + 2 * c1) / 3, 0)], 1)  # (n, 4, 3)
        index = (bits[:, None] >> (2 * np.arange(16))) & 3  # (n, 16)
        pixels = np.take_along_axis(palette, index[:, :, None], 1)  # (n, 16, 3)
        alpha = np.where((~four) & (index == 3), 0.0, 255.0)[:, :, None]
        pixels = np.concatenate([pixels, alpha], -1).reshape(bh, bw, 4, 4, 4)
        return pixels.transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 4) / 255.0
    bits_per_pixel = struct.unpack_from("<I", dds, 88)[0]
    assert bits_per_pixel == 32, f"unsupported DDS format {fourcc!r}/{bits_per_pixel}"
    masks = struct.unpack_from("<4I", dds, 92)
    words = np.frombuffer(dds, "<u4", width * height, 128).reshape(height, width)
    channels = []
    for mask in masks:
        if not mask:
            channels.append(np.ones((height, width), np.float32))
            continue
        shift = (mask & -mask).bit_length() - 1
        channels.append(((words & mask) >> shift).astype(np.float32) / (mask >> shift))
    return np.stack(channels, -1)


def read_layer(path):
    """(corner positions (T, 3, 3), corner UVs (T, 3, 2) as image coordinates (u, row/height), corner alpha (T, 3)) of
    a sky OBJ from nif2obj; vertex alpha comes from its `#vc r g b a` lines (1 without them)."""
    positions, colours, uvs, faces = [], [], [], []
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                positions.append([float(x) for x in line.split()[1:4]])
                colours.append(1.0)
            elif line.startswith("#vc "):
                colours[-1] = float(line.split()[4])
            elif line.startswith("vt "):
                u, v = (float(x) for x in line.split()[1:3])
                uvs.append([u, 1.0 - v])  # nif2obj flips V for the OBJ; image rows run top down
            elif line.startswith("f "):
                corners = [[int(i) - 1 for i in c.split("/")[:2]] for c in line.split()[1:]]
                for k in range(1, len(corners) - 1):
                    faces.append([corners[0], corners[k], corners[k + 1]])
    faces = np.array(faces)
    positions, colours, uvs = np.array(positions), np.array(colours), np.array(uvs)
    return positions[faces[:, :, 0]], uvs[faces[:, :, 1]], colours[faces[:, :, 0]]


def directions(width=WIDTH, rows=ROWS):
    """Unit direction (rows, width, 3) of each texel centre: column = azimuth from +x towards +z, row = elevation."""
    azimuth = (np.arange(width) + 0.5) / width * 2 * math.pi
    elevation = np.radians(ELEV_TOP - (np.arange(rows) + 0.5) / rows * (ELEV_TOP - ELEV_BOTTOM))
    az, el = np.meshgrid(azimuth, elevation)
    return np.stack([np.cos(el) * np.cos(az), np.sin(el), np.cos(el) * np.sin(az)], -1)


def stretched(rays):
    """The Wizard101 directions (same shape) whose sky the dome shows in the directions `rays` (see STRETCH)."""
    elevation = np.degrees(np.arcsin(np.clip(rays[..., 1], -1, 1)))
    seen = np.radians(np.interp(elevation, *STRETCH))
    flat = np.linalg.norm(rays[..., [0, 2]], axis=-1, keepdims=True)
    horizontal = rays[..., [0, 2]] / np.maximum(flat, 1e-9)
    out = np.empty_like(rays)
    out[..., [0, 2]] = horizontal * np.cos(seen)[..., None]
    out[..., 1] = np.sin(seen)
    return out


def raycast(rays, triangles):
    """(hit triangle or -1, barycentric b1, b2) of rays (n, 3) from the origin against triangles (T, 3, 3), nearest
    hit, both faces."""
    best = np.full(len(rays), np.inf)
    hit = np.full(len(rays), -1)
    b1 = np.zeros(len(rays))
    b2 = np.zeros(len(rays))
    for t, (a, b, c) in enumerate(triangles):  # Moller-Trumbore with the ray origin at 0
        e1, e2 = b - a, c - a
        p = np.cross(rays, e2)
        det = p @ e1
        ok = np.abs(det) > 1e-12
        inv = np.where(ok, 1.0 / np.where(ok, det, 1.0), 0.0)
        s = -a
        u = (p @ s) * inv
        q = np.cross(s, e1)
        v = (rays @ q) * inv
        dist = (q @ e2) * inv
        take = ok & (u >= -1e-9) & (v >= -1e-9) & (u + v <= 1 + 1e-9) & (dist > 0) & (dist < best)
        best[take], hit[take], b1[take], b2[take] = dist[take], t, u[take], v[take]
    return hit, b1, b2


def sample(image, uv):
    """Bilinear, wrapping sample of an image (rows, columns, channels) at image coordinates uv (n, 2) in texture
    units (u across, v down)."""
    rows, columns = image.shape[:2]
    x = uv[:, 0] * columns - 0.5
    y = uv[:, 1] * rows - 0.5
    x0, y0 = np.floor(x).astype(int), np.floor(y).astype(int)
    fx, fy = (x - x0)[:, None], (y - y0)[:, None]
    x1, y1 = (x0 + 1) % columns, (y0 + 1) % rows
    x0, y0 = x0 % columns, y0 % rows
    top = image[y0, x0] * (1 - fx) + image[y0, x1] * fx
    bottom = image[y1, x0] * (1 - fx) + image[y1, x1] * fx
    return top * (1 - fy) + bottom * fy


def _layer_hits(layer, rays):
    triangles, uvs, alpha = layer
    hit, b1, b2 = raycast(rays, triangles)
    found = hit >= 0
    h = np.where(found, hit, 0)
    w = np.stack([1 - b1 - b2, b1, b2], -1)[:, :, None]
    uv = (uvs[h] * w).sum(1)
    a = (alpha[h] * w[:, :, 0]).sum(1)
    return found, h, uv, a


def bake(sky_obj, sky_dds, cloud_obj=None, cloud_dds=None, width=WIDTH, rows=ROWS):
    """RGB float image (rows, width, 3) of the Wizard101 sky seen from its centre, by direction (see `directions`), with
    the band stretched up over the sky (see STRETCH)."""
    rays = stretched(directions(width, rows)).reshape(-1, 3)
    sky = read_layer(sky_obj)
    found, hit, uv, _ = _layer_hits(sky, rays)
    assert found.all(), "the sky's first layer should enclose its centre"
    colour = sample(read_dds(sky_dds)[:, :, :3], uv)
    # Below the band Wizard101 shows its bottom cap, which samples the top edge of the texture (purple) and is always
    # under the ground there. Here the dome can show below the horizon, so continue the band's lowest colour instead.
    triangles = sky[0]
    normals = np.cross(triangles[:, 1] - triangles[:, 0], triangles[:, 2] - triangles[:, 0])
    flat = np.abs(normals[:, 1]) > 0.5 * np.linalg.norm(normals, axis=1)
    cap_below = flat & (triangles[:, :, 1].mean(1) < 0)
    colour = colour.reshape(rows, width, 3)
    below = cap_below[hit].reshape(rows, width)
    for column in range(width):
        if below[:, column].any():
            first = int(np.argmax(below[:, column]))
            colour[first:, column] = colour[max(first - 1, 0), column]
    if cloud_obj:
        found, _, uv, vertex_alpha = _layer_hits(read_layer(cloud_obj), rays)
        clouds = sample(read_dds(cloud_dds), uv)
        a = (clouds[:, 3] * np.clip(vertex_alpha, 0, 1) * found)[:, None]
        colour = colour.reshape(-1, 3) * (1 - a) + clouds[:, :3] * a
    return np.clip(colour.reshape(rows, width, 3), 0, 1)


def _dxt1_blocks(rgb):
    """DXT1 blocks (bytes) of an RGB float image (0..1) whose sides are multiples of 4."""
    rows, columns = rgb.shape[:2]
    px = (rgb * 255).reshape(rows // 4, 4, columns // 4, 4, 3).transpose(0, 2, 1, 3, 4).reshape(-1, 16, 3)
    mean = px.mean(1, keepdims=True)
    centred = px - mean
    # Endpoints: the block's extreme texels along its main colour axis (power iteration on the covariance).
    cov = np.einsum("nki,nkj->nij", centred, centred)
    axis = np.ones((len(px), 3)) / math.sqrt(3)
    for _ in range(4):
        axis = np.einsum("nij,nj->ni", cov, axis)
        axis /= np.linalg.norm(axis, axis=1, keepdims=True) + 1e-12
    proj = np.einsum("nki,ni->nk", centred, axis)
    hi = np.take_along_axis(px, proj.argmax(1)[:, None, None].repeat(3, 2), 1)[:, 0]
    lo = np.take_along_axis(px, proj.argmin(1)[:, None, None].repeat(3, 2), 1)[:, 0]

    def pack(c):
        c = np.clip(np.rint(c / 255 * [31, 63, 31]), 0, [31, 63, 31]).astype(np.int64)
        return c[:, 0] << 11 | c[:, 1] << 5 | c[:, 2]

    def unpack(c):
        return np.stack([(c >> 11 & 31) * 255 / 31, (c >> 5 & 63) * 255 / 63, (c & 31) * 255 / 31], -1)

    c0, c1 = pack(hi), pack(lo)
    swap = c0 < c1
    c0, c1 = np.where(swap, c1, c0), np.where(swap, c0, c1)
    e0, e1 = unpack(c0), unpack(c1)
    palette = np.stack([e0, e1, (2 * e0 + e1) / 3, (e0 + 2 * e1) / 3], 1)
    index = ((px[:, :, None, :] - palette[:, None, :, :]) ** 2).sum(-1).argmin(-1)
    index[c0 == c1] = 0  # one colour: 4-colour mode isn't available when the endpoints are equal
    bits = (index.astype(np.uint64) << (2 * np.arange(16, dtype=np.uint64))).sum(1)
    out = np.zeros(len(px), dtype=[("c0", "<u2"), ("c1", "<u2"), ("bits", "<u4")])
    out["c0"], out["c1"], out["bits"] = c0, c1, bits
    return out.tobytes()


def grade(rgb, target, detail=0.5):
    """The sRGB image `rgb` (rows = dome elevation, ELEV_TOP..ELEV_BOTTOM) recoloured to a reference look: each row's
    mean colour becomes `target` (sorted [(Wizard101 elevation in degrees, (r, g, b) 0..255)], interpolated, taken at
    the Wizard101 elevation that row shows, see STRETCH) and its swirls and clouds (each texel's difference from its
    row's mean) are kept at `detail` times their strength."""
    rows = rgb.shape[0]
    dome = ELEV_TOP - (np.arange(rows) + 0.5) / rows * (ELEV_TOP - ELEV_BOTTOM)
    seen = np.interp(dome, *STRETCH)
    elevations = [e for e, _ in target]
    colour = np.stack([np.interp(seen, elevations, [c[k] / 255.0 for _, c in target]) for k in range(3)], -1)
    mean = rgb.mean(1, keepdims=True)
    return np.clip(colour[:, None, :] + (rgb - mean) * detail, 0.0, 1.0)


def dim(rgb, factor, saturation=1.0):
    """The sRGB image `rgb` with its light (linear) intensity times `factor` and its colour pushed away from grey by
    `saturation` (1.0 keeps it): Elden Ring draws an unlit (emissive) texture far brighter than Wizard101 does, and its
    tone mapping washes the pastel sky out towards grey, so the sky is darkened and saturated to keep its colours."""
    rgb = np.clip(rgb, 0.0, 1.0)
    linear = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    grey = (linear @ np.array([0.2126, 0.7152, 0.0722]))[..., None]
    linear = np.clip(grey + (linear - grey) * saturation, 0.0, 1.0) * factor
    return np.where(linear <= 0.0031308, linear * 12.92, 1.055 * linear ** (1 / 2.4) - 0.055)


def dxt1_dds(rgb, dither=0.0):
    """DXT1 DDS bytes, with mipmaps down to 4 texels, of an RGB float image whose sides are powers of 2 (>= 4).
    `dither` adds that much uniform noise (+-, 0..1 units, fixed seed) to the top level first, so smooth gradients
    don't quantise into visible stripes (DXT1 endpoints are 5/6/5 bits: 1/32 steps in red and blue)."""
    rows, columns = rgb.shape[:2]
    if dither:
        rgb = np.clip(rgb + np.random.default_rng(0).uniform(-dither, dither, rgb.shape), 0.0, 1.0)
    levels, data, image = 0, bytearray(), rgb
    while min(image.shape[:2]) >= 4:
        data += _dxt1_blocks(image)
        levels += 1
        r, c = image.shape[:2]
        image = image.reshape(r // 2, 2, c // 2, 2, 3).mean((1, 3))
    pixel_format = struct.pack("<II4s5I", 32, 0x4, b"DXT1", 0, 0, 0, 0, 0)
    header = struct.pack("<4s7I44x", b"DDS ", 124, 0xA1007, rows, columns, columns * rows // 2, 0, levels)
    return header + pixel_format + struct.pack("<5I", 0x401008, 0, 0, 0, 0) + bytes(data)


def zone_points(obj_path, spacing=4.0):
    """Points (n, 3) covering a zone OBJ's surface: its vertices plus a grid on every triangle bigger than `spacing`."""
    positions, faces = [], []
    with open(obj_path) as f:
        for line in f:
            if line.startswith("v "):
                positions.append([float(x) for x in line.split()[1:4]])
            elif line.startswith("f "):
                corners = [int(c.split("/")[0]) for c in line.split()[1:]]
                corners = [c - 1 if c > 0 else len(positions) + c for c in corners]
                for k in range(1, len(corners) - 1):
                    faces.append((corners[0], corners[k], corners[k + 1]))
    return surface_points(np.array(positions), np.array(faces), spacing)


def surface_points(positions, faces, spacing=4.0):
    """Points (n, 3) covering a triangle mesh (positions (v, 3), faces (t, 3) vertex indices): its vertices plus a
    grid on every triangle bigger than `spacing`."""
    positions = np.asarray(positions, dtype=float)
    if not len(faces):
        return positions
    triangles = positions[np.asarray(faces)]
    edge = np.linalg.norm(triangles - np.roll(triangles, 1, axis=1), axis=2).max(1)
    points = [positions]
    for n in (2, 4, 8, 16, 32, 64, 128):
        pick = triangles[(edge > spacing * n / 2) & (edge <= spacing * n)] if n < 128 else triangles[edge > spacing * 32]
        for i in range(n + 1):
            for j in range(n + 1 - i):
                a, b = i / n, j / n
                points.append(pick[:, 0] * (1 - a - b) + pick[:, 1] * a + pick[:, 2] * b)
    return np.concatenate(points)


def hull(points, centre=CENTRE, segments=SEGMENTS, rings=RINGS, grow=2, margin=1.05, pad=10.0, minimum=MINIMUM,
         smoothing=((2, 1), (2.0, 1.5))):
    """Smoothed (see `smooth_envelope`) dome radius (rings + 1, segments + 1) at each vertex direction of `dome`: just beyond the farthest zone point
    near that direction (within `grow` grid steps), at least `minimum` (low, high: ramped from 5 to 35 degrees up, so
    the sky overhead is farther away and shifts less as you walk; Elden Ring land doesn't rise that high nearby). Below the horizon it keeps the horizon's
    radius, so the dome closes under the street instead of cutting through it."""
    d = points - np.array(centre)
    distance = np.linalg.norm(d, axis=1)
    keep = distance > 1e-6
    d, distance = d[keep], distance[keep]
    elevation = np.degrees(np.arcsin(np.clip(d[:, 1] / distance, -1, 1)))
    azimuth = np.degrees(np.arctan2(d[:, 2], d[:, 0])) % 360
    ring = np.clip(np.rint((90 - elevation) / 180 * rings).astype(int), 0, rings)
    segment = np.rint(azimuth / 360 * segments).astype(int) % segments
    need = np.zeros((rings + 1, segments))
    np.maximum.at(need, (ring, segment), distance)
    grown = need.copy()
    for dr in range(-grow, grow + 1):
        for ds in range(-grow, grow + 1):
            shifted = np.roll(need, ds, axis=1)
            if dr > 0:
                shifted = np.vstack([np.zeros((dr, segments)), shifted[:-dr]])
            elif dr < 0:
                shifted = np.vstack([shifted[-dr:], np.zeros((-dr, segments))])
            grown = np.maximum(grown, shifted)
    horizon = rings // 2
    grown[horizon:] = np.maximum(grown[horizon:], grown[horizon])
    ring_elevation = 90 - 180 * np.arange(rings + 1) / rings
    floor = np.interp(ring_elevation, [5.0, 35.0], minimum)[:, None]
    radius = smooth_envelope(np.maximum(grown * margin + pad, floor), *smoothing)
    radius[0], radius[-1] = radius[1].mean(), radius[-2].mean()  # one point at each pole
    return np.hstack([radius, radius[:, :1]])  # the seam column repeats the first


def _max_filter(a, segments, rings):
    """Max of `a` (rings, segments; azimuth wraps) over a window of +-segments, +-rings."""
    out = a.copy()
    for dr in range(-rings, rings + 1):
        if dr > 0:
            shifted = np.vstack([np.repeat(a[:1], dr, 0), a[:-dr]])
        elif dr < 0:
            shifted = np.vstack([a[-dr:], np.repeat(a[-1:], -dr, 0)])
        else:
            shifted = a
        for ds in range(-segments, segments + 1):
            out = np.maximum(out, np.roll(shifted, ds, 1))
    return out


def _blur(a, sigma_segments, sigma_rings):
    """Gaussian blur of `a` (rings, segments; azimuth wraps, rings clamp at the poles)."""
    def kernel(sigma):
        x = np.arange(-int(3 * sigma) - 1, int(3 * sigma) + 2)
        w = np.exp(-x * x / (2 * sigma * sigma))
        return x, w / w.sum()

    x, w = kernel(sigma_segments)
    out = sum(wi * np.roll(a, xi, 1) for xi, wi in zip(x, w))
    x, w = kernel(sigma_rings)
    n = len(x)
    padded = np.vstack([np.repeat(out[:1], n, 0), out, np.repeat(out[-1:], n, 0)])
    return sum(wi * padded[n + xi:n + xi + len(a)] for xi, wi in zip(x, w))


def smooth_envelope(need, widen=(2, 1), sigma=(2.0, 1.5)):
    """A smooth radius field (rings, segments) that is >= `need` everywhere. The raw hull follows every backdrop tree
    and jumps by up to 500 m between neighbouring 3-degree directions; seen from the street (off the dome's centre)
    those jumps are near-radial walls that read as the sky stepping and cutting off. This widens the peaks
    (by default +-6 degrees around, +-2.5 up), blurs them, re-imposes `need` and blurs again a few times, then
    scales the result up just enough to cover `need`. On Unicorn Way the largest visible discontinuity from the
    street drops from 31 degrees to under 3; wider settings (e.g. widen=(6, 3), sigma=(4, 3)) are smoother still
    but push the dome out far enough that more Elden Ring terrain and assets show above the W101 backdrop."""
    smooth = _blur(_max_filter(need, *widen), *sigma)
    for _ in range(3):
        smooth = _blur(np.maximum(smooth, need), sigma[0] * 0.75, sigma[1] * 0.67)
    smooth = _blur(np.maximum(smooth, need), sigma[0] * 0.5, sigma[1] * 0.5)
    return smooth * max(1.0, float((need / smooth).max()))


def dome(radii, centre=(0.0, 0.0, 0.0), segments=SEGMENTS, rings=RINGS):
    """(positions, uvs, faces) of the sky dome in OBJ conventions, as flver_build.parse_obj returns them: a closed
    surface around `centre` with radius radii[ring][segment] (from `hull`) in each direction, each vertex's UV taken
    from its direction (see `directions`), and every triangle facing the centre. Material None."""
    positions, uvs, faces = [], [], []
    span = ELEV_TOP - ELEV_BOTTOM
    for ring in range(rings + 1):
        elevation = 90.0 - 180.0 * ring / rings
        e = math.radians(elevation)
        ce, se = (0.0, 1.0 if ring == 0 else -1.0) if ring in (0, rings) else (math.cos(e), math.sin(e))
        row = min((ELEV_TOP - elevation) / span, 1.0 - 0.5 / ROWS)
        for segment in range(segments + 1):
            azimuth = 2 * math.pi * segment / segments
            d = (ce * math.cos(azimuth), se, ce * math.sin(azimuth))
            reach = float(radii[ring][segment])
            positions.append(tuple(c + reach * k for c, k in zip(centre, d)))
            u = segment / segments
            if ring in (0, rings):
                u = (segment + 0.5) / segments  # pole: one vertex per segment, at the segment's middle
            uvs.append((u, 1.0 - row))  # OBJ vt (V up)
    stride = segments + 1
    for ring in range(rings):
        for segment in range(segments):
            a, b = ring * stride + segment, ring * stride + segment + 1
            c, d = a + stride, b + stride
            # Seen from the centre, (a, b, d) and (a, d, c) wind counter-clockwise: their normals face inward.
            for tri in ((a, d, b), (a, c, d)):
                if len({positions[i] for i in tri}) == 3:
                    faces.append((None, tuple((i, i) for i in tri)))
    return positions, uvs, faces


def flver_mesh(data):
    """(positions (v, 3), faces (t, 3)) of every mesh of an uncompressed FLVER2 (all face sets, triangle lists or
    strips), in FLVER coordinates (OBJ with z negated)."""
    import flver_fix as ff

    _, counts = ff._counts(data)
    starts, _ = ff._section_starts(counts)
    vertex_data = struct.unpack_from("<i", data, 0x0C)[0]
    header_index_size = data[0x48]
    positions, faces, base = [], [], 0
    for i in range(counts["mesh"]):
        mesh = starts["mesh"] + i * ff.SIZES["mesh"]
        face_set_count, face_set_indices, buffer_count, buffer_indices = struct.unpack_from("<4i", data, mesh + 0x20)
        mesh_positions = None
        for b in struct.unpack_from(f"<{buffer_count}i", data, buffer_indices):
            vb = starts["vertex_buffer"] + b * ff.SIZES["vertex_buffer"]
            _, layout, vertex_size, vertex_count, _, _, _, offset = struct.unpack_from("<8i", data, vb)
            at = ff._position_offset(data, starts, layout)
            if at is None:
                continue
            raw = np.frombuffer(data, np.uint8, vertex_size * vertex_count, vertex_data + offset)
            rows = raw.reshape(vertex_count, vertex_size)[:, at:at + 12]
            mesh_positions = np.ascontiguousarray(rows).view("<f4").reshape(vertex_count, 3).astype(float)
        if mesh_positions is None:
            continue
        for f in struct.unpack_from(f"<{face_set_count}i", data, face_set_indices):
            fs = starts["face_set"] + f * ff.SIZES["face_set"]
            strip = data[fs + 4]
            index_count, index_offset = struct.unpack_from("<2i", data, fs + 0x08)
            size = struct.unpack_from("<i", data, fs + 0x18)[0] or header_index_size or 16
            index = np.frombuffer(data, "<u4" if size == 32 else "<u2", index_count, vertex_data + index_offset)
            index = index.astype(np.int64)
            if strip:
                tri = np.stack([index[:-2], index[1:-1], index[2:]], 1)
                tri = tri[(tri[:, 0] != tri[:, 1]) & (tri[:, 1] != tri[:, 2]) & (tri[:, 0] != tri[:, 2])]
                tri = tri[(tri < 0xFFFF).all(1)] if size == 16 else tri
            else:
                tri = index[:len(index) // 3 * 3].reshape(-1, 3)
            tri = tri[(tri < len(mesh_positions)).all(1)]
            faces.append(tri + base)
        positions.append(mesh_positions)
        base += len(mesh_positions)
    if not positions:
        return np.zeros((0, 3)), np.zeros((0, 3), np.int64)
    return np.concatenate(positions), (np.concatenate(faces) if faces else np.zeros((0, 3), np.int64))


def to_street(points, part_position, part_rotation_y, street_position, street_rotation_y):
    """FLVER-space points (n, 3) of an MSB part (position, y rotation in degrees) in the street OBJ's coordinates
    (the frame of CENTRE and `hull`): MSB parts map FLVER (x, y, z) to position + (x cos + z sin, y, -x sin + z cos);
    this goes to the world and back through the street part's transform, then negates z (FLVER -> OBJ)."""
    def rotate(p, degrees):
        c, s = math.cos(math.radians(degrees)), math.sin(math.radians(degrees))
        return np.stack([p[:, 0] * c + p[:, 2] * s, p[:, 1], -p[:, 0] * s + p[:, 2] * c], 1)

    world = rotate(np.asarray(points, dtype=float), part_rotation_y) + np.asarray(part_position)
    local = rotate(world - np.asarray(street_position), -street_rotation_y)
    return local * [1.0, 1.0, -1.0]


def dome_radius_at(points, radii, centre=CENTRE, segments=SEGMENTS, rings=RINGS):
    """(distance from `centre`, dome radius in that direction) for points (n, 3) in street OBJ coordinates, the radius
    interpolated bilinearly over the `hull` grid (the dome's flat triangles sag inward by under 0.1%)."""
    d = np.asarray(points, dtype=float) - np.array(centre)
    distance = np.linalg.norm(d, axis=1)
    elevation = np.degrees(np.arcsin(np.clip(d[:, 1] / np.maximum(distance, 1e-9), -1, 1)))
    azimuth = np.degrees(np.arctan2(d[:, 2], d[:, 0])) % 360
    r = np.clip((90 - elevation) / 180 * rings, 0, rings)
    s = azimuth / 360 * segments
    r0, s0 = np.minimum(np.floor(r).astype(int), rings - 1), np.floor(s).astype(int) % segments
    fr, fs = r - r0, s - np.floor(s)
    grid = np.asarray(radii)
    a, b = grid[r0, s0], grid[r0, (s0 + 1) % segments]
    c, e = grid[r0 + 1, s0], grid[r0 + 1, (s0 + 1) % segments]
    radius = (a * (1 - fs) + b * fs) * (1 - fr) + (c * (1 - fs) + e * fs) * fr
    return distance, radius * 0.999


SEGMENT_DIGITS = {"0": "abcdef", "1": "bc", "2": "abdeg", "3": "abcdg", "4": "bcfg", "5": "acdfg", "6": "acdefg",
                  "7": "abc", "8": "abcdefg", "9": "abcdfg"}


def draw_labels(rgb, labels, elevation=(14.0, 30.0), colour=(1.0, 1.0, 1.0), outline=(0.15, 0.1, 0.3)):
    """The image `rgb` (rows = dome elevation ELEV_TOP..ELEV_BOTTOM, columns = azimuth) with each label (a string of
    digits) drawn as big seven-segment numbers centred at its azimuth (degrees, as `directions`) between two dome
    elevations, white with a dark outline so it reads on any brightness."""
    out = rgb.copy()
    rows, columns = rgb.shape[:2]
    top = int((ELEV_TOP - elevation[1]) / (ELEV_TOP - ELEV_BOTTOM) * rows)
    bottom = int((ELEV_TOP - elevation[0]) / (ELEV_TOP - ELEV_BOTTOM) * rows)
    height = bottom - top
    width, stroke = height * 11 // 20, max(3, height // 6)
    for azimuth, text in labels:
        mask = np.zeros((height, (width + stroke * 2) * len(text)), bool)
        for k, ch in enumerate(text):
            x0 = k * (width + stroke * 2) + stroke
            h2 = height // 2
            bars = {"a": (0, stroke, x0, x0 + width), "g": (h2 - stroke // 2, h2 + stroke - stroke // 2, x0, x0 + width),
                    "d": (height - stroke, height, x0, x0 + width), "f": (0, h2, x0, x0 + stroke),
                    "b": (0, h2, x0 + width - stroke, x0 + width), "e": (h2, height, x0, x0 + stroke),
                    "c": (h2, height, x0 + width - stroke, x0 + width)}
            for seg in SEGMENT_DIGITS[ch]:
                r0, r1, c0, c1 = bars[seg]
                mask[r0:r1, c0:c1] = True
        padded = np.pad(mask, 2)
        grown = np.zeros_like(mask)
        for dr in range(5):
            for dc in range(5):
                grown |= padded[dr:dr + mask.shape[0], dc:dc + mask.shape[1]]
        left = int(round(azimuth / 360 * columns)) - mask.shape[1] // 2
        cols = (left + np.arange(mask.shape[1])) % columns
        region = out[top:bottom][:, cols]
        region[grown] = outline
        region[mask] = colour
        out[top:bottom, cols] = region
    return out


def face_sectors(faces, count, segments=SEGMENTS):
    """For `dome` faces, the azimuth sector (0..count-1, equal slices from azimuth 0 towards +z) of each face, from its
    first vertex's segment (dome vertices are (segments + 1) per ring)."""
    per = segments // count
    return [((corners[0][0] % (segments + 1)) % segments) // per for _, corners in faces]
