package app.barkfart.eval;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.Image;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class BackgroundAndEaseTest {
    private static int px(Project p, double t, boolean camera, int x, int y) {
        Image im = new Image(p.settings.width, p.settings.height, -p.settings.width / 2f, p.settings.height / 2f, 1);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, t, false, camera)), im);
        return Compositor.rgba(im, x, y);
    }

    @Test
    void cameraBackgroundIsSolidOrAGradientAndKeyable() throws Exception {
        Project p = Fixtures.empty();
        p.settings.width = 100;
        p.settings.height = 80;
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        p.root().layers.add(cam);
        p.invalidateIndex();
        assertEquals(0, px(p, 0, true, 50, 40) & 0xff, "no background colour: nothing drawn");
        cam.background.track().put(new Key(0, Color.rgba8888(0xff0000ff), Interp.HOLD));
        cam.background.track().put(new Key(1, Color.rgba8888(0x0000ffff), Interp.HOLD));
        assertEquals(0xff0000ff, px(p, 0, true, 50, 40), "frame 1 red");
        assertEquals(0x0000ffff, px(p, 1, true, 50, 40), "frame 2 blue (per-frame background)");
        cam.background.track().clear();
        cam.background.setValue(Color.rgba8888(0xffffffff));
        cam.background2.setValue(Color.rgba8888(0x000000ff));
        int top = px(p, 0, true, 50, 0), bottom = px(p, 0, true, 50, 79), mid = px(p, 0, true, 50, 40);
        assertTrue((top >>> 24) > 240 && (bottom >>> 24) < 15, "white top, black bottom");
        assertEquals(128, mid >>> 24, 8, "halfway grey");
        // editor view (no camera applied): the background covers the camera's view area
        cam.x.setValue(30f);
        cam.zoom.setValue(2f);
        assertEquals(0, px(p, 0, false, 5, 40) & 0xff, "outside the camera's area in the editor");
        assertEquals(0xff, px(p, 0, false, 80, 40) & 0xff, "inside it");
        // under figures; saved
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        Layer.CameraLayer qc = (Layer.CameraLayer) q.root().layers.get(0);
        assertEquals(Color.rgba8888(0x000000ff), qc.background2.value());
        Group root = Evaluator.evaluate(p, p.rootSymbol, 0, false, true).root;
        assertEquals("background", ((Group) root.children.get(0)).source);
        assertTrue(Evaluator.evaluate(p, p.rootSymbol, 0, true, true).root.children.isEmpty(), "not in onion ghosts");
    }

    @Test
    void bounceAndElasticCurvesEndOnTargetAndSurviveSaving() throws Exception {
        for (Ease e : new Ease[] {Ease.BOUNCE_OUT, Ease.BOUNCE_IN, Ease.ELASTIC_OUT, Ease.ELASTIC_IN}) {
            assertEquals(0, e.apply(0), 1e-6);
            assertEquals(1, e.apply(1), 1e-6);
            assertEquals(0, e.apply(0.001f), 0.02, e.kind + " starts at 0");
            assertEquals(1, e.apply(0.999f), 0.02, e.kind + " ends at 1");
        }
        assertEquals(1, Ease.BOUNCE_OUT.apply(1 / 2.75f), 1e-4, "first bounce touches the target");
        assertTrue(Ease.BOUNCE_OUT.apply(0.5f) < 1, "then bounces back");
        float max = 0;
        for (int i = 0; i <= 100; i++) max = Math.max(max, Ease.ELASTIC_OUT.apply(i / 100f));
        assertTrue(max > 1.2f, "elastic overshoots");
        Project p = Fixtures.empty();
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.x.track().put(new Key(0, 0f, Interp.BEZIER, Ease.BOUNCE_OUT));
        cam.x.track().put(new Key(1, 100f, Interp.LINEAR));
        p.root().layers.add(cam);
        p.invalidateIndex();
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        Layer.CameraLayer qc = (Layer.CameraLayer) q.root().layers.get(0);
        assertEquals(Ease.BOUNCE_OUT, qc.x.track().keys().get(0).ease);
        PropertyResolver r = new PropertyResolver(q, new java.util.ArrayList<Diagnostic>());
        assertEquals(100 * Ease.BOUNCE_OUT.apply(0.5f), r.f(qc.x, 0.5), 1e-3);
    }
}
