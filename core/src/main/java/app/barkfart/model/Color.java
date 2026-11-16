package app.barkfart.model;

/** Immutable straight (non-premultiplied) sRGB colour, channels 0..1. */
public final class Color {
    public static final Color BLACK = new Color(0, 0, 0, 1);
    public static final Color WHITE = new Color(1, 1, 1, 1);

    public final float r, g, b, a;

    public Color(float r, float g, float b, float a) {
        this.r = r;
        this.g = g;
        this.b = b;
        this.a = a;
    }

    /** From 0xRRGGBBAA. */
    public static Color rgba8888(int v) {
        return new Color(((v >>> 24) & 0xff) / 255f, ((v >>> 16) & 0xff) / 255f,
                ((v >>> 8) & 0xff) / 255f, (v & 0xff) / 255f);
    }

    public int toRgba8888() {
        return (ch(r) << 24) | (ch(g) << 16) | (ch(b) << 8) | ch(a);
    }

    private static int ch(float v) {
        int i = Math.round(v * 255f);
        return i < 0 ? 0 : (i > 255 ? 255 : i);
    }

    public Color lerp(Color o, float t) {
        return new Color(r + (o.r - r) * t, g + (o.g - g) * t, b + (o.b - b) * t, a + (o.a - a) * t);
    }

    public Color withAlpha(float alpha) {
        return new Color(r, g, b, alpha);
    }

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof Color)) return false;
        Color c = (Color) o;
        return Float.compare(r, c.r) == 0 && Float.compare(g, c.g) == 0
                && Float.compare(b, c.b) == 0 && Float.compare(a, c.a) == 0;
    }

    @Override
    public int hashCode() {
        return ((Float.floatToIntBits(r) * 31 + Float.floatToIntBits(g)) * 31
                + Float.floatToIntBits(b)) * 31 + Float.floatToIntBits(a);
    }

    @Override
    public String toString() {
        return String.format("#%08x", toRgba8888());
    }
}
