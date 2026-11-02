"""Round 26 (N6): the button pictures of the HUD's "Press [key] to Talk" prompt (hud_orbs/src/prompt.rs), drawn here
from scratch (no Microsoft or Wizard101 art), 128 px RGBA, supersampled 4x:

  Art_XButton_A.png / _B / _X / _Y   an Xbox-style face button: a filled circle in the button's colour (A green,
                                     B red, X blue, Y yellow) with a lighter top, a darker rim and a white letter with a
                                     soft shadow, as the real controller prompt (Ryan: "green circle, white/grey A")
  Art_XButton_Blank.png              the same in dark grey, for any other pad input (LB, RB, LT, RT, the d-pad, the
                                     sticks); prompt.rs writes its label on it
  Art_Key_Blank.png                  a keyboard key cap without the black outline of Wizard101's Art_Keyboard_Blank
                                     (Ryan: "the E key art has a thin black border"): off-white top face, light grey
                                     bevel, darker front lip; prompt.rs writes the key's letter on it

Usage: python3 w101_prompt_glyphs.py <out folder>   (then copy to w101_orbs\\book\\art\\GUI\\Art\\ on the PC)
Needs Pillow and a bold sans font (DejaVuSans-Bold.ttf; --font PATH to choose another).
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

SIZE, SS = 128, 4
BUTTONS = {"A": (92, 184, 46), "B": (226, 44, 46), "X": (36, 120, 214), "Y": (240, 190, 22), "Blank": (70, 72, 76)}


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def font(path, px):
    return ImageFont.truetype(path, px)


def face_button(letter, colour, font_path):
    n = SIZE * SS
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad = 6 * SS
    rim = mix(colour, (0, 0, 0), 0.45)
    d.ellipse([pad, pad, n - pad, n - pad], fill=rim + (255,))
    inner = pad + 5 * SS
    # vertical gradient fill: lighter at the top
    grad = Image.new("RGBA", (n, n))
    gd = ImageDraw.Draw(grad)
    for y in range(n):
        t = (y - inner) / max(1, n - 2 * inner)
        gd.line([(0, y), (n, y)], fill=mix(mix(colour, (255, 255, 255), 0.30), mix(colour, (0, 0, 0), 0.12),
                                           min(1, max(0, t))) + (255,))
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).ellipse([inner, inner, n - inner, n - inner], fill=255)
    img.paste(grad, (0, 0), mask)
    # a soft gloss on the upper half
    gloss = Image.new("L", (n, n), 0)
    ImageDraw.Draw(gloss).ellipse([inner + 12 * SS, inner + 4 * SS, n - inner - 12 * SS, n // 2], fill=70)
    gloss = gloss.filter(ImageFilter.GaussianBlur(6 * SS))
    img.paste(Image.new("RGBA", (n, n), (255, 255, 255, 255)), (0, 0), Image.composite(gloss, Image.new("L", (n, n), 0), mask))
    if letter != "Blank":
        f = font(font_path, 66 * SS)
        box = d.textbbox((0, 0), letter, font=f)
        x = (n - (box[2] - box[0])) / 2 - box[0]
        y = (n - (box[3] - box[1])) / 2 - box[1]
        shadow = Image.new("L", (n, n), 0)
        ImageDraw.Draw(shadow).text((x + 2 * SS, y + 3 * SS), letter, font=f, fill=150)
        shadow = shadow.filter(ImageFilter.GaussianBlur(2 * SS))
        img.paste(Image.new("RGBA", (n, n), mix(colour, (0, 0, 0), 0.6) + (255,)), (0, 0), shadow)
        ImageDraw.Draw(img).text((x, y), letter, font=f, fill=(255, 255, 255, 255))
    return img.resize((SIZE, SIZE), Image.LANCZOS)


def key_cap():
    n = SIZE * SS
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    r = 22 * SS
    # front lip (darker), then the bevel, then the top face: no outline
    d.rounded_rectangle([8 * SS, 10 * SS, n - 8 * SS, n - 6 * SS], r, fill=(150, 146, 140, 255))
    d.rounded_rectangle([8 * SS, 8 * SS, n - 8 * SS, n - 16 * SS], r, fill=(200, 196, 190, 255))
    top = Image.new("RGBA", (n, n))
    td = ImageDraw.Draw(top)
    for y in range(n):
        t = y / n
        td.line([(0, y), (n, y)], fill=mix((252, 250, 246), (226, 222, 216), t) + (255,))
    mask = Image.new("L", (n, n), 0)
    ImageDraw.Draw(mask).rounded_rectangle([17 * SS, 14 * SS, n - 17 * SS, n - 26 * SS], r - 6 * SS, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(1.5 * SS))
    img.paste(top, (0, 0), mask)
    return img.resize((SIZE, SIZE), Image.LANCZOS)


def main(argv):
    font_path = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
    if "--font" in argv:
        font_path = argv[argv.index("--font") + 1]
    out = Path(argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for letter, colour in BUTTONS.items():
        face_button(letter, colour, font_path).save(out / f"Art_XButton_{letter}.png")
    key_cap().save(out / "Art_Key_Blank.png")
    print(f"{out}: Art_XButton_A/B/X/Y/Blank.png, Art_Key_Blank.png")


if __name__ == "__main__":
    main(sys.argv)
