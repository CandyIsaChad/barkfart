package app.barkfart.canvas;

import app.barkfart.edit.Editor;
import app.barkfart.eval.Affine;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;

/**
 * Touch interaction on the stage, independent of any widget toolkit. All
 * coordinates are view space (camera applied, y up, project pixels). A drag
 * is one undo step: begin on touch-down, update per move, end on touch-up.
 */
public final class CanvasTools {
    /** A picked joint. node == root means "move the whole figure". */
    public static final class Hit {
        public final Layer.FigureLayer layer;
        public final Node node;
        public final float x, y;

        public Hit(Layer.FigureLayer layer, Node node, float x, float y) {
            this.layer = layer;
            this.node = node;
            this.x = x;
            this.y = y;
        }
    }

    /** One joint handle to draw on the canvas. */
    public static final class Handle {
        public final Layer.FigureLayer layer;
        public final Node node;
        public final float x, y;
        public final boolean root;

        Handle(Layer.FigureLayer layer, Node node, float x, float y) {
            this.layer = layer;
            this.node = node;
            this.x = x;
            this.y = y;
            this.root = node.parent == null;
        }
    }

    private final Editor editor;
    /** When on, dragging a joint changes length as well as angle. */
    public boolean stretch;

    /** What a drag keys at the playhead besides the value being changed. */
    public enum KeyScope {
        /** The dragged joint's angle and length (so stretching is never left unkeyed). */
        JOINT,
        /** Every joint's angle and length plus the figure's position: a full pose, like the original. */
        FIGURE
    }

    public KeyScope keyScope = KeyScope.JOINT;
    /** Show and pick static (detail) nodes too; off while animating, like the original. */
    public boolean showStatic;
    /** Editing view through the camera (old behaviour); the editor now edits in stage space. */
    public boolean throughCamera;

    private Hit drag;
    private Affine dragView;
    private float grabDx, grabDy;
    private float startLayerX, startLayerY;
    /** Lengths at drag start, used as rest lengths where a node has none (smart-stretch). */
    private final java.util.Map<Node, Float> startLength = new java.util.HashMap<Node, Float>();

    public CanvasTools(Editor editor) {
        this.editor = editor;
    }

    public Symbol symbol() {
        return editor.doc.project.root();
    }

    /** Handles for every visible, unlocked figure layer of the root symbol at time t. */
    public List<Handle> handles(double t) {
        List<Handle> out = new ArrayList<Handle>();
        Symbol s = symbol();
        PropertyResolver r = new PropertyResolver(editor.doc.project, new ArrayList<Diagnostic>());
        for (Layer l : s.layers) {
            if (!(l instanceof Layer.FigureLayer) || !l.visible || l.locked || !r.b(l.shown, t)) continue;
            Layer.FigureLayer fl = (Layer.FigureLayer) l;
            Affine v = view(s, l, t);
            FigurePose pose = FigurePose.compute(fl, t, r);
            List<Node> nodes = fl.figure.nodes();
            for (int i = 0; i < nodes.size(); i++) {
                if (nodes.get(i).isStatic && !showStatic) continue;
                out.add(new Handle(fl, nodes.get(i), (float) v.mapX(pose.x[i], pose.y[i]), (float) v.mapY(pose.x[i], pose.y[i])));
            }
        }
        return out;
    }

    /**
     * Nearest handle within radius (view units). Ties go to the topmost layer,
     * then to non-root joints, so a small figure on top stays grabbable.
     */
    public Hit pick(float x, float y, float radius, double t) {
        List<Handle> hs = handles(t);
        Handle best = null;
        double bestD = radius * radius;
        for (Handle h : hs) {   // later layers are on top, so >= prefers them
            double d = (h.x - x) * (h.x - x) + (h.y - y) * (h.y - y);
            if (d <= bestD || (best != null && d <= bestD + 1e-9 && best.root && !h.root)) {
                bestD = d;
                best = h;
            }
        }
        return best == null ? null : new Hit(best.layer, best.node, best.x, best.y);
    }

    /**
     * Picking with node-digging (Stick Nodes 4.2.0): all handles within the
     * radius, nearest first; tapping again on the same cluster (previous is
     * one of them) gives the next one, wrapping. A selected joint within reach
     * wins the first tap ("selected node sticks").
     */
    public Hit pickCycling(float x, float y, float radius, double t, Node selected, Node previous) {
        List<Handle> near = new ArrayList<Handle>();
        for (Handle h : handles(t)) if ((h.x - x) * (h.x - x) + (h.y - y) * (h.y - y) <= radius * radius) near.add(h);
        if (near.isEmpty()) return null;
        final float fx = x, fy = y;
        java.util.Collections.sort(near, new java.util.Comparator<Handle>() {
            public int compare(Handle a, Handle b) {
                double da = (a.x - fx) * (a.x - fx) + (a.y - fy) * (a.y - fy), db = (b.x - fx) * (b.x - fx) + (b.y - fy) * (b.y - fy);
                if (Math.abs(da - db) > 1e-6) return da < db ? -1 : 1;
                return a.root == b.root ? 0 : (a.root ? 1 : -1);   // joints before main nodes
            }
        });
        int at = -1;
        for (int i = 0; i < near.size(); i++) if (near.get(i).node == previous) at = i;
        Handle h;
        if (at >= 0) h = near.get((at + 1) % near.size());
        else {
            h = near.get(0);
            for (Handle c : near) if (c.node == selected) h = c;
        }
        return new Hit(h.layer, h.node, h.x, h.y);
    }

