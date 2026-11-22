import sys
from PIL import Image
src, out = sys.argv[1], sys.argv[2]
im = Image.open(src)
w, h = im.size
for i, (x, y) in enumerate([(0, 0), (w // 2, 0), (0, h // 2), (w // 2, h // 2)]):
    im.crop((x, y, x + w // 2, y + h // 2)).save(f"{out}-{i}.png")
im.resize((w // 2, h // 2), Image.LANCZOS).save(f"{out}-full.png")
