package app.barkfart.geom;

import app.barkfart.model.Layer;
import java.util.ArrayList;
import java.util.List;

/**
 * Triangles with per-vertex colour and texture coordinates, in view space (y
 * up, project pixels, origin at the canvas centre). Split into batches
 * wherever blend mode, effect group or texture changes. Renderers only fill
 * these; they compute no geometry.
 */
public final class DrawList {
    public static final class Batch {
        public final Layer.Blend blend;
        /** Source group for effect batches (effects run on this batch only), else null. */
        public final String effectGroup;
        /** Asset id of the texture, or null for untextured (colour only). */
        public final String texture;
        float[] xy = new float[256];
        float[] uv = new float[256];
        int[] rgba = new int[128];
        int vertices;

        Batch(Layer.Blend blend, String effectGroup, String texture) {
            this.blend = blend;
            this.effectGroup = effectGroup;
            this.texture = texture;
        }

        public int vertexCount() {
            return vertices;
        }

        public float x(int i) {
            return xy[i * 2];
        }

        public float y(int i) {
            return xy[i * 2 + 1];
        }

        /** Texture coordinates (0..1, v down the image) when textured. */
        public float u(int i) {
            return uv[i * 2];
        }

        public float v(int i) {
            return uv[i * 2 + 1];
        }

        /** Straight RGBA8888 (vertex colour; multiplies the texture when textured). */
        public int rgba(int i) {
            return rgba[i];
        }

        void add(float x, float y, int c) {
            add(x, y, c, 0, 0);
        }

        /** The isolate this batch belongs to (layered lists), for bounds. */
        Isolate owner;

        void add(float x, float y, int c, float u, float v) {
            if (owner != null && x == x && y == y && !Float.isInfinite(x) && !Float.isInfinite(y)) owner.include(x, y, x, y);
            if (vertices * 2 + 2 > xy.length) {
                xy = grow(xy, vertices * 2);
                uv = grow(uv, vertices * 2);
                int[] nc = new int[rgba.length * 2];
                System.arraycopy(rgba, 0, nc, 0, vertices);
                rgba = nc;
            }
            xy[vertices * 2] = x;
            xy[vertices * 2 + 1] = y;
            uv[vertices * 2] = u;
            uv[vertices * 2 + 1] = v;
            rgba[vertices] = c;
            vertices++;
        }

        private static float[] grow(float[] a, int used) {
            float[] n = new float[a.length * 2];
            System.arraycopy(a, 0, n, 0, used);
            return n;
        }
    }

    /**
     * A group drawn in isolation: its items render into their own buffer
     * (premultiplied), its effects run on that buffer, and the result is
     * composited with the group's opacity and blend. An adjustment isolate has
     * no items; its effects run on what its parent has drawn so far.
     */
    public static final class Isolate {
        public final String source;
        public final float opacity;
        public final Layer.Blend blend;
        public final boolean adjustment;
        public final List<app.barkfart.eval.ResolvedEffect> effects;
        /** Batch or Isolate, in draw order. */
        public final List<Object> items = new ArrayList<Object>();
        /** Content bounds in view space (no effect padding); empty when min > max. */
        public float minX = Float.POSITIVE_INFINITY, minY = Float.POSITIVE_INFINITY, maxX = Float.NEGATIVE_INFINITY, maxY = Float.NEGATIVE_INFINITY;
        final Isolate parent;

        Isolate(Isolate parent, String source, float opacity, Layer.Blend blend, boolean adjustment, List<app.barkfart.eval.ResolvedEffect> effects) {
            this.parent = parent;
            this.source = source;
            this.opacity = opacity;
            this.blend = blend;
            this.adjustment = adjustment;
            this.effects = effects;
        }

        public boolean empty() {
            return !(minX <= maxX && minY <= maxY);
        }

        void include(float x0, float y0, float x1, float y1) {
            if (!(x0 <= x1) || !(y0 <= y1)) return;   // also drops NaN
            minX = Math.min(minX, x0);
            minY = Math.min(minY, y0);
            maxX = Math.max(maxX, x1);
            maxY = Math.max(maxY, y1);
        }
    }

    public final List<Batch> batches = new ArrayList<Batch>();
    /** Set by Tessellator.layered(): the root of the isolate tree. Null for flat lists. */
    public Isolate root;
    private Isolate current;

    Isolate open(String source, float opacity, Layer.Blend blend, boolean adjustment, List<app.barkfart.eval.ResolvedEffect> effects) {
        Isolate i = new Isolate(current, source, opacity, blend, adjustment, effects);
        if (current == null) root = i;
        else current.items.add(i);
        current = i;
        return i;
    }

    void close() {
        Isolate done = current;
        current = current.parent;
        if (current != null && !done.empty()) current.include(done.minX, done.minY, done.maxX, done.maxY);
    }

    Batch batch(Layer.Blend blend, String effectGroup) {
        return batch(blend, effectGroup, null);
    }

    Batch batch(Layer.Blend blend, String effectGroup, String texture) {
        if (current != null) {
            Object last = current.items.isEmpty() ? null : current.items.get(current.items.size() - 1);
            if (last instanceof Batch) {
                Batch lb = (Batch) last;
                if (lb.blend == blend && eq(lb.effectGroup, effectGroup) && eq(lb.texture, texture)) return lb;
            }
            Batch b = new Batch(blend, effectGroup, texture);
            b.owner = current;
            current.items.add(b);
            batches.add(b);
            return b;
        }
        if (!batches.isEmpty()) {
            Batch last = batches.get(batches.size() - 1);
            boolean same = last.blend == blend && eq(last.effectGroup, effectGroup) && eq(last.texture, texture);
            if (same) return last;
        }
        Batch b = new Batch(blend, effectGroup, texture);
        batches.add(b);
        return b;
    }

    private static boolean eq(String a, String b) {
        return a == null ? b == null : a.equals(b);
    }

    public int triangleCount() {
        int n = 0;
        for (Batch b : batches) n += b.vertices / 3;
        return n;
    }
}
