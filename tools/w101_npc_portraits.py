"""Dialogue portraits for the street NPCs (round 18 G3): each Elden Ring stand-in's face on Wizard101's blue NPC
portrait background inside its gold ring, as w101_orbs\\book\\art\\GUI\\NpcPortraits\\ER\\<slug>.png (hud_orbs
markers::er_portraits uses them in place of the Wizard101 picture). Personal art: never commit the output.

How:
- The heads are drawn by the book's own software FLVER renderer (hud_orbs/src/model.rs) through the preview binary
  `npc_portrait` (cargo build --release --bin npc_portrait in hud_orbs/preview), posed with the book's idle pose.
- The c0000 NPCs wear their CharaInitParam equipment (vanilla regulation, read with er_params.py): Private Connelly
  and Private O'Ryan row 6194 (head/body/arms/legs model 2350), Ceren Nightchant 6164 (head/body 2130), Olivia
  Dawnwillow 23201 (body/arms/legs 1390, female), with each armour's invisibleFlag_SexVerNN bits (HIDE below) and
  FaceParam hair (row 1: hair 101 55,45,35; row 23200: hair 105 233,219,151).
- Lady Oriel is Melina (c2180): her chrbnd FLVER with her c2180_h textures, packed as a part. model.rs picks a
  mesh's albedo by the material's "P[NAME]" and a word of it, so each material path "C[c2180]_<name>" is rewritten in
  place (same length) to "P[c2180]_<token>" and the textures renamed "c2180_<token>_a"; her hair (tinted in the
  shader, no albedo) gets a flat texture of her eyebrows' colour; the eye occlusion shell is named "shadow" (left
  out by model.rs).
- The background: Wizard101's NpcPortraits pictures (128 px) all share the gold ring (radius >= 51.5 px; their
  per-pixel median is used) and a painted blue inside it, here a quadratic in x, y fitted to the blue pixels of six
  of them. Output 256 px (Wizard101 shows its 128 px portraits scaled up in the 180-unit imgPortrait the same way).

Usage (cloud container: needs Pillow, numpy and an `ooz` command-line build for DCX KRAK):
  python3 -I w101_npc_portraits.py <ooz> <ELDEN RING\\Game> <book\\art\\GUI\\NpcPortraits> <book\\er\\idle_pose.tsv>
                                   <npc_portrait binary> <out folder>
Files read from Game: parts\\{fc_m_0000,fc_f_0000,fg_a_0100,fg_a_1500,hd_m_2350,bd_m_2350,am_m_2350,lg_m_2350,
hd_m_2130,bd_m_2130,bd_f_1390,am_m_1390,lg_m_1390,hr_a_0101,hr_a_0105}_l.partsbnd.dcx, chr\\c2180.chrbnd.dcx,
chr\\c2180_h.texbnd.dcx.
"""
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from io import BytesIO
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from er_text import bnd4  # noqa: E402

# invisibleFlag_SexVerNN bits of the worn armour (EquipParamProtector rows of the CharaInitParam row)
HIDE_6194 = [3, 4, 32, 34, 37, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 75, 76, 79, 95, 5, 6, 7, 8, 9, 10, 19, 20, 21,
             40, 41, 43, 44, 46, 48, 49, 50, 51, 54, 55, 56, 57, 59, 70, 73, 77, 11, 13, 18, 24, 26, 28, 29, 31, 12, 14,
             15, 16, 72]
HIDE_6164 = [32, 34, 37, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 75, 76, 5, 6, 7, 8, 9, 19, 20, 21, 40, 42, 43, 45, 46,
             48, 49, 50, 51, 53, 54, 55, 56, 57, 58, 59, 73, 77, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94]
HIDE_23201 = [5, 6, 7, 8, 9, 19, 20, 21, 41, 42, 43, 45, 47, 48, 49, 50, 51, 54, 55, 56, 57, 59, 67, 68, 69, 73, 77, 11,
              13, 18, 24, 26, 28, 29, 31, 12, 14, 15, 16, 72]
# (slug, parts set, sex, hide, hair, hair colour, view (centre, height m), armour stems)
NPCS = [
    ("private_connelly", "parts", "m", HIDE_6194, "101", "55,45,35", "1.6,0.45", ["hd_m_2350", "bd_m_2350", "am_m_2350", "lg_m_2350"]),
    ("ceren_nightchant", "parts", "m", HIDE_6164, "101", "55,45,35", "1.57,0.45", ["hd_m_2130", "bd_m_2130"]),
    ("olivia_dawnwillow", "parts", "f", HIDE_23201, "105", "233,219,151", "1.53,0.45", ["bd_m_1390", "am_m_1390", "lg_m_1390"]),
    ("lady_oriel", "melina", "f", None, "-", "0,0,0", "1.53,0.42", ["c2180"]),
]
SAME_LOOK = {"private_oryan": "private_connelly"}  # same CharaInitParam row
PARTS = ["fc_m_0000", "fc_f_0000", "fg_a_0100", "fg_a_1500", "hd_m_2350", "bd_m_2350", "am_m_2350", "lg_m_2350",
         "hd_m_2130", "bd_m_2130", "bd_f_1390", "am_m_1390", "lg_m_1390", "hr_a_0101", "hr_a_0105"]
