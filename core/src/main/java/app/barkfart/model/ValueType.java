package app.barkfart.model;

/** Kinds of animatable value and how each interpolates. */
public enum ValueType {
    /** Plain float. */
    FLOAT,
    /** Degrees, stored unwrapped, interpolated numerically (DESIGN Q1). */
    ANGLE,
    /** Double, used for time values (symbol time offset, speed). */
    DOUBLE,
    /** Straight sRGB colour, per-channel interpolation (DESIGN Q3). */
    COLOR,
    /** Boolean, always held. */
    BOOL;

    public boolean continuous() {
        return this != BOOL;
    }

    public Object lerp(Object a, Object b, float t) {
        switch (this) {
            case FLOAT:
            case ANGLE: {
                float x = (Float) a, y = (Float) b;
                return x + (y - x) * t;
            }
            case DOUBLE: {
                double x = (Double) a, y = (Double) b;
                return x + (y - x) * t;
            }
            case COLOR:
                return ((Color) a).lerp((Color) b, t);
            default:
                return a;
        }
    }

    /** Adds a scalar offset (used by additive modifiers); non-numeric types are unchanged. */
    public Object add(Object v, double d) {
        switch (this) {
            case FLOAT:
            case ANGLE:
                return (float) ((Float) v + d);
            case DOUBLE:
                return (Double) v + d;
            default:
                return v;
        }
    }

    /** Maps a numeric source value through scale and offset into this type. */
    public Object fromScalar(double s, Object fallback) {
        switch (this) {
            case FLOAT:
            case ANGLE:
                return (float) s;
            case DOUBLE:
                return s;
            case BOOL:
                return s >= 0.5;
            default:
                return fallback;
        }
    }

    /** Numeric view of a value, for links between properties of different types. */
    public static double toScalar(Object v) {
        if (v instanceof Float) return (Float) v;
        if (v instanceof Double) return (Double) v;
        if (v instanceof Boolean) return ((Boolean) v) ? 1 : 0;
        if (v instanceof Color) {
            Color c = (Color) v;
            return (c.r + c.g + c.b) / 3.0;
        }
        return 0;
    }

    /** True when a and b differ by at most eps (colours per channel, eps/255 scale). */
    public boolean near(Object a, Object b, double eps) {
        switch (this) {
            case FLOAT:
            case ANGLE:
                return Math.abs((Float) a - (Float) b) <= eps;
            case DOUBLE:
                return Math.abs((Double) a - (Double) b) <= eps;
            case COLOR: {
                Color x = (Color) a, y = (Color) b;
                double e = 1.0 / 255.0;
                return Math.abs(x.r - y.r) <= e && Math.abs(x.g - y.g) <= e
                        && Math.abs(x.b - y.b) <= e && Math.abs(x.a - y.a) <= e;
            }
            default:
                return a.equals(b);
        }
    }

    public Object checked(Object v) {
        boolean ok;
        switch (this) {
            case FLOAT:
            case ANGLE: ok = v instanceof Float; break;
            case DOUBLE: ok = v instanceof Double; break;
            case COLOR: ok = v instanceof Color; break;
            default: ok = v instanceof Boolean;
        }
        if (!ok) throw new IllegalArgumentException(this + " cannot hold " + (v == null ? "null" : v.getClass()));
        return v;
    }
}
