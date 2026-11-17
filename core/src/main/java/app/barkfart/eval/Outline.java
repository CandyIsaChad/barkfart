package app.barkfart.eval;

import app.barkfart.model.Node;

/** Convex outlines for node shapes, in view space, from the segment A (parent) to B (node). */
final class Outline {
    private Outline() {}

    static float[] of(Node.Shape shape, float ax, float ay, float bx, float by, float width) {
        return of(shape, ax, ay, bx, by, width, false, 0);
    }

    /**
     * upsideDown puts the triangle's apex at the parent (A) and its base at the
     * node (B); rightTriangle -1 / +1 puts the right angle on one side of the base.
     */
    static float[] of(Node.Shape shape, float ax, float ay, float bx, float by, float width, boolean upsideDown, int rightTriangle) {
        if (shape == Node.Shape.TRIANGLE && upsideDown) {
            float t;
            t = ax; ax = bx; bx = t;
            t = ay; ay = by; by = t;
            rightTriangle = -rightTriangle;   // swapping ends mirrors the sides
        }
        float dx = bx - ax, dy = by - ay;
        float len = (float) Math.sqrt(dx * dx + dy * dy);
        if (len < 1e-6f || width <= 0) return null;
        float ux = dx / len, uy = dy / len;           // along the segment
        float nx = -uy * width / 2, ny = ux * width / 2; // half-width normal
        switch (shape) {
            case SQUARE_LINE:
                return new float[] {ax + nx, ay + ny, bx + nx, by + ny, bx - nx, by - ny, ax - nx, ay - ny};
            case TRIANGLE:
                if (rightTriangle > 0) return new float[] {ax + nx, ay + ny, bx + nx, by + ny, ax - nx, ay - ny};
                if (rightTriangle < 0) return new float[] {ax + nx, ay + ny, bx - nx, by - ny, ax - nx, ay - ny};
                return new float[] {ax + nx, ay + ny, bx, by, ax - nx, ay - ny};
            case ELLIPSE: {
                float cx = (ax + bx) / 2, cy = (ay + by) / 2, ra = len / 2, rb = width / 2;
                int n = Math.max(12, Math.min(96, (int) (Math.max(ra, rb) / 1.5f) + 12));
                float[] p = new float[n * 2];
                for (int i = 0; i < n; i++) {
                    double a = 2 * Math.PI * i / n;
                    float ca = (float) Math.cos(a) * ra, sa = (float) Math.sin(a) * rb;
                    p[i * 2] = cx + ux * ca - uy * sa;
                    p[i * 2 + 1] = cy + uy * ca + ux * sa;
                }
                return p;
            }
            default:
                return null;
        }
    }
}
