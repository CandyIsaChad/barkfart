package app.barkfart.canvas;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.ArrayList;
import org.junit.jupiter.api.Test;

/** Node options: smart-stretch (fitted to the original), static nodes, angle and drag locks. */
class NodeOptionsTest {
    private Project p;
    private Layer.FigureLayer l;
    private Document d;
    private CanvasTools tools;

    private Node add(String id, String parent, float rot, float len, boolean isStatic) {
        Node n = Fixtures.add(l, id, parent, Node.Shape.LINE, rot, len);
        n.isStatic = isStatic;
        n.restLength = len;
        return n;
    }

    private void figure() {
        p = Fixtures.empty();
        l = new Layer.FigureLayer(p.newId("L"), "f");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        p.root().layers.add(l);
    }

    private void start() {
        p.invalidateIndex();
        d = new Document(p);
        tools = new CanvasTools(new Editor(d));
        tools.stretch = true;
    }

    private float len(String id) {
        return new PropertyResolver(p, new ArrayList<Diagnostic>()).f(l.figure.node(id).length, 0);
    }

    private FigurePose pose() {
        return FigurePose.compute(l, 0, new PropertyResolver(p, new ArrayList<Diagnostic>()));
    }

    @Test
    void smartStretchReproducesTheOriginalsOwnStretchedCircle() {
        // The original's Rain clip: a smart-stretch node (90 deg, 132.3) stretched to
        // 701.2; static details hang off a static node pointing back (180) at 85..5 deg.
        // These are the lengths the original stored for that stretch.
        figure();
        Node s = add("s", "r", 90, 132.3f, false);
        s.stretchy = true;
        s.smartStretch = true;
        add("back", "s", 180, 132.3f, true);
        float[] want = {145.3f, 178.4f, 222.0f, 270.1f, 319.7f, 368.8f, 416.5f, 462.0f, 504.5f, 543.8f, 579.4f, 610.8f, 637.9f, 660.4f, 678.1f, 690.9f, 698.6f};
        for (int k = 0; k < want.length; k++) add("d" + k, "back", 85 - 5 * k, 132.3f, true);
        start();
        tools.smartStretch(l, pose(), l.figure.indexOf("s"), 701.2f, 0);
        assertEquals(701.2f, len("back"), 0.2f);
        for (int k = 0; k < want.length; k++) assertEquals(want[k], len("d" + k), 0.2f, "d" + k);
    }

    @Test
    void smartStretchKeepsPerpendicularDetailsAndSkipsMovingOrExcludedBranches() {
        // Action Lines: details across the stretched node keep their length (966).
        figure();
        Node s = add("s", "r", 90, 484, false);
        s.smartStretch = true;
        add("across", "s", 90, 966, true);
        add("along", "s", 180, 484, true);
        add("free", "s", 180, 100, false);          // not static: a moving part, left alone
        add("freeDetail", "free", 0, 50, true);     // behind a moving node: left alone
        Node no = add("no", "s", 180, 80, true);
        no.noSmartStretch = true;
        add("noChild", "no", 0, 40, true);
        start();
        tools.smartStretch(l, pose(), l.figure.indexOf("s"), 234, 0);
        assertEquals(966, len("across"), 0.01f);
        assertEquals(234, len("along"), 0.01f);
        assertEquals(100, len("free"), 0.01f);
        assertEquals(50, len("freeDetail"), 0.01f);
        assertEquals(80, len("no"), 0.01f);
        assertEquals(40, len("noChild"), 0.01f);
    }

    @Test
    void stretchingASmartStretchJointIsOneUndoStep() {
        figure();
        Node s = add("s", "r", 0, 100, false);
        s.smartStretch = true;
        add("along", "s", 0, 50, true);
        start();
        String before = ProjectCodec.encode(p);
        CanvasTools.Handle h = null;
        for (CanvasTools.Handle x : tools.handles(0)) if (x.node.id.equals("s")) h = x;
        tools.begin(tools.pick(h.x, h.y, 5, 0), h.x, h.y, 0);
        tools.move(h.x + 100, h.y, 0);
        tools.end();
        assertEquals(200, len("s"), 0.5f);
        assertEquals(100, len("along"), 0.5f);
        d.undo();
        assertEquals(before, ProjectCodec.encode(p));
    }

