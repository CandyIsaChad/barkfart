package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.IOException;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

/**
 * Self-regression goldens (DESIGN §5): synthetic projects, evaluated,
 * tessellated by core and filled by the test rasterizer. They guard against
 * unintended changes; they are NOT yet compared to the original app.
 * Regenerate deliberately with -Dbarkfart.updateGoldens=true.
 * Tolerance: at most 0.1% of pixels may differ by more than 2 levels.
 */
class GoldenTest {
    private static final int W = 320, H = 240;

    static Project stickmanProject() {
        Project p = Fixtures.empty();
        p.root().layers.add(Fixtures.stickman(p, "man"));
        return p;
    }

    @Test
    void stickmanAtRest() throws IOException {
        check("stickman_rest", stickmanProject(), 0);
    }

    @Test
    void animatedStickmanThroughRotatedZoomedCamera() throws IOException {
        Project p = stickmanProject();
        Layer.FigureLayer man = (Layer.FigureLayer) p.root().layers.get(0);
        Node arm = man.figure.node("armR1");
        arm.rotation.track().put(new Key(0, 150f, Interp.BEZIER, Ease.EASE_IN_OUT));
        arm.rotation.track().put(new Key(1, 30f, Interp.LINEAR));
        man.figure.node("head").color.setValue(Color.rgba8888(0xe0602aff));
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.zoom.setValue(1.4f);
        cam.rotation.setValue(10f);
        cam.y.setValue(20f);
        p.root().layers.add(cam);
        check("stickman_anim_camera", p, 0.5);
    }

    @Test
    void selfNestingSymbol() throws IOException {
        Project p = stickmanProject();
        Symbol root = p.root();
        Layer.SymbolLayer self = new Layer.SymbolLayer(p.newId("L"), "self", root.id);
        self.transform.x.setValue(60f);
        self.transform.scaleX.setValue(0.6f);
        self.transform.scaleY.setValue(0.6f);
        self.recursionLimit = 5;
        root.layers.add(self);
        check("self_nesting", p, 0);
    }

