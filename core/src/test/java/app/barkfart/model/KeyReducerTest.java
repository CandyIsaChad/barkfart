package app.barkfart.model;

import static org.junit.jupiter.api.Assertions.*;

import java.util.Random;
import org.junit.jupiter.api.Test;

class KeyReducerTest {
    @Test
    void linearRampCollapsesToEnds() {
        Track t = new Track(ValueType.FLOAT);
        for (int i = 0; i <= 30; i++) t.put(new Key(i / 30.0, i * 2f, Interp.LINEAR));
        assertEquals(29, KeyReducer.reduce(t, 0.01));
        assertEquals(2, t.size());
        assertEquals(30f, (Float) t.sample(0.5), 1e-4);
    }

    @Test
    void repeatedHoldValuesCollapse() {
        Track t = new Track(ValueType.BOOL);
        t.put(new Key(0, true, Interp.HOLD));
        t.put(new Key(1, true, Interp.HOLD));
        t.put(new Key(2, false, Interp.HOLD));
        t.put(new Key(3, false, Interp.HOLD));
        KeyReducer.reduce(t, 0);
        assertEquals(3, t.size());   // the step at 2 must survive, the duplicate at 1 goes
        assertEquals(true, t.sample(1.99));
        assertEquals(false, t.sample(2));
    }

    @Test
    void curvesKeepTheKeysTheyNeed() {
        Track t = new Track(ValueType.ANGLE);
        for (int i = 0; i <= 40; i++) t.put(new Key(i / 40.0, (float) (90 * Math.sin(i / 40.0 * Math.PI * 2)), Interp.LINEAR));
        int before = t.size();
        KeyReducer.reduce(t, 0.01);
        // Only the zero crossing at t = 0.5 goes: its neighbours are symmetric
        // (+14.08, 0, -14.08), so those three keys are collinear. Every other
        // sample bends the curve and must stay.
        assertEquals(before - 1, t.size());
        assertTrue(t.indexOf(0.5) < 0, "zero crossing removed");
        assertTrue(t.indexOf(0.25) >= 0, "peak kept");
    }

    /** Property check: after reduction every point stays within tolerance of the original. */
    @Test
    void randomTracksStayWithinTolerance() {
        Random r = new Random(7);
        for (int trial = 0; trial < 300; trial++) {
            Track t = new Track(ValueType.FLOAT);
            double time = 0;
            float v = 0;
            int n = 3 + r.nextInt(25);
            for (int i = 0; i < n; i++) {
                time += 0.01 + r.nextDouble();
                if (r.nextInt(3) > 0) v += (float) (r.nextGaussian() * 5);
                Interp in = Interp.values()[r.nextInt(3)];
                t.put(new Key(time, v, in, new Ease(r.nextFloat(), r.nextFloat() * 2 - 0.5f, r.nextFloat(), r.nextFloat() * 2 - 0.5f)));
            }
            Track orig = t.copy();
            KeyReducer.reduce(t, 0.01);
            assertEquals(orig.get(0).time, t.get(0).time);
            assertEquals(orig.get(orig.size() - 1).time, t.get(t.size() - 1).time);
            for (int s = 0; s <= 500; s++) {
                double at = -1 + (time + 2) * s / 500;
                assertEquals((Float) orig.sample(at), (Float) t.sample(at), 0.0101, "trial " + trial + " t=" + at);
            }
            for (Key k : orig.keys()) assertEquals((Float) orig.sample(k.time), (Float) t.sample(k.time), 0.0101);
        }
    }
}
