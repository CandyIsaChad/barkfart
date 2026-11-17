package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Group;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.GlyphSource;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class TextCornerTest {
    static final GlyphSource BOXES = new GlyphSource() {
        public float basePx() {
            return 20;
        }

        public float lineHeight() {
            return 20;
        }

        public Glyph glyph(char c) {
            return new Glyph("glyphs:0", 0, 0, 1, 1, 0, 0, 10, 10, 10);
        }
    };

    private static float[] bounds(Project p) {
        DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0, false, true), BOXES);
        float[] b = {Float.MAX_VALUE, Float.MAX_VALUE, -Float.MAX_VALUE, -Float.MAX_VALUE};
        for (DrawList.Batch x : dl.batches) {
            if (!"glyphs:0".equals(x.texture)) continue;
            for (int i = 0; i < x.vertexCount(); i++) {
                b[0] = Math.min(b[0], x.x(i));
                b[1] = Math.min(b[1], x.y(i));
                b[2] = Math.max(b[2], x.x(i));
                b[3] = Math.max(b[3], x.y(i));
            }
        }
        return b;
    }

    @Test
    void textLocksToAShotCornerAndCanSitAboveTheBars() throws Exception {
        Project p = Fixtures.empty();
        p.settings.width = 200;
        p.settings.height = 100;
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.x.setValue(500f);
        cam.zoom.setValue(2f);
        Layer.TextLayer tl = new Layer.TextLayer(p.newId("L"), "name");
        tl.text = "ab";
        tl.size = 20;
        p.root().layers.add(tl);
        p.root().layers.add(cam);
        p.invalidateIndex();
        Document d = new Document(p);
        Editor ed = new Editor(d);
        CameraOps.pinToCorner(ed, p.root(), tl, cam, Layer.TextLayer.TOP_RIGHT, 5, 0);
        float[] b = bounds(p);
        assertEquals(95, b[2], 1e-3, "right edge 5 in from the shot's right");
        // locking keeps the size the text had in the shot (camera zoom 2): glyphs are 20 wide
        assertEquals(55, b[0], 1e-3, "two glyphs, right-aligned");
        assertEquals(35, b[3], 1e-3, "line box top 5 in from the top; glyph top 2 x 5 below it");
        CameraOps.pinToCorner(ed, p.root(), tl, cam, Layer.TextLayer.BOTTOM_LEFT, 5, 0);
        b = bounds(p);
        assertEquals(-95, b[0], 1e-3);
        assertEquals(-45 + 10, b[1], 1e-3, "baseline a quarter line above the box bottom (x2)");
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        assertEquals(Layer.TextLayer.BOTTOM_LEFT, ((Layer.TextLayer) q.root().layers.get(0)).corner);
        d.undo();
        d.undo();
        assertNull(tl.parent, "undo unlocks");
        assertEquals(0, tl.corner);
        // above the widescreen bars
        p.settings.letterbox = 3f;
        Group root = Evaluator.evaluate(p, p.rootSymbol, 0, false, true).root;
        assertEquals("letterbox", ((Group) root.children.get(root.children.size() - 1)).source);
        tl.aboveBars = true;
        root = Evaluator.evaluate(p, p.rootSymbol, 0, false, true).root;
        assertEquals(p.rootSymbol + "/" + tl.id, ((Group) root.children.get(root.children.size() - 1)).source);
        assertTrue(((Layer.TextLayer) ProjectCodec.decode(ProjectCodec.encode(p)).root().layers.get(0)).aboveBars);
    }
}
