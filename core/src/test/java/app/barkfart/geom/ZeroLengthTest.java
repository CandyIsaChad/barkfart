package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.Image;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class ZeroLengthTest {
    @Test
    void aZeroLengthSegmentDrawsAsADot() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "f");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node n = Fixtures.add(l, "dot", "r", Node.Shape.LINE, 0, 0);
        n.thickness.setValue(10f);
        p.root().layers.add(l);
        p.invalidateIndex();
        Image im = new Image(40, 40, -20, 20, 1);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0)), im);
        assertEquals(255, Compositor.rgba(im, 20, 20) & 0xff, "centre covered");
        assertEquals(255, Compositor.rgba(im, 23, 20) & 0xff, "radius 5");
        assertEquals(0, Compositor.rgba(im, 27, 20) & 0xff, "nothing beyond");
    }
}
