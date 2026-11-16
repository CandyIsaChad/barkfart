package app.barkfart.geom;

/**
 * Ear-clipping triangulation of a simple polygon (either winding). For
 * self-intersecting input it stops clipping ears when none is found and fans
 * the rest, so it always terminates and always covers the outline.
 */
final class EarClip {
    private EarClip() {}

    static int[] triangulate(float[] p) {
        int n = p.length / 2;
        if (n < 3) return new int[0];
        int[] v = new int[n];
        boolean ccw = area(p) > 0;
        for (int i = 0; i < n; i++) v[i] = ccw ? i : n - 1 - i;
        int[] out = new int[(n - 2) * 3];
        int o = 0, count = n, guard = 2 * n * n;
        int i = 0;
        while (count > 3 && guard-- > 0) {
            int a = v[(i + count - 1) % count], b = v[i % count], c = v[(i + 1) % count];
            if (isEar(p, v, count, a, b, c)) {
                out[o++] = a;
                out[o++] = b;
                out[o++] = c;
                int k = i % count;
                System.arraycopy(v, k + 1, v, k, count - k - 1);
                count--;
                guard = 2 * count * count;
            } else {
                i++;
            }
        }
        // Remaining vertices (3, or a stuck self-intersecting rest): fan them.
        for (int k = 1; k + 1 < count && o + 3 <= out.length; k++) {
            out[o++] = v[0];
            out[o++] = v[k];
            out[o++] = v[k + 1];
        }
        int[] trimmed = new int[o];
        System.arraycopy(out, 0, trimmed, 0, o);
        return trimmed;
    }

    private static double area(float[] p) {
        double s = 0;
        int n = p.length / 2;
        for (int i = 0, j = n - 1; i < n; j = i++) s += (double) p[j * 2] * p[i * 2 + 1] - (double) p[i * 2] * p[j * 2 + 1];
        return s / 2;
    }

    private static double cross(float[] p, int a, int b, int c) {
        return ((double) p[b * 2] - p[a * 2]) * ((double) p[c * 2 + 1] - p[a * 2 + 1])
                - ((double) p[b * 2 + 1] - p[a * 2 + 1]) * ((double) p[c * 2] - p[a * 2]);
    }

    private static boolean isEar(float[] p, int[] v, int count, int a, int b, int c) {
        if (cross(p, a, b, c) <= 0) return false;  // reflex or degenerate (ccw order)
        for (int k = 0; k < count; k++) {
            int q = v[k];
            if (q == a || q == b || q == c) continue;
            if (cross(p, a, b, q) >= 0 && cross(p, b, c, q) >= 0 && cross(p, c, a, q) >= 0) return false;
        }
        return true;
    }
}
