package app.barkfart.eval;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.StructureEdits;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class ParallaxTest {
    @Test
    void cameraLockedLayersFollowTheCameraByTheirParallax() throws Exception {
        Project p = Fixtures.empty();
        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        Layer.FigureLayer bg = new Layer.FigureLayer(p.newId("L"), "bg");
        bg.parent = cam.id;
        p.root().layers.add(bg);
        p.root().layers.add(cam);
        p.invalidateIndex();
        cam.x.setValue(100f);
        cam.zoom.setValue(4f);
        // in the shot: fixed at parallax 0
        Affine v0 = Evaluator.layerView(p, p.root(), bg, 0);
        assertEquals(0, v0.mapX(0, 0), 1e-6);
        assertEquals(1, v0.a, 1e-6);
        new Document(p).apply(StructureEdits.setParallax(bg, 1));
        Affine v1 = Evaluator.layerView(p, p.root(), bg, 0);
        assertEquals(-400, v1.mapX(0, 0), 1e-4, "parallax 1: moves with the world");
        assertEquals(4, v1.a, 1e-6);
        bg.parallax = 0.5f;
        Affine vh = Evaluator.layerView(p, p.root(), bg, 0);
        assertEquals(2, vh.a, 1e-6, "half the zoom (geometric)");
        assertEquals(-200, vh.mapX(0, 0), 1e-4, "half the move in world units (50 of 100), times the shot zoom 4");
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        assertEquals(0.5f, q.root().layers.get(0).parallax, 1e-6);
        Document d = new Document(p);
        d.apply(StructureEdits.setParallax(bg, 7));
        assertEquals(1, bg.parallax, 1e-6, "clamped");
        d.undo();
        assertEquals(0.5f, bg.parallax, 1e-6);
    }
}
