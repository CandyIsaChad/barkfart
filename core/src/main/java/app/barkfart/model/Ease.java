package app.barkfart.model;

/**
 * Normalised cubic timing curve with CSS cubic-bezier semantics: the curve runs
 * from (0,0) to (1,1) through control points (x1,y1) and (x2,y2). x is time
 * fraction, y is value fraction. y may leave 0..1 (overshoot).
 */
public final class Ease {
    public static final Ease LINEAR = new Ease(0, 0, 1, 1);
    public static final Ease EASE_IN_OUT = new Ease(0.42f, 0f, 0.58f, 1f);
    public static final Ease EASE_IN = new Ease(0.42f, 0f, 1f, 1f);
    public static final Ease EASE_OUT = new Ease(0f, 0f, 0.58f, 1f);
    public static final Ease SINE_IN_OUT = new Ease(0.37f, 0f, 0.63f, 1f);
    public static final Ease STRONG_IN_OUT = new Ease(0.87f, 0f, 0.13f, 1f);
    /** Pulls back before moving (y dips below 0). */
    public static final Ease ANTICIPATE = new Ease(0.36f, 0f, 0.66f, -0.56f);
    /** Goes past the target and settles back (y rises above 1). */
    public static final Ease OVERSHOOT = new Ease(0.34f, 1.56f, 0.64f, 1f);
    public static final Ease ANTICIPATE_OVERSHOOT = new Ease(0.68f, -0.6f, 0.32f, 1.6f);

    /**
     * Curves a cubic cannot draw (Stick Nodes 2.5.6 camera easing: bounce,
     * elastic). CUBIC uses the control points; the others ignore them.
     */
    public enum Kind { CUBIC, BOUNCE_OUT, BOUNCE_IN, ELASTIC_OUT, ELASTIC_IN }

    public static final Ease BOUNCE_OUT = new Ease(Kind.BOUNCE_OUT);
    public static final Ease BOUNCE_IN = new Ease(Kind.BOUNCE_IN);
    public static final Ease ELASTIC_OUT = new Ease(Kind.ELASTIC_OUT);
    public static final Ease ELASTIC_IN = new Ease(Kind.ELASTIC_IN);

    public final float x1, y1, x2, y2;
    public final Kind kind;

    public Ease(float x1, float y1, float x2, float y2) {
        this.x1 = clamp01(x1);
        this.y1 = y1;
        this.x2 = clamp01(x2);
        this.y2 = y2;
        this.kind = Kind.CUBIC;
    }

    public Ease(Kind kind) {
        this.x1 = 0;
        this.y1 = 0;
        this.x2 = 1;
        this.y2 = 1;
        this.kind = kind == null ? Kind.CUBIC : kind;
    }

    static float bounceOut(float u) {
        final float n = 7.5625f, d = 2.75f;
        if (u < 1 / d) return n * u * u;
        if (u < 2 / d) {
            u -= 1.5f / d;
            return n * u * u + 0.75f;
        }
        if (u < 2.5f / d) {
            u -= 2.25f / d;
            return n * u * u + 0.9375f;
        }
        u -= 2.625f / d;
        return n * u * u + 0.984375f;
    }

    static float elasticOut(float u) {
        return (float) (Math.pow(2, -10 * u) * Math.sin((u * 10 - 0.75) * (2 * Math.PI / 3)) + 1);
    }

    private static float clamp01(float v) {
        return v < 0 ? 0 : (v > 1 ? 1 : v);
    }

    /** Value fraction for time fraction u in [0,1]. */
    public float apply(float u) {
        if (u <= 0) return 0;
        if (u >= 1) return 1;
        switch (kind) {
            case BOUNCE_OUT: return bounceOut(u);
            case BOUNCE_IN: return 1 - bounceOut(1 - u);
            case ELASTIC_OUT: return elasticOut(u);
            case ELASTIC_IN: return 1 - elasticOut(1 - u);
            default: break;
        }
        // Solve x(s) = u for s with Newton, falling back to bisection.
        float s = u;
        for (int i = 0; i < 8; i++) {
            float x = bez(s, x1, x2) - u;
            if (Math.abs(x) < 1e-6f) return bez(s, y1, y2);
            float dx = dbez(s, x1, x2);
            if (Math.abs(dx) < 1e-6f) break;
            s -= x / dx;
            if (s < 0 || s > 1) break;
        }
        float lo = 0, hi = 1;
        s = u;
        for (int i = 0; i < 40; i++) {
            float x = bez(s, x1, x2);
            if (Math.abs(x - u) < 1e-6f) break;
            if (x < u) lo = s; else hi = s;
            s = (lo + hi) / 2;
        }
        return bez(s, y1, y2);
    }

    private static float bez(float s, float p1, float p2) {
        float v = 1 - s;
        return 3 * v * v * s * p1 + 3 * v * s * s * p2 + s * s * s;
    }

    private static float dbez(float s, float p1, float p2) {
        float v = 1 - s;
        return 3 * v * v * p1 + 6 * v * s * (p2 - p1) + 3 * s * s * (1 - p2);
    }

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof Ease)) return false;
        Ease e = (Ease) o;
        return kind == e.kind && x1 == e.x1 && y1 == e.y1 && x2 == e.x2 && y2 == e.y2;
    }

    @Override
    public int hashCode() {
        return Float.floatToIntBits(x1) ^ Float.floatToIntBits(y1) * 7 ^ Float.floatToIntBits(x2) * 13
                ^ Float.floatToIntBits(y2) * 31 ^ kind.ordinal() * 101;
    }
}
