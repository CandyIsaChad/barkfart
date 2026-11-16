"""Draws the placeholder app icon (original art) and writes the iOS sizes.

Usage: python3 -I tools/make_icon.py <repo root>
"""
import math
import os
import sys
from PIL import Image, ImageDraw

ORANGE = (255, 122, 26)
WHITE = (255, 255, 255)
PUFF = (255, 214, 170)
S = 1024


def limb(d, a, b, w):
    d.line([a, b], fill=WHITE, width=w)
    r = w // 2
    for (x, y) in (a, b):
        d.ellipse([x - r, y - r, x + r, y + r], fill=WHITE)


def master():
    img = Image.new("RGB", (S, S), ORANGE)
    d = ImageDraw.Draw(img)
    # Puff cloud behind the figure.
    for cx, cy, r in ((250, 700, 95), (175, 610, 70), (330, 790, 60), (150, 760, 55)):
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=PUFF)
    w = 64
    hip = (560, 600)
    neck = (600, 380)
    limb(d, hip, neck, w)                      # torso
    limb(d, neck, (760, 470), w)               # front arm
    limb(d, (760, 470), (820, 360), w)
    limb(d, neck, (470, 470), w)               # back arm
    limb(d, (470, 470), (420, 560), w)
    limb(d, hip, (700, 730), w)                # front leg
    limb(d, (700, 730), (690, 870), w)
    limb(d, hip, (470, 740), w)                # back leg
    limb(d, (470, 740), (370, 820), w)
    hr = 95
    hx, hy = 625, 255
    d.ellipse([hx - hr, hy - hr, hx + hr, hy + hr], fill=WHITE)
    return img


def main(root):
    img = master()
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
