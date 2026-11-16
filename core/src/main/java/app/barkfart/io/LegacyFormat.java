package app.barkfart.io;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.zip.GZIPInputStream;
import java.util.zip.InflaterInputStream;

/**
 * Identifies and unwraps legacy Stick Nodes files. Layouts are documented in
 * docs/barkfart/NOTES.md and were learned from data files only.
 */
public final class LegacyFormat {
    public enum Kind {
        /** zlib-compressed stickfigure (older files, little-endian body). */
        STICKFIGURE,
        /** Uncompressed big-endian body that starts with an int32 version. */
        RAW,
        /** 9-byte magic + gzip: movieclips and projects. */
        PACKED,
        UNKNOWN
    }

    /** Upper bound for a plausible format version in a RAW file. */
    static final int MAX_VERSION = 10000;

    /** 9-byte prefix 01 02 .. 09 that precedes the gzip stream in .nodemc/.stknds. */
    static final byte[] PACKED_MAGIC = {1, 2, 3, 4, 5, 6, 7, 8, 9};

    private LegacyFormat() {}

    public static Kind sniff(byte[] data) {
        if (data.length > PACKED_MAGIC.length + 2 && startsWith(data, PACKED_MAGIC)
                && (data[9] & 0xff) == 0x1f && (data[10] & 0xff) == 0x8b) {
            return Kind.PACKED;
        }
        // zlib header: CMF 0x78, and (CMF*256 + FLG) divisible by 31.
        if (data.length > 2 && (data[0] & 0xff) == 0x78
                && (((data[0] & 0xff) << 8) | (data[1] & 0xff)) % 31 == 0) {
            return Kind.STICKFIGURE;
        }
        if (data.length > 4) {
            int v = ((data[0] & 0xff) << 24) | ((data[1] & 0xff) << 16) | ((data[2] & 0xff) << 8) | (data[3] & 0xff);
            if (v > 0 && v < MAX_VERSION) return Kind.RAW;
        }
        return Kind.UNKNOWN;
    }

    /** Returns the decompressed body, or throws if the wrapper is not recognised. */
    public static byte[] unwrap(byte[] data) throws IOException {
        switch (sniff(data)) {
            case PACKED:
                return readAll(new GZIPInputStream(new ByteArrayInputStream(
                        data, PACKED_MAGIC.length, data.length - PACKED_MAGIC.length)));
            case STICKFIGURE:
                return readAll(new InflaterInputStream(new ByteArrayInputStream(data)));
            case RAW:
                return data.clone();
            default:
                throw new IOException("Not a recognised Stick Nodes file");
        }
    }

    private static boolean startsWith(byte[] data, byte[] prefix) {
        for (int i = 0; i < prefix.length; i++) if (data[i] != prefix[i]) return false;
        return true;
    }

    private static byte[] readAll(InputStream in) throws IOException {
        try (InputStream s = in) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[8192];
            int n;
            while ((n = s.read(buf)) > 0) out.write(buf, 0, n);
            return out.toByteArray();
        }
    }
}
