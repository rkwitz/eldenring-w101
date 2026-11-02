"""Dialogue portraits for the street NPCs (round 18 G3): each Elden Ring stand-in's face on Wizard101's blue NPC
portrait background inside its gold ring, as w101_orbs\\book\\art\\GUI\\NpcPortraits\\ER\\<slug>.png (hud_orbs
markers::er_portraits uses them in place of the Wizard101 picture). Personal art: never commit the output.

How:
- The heads are drawn by the book's own software FLVER renderer (hud_orbs/src/model.rs) through the preview binary
  `npc_portrait` (cargo build --release --bin npc_portrait in hud_orbs/preview), posed with the book's idle pose.
- The c0000 NPCs wear their CharaInitParam equipment (vanilla regulation, read with er_params.py) with each
  armour's invisibleFlag_SexVerNN bits (HIDE below) and their FaceParam hair and beard.
- Characters with their own model (chr) are packed as a part: the chrbnd FLVER with its _h textures. model.rs picks
  a mesh's albedo by the material's "P[NAME]" and a word of it, so each material path "C[cXXXX]_<name>" is rewritten
  in place (same length) to "P[cXXXX]_<token>" and the textures renamed "cXXXX_<token>_a"; a material without an
  albedo gets a flat colour, and one left out is named "shadow" (model.rs skips it). These are drawn in their bind
  pose (the idle pose is the c0000 skeleton's).
- The background: Wizard101's NpcPortraits pictures (128 px) all share the gold ring (radius >= 51.5 px; their
  per-pixel median is used) and a painted blue inside it, here a quadratic in x, y fitted to the blue pixels of six
  of them. Output 256 px (Wizard101 shows its 128 px portraits scaled up in the 180-unit imgPortrait the same way).

Usage (cloud container: needs Pillow, numpy and pyooz):
  python3 -I w101_npc_portraits.py <ooz> <ELDEN RING\\Game> <book\\art\\GUI\\NpcPortraits> <book\\er\\idle_pose.tsv>
                                   <npc_portrait binary> <out folder>
Files read from Game: parts\\<PARTS>_l.partsbnd.dcx, chr\\<CHRS>.chrbnd.dcx and chr\\<CHRS>_h.texbnd.dcx.
ooz: unused (DCX KRAK is read by dcx_krak.py with pyooz); the argument is kept for the old command line.

Round 24 (N1, Ryan's picks; w101_quests.BERNAHL ...): the stand-ins are now (vanilla CharaInitParam /
EquipParamProtector equipModelId and invisibleFlag_SexVerNN, FaceParam hair_partsId -> hr_a_<id>, eyebrow_partsId
-> fg_a_20<id>, beard_partsId -> fg_a_30<id>, all three drawn in the hair colour by hud_orbs model.rs):
  private_connelly: Knight Bernahl 23260 (no helm, bd_m_1331, am_m_1330, lg_m_1330; hair 9, brow 5, beard 6, 79,69,59)
  private_oryan: Castellan Edgar 23110 (no helm, bd_m_1652, am_m_1650, lg_m_1650; hair 5, brow 3, beard 2, 115,108,80)
  ceren_nightchant: Brother Corhyn 23510 (hd_m_1240, bd_m_1240, lg_m_1240; hair 112, brow 6, 233,223,175)
  olivia_dawnwillow: Finger Reader Enia c2170; lady_oriel: St. Trina c5330 (chrbnd + _h.texbnd, packed as a part by
  chr_part with a per-model material -> texture table, CHRS).
"""
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).parent))
from dcx_krak import decompress  # noqa: E402
from er_text import bnd4  # noqa: E402

# invisibleFlag_SexVerNN bits of the worn armour (EquipParamProtector rows of the CharaInitParam row)
HIDE_23260 = [6, 7, 8, 9, 19, 20, 21, 41, 42, 43, 45, 46, 47, 49, 50, 52, 54, 55, 57, 59, 66, 68, 69, 70, 73, 77, 11,
              13, 18, 25, 26, 28, 29, 30, 12, 14, 15, 16, 72]