W101 = ["Human_WCGuard_A", "Human_WCGuard_B", "Art_Portrait_Boy_Life02", "Human_WCShopKeepF_A_NPC",
        "Portrait_Seraph_Aerial_B_NPC", "Portrait_Owl_Gamma_A_NPC"]
YAW = "0.3"
RENDER = 300
SIDE = 256


def undcx(ooz, path, tmp):
    """The payload of a DCX KRAK file (ooz CLI: an 8-byte size header then the stream)."""
    b = Path(path).read_bytes()
    size, packed = struct.unpack_from(">II", b, 0x1C)
    src, dst = Path(tmp) / "x.krak", Path(tmp) / "x.out"
    src.write_bytes(struct.pack("<Q", size) + b[0x4C:0x4C + packed])
    subprocess.run([ooz, "-d", "-f", str(src), str(dst)], check=True, capture_output=True)
    out = dst.read_bytes()
    assert len(out) == size, path
    return out


def make_bnd4(entries):
    """A minimal uncompressed BND4 (ASCII names) as model.rs reads it."""
    eh = 0x24
    names, name_off = b"", []
    base = 0x40 + eh * len(entries)
    for n, _ in entries:
        name_off.append(base + len(names))
        names += n.encode("ascii") + b"\0"
    data_start = (base + len(names) + 15) // 16 * 16
    head = bytearray(b"BND4" + b"\0" * 0x3C)
    struct.pack_into("<i", head, 0x0C, len(entries))
    struct.pack_into("<Q", head, 0x20, eh)
    ents, body = bytearray(), bytearray()
    for i, (_, d) in enumerate(entries):
        ents += struct.pack("<BxxxiQQIiI", 0x40, -1, len(d), len(d), data_start + len(body), i, name_off[i])
        body += d + b"\0" * ((-len(d)) % 16)
    blob = bytes(head) + bytes(ents) + names
    return blob + b"\0" * (data_start - len(blob)) + bytes(body)


def tpf_entries(tpf):
    count = struct.unpack_from("<i", tpf, 8)[0]
    out = []
    for i in range(count):
        o = 0x10 + 20 * i
        off, size = struct.unpack_from("<II", tpf, o)
        at = end = struct.unpack_from("<I", tpf, o + 12)[0]
        while tpf[end:end + 2] != b"\0\0":
            end += 2
        out.append((tpf[at:end].decode("utf-16-le"), bytes(tpf[off:off + size])))
    return out


def make_tpf(textures):
    names = [n.encode("utf-16-le") + b"\0\0" for n, _ in textures]
    n_at = 0x10 + 20 * len(textures)
    data_at = (n_at + sum(len(x) for x in names) + 15) // 16 * 16
    ent, data = bytearray(), bytearray()
    for (_, d), nb in zip(textures, names):
        ent += struct.pack("<IIBBBBII", data_at + len(data), len(d), 0, 0, 0, 0, n_at, 0)
        n_at += len(nb)
        data += d + b"\0" * ((-len(d)) % 16)
    head = bytearray(b"TPF\0" + b"\0" * 12)
    struct.pack_into("<IiBBBB", head, 4, len(data), len(textures), 0, 0, 0, 1)
    blob = bytes(head) + bytes(ent) + b"".join(names)
    return blob + b"\0" * (data_at - len(blob)) + bytes(data)


