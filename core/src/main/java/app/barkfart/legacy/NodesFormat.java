package app.barkfart.legacy;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.zip.DeflaterOutputStream;
import java.util.zip.InflaterInputStream;

/**
 * Legacy stickfigure (.nodes) reader and writer. Field layout follows the MIT
 * sticknodes-rs spec (NOTES.md): big-endian for version >= 160; older files
 * are zlib-compressed little-endian. Every field, including unused ones, is
 * kept by name so write(read(x)) reproduces the body byte for byte.
 */
public final class NodesFormat {
    /** Newest version/build this reader has been checked against. */
    public static final int MAX_VERSION = 423;

    enum Kind { I8, U8, I16, I32, F32 }

    /** One schema field with its version/build window. */
    static final class Field {
        final String name;
        final Kind kind;
        final int minVersion, minBuild, maxBuild;
        final boolean notBuild36;

        Field(String name, Kind kind, int minVersion, int minBuild, int maxBuild, boolean notBuild36) {
            this.name = name;
            this.kind = kind;
            this.minVersion = minVersion;
            this.minBuild = minBuild;
            this.maxBuild = maxBuild;
            this.notBuild36 = notBuild36;
        }

        boolean present(int version, int build) {
            if (version < minVersion) return false;
            if (minVersion >= 403 && (build < minBuild || build > maxBuild)) return false;
            return !(notBuild36 && build == 36);
        }
    }

    private static final int ANY = Integer.MAX_VALUE;

    private static Field f(String n, Kind k) {
        return new Field(n, k, 0, 0, ANY, false);
    }

    private static Field v(String n, Kind k, int minVersion) {
        return new Field(n, k, minVersion, 0, ANY, false);
    }

    private static Field b(String n, Kind k, int minBuild, int maxBuild) {
        return new Field(n, k, 403, minBuild, maxBuild, false);
    }

    /** Node fields in file order. */
    static final Field[] NODE = {
        f("type", Kind.I8),
        f("drawIndex", Kind.I32),
        f("static", Kind.U8),
        f("stretchy", Kind.U8),
        b("floaty", Kind.U8, 48, ANY),
        v("smartStretch", Kind.U8, 248),
        v("noSmartStretch", Kind.U8, 252),
        b("smartStretchResetImpulse", Kind.U8, 50, ANY),
        f("useSegmentColor", Kind.U8),
        v("circleOutline", Kind.U8, 256),
        b("circleHollow", Kind.U8, 21, ANY),
        v("gradient", Kind.U8, 176),
        v("reverseGradient", Kind.U8, 176),
        b("gradientMode", Kind.I16, 20, ANY),
        f("useSegmentScale", Kind.U8),
        f("localX", Kind.F32),
        f("localY", Kind.F32),
        f("scale", Kind.F32),
        f("defaultLength", Kind.F32),
        f("length", Kind.F32),
        f("defaultThickness", Kind.I32),
        f("thickness", Kind.I32),
        v("curveRadius", Kind.I32, 320),
        b("curveCircularize", Kind.U8, 20, ANY),
        b("curvePolyfillPrecision", Kind.I16, 21, ANY),
        v("halfArc", Kind.U8, 256),
        v("rightTriangleDirection", Kind.I16, 256),
        v("triangleUpsideDown", Kind.U8, 300),
        b("trapezoidStartI", Kind.I32, 36, 63),
        b("trapezoidEndI", Kind.I32, 36, 63),
        b("trapezoidStartF", Kind.F32, 64, ANY),
        b("trapezoidEndF", Kind.F32, 64, ANY),
        b("unused36a", Kind.I32, 36, 36),
        b("unused36b", Kind.I32, 36, 36),
        b("useTrapezoidStart", Kind.U8, 36, 36),
        b("useTrapezoidEnd", Kind.U8, 36, 36),
        new Field("trapezoidTopRatio", Kind.F32, 256, 0, ANY, true),
        b("trapezoidRoundedStart", Kind.U8, 36, ANY),
        b("trapezoidRoundedEnd", Kind.U8, 36, ANY),
        v("polygonVertices", Kind.I16, 256),
        v("defaultLocalAngle", Kind.F32, 248),
        f("localAngle", Kind.F32),
        v("defaultAngle", Kind.F32, 248),
        f("color", Kind.I32),
        v("gradientColor", Kind.I32, 176),
        v("circleOutlineColor", Kind.I32, 256),
        b("angleLocked", Kind.U8, 39, ANY),
        b("unused39to50", Kind.F32, 39, 50),
        b("angleLockMain", Kind.U8, 51, ANY),
        b("angleLockMinuend", Kind.F32, 51, 56),
        b("angleLockSubtrahend", Kind.F32, 51, 56),
        b("angleLockOffset", Kind.F32, 57, ANY),
        b("angleLockRelativeStart", Kind.F32, 63, ANY),
        b("angleLockFigureStart", Kind.F32, 67, ANY),
        b("angleLockRelativeMultiplier", Kind.I8, 63, ANY),
        b("dragLockedShort", Kind.I16, 39, 40),
        b("dragLocked", Kind.U8, 41, ANY),
        b("unused41to45s", Kind.I16, 41, 45),
        b("dragLockAngle", Kind.F32, 46, ANY),
        b("smartStretchMultiplier", Kind.F32, 41, ANY),
        b("unused41to45b", Kind.U8, 41, 45),
    };

