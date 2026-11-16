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

    private static void check(String name, Project p, double t) throws IOException {
        Scene s = Evaluator.evaluate(p, p.rootSymbol, t);
        BufferedImage img = Raster.fill(Tessellator.tessellate(s), W, H, 0xffffffff);
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
