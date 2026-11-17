package app.barkfart.audio;

/**
 * RIFF/WAVE decoder: integer PCM (8, 16, 24, 32 bit) and 32-bit float, any
 * channel count (more than two are downmixed to stereo). Returns null for
 * anything else; never throws on bad input.
 */
public final class Wav {
    private Wav() {}

    public static boolean looksLike(byte[] b) {
        return b != null && b.length >= 12 && b[0] == 'R' && b[1] == 'I' && b[2] == 'F' && b[3] == 'F' && b[8] == 'W' && b[9] == 'A' && b[10] == 'V' && b[11] == 'E';
    }

    public static Pcm decode(byte[] b) {
        try {
            if (!looksLike(b)) return null;
            int format = 0, channels = 0, rate = 0, bits = 0, dataAt = -1, dataLen = 0;
            int p = 12;
            while (p + 8 <= b.length) {
                String id = new String(b, p, 4, "ISO-8859-1");
                long len = u32(b, p + 4);
                int body = p + 8;
                if (id.equals("fmt ") && body + 16 <= b.length) {
                    format = u16(b, body);
                    channels = u16(b, body + 2);
                    rate = (int) u32(b, body + 4);
                    bits = u16(b, body + 14);
                    if (format == 0xFFFE && len >= 26 && body + 26 <= b.length) format = u16(b, body + 24);   // extensible: sub-format
                } else if (id.equals("data")) {
                    dataAt = body;
                    dataLen = (int) Math.min(len, (long) b.length - body);
                    break;
                }
                p = (int) Math.min((long) b.length, body + len + (len & 1));
            }
            if (dataAt < 0 || channels <= 0 || channels > 32 || rate <= 0 || rate > 384000) return null;
            boolean isFloat = format == 3 && bits == 32;
            if (!(format == 1 && (bits == 8 || bits == 16 || bits == 24 || bits == 32)) && !isFloat) return null;
            int bytes = bits / 8, stride = bytes * channels;
            int frames = Math.min(dataLen / stride, rate * Pcm.MAX_SECONDS);
            int out = channels >= 2 ? 2 : 1;
            short[] d = new short[frames * out];
            for (int i = 0; i < frames; i++) {
                for (int c = 0; c < out; c++) {
                    int at = dataAt + i * stride + c * bytes;
                    if (bits == 16 && !isFloat) d[i * out + c] = (short) u16(b, at);   // exact, no float round trip
                    else d[i * out + c] = Pcm.s16(sample(b, at, bits, isFloat));
                }
            }
            return new Pcm(rate, out, d);
        } catch (RuntimeException e) {
            return null;
        } catch (java.io.UnsupportedEncodingException e) {
            return null;
        }
    }

    private static float sample(byte[] b, int o, int bits, boolean isFloat) {
        if (isFloat) return Float.intBitsToFloat((int) u32(b, o));
        switch (bits) {
            case 8: return ((b[o] & 0xff) - 128) / 128f;
            case 16: return (short) u16(b, o) / 32768f;
            case 24: return ((b[o] & 0xff) | ((b[o + 1] & 0xff) << 8) | (b[o + 2] << 16)) / 8388608f;
            default: return (int) u32(b, o) / 2147483648f;
        }
    }

    private static int u16(byte[] b, int o) {
        return (b[o] & 0xff) | ((b[o + 1] & 0xff) << 8);
    }

    private static long u32(byte[] b, int o) {
        return (b[o] & 0xffL) | ((b[o + 1] & 0xffL) << 8) | ((b[o + 2] & 0xffL) << 16) | ((b[o + 3] & 0xffL) << 24);
    }

    /** Encodes 16-bit PCM as a WAV file (tests, the test tone, export later). */
    public static byte[] encode(Pcm pcm) {
        int dataLen = pcm.data.length * 2;
        byte[] b = new byte[44 + dataLen];
        put(b, 0, "RIFF");
        le32(b, 4, 36 + dataLen);
        put(b, 8, "WAVE");
        put(b, 12, "fmt ");
        le32(b, 16, 16);
        le16(b, 20, 1);
        le16(b, 22, pcm.channels);
        le32(b, 24, pcm.rate);
        le32(b, 28, pcm.rate * pcm.channels * 2);
        le16(b, 32, pcm.channels * 2);
        le16(b, 34, 16);
        put(b, 36, "data");
        le32(b, 40, dataLen);
        for (int i = 0; i < pcm.data.length; i++) le16(b, 44 + i * 2, pcm.data[i]);
        return b;
    }

    private static void put(byte[] b, int o, String s) {
        for (int i = 0; i < 4; i++) b[o + i] = (byte) s.charAt(i);
    }

    private static void le16(byte[] b, int o, int v) {
        b[o] = (byte) v;
        b[o + 1] = (byte) (v >> 8);
    }

    private static void le32(byte[] b, int o, int v) {
        le16(b, o, v);
        le16(b, o + 2, v >> 16);
    }
}
