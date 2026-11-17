package app.barkfart.image;

import java.util.ArrayList;
import java.util.List;

/**
 * Animated GIF decoder: global and local colour tables, LZW, interlacing,
 * transparency, frame delays and the disposal modes (keep, clear to
 * transparent, restore previous), composed onto a canvas. Returns null on
 * anything it cannot read; never throws on bad input.
 */
public final class GifDecoder {
    private GifDecoder() {}

    public static final int MAX_FRAMES = 500, MAX_SIDE = 4096;

    public static final class Frame {
        /** Whole-canvas straight RGBA8, rows top-down. */
        public final byte[] rgba;
        /** Seconds this frame shows. */
        public final double delay;

        Frame(byte[] rgba, double delay) {
            this.rgba = rgba;
            this.delay = delay;
        }
    }

    public static final class Result {
        public final int w, h;
        public final List<Frame> frames = new ArrayList<Frame>();

        Result(int w, int h) {
            this.w = w;
            this.h = h;
        }
    }

    public static Result decode(byte[] b) {
        try {
            return read(b);
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static int u16(byte[] b, int o) {
        return (b[o] & 0xff) | ((b[o + 1] & 0xff) << 8);
    }

    private static Result read(byte[] b) {
        if (b == null || b.length < 13 || b[0] != 'G' || b[1] != 'I' || b[2] != 'F') return null;
        int w = u16(b, 6), h = u16(b, 8), flags = b[10] & 0xff;
        if (w <= 0 || h <= 0 || w > MAX_SIDE || h > MAX_SIDE) return null;
        int p = 13;
        int[] global = null;
        if ((flags & 0x80) != 0) {
            int n = 2 << (flags & 7);
            global = palette(b, p, n);
            p += 3 * n;
        }
        Result res = new Result(w, h);
        byte[] canvas = new byte[w * h * 4];
        int delay = 0, transparent = -1, disposal = 0;
        while (p < b.length && res.frames.size() < MAX_FRAMES) {
            int block = b[p++] & 0xff;
            if (block == 0x3B) break;                       // trailer
            if (block == 0x21) {                            // extension
                int label = b[p++] & 0xff;
                if (label == 0xF9 && (b[p] & 0xff) >= 4) {   // graphic control
                    int packed = b[p + 1] & 0xff;
                    disposal = (packed >> 2) & 7;
                    delay = u16(b, p + 2);
                    transparent = (packed & 1) != 0 ? b[p + 4] & 0xff : -1;
                }
                while (p < b.length) {                       // skip sub-blocks
                    int len = b[p++] & 0xff;
                    if (len == 0) break;
                    p += len;
                }
                continue;
            }
            if (block != 0x2C) return res.frames.isEmpty() ? null : res;   // unknown: stop with what we have
            int fx = u16(b, p), fy = u16(b, p + 2), fw = u16(b, p + 4), fh = u16(b, p + 6), ff = b[p + 8] & 0xff;
            p += 9;
            int[] pal = global;
            if ((ff & 0x80) != 0) {
                int n = 2 << (ff & 7);
                pal = palette(b, p, n);
                p += 3 * n;
            }
            boolean interlaced = (ff & 0x40) != 0;
            int minCode = b[p++] & 0xff;
            java.io.ByteArrayOutputStream data = new java.io.ByteArrayOutputStream();
            while (p < b.length) {
                int len = b[p++] & 0xff;
                if (len == 0) break;
                data.write(b, p, Math.min(len, b.length - p));
                p += len;
            }
            byte[] idx = lzw(data.toByteArray(), minCode, fw * fh);
            byte[] before = disposal == 3 ? canvas.clone() : null;
            if (pal != null) {
                for (int i = 0; i < fw * fh; i++) {
                    int row = i / fw, col = i % fw;
                    int y = interlaced ? deinterlace(row, fh) : row;
                    int cx = fx + col, cy = fy + y;
                    if (cx >= w || cy >= h) continue;
                    int c = idx[i] & 0xff;
                    if (c == transparent || c >= pal.length) continue;
                    int o = (cy * w + cx) * 4, rgb = pal[c];
                    canvas[o] = (byte) (rgb >> 16);
                    canvas[o + 1] = (byte) (rgb >> 8);
                    canvas[o + 2] = (byte) rgb;
                    canvas[o + 3] = (byte) 255;
                }
            }
            double seconds = delay <= 1 ? 0.1 : delay / 100.0;   // browsers treat 0-1 as 0.1 s
            res.frames.add(new Frame(canvas.clone(), seconds));
            if (disposal == 2) {                               // clear the frame's area
                for (int y = fy; y < Math.min(h, fy + fh); y++) {
                    for (int x = fx; x < Math.min(w, fx + fw); x++) {
                        int o = (y * w + x) * 4;
                        canvas[o] = canvas[o + 1] = canvas[o + 2] = canvas[o + 3] = 0;
                    }
                }
            } else if (disposal == 3 && before != null) {
                canvas = before;
            }
            delay = 0;
            transparent = -1;
            disposal = 0;
        }
        return res.frames.isEmpty() ? null : res;
    }

    private static int[] palette(byte[] b, int p, int n) {
        int[] pal = new int[n];
        for (int i = 0; i < n; i++) pal[i] = ((b[p + i * 3] & 0xff) << 16) | ((b[p + i * 3 + 1] & 0xff) << 8) | (b[p + i * 3 + 2] & 0xff);
        return pal;
    }

    /** Row in the image for the n-th stored row of an interlaced frame. */
    static int deinterlace(int n, int h) {
        int[] start = {0, 4, 2, 1}, step = {8, 8, 4, 2};
        for (int pass = 0; pass < 4; pass++) {
            int rows = (h - start[pass] + step[pass] - 1) / step[pass];
            if (rows < 0) rows = 0;
            if (n < rows) return start[pass] + n * step[pass];
            n -= rows;
        }
        return h - 1;
    }

    /** Variable-width LZW decode into at most `count` indices (short data leaves zeros). */
    static byte[] lzw(byte[] data, int minCode, int count) {
        byte[] out = new byte[Math.max(0, count)];
        if (minCode < 2 || minCode > 11) minCode = 8;
        int clear = 1 << minCode, eoi = clear + 1;
        int[] prefix = new int[4096];
        byte[] suffix = new byte[4096], first = new byte[4096];
        int[] length = new int[4096];
        for (int i = 0; i < clear; i++) {
            suffix[i] = (byte) i;
            first[i] = (byte) i;
            length[i] = 1;
        }
        int width = minCode + 1, next = eoi + 1, prev = -1, o = 0;
        int bits = 0, buf = 0;
        byte[] stack = new byte[4097];
        for (int i = 0; i < data.length && o < count; i++) {
            buf |= (data[i] & 0xff) << bits;
            bits += 8;
            while (bits >= width && o < count) {
                int code = buf & ((1 << width) - 1);
                buf >>>= width;
                bits -= width;
                if (code == clear) {
                    width = minCode + 1;
                    next = eoi + 1;
                    prev = -1;
                    continue;
                }
                if (code == eoi) return out;
                int c = code;
                if (code >= next) {                           // KwKwK
                    if (prev < 0) return out;
                    c = prev;
                }
                int sp = 0;
                while (c >= clear && sp < 4096) {
                    stack[sp++] = suffix[c];
                    c = prefix[c];
                }
                stack[sp++] = (byte) c;
                byte firstByte = (byte) c;
                if (code >= next) {
                    // the code is prev + first(prev): write prev's string then its first byte
                    for (int k = sp - 1; k >= 0 && o < count; k--) out[o++] = stack[k];
                    if (o < count) out[o++] = firstByte;
                } else {
                    for (int k = sp - 1; k >= 0 && o < count; k--) out[o++] = stack[k];
                }
                if (prev >= 0 && next < 4096) {
                    prefix[next] = prev;
                    suffix[next] = firstByte;
                    next++;
                    if (next == (1 << width) && width < 12) width++;
                }
                prev = code;
            }
        }
        return out;
    }
}