HIDE_23510 = [17, 23, 33, 37, 60, 61, 64, 65, 75, 19, 21, 40, 41, 42, 44, 46, 47, 48, 50, 51, 53, 54, 55, 56, 57, 58,
              59, 66, 67, 68, 70, 73, 77, 12, 72]
HIDE_23110 = [6, 7, 8, 9, 10, 19, 20, 21, 40, 41, 42, 45, 46, 48, 49, 50, 51, 53, 55, 57, 59, 66, 68, 69, 70, 73, 77,
              81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 11, 13, 18, 24, 26, 28, 29, 30, 12, 14, 15, 16, 72]
# (slug, parts set, sex, hide, hair, hair colour, view (centre, height m), armour stems (+ eyebrows fg_a_20NN, beard
# fg_a_30NN), the face in the render (None: centred; else render size, centre x, y px: Trina sleeps on her side
# 0.27 m left of her root, so a wider render at the same scale is cut round her face))
NPCS = [
    ("private_connelly", "parts", "m", HIDE_23260, "9", "79,69,59", "1.6,0.45", ["bd_m_1331", "am_m_1330", "lg_m_1330", "fg_a_2005", "fg_a_3006"], None),
    ("private_oryan", "parts", "m", HIDE_23110, "5", "115,108,80", "1.6,0.45", ["bd_m_1652", "am_m_1650", "lg_m_1650", "fg_a_2003", "fg_a_3002"], None),
    ("ceren_nightchant", "parts", "m", HIDE_23510, "112", "233,223,175", "1.6,0.45", ["hd_m_1240", "bd_m_1240", "lg_m_1240", "fg_a_2006"], None),
    ("olivia_dawnwillow", "c2170", "f", None, "-", "0,0,0", "1.86,0.7", ["c2170"], None),
    ("lady_oriel", "c5330", "f", None, "-", "0,0,0", "1.25,1.1", ["c5330"], (600, 206, 344)),
]
PARTS = ["fc_m_0000", "fg_a_0100", "fg_a_1500", "bd_m_1331", "am_m_1330", "lg_m_1330", "bd_m_1652", "am_m_1650",
         "lg_m_1650", "hd_m_1240", "bd_m_1240", "lg_m_1240", "hr_a_0009", "hr_a_0005", "hr_a_0112", "fg_a_2003",
         "fg_a_2005", "fg_a_2006", "fg_a_3002", "fg_a_3006"]
# Each chr's materials -> the cXXXX_<x>_a texture it uses (a tuple: a flat colour; None: left out)
CHRS = {
    "c2170": {
        "Body": "Body", "Fabric": "Fabric", "Fabric_cloth": "Fabric", "Fraying": "Fraying",
        "Fraying_cloth": "Fraying", "Hair": (200, 196, 186), "Metal": "Body", "WP_Wand": "WP_Wand",
    },
    "c5330": {
        "Eyebrow": "Eyebrow", "Eyelash": "Eyelash", "Eyelash_Under": "Eyelash", "Fabric": "Trina_Body",
        "Face": "Face", "Hair_glow": None, "Hair_glow_quick": None, "Hair_glow_slow": None,
        "Hair_model": "Hair_model", "Hair_model_back": "Hair_model", "Mouse": "Mouth", "Plant_Body": "Plant_Body",
        "Plant_Body_2": "Plant_Parts", "Plant_Body_Veil": "Petal_color", "Trina_Body": "Trina_Body",
        "stem": "Plant_Parts",
    },
}
W101 = ["Human_WCGuard_A", "Human_WCGuard_B", "Art_Portrait_Boy_Life02", "Human_WCShopKeepF_A_NPC",
        "Portrait_Seraph_Aerial_B_NPC", "Portrait_Owl_Gamma_A_NPC"]
YAW = "0.3"
YAWS = {"olivia_dawnwillow": "0", "lady_oriel": "-0.6"}  # facing the camera
RENDER = 300
SIDE = 256


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


