"""Draws the placeholder app icon and writes the iOS sizes.

Original art after the user's sketch: a white fox-like stick figure with a :3
mouth, waving, with a black offset backdrop shadow, on orange.

Usage: python3 -I tools/make_icon.py <repo root>
"""
import math
import os
import sys
from PIL import Image, ImageDraw

ORANGE = (242, 172, 92)
WHITE = (252, 252, 252)
BLACK = (12, 12, 12)
SS = 4                      # supersampling factor
S = 1024 * SS
SHADOW = (-14, 12)          # backdrop offset in 1024 space
LIMB = 52


def P(x, y, off=(0, 0)):
    return ((x + off[0]) * SS, (y + off[1]) * SS)


def bezier(pts, n=40):
    """Samples a cubic Bezier chain: p0 c1 c2 p1 c1 c2 p2 ..."""
    out = []
    for i in range(0, len(pts) - 3, 3):
        p0, c1, c2, p1 = pts[i:i + 4]
        for k in range(n):
            t = k / n
            u = 1 - t
            out.append((u**3 * p0[0] + 3*u*u*t * c1[0] + 3*u*t*t * c2[0] + t**3 * p1[0],
                        u**3 * p0[1] + 3*u*u*t * c1[1] + 3*u*t*t * c2[1] + t**3 * p1[1]))
    out.append(pts[-1])
    return out


def limb(d, a, b, w, col, off):
    d.line([P(*a, off), P(*b, off)], fill=col, width=w * SS)
    r = w * SS / 2
    for (x, y) in (P(*a, off), P(*b, off)):
        d.ellipse([x - r, y - r, x + r, y + r], fill=col)


def chain(d, pts, w, col, off):
    for a, b in zip(pts, pts[1:]):
        limb(d, a, b, w, col, off)


def blob(d, cx, cy, rx, ry, col, off):
    x, y = P(cx, cy, off)
    d.ellipse([x - rx * SS, y - ry * SS, x + rx * SS, y + ry * SS], fill=col)


def poly(d, pts, col, off):
    d.polygon([P(x, y, off) for (x, y) in pts], fill=col)


def figure(d, col, off):
    # Tail: bushy teardrop sweeping down-left from the hip, with tufts at the tip.
    tail = bezier([(520, 470), (430, 470), (330, 560), (290, 650),
                   (250, 700), (180, 700), (150, 690),
                   (200, 740), (330, 770), (470, 700),
                   (560, 640), (580, 540), (520, 470)])
    poly(d, tail, col, off)
    for tuft in (((260, 650), (135, 648), (240, 690)),
                 ((240, 680), (120, 695), (250, 715)),
                 ((265, 705), (150, 742), (300, 735))):
        poly(d, tuft, col, off)
    # Legs, with rounded feet.
    chain(d, [(530, 480), (450, 760), (380, 960)], LIMB, col, off)
    chain(d, [(545, 480), (620, 700), (690, 905)], LIMB, col, off)
    blob(d, 375, 975, 32, 26, col, off)
    blob(d, 712, 925, 44, 34, col, off)
    # Torso.
    limb(d, (405, 282), (540, 478), LIMB, col, off)
    # Hand-on-hip arm: down-left to the elbow, then back to the hip.
    chain(d, [(395, 290), (300, 470), (480, 462)], LIMB - 6, col, off)
    # Waving arm: out to the right, forearm up.
    chain(d, [(420, 268), (620, 272), (690, 125)], LIMB - 6, col, off)
    blob(d, 696, 112, 30, 34, col, off)
    # Head with ears: left ear tall, right ear swept sideways.
    blob(d, 405, 195, 108, 100, col, off)
    poly(d, [(300, 170), (372, 18), (398, 120)], col, off)
    poly(d, [(440, 92), (575, 108), (498, 150)], col, off)


def face(d):
    w = 9 * SS
    # :3 mouth: two small arcs meeting in the middle, plus a little open mouth.
    for cx in (395, 433):
        x, y = P(cx, 222)
        r = 19 * SS
        d.arc([x - r, y - r, x + r, y + r], start=10, end=170, fill=BLACK, width=w)
    x, y = P(414, 248)
    r = 15 * SS
    d.chord([x - r, y - r * 1.3, x + r, y + r * 1.3], start=0, end=180, fill=BLACK)
    rr = 9 * SS
    d.ellipse([x - rr, y + rr * 0.2, x + rr, y + rr * 1.6], fill=(230, 110, 110))


def master():
    img = Image.new("RGB", (S, S), ORANGE)
    d = ImageDraw.Draw(img)
    figure(d, BLACK, SHADOW)
    figure(d, WHITE, (0, 0))
    face(d)
    return img.resize((1024, 1024), Image.LANCZOS)


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
