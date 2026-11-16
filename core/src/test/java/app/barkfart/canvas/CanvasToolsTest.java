package app.barkfart.canvas;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class CanvasToolsTest {
    private Project p;
    private Layer.FigureLayer man;
    private Document d;
    private CanvasTools tools;

    private void setUp(float zoom, float camRot) {
        p = Fixtures.empty();
        man = Fixtures.stickman(p, "man");
        man.transform.x.setValue(30f);
        p.root().layers.add(man);
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.zoom.setValue(zoom);
        cam.rotation.setValue(camRot);
        p.root().layers.add(cam);
        p.invalidateIndex();
        d = new Document(p);
        tools = new CanvasTools(new Editor(d));
        tools.throughCamera = true;   // these tests exercise the camera math path
    }

    private CanvasTools.Handle handle(String id) {
        for (CanvasTools.Handle h : tools.handles(0)) if (h.node.id.equals(id)) return h;
        throw new AssertionError(id);
    }

    @Test
    void pickFindsNearestJointWithinRadius() {
        setUp(1, 0);
        CanvasTools.Handle head = handle("head");
        CanvasTools.Hit hit = tools.pick(head.x + 3, head.y - 2, 10, 0);
        assertNotNull(hit);
        assertEquals("head", hit.node.id);
        assertNull(tools.pick(head.x + 500, head.y, 10, 0));
    }

    @Test
    void draggingAJointPointsItAtTheFingerThroughARotatedZoomedCamera() {
        setUp(2, 30);
        CanvasTools.Handle arm = handle("armR2");
        CanvasTools.Handle elbow = handle("armR1");
        tools.begin(tools.pick(arm.x, arm.y, 5, 0), arm.x, arm.y, 0);
        // Drag the hand to a point straight "up" from the elbow on screen.
        float tx = elbow.x, ty = elbow.y + 60;
        tools.move(tx, ty, 0);
        tools.end();
        CanvasTools.Handle after = handle("armR2");
        double dirX = after.x - elbow.x, dirY = after.y - elbow.y;
        assertEquals(90, Math.toDegrees(Math.atan2(dirY, dirX)), 1e-3, "hand now points up from the elbow on screen");
        assertEquals(45 * 2, Math.hypot(dirX, dirY), 1e-3, "length unchanged (45 units at zoom 2)");
    }

    @Test
    void stretchModeChangesLength() {
        setUp(1, 0);
        tools.stretch = true;
        CanvasTools.Handle arm = handle("armR2");
        CanvasTools.Handle elbow = handle("armR1");
        tools.begin(tools.pick(arm.x, arm.y, 5, 0), arm.x, arm.y, 0);
        tools.move(elbow.x + 100, elbow.y, 0);
        tools.end();
        assertEquals(100f, (Float) man.figure.node("armR2").length.track().get(0).value, 1e-3);
    }

    @Test
    void draggingTheRootMovesTheFigureAndOneUndoRestoresAll() {
        setUp(2, 0);
        String before = ProjectCodec.encode(p);
        CanvasTools.Handle hip = handle("hip");
        tools.begin(tools.pick(hip.x, hip.y, 5, 0), hip.x, hip.y, 0);
        for (int i = 1; i <= 10; i++) tools.move(hip.x + i * 10, hip.y, 0);   // 100 view px at zoom 2
        tools.end();
        assertEquals(30f + 50f, (Float) man.transform.x.track().get(0).value, 1e-3);
        assertTrue(d.undo());
        assertEquals(before, ProjectCodec.encode(p));
        assertFalse(d.canUndo());
    }

    @Test
    void rotationStaysContinuousAcrossTheMinus180Seam() {
        setUp(1, 0);
        Node arm = man.figure.node("armR1");
        CanvasTools.Handle elbow = handle("armR1"), shoulder = handle("torso");
        tools.begin(tools.pick(elbow.x, elbow.y, 5, 0), elbow.x, elbow.y, 0);
        // Sweep the arm all the way round; the angle must change smoothly, never jump by ~360.
        float prev = arm.rotation.floatValue();
        for (int k = 0; k <= 72; k++) {
            double a = Math.toRadians(Math.toDegrees(Math.atan2(elbow.y - shoulder.y, elbow.x - shoulder.x)) + k * 5);
            tools.move((float) (shoulder.x + 45 * Math.cos(a)), (float) (shoulder.y + 45 * Math.sin(a)), 0);
            float now = (Float) arm.rotation.track().get(0).value;
            assertTrue(Math.abs(now - prev) < 10, "jump from " + prev + " to " + now);
            prev = now;
        }
        tools.end();
    }

    @Test
    void jointScopeKeysLengthWhenRotatingSoStretchIsNeverUnkeyed() {
        setUp(1, 0);
        CanvasTools.Handle arm = handle("armR2");
        tools.begin(tools.pick(arm.x, arm.y, 5, 0), arm.x, arm.y, 0.5);
        tools.move(arm.x, arm.y + 10, 0.5);
        tools.end();
        Node n = man.figure.node("armR2");
        assertEquals(1, n.rotation.track().size());
        assertEquals(1, n.length.track().size(), "length pinned at the playhead");
        assertEquals(0, man.figure.node("armL2").rotation.track().size(), "other joints untouched");
    }

    @Test
    void figureScopeKeysTheWholePoseInOneUndoStep() {
        setUp(1, 0);
        tools.keyScope = CanvasTools.KeyScope.FIGURE;
        CanvasTools.Handle arm = handle("armR2");
        tools.begin(tools.pick(arm.x, arm.y, 5, 0), arm.x, arm.y, 0.5);
        tools.move(arm.x, arm.y + 10, 0.5);
        tools.end();
        for (Node n : man.figure.nodes()) {
            if (n.parent == null) continue;
            assertEquals(1, n.rotation.track().size(), n.id);
            assertEquals(1, n.length.track().size(), n.id);
        }
        assertEquals(1, man.transform.x.track().size());
        assertTrue(d.undo());
        assertEquals(0, man.figure.node("armL2").rotation.track().size());
    }

    @Test
    void editingViewIgnoresTheCameraByDefault() {
        setUp(2, 30);
        tools.throughCamera = false;
        CanvasTools.Handle hip = handle("hip");
        assertEquals(30, hip.x, 1e-4, "stage space: the hip sits at the layer's x, camera not applied");
        assertEquals(0, hip.y, 1e-4);
    }

    @Test
    void lockedLayersAreNotPickable() {
        setUp(1, 0);
        man.locked = true;
        assertTrue(tools.handles(0).isEmpty());
    }
}
