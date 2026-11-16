package app.barkfart.eval;

import app.barkfart.model.Modifier;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.ValueType;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;

/**
 * Resolves property values at a time: keys, then modifiers. Memoised per
 * evaluate() call. Link and DelayFollow can form cycles; a property reached
 * again while it is being resolved returns its keys-only value and records a
 * CYCLE diagnostic, so resolution always terminates. Recursion depth through
 * links is bounded by the number of properties (each can be visiting once).
 */
public final class PropertyResolver {
    private final Project project;
    private final List<Diagnostic> diagnostics;
    private final HashMap<String, Object> memo = new HashMap<String, Object>();
    private final HashSet<String> visiting = new HashSet<String>();
    /** Hard cap on nested link hops, in case a project has a huge chain. */
    private static final int MAX_LINK_DEPTH = 256;
    private int linkDepth;

    public PropertyResolver(Project project, List<Diagnostic> diagnostics) {
        this.project = project;
        this.diagnostics = diagnostics;
    }

    public float f(Property p, double t) {
        return (Float) resolve(p, t);
    }

    public double d(Property p, double t) {
        return (Double) resolve(p, t);
    }

    public boolean b(Property p, double t) {
        return (Boolean) resolve(p, t);
    }

    public Object resolve(Property p, double t) {
        String key = p.id + '@' + Double.doubleToLongBits(t);
        Object m = memo.get(key);
        if (m != null) return m;
        if (visiting.contains(key) || linkDepth >= MAX_LINK_DEPTH) {
            diagnostics.add(new Diagnostic(Diagnostic.Kind.CYCLE, p.id));
            return p.keyValue(t);
        }
        visiting.add(key);
        linkDepth++;
        Object v;
        try {
            v = compute(p, t);
        } finally {
            linkDepth--;
            visiting.remove(key);
        }
        memo.put(key, v);
        return v;
    }

    private Object compute(Property p, double t) {
        List<Modifier> mods = p.modifiers();
        double kt = t;
        int start = 0;
        if (!mods.isEmpty() && mods.get(0) instanceof Modifier.Loop) {
            Modifier.Loop loop = (Modifier.Loop) mods.get(0);
            if (loop.enabled) kt = loop.remap(t);
            start = 1;
        }
        Object v = p.keyValue(kt);
        for (int i = start; i < mods.size(); i++) {
            Modifier mod = mods.get(i);
            if (!mod.enabled) continue;
            if (mod instanceof Modifier.Wiggle) {
                Modifier.Wiggle w = (Modifier.Wiggle) mod;
                v = p.type.add(v, w.amplitude * Noise.fbm(t * w.frequency, w.seed, w.octaves));
            } else if (mod instanceof Modifier.Sine) {
                Modifier.Sine s = (Modifier.Sine) mod;
                v = p.type.add(v, s.amplitude * Math.sin(2 * Math.PI * (s.frequency * t + s.phase)));
            } else if (mod instanceof Modifier.Link) {
                Modifier.Link l = (Modifier.Link) mod;
                v = follow(p, v, l.source, t, l.scale, l.offset);
            } else if (mod instanceof Modifier.DelayFollow) {
                Modifier.DelayFollow df = (Modifier.DelayFollow) mod;
                v = follow(p, v, df.source, t - df.delay, df.scale, df.offset);
            }
        }
        return sanitize(p, v);
    }

    private Object follow(Property p, Object current, String sourceId, double t, float scale, float offset) {
        Property src = project.property(sourceId);
        if (src == null) {
            diagnostics.add(new Diagnostic(Diagnostic.Kind.MISSING_REF, p.id + "->" + sourceId));
            return current;
        }
        Object s = resolve(src, t);
        if (src.type == p.type && (!p.type.continuous() || p.type == ValueType.COLOR)) return s;
        return p.type.fromScalar(ValueType.toScalar(s) * scale + offset, current);
    }

    /** Replaces NaN/infinite numbers (from extreme modifier values) with the keys-only value. */
    private Object sanitize(Property p, Object v) {
        double x;
        if (v instanceof Float) x = (Float) v;
        else if (v instanceof Double) x = (Double) v;
        else return v;
        if (Double.isNaN(x) || Double.isInfinite(x)) {
            diagnostics.add(new Diagnostic(Diagnostic.Kind.BAD_VALUE, p.id));
            return p.type.fromScalar(0, v);
        }
        return v;
    }
}
