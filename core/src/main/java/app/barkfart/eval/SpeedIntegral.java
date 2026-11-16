package app.barkfart.eval;

import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Property;
import app.barkfart.model.Track;

/**
 * Local time contributed by an instance's speed: the integral of speed from
 * 0 to t (DESIGN Q9). Constant speed gives speed x t; keyed speed is
 * integrated exactly for HOLD/LINEAR segments and numerically otherwise, so a
 * clip slowing to a stop never runs backwards.
 */
final class SpeedIntegral {
    /** Numeric step for eased segments or modifiers (seconds), and a cost cap. */
    private static final double STEP = 1.0 / 240;
    private static final int MAX_STEPS = 4096;

    private SpeedIntegral() {}

    static double elapsed(Property speed, double t, PropertyResolver r) {
        Track tr = speed.track();
        if (tr.isEmpty() && speed.modifiers().isEmpty()) return (Double) speed.value() * t;
        if (!speed.modifiers().isEmpty() || hasBezier(tr)) return numeric(speed, t, r);
        return exact(tr, t);
    }

    private static boolean hasBezier(Track tr) {
        for (Key k : tr.keys()) if (k.interp == Interp.BEZIER) return true;
        return false;
    }

    /** Integral of a HOLD/LINEAR track from 0 to t (negative t integrates backwards). */
    static double exact(Track tr, double t) {
        if (t == 0) return 0;
        double a = Math.min(0, t), b = Math.max(0, t);
        double sum = 0;
        int n = tr.size();
        // Breakpoints: integration bounds plus every key time inside them.
        double prev = a;
        for (int i = 0; i <= n; i++) {
            double next = i < n ? Math.min(Math.max(tr.get(i).time, a), b) : b;
            if (next > prev) sum += segment(tr, prev, next);
            prev = Math.max(prev, next);
        }
        return t < 0 ? -sum : sum;
    }

    /**
     * Integral over [x0, x1] where no key lies strictly inside: the speed is
     * constant (HOLD) or linear there, so the trapezoid of the values just
     * inside each end is exact (just inside, so a HOLD step at x1 is excluded).
     */
    private static double segment(Track tr, double x0, double x1) {
        double e = (x1 - x0) * 1e-9;
        double v0 = (Double) tr.sample(x0 + e), v1 = (Double) tr.sample(x1 - e);
        return (x1 - x0) * (v0 + v1) / 2;
    }

    private static double numeric(Property speed, double t, PropertyResolver r) {
        int steps = (int) Math.min(MAX_STEPS, Math.max(1, Math.ceil(Math.abs(t) / STEP)));
        double h = t / steps, sum = 0;
        double prev = r.d(speed, 0);
        for (int i = 1; i <= steps; i++) {
            double cur = r.d(speed, i * h);
            sum += (prev + cur) / 2 * h;
            prev = cur;
        }
        return sum;
    }
}
