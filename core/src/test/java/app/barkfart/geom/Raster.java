package app.barkfart.geom;

import java.awt.AlphaComposite;
import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.geom.Path2D;
import java.awt.image.BufferedImage;

/** Test-only software fill of a DrawList (Java2D, no anti-aliasing, deterministic). */
public final class Raster {
    private Raster() {}

    public static BufferedImage fill(DrawList dl, int w, int h, int background) {
        BufferedImage img = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
        Graphics2D g = img.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_OFF);
        g.setComposite(AlphaComposite.Src);
        g.setColor(new java.awt.Color(background, true));
        g.fillRect(0, 0, w, h);
        g.setComposite(AlphaComposite.SrcOver);
        for (DrawList.Batch b : dl.batches) {
            int n = b.vertexCount();
            for (int i = 0; i + 2 < n; i += 3) {
                Path2D.Float p = new Path2D.Float();
                p.moveTo(b.x(i) + w / 2f, h / 2f - b.y(i));
                p.lineTo(b.x(i + 1) + w / 2f, h / 2f - b.y(i + 1));
                p.lineTo(b.x(i + 2) + w / 2f, h / 2f - b.y(i + 2));
                p.closePath();
                int c = b.rgba(i);
                g.setColor(new java.awt.Color(c >>> 24, (c >>> 16) & 0xff, (c >>> 8) & 0xff, c & 0xff));
                g.fill(p);
            }
        }
        g.dispose();
        return img;
    }
}
