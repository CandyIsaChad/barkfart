package app.barkfart.fx;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.ResolvedEffect;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Color;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.ValueType;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

/** The effects pipeline, checked on the software reference renderer. */
class EffectsTest {
    static final int W = 320, H = 240;

    /** A figure layer with one square (40 x 40) centred at (cx, cy) in view space. */
    static Layer.FigureLayer square(Project p, String name, float cx, float cy, int rgba) {
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), name);
        Node r = new Node(l.id, "r", null, Node.Shape.NONE);
        l.figure.add(r);
        Node s = Fixtures.add(l, "s", "r", Node.Shape.SQUARE_LINE, 0, 40);
        s.thickness.setValue(40f);
        s.color.setValue(Color.rgba8888(rgba));
        l.transform.x.setValue(cx - 20);
        l.transform.y.setValue(cy);
        p.root().layers.add(l);
        return l;
    }

    static Image render(Project p) {
        p.invalidateIndex();
        Image im = new Image(W, H, -W / 2f, H / 2f, 1);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0)), im);
        return im;
    }

    /** Pixel at view (x, y). */
    static int at(Image im, float x, float y) {
        return Compositor.rgba(im, (int) (x - im.x0), (int) (im.y0 - y));
    }

    static int alpha(int rgba) {
        return rgba & 0xff;
    }

    static int red(int rgba) {
        return rgba >>> 24;
    }

    static EffectNode fx(Layer l, String id, String type) {
        EffectNode e = new EffectNode(l.id, id, type);
        l.effects.append(e);
        return e;
    }

    @Test
    void aTranslucentGroupCompositesOnceWithoutOverlapSeams() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0x0000ffff);
        Node second = Fixtures.add(l, "s2", "r", Node.Shape.SQUARE_LINE, 0, 40);   // same place: full overlap
        second.thickness.setValue(40f);
        second.color.setValue(Color.rgba8888(0x0000ffff));
        Node third = Fixtures.add(l, "s3", "r", Node.Shape.SQUARE_LINE, 0, 80);    // reaches 40 further right
        third.thickness.setValue(40f);
        third.color.setValue(Color.rgba8888(0x0000ffff));
        l.opacity.setValue(0.5f);
        Image im = render(p);
        int overlap = alpha(at(im, 0, 0)), single = alpha(at(im, 50, 0));
        assertEquals(128, overlap, 2);
        assertEquals(overlap, single, 1, "one composite: overlaps are not darker");
        // the flat path (vertex alpha) shows the seam; the layered path is the fix
        DrawList flat = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0));
        assertNull(flat.root);
    }

    @Test
    void blurSpreadsWithinItsReachAndKeepsCoverage() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0x000000ff);
        fx(l, "b", EffectTypes.BLUR).param(l.id, "radius", ValueType.FLOAT, 10f);
        Image im = render(p);
        assertTrue(alpha(at(im, 24, 0)) > 20, "spreads past the edge");
        assertEquals(0, alpha(at(im, 20 + 3 * 10 / 2 * 1.2f + 4, 0)), "nothing beyond ~3 sigma");
        assertTrue(alpha(at(im, 0, 0)) > 240, "centre stays covered");
        float sum = 0;
        for (int i = 3; i < im.px.length; i += 4) sum += im.px[i];
        assertEquals(40 * 40, sum, 40 * 40 * 0.03, "blur keeps total coverage");
    }

    @Test
    void dropShadowFallsDownRightAndSitsUnderTheShape() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0xffffffff);
        EffectNode s = fx(l, "shadow", EffectTypes.SHADOW);   // not "s": node s owns l/s/* ids
        s.param(l.id, "dx", ValueType.FLOAT, 12f);
        s.param(l.id, "dy", ValueType.FLOAT, -12f);
        s.param(l.id, "radius", ValueType.FLOAT, 0f);
        s.param(l.id, "color", ValueType.COLOR, Color.rgba8888(0x000000ff));
        Image im = render(p);
        assertEquals(0x000000ff, at(im, 26, -26), "shadow below-right");
        assertEquals(0, alpha(at(im, -26, 26)), "nothing above-left");
        assertEquals(0xffffffff, at(im, 5, -5), "the shape stays on top");
    }

    @Test
    void glowSurroundsTheShapeInItsColour() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0x000000ff);
        EffectNode g = fx(l, "g", EffectTypes.GLOW);
        g.param(l.id, "radius", ValueType.FLOAT, 12f);
        g.param(l.id, "color", ValueType.COLOR, Color.rgba8888(0xffee00ff));
        Image im = render(p);
        for (float[] d : new float[][] {{25, 0}, {-25, 0}, {0, 25}, {0, -25}}) {
            int c = at(im, d[0], d[1]);
            assertTrue(alpha(c) > 40, "glow on every side");
            assertTrue(red(c) > 240 && ((c >>> 16) & 0xff) > 220 && ((c >>> 8) & 0xff) < 30, Integer.toHexString(c));
        }
        assertEquals(0x000000ff, at(im, 0, 0));
    }

    @Test
    void invertAndMix() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0xffffffff);
        EffectNode inv = fx(l, "i", EffectTypes.INVERT);
        assertEquals(0x000000ff, at(render(p), 0, 0));
        inv.mix.setValue(0.5f);
        int half = at(render(p), 0, 0);
        assertEquals(128, red(half), 2);
    }

    @Test
    void adjustmentLayersChangeOnlyWhatIsBelowThem() {
        Project p = Fixtures.empty();
        square(p, "below", -40, 0, 0xffffffff);
        Layer.AdjustmentLayer adj = new Layer.AdjustmentLayer(p.newId("L"), "adj");
        adj.effects.append(new EffectNode(adj.id, "i", EffectTypes.INVERT));
        p.root().layers.add(adj);
        square(p, "above", 40, 0, 0xffffffff);
        Image im = render(p);
        assertEquals(0x000000ff, at(im, -40, 0), "below: inverted");
        assertEquals(0xffffffff, at(im, 40, 0), "above: untouched");
        assertEquals(0, alpha(at(im, 0, 80)), "empty stays empty");
    }

    @Test
    void colourAdjustAndTint() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0xff0000ff);
        EffectNode a = fx(l, "a", EffectTypes.ADJUST);
        a.param(l.id, "saturation", ValueType.FLOAT, 0f);
        int grey = at(render(p), 0, 0);
        assertEquals(red(grey), (grey >>> 16) & 0xff, 1);
        assertEquals(54, red(grey), 2, "Rec. 709 luma of pure red");
        a.params.clear();
        a.param(l.id, "hue", ValueType.FLOAT, 120f);
        int hue = at(render(p), 0, 0);
        assertTrue(((hue >>> 16) & 0xff) > 200 && red(hue) < 40, "red turns green: " + Integer.toHexString(hue));
        l.effects.nodes.clear();
        l.effects.output = null;
        EffectNode t = fx(l, "t", EffectTypes.TINT);
        t.param(l.id, "color", ValueType.COLOR, Color.rgba8888(0x0000ffff));
        t.param(l.id, "amount", ValueType.FLOAT, 1f);
        assertEquals(0x0000ffff, at(render(p), 0, 0));
    }

    @Test
    void pixelateGridStaysOnTheStageOutlineGrowsMotionSmears() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = square(p, "a", 0, 0, 0xffffffff);
        EffectNode px = fx(l, "px", EffectTypes.PIXELATE);
        px.param(l.id, "size", ValueType.FLOAT, 8f);
        Image a = render(p);
        // every 8x8 stage block is one colour
        for (int by = -24; by < 24; by += 8) {
            for (int bx = -24; bx < 24; bx += 8) {
                int c = at(a, bx + 0.5f, by + 0.5f);
                assertEquals(c, at(a, bx + 7.5f, by + 7.5f), "block " + bx + "," + by);
            }
        }
        // moving the layer by a fraction of a block does not move the grid
        l.transform.x.setValue(l.transform.x.floatValue() + 3);
        Image b = render(p);
        for (int bx = -24; bx < 24; bx += 8) assertEquals(at(b, bx + 0.5f, 0.5f), at(b, bx + 7.5f, 0.5f));
        l.transform.x.setValue(l.transform.x.floatValue() - 3);
        l.effects.nodes.clear();
        l.effects.output = null;
        EffectNode o = fx(l, "ol", EffectTypes.OUTLINE);
        o.param(l.id, "width", ValueType.FLOAT, 5f);
        o.param(l.id, "color", ValueType.COLOR, Color.rgba8888(0xff0000ff));
        Image c = render(p);
        assertEquals(0xff0000ff, at(c, 23, 0), "outline just outside the edge");
        assertEquals(0, alpha(at(c, 27, 0)), "and no further than its width");
        assertEquals(0xffffffff, at(c, 0, 0), "the shape stays on top");
        l.effects.nodes.clear();
        l.effects.output = null;
        EffectNode m = fx(l, "mb", EffectTypes.MOTION);
        m.param(l.id, "angle", ValueType.FLOAT, 0f);
        m.param(l.id, "distance", ValueType.FLOAT, 20f);
        Image d = render(p);
        assertTrue(alpha(at(d, 27, 0)) > 20, "smears sideways");
        assertEquals(0, alpha(at(d, 0, 27)), "not up");
    }

    @Test
    void badParametersFallBackAndUnknownTypesPassThrough() {
        ResolvedEffect e = new ResolvedEffect(EffectTypes.BLUR, Float.NaN);
        e.params.put("radius", Float.NaN);
        assertEquals(6, EffectTypes.f(e, "radius", 6, 0, EffectTypes.MAX_LENGTH));
        e.params.put("radius", 1e30f);
        assertEquals(EffectTypes.MAX_LENGTH, EffectTypes.f(e, "radius", 6, 0, EffectTypes.MAX_LENGTH));
        Image im = new Image(8, 8, 0, 0, 1);
        im.px[3] = 1;
        assertSame(im, EffectTypes.apply(new ResolvedEffect("no.such.effect", 1), im));
        Image out = EffectTypes.apply(e, im);   // NaN mix -> 1, huge radius clamped
        assertNotNull(out);
    }

    @Test
    void theLockedFixturesEffectsRenderThroughTheLayeredPath() throws Exception {
        // v1-kitchen-sink.png stays the flat render (format lock); this golden is the
        // same project through the effects pipeline (glow, blurred glow, inverting
        // adjustment layer at 75%).
        Project p = app.barkfart.io.BarkFile.load(new File(System.getProperty("barkfart.root", ".."), "core/src/test/resources/formats/v1-kitchen-sink.bark"));
        Image im = new Image(W, H, -W / 2f, H / 2f, 0.5f);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0.5)), im);
        compare("v1_effects", im);
    }

    static void compare(String name, Image im) throws IOException {
        BufferedImage img = new BufferedImage(im.w, im.h, BufferedImage.TYPE_INT_RGB);
        for (int y = 0; y < im.h; y++) {
            for (int x = 0; x < im.w; x++) {
                int o = (y * im.w + x) * 4;
                float a = im.px[o + 3];
                int r = Math.round(255 * Math.min(1, im.px[o] + (1 - a))), g = Math.round(255 * Math.min(1, im.px[o + 1] + (1 - a))), b = Math.round(255 * Math.min(1, im.px[o + 2] + (1 - a)));
                img.setRGB(x, y, (r << 16) | (g << 8) | b);   // over white
            }
        }
        File dir = new File(System.getProperty("barkfart.root", ".."), "core/src/test/resources/golden");
        File f = new File(dir, name + ".png");
        if (Boolean.getBoolean("barkfart.updateGoldens") || !f.exists()) {
            ImageIO.write(img, "png", f);
            if (!Boolean.getBoolean("barkfart.updateGoldens")) fail("golden created, review and commit: " + f);
            return;
        }
        BufferedImage ref = ImageIO.read(f);
        int bad = 0;
        for (int y = 0; y < im.h; y++) {
            for (int x = 0; x < im.w; x++) {
                int a = ref.getRGB(x, y), b = img.getRGB(x, y);
                for (int s = 0; s < 24; s += 8) if (Math.abs(((a >> s) & 0xff) - ((b >> s) & 0xff)) > 8) {
                    bad++;
                    break;
                }
            }
        }
        if (bad > im.w * im.h / 1000) {
            File out = new File(System.getProperty("barkfart.root", ".."), "core/build/golden-actual");
            out.mkdirs();
            ImageIO.write(img, "png", new File(out, name + ".png"));
            fail(name + ": " + bad + " pixels differ");
        }
    }
}
