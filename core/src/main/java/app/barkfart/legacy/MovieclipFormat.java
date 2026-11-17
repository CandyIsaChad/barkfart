package app.barkfart.legacy;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;
import java.util.zip.GZIPInputStream;
import java.util.zip.GZIPOutputStream;

/**
 * Legacy movieclip (.nodemc) reader/writer. Header and library follow the MIT
 * sticknodes-rs spec; the frame layout was decoded from local samples (NOTES.md)
 * and is verified for version 330 only. Unknown frame bytes are kept raw so
 * writeBody(read(x)) reproduces the decompressed body exactly.
 */
public final class MovieclipFormat {
    static final byte[] MAGIC = {1, 2, 3, 4, 5, 6, 7, 8, 9};
    /** Versions whose frame layout has been verified against real files. */
    static final int VERIFIED_VERSION = 330;
    static final int FIGURE_HEADER_BYTES = 97;
    /** Bytes after the last figure of each frame (meaning unknown, kept raw). */
    static final int FRAME_TRAILER_BYTES = 4;
    static final int NODE_RECORD_BYTES = 58;

    public static final class NamedFigure {
        public String name;
        public NodesFormat.LegacyFigure figure;
    }

    /** One figure instance inside a frame. */
    public static final class FrameFigure {
        public int libraryId;
        /** Raw 97-byte per-instance header; decoded fields below. */
        public byte[] header;
        /** Per node in hierarchy order: raw 58-byte records. */
        public final List<byte[]> nodes = new ArrayList<byte[]>();

        public int instanceId() {
            return be32(header, 0);
        }

        public float scale() {
            return Float.intBitsToFloat(be32(header, 4));
        }

        public float opacity() {
            return Float.intBitsToFloat(be32(header, 8));
        }

        /** Stage position (1920x1080 stage, centre 960,540). */
        public float stageX() {
            return Float.intBitsToFloat(be32(header, 67));
        }

        public float stageY() {
            return Float.intBitsToFloat(be32(header, 71));
        }

        public static float nodeScale(byte[] r) {
            return Float.intBitsToFloat(be32(r, 7));  // scale
        }

        public static float nodeLength(byte[] r) {
            return Float.intBitsToFloat(be32(r, 11));
        }

        public static int nodeThickness(byte[] r) {
            return be32(r, 15);
        }

        public static float nodeLocalAngle(byte[] r) {
            return Float.intBitsToFloat(be32(r, 23));
        }

        public static int nodeColor(byte[] r) {
            return be32(r, 31);
        }

        public static int nodeGradientColor(byte[] r) {
            return be32(r, 35);
        }

        public static int nodeOutlineColor(byte[] r) {
            return be32(r, 39);
        }
    }

    public static final class Frame {
        public float soundVolume, soundPan, soundPitch;
        public int soundId;
        public final List<FrameFigure> figures = new ArrayList<FrameFigure>();
        /** Raw 4 bytes after the figures (meaning unknown). */
        public byte[] trailer = new byte[FRAME_TRAILER_BYTES];
    }

    public static final class LegacyMovieclip {
        public int version, build;
        public String name;
        public int previewCount, previewWidth, previewHeight;
        public byte[] preview;
        public int fps, tweenedFrames, tweening, loopProtection, uniqueFigureId;
        public final List<NamedFigure> library = new ArrayList<NamedFigure>();
        public final List<String> sounds = new ArrayList<String>();
        public final List<Frame> frames = new ArrayList<Frame>();
    }

    /** A file in a layout this reader does not decode yet (as opposed to a corrupt file). */
    public static final class UnsupportedLayoutException extends IOException {
        public UnsupportedLayoutException(String m) {
            super(m);
        }
    }

    private MovieclipFormat() {}

    static int be32(byte[] b, int p) {
        return (b[p] << 24) | ((b[p + 1] & 0xff) << 16) | ((b[p + 2] & 0xff) << 8) | (b[p + 3] & 0xff);
    }

    public static byte[] unpack(byte[] file) throws IOException {
        if (file.length < MAGIC.length + 2) throw new IOException("not a movieclip");
        for (int i = 0; i < MAGIC.length; i++) if (file[i] != MAGIC[i]) throw new IOException("not a movieclip");
        GZIPInputStream g = new GZIPInputStream(new ByteArrayInputStream(file, MAGIC.length, file.length - MAGIC.length));
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[65536];
        int n;
        while ((n = g.read(buf)) > 0) {
            out.write(buf, 0, n);
            if (out.size() > 512 * 1024 * 1024) throw new IOException("movieclip too large");
        }
        return out.toByteArray();
    }

    public static LegacyMovieclip read(byte[] file) throws IOException {
        byte[] body = unpack(file);
        BinIO.In in = new BinIO.In(body, false);
        LegacyMovieclip mc = readBody(in);
        if (!in.atEnd()) throw new IOException("trailing data at " + in.pos + " of " + body.length);
        return mc;
    }

