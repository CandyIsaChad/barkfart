package app.barkfart.eval;

/** Immutable 2D affine transform: x' = a x + c y + tx, y' = b x + d y + ty. */
public final class Affine {
    public static final Affine IDENTITY = new Affine(1, 0, 0, 1, 0, 0);

    public final double a, b, c, d, tx, ty;

    public Affine(double a, double b, double c, double d, double tx, double ty) {
        this.a = a;
        this.b = b;
        this.c = c;
        this.d = d;
        this.tx = tx;
        this.ty = ty;
    }

    public static Affine translate(double x, double y) {
        return new Affine(1, 0, 0, 1, x, y);
    }

    public static Affine rotateDeg(double deg) {
        double r = Math.toRadians(deg), cs = Math.cos(r), sn = Math.sin(r);
        return new Affine(cs, sn, -sn, cs, 0, 0);
    }

    public static Affine scale(double sx, double sy) {
        return new Affine(sx, 0, 0, sy, 0, 0);
    }

    /** this * o: applies o first, then this. */
    public Affine mul(Affine o) {
        return new Affine(a * o.a + c * o.b, b * o.a + d * o.b,
                a * o.c + c * o.d, b * o.c + d * o.d,
                a * o.tx + c * o.ty + tx, b * o.tx + d * o.ty + ty);
    }

    public double mapX(double x, double y) {
        return a * x + c * y + tx;
    }

    public double mapY(double x, double y) {
        return b * x + d * y + ty;
    }

    /** Uniform scale estimate, used for stroke widths and radii. */
    public double scaleFactor() {
        return Math.sqrt(Math.abs(a * d - b * c));
    }

    public Affine inverse() {
        double det = a * d - b * c;
        if (Math.abs(det) < 1e-12) return IDENTITY;
        double ia = d / det, ib = -b / det, ic = -c / det, id = a / det;
        return new Affine(ia, ib, ic, id, -(ia * tx + ic * ty), -(ib * tx + id * ty));
    }

    public boolean isFinite() {
        return finite(a) && finite(b) && finite(c) && finite(d) && finite(tx) && finite(ty);
    }

    private static boolean finite(double v) {
        return !Double.isNaN(v) && !Double.isInfinite(v);
    }
}
