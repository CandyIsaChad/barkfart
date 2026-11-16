package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.HashMap;
import java.util.Map;
import org.junit.jupiter.api.Test;

class FrameOpsTest {
    private Project p;
    private Layer.FigureLayer man;
    private Document d;
    private Editor e;
    private FrameOps f;

    private void setUp() {
        p = Fixtures.empty();            // 2.0 s at 30 fps = 60 frames
        p.settings.fps = 10;             // -> 20 frames
        man = Fixtures.stickman(p, "man");
        p.root().layers.add(man);
        p.invalidateIndex();
        d = new Document(p);
        e = new Editor(d);
        f = new FrameOps(e);
    }

    private void poseArm(int frame, float deg) {
        Map<String, Float> rot = new HashMap<String, Float>();
        rot.put("armR1", deg);
        e.pose(man, rot, null, f.time(frame));
    }

    @Test
    void insertShiftsLaterKeysAndCopiesThePose() {
        setUp();
        poseArm(0, 10);
        poseArm(2, 50);
        String before = ProjectCodec.encode(p);
        f.insertAfter(0, true);
        assertEquals(21, f.frameCount());
        Node arm = man.figure.node("armR1");
        assertEquals(3, arm.rotation.track().size());
        assertEquals(10f, (Float) arm.rotation.track().get(1).value);   // copied to frame 1
        assertEquals(0.3, arm.rotation.track().get(2).time, 1e-9);       // frame 2 -> 3
        assertTrue(d.undo());
        assertEquals(before, ProjectCodec.encode(p));
    }

    @Test
    void deleteRemovesKeysAndPullsLaterOnesBack() {
        setUp();
        poseArm(0, 10);
        poseArm(1, 30);
        poseArm(3, 70);
        assertTrue(f.delete(1));
        Node arm = man.figure.node("armR1");
        assertEquals(2, arm.rotation.track().size());
        assertEquals(0.2, arm.rotation.track().get(1).time, 1e-9);
        assertEquals(19, f.frameCount());
        assertTrue(f.hasKeys(2));
        assertFalse(f.hasKeys(1));
    }

    @Test
    void tweenToggleSetsInterpolationOnTheFrame() {
        setUp();
        poseArm(0, 10);
        poseArm(4, 90);
        f.setTween(0, false);
        assertEquals(Interp.HOLD, man.figure.node("armR1").rotation.track().get(0).interp);
        f.setTween(0, true);
        assertEquals(Interp.LINEAR, man.figure.node("armR1").rotation.track().get(0).interp);
    }

    @Test
    void cannotDeleteTheOnlyFrame() {
        setUp();
        p.root().duration = 0.1;
        assertEquals(1, f.frameCount());
        assertFalse(f.delete(0));
    }
}
