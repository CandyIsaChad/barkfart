"""Builds the app icon from the user's own sketch (art/icon-sketch.jpg).

Steps: crop the drawing canvas out of the Procreate screenshot, snap every
pixel to the sketch's three colours (removes JPEG noise), add a margin so iOS's
rounded corners do not clip the feet, then downscale to each icon size.

Usage: python3 -I tools/make_icon.py <repo root>
"""
import os
import sys
from PIL import Image

CANVAS = (416, 100, 2205, 1891)          # inner box of the orange canvas
ORANGE = (241, 174, 93)
WHITE = (252, 252, 252)
BLACK = (14, 14, 14)
MARGIN = 0.06                             # fraction of the icon on each side


def snap(img):
    """Maps each pixel to the nearest of the three palette colours."""
    pal = Image.new("P", (1, 1))
    flat = list(ORANGE) + list(WHITE) + list(BLACK)
    pal.putpalette(flat + [0] * (768 - len(flat)))
    return img.quantize(palette=pal, dither=Image.Dither.NONE).convert("RGB")


def master(root):
    src = Image.open(os.path.join(root, "art", "icon-sketch.jpg")).convert("RGB")
    art = snap(src.crop(CANVAS))
    side = max(art.size)
    full = int(round(side / (1 - 2 * MARGIN)))
    out = Image.new("RGB", (full, full), ORANGE)
    out.paste(art, ((full - art.size[0]) // 2, (full - art.size[1]) // 2))
    return out.resize((1024, 1024), Image.LANCZOS)


def main(root):
    img = master(root)
    out = os.path.join(root, "ios", "data")
    sizes = {
        "AppIcon60x60@2x.png": 120,
        "AppIcon60x60@3x.png": 180,
        "AppIcon76x76@2x~ipad.png": 152,
        "AppIcon83.5x83.5@2x~ipad.png": 167,
    }
    for name, px in sizes.items():
        img.resize((px, px), Image.LANCZOS).save(os.path.join(out, name))
    img.save(os.path.join(root, "art", "icon-1024.png"))
    print("wrote", ", ".join(sizes))


if __name__ == "__main__":
    main(sys.argv[1])