    @Test
    void translucentOverlappingShapes() throws IOException {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "shapes");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node a = Fixtures.add(l, "a", "r", Node.Shape.LINE, 20, 90);
        a.thickness.setValue(40f);
        a.color.setValue(Color.rgba8888(0x2a6fe0c0));
        Node b = Fixtures.add(l, "b", "r", Node.Shape.CIRCLE, 160, 120);
        b.color.setValue(Color.rgba8888(0xe02a6f80));
        l.opacity.setValue(0.9f);
        p.root().layers.add(l);
        check("translucent_shapes", p, 0);
    }

    @Test
    void gradientShapes() throws IOException {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "g");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node along = Fixtures.add(l, "along", "r", Node.Shape.LINE, 160, 110);
        along.thickness.setValue(26f);
        along.color.setValue(Color.rgba8888(0xff3b30ff));
        along.gradient = true;
        along.gradientColor.setValue(Color.rgba8888(0xffd60aff));
        Node side = Fixtures.add(l, "side", "r", Node.Shape.SQUARE_LINE, 20, 110);
        side.thickness.setValue(30f);
        side.color.setValue(Color.rgba8888(0x0a84ffff));
        side.gradient = true;
        side.gradientSideways = true;
        side.gradientColor.setValue(Color.rgba8888(0x30d158ff));
        Node ball = Fixtures.add(l, "ball", "r", Node.Shape.CIRCLE, -90, 90);
        ball.color.setValue(Color.rgba8888(0xffffffff));
        ball.gradient = true;
        ball.gradientColor.setValue(Color.rgba8888(0x5e5ce6ff));
        Node fade = Fixtures.add(l, "fade", "r", Node.Shape.LINE, -160, 100);
        fade.thickness.setValue(20f);
        fade.color.setValue(Color.rgba8888(0x000000ff));
        fade.gradient = true;
        fade.gradientColor.setValue(Color.rgba8888(0x00000000));   // fades out
        p.root().layers.add(l);
        check("gradient_shapes", p, 0);
    }

    @Test
    void ellipseRadialGradientHasItsCentreColour() {
        // An ellipse fanned from a rim vertex has no interior vertex, so a radial
        // gradient came out flat at the rim colour (SSJ Aura's glow, NOTES.md).
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "g");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node e = Fixtures.add(l, "e", "r", Node.Shape.ELLIPSE, 0, 160);
        e.thickness.setValue(60f);
        e.color.setValue(Color.rgba8888(0xffffffff));
        e.gradient = true;
        e.gradientColor.setValue(Color.rgba8888(0x0000ffff));
        p.root().layers.add(l);
        DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0));
        boolean centre = false, rim = false;
        for (DrawList.Batch b : dl.batches) {
            for (int i = 0; i < b.vertices; i++) {
                int red = b.rgba(i) >>> 24;                       // RGBA8888: white has red 255, blue 0
                centre |= red >= 0xf0;
                rim |= red <= 0x0f;
            }
        }
        assertTrue(centre, "a vertex carries the centre colour");
        assertTrue(rim, "the outline carries the rim colour");
    }

    @Test
    void perVertexColourGradient() throws IOException {
        DrawList dl = new DrawList();
        DrawList.Batch b = dl.batch(Layer.Blend.NORMAL, null);
        b.add(-120, -90, 0xff0000ff);
        b.add(120, -90, 0x00ff00ff);
        b.add(0, 100, 0x0000ff80);
        BufferedImage img = Raster.fill(dl, W, H, 0xffffffff);
        compare("gradient_triangle", img);
        // centre pixel is a mix of all three corners, not one flat colour
        int c = img.getRGB(W / 2, H / 2 + 20);
        assertTrue(((c >> 16) & 0xff) > 20 && ((c >> 8) & 0xff) > 20 && (c & 0xff) > 20, Integer.toHexString(c));
    }

    @Test
    void texturedSprite() throws IOException {
        Project p = Fixtures.empty();
        p.assets.put("A1", new app.barkfart.model.Asset("A1", app.barkfart.model.Asset.Kind.IMAGE, "assets/A1.png"));
        Layer.SpriteLayer sp = new Layer.SpriteLayer(p.newId("L"), "checker");
        sp.asset = "A1";
        sp.width = 120;
        sp.height = 90;
        sp.transform.x.setValue(-60f);
        sp.transform.y.setValue(-45f);
        sp.transform.rotation.setValue(12f);
        sp.tint.setValue(Color.rgba8888(0xffffffc0));
        p.root().layers.add(sp);
        BufferedImage tex = new BufferedImage(4, 4, BufferedImage.TYPE_INT_ARGB);
        for (int y = 0; y < 4; y++) for (int x = 0; x < 4; x++) tex.setRGB(x, y, y < 2 ? 0xff0a84ff : ((x + y) % 2 == 0 ? 0xffff9f0a : 0xff30d158));
        java.util.Map<String, BufferedImage> texs = new java.util.HashMap<String, BufferedImage>();
        texs.put("A1", tex);
        BufferedImage img = Raster.fill(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0)), W, H, 0xffffffff, texs);
        compare("textured_sprite", img);
        // top of the sprite shows the image's top rows (blue), i.e. not flipped
        int top = img.getRGB(W / 2 - 10, H / 2 - 30);
        assertTrue((top & 0xff) > ((top >> 16) & 0xff), "blue on top: " + Integer.toHexString(top));
    }

    private static void check(String name, Project p, double t) throws IOException {
        Scene s = Evaluator.evaluate(p, p.rootSymbol, t);
        compare(name, Raster.fill(Tessellator.tessellate(s), W, H, 0xffffffff));
    }

    private static void compare(String name, BufferedImage img) throws IOException {
        File dir = new File(System.getProperty("barkfart.root", ".."), "core/src/test/resources/golden");
        File f = new File(dir, name + ".png");
        if (Boolean.getBoolean("barkfart.updateGoldens") || !f.exists()) {
            dir.mkdirs();
            ImageIO.write(img, "png", f);
            if (!Boolean.getBoolean("barkfart.updateGoldens")) fail("golden created, review and commit: " + f);
            return;
        }
        BufferedImage ref = ImageIO.read(f);
        assertEquals(W, ref.getWidth());
        assertEquals(H, ref.getHeight());
        int bad = 0;
        for (int y = 0; y < H; y++) {
            for (int x = 0; x < W; x++) {
                if (differs(ref.getRGB(x, y), img.getRGB(x, y))) bad++;
            }
        }
        if (bad > W * H / 1000) {
            File out = new File(System.getProperty("barkfart.root", ".."), "core/build/golden-actual");
            out.mkdirs();
            ImageIO.write(img, "png", new File(out, name + ".png"));
            fail(name + ": " + bad + " pixels differ; actual written to " + out);
        }
    }

    private static boolean differs(int a, int b) {
        for (int sh = 0; sh < 32; sh += 8) {
            if (Math.abs(((a >>> sh) & 0xff) - ((b >>> sh) & 0xff)) > 2) return true;
        }
        return false;
    }
}