    /** Connector data, read before a connector node's own fields. */
    static final Field[] CONNECTOR = {
        b("cLocalX", Kind.F32, 0, ANY),
        b("cLocalY", Kind.F32, 0, ANY),
        b("cPercent", Kind.F32, 0, ANY),
        b("cPercentDefault", Kind.F32, 44, ANY),
        b("cValue", Kind.F32, 0, ANY),
        b("cMethod", Kind.I32, 0, ANY),
        b("cReversed", Kind.U8, 0, ANY),
        b("cSmartStretchAncestral", Kind.F32, 65, ANY),
    };

    public static final class LegacyNode {
        public final Map<String, Object> fields = new LinkedHashMap<String, Object>();
        public final Map<String, Object> connector = new LinkedHashMap<String, Object>();
        public boolean isConnector;
        public final List<LegacyNode> children = new ArrayList<LegacyNode>();

        public int i(String k) {
            Object o = fields.get(k);
            return o == null ? 0 : ((Number) o).intValue();
        }

        public float f(String k) {
            Object o = fields.get(k);
            return o == null ? 0 : ((Number) o).floatValue();
        }
    }

    public static final class Polyfill {
        public int anchor, color, useColor;
        public final List<Integer> nodes = new ArrayList<Integer>();
    }

    public static final class LegacyFigure {
        public int version, build;
        public float scale;
        public int color;
        public boolean littleEndian, compressed;
        public LegacyNode root;
        public final List<Polyfill> polyfills = new ArrayList<Polyfill>();
        public final List<int[]> connections = new ArrayList<int[]>();

        /** All nodes, depth-first (hierarchy order). */
        public List<LegacyNode> all() {
            List<LegacyNode> out = new ArrayList<LegacyNode>();
            ArrayList<LegacyNode> stack = new ArrayList<LegacyNode>();
            if (root != null) stack.add(root);
            while (!stack.isEmpty()) {
                LegacyNode n = stack.remove(stack.size() - 1);
                out.add(n);
                for (int i = n.children.size() - 1; i >= 0; i--) stack.add(n.children.get(i));
            }
            return out;
        }
    }

    private NodesFormat() {}

    /** Decompresses if needed, then parses. Must consume the body to the exact end. */
    public static LegacyFigure read(byte[] file) throws IOException {
        boolean compressed = file.length > 2 && (file[0] & 0xff) == 0x78
                && ((((file[0] & 0xff) << 8) | (file[1] & 0xff)) % 31) == 0;
        byte[] body = compressed ? inflate(file) : file;
        LegacyFigure fig = readBody(new BinIO.In(body, compressed));
        fig.compressed = compressed;
        return fig;
    }

    /** Parses an uncompressed figure body that is embedded in another stream (projects, movieclips). */
    static LegacyFigure readEmbedded(BinIO.In in) throws IOException {
        return readFigure(in);
    }

    private static LegacyFigure readBody(BinIO.In in) throws IOException {
        LegacyFigure fig = readFigure(in);
        if (!in.atEnd()) throw new IOException("trailing data at " + in.pos + " of " + in.b.length);
        return fig;
    }

    private static LegacyFigure readFigure(BinIO.In in) throws IOException {
        LegacyFigure fig = new LegacyFigure();
        fig.littleEndian = in.little;
        fig.version = in.i32();
        if (fig.version < 0 || fig.version > MAX_VERSION) throw new IOException("unsupported .nodes version " + fig.version);
        if (fig.version >= 403) fig.build = in.i32();
        fig.scale = Float.intBitsToFloat(in.i32());
        fig.color = in.i32();
        // Nodes: iterative depth-first read with an explicit stack (no recursion on hostile files).
        fig.root = readNode(in, fig, false);
        ArrayList<Object[]> stack = new ArrayList<Object[]>();
        stack.add(new Object[] {fig.root, pendingChildren(fig.root), 0});
        int total = 1;
        while (!stack.isEmpty()) {
            Object[] top = stack.get(stack.size() - 1);
            LegacyNode parent = (LegacyNode) top[0];
            int[] flags = (int[]) top[1];
            int next = (Integer) top[2];
            if (next >= flags.length) {
                stack.remove(stack.size() - 1);
                continue;
            }
            top[2] = next + 1;
            LegacyNode child = readNode(in, fig, flags[next] != 0);
            parent.children.add(child);
            if (++total > 100000) throw new IOException("too many nodes");
            stack.add(new Object[] {child, pendingChildren(child), 0});
        }
        if (fig.version >= 230 && !(fig.littleEndian)) {
            int n = in.count(13);
            for (int i = 0; i < n; i++) {
                Polyfill p = new Polyfill();
                p.anchor = in.i32();
                p.color = in.i32();
                p.useColor = in.u8();
                int m = in.count(4);
                for (int k = 0; k < m; k++) p.nodes.add(in.i32());
                fig.polyfills.add(p);
            }
        }
        if (fig.version >= 403 && fig.build >= 38) {
            int n = in.count(8);
            for (int i = 0; i < n; i++) fig.connections.add(new int[] {in.i32(), in.i32()});
        }
        return fig;
    }

