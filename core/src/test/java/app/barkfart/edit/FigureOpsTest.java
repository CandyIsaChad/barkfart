package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.eval.Shape;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.ArrayList;
import org.junit.jupiter.api.Test;

class FigureOpsTest {
    private Project p;
    private Layer.FigureLayer l;
    private Document d;

    private void setUp() {
        p = Fixtures.empty();
        l = Fixtures.stickman(p, "man");
        p.root().layers.add(l);
        p.invalidateIndex();
        d = new Document(p);
    }

    private FigurePose pose(double t) {
        return FigurePose.compute(l, t, new PropertyResolver(p, new ArrayList<Diagnostic>()));
    }

    private double[] at(String id) {
        FigurePose ps = pose(0);
        int i = l.figure.indexOf(id);
        return new double[] {ps.x[i], ps.y[i]};
    }

    /** Applies, checks undo restores the file exactly, redoes, returns the after-state text. */
    private String roundTrip(Command c) {
        String before = ProjectCodec.encode(p);
        d.apply(c);
        String after = ProjectCodec.encode(p);
        assertNotEquals(before, after);
        d.undo();
        assertEquals(before, ProjectCodec.encode(p), "undo restores exactly");
        d.redo();
        assertEquals(after, ProjectCodec.encode(p), "redo gives the same result");
        return after;
    }

    @Test
    void addSegmentInheritsItsParentsLook() {
        setUp();
        Node arm = l.figure.node("armR1");
        arm.isStatic = true;
        arm.color.setValue(Color.rgba8888(0xff0000ff));
        arm.thickness.setValue(17f);
        FigureOps.Made m = new FigureOps.Made();
        roundTrip(FigureOps.addSegment(l, "armR1", 30, 40, m));
        Node n = l.figure.node(m.node.id);
        assertSame(m.node, n, "redo restores the same node object");
        assertEquals("armR1", n.parent);
        assertTrue(n.isStatic);
        assertEquals(17f, n.thickness.floatValue());
        assertEquals(Color.rgba8888(0xff0000ff), n.color.value());
        assertEquals(40f, n.restLength);
    }

    @Test
    void splittingAStraightSegmentKeepsItsEndAndChildren() {
        setUp();
        double[] hand = at("armR2"), end = at("armR1");
        float len = l.figure.node("armR1").length.floatValue();
        roundTrip(FigureOps.split(l, "armR1", 3));
        assertArrayEquals(end, at("armR1"), 1e-4, "the split node still ends where it did");
        assertArrayEquals(hand, at("armR2"), 1e-4, "its children do not move");
        Node a = l.figure.node("armR1");
        assertEquals(len / 3, a.length.floatValue(), 1e-4);
        assertEquals(len / 3, l.figure.node(a.parent).length.floatValue(), 1e-4);
    }

    @Test
    void splittingACurvedSegmentFollowsItsArc() {
        setUp();
        Node a = l.figure.node("armR1");
        a.curve = 25;
        double[] end = at("armR1");
        FigurePose before = pose(0);
        int ai = l.figure.indexOf("armR1"), pi = before.parent[ai];
        double ax = before.x[pi], ay = before.y[pi], bx = before.x[ai], by = before.y[ai];
        double chord = Math.hypot(bx - ax, by - ay), h = a.curve * Shape.CURVE_SAGITTA;
        double rad = (chord * chord / 4 + h * h) / (2 * h);
        double nx = -(by - ay) / chord, ny = (bx - ax) / chord;                  // left of parent -> node
        double cx = (ax + bx) / 2 + nx * (h - rad), cy = (ay + by) / 2 + ny * (h - rad);   // positive curve bulges left
        roundTrip(FigureOps.split(l, "armR1", 4));
        assertArrayEquals(end, at("armR1"), 1e-3, "end joint unchanged");
        // every new joint lies on the original arc, on the left of the chord
        String id = l.figure.node("armR1").parent;
        int joints = 0;
        while (!id.equals(l.figure.node("armR1").parent) || joints == 0) {
            double[] q = at(id);
            assertEquals(Math.abs(rad), Math.hypot(q[0] - cx, q[1] - cy), 1e-2, "joint on the arc");
            assertTrue((bx - ax) * (q[1] - ay) - (by - ay) * (q[0] - ax) > 0, "bulges left");
            joints++;
            String up = l.figure.node(id).parent;
            if (up.equals(before.parent[ai] < 0 ? "" : l.figure.nodes().get(pi).id)) break;
            id = up;
        }
        assertEquals(3, joints);
        assertEquals(a.curve, l.figure.node(l.figure.node("armR1").parent).curve, 1e-4, "pieces share the curvature");
    }

