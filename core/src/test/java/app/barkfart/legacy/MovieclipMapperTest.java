package app.barkfart.legacy;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.geom.Raster;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.awt.Graphics2D;
import java.awt.image.BufferedImage;
import java.io.File;
import java.nio.file.Files;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

class MovieclipMapperTest {
    /**
     * Local-only: imports each supported clip, checks it evaluates cleanly and
     * survives .bark save/load, and writes side-by-side sheets (original
     * preview frames vs our render of each frame at 1/3 scale) under build/
     * for visual comparison. Never committed.
     */
    @Test
    void referenceClipsImportAndRender() throws Exception {
        java.util.List<File> files = MovieclipFormatTest.referenceClips();
        assumeTrue(!files.isEmpty(), "reference bundle not present");
        File outDir = new File(System.getProperty("barkfart.root", ".."), "core/build/legacy-renders");
        outDir.mkdirs();
        for (File f : files) {
            MovieclipFormat.LegacyMovieclip mc;
            try {
                mc = MovieclipFormat.read(Files.readAllBytes(f.toPath()));
            } catch (MovieclipFormat.UnsupportedLayoutException e) {
                continue;
            }
            Project p = Fixtures.empty();
            Symbol s = MovieclipMapper.toSymbol(p, mc, new FigureMapper.Report());
            p.rootSymbol = s.id;
            Project q = ProjectCodec.decode(ProjectCodec.encode(p));
            int w = mc.previewWidth, h = mc.previewHeight, n = mc.frames.size();
            BufferedImage sheet = new BufferedImage(w * Math.max(n, mc.previewCount), h * 2, BufferedImage.TYPE_INT_ARGB);
            Graphics2D g = sheet.createGraphics();
            FigureMapper.Report rep = new FigureMapper.Report();
            for (MovieclipFormat.NamedFigure nf : mc.library) FigureMapper.toLayer(Fixtures.empty(), nf.figure, nf.name, rep);
            boolean hasGradients = false;
            for (MovieclipFormat.NamedFigure nf : mc.library) {
                for (NodesFormat.LegacyNode ln : nf.figure.all()) hasGradients |= ln.i("gradient") != 0;
                hasGradients |= !nf.figure.polyfills.isEmpty();
            }
            BufferedImage[] previews = new BufferedImage[mc.previewCount];
            for (int i = 0; i < mc.previewCount; i++) {
                BufferedImage pv = new BufferedImage(w, h, BufferedImage.TYPE_INT_ARGB);
                previews[i] = pv;
                for (int y = 0; y < h; y++) {
                    for (int x = 0; x < w; x++) {
                        int o = ((h - 1 - y) * w + x) * 4 + i * w * h * 4;   // stored bottom-up
                        byte[] px = mc.preview;
                        int argb = ((px[o + 3] & 0xff) << 24) | ((px[o] & 0xff) << 16) | ((px[o + 1] & 0xff) << 8) | (px[o + 2] & 0xff);
                        pv.setRGB(x, y, argb | 0xff000000);
                    }
                }
                // The original renders onto an opaque stage background. A pixel counts as
                // drawn when it differs from the stage colour by more than half of the
                // clip's maximum contrast (handles pale content and anti-aliased edges).
                int bg = pv.getRGB(0, 0);
                int maxDiff = 0;
                for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) maxDiff = Math.max(maxDiff, diff(pv.getRGB(x, y), bg));
                int threshold = Math.max(4, maxDiff / 2);
                for (int y = 0; y < h; y++) {
                    for (int x = 0; x < w; x++) if (diff(pv.getRGB(x, y), bg) <= threshold) pv.setRGB(x, y, 0xffdddddd);
                }
                g.drawImage(pv, i * w, 0, null);
            }
            // Diagnostic: which of our frames best matches each preview.
            BufferedImage[] oursAll = new BufferedImage[n];
            for (int i = 0; i < n; i++) {
                oursAll[i] = Raster.fill(Tessellator.tessellate(scaled(Evaluator.evaluate(q, q.rootSymbol, i / (double) mc.fps))), w, h, 0xffdddddd);
            }
            StringBuilder best = new StringBuilder();
            for (int pvi = 0; pvi < previews.length; pvi++) {
                int bi = -1;
                double bv = -1;
                for (int i = 0; i < n; i++) {
                    double v = iou(previews[pvi], oursAll[i]);
                    if (v > bv) {
                        bv = v;
                        bi = i;
                    }
                }
                best.append(String.format(" p%d->f%d(%.2f)", pvi, bi, bv));
            }
            System.out.println("[mc-match] " + f.getName() + " frames " + n + ":" + best);
            StringBuilder ious = new StringBuilder();
            for (int i = 0; i < n; i++) {
                Scene sc = Evaluator.evaluate(q, q.rootSymbol, i / (double) mc.fps);
                assertTrue(sc.diagnostics.isEmpty(), f.getName() + " frame " + i + ": " + sc.diagnostics);
                g.drawImage(oursAll[i], i * w, h, null);
            }
            // Preview k shows frame k * floor(frames / previews) (found by best-match search).
            int step = Math.max(1, n / Math.max(1, previews.length));
            for (int k = 0; k < previews.length; k++) {
                int fi = Math.min(n - 1, k * step);
                double iou = iou(previews[k], oursAll[fi]);
                double f1 = tolerantF1(previews[k], oursAll[fi]);
                long area = union(previews[k], oursAll[fi]);
                ious.append(String.format(" f%d: IoU %.3f F1 %.3f (%dpx);", fi, iou, f1, area));
                // Golden vs the ORIGINAL renderer (local only): clips that use only
                // shapes we draw natively must match the original's silhouette.
                if (rep.approximations.isEmpty() && !hasGradients && area >= MIN_AREA) {
                    assertTrue(f1 >= GOLDEN_F1, f.getName() + " frame " + fi + " silhouette F1 " + f1);
                }
            }
            System.out.println("[mc-golden] " + f.getName() + ":" + ious
                    + (rep.approximations.isEmpty() && !hasGradients ? "  (asserted F1 >= " + GOLDEN_F1 + " where >= " + MIN_AREA + "px)"
                    : "  (not asserted: approximations " + rep.approximations + (hasGradients ? ", gradients/polyfills" : "") + ")"));
            g.dispose();
            ImageIO.write(sheet, "png", new File(outDir, "mc_" + f.getName().replaceAll("[^A-Za-z0-9]+", "_") + ".png"));
        }
    }

    /** Documented tolerance for silhouette agreement with the original's previews. */
    static final double GOLDEN_IOU = 0.85;
    /**
     * Asserted metric: F1 of precision/recall where a drawn pixel counts as
     * matched if the other image has a drawn pixel within 1 px. Tolerates the
     * original's anti-aliasing on thin shapes. Documented tolerance: >= 0.9.
     */
    static final double GOLDEN_F1 = 0.9;

    static boolean drawn(BufferedImage im, int x, int y) {
        return x >= 0 && y >= 0 && x < im.getWidth() && y < im.getHeight() && (im.getRGB(x, y) & 0xffffff) != 0xdddddd;
    }

    static boolean near1(BufferedImage im, int x, int y) {
        for (int dy = -1; dy <= 1; dy++) for (int dx = -1; dx <= 1; dx++) if (drawn(im, x + dx, y + dy)) return true;
        return false;
    }

    static double tolerantF1(BufferedImage orig, BufferedImage ours) {
        long o = 0, om = 0, u = 0, um = 0;
        for (int y = 0; y < orig.getHeight(); y++) {
            for (int x = 0; x < orig.getWidth(); x++) {
                if (drawn(orig, x, y)) {
                    o++;
                    if (near1(ours, x, y)) om++;
                }
                if (drawn(ours, x, y)) {
                    u++;
                    if (near1(orig, x, y)) um++;
                }
            }
        }
        if (o == 0 && u == 0) return 1;
        double recall = o == 0 ? 0 : om / (double) o, precision = u == 0 ? 0 : um / (double) u;
        return precision + recall == 0 ? 0 : 2 * precision * recall / (precision + recall);
    }
    /** Below this many drawn pixels IoU is dominated by edge pixels; such frames are reported only. */
    static final long MIN_AREA = 500;

    static long union(BufferedImage a, BufferedImage b) {
        long u = 0;
        for (int y = 0; y < a.getHeight(); y++) {
            for (int x = 0; x < a.getWidth(); x++) {
                if ((a.getRGB(x, y) & 0xffffff) != 0xdddddd || (b.getRGB(x, y) & 0xffffff) != 0xdddddd) u++;
            }
        }
        return u;
    }

    private static int diff(int a, int b) {
        int m = 0;
        for (int sh = 0; sh < 24; sh += 8) m = Math.max(m, Math.abs(((a >> sh) & 0xff) - ((b >> sh) & 0xff)));
        return m;
    }

    /** Intersection over union of drawn (non-background) pixels. */
    static double iou(BufferedImage a, BufferedImage b) {
        long inter = 0, uni = 0;
        for (int y = 0; y < a.getHeight(); y++) {
            for (int x = 0; x < a.getWidth(); x++) {
                boolean pa = (a.getRGB(x, y) & 0xffffff) != 0xdddddd, pb = (b.getRGB(x, y) & 0xffffff) != 0xdddddd;
                if (pa && pb) inter++;
                if (pa || pb) uni++;
            }
        }
        if (Boolean.getBoolean("barkfart.debugIou")) {
            long ca = 0, cb = 0;
            for (int y = 0; y < a.getHeight(); y++) for (int x = 0; x < a.getWidth(); x++) {
                if ((a.getRGB(x, y) & 0xffffff) != 0xdddddd) ca++;
                if ((b.getRGB(x, y) & 0xffffff) != 0xdddddd) cb++;
            }
            System.out.println("[iou-debug] preview " + ca + " ours " + cb + " inter " + inter + " sample " + Integer.toHexString(a.getRGB(5, 5)) + " " + Integer.toHexString(b.getRGB(5, 5)));
        }
        return uni == 0 ? 1 : inter / (double) uni;
    }

    /** Scales a view-space scene by 1/3 by wrapping it (test helper). */
    private static Scene scaled(Scene s) {
        app.barkfart.eval.Group g = new app.barkfart.eval.Group("scaled");
        scaleInto(s.root, g, 1f / 3);
        Scene out = new Scene(g, s.time);
        return out;
    }

    private static void scaleInto(app.barkfart.eval.Group src, app.barkfart.eval.Group dst, float k) {
        dst.opacity = src.opacity;
        for (Object o : src.children) {
            if (o instanceof app.barkfart.eval.Group) {
                app.barkfart.eval.Group c = new app.barkfart.eval.Group("c");
                scaleInto((app.barkfart.eval.Group) o, c, k);
                dst.children.add(c);
            } else if (o instanceof app.barkfart.eval.Shape.Line) {
                app.barkfart.eval.Shape.Line l = (app.barkfart.eval.Shape.Line) o;
                dst.children.add(new app.barkfart.eval.Shape.Line(l.x1 * k, l.y1 * k, l.x2 * k, l.y2 * k, l.width * k, l.color, l.source));
            } else if (o instanceof app.barkfart.eval.Shape.Polygon) {
                app.barkfart.eval.Shape.Polygon pg = (app.barkfart.eval.Shape.Polygon) o;
                float[] q = pg.points.clone();
                for (int i = 0; i < q.length; i++) q[i] *= k;
                dst.children.add(new app.barkfart.eval.Shape.Polygon(q, pg.color, pg.source));
            } else if (o instanceof app.barkfart.eval.Shape.Circle) {
                app.barkfart.eval.Shape.Circle c = (app.barkfart.eval.Shape.Circle) o;
                dst.children.add(new app.barkfart.eval.Shape.Circle(c.cx * k, c.cy * k, c.r * k, c.color, c.source));
            }
        }
    }
}
