package app.barkfart.model;

import static org.junit.jupiter.api.Assertions.*;

import org.junit.jupiter.api.Test;

class TrackTest {
    private static Track floats(Interp interp) {
        Track t = new Track(ValueType.FLOAT);
        t.put(new Key(0, 0f, interp));
        t.put(new Key(1, 10f, interp));
        return t;
    }

    @Test
    void linearInterpolatesAndHoldsAtEnds() {
        Track t = floats(Interp.LINEAR);
        assertEquals(0f, (Float) t.sample(-5));
        assertEquals(2.5f, (Float) t.sample(0.25), 1e-5);
        assertEquals(10f, (Float) t.sample(1));
        assertEquals(10f, (Float) t.sample(7));
    }

    @Test
    void holdStepsAtNextKey() {
        Track t = floats(Interp.HOLD);
        assertEquals(0f, (Float) t.sample(0.999));
        assertEquals(10f, (Float) t.sample(1.0));
    }

    @Test
    void bezierUsesEase() {
        Track t = new Track(ValueType.FLOAT);
        t.put(new Key(0, 0f, Interp.BEZIER, Ease.EASE_IN));
        t.put(new Key(1, 10f, Interp.LINEAR));
        float mid = (Float) t.sample(0.5);
        assertTrue(mid < 5f && mid > 0f, "ease-in is below linear at the midpoint: " + mid);
    }

    @Test
    void putReplacesSameTimeAndKeepsOrder() {
        Track t = new Track(ValueType.FLOAT);
        t.put(new Key(2, 2f, Interp.LINEAR));
        t.put(new Key(0, 0f, Interp.LINEAR));
        t.put(new Key(1, 1f, Interp.LINEAR));
        assertNotNull(t.put(new Key(1, 5f, Interp.LINEAR)));
        assertEquals(3, t.size());
        assertEquals(0, t.get(0).time);
        assertEquals(5f, (Float) t.get(1).value);
        assertEquals(2, t.get(2).time);
    }

    @Test
    void boolTracksAlwaysHold() {
        Track t = new Track(ValueType.BOOL);
        t.put(new Key(0, true, Interp.LINEAR));
        t.put(new Key(1, false, Interp.LINEAR));
        assertEquals(Interp.HOLD, t.get(0).interp);
        assertEquals(true, t.sample(0.9));
    }

    @Test
    void rejectsWrongValueType() {
        Track t = new Track(ValueType.FLOAT);
        assertThrows(IllegalArgumentException.class, () -> t.put(new Key(0, 1.0, Interp.LINEAR)));
    }

    @Test
    void anglesInterpolateNumericallyWithoutWrapping() {
        Track t = new Track(ValueType.ANGLE);
        t.put(new Key(0, 350f, Interp.LINEAR));
        t.put(new Key(1, -363f, Interp.LINEAR));
        assertEquals(-6.5f, (Float) t.sample(0.5), 1e-4);
    }

    @Test
    void easeEndpointsAndMonotonic() {
        Ease e = Ease.EASE_IN_OUT;
        assertEquals(0f, e.apply(0));
        assertEquals(1f, e.apply(1));
        float prev = 0;
        for (int i = 1; i <= 100; i++) {
            float v = e.apply(i / 100f);
            assertTrue(v >= prev - 1e-5, "monotonic");
            prev = v;
        }
    }

    @Test
    void loopRemap() {
        Modifier.Loop c = new Modifier.Loop(Modifier.Loop.Mode.CYCLE, 0, 1);
        assertEquals(0.25, c.remap(2.25), 1e-9);
        Modifier.Loop pp = new Modifier.Loop(Modifier.Loop.Mode.PING_PONG, 0, 1);
        assertEquals(0.75, pp.remap(1.25), 1e-9);
        assertEquals(0.25, pp.remap(2.25), 1e-9);
    }
}
