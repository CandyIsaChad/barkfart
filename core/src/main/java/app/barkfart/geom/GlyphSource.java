package app.barkfart.geom;

/** Glyph metrics and atlas coordinates for laying out text (a font rasterized at basePx). */
public interface GlyphSource {
    /** Pixel size the glyphs were rasterized at (the line height text sizes are relative to). */
    float basePx();

    float lineHeight();

    /** A glyph, or null when the font has none (it is skipped, advancing by a space). */
    Glyph glyph(char c);

    final class Glyph {
        /** Texture id for the batch ("glyphs:..."), and the atlas rectangle (v is texture-space, bottom then top). */
        public final String texture;
        public final float u0, v0, u1, v1;
        /** Quad offset from the pen at the baseline (y up), size, and the pen advance, all in base pixels. */
        public final float x, y, w, h, advance;

        public Glyph(String texture, float u0, float v0, float u1, float v1, float x, float y, float w, float h, float advance) {
            this.texture = texture;
            this.u0 = u0;
            this.v0 = v0;
            this.u1 = u1;
            this.v1 = v1;
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
            this.advance = advance;
        }
    }
}
