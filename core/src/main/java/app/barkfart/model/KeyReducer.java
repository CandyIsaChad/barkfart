package app.barkfart.model;

import java.util.ArrayList;
import java.util.List;

/**
 * Optional key reduction (DESIGN.md §7): drops interior keys when the
 * remaining keys reproduce the original curve within a per-type tolerance.
 * First and last keys are always kept. Each candidate is checked against the
 * ORIGINAL curve, so error never accumulates.
 */
public final class KeyReducer {
    /** Default tolerances: 0.01 units / degrees, 1e-6 for time values, 1/255 per colour channel. */
    public static double defaultEpsilon(ValueType t) {
        return t == ValueType.DOUBLE ? 1e-6 : 0.01;
    }

    private static final int SAMPLES_PER_SPAN = 12;

    private KeyReducer() {}

    /** Reduces one track in place. Returns the number of keys removed. */
    public static int reduce(Track track, double eps) {
        if (track.size() < 3) return 0;
        Track original = track.copy();
        int removed = 0;
        int i = 1;
        while (i < track.size() - 1) {
            Key prev = track.get(i - 1), cur = track.get(i), next = track.get(i + 1);
            Track trial = track.copy();
            trial.removeAt(i);
            if (matches(original, trial, prev.time, next.time, eps) && near(original, trial, cur.time, eps)) {
                track.removeAt(i);
                removed++;
            } else {
                i++;
            }
        }
        return removed;
    }

    private static boolean near(Track a, Track b, double t, double eps) {
        return a.type().near(a.sample(t), b.sample(t), eps);
    }

    private static boolean matches(Track original, Track trial, double t0, double t1, double eps) {
        // Check every original key time in the span, plus evenly spaced samples.
        for (Key k : original.keys()) {
            if (k.time >= t0 && k.time <= t1 && !near(original, trial, k.time, eps)) return false;
        }
        for (int s = 1; s < SAMPLES_PER_SPAN; s++) {
            double t = t0 + (t1 - t0) * s / SAMPLES_PER_SPAN;
            if (!near(original, trial, t, eps)) return false;
        }
        // Just before t1 matters for HOLD steps.
        double before = t1 - (t1 - t0) * 1e-6;
        return near(original, trial, before, eps);
    }

    /** Reduces every track in the project. Returns keys removed. */
    public static int reduce(Project p) {
        int n = 0;
        List<Property> props = new ArrayList<Property>();
        for (Symbol s : p.library.values()) for (Layer l : s.layers) props.addAll(l.properties());
        for (Property prop : props) n += reduce(prop.track(), defaultEpsilon(prop.type));
        return n;
    }
}
