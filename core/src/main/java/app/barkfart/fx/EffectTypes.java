package app.barkfart.fx;

import app.barkfart.eval.ResolvedEffect;
import app.barkfart.model.Color;

/**
 * Every effect type: its parameters (defaults and limits), how far it reaches
 * past the content (padding), and what it does (via Kernels). Lengths are in
 * project pixels, so effects look the same at any zoom or export size.
 * Unknown types pass the image through unchanged.
 */
public final class EffectTypes {
    private EffectTypes() {}

    public static final String BLUR = "blur.gaussian", GLOW = "glow", SHADOW = "shadow.drop", INVERT = "invert",
            ADJUST = "color.adjust", TINT = "tint";
    public static final String[] ALL = {BLUR, GLOW, SHADOW, INVERT, ADJUST, TINT};

    /** Largest radius or offset accepted, in project pixels. */
    public static final float MAX_LENGTH = 400;

    public static boolean known(String type) {
        for (String t : ALL) if (t.equals(type)) return true;
        return false;
    }

    /** Blur sigma for a radius: the radius covers about two standard deviations. */
    public static float sigma(float radius) {
        return radius / 2;
    }

    /** View units the effect's output can reach beyond the content. */
    public static float padding(ResolvedEffect e) {
        if (BLUR.equals(e.type)) return 1.5f * f(e, "radius", 6, 0, MAX_LENGTH);
        if (GLOW.equals(e.type)) return 1.5f * f(e, "radius", 8, 0, MAX_LENGTH);
        if (SHADOW.equals(e.type)) {
            return 1.5f * f(e, "radius", 6, 0, MAX_LENGTH) + Math.max(Math.abs(f(e, "dx", 8, -MAX_LENGTH, MAX_LENGTH)), Math.abs(f(e, "dy", -8, -MAX_LENGTH, MAX_LENGTH)));
        }
        return 0;
    }

    /** Runs one effect (mix applied) on a premultiplied image; may return a new image. */
    public static Image apply(ResolvedEffect e, Image in) {
        float mix = clamp(e.mix, 0, 1, 1);
        if (mix <= 0 || !known(e.type)) return in;
        Image out = in.copy();
        float s = in.scale;
        if (BLUR.equals(e.type)) {
            Kernels.blur(out, sigma(f(e, "radius", 6, 0, MAX_LENGTH)) * s);
        } else if (GLOW.equals(e.type)) {
            Color c = c(e, "color", Color.WHITE);
            out = Kernels.under(in, 0, 0, sigma(f(e, "radius", 8, 0, MAX_LENGTH)) * s, c.r, c.g, c.b, c.a, f(e, "strength", 1, 0, 4));
        } else if (SHADOW.equals(e.type)) {
            Color c = c(e, "color", new Color(0, 0, 0, 0.6f));
            float dx = f(e, "dx", 8, -MAX_LENGTH, MAX_LENGTH) * s, dy = -f(e, "dy", -8, -MAX_LENGTH, MAX_LENGTH) * s;   // y up -> rows down
            out = Kernels.under(in, dx, dy, sigma(f(e, "radius", 6, 0, MAX_LENGTH)) * s, c.r, c.g, c.b, c.a, 1);
        } else if (INVERT.equals(e.type)) {
            Kernels.invert(out);
        } else if (ADJUST.equals(e.type)) {
            Kernels.adjust(out, f(e, "hue", 0, -3600, 3600), f(e, "saturation", 1, 0, 4), f(e, "brightness", 0, -1, 1));
        } else if (TINT.equals(e.type)) {
            Color c = c(e, "color", new Color(1, 0, 0, 1));
            Kernels.tint(out, c.r, c.g, c.b, f(e, "amount", 0.5f, 0, 1) * c.a);
        }
        return Kernels.lerp(in, out, mix);
    }

    /** A float parameter with its default, clamped; NaN and missing give the default. */
    public static float f(ResolvedEffect e, String name, float def, float min, float max) {
        Object o = e.params.get(name);
        float v = o instanceof Number ? ((Number) o).floatValue() : def;
        return clamp(v, min, max, def);
    }

    public static Color c(ResolvedEffect e, String name, Color def) {
        Object o = e.params.get(name);
        return o instanceof Color ? (Color) o : def;
    }

    private static float clamp(float v, float min, float max, float def) {
        if (v != v) return def;
        return v < min ? min : (v > max ? max : v);
    }
}
