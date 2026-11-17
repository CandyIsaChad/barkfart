package app.barkfart.export;

import java.io.ByteArrayOutputStream;
import java.util.zip.CRC32;
import java.util.zip.Deflater;

/** PNG encoder for straight RGBA8 pixels (rows top-down). */
public final class Png {
    private Png() {}

    public static byte[] encode(byte[] rgba, int w, int h, boolean alpha) {
        int bpp = alpha ? 4 : 3;
        byte[] raw = new byte[h * (1 + w * bpp)];
        int o = 0;
        for (int y = 0; y < h; y++) {
            raw[o++] = 0;   // filter: none
            for (int x = 0; x < w; x++) {
                int i = (y * w + x) * 4;
                raw[o++] = rgba[i];
                raw[o++] = rgba[i + 1];
                raw[o++] = rgba[i + 2];
                if (alpha) raw[o++] = rgba[i + 3];
            }
        }
        Deflater d = new Deflater(6);
        d.setInput(raw);
        d.finish();
        ByteArrayOutputStream z = new ByteArrayOutputStream(raw.length / 4 + 64);
        byte[] buf = new byte[65536];
        while (!d.finished()) z.write(buf, 0, d.deflate(buf));
        d.end();
        ByteArrayOutputStream out = new ByteArrayOutputStream(z.size() + 64);
        out.write(new byte[] {(byte) 0x89, 'P', 'N', 'G', 13, 10, 26, 10}, 0, 8);
        byte[] ihdr = new byte[13];
        be32(ihdr, 0, w);
        be32(ihdr, 4, h);
        ihdr[8] = 8;
        ihdr[9] = (byte) (alpha ? 6 : 2);
        chunk(out, "IHDR", ihdr);
        chunk(out, "IDAT", z.toByteArray());
        chunk(out, "IEND", new byte[0]);
        return out.toByteArray();
    }

    private static void chunk(ByteArrayOutputStream out, String type, byte[] data) {
        byte[] len = new byte[4];
        be32(len, 0, data.length);
        out.write(len, 0, 4);
        byte[] t = type.getBytes(java.nio.charset.Charset.forName("US-ASCII"));
        out.write(t, 0, 4);
        out.write(data, 0, data.length);
        CRC32 crc = new CRC32();
        crc.update(t);
        crc.update(data);
        byte[] c = new byte[4];
        be32(c, 0, (int) crc.getValue());
        out.write(c, 0, 4);
    }

    private static void be32(byte[] b, int o, int v) {
        b[o] = (byte) (v >>> 24);
        b[o + 1] = (byte) (v >>> 16);
        b[o + 2] = (byte) (v >>> 8);
        b[o + 3] = (byte) v;
    }
}
