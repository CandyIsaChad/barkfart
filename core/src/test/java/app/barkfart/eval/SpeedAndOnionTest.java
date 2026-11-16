package app.barkfart.eval;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.OnionSkin;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Track;
import app.barkfart.model.ValueType;
import java.util.ArrayList;
import org.junit.jupiter.api.Test;

class SpeedAndOnionTest {
    private static Property speed() {
        return new Property("s", ValueType.DOUBLE, 1.0);
    }

    private static PropertyResolver r() {
        return new PropertyResolver(Fixtures.empty(), new ArrayList<Diagnostic>());
    }

    @Test
    void constantSpeedIsSpeedTimesTime() {
        Property s = speed();
        s.setValue(2.0);
        assertEquals(3.0, SpeedIntegral.elapsed(s, 1.5, r()), 1e-12);
    }

    @Test
    void linearRampToZeroNeverRunsBackwards() {
        Property s = speed();
        s.track().put(new Key(0, 1.0, Interp.LINEAR));
        s.track().put(new Key(1, 0.0, Interp.LINEAR));
        double prev = -1;
        for (int i = 0; i <= 40; i++) {
            double e = SpeedIntegral.elapsed(s, i / 20.0, r());
            assertTrue(e >= prev - 1e-12, "monotonic at " + i);
            prev = e;
        }
        // Exact up to the 1e-9 "just inside the segment" sampling.
        assertEquals(0.5, SpeedIntegral.elapsed(s, 1, r()), 1e-8);       // area of the ramp
        assertEquals(0.5, SpeedIntegral.elapsed(s, 2, r()), 1e-8);       // stopped after 1 s
    }

    @Test
    void holdStepsIntegrateExactly() {
        Property s = speed();
        s.track().put(new Key(0, 1.0, Interp.HOLD));
        s.track().put(new Key(1, 3.0, Interp.HOLD));
        assertEquals(1.0, SpeedIntegral.elapsed(s, 1, r()), 1e-9);
        assertEquals(1.0 + 3 * 0.5, SpeedIntegral.elapsed(s, 1.5, r()), 1e-9);
    }

    @Test
    void easedAndModifiedSpeedsUseNumericIntegration() {
        Property s = speed();
        s.track().put(new Key(0, 0.0, Interp.BEZIER, Ease.EASE_IN_OUT));
        s.track().put(new Key(2, 2.0, Interp.LINEAR));
        assertEquals(2.0, SpeedIntegral.elapsed(s, 2, r()), 1e-3);       // symmetric ease: area = average x time
        Property m = speed();
        m.addModifier(new Modifier.Sine(1, 0.5f, 0));
        assertEquals(1.0, SpeedIntegral.elapsed(m, 1, r()), 1e-3);        // a full sine period averages out
    }

    @Test
    void exactMatchesNumericOnRandomLinearTracks() {
        java.util.Random rnd = new java.util.Random(4);
        for (int trial = 0; trial < 100; trial++) {
            Track t = new Track(ValueType.DOUBLE);
            double time = -1;
            for (int k = 0; k < 2 + rnd.nextInt(6); k++) {
                time += 0.1 + rnd.nextDouble();
                t.put(new Key(time, rnd.nextDouble() * 3, rnd.nextBoolean() ? Interp.LINEAR : Interp.HOLD));
            }
            double at = rnd.nextDouble() * 6 - 1;
            // Fine numeric reference.
            int n = 200000;
            double h = at / n, sum = 0;
            for (int i = 0; i < n; i++) sum += (Double) t.sample((i + 0.5) * h) * h;
            assertEquals(sum, SpeedIntegral.exact(t, at), 1e-3, "trial " + trial);
        }
    }

    @Test
    void onionSkinGhostsAdjacentFramesAndSkipsExcludedLayers() {
        Project p = Fixtures.empty();
        Layer.FigureLayer man = Fixtures.stickman(p, "man"), bg = Fixtures.stickman(p, "bg");
        Node arm = man.figure.node("armR1");
        arm.rotation.track().put(new Key(0, 0f, Interp.LINEAR));
        arm.rotation.track().put(new Key(1, 180f, Interp.LINEAR));
        bg.onion = false;
        p.root().layers.add(bg);
        p.root().layers.add(man);
        OnionSkin o = new OnionSkin();
        o.before = 2;
        o.after = 1;
        DrawList ghosts = o.build(p, p.rootSymbol, 0.5);
        int perFigure = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0.5)).triangleCount() / 2;
        assertEquals(3 * perFigure, ghosts.triangleCount(), "3 ghosts of the man only");
        int c = ghosts.batches.get(0).rgba(0);
        assertTrue((c & 0xff) > 0 && (c & 0xff) < 255, "ghosts are translucent");
        assertTrue(ProjectCodec.encode(p).contains("\"onion\": false"));
    }
}