    @Test
    void duplicatingABranchCopiesItOnTop() {
        setUp();
        FigureOps.Made m = new FigureOps.Made();
        int before = l.figure.nodes().size();
        roundTrip(FigureOps.duplicateBranch(l, "armR1", m));
        assertEquals(before + 2, l.figure.nodes().size());
        Node copy = l.figure.node(m.node.id);
        assertArrayEquals(at("armR1"), at(copy.id), 1e-6, "same place");
        int maxOther = Integer.MIN_VALUE;
        for (Node x : l.figure.nodes()) if (x.drawOrder > maxOther && x != copy && !copy.id.equals(x.parent)) maxOther = Math.max(maxOther, x.drawOrder);
        assertTrue(copy.drawOrder > l.figure.node("armR1").drawOrder, "copy drawn above the original");
    }

    @Test
    void flippingABranchMirrorsItAboutItsJoint() {
        setUp();
        double[] joint = at(l.figure.node("armR1").parent), hand = at("armR2"), elbow = at("armR1");
        l.figure.node("armR1").rotation.track().put(new Key(0, l.figure.node("armR1").rotation.floatValue(), Interp.LINEAR));
        roundTrip(FigureOps.flipBranch(p, l, "armR1", true, 0));
        assertEquals(2 * joint[0] - elbow[0], at("armR1")[0], 1e-3, "elbow mirrored in x");
        assertEquals(elbow[1], at("armR1")[1], 1e-3);
        assertEquals(2 * joint[0] - hand[0], at("armR2")[0], 1e-3, "hand mirrored in x");
        assertEquals(hand[1], at("armR2")[1], 1e-3);
        d.undo();
        roundTrip(FigureOps.flipBranch(p, l, "armR1", false, 0));
        assertEquals(2 * joint[1] - hand[1], at("armR2")[1], 1e-3, "vertical flip mirrors y");
    }

    @Test
    void flipFigureModifyBranchAndPushBranch() {
        setUp();
        roundTrip(FigureOps.flipFigure(l, true));
        assertEquals(-1f, l.transform.scaleX.floatValue());
        roundTrip(FigureOps.modifyBranch(l, "armR1", "Thickness", FigureOps.setThickness(3)));
        assertEquals(3f, l.figure.node("armR2").thickness.floatValue());
        assertNotEquals(3f, l.figure.node("armL1").thickness.floatValue(), "other branches untouched");
        roundTrip(FigureOps.modifyBranch(l, "armR1", "Static", FigureOps.setStatic(true)));
        assertTrue(l.figure.node("armR2").isStatic);
        // push the right arm to the very back, one step at a time; its own order stays
        for (int i = 0; i < 20; i++) d.apply(FigureOps.pushBranch(l, "armR1", false));
        int[] seq = l.figure.drawSequence();
        java.util.List<String> order = new ArrayList<String>();
        for (int i : seq) order.add(l.figure.nodes().get(i).id);
        assertTrue(order.indexOf("armR1") < order.indexOf("armR2"));
        for (Node x : l.figure.nodes()) {
            if (x.id.equals("armR1") || x.id.equals("armR2")) continue;
            assertTrue(order.indexOf(x.id) > order.indexOf("armR2"), x.id + " drawn in front of the pushed branch");
        }
    }

    @Test
    void resetLengthReturnsToTheRestLengthAsAKey() {
        setUp();
        Node a = l.figure.node("armR1");
        a.restLength = a.length.floatValue();
        Editor ed = new Editor(d);
        ed.autoKey = true;
        d.begin("stretch");
        ed.change(a.length, a.restLength * 2, 0.5);
        d.commit();
        app.barkfart.canvas.CanvasTools tools = new app.barkfart.canvas.CanvasTools(ed);
        assertTrue(FigureOps.resetLength(tools, ed, p, l, a, 0.5));
        assertEquals(a.restLength, new PropertyResolver(p, new ArrayList<Diagnostic>()).f(a.length, 0.5), 1e-4);
    }
}
