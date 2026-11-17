package app.barkfart.canvas;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.edit.JoinOps;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

class HandleFiltersTest {
    private static Set<String> ids(List<CanvasTools.Handle> hs, Layer l) {
        Set<String> out = new HashSet<String>();
        for (CanvasTools.Handle h : hs) if (h.layer == l) out.add(h.node.id);
        return out;
    }

    @Test
    void mainNodesOnlyHiddenPinsAndNeighbours() {
        Project p = Fixtures.empty();
        Layer.FigureLayer body = new Layer.FigureLayer(p.newId("L"), "body");
        body.figure.add(new Node(body.id, "r", null, Node.Shape.NONE));
        Fixtures.add(body, "arm", "r", Node.Shape.LINE, 0, 50);
        Fixtures.add(body, "hand", "arm", Node.Shape.LINE, 0, 30);
        Fixtures.add(body, "leg", "r", Node.Shape.LINE, -90, 50);
        Layer.FigureLayer sword = new Layer.FigureLayer(p.newId("L"), "sword");
        sword.figure.add(new Node(sword.id, "r", null, Node.Shape.NONE));
        Fixtures.add(sword, "tip", "r", Node.Shape.LINE, 0, 40);
        p.root().layers.add(body);
        p.root().layers.add(sword);
        p.invalidateIndex();
        Document d = new Document(p);
        d.apply(JoinOps.join(p, p.root(), sword, body, "hand", true, 0));
        CanvasTools tools = new CanvasTools(new Editor(d));
        assertEquals(4, ids(tools.handles(0), body).size());
        tools.mainNodesOnly = true;
        assertTrue(tools.filtering());
        assertEquals(new HashSet<String>(java.util.Arrays.asList("r", "hand")), ids(tools.handles(0), body), "main node and the join anchor");
        assertEquals(new HashSet<String>(java.util.Arrays.asList("r")), ids(tools.handles(0), sword));
        tools.activeFigure = sword;
        assertEquals(2, ids(tools.handles(0), sword).size(), "the active figure shows all its joints");
        tools.mainNodesOnly = false;
        tools.activeFigure = null;
        body.figure.node("leg").pinned = true;
        tools.hideLocked = true;
        assertFalse(ids(tools.handles(0), body).contains("leg"), "pinned joints hidden");
        tools.hideLocked = false;
        Set<String> n1 = new HashSet<String>();
        for (CanvasTools.Handle h : tools.neighbours(body, body.figure.node("hand"), false, 0)) n1.add(h.layer.name + ":" + h.node.id);
        assertEquals(new HashSet<String>(java.util.Arrays.asList("body:arm")), n1, "parent only (no children)");
        Set<String> n2 = new HashSet<String>();
        for (CanvasTools.Handle h : tools.neighbours(body, body.figure.node("hand"), true, 0)) n2.add(h.layer.name + ":" + h.node.id);
        assertEquals(new HashSet<String>(java.util.Arrays.asList("body:arm", "sword:r")), n2, "across the join");
        Set<String> n3 = new HashSet<String>();
        for (CanvasTools.Handle h : tools.neighbours(sword, sword.figure.root(), true, 0)) n3.add(h.layer.name + ":" + h.node.id);
        assertEquals(new HashSet<String>(java.util.Arrays.asList("sword:tip", "body:hand")), n3, "and back to the anchor");
    }
}
