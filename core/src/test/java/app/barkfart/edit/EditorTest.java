package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import org.junit.jupiter.api.Test;

class EditorTest {
    private static String state(Document d) {
        return ProjectCodec.encode(d.project);
    }

    private static Layer.FigureLayer man(Document d) {
        return (Layer.FigureLayer) d.project.root().layers.get(0);
    }

    private static Document doc() {
        Project p = Fixtures.empty();
        p.root().layers.add(Fixtures.stickman(p, "man"));
        p.invalidateIndex();
        return new Document(p);
    }

    @Test
    void autoKeyKeysOnlyTheChangedProperty() {
        Document d = doc();
        Editor e = new Editor(d);
        Node arm = man(d).figure.node("armR1");
        assertEquals(Editor.Result.CHANGED, e.change(arm.rotation, 45f, 0.5));
        assertEquals(1, arm.rotation.track().size());
        for (Node n : man(d).figure.nodes()) {
            if (n != arm) assertEquals(0, n.rotation.track().size(), n.id);
            assertEquals(0, n.length.track().size());
        }
    }

    @Test
    void autoKeyOffEditsStaticValueOrRefuses() {
        Document d = doc();
        Editor e = new Editor(d);
        e.autoKey = false;
        Node arm = man(d).figure.node("armR1");
        assertEquals(Editor.Result.CHANGED, e.change(arm.rotation, 10f, 0));
        assertEquals(10f, arm.rotation.floatValue());
        arm.rotation.track().put(new Key(0, 1f, Interp.LINEAR));
        assertEquals(Editor.Result.REFUSED_NEEDS_AUTOKEY, e.change(arm.rotation, 20f, 0));
    }

    @Test
    void undoRedoRestoreExactState() {
        Document d = doc();
        Editor e = new Editor(d);
        String s0 = state(d);
        e.change(man(d).figure.node("armR1").rotation, 45f, 0.5);
        String s1 = state(d);
        e.change(man(d).figure.node("head").color, app.barkfart.model.Color.rgba8888(0xff0000ff), 1.0);
        String s2 = state(d);
        assertTrue(d.undo());
        assertEquals(s1, state(d));
        assertTrue(d.undo());
        assertEquals(s0, state(d));
        assertFalse(d.undo());
        assertTrue(d.redo());
        assertTrue(d.redo());
        assertEquals(s2, state(d));
        assertFalse(d.redo());
    }

    @Test
    void gestureIsOneUndoStepAndCancelReverts() {
        Document d = doc();
        Editor e = new Editor(d);
        String s0 = state(d);
        Node arm = man(d).figure.node("armR1");
        d.begin("Drag");
        for (int i = 0; i < 20; i++) e.change(arm.rotation, (float) i, 0);
        d.commit();
        assertEquals(19f, (Float) arm.rotation.track().get(0).value);
        assertTrue(d.undo());
        assertEquals(s0, state(d));
        d.begin("Drag");
        e.change(arm.rotation, 99f, 0);
        d.cancel();
        assertEquals(s0, state(d));
        assertFalse(d.canUndo());
    }

    @Test
    void newEditClearsRedoAndHistoryIsCapped() {
        Project p = Fixtures.empty();
        p.root().layers.add(Fixtures.stickman(p, "man"));
        Document d = new Document(p, 3);
        Editor e = new Editor(d);
        Node arm = man(d).figure.node("armR1");
        for (int i = 0; i < 5; i++) e.change(arm.rotation, (float) i, i);
        int undos = 0;
        while (d.undo()) undos++;
        assertEquals(3, undos);
        d.redo();
        e.change(arm.rotation, 7f, 9);
        assertFalse(d.canRedo());
    }

    @Test
    void posingKeysEveryNodeInOneStep() {
        Document d = doc();
        Editor e = new Editor(d);
        Map<String, Float> rot = new HashMap<String, Float>();
        rot.put("armR1", 10f);
        e.pose(man(d), rot, null, 0.5 + 0.01);   // snaps to the frame grid at 30 fps
        for (Node n : man(d).figure.nodes()) {
            assertEquals(1, n.rotation.track().size(), n.id);
            assertEquals(1, n.length.track().size(), n.id);
            assertEquals(0.5, n.rotation.track().get(0).time, 1e-9);
        }
        assertEquals(10f, (Float) man(d).figure.node("armR1").rotation.track().get(0).value);
        assertEquals(-110f, (Float) man(d).figure.node("legL1").rotation.track().get(0).value);
        assertTrue(d.undo());
        assertEquals(0, man(d).figure.node("armR1").rotation.track().size());
    }

    @Test
    void posingDoesNotBakeModifiers() {
        Document d = doc();
        Editor e = new Editor(d);
        Node head = man(d).figure.node("head");
        head.rotation.addModifier(new Modifier.Sine(1, 30, 0));
        e.pose(man(d), null, null, 0.25);
        assertEquals(0f, (Float) head.rotation.track().get(0).value, 1e-6);
    }

    private static List<KeyRef> keysOf(app.barkfart.model.Property p) {
        List<KeyRef> out = new ArrayList<KeyRef>();
        for (Key k : p.track().keys()) out.add(new KeyRef(p, k.time));
        return out;
    }

