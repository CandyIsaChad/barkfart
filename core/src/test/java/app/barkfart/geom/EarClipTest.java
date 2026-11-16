package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Random;
import org.junit.jupiter.api.Test;

class EarClipTest {
    private static double area(float[] p, int[] idx) {
        double s = 0;
        for (int i = 0; i < idx.length; i += 3) {
            int a = idx[i], b = idx[i + 1], c = idx[i + 2];
            s += Math.abs(((double) p[b * 2] - p[a * 2]) * (p[c * 2 + 1] - p[a * 2 + 1])
                    - ((double) p[b * 2 + 1] - p[a * 2 + 1]) * (p[c * 2] - p[a * 2])) / 2;
        }
        return s;
    }

    private static double polyArea(float[] p) {
        double s = 0;
        int n = p.length / 2;
        for (int i = 0, j = n - 1; i < n; j = i++) s += (double) p[j * 2] * p[i * 2 + 1] - (double) p[i * 2] * p[j * 2 + 1];
        return Math.abs(s / 2);
    }

    @Test
    void concaveLShapeInBothWindings() {
        float[] l = {0, 0, 2, 0, 2, 1, 1, 1, 1, 2, 0, 2};
        int[] t = EarClip.triangulate(l);
        assertEquals(4 * 3, t.length);
        assertEquals(3.0, area(l, t), 1e-9);
        float[] rev = new float[l.length];
        for (int i = 0; i < l.length / 2; i++) {
            rev[i * 2] = l[l.length - 2 - i * 2];
            rev[i * 2 + 1] = l[l.length - 1 - i * 2];
        }
        assertEquals(3.0, area(rev, EarClip.triangulate(rev)), 1e-9);
    }

    /** Random star-shaped (simple, concave) polygons: area preserved exactly. */
    @Test
    void randomStarPolygons() {
        Random r = new Random(5);
        for (int trial = 0; trial < 500; trial++) {
            int n = 3 + r.nextInt(30);
            float[] p = new float[n * 2];
            for (int i = 0; i < n; i++) {
                double a = 2 * Math.PI * i / n, rad = 1 + r.nextDouble() * 4;
                p[i * 2] = (float) (Math.cos(a) * rad);
                p[i * 2 + 1] = (float) (Math.sin(a) * rad);
            }
            int[] t = EarClip.triangulate(p);
            assertEquals((n - 2) * 3, t.length, "triangle count, trial " + trial);
            assertEquals(polyArea(p), area(p, t), 1e-3 * polyArea(p), "area, trial " + trial);
        }
    }

    /** Self-intersecting and degenerate input terminates and stays within the triangle budget. */
    @Test
    void hostileInputTerminates() {
        Random r = new Random(9);
        for (int trial = 0; trial < 500; trial++) {
            int n = 3 + r.nextInt(40);
            float[] p = new float[n * 2];
            for (int i = 0; i < p.length; i++) p[i] = r.nextInt(4) == 0 ? 0 : (float) r.nextGaussian();
            assertTrue(EarClip.triangulate(p).length <= (n - 2) * 3);
        }
    }
}
