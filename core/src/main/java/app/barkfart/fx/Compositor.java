package app.barkfart.fx;

import app.barkfart.eval.ResolvedEffect;
import app.barkfart.geom.DrawList;
import app.barkfart.model.Layer;

/**
 * Software reference renderer for a layered DrawList: isolates draw into
 * their own premultiplied buffers, effects run in order, and results
 * composite with opacity and blend. Textures are not sampled here (textured
 * quads draw in their vertex colour). The GL renderer must match this.
 */
public final class Compositor {
    private Compositor() {}

    /** Largest buffer this renderer allocates, in pixels (memory guard). */
    public static final int MAX_PIXELS = 4096 * 4096;

    /** Renders dl (built by Tessellator.layered) over target. */
    public static void render(DrawList dl, Image target) {
        if (dl.root == null || (long) target.w * target.h > MAX_PIXELS) return;
        Image layer = target.blank();
        draw(dl.root, layer);
        Kernels.over(target, layer, 1);
    }

    private static void draw(DrawList.Isolate iso, Image canvas) {
        for (Object o : iso.items) {
            if (o instanceof DrawList.Batch) {
                fill((DrawList.Batch) o, canvas);
                continue;
            }
            DrawList.Isolate child = (DrawList.Isolate) o;
            if (child.adjustment) {
                Image img = canvas.copy();
                for (ResolvedEffect e : child.effects) img = EffectTypes.apply(e, img);
                Image mixed = Kernels.lerp(canvas, img, child.opacity);
                System.arraycopy(mixed.px, 0, canvas.px, 0, canvas.px.length);
                continue;
            }
            Image buf = canvas.blank();
            draw(child, buf);
            for (ResolvedEffect e : child.effects) buf = EffectTypes.apply(e, buf);
            composite(canvas, buf, child.opacity, child.blend);
        }
    }

    /** Premultiplied composite with the four blend modes. */
    static void composite(Image dst, Image src, float op, Layer.Blend blend) {
        if (blend == Layer.Blend.NORMAL || blend == null) {
            Kernels.over(dst, src, op);
            return;
        }
        for (int i = 0; i < dst.w * dst.h; i++) {
            float sa = src.px[i * 4 + 3] * op;
            for (int c = 0; c < 3; c++) {
                float s = src.px[i * 4 + c] * op, d = dst.px[i * 4 + c];
                float v;
                switch (blend) {
                    case ADD: v = d + s; break;
                    case MULTIPLY: v = s * d + d * (1 - sa); break;   // GL: DST_COLOR, ONE_MINUS_SRC_ALPHA
                    default: v = s + d - s * d; break;   // SCREEN
                }
                dst.px[i * 4 + c] = Math.min(1f, v);
            }
            float da = dst.px[i * 4 + 3];
            dst.px[i * 4 + 3] = blend == Layer.Blend.ADD ? Math.min(1f, da + sa) : sa + da * (1 - sa);
        }
    }