    /** While dragging a whole figure, keep to the axis it moved most along (Stick Nodes 2.2.0). */
    public boolean axisLock;
    /** When > 0, joint angles from drags snap to multiples of this many degrees (e.g. 45). */
    public float snapDegrees;

    /** Snaps an angle to the nearest multiple of step degrees (step <= 0: unchanged). */
    public static double snap(double deg, double step) {
        return step > 0 ? Math.round(deg / step) * step : deg;
    }

    /**
     * Turns a whole figure about its main node by delta degrees at time t
     * (pinch-rotate, 1.2.8): rotation and position change together so the main
     * node stays put. One undo step.
     */
    public void rotateFigure(Layer.FigureLayer l, double delta, double t) {
        PropertyResolver r = new PropertyResolver(editor.doc.project, new ArrayList<Diagnostic>());
        Affine stage = Evaluator.layerStage(editor.doc.project, symbol(), l, t);
        FigurePose pose = FigurePose.compute(l, t, r);
        double px = stage.mapX(pose.x[0], pose.y[0]), py = stage.mapY(pose.x[0], pose.y[0]);
        double rot = r.f(l.transform.rotation, t) + delta;
        editor.doc.begin("Rotate figure");
        try {
            editor.change(l.transform.rotation, (float) rot, t);
            Affine after = Evaluator.layerStage(editor.doc.project, symbol(), l, t);
            double qx = after.mapX(pose.x[0], pose.y[0]), qy = after.mapY(pose.x[0], pose.y[0]);
            editor.change(l.transform.x, (float) (r.f(l.transform.x, t) + (px - qx)), t);
            editor.change(l.transform.y, (float) (r.f(l.transform.y, t) + (py - qy)), t);
        } finally {
            editor.doc.commit();
        }
    }

    /**
     * Panning mode (1.1.3): moves every top-level figure, sprite, text and
     * movieclip layer by (dx, dy) stage units at time t (joined and
     * camera-locked layers follow their parents). One undo step.
     */
    public void moveAll(float dx, float dy, double t) {
        PropertyResolver r = new PropertyResolver(editor.doc.project, new ArrayList<Diagnostic>());
        editor.doc.begin("Move everything");
        try {
            for (Layer l : symbol().layers) {
                if (l.parent != null || l instanceof Layer.CameraLayer || l instanceof Layer.AudioLayer || l.locked) continue;
                editor.change(l.transform.x, r.f(l.transform.x, t) + dx, t);
                editor.change(l.transform.y, r.f(l.transform.y, t) + dy, t);
            }
        } finally {
            editor.doc.commit();
        }
    }

    public boolean dragging() {
        return drag != null;
    }

    /** Starts a drag on a picked handle; opens one undo step. */
    public void begin(Hit hit, float x, float y, double t) {
        drag = hit;
        dragView = view(symbol(), hit.layer, t);
        grabDx = hit.x - x;
        grabDy = hit.y - y;
        startLayerX = hit.layer.transform.x.floatValue();
        startLayerY = hit.layer.transform.y.floatValue();
        if (!hit.layer.transform.x.track().isEmpty()) startLayerX = (Float) hit.layer.transform.x.keyValue(t);
        if (!hit.layer.transform.y.track().isEmpty()) startLayerY = (Float) hit.layer.transform.y.keyValue(t);
        startLength.clear();
        PropertyResolver r0 = new PropertyResolver(editor.doc.project, new ArrayList<Diagnostic>());
        for (Node n : hit.layer.figure.nodes()) startLength.put(n, r0.f(n.length, t));
        editor.doc.begin(hit.node.parent == null ? "Move figure" : "Pose node");
        pinKeys(hit, t);
    }

    private Affine view(Symbol s, Layer l, double t) {
        return throughCamera ? Evaluator.layerView(editor.doc.project, s, l, t) : Evaluator.layerStage(editor.doc.project, s, l, t);
    }

    /** Keys the scope's properties at t with their current values (only where no key exists yet). */
    private void pinKeys(Hit hit, double t) {
        java.util.List<app.barkfart.model.Property> ps = new java.util.ArrayList<app.barkfart.model.Property>();
        if (keyScope == KeyScope.FIGURE) {
            for (Node n : hit.layer.figure.nodes()) {
                if (n.parent == null) continue;
                ps.add(n.rotation);
                ps.add(n.length);
            }
            ps.add(hit.layer.transform.x);
            ps.add(hit.layer.transform.y);
        } else if (hit.node.parent != null) {
            ps.add(hit.node.rotation);
            ps.add(hit.node.length);
        } else {
            ps.add(hit.layer.transform.x);
            ps.add(hit.layer.transform.y);
        }
        for (app.barkfart.model.Property p : ps) {
            if (p.track().indexOf(t) >= 0) continue;
            app.barkfart.model.Interp in = p.track().isEmpty() ? editor.defaultInterp : p.track().get(Math.max(0, -p.track().indexOf(t) - 2)).interp;
            editor.doc.apply(app.barkfart.edit.Edits.putKey(p, new app.barkfart.model.Key(t, p.keyValue(t), in)));
        }
    }

