package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Affine;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class FlipJoinedTest {
    private static double[] at(Project p, Layer.FigureLayer l, String id, double t) {
        FigurePose pose = FigurePose.compute(l, t, new PropertyResolver(p, new java.util.ArrayList<Diagnostic>()));
        int i = l.figure.indexOf(id);
        Affine a = Evaluator.layerStage(p, p.root(), l, t);
        return new double[] {a.mapX(pose.x[i], pose.y[i]), a.mapY(pose.x[i], pose.y[i])};
    }

    @Test
    void joinedFiguresMirrorAcrossTheAnchorsAxis() {
        Project p = Fixtures.empty();
        Layer.FigureLayer body = new Layer.FigureLayer(p.newId("L"), "body");
        body.figure.add(new Node(body.id, "r", null, Node.Shape.NONE));
        Fixtures.add(body, "hand", "r", Node.Shape.LINE, 30, 60);
        Layer.FigureLayer sword = new Layer.FigureLayer(p.newId("L"), "sword");
        sword.figure.add(new Node(sword.id, "r", null, Node.Shape.NONE));
        Fixtures.add(sword, "tip", "r", Node.Shape.LINE, 40, 50);
        p.root().layers.add(body);
        p.root().layers.add(sword);
        p.invalidateIndex();
        Document d = new Document(p);
        d.apply(JoinOps.join(p, p.root(), sword, body, "hand", true, 0));
        sword.transform.y.track().put(new Key(1, 7f, Interp.LINEAR));
        sword.transform.y.track().put(new Key(0, 3f, Interp.LINEAR));
        double[] before = at(p, sword, "tip", 0), hand = at(p, body, "hand", 0);
        double ang = Math.toRadians(30);
        d.apply(JoinOps.flipJoined(p.root(), body, "hand"));
        double[] after = at(p, sword, "tip", 0);
        // reflect "before" across the line through the hand along 30 degrees
        double vx = before[0] - hand[0], vy = before[1] - hand[1];
        double ux = Math.cos(ang), uy = Math.sin(ang);
        double along = vx * ux + vy * uy;
        double rx = hand[0] + 2 * along * ux - vx, ry = hand[1] + 2 * along * uy - vy;
        assertEquals(rx, after[0], 1e-3);
        assertEquals(ry, after[1], 1e-3);
        assertEquals(-7f, (Float) sword.transform.y.track().keys().get(1).value, 1e-6, "keys flipped too");
        d.undo();
        assertEquals(before[0], at(p, sword, "tip", 0)[0], 1e-4);
        assertThrows(IllegalArgumentException.class, () -> JoinOps.flipJoined(p.root(), body, "r"));
    }
}
