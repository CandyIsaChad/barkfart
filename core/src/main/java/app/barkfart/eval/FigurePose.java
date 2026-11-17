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
            p.angle[i] = base(nodes, p, i, pi) + rot;
            double len = r.f(node.length, t) * r.f(node.scale, t);
            double rad = Math.toRadians(p.angle[i]);
            p.x[i] = p.x[pi] + len * Math.cos(rad);
            p.y[i] = p.y[pi] + len * Math.sin(rad);
        }
        return p;
    }

    /** World angle the node's rotation is measured from (angle locks, Node.LOCK_*). */
    static double base(List<Node> nodes, FigurePose p, int i, int pi) {
        int lock = nodes.get(i).angleLock;
        if (lock == Node.LOCK_ABSOLUTE) return 0;
        if (lock == Node.LOCK_RELATIVE) {
            int a = pi;
            while (a >= 0 && nodes.get(a).isStatic && p.parent[a] >= 0) a = p.parent[a];
            return p.angle[a];
        }
        return p.angle[pi];
    }

    /** World angle node i's rotation is measured from, for editing tools. */
    public double baseAngle(List<Node> nodes, int i) {
        return parent[i] < 0 ? 0 : base(nodes, this, i, parent[i]);
    }
}