    /** Moves the dragged joint toward the finger (view space). */
    public void move(float x, float y, double t) {
        if (drag == null) return;
        float tx = x + grabDx, ty = y + grabDy;
        Node n = drag.node;
        if (n.dragLock == Node.DRAG_HORIZONTAL) ty = drag.y;
        else if (n.dragLock == Node.DRAG_VERTICAL) tx = drag.x;
        if (n.parent == null) {
            // Move the figure: convert the view-space offset into the layer's parent space.
            if (axisLock) {   // keep to the axis moved along most
                if (Math.abs(tx - drag.x) >= Math.abs(ty - drag.y)) ty = drag.y;
                else tx = drag.x;
            }
            Affine inv = dragView.inverse();
            double ox = inv.mapX(drag.x, drag.y), oy = inv.mapY(drag.x, drag.y);
            double nx = inv.mapX(tx, ty), ny = inv.mapY(tx, ty);
            // Offset in layer-local units, scaled into the parent's space by the layer's own transform.
            Affine local = Evaluator.layerStage(editor.doc.project, symbol(), drag.layer, t);
            double dxp = local.a * (nx - ox) + local.c * (ny - oy);
            double dyp = local.b * (nx - ox) + local.d * (ny - oy);
            editor.change(drag.layer.transform.x, (float) (startLayerX + dxp), t);
            editor.change(drag.layer.transform.y, (float) (startLayerY + dyp), t);
            return;
        }
        PropertyResolver r = new PropertyResolver(editor.doc.project, new ArrayList<Diagnostic>());
        FigurePose pose = FigurePose.compute(drag.layer, t, r);
        int i = drag.layer.figure.indexOf(n.id), pi = pose.parent[i];
        Affine inv = dragView.inverse();
        double lx = inv.mapX(tx, ty), ly = inv.mapY(tx, ty);           // finger in layer space
        double dx = lx - pose.x[pi], dy = ly - pose.y[pi];
        if (dx * dx + dy * dy < 1e-9) return;
        double world = Math.toDegrees(Math.atan2(dy, dx));
        double current = r.f(n.rotation, t);
        double rel = world - pose.baseAngle(drag.layer.figure.nodes(), i);   // angle locks measure from elsewhere
        rel = snap(rel, snapDegrees);
        rel += 360 * Math.round((current - rel) / 360);                 // stay continuous with the old angle
        editor.change(n.rotation, (float) rel, t);
        if (stretch) {
            float scale = r.f(n.scale, t);
            if (scale > 1e-6f) {
                float len = (float) (Math.sqrt(dx * dx + dy * dy) / scale);
                editor.change(n.length, len, t);
                if (n.smartStretch) smartStretch(drag.layer, pose, i, len, t);
            }
        }
    }

    /**
     * Smart-stretch, fitted against the original's own movieclips (Rain, Action
     * Lines; NOTES.md): with k = length / rest length of the stretched node, each
     * static descendant (not past a non-static node or a "do not smart-stretch"
     * branch) keeps its angle and gets rest x sqrt((k cos b)^2 + sin^2 b), b being
     * its angle to the stretched node. Parallel details scale by k, perpendicular
     * ones keep their length.
     */
    public void smartStretch(Layer.FigureLayer layer, FigurePose pose, int si, float len, double t) {
        List<Node> nodes = layer.figure.nodes();
        Node s = nodes.get(si);
        float rest = s.restLength > 0 ? s.restLength : startLength.containsKey(s) ? startLength.get(s) : 0;
        if (rest <= 1e-6f) return;
        double k = len / rest;
        boolean[] in = new boolean[nodes.size()];
        in[si] = true;
        for (int j = si + 1; j < nodes.size(); j++) {   // parents precede children
            int pj = pose.parent[j];
            Node nj = nodes.get(j);
            if (pj < 0 || !in[pj] || !nj.isStatic || nj.noSmartStretch) continue;
            in[j] = true;
            float rj = nj.restLength > 0 ? nj.restLength : startLength.containsKey(nj) ? startLength.get(nj) : 0;
            double b = Math.toRadians(pose.angle[j] - pose.angle[si]);
            double f = Math.sqrt(k * k * Math.cos(b) * Math.cos(b) + Math.sin(b) * Math.sin(b));
            editor.change(nj.length, (float) (rj * f), t);
        }
    }

    /** Ends the drag: one undo step. */
    public void end() {
        if (drag == null) return;
        drag = null;
        editor.doc.commit();
    }

    /** Abandons the drag and reverts it. */
    public void cancel() {
        if (drag == null) return;
        drag = null;
        editor.doc.cancel();
    }
}
