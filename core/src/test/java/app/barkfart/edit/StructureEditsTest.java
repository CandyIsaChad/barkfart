package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Marker;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Polyfill;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import org.junit.jupiter.api.Test;

class StructureEditsTest {
    @Test
    void removeNodeTakesSubtreeAndPolyfillsAndUndoRestoresPositions() {
        Project p = Fixtures.kitchenSink();
        Document d = new Document(p);
        Layer.FigureLayer man = (Layer.FigureLayer) p.root().layers.get(0);
        String before = ProjectCodec.encode(p);
        int nodes = man.figure.nodes().size();
        d.apply(StructureEdits.removeNode(man, "armL1"));      // takes armL2 and the polyfill using both
        assertEquals(nodes - 2, man.figure.nodes().size());
        assertNull(man.figure.node("armL2"));
        assertTrue(man.figure.polyfills.isEmpty());
        d.undo();
        assertEquals(before, ProjectCodec.encode(p));
        d.redo();
        assertNull(man.figure.node("armL1"));
    }

    @Test
    void rootCannotBeRemovedAndParentCyclesAreRefused() {
        Project p = Fixtures.kitchenSink();
        Layer.FigureLayer man = (Layer.FigureLayer) p.root().layers.get(0);
        assertThrows(IllegalArgumentException.class, () -> StructureEdits.removeNode(man, "hip"));
        Layer inst = p.root().layers.get(1);   // parented to man
        assertEquals(man.id, inst.parent);
        assertThrows(IllegalArgumentException.class, () -> StructureEdits.setParent(p.root(), man, inst.id));
    }

    /** Random structural edits, all undone: exact original bytes; all redone: exact edited bytes. */
    @Test
    void randomStructuralSequencesUndoAndRedoExactly() {
        Random r = new Random(11);
        for (int trial = 0; trial < 60; trial++) {
            Project p = Fixtures.kitchenSink();
            Document d = new Document(p);
            String start = ProjectCodec.encode(p);
            int steps = 1 + r.nextInt(25);
            for (int s = 0; s < steps; s++) {
                try {
                    Command c = randomCommand(p, r);
                    if (c != null) d.apply(c);
                } catch (IllegalArgumentException | IllegalStateException refused) {
                    // refused edits (cycles, root removal, loop ordering) change nothing
                }
            }
            String end = state(p);
            while (d.undo()) { }
            assertEquals(state(start), state(p), "undo, trial " + trial);
            while (d.redo()) { }
            assertEquals(end, state(p), "redo, trial " + trial);
        }
    }

    /**
     * Saved text minus the id counter: new ids are drawn from a counter that
     * deliberately never rolls back, so undo/redo can never reuse an id.
     */
    private static String state(Project p) {
        return state(ProjectCodec.encode(p));
    }

    private static String state(String json) {
        return json.replaceFirst("\"idCounter\": \\d+", "\"idCounter\": _");
    }

