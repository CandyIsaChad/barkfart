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

        void add(float x, float y, int c, float u, float v) {
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

    public final List<Batch> batches = new ArrayList<Batch>();

    Batch batch(Layer.Blend blend, String effectGroup) {
        return batch(blend, effectGroup, null);
    }

    Batch batch(Layer.Blend blend, String effectGroup, String texture) {
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
