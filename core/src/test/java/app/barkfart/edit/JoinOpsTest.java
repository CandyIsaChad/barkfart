package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Affine;
import app.barkfart.eval.Evaluator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class JoinOpsTest {
    private Project p;
    private Layer.FigureLayer man, hat;
    private Document d;

    private void setUp() {
        p = Fixtures.empty();
        man = Fixtures.stickman(p, "man");
        man.transform.x.setValue(-50f);
        hat = Fixtures.stickman(p, "hat");
        hat.transform.x.setValue(120f);
        hat.transform.y.setValue(30f);
        hat.transform.scaleX.setValue(0.5f);
        hat.transform.scaleY.setValue(0.5f);
        p.root().layers.add(man);
        p.root().layers.add(hat);
        // the man waves: the hand moves over time
        man.figure.node("armR1").rotation.track().put(new Key(0, -60f, Interp.LINEAR));
        man.figure.node("armR1").rotation.track().put(new Key(1, 40f, Interp.LINEAR));
        p.invalidateIndex();
        d = new Document(p);
    }

    /** Where the hat's origin is on the stage at t. */
    private double[] hatAt(double t) {
        Affine a = Evaluator.layerStage(p, p.root(), hat, t);
        return new double[] {a.mapX(0, 0), a.mapY(0, 0)};
    }

    @Test
    void joiningKeepsTheFigureInPlaceThenFollowsTheJoint() throws Exception {
        setUp();
        double[] before = hatAt(0);
        String start = ProjectCodec.encode(p);
        d.apply(JoinOps.join(p, p.root(), hat, man, "armR2", true, 0));
        assertArrayEquals(before, hatAt(0), 1e-3, "no jump when joining");
        double[] later = hatAt(1);
        assertTrue(Math.hypot(later[0] - before[0], later[1] - before[1]) > 10, "the hat moves with the waving hand");
        // the hat keeps its distance to the hand at every time
        for (double t : new double[] {0, 0.3, 0.7, 1}) {
            Affine hand = Evaluator.layerStage(p, p.root(), man, t);
            app.barkfart.eval.FigurePose ps = app.barkfart.eval.FigurePose.compute(man, t, new app.barkfart.eval.PropertyResolver(p, new java.util.ArrayList<app.barkfart.eval.Diagnostic>()));
            int i = man.figure.indexOf("armR2");
            double hx = hand.mapX(ps.x[i], ps.y[i]), hy = hand.mapY(ps.x[i], ps.y[i]);
            double[] h = hatAt(t);
            double[] h0 = hatAt(0);
            Affine hand0 = Evaluator.layerStage(p, p.root(), man, 0);
            app.barkfart.eval.FigurePose p0 = app.barkfart.eval.FigurePose.compute(man, 0, new app.barkfart.eval.PropertyResolver(p, new java.util.ArrayList<app.barkfart.eval.Diagnostic>()));
            double dist0 = Math.hypot(h0[0] - hand0.mapX(p0.x[i], p0.y[i]), h0[1] - hand0.mapY(p0.x[i], p0.y[i]));
            assertEquals(dist0, Math.hypot(h[0] - hx, h[1] - hy), 1e-3, "constant distance at " + t);
        }
        String joined = ProjectCodec.encode(p);
        assertTrue(joined.contains("\"parentNode\""));
        assertEquals(joined, ProjectCodec.encode(ProjectCodec.decode(joined)), "joins save and load");
        double[] at1 = hatAt(1);
        d.apply(JoinOps.unjoin(p, p.root(), hat, 1));
        assertArrayEquals(at1, hatAt(1), 1e-3, "no jump when unjoining");
        d.undo();
        d.undo();
        assertEquals(start, ProjectCodec.encode(p), "undo restores exactly");
    }

    @Test
    void joinsKeepKeyedMotionAndRefuseCycles() throws Exception {
        setUp();
        hat.transform.x.track().put(new Key(0, 120f, Interp.LINEAR));
        hat.transform.x.track().put(new Key(1, 200f, Interp.LINEAR));
        double[] a0 = hatAt(0), a1 = hatAt(1);
        d.apply(JoinOps.join(p, p.root(), hat, man, "head", true, 0));
        assertArrayEquals(a0, hatAt(0), 1e-3);
        assertArrayEquals(a1, hatAt(1), 1e-3, "keyed motion still lands on its keys");
        assertThrows(IllegalArgumentException.class, () -> JoinOps.join(p, p.root(), man, hat, null, true, 0));
        assertThrows(IllegalArgumentException.class, () -> JoinOps.join(p, p.root(), man, man, null, true, 0));
    }
}
