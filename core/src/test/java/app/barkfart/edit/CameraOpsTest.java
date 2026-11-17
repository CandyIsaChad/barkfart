package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Affine;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Group;
import app.barkfart.eval.Scene;
import app.barkfart.eval.Shape;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class CameraOpsTest {
    private Project p;
    private Layer.FigureLayer man, caption;
    private Layer.CameraLayer cam;
    private Document d;

    private void setUp() {
        p = Fixtures.empty();
        p.root().duration = 2;
        man = Fixtures.stickman(p, "man");
        man.transform.x.track().put(new Key(0, -200f, Interp.LINEAR));
        man.transform.x.track().put(new Key(2, 300f, Interp.LINEAR));
        man.transform.y.track().put(new Key(0, 0f, Interp.LINEAR));
        man.transform.y.track().put(new Key(1, 80f, Interp.LINEAR));
        man.transform.y.track().put(new Key(2, 0f, Interp.LINEAR));
        caption = Fixtures.stickman(p, "caption");
        caption.transform.x.setValue(400f);
        caption.transform.y.setValue(250f);
        cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.x.track().put(new Key(0, 0f, Interp.LINEAR));
        cam.x.track().put(new Key(2, 150f, Interp.LINEAR));
        cam.zoom.setValue(1.5f);
        p.root().layers.add(man);
        p.root().layers.add(caption);
        p.root().layers.add(cam);
        p.invalidateIndex();
        d = new Document(p);
    }

    /** Where a layer's origin lands in the shot (camera applied). */
    private double[] inShot(Layer l, double t) {
        Affine a = Evaluator.layerView(p, p.root(), l, t);
        return new double[] {a.mapX(0, 0), a.mapY(0, 0)};
    }

    private double[] onStage(Layer l, double t) {
        Affine a = Evaluator.layerStage(p, p.root(), l, t);
        return new double[] {a.mapX(0, 0), a.mapY(0, 0)};
    }

    @Test
    void aLayerLockedToTheCameraStaysPutInTheShot() {
        setUp();
        double[] stage = onStage(caption, 0);
        String before = ProjectCodec.encode(p);
        d.apply(CameraOps.lock(p, p.root(), caption, cam, true, 0));
        assertArrayEquals(stage, onStage(caption, 0), 1e-3, "no jump when locking");
        double[] shot0 = inShot(caption, 0);
        for (double t : new double[] {0.5, 1, 2}) assertArrayEquals(shot0, inShot(caption, t), 1e-3, "fixed in the shot at " + t);
        assertNotEquals(onStage(caption, 0)[0], onStage(caption, 2)[0], 1e-3, "on the stage it travels with the camera frame");
        d.undo();
        assertEquals(before, ProjectCodec.encode(p));
    }

    @Test
    void autoCameraFollowsAFigureWithFewKeys() {
        setUp();
        d.apply(CameraOps.follow(p, p.root(), cam, man, 0, 60));
        double[] start = inShot(man, 0);
        for (int f = 0; f <= 60; f += 7) {
            double[] s = inShot(man, f / 30.0);
            assertEquals(start[0], s[0], 3, "x held in the shot at frame " + f);
            assertEquals(start[1], s[1], 3, "y held in the shot at frame " + f);
        }
        assertTrue(cam.x.track().size() < 20, "keys were thinned: " + cam.x.track().size());
        d.undo();
        assertEquals(2, cam.x.track().size());
    }

    @Test
    void widescreenBarsCoverTheShotOnly() throws Exception {
        setUp();
        p.settings.width = 1280;
        p.settings.height = 720;
        p.settings.letterbox = 2.39f;
        Scene shot = Evaluator.evaluate(p, p.rootSymbol, 0, false, true);
        Group bars = (Group) shot.root.children.get(shot.root.children.size() - 1);
        assertEquals("letterbox", bars.source);
        Shape.Polygon top = (Shape.Polygon) bars.children.get(0);
        double barH = (720 - 1280 / 2.39) / 2;
        assertEquals(360 - barH, top.points[1], 1e-3);
        Scene edit = Evaluator.evaluate(p, p.rootSymbol, 0, false, false);
        for (Object o : edit.root.children) assertFalse(o instanceof Group && "letterbox".equals(((Group) o).source), "not while editing");
        String s = ProjectCodec.encode(p);
        assertEquals(2.39f, ProjectCodec.decode(s).settings.letterbox, 1e-6);
    }
}
