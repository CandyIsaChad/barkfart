package app.barkfart.eval;

import app.barkfart.model.Node;

/** Convex outlines for node shapes, in view space, from the segment A (parent) to B (node). */
final class Outline {
    private Outline() {}

    /**
     * Trapezoid: width at A, width x ratio at B, optionally half-round ends.
     * Polygon: regular, inscribed in the circle on AB as diameter, a corner at B.
     * Both unverified against the original's renderer (no bundled file uses them).
     */
    static float[] of(Node node, float ax, float ay, float bx, float by, float width) {
        float dx = bx - ax, dy = by - ay;
        float len = (float) Math.sqrt(dx * dx + dy * dy);
        if (node.shape == Node.Shape.POLYGON) {
            if (len < 1e-6f) return null;
            int n = node.sides();
            float cx = (ax + bx) / 2, cy = (ay + by) / 2, r = len / 2;
            double a0 = Math.atan2(by - cy, bx - cx);
            float[] p = new float[n * 2];
            for (int i = 0; i < n; i++) {
                double a = a0 + 2 * Math.PI * i / n;
                p[i * 2] = cx + r * (float) Math.cos(a);
                p[i * 2 + 1] = cy + r * (float) Math.sin(a);
            }
            return p;
        }
        if (node.shape == Node.Shape.TRAPEZOID) {
            if (len < 1e-6f || width <= 0) return null;
            float ux = dx / len, uy = dy / len;
            float w0 = width / 2, w1 = width * Math.max(0, node.trapezoidRatio) / 2;
            java.util.List<Float> pts = new java.util.ArrayList<Float>();
            // right side from A to B, then around B, left side back, then around A
            addEnd(pts, ax, ay, ux, uy, w0, node.trapezoidRoundStart, false);
            addEnd(pts, bx, by, ux, uy, w1, node.trapezoidRoundEnd, true);
            float[] p = new float[pts.size()];
            for (int i = 0; i < p.length; i++) p[i] = pts.get(i);
            return p;
        }
        return of(node.shape, ax, ay, bx, by, width, node.triangleUpsideDown, node.rightTriangle);
    }

    /** One end of a trapezoid: its two corners, or a half circle bulging outward. */
    private static void addEnd(java.util.List<Float> pts, float x, float y, float ux, float uy, float hw, boolean round, boolean atB) {
        float nx = -uy, ny = ux;   // left normal
        if (!round || hw <= 0) {
            // order: A-left, A-right, B-right, B-left
            if (!atB) {
                pts.add(x + nx * hw);
                pts.add(y + ny * hw);
                pts.add(x - nx * hw);
                pts.add(y - ny * hw);
            } else {
                pts.add(x - nx * hw);
                pts.add(y - ny * hw);
                pts.add(x + nx * hw);
                pts.add(y + ny * hw);
            }
            return;
        }
        int n = 12;
        double base = Math.atan2(ny, nx);   // left normal direction
        for (int i = 0; i <= n; i++) {
            // A: from left normal through backward (-u) to right normal; B: from right through forward (+u) to left
            double a = atB ? base + Math.PI + Math.PI * i / n : base + Math.PI * i / n;
            pts.add(x + hw * (float) Math.cos(a));
            pts.add(y + hw * (float) Math.sin(a));
        }
    }

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