def chr_part(cid, chrbnd, texbnd):
    flver = bytearray(next(d for n, d in bnd4(chrbnd).items() if n.lower().endswith(".flver")))
    tpf = next(iter(bnd4(texbnd).values()))
    table = CHRS[cid]
    tex = sorted({t for t in table.values() if t}, key=str)
    token = {t: "q" + chr(ord("a") + i) for i, t in enumerate(tex)}
    for name, t in table.items():
        # Left out: the folder "matxml" becomes "shadow" (same length), which model.rs skips.
        old = f"matxml\\C[{cid}]_{name}.matxml".encode("utf-16-le")
        body = token[t] if t else "z" * len(name)
        new = f"{'matxml' if t else 'shadow'}\\P[{cid}]_{body + '_' * (len(name) - len(body))}.matxml"
        assert old in flver and len(new) * 2 == len(old), (cid, name)
        flver = flver.replace(old, new.encode("utf-16-le"))
    by = {n.lower(): d for n, d in tpf_entries(tpf)}
    textures = [(f"{cid}_{token[t]}_a", dxt1_solid(t) if isinstance(t, tuple) else by[f"{cid}_{t}_a".lower()])
                for t in tex]
    return make_bnd4([(f"{cid}.flver", bytes(flver)), (f"{cid}.tpf", make_tpf(textures))])


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


def compose(ring, coef, render, crop=None):
    k = SIDE / 128.0
    yy, xx = np.mgrid[0:SIDE, 0:SIDE]
    x, y = (xx + 0.5) / k - 0.5, (yy + 0.5) / k - 0.5
    X = np.stack([np.ones_like(x), x, y, x * x, y * y, x * y], -1)
    canvas = np.zeros((SIDE, SIDE, 4))
    canvas[..., :3] = np.clip(X @ coef, 0, 255)
    r = np.hypot(x - 63.5, y - 63.5)
    face = np.asarray(Image.open(render).convert("RGBA")).astype(float)
    cx, cy = (face.shape[1] // 2, face.shape[0] // 2) if crop is None else crop[1:]
    face = face[cy - SIDE // 2:cy + SIDE // 2, cx - SIDE // 2:cx + SIDE // 2]
    a = face[..., 3:4] / 255.0 * np.clip(52.0 - r, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - a) + face[..., :3] * a
    ring_px = np.asarray(Image.fromarray(ring.astype(np.uint8), "RGBA").resize((SIDE, SIDE), Image.BICUBIC)).astype(float)
    m = np.clip(r - 51.0, 0, 1)[..., None]
    canvas[..., :3] = canvas[..., :3] * (1 - m) + ring_px[..., :3] * m
    canvas[..., 3] = np.where(r >= 51.0, ring_px[..., 3], 255)
    return Image.fromarray(np.clip(canvas, 0, 255).astype(np.uint8), "RGBA")


def main():
    _ooz, game, portraits, idle, binary, out = sys.argv[1:7]
    game, portraits, out = Path(game), Path(portraits), Path(out)
    out.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix="npc_portraits_"))
    try:
        (work / "parts").mkdir()
        for p in PARTS:
            (work / "parts" / f"{p}_l.partsbnd.dcx").write_bytes(
                decompress((game / "parts" / f"{p}_l.partsbnd.dcx").read_bytes()))
        for cid in CHRS:
            (work / cid).mkdir()
            part = chr_part(cid, decompress((game / "chr" / f"{cid}.chrbnd.dcx").read_bytes()),
                            decompress((game / "chr" / f"{cid}_h.texbnd.dcx").read_bytes()))
            (work / cid / f"{cid}_l.partsbnd.dcx").write_bytes(part)
        ring, coef = background(portraits)
        env = dict(os.environ, IDLE=idle)
        for slug, folder, sex, hide, hair, rgb, view, armour, crop in NPCS:
            render = work / f"{slug}.png"
            bits = ".".join(map(str, hide)) if hide else "-"
            subprocess.run([binary, str(work / folder), str(render), sex, bits, hair, rgb, view, YAWS.get(slug, YAW),
                            str(crop[0] if crop else RENDER), *armour], env=env, check=True)
            compose(ring, coef, render, crop).save(out / f"{slug}.png")
            if os.environ.get("KEEP"):
                shutil.copyfile(render, out / f"{slug}.raw.png")
            print(out / f"{slug}.png")
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