    static String str(BinIO.In in) throws IOException {
        int n = in.count(1);
        String s = new String(in.b, in.pos, n, "UTF-8");
        in.pos += n;
        return s;
    }

    static LegacyMovieclip readBody(BinIO.In in) throws IOException {
        LegacyMovieclip mc = new LegacyMovieclip();
        mc.version = in.i32();
        if (mc.version != VERIFIED_VERSION) {
            throw new UnsupportedLayoutException("movieclip version " + mc.version + " is not supported yet (verified: " + VERIFIED_VERSION + ")");
        }
        mc.name = str(in);
        int frames = in.count(20), figs = in.count(16), sounds = in.count(4);
        mc.previewCount = in.i32();
        mc.previewWidth = in.i32();
        mc.previewHeight = in.i32();
        long px = (long) mc.previewCount * mc.previewWidth * mc.previewHeight * 4;
        if (mc.previewCount < 0 || mc.previewWidth < 0 || mc.previewHeight < 0 || px > in.b.length - in.pos) {
            throw new IOException("bad preview size");
        }
        mc.preview = new byte[(int) px];
        System.arraycopy(in.b, in.pos, mc.preview, 0, (int) px);
        in.pos += (int) px;
        mc.fps = in.i32();
        mc.tweenedFrames = in.i32();
        mc.tweening = in.u8();
        mc.loopProtection = in.u8();
        mc.uniqueFigureId = in.i32();
        for (int i = 0; i < figs; i++) {
            NamedFigure nf = new NamedFigure();
            nf.name = str(in);
            nf.figure = NodesFormat.readEmbedded(in);
            mc.library.add(nf);
        }
        for (int i = 0; i < sounds; i++) mc.sounds.add(str(in));
        for (int f = 0; f < frames; f++) {
            Frame fr = new Frame();
            fr.soundVolume = Float.intBitsToFloat(in.i32());
            fr.soundPan = Float.intBitsToFloat(in.i32());
            fr.soundPitch = Float.intBitsToFloat(in.i32());
            fr.soundId = in.i32();
            int assets = in.count(4 + FIGURE_HEADER_BYTES);
            for (int a = 0; a < assets; a++) {
                FrameFigure ff = new FrameFigure();
                ff.libraryId = in.i32();
                if (ff.libraryId < 0 || ff.libraryId >= mc.library.size()) throw new IOException("bad library id " + ff.libraryId);
                ff.header = take(in, FIGURE_HEADER_BYTES);
                int nodes = mc.library.get(ff.libraryId).figure.all().size();
                for (int k = 0; k < nodes; k++) ff.nodes.add(take(in, NODE_RECORD_BYTES));
                fr.figures.add(ff);
            }
            fr.trailer = take(in, FRAME_TRAILER_BYTES);
            mc.frames.add(fr);
        }
        return mc;
    }

    private static byte[] take(BinIO.In in, int n) throws IOException {
        if (in.pos + n > in.b.length) throw new IOException("unexpected end of file at " + in.pos);
        byte[] out = new byte[n];
        System.arraycopy(in.b, in.pos, out, 0, n);
        in.pos += n;
        return out;
    }

    public static byte[] writeBody(LegacyMovieclip mc) throws IOException {
        BinIO.Out out = new BinIO.Out(false);
        out.i32(mc.version);
        byte[] name = mc.name.getBytes("UTF-8");
        out.i32(name.length);
        out.o.write(name);
        out.i32(mc.frames.size());
        out.i32(mc.library.size());
        out.i32(mc.sounds.size());
        out.i32(mc.previewCount);
        out.i32(mc.previewWidth);
        out.i32(mc.previewHeight);
        out.o.write(mc.preview);
        out.i32(mc.fps);
        out.i32(mc.tweenedFrames);
        out.i8(mc.tweening);
        out.i8(mc.loopProtection);
        out.i32(mc.uniqueFigureId);
        for (NamedFigure nf : mc.library) {
            byte[] n = nf.name.getBytes("UTF-8");
            out.i32(n.length);
            out.o.write(n);
            NodesFormat.writeFigure(out, nf.figure);
        }
        for (String s : mc.sounds) {
            byte[] n = s.getBytes("UTF-8");
            out.i32(n.length);
            out.o.write(n);
        }
        for (Frame fr : mc.frames) {
            out.i32(Float.floatToRawIntBits(fr.soundVolume));
            out.i32(Float.floatToRawIntBits(fr.soundPan));
            out.i32(Float.floatToRawIntBits(fr.soundPitch));
            out.i32(fr.soundId);
            out.i32(fr.figures.size());
            for (FrameFigure ff : fr.figures) {
                out.i32(ff.libraryId);
                out.o.write(ff.header);
                for (byte[] r : ff.nodes) out.o.write(r);
            }
            out.o.write(fr.trailer);
        }
        return out.bytes();
    }

    public static byte[] write(LegacyMovieclip mc) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(MAGIC);
        GZIPOutputStream g = new GZIPOutputStream(out, 4096);
        g.write(writeBody(mc));
        g.close();
        return out.toByteArray();
    }
}
