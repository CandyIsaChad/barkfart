package app.barkfart.geom;

import app.barkfart.model.Layer;
import java.util.ArrayList;
import java.util.List;

/**
 * Triangles with per-vertex colour, in view space (y up, project pixels,
 * origin at the canvas centre). Split into batches wherever blend mode or
 * effects change. Renderers only fill these; they compute no geometry.
 */
public final class DrawList {
    public static final class Batch {
        public final Layer.Blend blend;
        /** Source group for effect batches (effects run on this batch only), else null. */
        public final String effectGroup;
        float[] xy = new float[256];
        int[] rgba = new int[128];
        int vertices;

        Batch(Layer.Blend blend, String effectGroup) {
            this.blend = blend;
            this.effectGroup = effectGroup;
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

        /** Straight RGBA8888. */
        public int rgba(int i) {
            return rgba[i];
        }

        void add(float x, float y, int c) {
            if (vertices * 2 + 2 > xy.length) {
                float[] nx = new float[xy.length * 2];
                System.arraycopy(xy, 0, nx, 0, vertices * 2);
                xy = nx;
                int[] nc = new int[rgba.length * 2];
                System.arraycopy(rgba, 0, nc, 0, vertices);
                rgba = nc;
            }
            xy[vertices * 2] = x;
            xy[vertices * 2 + 1] = y;
            rgba[vertices] = c;
            vertices++;
        }
    }

    public final List<Batch> batches = new ArrayList<Batch>();

    Batch batch(Layer.Blend blend, String effectGroup) {
        if (!batches.isEmpty()) {
            Batch last = batches.get(batches.size() - 1);
            boolean same = last.blend == blend
                    && (last.effectGroup == null ? effectGroup == null : last.effectGroup.equals(effectGroup));
            if (same) return last;
        }
        Batch b = new Batch(blend, effectGroup);
        batches.add(b);
        return b;
    }

    public int triangleCount() {
        int n = 0;
        for (Batch b : batches) n += b.vertices / 3;
        return n;
    }
}
