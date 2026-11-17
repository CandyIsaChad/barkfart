package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class SpriteDistortTest {
    private static DrawList.Batch quad(Project p, double t) {
        DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, t, false, false));
        for (DrawList.Batch b : dl.batches) if ("img".equals(b.texture)) return b;
        throw new AssertionError("no sprite batch");
    }

    /** Vertex nearest to texture coordinate (u, v). */
    private static float[] at(DrawList.Batch b, float u, float v) {
        for (int i = 0; i < b.vertexCount(); i++) {
            if (Math.abs(b.u(i) - u) < 1e-4 && Math.abs(b.v(i) - v) < 1e-4) return new float[] {b.x(i), b.y(i)};
        }
        throw new AssertionError("no vertex at " + u + "," + v);
    }

    @Test
    void cornersMoveAndTheInsideFollowsBilinearly() throws Exception {
        Project p = Fixtures.empty();
        Layer.SpriteLayer s = new Layer.SpriteLayer(p.newId("L"), "s");
        s.asset = "img";
        s.width = 100;
        s.height = 50;
        p.root().layers.add(s);
        p.invalidateIndex();
        assertEquals(6, quad(p, 0).vertexCount(), "plain sprite: two triangles");
        s.distort[4].track().put(new Key(0, 0f, Interp.LINEAR));   // top-right x
        s.distort[4].track().put(new Key(1, 40f, Interp.LINEAR));
        DrawList.Batch b = quad(p, 1);
        assertTrue(b.vertexCount() > 6, "distorted: a grid");
        float[] tr = at(b, 1, 0), bl = at(b, 0, 1), mid = at(b, 0.5f, 0.5f);
        assertEquals(140, tr[0], 1e-3, "top-right corner moved");
        assertEquals(50, tr[1], 1e-3);
        assertEquals(0, bl[0], 1e-3);
        assertEquals(60, mid[0], 1e-3, "centre: average of the four corners' x (0+100+140+0)/4");
        assertEquals(25, mid[1], 1e-3);
        assertEquals(20, at(quad(p, 0.5), 1, 0)[0] - 100, 1e-3, "keyed: halfway");
        // a pure shear stays two triangles
        s.distort[4].track().clear();
        s.distort[4].setValue(30f);
        s.distort[6].setValue(30f);
        assertEquals(6, quad(p, 0).vertexCount());
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        Layer.SpriteLayer qs = (Layer.SpriteLayer) q.root().layers.get(0);
        assertEquals(30f, qs.distort[6].floatValue(), 1e-6, "saved");
        s.distort[4].setValue(0f);
        s.distort[6].setValue(0f);
        assertFalse(ProjectCodec.encode(p).contains("distort"), "plain sprites save no distort field");
    }
}
