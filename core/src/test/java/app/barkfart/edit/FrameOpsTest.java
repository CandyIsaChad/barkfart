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

    @Test
    void keyingTheTweenedPoseChangesNothingAndHoldFreezesAFrame() {
        app.barkfart.model.Project p = app.barkfart.Fixtures.empty();
        p.root().duration = 1;
        app.barkfart.model.Layer.FigureLayer man = app.barkfart.Fixtures.stickman(p, "man");
        app.barkfart.model.Property arm = man.figure.node("armR1").rotation;
        arm.track().put(new app.barkfart.model.Key(0, -80f, app.barkfart.model.Interp.LINEAR));
        arm.track().put(new app.barkfart.model.Key(1, 60f, app.barkfart.model.Interp.LINEAR));
        p.root().layers.add(man);
        p.invalidateIndex();
        Document d = new Document(p);
        Editor ed = new Editor(d);
        FrameOps ops = new FrameOps(ed);
        java.util.List<String> before = SymbolOpsTest.frame(p, 0.37);
        assertEquals(1, ops.keyPoseBetween(null, 0, 1, 0.25));
        assertTrue(arm.track().indexOf(0.25) >= 0, "a key at 25%");
        assertEquals(-45f, (Float) arm.keyValue(0.25), 1e-4);
        SymbolOpsTest.sameFrame(before, SymbolOpsTest.frame(p, 0.37), "nothing moves");
        d.undo();
        assertTrue(arm.track().indexOf(0.25) < 0);
        // hold frame 6 (0.2 s) for 4 frames at 30 fps
        double fps = ed.fps(), t = 6 / fps;
        float at = (Float) arm.keyValue(t), later = (Float) arm.keyValue(0.6);
        ops.hold(6, 4);
        for (int k = 0; k <= 4; k++) assertEquals(at, (Float) arm.keyValue(t + k / fps), 1e-4, "held at frame " + (6 + k));
        assertEquals(later, (Float) arm.keyValue(0.6 + 4 / fps), 1e-4, "the rest moved back 4 frames");
        assertEquals(1 + 4 / fps, p.root().duration, 1e-9);
    }

    @Test
    void exportAsksForExactlyTheRangesFrames() throws Exception {
        final java.util.List<Double> asked = new java.util.ArrayList<Double>();
        java.io.File out = java.io.File.createTempFile("range", ".gif");
        app.barkfart.export.ExportJob job = new app.barkfart.export.ExportJob(app.barkfart.export.ExportJob.Format.GIF, out, 4, 4, 5, 3, 10,
                false, new app.barkfart.export.ExportJob.Frames() {
                    public byte[] grab(double t) {
                        asked.add(t);
                        return new byte[4 * 4 * 4];
                    }

                    public void dispose() {
                    }
                }, null);
        while (job.state() == app.barkfart.export.ExportJob.State.RUNNING) job.step();
        assertEquals(java.util.Arrays.asList(0.5, 0.6, 0.7), asked);
        out.delete();
    }
}
