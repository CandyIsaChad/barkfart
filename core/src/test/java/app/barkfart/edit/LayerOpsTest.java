package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.ValueType;
import org.junit.jupiter.api.Test;

class LayerOpsTest {
    private Project p;
    private Layer.FigureLayer man, hat, other;
    private Document d;

    private void setUp() {
        p = Fixtures.empty();
        man = Fixtures.stickman(p, "man");
        hat = Fixtures.stickman(p, "hat");
        other = Fixtures.stickman(p, "other");
        p.root().layers.add(other);
        p.root().layers.add(man);
        p.root().layers.add(hat);
        p.invalidateIndex();
        d = new Document(p);
        d.apply(JoinOps.join(p, p.root(), hat, man, "head", true, 0));
    }

    @Test
    void duplicatingWithJoinedFiguresKeepsTheJoins() throws Exception {
        setUp();
        man.figure.node("armR1").rotation.track().put(new Key(0, -30f, Interp.LINEAR));
        String before = ProjectCodec.encode(p);
        LayerOps.Made m = new LayerOps.Made();
        d.apply(LayerOps.duplicate(p, p.root(), man, true, 50, 0, m));
        assertEquals(5, p.root().layers.size());
        Layer manCopy = m.layers.get(0), hatCopy = m.layers.get(1);
        assertEquals(manCopy.id, hatCopy.parent, "the copied hat is joined to the copied man");
        assertEquals("head", hatCopy.parentNode);
        double dx = Evaluator.layerStage(p, p.root(), hatCopy, 0).tx - Evaluator.layerStage(p, p.root(), hat, 0).tx;
        assertEquals(50, dx, 1e-3, "copy moved with its joined hat");
        assertEquals(1, ((Layer.FigureLayer) manCopy).figure.node("armR1").rotation.track().size(), "keys copied");
        assertNotSame(man.figure.node("armR1").rotation, ((Layer.FigureLayer) manCopy).figure.node("armR1").rotation);
        String after = ProjectCodec.encode(p);
        assertEquals(after, ProjectCodec.encode(ProjectCodec.decode(after)), "copies save and load (ids unique)");
        d.undo();
        assertEquals(before.replaceAll("\"idCounter\": \\d+", ""), ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
    }

    @Test
    void pushingBackMovesJoinedFiguresAlong() {
        setUp();
        d.apply(LayerOps.move(p, p.root(), man, false));
        assertEquals(java.util.Arrays.<Layer>asList(man, hat, other), p.root().layers, "man and his hat moved behind the other figure together");
        d.apply(LayerOps.move(p, p.root(), man, true));
        assertEquals(java.util.Arrays.<Layer>asList(other, man, hat), p.root().layers);
    }

    @Test
    void copyingAndPastingAllProperties() {
        setUp();
        other.transform.x.setValue(77f);
        other.transform.rotation.setValue(25f);
        other.transform.scaleX.setValue(2f);
        other.opacity.setValue(0.5f);
        other.blend = Layer.Blend.ADD;
        EffectNode g = new EffectNode(other.id, "FXg", "glow");
        g.param(other.id, "radius", ValueType.FLOAT, 12f);
        other.effects.append(g);
        LayerOps.Props c = LayerOps.copyProps(p, p.root(), other, 0);
        Editor ed = new Editor(d);
        ed.autoKey = true;
        String before = ProjectCodec.encode(p);
        LayerOps.pasteProps(ed, p.root(), man, c, false, false, 0);
        app.barkfart.eval.PropertyResolver r = new app.barkfart.eval.PropertyResolver(p, new java.util.ArrayList<app.barkfart.eval.Diagnostic>());
        assertEquals(25f, r.f(man.transform.rotation, 0), 1e-4);
        assertEquals(2f, r.f(man.transform.scaleX, 0), 1e-4);
        assertEquals(0.5f, r.f(man.opacity, 0), 1e-4);
        assertEquals(Layer.Blend.ADD, man.blend);
        assertEquals(1, man.effects.chain().size());
        assertEquals(12f, (Float) man.effects.chain().get(0).params.get("radius").value(), 1e-6);
        assertNotEquals(other.transform.x.floatValue(), r.f(man.transform.x, 0), "position not pasted unless asked");
        d.undo();
        assertEquals(before.replaceAll("\"idCounter\": \\d+", ""), ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
    }
}