    @Test
    void retimeAndScaleAndCollisions() {
        Document d = doc();
        Editor e = new Editor(d);
        app.barkfart.model.Property p = man(d).figure.node("armR1").rotation;
        for (int i = 0; i < 4; i++) p.track().put(new Key(i, (float) i, Interp.LINEAR));
        e.retime(Arrays.asList(new KeyRef(p, 1), new KeyRef(p, 2)), 0.5);
        assertEquals(Arrays.asList(0.0, 1.5, 2.5, 3.0), times(p));
        d.undo();
        assertEquals(Arrays.asList(0.0, 1.0, 2.0, 3.0), times(p));
        e.scaleTime(keysOf(p), 0, 2);
        assertEquals(Arrays.asList(0.0, 2.0, 4.0, 6.0), times(p));
        d.undo();
        // Moving key 1 onto key 2 replaces it; undo brings both back.
        e.retime(Arrays.asList(new KeyRef(p, 1)), 1);
        assertEquals(Arrays.asList(0.0, 2.0, 3.0), times(p));
        assertEquals(1f, (Float) p.track().get(1).value);
        d.undo();
        assertEquals(4, p.track().size());
    }

    private static List<Double> times(app.barkfart.model.Property p) {
        List<Double> out = new ArrayList<Double>();
        for (Key k : p.track().keys()) out.add(k.time);
        return out;
    }

    @Test
    void staggerOffsetsChainByFrames() {
        Document d = doc();
        Editor e = new Editor(d);
        String[] chain = {"armR1", "armR2"};
        List<List<KeyRef>> sel = new ArrayList<List<KeyRef>>();
        for (String id : chain) {
            app.barkfart.model.Property p = man(d).figure.node(id).rotation;
            p.track().put(new Key(0, 0f, Interp.LINEAR));
            p.track().put(new Key(1, 90f, Interp.LINEAR));
            sel.add(keysOf(p));
        }
        e.stagger(sel, 3);
        assertEquals(Arrays.asList(0.0, 1.0), times(man(d).figure.node("armR1").rotation));
        assertEquals(Arrays.asList(0.1, 1.1), times(man(d).figure.node("armR2").rotation), "3 frames at 30 fps");
        assertTrue(d.undo());
        assertEquals(Arrays.asList(0.0, 1.0), times(man(d).figure.node("armR2").rotation));
    }

    @Test
    void copyPasteOntoAnotherFigure() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a"), b = Fixtures.stickman(p, "b");
        p.root().layers.add(a);
        p.root().layers.add(b);
        p.invalidateIndex();
        Document d = new Document(p);
        Editor e = new Editor(d);
        a.figure.node("armR1").rotation.track().put(new Key(1, 10f, Interp.LINEAR));
        a.figure.node("armR1").rotation.track().put(new Key(2, 20f, Interp.LINEAR));
        Editor.Clip c = e.copy(keysOf(a.figure.node("armR1").rotation));
        Map<String, String> map = new HashMap<String, String>();
        map.put(a.figure.node("armR1").rotation.id, b.figure.node("armR1").rotation.id);
        assertEquals(2, e.paste(c, 5, map));
        assertEquals(Arrays.asList(5.0, 6.0), times(b.figure.node("armR1").rotation));
        d.undo();
        assertEquals(0, b.figure.node("armR1").rotation.track().size());
        // Type mismatch is skipped, not thrown.
        map.put(a.figure.node("armR1").rotation.id, b.figure.node("armR1").color.id);
        assertEquals(0, e.paste(c, 5, map));
    }

    @Test
    void easingPresetsApplyToSelection() {
        Document d = doc();
        Editor e = new Editor(d);
        app.barkfart.model.Property p = man(d).figure.node("armR1").rotation;
        p.track().put(new Key(0, 0f, Interp.LINEAR));
        p.track().put(new Key(1, 10f, Interp.LINEAR));
        e.setInterp(Arrays.asList(new KeyRef(p, 0)), Interp.BEZIER, Ease.EASE_IN);
        assertEquals(Interp.BEZIER, p.track().get(0).interp);
        assertEquals(Ease.EASE_IN, p.track().get(0).ease);
        d.undo();
        assertEquals(Interp.LINEAR, p.track().get(0).interp);
    }

    /** Random edit sequences: undoing everything returns the exact starting state. */
    @Test
    void randomEditSequencesUndoToStart() {
        Random r = new Random(3);
        for (int trial = 0; trial < 50; trial++) {
            Document d = doc();
            Editor e = new Editor(d);
            String start = state(d);
            List<Node> nodes = man(d).figure.nodes();
            int steps = 1 + r.nextInt(30);
            for (int s = 0; s < steps; s++) {
                Node n = nodes.get(r.nextInt(nodes.size()));
                switch (r.nextInt(5)) {
                    case 0: e.change(n.rotation, (float) r.nextGaussian() * 90, r.nextInt(30) / 30.0); break;
                    case 1: e.pose(man(d), null, null, r.nextInt(30) / 30.0); break;
                    case 2: e.retime(keysOf(n.rotation), r.nextInt(7) / 30.0 - 0.1); break;
                    case 3: e.delete(keysOf(n.rotation)); break;
                    default: e.scaleTime(keysOf(n.rotation), 0.5, 0.5 + r.nextDouble());
                }
            }
            while (d.undo()) { }
            assertEquals(start, state(d), "trial " + trial);
        }
    }
}