    @Test
    void staticNodesHaveNoHandlesWhileAnimating() {
        figure();
        add("arm", "r", 0, 100, false);
        add("detail", "arm", 90, 20, true);
        start();
        for (CanvasTools.Handle h : tools.handles(0)) assertNotEquals("detail", h.node.id);
        tools.showStatic = true;
        boolean seen = false;
        for (CanvasTools.Handle h : tools.handles(0)) seen |= h.node.id.equals("detail");
        assertTrue(seen);
    }

    @Test
    void angleLocksMeasureFromTheStageOrTheNearestMovingAncestor() {
        figure();
        add("arm", "r", 40, 100, false);
        add("plate", "arm", 30, 50, true);           // static detail
        Node abs = add("abs", "plate", 10, 30, false);
        abs.angleLock = Node.LOCK_ABSOLUTE;
        Node rel = add("rel", "plate", 10, 30, false);
        rel.angleLock = Node.LOCK_RELATIVE;
        Node plain = add("plain", "plate", 10, 30, false);
        start();
        FigurePose ps = pose();
        assertEquals(10, ps.angle[l.figure.indexOf("abs")], 1e-9);
        assertEquals(50, ps.angle[l.figure.indexOf("rel")], 1e-9);     // arm (40) + 10, skipping the static plate
        assertEquals(80, ps.angle[l.figure.indexOf("plain")], 1e-9);   // 40 + 30 + 10
        // Dragging a locked joint still points it at the finger.
        CanvasTools.Handle h = null;
        for (CanvasTools.Handle x : tools.handles(0)) if (x.node.id.equals("abs")) h = x;
        tools.stretch = false;
        tools.begin(tools.pick(h.x, h.y, 2, 0), h.x, h.y, 0);
        FigurePose p0 = pose();
        int ai = l.figure.indexOf("abs"), pi = p0.parent[ai];
        float px = (float) p0.x[pi], py = (float) p0.y[pi];
        tools.move(h.x + (px - h.x) + 0, h.y + 30, 0);   // straight up from the joint's parent... in stage space
        tools.end();
        FigurePose p1 = pose();
        assertEquals(90, ((p1.angle[ai] % 360) + 360) % 360, 1.0);
    }

    @Test
    void dragLockHoldsTheJointToOneAxis() {
        figure();
        Node a = add("a", "r", 0, 100, false);
        a.dragLock = Node.DRAG_HORIZONTAL;
        start();
        CanvasTools.Handle h = tools.handles(0).get(1);
        assertEquals("a", h.node.id);
        tools.begin(tools.pick(h.x, h.y, 2, 0), h.x, h.y, 0);
        tools.move(h.x - 20, h.y + 80, 0);   // the vertical part is ignored
        tools.end();
        assertEquals(0, new PropertyResolver(p, new ArrayList<Diagnostic>()).f(a.rotation, 0) % 360, 1e-3);
    }

    @Test
    void optionsSurviveSaveAndLoad() throws Exception {
        figure();
        Node c = add("c", "r", 0, 100, false);
        c.shape = Node.Shape.CIRCLE;
        c.isStatic = true;
        c.stretchy = true;
        c.smartStretch = true;
        c.noSmartStretch = true;
        c.restLength = 77;
        c.circleHollow = true;
        c.circleOutline = true;
        c.outlineColor.setValue(app.barkfart.model.Color.rgba8888(0x336699ff));
        c.halfArc = -1;
        c.polygonSides = 7;
        c.trapezoidRatio = 0.25f;
        c.trapezoidRoundStart = true;
        c.trapezoidRoundEnd = true;
        c.angleLock = Node.LOCK_RELATIVE;
        c.dragLock = Node.DRAG_VERTICAL;
        String a = ProjectCodec.encode(p);
        Project q = ProjectCodec.decode(a);
        assertEquals(a, ProjectCodec.encode(q));
        assertTrue(a.contains("\"outlineColor\""));
        // defaults are not written
        figure();
        add("plain", "r", 0, 100, false).restLength = 0;
        String b = ProjectCodec.encode(p);
        for (String k : new String[] {"static", "stretchy", "smartStretch", "hollow", "outline", "halfArc", "sides", "trapezoidRatio", "angleLock", "dragLock", "restLength"}) {
            assertFalse(b.contains("\"" + k + "\""), k);
        }
    }
}
