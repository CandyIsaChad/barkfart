package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.FigureOps;
import app.barkfart.eval.Evaluator;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.Image;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class CircleLinearGradientTest {
    private static int red(Project p, int x, int y) {
        Image im = new Image(120, 120, -60, 60, 1);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0, false, false)), im);
        return Compositor.rgba(im, x + 60, 60 - y) >>> 24;
    }

    @Test
    void circlesCanUseAStraightGradient() throws Exception {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "c");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node c = Fixtures.add(l, "c", "r", Node.Shape.CIRCLE, 0, 80);   // circle from x=0 to x=80... centred at 40
        c.color.setValue(Color.rgba8888(0x000000ff));
        c.gradient = true;
        c.gradientColor.setValue(Color.rgba8888(0xffffffff));
        l.transform.x.setValue(-40f);
        p.root().layers.add(l);
        p.invalidateIndex();
        // radial: centre dark, left and right rims both light
        int cl = red(p, -36, 0), cr = red(p, 36, 0);
        assertTrue(red(p, 0, 0) < 20 && cl > 200 && cr > 200, "radial " + cl + " " + cr);
        Document d = new Document(p);
        d.apply(FigureOps.edit("Linear gradient", l, new Runnable() {
            public void run() {
                c.gradientLinear = true;
            }
        }));
        assertTrue(red(p, -36, 0) < 20 && red(p, 36, 0) > 235, "along: dark at the joint side, light at the end");
        assertEquals(128, red(p, 0, 0), 10, "halfway in the middle");
        assertEquals(red(p, 0, 30), red(p, 0, -30), 3, "same colour across");
        c.gradientSideways = true;
        assertTrue(red(p, 0, -36) < 20 && red(p, 0, 36) > 235, "sideways: varies across the segment");
        assertTrue(((Layer.FigureLayer) ProjectCodec.decode(ProjectCodec.encode(p)).root().layers.get(0)).figure.node("c").gradientLinear, "saved");
        c.gradientSideways = false;
        d.undo();
        assertFalse(c.gradientLinear, "undo restores the flag");
        d.apply(FigureOps.setPinned(l, "c", true));
        d.undo();
        assertFalse(c.pinned, "undo unpins");
    }
}
