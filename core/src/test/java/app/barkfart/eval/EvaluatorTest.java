package app.barkfart.eval;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class EvaluatorTest {
    private static List<Shape> shapes(Group g) {
        List<Shape> out = new ArrayList<Shape>();
        collect(g, out);
        return out;
    }

    private static void collect(Group g, List<Shape> out) {
        for (Object o : g.children) {
            if (o instanceof Shape) out.add((Shape) o);
            else collect((Group) o, out);
        }
    }

    private static int groups(Group g) {
        int n = 1;
        for (Object o : g.children) if (o instanceof Group) n += groups((Group) o);
        return n;
    }

    @Test
    void forwardKinematicsChainsRelativeAngles() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "arm");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Fixtures.add(l, "a", "r", Node.Shape.LINE, 0, 10);
        Fixtures.add(l, "b", "a", Node.Shape.LINE, 90, 10);
        p.root().layers.add(l);
        List<Shape> s = shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root);
        assertEquals(2, s.size());
        Shape.Line b = (Shape.Line) s.get(1);
        assertEquals(10, b.x1, 1e-4);
        assertEquals(0, b.y1, 1e-4);
        assertEquals(10, b.x2, 1e-4);
        assertEquals(10, b.y2, 1e-4);
    }

    @Test
    void keysAnimateNodeRotation() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "arm");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node a = Fixtures.add(l, "a", "r", Node.Shape.LINE, 0, 10);
        a.rotation.track().put(new Key(0, 0f, Interp.LINEAR));
        a.rotation.track().put(new Key(1, 90f, Interp.LINEAR));
        p.root().layers.add(l);
        Shape.Line s = (Shape.Line) shapes(Evaluator.evaluate(p, p.rootSymbol, 0.5).root).get(0);
        assertEquals(10 * Math.cos(Math.toRadians(45)), s.x2, 1e-4);
        assertEquals(10 * Math.sin(Math.toRadians(45)), s.y2, 1e-4);
    }

    @Test
    void cameraMapsWorldToView() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "dot");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Fixtures.add(l, "a", "r", Node.Shape.LINE, 0, 100);
        p.root().layers.add(l);
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.x.setValue(100f);
        cam.zoom.setValue(2f);
        p.root().layers.add(cam);
        Shape.Line s = (Shape.Line) shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root).get(0);
        assertEquals(-200, s.x1, 1e-4);   // world 0 is 100 left of camera, zoom 2
        assertEquals(0, s.x2, 1e-4);      // world 100 is the camera centre
        assertEquals(24, s.width, 1e-4);  // thickness 12 scaled by zoom
    }

    @Test
    void hiddenAndSoloLayers() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a"), b = Fixtures.stickman(p, "b");
        p.root().layers.add(a);
        p.root().layers.add(b);
        int both = shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root).size();
        b.solo = true;
        assertEquals(both / 2, shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root).size());
        b.solo = false;
        a.visible = false;
        assertEquals(both / 2, shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root).size());
        a.visible = true;
        a.shown.track().put(new Key(0, true, Interp.HOLD));
        a.shown.track().put(new Key(1, false, Interp.HOLD));
        assertEquals(both, shapes(Evaluator.evaluate(p, p.rootSymbol, 0.5).root).size());
        assertEquals(both / 2, shapes(Evaluator.evaluate(p, p.rootSymbol, 1).root).size());
    }

    @Test
    void layerParentingComposesTransforms() {
        Project p = Fixtures.empty();
        Layer.FigureLayer parent = Fixtures.stickman(p, "parent");
        Layer.FigureLayer child = new Layer.FigureLayer(p.newId("L"), "child");
        child.figure.add(new Node(child.id, "r", null, Node.Shape.NONE));
        Fixtures.add(child, "a", "r", Node.Shape.LINE, 0, 10);
        parent.transform.x.setValue(50f);
        child.parent = parent.id;
        p.root().layers.add(parent);
        p.root().layers.add(child);
        List<Shape> s = shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root);
        Shape.Line last = (Shape.Line) s.get(s.size() - 1);
        assertEquals(50, last.x1, 1e-4);
    }

    @Test
    void parentCycleIsCut() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a"), b = Fixtures.stickman(p, "b");
        a.parent = b.id;
        b.parent = a.id;
        p.root().layers.add(a);
        p.root().layers.add(b);
        Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
        assertTrue(s.has(Diagnostic.Kind.PARENT_CYCLE));
    }

    @Test
    void symbolInstanceTimeOffsetSpeedAndLoop() {
        Project p = Fixtures.empty();
        Symbol inner = p.addSymbol(new Symbol(p.newId("S"), "inner", 1.0));
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "arm");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node a = Fixtures.add(l, "a", "r", Node.Shape.LINE, 0, 10);
        a.rotation.track().put(new Key(0, 0f, Interp.LINEAR));
        a.rotation.track().put(new Key(1, 90f, Interp.LINEAR));
        inner.layers.add(l);
        Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), "inst", inner.id);
        inst.speed.setValue(2.0);
        inst.timeOffset.setValue(0.25);
        p.root().layers.add(inst);
        // root t = 0.5 -> local 0.25 + 0.5 * 2 = 1.25 -> LOOP over 1.0 -> 0.25 -> 22.5 degrees
        Shape.Line s = (Shape.Line) shapes(Evaluator.evaluate(p, p.rootSymbol, 0.5).root).get(0);
        assertEquals(10 * Math.sin(Math.toRadians(22.5)), s.y2, 1e-4);
        assertEquals(0.25, Evaluator.localTime(1.25, 1, Layer.SymbolLayer.LoopMode.LOOP), 1e-12);
        assertEquals(0.75, Evaluator.localTime(1.25, 1, Layer.SymbolLayer.LoopMode.PING_PONG), 1e-12);
        assertEquals(1.0, Evaluator.localTime(1.25, 1, Layer.SymbolLayer.LoopMode.ONCE), 1e-12);
    }

    @Test
    void selfReferencingSymbolStopsAtItsRecursionLimit() {
        Project p = Fixtures.empty();
        Symbol root = p.root();
        root.layers.add(Fixtures.stickman(p, "man"));
        Layer.SymbolLayer self = new Layer.SymbolLayer(p.newId("L"), "self", root.id);
        self.transform.scaleX.setValue(0.5f);
        self.transform.scaleY.setValue(0.5f);
        root.layers.add(self);
        Scene s = Evaluator.evaluate(p, root.id, 0);
        assertTrue(s.has(Diagnostic.Kind.RECURSION_LIMIT));
        int perLevel = 10;  // stickman shapes
        // Root occurrence plus 8 recursive levels.
        assertEquals(perLevel * 9, shapes(s.root).size());

        self.recursionLimit = 0;
        assertEquals(perLevel, shapes(Evaluator.evaluate(p, root.id, 0).root).size());
    }

    @Test
    void exponentialSelfNestingHitsTheBudgetNotTheStack() {
        Project p = Fixtures.empty();
        Symbol root = p.root();
        for (int i = 0; i < 10; i++) {
            Layer.SymbolLayer self = new Layer.SymbolLayer(p.newId("L"), "self" + i, root.id);
            self.recursionLimit = 60;
            root.layers.add(self);
        }
        Scene s = Evaluator.evaluate(p, root.id, 0);
        assertTrue(s.has(Diagnostic.Kind.PRIMITIVE_BUDGET));
        assertTrue(s.primitives <= Evaluator.PRIMITIVE_BUDGET);
        assertTrue(groups(s.root) + shapes(s.root).size() <= Evaluator.PRIMITIVE_BUDGET + 1);
    }

    @Test
    void deepNonRecursiveChainHitsDepthCap() {
        Project p = Fixtures.empty();
        Symbol prev = p.root();
        for (int i = 0; i < 100; i++) {
            Symbol s = p.addSymbol(new Symbol(p.newId("S"), "s" + i, 1));
            prev.layers.add(new Layer.SymbolLayer(p.newId("L"), "i" + i, s.id));
            prev = s;
        }
        Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
        assertTrue(s.has(Diagnostic.Kind.DEPTH_CAP));
    }

    @Test
    void linkCycleTerminatesWithDiagnostic() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "man");
        p.root().layers.add(l);
        Node a = l.figure.node("armL1"), b = l.figure.node("armR1");
        a.rotation.addModifier(new Modifier.Link(b.rotation.id, 1, 0));
        b.rotation.addModifier(new Modifier.Link(a.rotation.id, 1, 0));
        p.invalidateIndex();
        Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
        assertTrue(s.has(Diagnostic.Kind.CYCLE));
        assertEquals(10, shapes(s.root).size());
    }

    @Test
    void delayFollowReadsSourceEarlier() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "man");
        p.root().layers.add(l);
        Node lead = l.figure.node("armL1"), follow = l.figure.node("armR1");
        lead.rotation.track().put(new Key(0, 0f, Interp.LINEAR));
        lead.rotation.track().put(new Key(1, 100f, Interp.LINEAR));
        follow.rotation.addModifier(new Modifier.DelayFollow(lead.rotation.id, 0.25, 1, 5));
        p.invalidateIndex();
        PropertyResolver r = new PropertyResolver(p, new ArrayList<Diagnostic>());
        assertEquals(30f, r.f(follow.rotation, 0.5), 1e-4);  // lead(0.25) = 25, +5
    }

    @Test
    void sineAndWiggleAreDeterministic() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "man");
        Node n = l.figure.node("head");
        n.rotation.addModifier(new Modifier.Sine(1, 10, 0));
        n.rotation.addModifier(new Modifier.Wiggle(3, 5, 42, 2));
        PropertyResolver r1 = new PropertyResolver(p, new ArrayList<Diagnostic>());
        PropertyResolver r2 = new PropertyResolver(p, new ArrayList<Diagnostic>());
        assertEquals(r1.f(n.rotation, 0.37), r2.f(n.rotation, 0.37));
        float v = r1.f(n.rotation, 0.25);
        assertTrue(Math.abs(v - 10) <= 5.0001, "sine peak 10 plus wiggle within 5: " + v);
    }

    @Test
    void loopModifierRepeatsKeys() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "man");
        Node n = l.figure.node("head");
        n.rotation.track().put(new Key(0, 0f, Interp.LINEAR));
        n.rotation.track().put(new Key(1, 10f, Interp.LINEAR));
        n.rotation.addModifier(new Modifier.Loop(Modifier.Loop.Mode.CYCLE, 0, 1));
        PropertyResolver r = new PropertyResolver(p, new ArrayList<Diagnostic>());
        assertEquals(2.5f, r.f(n.rotation, 3.25), 1e-4);
    }

    @Test
    void missingSymbolIsDiagnosedNotThrown() {
        Project p = Fixtures.empty();
        p.root().layers.add(new Layer.SymbolLayer(p.newId("L"), "ghost", "nope"));
        assertTrue(Evaluator.evaluate(p, p.rootSymbol, 0).has(Diagnostic.Kind.MISSING_REF));
        assertTrue(Evaluator.evaluate(p, "nope", 0).has(Diagnostic.Kind.MISSING_REF));
    }

    @Test
    void evaluationDoesNotMutateTheModel() {
        Project p = Fixtures.empty();
        Layer.FigureLayer l = Fixtures.stickman(p, "man");
        p.root().layers.add(l);
        Object before = l.figure.node("head").rotation.value();
        Evaluator.evaluate(p, p.rootSymbol, 1.5);
        assertSame(before, l.figure.node("head").rotation.value());
        assertEquals(0, l.figure.node("head").rotation.track().size());
    }
}
