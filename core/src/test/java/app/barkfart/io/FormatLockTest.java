package app.barkfart.io;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.geom.Raster;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

/**
 * Locks the .bark format (FORMAT.md): files written by format version 1 must
 * load in every later build and render the same.
 */
class FormatLockTest {
    static File res(String name) {
        return new File(System.getProperty("barkfart.root", ".."), "core/src/test/resources/formats/" + name);
    }

    /** Creates the fixture once (or with -Dbarkfart.updateGoldens=true). Never regenerated otherwise. */
    @Test
    void version1FixtureLoadsAndRendersTheSame() throws Exception {
        File fixture = res("v1-kitchen-sink.bark"), image = res("v1-kitchen-sink.png");
        if (!fixture.exists() || Boolean.getBoolean("barkfart.updateGoldens")) {
            FileOutputStream out = new FileOutputStream(fixture);
            BarkFile.write(Fixtures.kitchenSink(), out);
            out.close();
            Project p = BarkFile.load(fixture);
            ImageIO.write(render(p), "png", image);
            fail("format fixture created; review and commit " + fixture);
        }
        Project p = BarkFile.load(fixture);
        assertEquals(1 + 1, p.library.size());
        assertEquals(7, p.root().layers.size());
        Layer.FigureLayer man = (Layer.FigureLayer) p.root().layers.get(0);
        assertEquals(3, man.figure.node("armR1").rotation.track().size());
        assertEquals(1, man.figure.polyfills.size());
        BufferedImage now = render(p), ref = ImageIO.read(image);
        int bad = 0;
        for (int y = 0; y < ref.getHeight(); y++) for (int x = 0; x < ref.getWidth(); x++) if (ref.getRGB(x, y) != now.getRGB(x, y)) bad++;
        assertTrue(bad <= ref.getWidth() * ref.getHeight() / 1000, bad + " pixels differ from the v1 render");
    }

    private static BufferedImage render(Project p) {
        return Raster.fill(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0.6)), 320, 240, 0xffffffff);
    }

    @Test
    void minimalFileLoadsWithDefaults() throws Exception {
        String json = "{\"format\": \"barkfart\", \"formatVersion\": 1, \"rootSymbol\": \"S1\","
                + " \"library\": [{\"id\": \"S1\", \"layers\": [{\"id\": \"L1\", \"type\": \"FIGURE\","
                + " \"nodes\": [{\"id\": \"r\"}, {\"id\": \"a\", \"parent\": \"r\", \"length\": {\"v\": 20.0}}]}]}]}";
        Project p = ProjectCodec.decode(json);
        Layer.FigureLayer l = (Layer.FigureLayer) p.root().layers.get(0);
        assertTrue(l.visible);
        assertEquals(Layer.Blend.NORMAL, l.blend);
        assertEquals(30f, p.settings.fps);
        assertEquals(1.0, p.root().duration);
        assertEquals(Node.Shape.LINE, l.figure.node("a").shape);
        assertEquals(1, Tessellator.tessellate(Evaluator.evaluate(p, "S1", 0)).batches.size());
    }

    @Test
    void unknownFieldsAreIgnored() throws Exception {
        String s = ProjectCodec.encode(Fixtures.kitchenSink())
                .replace("\"formatVersion\": 1,", "\"formatVersion\": 1, \"futureThing\": {\"x\": [1, 2]},");
        assertEquals(ProjectCodec.encode(Fixtures.kitchenSink()), ProjectCodec.encode(ProjectCodec.decode(s)));
    }

    @Test
    void drawOrderZeroOnANonRootNodeSurvives() throws Exception {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "m");
        l.figure.node("legL2").drawOrder = 0;   // behind everything
        p.root().layers.add(l);
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        assertEquals(0, ((Layer.FigureLayer) q.root().layers.get(0)).figure.node("legL2").drawOrder);
    }

    @Test
    void migrationStepsRunInOrder() {
        List<Migrations.Step> steps = new ArrayList<Migrations.Step>();
        steps.add(new Migrations.Step() {   // v1 -> v2: rename a field
            public Map<String, Object> apply(Map<String, Object> m) {
                m.put("b", m.remove("a"));
                return m;
            }
        });
        steps.add(new Migrations.Step() {   // v2 -> v3: add a field
            public Map<String, Object> apply(Map<String, Object> m) {
                m.put("c", "added");
                return m;
            }
        });
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        m.put("a", "x");
        Map<String, Object> out = Migrations.upgrade(m, 1, 3, steps);
        assertEquals("x", out.get("b"));
        assertEquals("added", out.get("c"));
        assertFalse(out.containsKey("a"));
        assertThrows(IllegalArgumentException.class, () -> Migrations.upgrade(new LinkedHashMap<String, Object>(), 1, 4, steps));
    }
}
