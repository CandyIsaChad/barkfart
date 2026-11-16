package app.barkfart.legacy;

import java.io.ByteArrayOutputStream;
import java.io.IOException;

/** Endian-aware binary reader/writer for legacy formats. */
final class BinIO {
    private BinIO() {}

    static final class In {
        final byte[] b;
        final boolean little;
        int pos;

        In(byte[] b, boolean little) {
            this.b = b;
            this.little = little;
        }

        private void need(int n) throws IOException {
            if (pos + n > b.length || pos + n < pos) throw new IOException("unexpected end of file at " + pos);
        }

        int i8() throws IOException {
            need(1);
            return b[pos++];
        }

        int u8() throws IOException {
            need(1);
            return b[pos++] & 0xff;
        }

        int i16() throws IOException {
            need(2);
            int x = little ? (b[pos] & 0xff) | (b[pos + 1] << 8) : (b[pos] << 8) | (b[pos + 1] & 0xff);
            pos += 2;
            return (short) x;
        }

        int i32() throws IOException {
            need(4);
            int x;
            if (little) {
                x = (b[pos] & 0xff) | ((b[pos + 1] & 0xff) << 8) | ((b[pos + 2] & 0xff) << 16) | (b[pos + 3] << 24);
            } else {
                x = (b[pos] << 24) | ((b[pos + 1] & 0xff) << 16) | ((b[pos + 2] & 0xff) << 8) | (b[pos + 3] & 0xff);
            }
            pos += 4;
            return x;
        }

        /** A count that must be plausible for the remaining bytes (each item at least minBytes). */
        int count(int minBytes) throws IOException {
            int n = i32();
            if (n < 0 || (long) n * Math.max(1, minBytes) > b.length - pos) throw new IOException("bad count " + n + " at " + pos);
            return n;
        }

        boolean atEnd() {
            return pos == b.length;
        }
    }

    static final class Out {
        final ByteArrayOutputStream o = new ByteArrayOutputStream();
        final boolean little;

        Out(boolean little) {
            this.little = little;
        }

        void i8(int v) {
            o.write(v);
        }

        void i16(int v) {
            if (little) {
                o.write(v);
                o.write(v >> 8);
            } else {
                o.write(v >> 8);
                o.write(v);
            }
        }

        void i32(int v) {
            if (little) {
                o.write(v);
                o.write(v >> 8);
                o.write(v >> 16);
                o.write(v >> 24);
            } else {
                o.write(v >> 24);
                o.write(v >> 16);
                o.write(v >> 8);
                o.write(v);
            }
        }

        byte[] bytes() {
            return o.toByteArray();
        }
    }
}