def dxt1_solid(rgb, side=16):
    r, g, b = rgb
    c = (r >> 3) << 11 | (g >> 2) << 5 | b >> 3
    head = bytearray(128)
    head[0:4] = b"DDS "
    struct.pack_into("<IIIII", head, 4, 124, 0x1007, side, side, 0)
    struct.pack_into("<II", head, 76, 32, 4)
    head[84:88] = b"DXT1"
    return bytes(head) + struct.pack("<HHI", c, c, 0) * ((side // 4) ** 2)


# Melina's materials -> the c2180_<x>_a texture each uses (None: left out)
MELINA = {
    "AM_Gloves": "am", "Stitch": "bd_stitch", "BD_Fabrick_Robe": "bd", "BD_Fabrick_Inner": "bd", "BD_Metal": "bd",
    "BD_Mant_Fabric": "bd_mant", "BD_Mant_Metal": "bd_mant", "BD_Fabrick_Robe_cloth": "bd",
    "BD_Fabrick_Inner_cloth": "bd", "BD_Mant_Fabric_cloth": "bd_mant", "Hair_ba1": "hair", "Hair_bu1": "hair",
    "LG_Pants": "lg", "LG_Boots": "lg", "Face_SSS": "face", "Body_SSS": "body", "Eye": "eye",
    "Eye_Occulusion": None, "Mouse_SSS": "mouth", "Eyelash": "eyelash",
}


def melina_part(chrbnd, texbnd):
    flver = bytearray(next(d for n, d in bnd4(chrbnd).items() if n.lower().endswith(".flver")))
    tpf = next(iter(bnd4(texbnd).values()))
    tex = sorted({t for t in MELINA.values() if t})
    token = {t: "q" + chr(ord("a") + i) for i, t in enumerate(tex)}
    for name, t in MELINA.items():
        old = f"C[c2180]_{name}.matxml".encode("utf-16-le")
        body = token[t] if t else "shadow"
        new = f"P[c2180]_{body + '_' * (len(name) - len(body))}.matxml".encode("utf-16-le")
        flver = flver.replace(old, new)
    by = {n.lower(): d for n, d in tpf_entries(tpf)}
    brow = Image.open(BytesIO(by["c2180_eyebrow_a"])).convert("RGBA")
    px = np.asarray(brow).reshape(-1, 4)
    hair = tuple(int(v) for v in px[px[:, 3] > 128][:, :3].mean(0))
    textures = [(f"c2180_{token[t]}_a", by[f"c2180_{t}_a"] if t != "hair" else dxt1_solid(hair)) for t in tex]
    return make_bnd4([("c2180.flver", bytes(flver)), ("c2180.tpf", make_tpf(textures))])


def background(portraits):
    """(ring RGBA 128 px median, quadratic colour coefficients) from Wizard101's portraits."""
    ims = np.stack([np.asarray(Image.open(portraits / f"{n}.png").convert("RGBA")).astype(float) for n in W101])
    yy, xx = np.mgrid[0:128, 0:128]
    r = np.hypot(xx - 63.5, yy - 63.5)
    pts, cols = [], []
    for im in ims:
        R, G, B = im[..., 0], im[..., 1], im[..., 2]
        blue = (B > R + 60) & (B > G + 25) & (r < 51)
        pts.append(np.stack([xx[blue], yy[blue]], 1))
        cols.append(im[blue][:, :3])
    pts, cols = np.concatenate(pts).astype(float), np.concatenate(cols)
    X = np.stack([np.ones(len(pts)), pts[:, 0], pts[:, 1], pts[:, 0] ** 2, pts[:, 1] ** 2, pts[:, 0] * pts[:, 1]], 1)
    coef = np.linalg.lstsq(X, cols, rcond=None)[0]
    return np.median(ims, 0), coef


def compose(ring, coef, render):
    k = SIDE / 128.0
    yy, xx = np.mgrid[0:SIDE, 0:SIDE]
    x, y = (xx + 0.5) / k - 0.5, (yy + 0.5) / k - 0.5
    X = np.stack([np.ones_like(x), x, y, x * x, y * y, x * y], -1)
    canvas = np.zeros((SIDE, SIDE, 4))
    canvas[..., :3] = np.clip(X @ coef, 0, 255)
    r = np.hypot(x - 63.5, y - 63.5)
    face = np.asarray(Image.open(render).convert("RGBA")).astype(float)
    o = (face.shape[0] - SIDE) // 2
    face = face[o:o + SIDE, o:o + SIDE]
    a = face[..., 3:4] / 255.0 * np.clip(52.0 - r, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - a) + face[..., :3] * a
    ring_px = np.asarray(Image.fromarray(ring.astype(np.uint8), "RGBA").resize((SIDE, SIDE), Image.BICUBIC)).astype(float)
    m = np.clip(r - 51.0, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - m) + ring_px[..., :3] * m
    canvas[..., 3] = np.where(r >= 51.0, ring_px[..., 3], 255)
    return Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8), "RGBA")


def main():
    ooz, game, portraits, idle, binary, out = sys.argv[1:7]
    game, portraits, out = Path(game), Path(portraits), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="npc_portraits_"))
    try:
        (work / "parts").mkdir()
        (work / "melina").mkdir()
        for p in PARTS:
            (work / "parts" / f"{p}_l.partsbnd.dcx").write_bytes(undcx(ooz, game / "parts" / f"{p}_l.partsbnd.dcx", work))
        part = melina_part(undcx(ooz, game / "chr" / "c2180.chrbnd.dcx", work),
                           undcx(ooz, game / "chr" / "c2180_h.texbnd.dcx", work))
        (work / "melina" / "c2180_l.partsbnd.dcx").write_bytes(part)
        ring, coef = background(portraits)
        env = dict(os.environ, IDLE=idle)
        for slug, folder, sex, hide, hair, rgb, view, armour in NPCS:
            render = work / f"{slug}.png"
            bits = ".".join(map(str, hide)) if hide else "-"
            subprocess.run([binary, str(work / folder), str(render), sex, bits, hair, rgb, view, YAW, str(RENDER),
                            *armour], env=env, check=True)
            compose(ring, coef, render).save(out / f"{slug}.png")
            print(out / f"{slug}.png")
        for slug, same in SAME_LOOK.items():
            shutil.copyfile(out / f"{same}.png", out / f"{slug}.png")
            print(out / f"{slug}.png")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
