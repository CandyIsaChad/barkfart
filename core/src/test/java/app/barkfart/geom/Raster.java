package app.barkfart.geom;

import java.awt.AlphaComposite;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.geom.Path2D;
import java.awt.image.BufferedImage;
import java.util.Collections;
import java.util.Map;

/**
 * Test-only software fill of a DrawList (no anti-aliasing, deterministic).
 * Flat-coloured untextured triangles use Java2D exactly as before, so older
 * goldens stay byte-stable; triangles with differing vertex colours or a
 * texture use a barycentric per-pixel fill (pixel centres, same coverage rule
 * approximately).
 */
public final class Raster {
    private Raster() {}

    public static BufferedImage fill(DrawList dl, int w, int h, int background) {
        return fill(dl, w, h, background, Collections.<String, BufferedImage>emptyMap());
    }

    public static BufferedImage fill(DrawList dl, int w, int h, int background, Map<String, BufferedImage> textures) {
        BufferedImage img = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
        Graphics2D g = img.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
        g.setComposite(AlphaComposite.Src);
        g.setColor(new java.awt.Color(background, true));
        g.fillRect(0, 0, w, h);
        g.setComposite(AlphaComposite.SrcOver);
        for (DrawList.Batch b : dl.batches) {
            BufferedImage tex = b.texture == null ? null : textures.get(b.texture);
            int n = b.vertexCount();
            for (int i = 0; i + 2 < n; i += 3) {
                boolean flat = tex == null && b.rgba(i) == b.rgba(i + 1) && b.rgba(i) == b.rgba(i + 2);
                if (flat) {
                    Path2D.Float p = new Path2D.Float();
                    p.moveTo(b.x(i) + w / 2f, h / 2f - b.y(i));
                    p.lineTo(b.x(i + 1) + w / 2f, h / 2f - b.y(i + 1));
                    p.lineTo(b.x(i + 2) + w / 2f, h / 2f - b.y(i + 2));
                    p.closePath();
                    int c = b.rgba(i);
                    g.setColor(new java.awt.Color(c >>> 24, (c >>> 16) & 0xff, (c >>> 8) & 0xff, c & 0xff));
                    g.fill(p);
                } else {
                    g.dispose();
                    shade(img, b, i, tex);
                    g = img.createGraphics();
                    g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
                    g.setComposite(AlphaComposite.SrcOver);
                }
            }
        }
        g.dispose();
        return img;
    }

    /** Barycentric fill with interpolated colour and (nearest) texture sampling, source-over. */
    private static void shade(BufferedImage img, DrawList.Batch b, int i, BufferedImage tex) {
        int w = img.getWidth(), h = img.getHeight();
        double[] x = new double[3], y = new double[3];
        for (int k = 0; k < 3; k++) {
            x[k] = b.x(i + k) + w / 2.0;
            y[k] = h / 2.0 - b.y(i + k);
        }
        double area = (x[1] - x[0]) * (y[2] - y[0]) - (x[2] - x[0]) * (y[1] - y[0]);
        if (Math.abs(area) < 1e-12) return;
        int x0 = Math.max(0, (int) Math.floor(Math.min(x[0], Math.min(x[1], x[2])))), x1 = Math.min(w - 1, (int) Math.ceil(Math.max(x[0], Math.max(x[1], x[2]))));
        int y0 = Math.max(0, (int) Math.floor(Math.min(y[0], Math.min(y[1], y[2])))), y1 = Math.min(h - 1, (int) Math.ceil(Math.max(y[0], Math.max(y[1], y[2]))));
        for (int py = y0; py <= y1; py++) {
            for (int px = x0; px <= x1; px++) {
                double cx = px + 0.5, cy = py + 0.5;
                double w0 = ((x[1] - cx) * (y[2] - cy) - (x[2] - cx) * (y[1] - cy)) / area;
                double w1 = ((x[2] - cx) * (y[0] - cy) - (x[0] - cx) * (y[2] - cy)) / area;
                double w2 = 1 - w0 - w1;
                if (w0 < 0 || w1 < 0 || w2 < 0) continue;
                double[] c = new double[4];
                for (int k = 0; k < 3; k++) {
                    int v = b.rgba(i + k);
                    double wk = k == 0 ? w0 : (k == 1 ? w1 : w2);
                    c[0] += ((v >>> 24) & 0xff) * wk;
                    c[1] += ((v >>> 16) & 0xff) * wk;
                    c[2] += ((v >>> 8) & 0xff) * wk;
                    c[3] += (v & 0xff) * wk;
                }
                if (tex != null) {
                    double u = w0 * b.u(i) + w1 * b.u(i + 1) + w2 * b.u(i + 2);
                    double vv = w0 * b.v(i) + w1 * b.v(i + 1) + w2 * b.v(i + 2);
                    int tx = Math.min(tex.getWidth() - 1, Math.max(0, (int) (u * tex.getWidth())));
                    int ty = Math.min(tex.getHeight() - 1, Math.max(0, (int) (vv * tex.getHeight())));
                    int t = tex.getRGB(tx, ty);
                    c[0] *= ((t >>> 16) & 0xff) / 255.0;
                    c[1] *= ((t >>> 8) & 0xff) / 255.0;
                    c[2] *= (t & 0xff) / 255.0;
                    c[3] *= ((t >>> 24) & 0xff) / 255.0;
                }
                double a = c[3] / 255.0;
                int dst = img.getRGB(px, py);
                double da = ((dst >>> 24) & 0xff) / 255.0;
                double oa = a + da * (1 - a);
                int r = (int) Math.round(oa == 0 ? 0 : (c[0] * a + ((dst >>> 16) & 0xff) * da * (1 - a)) / oa);
                int gg = (int) Math.round(oa == 0 ? 0 : (c[1] * a + ((dst >>> 8) & 0xff) * da * (1 - a)) / oa);
                int bb = (int) Math.round(oa == 0 ? 0 : (c[2] * a + (dst & 0xff) * da * (1 - a)) / oa);
                img.setRGB(px, py, ((int) Math.round(oa * 255) << 24) | (clamp(r) << 16) | (clamp(gg) << 8) | clamp(bb));
            }
        }
    }

    private static int clamp(int v) {
        return v < 0 ? 0 : (v > 255 ? 255 : v);
    }
}
