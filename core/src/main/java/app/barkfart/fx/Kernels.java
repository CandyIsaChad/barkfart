package app.barkfart.fx;

/**
 * Image operations on premultiplied RGBA float images. These are the
 * reference definition of every effect; the GL shaders mirror them.
 */
public final class Kernels {
    private Kernels() {}

    /** Largest blur sigma in pixels; larger requests are clamped (cost and memory guard). */
    public static final float MAX_SIGMA = 64;

    /** Separable Gaussian blur of all four channels, in place. Edges read as transparent. */
    public static void blur(Image im, float sigma) {
        if (!(sigma > 0.05f)) return;
        sigma = Math.min(sigma, MAX_SIGMA);
        int r = (int) Math.ceil(3 * sigma);
        float[] k = new float[2 * r + 1];
        float sum = 0;
        for (int i = -r; i <= r; i++) {
            k[i + r] = (float) Math.exp(-(i * i) / (2.0 * sigma * sigma));
            sum += k[i + r];
        }
        for (int i = 0; i < k.length; i++) k[i] /= sum;
        float[] tmp = new float[im.px.length];
        int w = im.w, h = im.h;
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                float a0 = 0, a1 = 0, a2 = 0, a3 = 0;
                for (int t = -r; t <= r; t++) {
                    int xx = x + t;
                    if (xx < 0 || xx >= w) continue;
                    int o = (y * w + xx) * 4;
                    float kw = k[t + r];
                    a0 += im.px[o] * kw;
                    a1 += im.px[o + 1] * kw;
                    a2 += im.px[o + 2] * kw;
                    a3 += im.px[o + 3] * kw;
                }
                int o = (y * w + x) * 4;
                tmp[o] = a0;
                tmp[o + 1] = a1;
                tmp[o + 2] = a2;
                tmp[o + 3] = a3;
            }
        }
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                float a0 = 0, a1 = 0, a2 = 0, a3 = 0;
                for (int t = -r; t <= r; t++) {
                    int yy = y + t;
                    if (yy < 0 || yy >= h) continue;
                    int o = (yy * w + x) * 4;
                    float kw = k[t + r];
                    a0 += tmp[o] * kw;
                    a1 += tmp[o + 1] * kw;
                    a2 += tmp[o + 2] * kw;
                    a3 += tmp[o + 3] * kw;
                }
                int o = (y * w + x) * 4;
                im.px[o] = a0;
                im.px[o + 1] = a1;
                im.px[o + 2] = a2;
                im.px[o + 3] = a3;
            }
        }
    }

    /**
     * A shadow or glow: the source's alpha, shifted by (dx, dy) pixels (y down),
     * blurred, times a straight colour and a strength; drawn under the source.
     */
    public static Image under(Image src, float dx, float dy, float sigma, float r, float g, float b, float a, float strength) {
        Image s = src.blank();
        int w = src.w, h = src.h;
        int ix = Math.round(dx), iy = Math.round(dy);
        for (int y = 0; y < h; y++) {
            int sy = y - iy;
            if (sy < 0 || sy >= h) continue;
            for (int x = 0; x < w; x++) {
                int sx = x - ix;
                if (sx < 0 || sx >= w) continue;
                s.px[(y * w + x) * 4 + 3] = src.px[(sy * w + sx) * 4 + 3];
            }
        }
        blur(s, sigma);
        Image out = src.blank();
        for (int i = 0; i < w * h; i++) {
            float sa = Math.min(1f, s.px[i * 4 + 3] * strength) * a;   // shadow alpha
            float top = src.px[i * 4 + 3];
            // source over shadow (premultiplied)
            out.px[i * 4] = src.px[i * 4] + r * sa * (1 - top);
            out.px[i * 4 + 1] = src.px[i * 4 + 1] + g * sa * (1 - top);
            out.px[i * 4 + 2] = src.px[i * 4 + 2] + b * sa * (1 - top);
            out.px[i * 4 + 3] = top + sa * (1 - top);
        }
        return out;
    }

    /** Colour inversion of covered pixels (premultiplied: rgb = a - rgb). */
    public static void invert(Image im) {
        for (int i = 0; i < im.w * im.h; i++) {
            float a = im.px[i * 4 + 3];
            im.px[i * 4] = a - im.px[i * 4];
            im.px[i * 4 + 1] = a - im.px[i * 4 + 1];
            im.px[i * 4 + 2] = a - im.px[i * 4 + 2];
        }
    }

    /**
     * Hue rotation (degrees, around the luma axis), saturation (1 = unchanged,
     * 0 = grey) and brightness (added, -1..1), on straight colour.
     */
    public static void adjust(Image im, float hueDeg, float saturation, float brightness) {
        float[] m = hueSaturation(hueDeg, saturation);
        for (int i = 0; i < im.w * im.h; i++) {
            float a = im.px[i * 4 + 3];
            if (a <= 0) continue;
            float r = im.px[i * 4] / a, g = im.px[i * 4 + 1] / a, b = im.px[i * 4 + 2] / a;
            float nr = m[0] * r + m[1] * g + m[2] * b + brightness;
            float ng = m[3] * r + m[4] * g + m[5] * b + brightness;
            float nb = m[6] * r + m[7] * g + m[8] * b + brightness;
            im.px[i * 4] = clamp01(nr) * a;
            im.px[i * 4 + 1] = clamp01(ng) * a;
            im.px[i * 4 + 2] = clamp01(nb) * a;
        }
    }

    /** 3x3 colour matrix (row-major) for a hue rotation then a saturation scale; shared with the shader. */
    public static float[] hueSaturation(float hueDeg, float s) {
        double a = Math.toRadians(hueDeg), c = Math.cos(a), sn = Math.sin(a);
        // hue rotation about (1,1,1)/sqrt(3)
        float k = (float) ((1 - c) / 3), q = (float) (sn / Math.sqrt(3));
        float[] h = {
            (float) c + k, k - q, k + q,
            k + q, (float) c + k, k - q,
            k - q, k + q, (float) c + k};
        // saturation around Rec. 709 luma
        float lr = 0.2126f, lg = 0.7152f, lb = 0.0722f;
        float[] sm = {
            lr + (1 - lr) * s, lg - lg * s, lb - lb * s,
            lr - lr * s, lg + (1 - lg) * s, lb - lb * s,
            lr - lr * s, lg - lg * s, lb + (1 - lb) * s};
        float[] out = new float[9];
        for (int r = 0; r < 3; r++) for (int col = 0; col < 3; col++) {
            float v = 0;
            for (int j = 0; j < 3; j++) v += sm[r * 3 + j] * h[j * 3 + col];
            out[r * 3 + col] = v;
        }
        return out;
    }

    /** Blends covered pixels toward a straight colour by amount (0..1). */
    public static void tint(Image im, float r, float g, float b, float amount) {
        for (int i = 0; i < im.w * im.h; i++) {
            float a = im.px[i * 4 + 3];
            im.px[i * 4] += (r * a - im.px[i * 4]) * amount;
            im.px[i * 4 + 1] += (g * a - im.px[i * 4 + 1]) * amount;
            im.px[i * 4 + 2] += (b * a - im.px[i * 4 + 2]) * amount;
        }
    }

    /** out = a + (b - a) * t, per channel (the effect's mix). */
    public static Image lerp(Image a, Image b, float t) {
        if (t >= 1) return b;
        Image out = a.blank();
        for (int i = 0; i < a.px.length; i++) out.px[i] = a.px[i] + (b.px[i] - a.px[i]) * t;
        return out;
    }

    /** Composites src over dst in place with an opacity (premultiplied source-over). */
    public static void over(Image dst, Image src, float opacity) {
        for (int i = 0; i < dst.w * dst.h; i++) {
            float sa = src.px[i * 4 + 3] * opacity;
            float k = 1 - sa;
            dst.px[i * 4] = src.px[i * 4] * opacity + dst.px[i * 4] * k;
            dst.px[i * 4 + 1] = src.px[i * 4 + 1] * opacity + dst.px[i * 4 + 1] * k;
            dst.px[i * 4 + 2] = src.px[i * 4 + 2] * opacity + dst.px[i * 4 + 2] * k;
            dst.px[i * 4 + 3] = sa + dst.px[i * 4 + 3] * k;
        }
    }

    /** Premultiplied pixel at integer (x, y) (rows down), transparent outside. */
    static void at(Image im, int x, int y, float[] out) {
        if (x < 0 || y < 0 || x >= im.w || y >= im.h) {
            out[0] = out[1] = out[2] = out[3] = 0;
            return;
        }
        int o = (y * im.w + x) * 4;
        out[0] = im.px[o];
        out[1] = im.px[o + 1];
        out[2] = im.px[o + 2];
        out[3] = im.px[o + 3];
    }

    /**
     * Pixelate: blocks of size view units anchored to the view origin (so the
     * grid does not crawl when the layer moves); each block takes the colour
     * of the pixel at its centre.
     */
    public static Image pixelate(Image im, float size) {
        Image out = im.blank();
        if (!(size * im.scale >= 1.5f)) return im.copy();
        float[] c = new float[4];
        for (int j = 0; j < im.h; j++) {
            for (int i = 0; i < im.w; i++) {
                double vx = im.x0 + (i + 0.5) / im.scale, vy = im.y0 - (j + 0.5) / im.scale;
                double cx = (Math.floor(vx / size) + 0.5) * size, cy = (Math.floor(vy / size) + 0.5) * size;
                int si = (int) Math.floor((cx - im.x0) * im.scale), sj = (int) Math.floor((im.y0 - cy) * im.scale);
                at(im, si, sj, c);
                int o = (j * im.w + i) * 4;
                out.px[o] = c[0];
                out.px[o + 1] = c[1];
                out.px[o + 2] = c[2];
                out.px[o + 3] = c[3];
            }
        }
        return out;
    }

    /** Offsets (pixels, rows down) the outline samples: 16 directions at r and r / 2, rounded. */
    public static int[] ringOffsets(float r) {
        int[] o = new int[64];
        for (int k = 0; k < 32; k++) {
            double rad = k < 16 ? r : r / 2, a = 2 * Math.PI * (k % 16) / 16;
            o[k * 2] = (int) Math.floor(Math.cos(a) * rad + 0.5);
            o[k * 2 + 1] = (int) Math.floor(Math.sin(a) * rad + 0.5);
        }
        return o;
    }

    /** Outline: the shape grown by r pixels, in a straight colour, under the source. */
    public static Image outline(Image src, float r, float cr, float cg, float cb, float ca) {
        Image out = src.blank();
        int[] off = ringOffsets(r);
        for (int y = 0; y < src.h; y++) {
            for (int x = 0; x < src.w; x++) {
                int o = (y * src.w + x) * 4;
                float a = src.px[o + 3];
                for (int k = 0; k < 32; k++) {
                    int sx = x + off[k * 2], sy = y - off[k * 2 + 1];   // offsets are y-up
                    if (sx < 0 || sy < 0 || sx >= src.w || sy >= src.h) continue;
                    a = Math.max(a, src.px[(sy * src.w + sx) * 4 + 3]);
                }
                float sa = a * ca, top = src.px[o + 3];
                out.px[o] = src.px[o] + cr * sa * (1 - top);
                out.px[o + 1] = src.px[o + 1] + cg * sa * (1 - top);
                out.px[o + 2] = src.px[o + 2] + cb * sa * (1 - top);
                out.px[o + 3] = top + sa * (1 - top);
            }
        }
        return out;
    }

    /** Taps of the motion blur: 16 offsets (pixels, y up) spread over distance along the angle, centred. */
    public static int[] smearOffsets(float angleDeg, float distance) {
        int[] o = new int[32];
        double a = Math.toRadians(angleDeg);
        for (int k = 0; k < 16; k++) {
            double d = distance * (k / 15.0 - 0.5);
            o[k * 2] = (int) Math.floor(Math.cos(a) * d + 0.5);
            o[k * 2 + 1] = (int) Math.floor(Math.sin(a) * d + 0.5);
        }
        return o;
    }

    /** Motion blur: the average of 16 copies spread along a direction. */
    public static Image smear(Image src, float angleDeg, float distance) {
        Image out = src.blank();
        int[] off = smearOffsets(angleDeg, distance);
        float[] c = new float[4];
        for (int y = 0; y < src.h; y++) {
            for (int x = 0; x < src.w; x++) {
                float r = 0, g = 0, b = 0, a = 0;
                for (int k = 0; k < 16; k++) {
                    at(src, x + off[k * 2], y - off[k * 2 + 1], c);
                    r += c[0];
                    g += c[1];
                    b += c[2];
                    a += c[3];
                }
                int o = (y * src.w + x) * 4;
                out.px[o] = r / 16;
                out.px[o + 1] = g / 16;
                out.px[o + 2] = b / 16;
                out.px[o + 3] = a / 16;
            }
        }
        return out;
    }

    static float clamp01(float v) {
        return v < 0 ? 0 : (v > 1 ? 1 : v);
    }
}