    private static Command randomCommand(Project p, Random r) {
        Symbol s = p.root();
        List<Layer> layers = s.layers;
        Layer l = layers.get(r.nextInt(layers.size()));
        Layer.FigureLayer fig = null;
        for (Layer x : layers) if (x instanceof Layer.FigureLayer) fig = (Layer.FigureLayer) x;
        switch (r.nextInt(14)) {
            case 0: return StructureEdits.addLayer(s, Fixtures.stickman(p, "new"), r.nextInt(layers.size() + 1));
            case 1: return layers.size() > 1 ? StructureEdits.removeLayer(s, l) : null;
            case 2: return StructureEdits.moveLayer(s, l, r.nextInt(layers.size()));
            case 3: return StructureEdits.setFlag(l, StructureEdits.Flag.values()[r.nextInt(3)], r.nextBoolean());
            case 4: return StructureEdits.setBlend(l, Layer.Blend.values()[r.nextInt(4)]);
            case 5: return StructureEdits.setParent(s, l, r.nextBoolean() ? null : layers.get(r.nextInt(layers.size())).id);
            case 6: {
                if (fig == null) return null;
                List<Node> ns = fig.figure.nodes();
                Node parent = ns.get(r.nextInt(ns.size()));
                Node n = new Node(fig.id, p.newId("n"), parent.id, Node.Shape.LINE);
                n.length.setValue(20f);
                return StructureEdits.addNode(fig, n);
            }
            case 7: {
                if (fig == null) return null;
                List<Node> ns = fig.figure.nodes();
                return StructureEdits.removeNode(fig, ns.get(r.nextInt(ns.size())).id);
            }
            case 8: {
                if (fig == null) return null;
                List<Node> ns = fig.figure.nodes();
                Node n = ns.get(r.nextInt(ns.size()));
                return r.nextBoolean() ? StructureEdits.setShape(n, Node.Shape.values()[r.nextInt(Node.Shape.values().length)])
                        : StructureEdits.setDrawOrder(n, r.nextInt(30) - 5);
            }
            case 9: {
                if (fig == null) return null;
                if (!fig.figure.polyfills.isEmpty() && r.nextBoolean()) {
                    return StructureEdits.removePolyfill(fig, fig.figure.polyfills.get(r.nextInt(fig.figure.polyfills.size())));
                }
                List<Node> ns = fig.figure.nodes();
                Polyfill pf = new Polyfill(fig.id, p.newId("pf"));
                pf.anchor = ns.get(r.nextInt(ns.size())).id;
                for (int k = 0; k < 3; k++) pf.nodes.add(ns.get(r.nextInt(ns.size())).id);
                return StructureEdits.addPolyfill(fig, pf);
            }
            case 10: {
                List<Property> props = l.properties();
                Property prop = props.get(r.nextInt(props.size()));
                if (!prop.modifiers().isEmpty() && r.nextBoolean()) {
                    return r.nextBoolean() ? StructureEdits.removeModifier(prop, r.nextInt(prop.modifiers().size()))
                            : StructureEdits.setModifierEnabled(prop.modifiers().get(0), r.nextBoolean());
                }
                Modifier m = r.nextInt(4) == 0 ? new Modifier.Loop(Modifier.Loop.Mode.CYCLE, 0, 1) : new Modifier.Sine(1, 2, 0);
                return StructureEdits.addModifier(prop, m);
            }
            case 11: {
                if (!s.markers.isEmpty() && r.nextBoolean()) return StructureEdits.removeMarker(s, s.markers.get(0));
                return StructureEdits.addMarker(s, new Marker(r.nextInt(30) / 30.0, "m", Color.WHITE));
            }
            case 12: return StructureEdits.rename(l, "renamed" + r.nextInt(5));
            default: return r.nextBoolean() ? StructureEdits.setDuration(s, 0.5 + r.nextInt(8))
                    : StructureEdits.addSymbol(p, new Symbol(p.newId("S"), "extra", 1));
        }
    }

    @Test
    void addingAndRemovingEffectsIsUndoableAndKeepsTheChainLinked() {
        app.barkfart.model.Project p = app.barkfart.Fixtures.empty();
        app.barkfart.model.Layer.FigureLayer l = app.barkfart.Fixtures.stickman(p, "m");
        p.root().layers.add(l);
        p.invalidateIndex();
        Document d = new Document(p);
        String before = app.barkfart.io.ProjectCodec.encode(p);
        app.barkfart.model.EffectNode a = new app.barkfart.model.EffectNode(l.id, "FXa", "blur.gaussian");
        app.barkfart.model.EffectNode b = new app.barkfart.model.EffectNode(l.id, "FXb", "glow");
        app.barkfart.model.EffectNode c = new app.barkfart.model.EffectNode(l.id, "FXc", "invert");
        d.apply(StructureEdits.addEffect(l, a));
        d.apply(StructureEdits.addEffect(l, b));
        d.apply(StructureEdits.addEffect(l, c));
        assertEquals(java.util.Arrays.asList(a, b, c), l.effects.chain());
        String three = app.barkfart.io.ProjectCodec.encode(p);
        d.apply(StructureEdits.removeEffect(l, b));
        assertEquals(java.util.Arrays.asList(a, c), l.effects.chain());
        d.apply(StructureEdits.removeEffect(l, c));
        assertEquals(java.util.Arrays.asList(a), l.effects.chain());
        d.undo();
        d.undo();
        assertEquals(three, app.barkfart.io.ProjectCodec.encode(p));
        d.undo();
        d.undo();
        d.undo();
        assertEquals(before, app.barkfart.io.ProjectCodec.encode(p));
    }

    @Test
    void canvasSizeAndBarsAreUndoable() {
        app.barkfart.model.Project p = app.barkfart.Fixtures.empty();
        Document d = new Document(p);
        d.apply(StructureEdits.setCanvasSize(p, 720, 1280));
        d.apply(StructureEdits.setLetterbox(p, 2.39f));
        assertEquals(720, p.settings.width);
        assertEquals(2.39f, p.settings.letterbox, 1e-6);
        d.undo();
        d.undo();
        assertEquals(1280, p.settings.width);
        assertEquals(0f, p.settings.letterbox);
        assertThrows(IllegalArgumentException.class, () -> StructureEdits.setCanvasSize(p, 0, 100));
    }
}
