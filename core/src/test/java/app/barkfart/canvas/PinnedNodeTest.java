package app.barkfart.canvas;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.eval.Affine;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class PinnedNodeTest {
    private static double[] stage(Project p, Layer.FigureLayer l, String id) {
        PropertyResolver r = new PropertyResolver(p, new java.util.ArrayList<Diagnostic>());
        FigurePose pose = FigurePose.compute(l, 0, r);
        int i = l.figure.indexOf(id);
        Affine a = Evaluator.layerStage(p, p.root(), l, 0);
        return new double[] {a.mapX(pose.x[i], pose.y[i]), a.mapY(pose.x[i], pose.y[i])};
    }

    @Test
    void aPinnedFootStaysPutWhileTheHipMoves() throws Exception {
        Project p = Fixtures.empty();
        Layer.FigureLayer leg = new Layer.FigureLayer(p.newId("L"), "leg");
        leg.figure.add(new Node(leg.id, "hip", null, Node.Shape.NONE));
        Fixtures.add(leg, "knee", "hip", Node.Shape.LINE, -60, 50);
        Fixtures.add(leg, "foot", "knee", Node.Shape.LINE, -60, 50);
        Fixtures.add(leg, "toe", "foot", Node.Shape.LINE, 90, 10);
        p.root().layers.add(leg);
        p.invalidateIndex();
        Document d = new Document(p);
        d.apply(app.barkfart.edit.FigureOps.setPinned(leg, "foot", true));
        CanvasTools tools = new CanvasTools(new Editor(d));
        double[] foot = stage(p, leg, "foot");
        double[] knee0 = stage(p, leg, "knee");
        tools.begin(new CanvasTools.Hit(leg, leg.figure.root(), 0, 0), 0, 0, 0);
        tools.move(20, 10, 0);
        tools.end();
        double[] after = stage(p, leg, "foot"), hip = stage(p, leg, "hip"), knee = stage(p, leg, "knee");
        assertEquals(20, hip[0], 1e-3, "the figure moved");
        assertEquals(foot[0], after[0], 1e-3, "foot x held");
        assertEquals(foot[1], after[1], 1e-3, "foot y held");
        assertEquals(50, Math.hypot(knee[0] - hip[0], knee[1] - hip[1]), 1e-3, "thigh keeps its length");
        assertEquals(50, Math.hypot(after[0] - knee[0], after[1] - knee[1]), 1e-3, "shin keeps its length");
        // the knee bends to the same side as before
        double side0 = (foot[0] - 0) * (knee0[1] - 0) - (foot[1] - 0) * (knee0[0] - 0);
        double side1 = (after[0] - hip[0]) * (knee[1] - hip[1]) - (after[1] - hip[1]) * (knee[0] - hip[0]);
        assertTrue(Math.signum(side0) == Math.signum(side1), "same bend");
        d.undo();
        assertEquals(knee0[0], stage(p, leg, "knee")[0], 1e-4, "one undo step");
        // out of reach: the leg points straight at the pin
        tools.begin(new CanvasTools.Hit(leg, leg.figure.root(), 0, 0), 0, 0, 0);
        tools.move(-300, 0, 0);
        tools.end();
        double[] h = stage(p, leg, "hip"), f = stage(p, leg, "foot"), k = stage(p, leg, "knee");
        double cross = (k[0] - h[0]) * (f[1] - h[1]) - (k[1] - h[1]) * (f[0] - h[0]);
        assertEquals(0, cross / 2500, 1e-3, "straight");
        assertTrue(ProjectCodec.decode(ProjectCodec.encode(p)).root().layers.get(0) instanceof Layer.FigureLayer);
        assertTrue(((Layer.FigureLayer) ProjectCodec.decode(ProjectCodec.encode(p)).root().layers.get(0)).figure.node("foot").pinned, "saved");
    }
}
