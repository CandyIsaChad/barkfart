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

    static float clamp01(float v) {
        return v < 0 ? 0 : (v > 1 ? 1 : v);
    }
}
