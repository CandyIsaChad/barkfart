package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

class SymbolOpsTest {
    /** Every vertex the frame draws (x, y rounded to 1/100, colour), sorted. */
    static List<String> frame(Project p, double t) {
        p.invalidateIndex();
        DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, t));
        List<String> out = new ArrayList<String>();
        for (DrawList.Batch b : dl.batches) {
            for (int i = 0; i < b.vertexCount(); i++) out.add(Math.round(b.x(i) * 100) + "," + Math.round(b.y(i) * 100) + "," + Integer.toHexString(b.rgba(i)));
        }
        java.util.Collections.sort(out);
        return out;
    }

    static void sameFrame(List<String> a, List<String> b, String what) {
        assertEquals(a.size(), b.size(), what + ": vertex count");
        int diff = 0;
        for (int i = 0; i < a.size(); i++) if (!a.get(i).equals(b.get(i))) diff++;
        assertTrue(diff <= a.size() / 200, what + ": " + diff + " of " + a.size() + " vertices differ");
    }

    @Test
    void makingAMovieclipKeepsTheAnimation() throws Exception {
        Project p = Fixtures.empty();
        p.root().duration = 2;
        Layer.FigureLayer a = Fixtures.stickman(p, "a"), b = Fixtures.stickman(p, "b"), c = Fixtures.stickman(p, "c");
        a.transform.x.setValue(-120f);
        c.transform.x.setValue(120f);
        a.figure.node("armR1").rotation.track().put(new Key(0, -80f, Interp.LINEAR));
        a.figure.node("armR1").rotation.track().put(new Key(1.5, 60f, Interp.LINEAR));
        b.transform.y.track().put(new Key(0, 0f, Interp.LINEAR));
        b.transform.y.track().put(new Key(2, 50f, Interp.LINEAR));
        p.root().layers.addAll(Arrays.asList(a, b, c));
        double[] times = {0, 0.5, 1.2, 1.9};
        List<List<String>> before = new ArrayList<List<String>>();
        for (double t : times) before.add(frame(p, t));
        String text = ProjectCodec.encode(p);
        Document d = new Document(p);
        SymbolOps.Made m = new SymbolOps.Made();
        d.apply(SymbolOps.convertToSymbol(p, p.root(), Arrays.<Layer>asList(a, b), "Clip", m));
        assertEquals(2, p.root().layers.size(), "instance + the layer left out");
        assertSame(m.instance, p.root().layers.get(0), "instance where the first moved layer was");
        for (int i = 0; i < times.length; i++) sameFrame(before.get(i), frame(p, times[i]), "t=" + times[i]);
        String after = ProjectCodec.encode(p);
        assertEquals(after, ProjectCodec.encode(ProjectCodec.decode(after)));
        d.undo();
        assertEquals(text.replaceAll("\"idCounter\": \\d+", ""), ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
        d.redo();
        for (int i = 0; i < times.length; i++) sameFrame(before.get(i), frame(p, times[i]), "redo t=" + times[i]);
        // a join across the selection is refused
        c.parent = b.id;
        assertThrows(IllegalArgumentException.class, () -> SymbolOps.convertToSymbol(p, p.root(), Arrays.<Layer>asList(p.root().layers.get(1)), "X", null));
    }

    @Test
    void breakingApartKeepsTheCurrentFrame() {
        Project p = Fixtures.empty();
        p.root().duration = 2;
        Symbol inner = p.addSymbol(new Symbol(p.newId("S"), "inner", 1.0));
        Layer.FigureLayer man = Fixtures.stickman(p, "man");
        man.figure.node("armR1").rotation.track().put(new Key(0, -80f, Interp.LINEAR));
        man.figure.node("armR1").rotation.track().put(new Key(1, 70f, Interp.LINEAR));
        man.transform.x.setValue(20f);
        inner.layers.add(man);
        Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), "inst", inner.id);
        inst.transform.x.setValue(-60f);
        inst.transform.rotation.setValue(30f);
        inst.transform.scaleX.setValue(0.5f);
        inst.transform.scaleY.setValue(0.5f);
        inst.timeOffset.setValue(0.25);
        inst.opacity.setValue(0.8f);
        p.root().layers.add(inst);
        double t = 0.4;
        List<String> before = frame(p, t);
        String text = ProjectCodec.encode(p);
        Document d = new Document(p);
        SymbolOps.Made m = new SymbolOps.Made();
        d.apply(SymbolOps.breakApart(p, p.root(), inst, t, m));
        assertEquals(1, m.layers.size());
        assertFalse(p.root().layers.contains(inst));
        sameFrame(before, frame(p, t), "the frame it was broken at");
        d.undo();
        assertEquals(text.replaceAll("\"idCounter\": \\d+", ""), ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
    }
}