    /** Triangles sampled at pixel centres with interpolated vertex colour, blended per the batch's mode. */
    static void fill(DrawList.Batch b, Image im) {
        int n = b.vertexCount() / 3 * 3;
        float s = im.scale;
        for (int t = 0; t < n; t += 3) {
            float ax = (b.x(t) - im.x0) * s, ay = (im.y0 - b.y(t)) * s;
            float bx = (b.x(t + 1) - im.x0) * s, by = (im.y0 - b.y(t + 1)) * s;
            float cx = (b.x(t + 2) - im.x0) * s, cy = (im.y0 - b.y(t + 2)) * s;
            float area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
            if (!(Math.abs(area) > 1e-9f)) continue;   // also NaN
            int x0 = Math.max(0, (int) Math.floor(Math.min(ax, Math.min(bx, cx))));
            int x1 = Math.min(im.w - 1, (int) Math.ceil(Math.max(ax, Math.max(bx, cx))));
            int y0 = Math.max(0, (int) Math.floor(Math.min(ay, Math.min(by, cy))));
            int y1 = Math.min(im.h - 1, (int) Math.ceil(Math.max(ay, Math.max(by, cy))));
            int ca = b.rgba(t), cb = b.rgba(t + 1), cc = b.rgba(t + 2);
            for (int y = y0; y <= y1; y++) {
                for (int x = x0; x <= x1; x++) {
                    // one sample per pixel centre, like GL without multisampling; the tiny
                    // offset keeps samples off shared edges so each pixel is drawn once
                    float px = x + 0.5f + 1.3e-4f, py = y + 0.5f + 0.7e-4f;
                    float w0 = ((bx - px) * (cy - py) - (by - py) * (cx - px)) / area;
                    float w1 = ((cx - px) * (ay - py) - (cy - py) * (ax - px)) / area;
                    float w2 = 1 - w0 - w1;
                    if (!(w0 >= 0 && w1 >= 0 && w2 >= 0)) continue;
                    int cov = 4;
                    float r = (ch(ca, 24) * w0 + ch(cb, 24) * w1 + ch(cc, 24) * w2);
                    float g = (ch(ca, 16) * w0 + ch(cb, 16) * w1 + ch(cc, 16) * w2);
                    float bl = (ch(ca, 8) * w0 + ch(cb, 8) * w1 + ch(cc, 8) * w2);
                    float a = (ch(ca, 0) * w0 + ch(cb, 0) * w1 + ch(cc, 0) * w2) * cov / 4f;
                    blendPixel(im, (y * im.w + x) * 4, r, g, bl, a, b.blend);
                }
            }
        }
    }

    private static float ch(int rgba, int shift) {
        return ((rgba >>> shift) & 0xff) / 255f;
    }

    private static void blendPixel(Image im, int o, float r, float g, float b, float a, Layer.Blend mode) {
        float[] p = im.px;
        float sr = r * a, sg = g * a, sb = b * a;
        switch (mode == null ? Layer.Blend.NORMAL : mode) {
            case ADD:
                p[o] = Math.min(1f, p[o] + sr);
                p[o + 1] = Math.min(1f, p[o + 1] + sg);
                p[o + 2] = Math.min(1f, p[o + 2] + sb);
                p[o + 3] = Math.min(1f, p[o + 3] + a);
                return;
            case MULTIPLY: {   // GL: DST_COLOR, ONE_MINUS_SRC_ALPHA (premultiplied source)
                float da = p[o + 3];
                p[o] = sr * p[o] + p[o] * (1 - a);
                p[o + 1] = sg * p[o + 1] + p[o + 1] * (1 - a);
                p[o + 2] = sb * p[o + 2] + p[o + 2] * (1 - a);
                p[o + 3] = a + da * (1 - a);
                return;
            }
            case SCREEN:
                p[o] = sr + p[o] - sr * p[o];
                p[o + 1] = sg + p[o + 1] - sg * p[o + 1];
                p[o + 2] = sb + p[o + 2] - sb * p[o + 2];
                p[o + 3] = a + p[o + 3] * (1 - a);
                return;
            default:
                p[o] = sr + p[o] * (1 - a);
                p[o + 1] = sg + p[o + 1] * (1 - a);
                p[o + 2] = sb + p[o + 2] * (1 - a);
                p[o + 3] = a + p[o + 3] * (1 - a);
        }
    }

    /** Straight RGBA8888 of a pixel (for tests and export). */
    public static int rgba(Image im, int x, int y) {
        int o = (y * im.w + x) * 4;
        float a = im.px[o + 3];
        int ai = Math.round(Kernels.clamp01(a) * 255);
        if (a <= 0) return 0;
        int r = Math.round(Kernels.clamp01(im.px[o] / a) * 255), g = Math.round(Kernels.clamp01(im.px[o + 1] / a) * 255), b = Math.round(Kernels.clamp01(im.px[o + 2] / a) * 255);
        return (r << 24) | (g << 16) | (b << 8) | ai;
    }
}