    /** Child count and connector flags were read into the node's fields; expose them as an array. */
    private static int[] pendingChildren(LegacyNode n) {
        int count = n.i("~children");
        int[] flags = new int[count];
        for (int i = 0; i < count; i++) {
            Object o = n.fields.get("~connector" + i);
            flags[i] = o == null ? 0 : ((Number) o).intValue();
        }
        return flags;
    }

    private static LegacyNode readNode(BinIO.In in, LegacyFigure fig, boolean isConnector) throws IOException {
        LegacyNode n = new LegacyNode();
        n.isConnector = isConnector;
        if (isConnector) for (Field fd : CONNECTOR) if (fd.present(fig.version, fig.build)) n.connector.put(fd.name, read(in, fd.kind));
        for (Field fd : NODE) if (fd.present(fig.version, fig.build)) n.fields.put(fd.name, read(in, fd.kind));
        int count = in.count(4);
        n.fields.put("~children", count);
        if (fig.version >= 403 && fig.build > 38) {
            for (int i = 0; i < count; i++) n.fields.put("~connector" + i, in.i32());
        }
        return n;
    }

    private static Object read(BinIO.In in, Kind k) throws IOException {
        switch (k) {
            case I8: return in.i8();
            case U8: return in.u8();
            case I16: return in.i16();
            case I32: return in.i32();
            default: return Float.intBitsToFloat(in.i32());
        }
    }

    // ---------------- writing ----------------

    /** Serializes to file bytes (zlib-compressed little-endian for old versions, as read). */
    public static byte[] write(LegacyFigure fig) throws IOException {
        byte[] body = writeBody(fig);
        return fig.compressed ? deflate(body) : body;
    }

    public static byte[] writeBody(LegacyFigure fig) {
        BinIO.Out out = new BinIO.Out(fig.littleEndian);
        writeFigure(out, fig);
        return out.bytes();
    }

    static void writeFigure(BinIO.Out out, LegacyFigure fig) {
        out.i32(fig.version);
        if (fig.version >= 403) out.i32(fig.build);
        out.i32(Float.floatToRawIntBits(fig.scale));
        out.i32(fig.color);
        ArrayList<LegacyNode> stack = new ArrayList<LegacyNode>();
        stack.add(fig.root);
        while (!stack.isEmpty()) {
            LegacyNode n = stack.remove(stack.size() - 1);
            writeNode(out, fig, n);
            for (int i = n.children.size() - 1; i >= 0; i--) stack.add(n.children.get(i));
        }
        if (fig.version >= 230 && !fig.littleEndian) {
            out.i32(fig.polyfills.size());
            for (Polyfill p : fig.polyfills) {
                out.i32(p.anchor);
                out.i32(p.color);
                out.i8(p.useColor);
                out.i32(p.nodes.size());
                for (int k : p.nodes) out.i32(k);
            }
        }
        if (fig.version >= 403 && fig.build >= 38) {
            out.i32(fig.connections.size());
            for (int[] c : fig.connections) {
                out.i32(c[0]);
                out.i32(c[1]);
            }
        }
    }

    private static void writeNode(BinIO.Out out, LegacyFigure fig, LegacyNode n) {
        if (n.isConnector) for (Field fd : CONNECTOR) if (fd.present(fig.version, fig.build)) write(out, fd.kind, n.connector.get(fd.name));
        for (Field fd : NODE) if (fd.present(fig.version, fig.build)) write(out, fd.kind, n.fields.get(fd.name));
        out.i32(n.children.size());
        if (fig.version >= 403 && fig.build > 38) {
            for (LegacyNode c : n.children) out.i32(c.isConnector ? 1 : 0);
        }
    }

    private static void write(BinIO.Out out, Kind k, Object v) {
        Number num = v == null ? Integer.valueOf(0) : (Number) v;
        switch (k) {
            case I8:
            case U8: out.i8(num.intValue()); break;
            case I16: out.i16(num.intValue()); break;
            case I32: out.i32(num.intValue()); break;
            default: out.i32(Float.floatToRawIntBits(num.floatValue()));
        }
    }

    private static byte[] inflate(byte[] z) throws IOException {
        InflaterInputStream in = new InflaterInputStream(new ByteArrayInputStream(z));
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[8192];
        int n;
        while ((n = in.read(buf)) > 0) {
            out.write(buf, 0, n);
            if (out.size() > 64 * 1024 * 1024) throw new IOException("figure too large");
        }
        return out.toByteArray();
    }

    private static byte[] deflate(byte[] body) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        DeflaterOutputStream z = new DeflaterOutputStream(out);
        z.write(body);
        z.close();
        return out.toByteArray();
    }
}
