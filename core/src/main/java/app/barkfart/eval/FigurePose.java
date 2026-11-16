package app.barkfart.eval;

import app.barkfart.model.Figure;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import java.util.List;

/**
 * Forward kinematics of one figure at time t, in the layer's local space.
 * Shared by the evaluator (drawing) and the canvas (picking and dragging), so
 * both always agree on where every joint is.
 */
public final class FigurePose {
    /** Joint positions and world angles (degrees), indexed like Figure.nodes(). */
    public final double[] x, y, angle;
    /** Parent index per node, -1 for the root. */
    public final int[] parent;

    private FigurePose(int n) {
        x = new double[n];
        y = new double[n];
        angle = new double[n];
        parent = new int[n];
    }

    public static FigurePose compute(Layer.FigureLayer layer, double t, PropertyResolver r) {
        Figure f = layer.figure;
        List<Node> nodes = f.nodes();
        FigurePose p = new FigurePose(nodes.size());
        for (int i = 0; i < nodes.size(); i++) {
            Node node = nodes.get(i);
            double rot = r.f(node.rotation, t);
            if (node.parent == null) {
                p.parent[i] = -1;
                p.angle[i] = rot;
                continue;
            }
            int pi = f.indexOf(node.parent);
            p.parent[i] = pi;
            p.angle[i] = p.angle[pi] + rot;
            double len = r.f(node.length, t) * r.f(node.scale, t);
            double rad = Math.toRadians(p.angle[i]);
            p.x[i] = p.x[pi] + len * Math.cos(rad);
            p.y[i] = p.y[pi] + len * Math.sin(rad);
        }
        return p;
    }
}
