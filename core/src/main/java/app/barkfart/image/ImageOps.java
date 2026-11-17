package app.barkfart.image;

/** Pixel operations on straight RGBA8 images (rows top-down). */
public final class ImageOps {
    private ImageOps() {}

    /**
     * Remove background colour (Stick Nodes 4.1.0): pixels within tolerance
     * (0..255, largest channel difference) of the colour become transparent;
     * pixels a little further away fade out over the next `soft` levels so
     * edges do not keep a hard fringe. edgesOnly: only the region connected to
     * the image border (keeps that colour inside the subject, e.g. a white
     * shirt on a white background). Returns a new array.
     */
    public static byte[] removeColor(byte[] rgba, int w, int h, int rgb, int tolerance, int soft, boolean edgesOnly) {
        byte[] out = rgba.clone();
        int r0 = (rgb >> 16) & 0xff, g0 = (rgb >> 8) & 0xff, b0 = rgb & 0xff;
        int tol = Math.max(0, Math.min(255, tolerance)), sf = Math.max(0, Math.min(255, soft));
        boolean[] remove = new boolean[w * h];
        if (edgesOnly) {
            int[] queue = new int[w * h];
            int head = 0, tail = 0;
            for (int x = 0; x < w; x++) {
                tail = seed(rgba, w, x, 0, r0, g0, b0, tol + sf, remove, queue, tail);
                tail = seed(rgba, w, x, h - 1, r0, g0, b0, tol + sf, remove, queue, tail);
            }
            for (int y = 0; y < h; y++) {
                tail = seed(rgba, w, 0, y, r0, g0, b0, tol + sf, remove, queue, tail);
                tail = seed(rgba, w, w - 1, y, r0, g0, b0, tol + sf, remove, queue, tail);
            }
            while (head < tail) {
                int i = queue[head++], x = i % w, y = i / w;
                if (x > 0) tail = seed(rgba, w, x - 1, y, r0, g0, b0, tol + sf, remove, queue, tail);
                if (x < w - 1) tail = seed(rgba, w, x + 1, y, r0, g0, b0, tol + sf, remove, queue, tail);
                if (y > 0) tail = seed(rgba, w, x, y - 1, r0, g0, b0, tol + sf, remove, queue, tail);
                if (y < h - 1) tail = seed(rgba, w, x, y + 1, r0, g0, b0, tol + sf, remove, queue, tail);
            }
        } else {
            for (int i = 0; i < w * h; i++) remove[i] = diff(rgba, i, r0, g0, b0) <= tol + sf;
        }
        for (int i = 0; i < w * h; i++) {
            if (!remove[i]) continue;
            int d = diff(rgba, i, r0, g0, b0);
            float keep = d <= tol ? 0 : (sf == 0 ? 1 : (d - tol) / (float) sf);
            out[i * 4 + 3] = (byte) Math.round((rgba[i * 4 + 3] & 0xff) * Math.min(1, keep));
        }
        return out;
    }

    private static int seed(byte[] px, int w, int x, int y, int r, int g, int b, int limit, boolean[] seen, int[] q, int tail) {
        int i = y * w + x;
        if (seen[i] || diff(px, i, r, g, b) > limit) return tail;
        seen[i] = true;
        q[tail] = i;
        return tail + 1;
    }

    private static int diff(byte[] px, int i, int r, int g, int b) {
        return Math.max(Math.abs((px[i * 4] & 0xff) - r), Math.max(Math.abs((px[i * 4 + 1] & 0xff) - g), Math.abs((px[i * 4 + 2] & 0xff) - b)));
    }
}
