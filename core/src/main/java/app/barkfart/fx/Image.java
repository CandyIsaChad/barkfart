package app.barkfart.fx;

/**
 * Premultiplied RGBA float image (row 0 at the top). Mapped to view space:
 * pixel (i, j) covers view x = x0 + (i + 0.5) / scale, y = y0 - (j + 0.5) / scale.
 */
public final class Image {
    public final int w, h;
    public final float[] px;
    /** View coordinates of the top-left corner and pixels per view unit. */
    public final float x0, y0, scale;

    public Image(int w, int h, float x0, float y0, float scale) {
        this.w = Math.max(0, w);
        this.h = Math.max(0, h);
        this.px = new float[this.w * this.h * 4];
        this.x0 = x0;
        this.y0 = y0;
        this.scale = scale;
    }

    public Image blank() {
        return new Image(w, h, x0, y0, scale);
    }

    public Image copy() {
        Image c = blank();
        System.arraycopy(px, 0, c.px, 0, px.length);
        return c;
    }
}
