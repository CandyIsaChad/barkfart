package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.Image;
import app.barkfart.model.Color;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class OnionLayeredTest {
    @Test
    void ghostsFadeAsOnePieceAndFollowTheCounts() throws Exception {
        Project p = Fixtures.empty();
        p.root().duration = 2;
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "sq");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        for (String id : new String[] {"a", "b"}) {   // two squares on top of each other
            Node n = Fixtures.add(l, id, "r", Node.Shape.SQUARE_LINE, 0, 40);
            n.thickness.setValue(40f);
            n.color.setValue(Color.rgba8888(0x000000ff));
        }
        l.transform.x.track().put(new Key(0, -100f, Interp.LINEAR));
        l.transform.x.track().put(new Key(2, 100f, Interp.LINEAR));
        p.root().layers.add(l);
        p.invalidateIndex();
        OnionSkin o = new OnionSkin();
        o.before = 1;
        o.after = 0;
        DrawList dl = o.buildLayered(p, p.rootSymbol, 1);
        assertEquals(1, dl.root.items.size(), "one ghost");
        Image im = new Image(320, 120, -160, 60, 1);
        Compositor.render(dl, im);
        // the ghost one frame back sits at x = -100 * (1 - 1/30) + ... ; sample its middle and a corner area
        double gx = -100 + 200 * (1 - 1 / 30.0) / 2;
        int mid = Compositor.rgba(im, (int) (gx + 20 + 160), 60) & 0xff;
        assertEquals(Math.round(o.opacity * 255), mid, 2, "the overlap is as light as a single shape");
        o.before = 3;
        o.after = 2;
        assertEquals(5, o.buildLayered(p, p.rootSymbol, 1).root.items.size());
        p.view.onionBefore = 4;
        p.view.onionInFront = true;
        Project q = app.barkfart.io.ProjectCodec.decode(app.barkfart.io.ProjectCodec.encode(p));
        assertEquals(4, q.view.onionBefore);
        assertTrue(q.view.onionInFront);
    }
}
