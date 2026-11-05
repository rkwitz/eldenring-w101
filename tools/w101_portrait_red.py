"""Round 26 (W8): boss portraits on Wizard101's red enemy disc instead of the blue NPC one.

Wizard101 draws creatures' portraits (the quest goal pictures, ER/skeletal_pirate.png, ER/lost_soul.png, its own
Fairy_DarkMinion_MOB.png, Art_Portrait_GhostLady_Green.png) on a red disc inside the gold ring, NPCs on a blue one.
w101_npc_portraits.py composed the bosses' Elden Ring faces (rattlebones.png, lady_blackhope.png) on the blue one;
Ryan wants the bosses red like the enemy pictures. This swaps the background: every pixel inside the ring whose blue
stands out (B - max(R, G), the painted blue background) is replaced by the red disc of an enemy picture at the same
place (scaled to the portrait's size), blended by how blue it was, so the face and the ring stay as they are.

Usage: python w101_portrait_red.py <NpcPortraits folder> rattlebones lady_blackhope ...
  reads <folder>/ER/<slug>.png and the red disc of <folder>/ER/skeletal_pirate.png; writes <folder>/ER/<slug>.png
  (keeping the first original as <slug>.png.before_round26). W101/ER art: never commit the pictures.
"""
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image

RING = 0.80  # inside this fraction of the half-size the background is painted (the gold ring starts ~51.5/64)
BLUE_LO, BLUE_HI = 25.0, 70.0  # B - max(R, G): below LO kept, above HI fully replaced, blended between


def recolour(portrait, red):
    im = np.asarray(portrait.convert("RGBA")).astype(np.float32)
    h, w = im.shape[:2]
    disc = np.asarray(red.convert("RGBA").resize((w, h), Image.LANCZOS)).astype(np.float32)
    yy, xx = np.mgrid[0:h, 0:w]
    r = np.hypot(xx - (w - 1) / 2, yy - (h - 1) / 2) / (w / 2)
    blue = im[..., 2] - np.maximum(im[..., 0], im[..., 1])
    k = np.clip((blue - BLUE_LO) / (BLUE_HI - BLUE_LO), 0.0, 1.0) * (r < RING)
    # The red picture's own creature covers its disc's middle: take the disc's colour by radius (its median per
    # ring of radius), which keeps the painted shading from the centre out.
    red_px = disc[..., :3]
    redness = red_px[..., 0] - np.maximum(red_px[..., 1], red_px[..., 2])
    bins = np.minimum((r * 64).astype(int), 63)
    by_r = np.zeros((64, 3), np.float32)
    for b in range(64):
        sel = (bins == b) & (redness > 60) & (r < RING)
        by_r[b] = np.median(red_px[sel], axis=0) if sel.sum() > 8 else (by_r[b - 1] if b else (200, 40, 40))
    bg = by_r[bins]
    out = im.copy()
    out[..., :3] = im[..., :3] * (1 - k[..., None]) + bg * k[..., None]
    return Image.fromarray(out.clip(0, 255).astype(np.uint8)), float(k.mean())


def main():
    folder = Path(sys.argv[1])
    red = Image.open(folder / "ER" / "skeletal_pirate.png")
    for slug in sys.argv[2:]:
        path = folder / "ER" / f"{slug}.png"
        keep = path.with_name(path.name + ".before_round26")
        if not keep.exists():
            shutil.copy2(path, keep)
        img, share = recolour(Image.open(keep), red)
        img.save(path)
        print(f"{path}: {share:.0%} of the picture recoloured red")


if __name__ == "__main__":
    main()
