package app.barkfart.export;

import java.io.IOException;
import java.io.OutputStream;

/**
 * Animated GIF writer: a fixed 6x7x6 colour cube (252 colours) with 4x4
 * ordered dithering, transparent index 255 for pixels under half alpha,
 * LZW compression, frame delays in hundredths with the rounding error carried
 * forward (30 fps = 3, 3, 4 ...; never under 2, which browsers slow down).
 */
public final class Gif {
    private static final int[][] BAYER = {{0, 8, 2, 10}, {12, 4, 14, 6}, {3, 11, 1, 9}, {15, 7, 13, 5}};
    private final OutputStream out;
    private final int w, h;
    private final double frameSeconds;
    private double owed;   // hundredths not yet given out

    public Gif(OutputStream out, int w, int h, double fps, boolean loop) throws IOException {
        this.out = out;
        this.w = w;
        this.h = h;
        this.frameSeconds = 1.0 / (fps > 0 ? fps : 30);
        write("GIF89a");
        le16(w);
        le16(h);
        out.write(0xF7);   // global colour table, 8 bits, 256 entries
        out.write(0);
        out.write(0);
        for (int i = 0; i < 256; i++) {
            int[] c = colour(i);
            out.write(c[0]);
            out.write(c[1]);
            out.write(c[2]);
        }
        if (loop) {
            out.write(new byte[] {0x21, (byte) 0xFF, 0x0B});
            write("NETSCAPE2.0");
            out.write(new byte[] {3, 1, 0, 0, 0});
        }
    }

    /** Palette entry i: the 6x7x6 cube for i < 252, black otherwise (255 is transparent). */
    static int[] colour(int i) {
        if (i >= 252) return new int[] {0, 0, 0};
        int r = i / 42, g = (i / 6) % 7, b = i % 6;
        return new int[] {r * 255 / 5, g * 255 / 6, b * 255 / 5};
    }

    /** Next frame's delay in hundredths of a second. */
    int nextDelay() {
        owed += frameSeconds * 100;
        int d = (int) Math.floor(owed + 1e-9);
        if (d < 2) d = 2;
        owed -= d;
        return d;
    }

    /** Adds a frame of straight RGBA8 pixels, rows top-down. */
    public void frame(byte[] rgba) throws IOException {
        int delay = nextDelay();
        out.write(new byte[] {0x21, (byte) 0xF9, 4, 0x09, (byte) delay, (byte) (delay >> 8), (byte) 255, 0});   // dispose: restore to background, transparent 255
        out.write(0x2C);
        le16(0);
        le16(0);
        le16(w);
        le16(h);
        out.write(0);
        byte[] idx = new byte[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int p = (y * w + x) * 4;
                if ((rgba[p + 3] & 0xff) < 128) {
                    idx[y * w + x] = (byte) 255;
                    continue;
                }
                float th = (BAYER[y & 3][x & 3] + 0.5f) / 16f;
                int r = q(rgba[p] & 0xff, 5, th), g = q(rgba[p + 1] & 0xff, 6, th), b = q(rgba[p + 2] & 0xff, 5, th);
                idx[y * w + x] = (byte) (r * 42 + g * 6 + b);
            }
        }
        lzw(idx);
    }

    private static int q(int v, int levels, float threshold) {
        float s = v * levels / 255f;
        int lo = (int) s;
        return Math.min(levels, lo + ((s - lo) > threshold ? 1 : 0));
    }

    public void finish() throws IOException {
        out.write(0x3B);
        out.flush();
    }

    // ---- LZW (8-bit codes, variable width up to 12 bits) ----

    private int bitBuf, bitCount;
    private final byte[] block = new byte[255];
    private int blockLen;

    private void lzw(byte[] data) throws IOException {
        final int min = 8, clear = 1 << min, eoi = clear + 1;
        out.write(min);
        bitBuf = 0;
        bitCount = 0;
        blockLen = 0;
        java.util.HashMap<Integer, Integer> dict = new java.util.HashMap<Integer, Integer>();
        int next = eoi + 1, width = min + 1;
        emit(clear, width);
        int prefix = data.length > 0 ? data[0] & 0xff : -1;
        for (int i = 1; i < data.length; i++) {
            int c = data[i] & 0xff;
            int key = (prefix << 8) | c;
            Integer code = dict.get(key);
            if (code != null) {
                prefix = code;
                continue;
            }
            emit(prefix, width);
            if (next < 4096) {
                dict.put(key, next++);
                if (next > (1 << width) && width < 12) width++;
            } else {
                emit(clear, width);
                dict.clear();
                next = eoi + 1;
                width = min + 1;
            }
            prefix = c;
        }
        if (prefix >= 0) emit(prefix, width);
        emit(eoi, width);
        if (bitCount > 0) byteOut(bitBuf & 0xff);
        if (blockLen > 0) flushBlock();
        out.write(0);
    }

    private void emit(int code, int width) throws IOException {
        bitBuf |= code << bitCount;
        bitCount += width;
        while (bitCount >= 8) {
            byteOut(bitBuf & 0xff);
            bitBuf >>>= 8;
            bitCount -= 8;
        }
    }

    private void byteOut(int b) throws IOException {
        block[blockLen++] = (byte) b;
        if (blockLen == 255) flushBlock();
    }

    private void flushBlock() throws IOException {
        out.write(blockLen);
        out.write(block, 0, blockLen);
        blockLen = 0;
    }

    private void write(String s) throws IOException {
        for (int i = 0; i < s.length(); i++) out.write(s.charAt(i));
    }

    private void le16(int v) throws IOException {
        out.write(v & 0xff);
        out.write((v >> 8) & 0xff);
    }
}
