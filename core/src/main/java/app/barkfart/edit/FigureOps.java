package app.barkfart.edit;

import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.eval.Shape;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Polyfill;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Building and reshaping figures (Stick Nodes' creation mode, as core
 * operations): add a segment, split one into pieces (along its curve),
 * duplicate a branch, flip a branch or a figure, change a whole branch,
 * move a branch's layering, reset a length. Every operation is one undo step:
 * the figure is snapshotted before and after, and undo / redo restore the
 * snapshots onto the same node objects (selections stay valid).
 */
public final class FigureOps {
    private FigureOps() {}

    // ---- snapshots ----

    /** Everything about one node that an operation may change. */
    private static final class NodeState {
        final String parent;
        final Node.Shape shape;
        final int drawOrder, rightTriangle, halfArc, polygonSides, angleLock, dragLock;
        final float curve, restLength, trapezoidRatio;
        final boolean upsideDown, gradient, gradientReverse, gradientSideways, gradientLinear, pinned, isStatic, stretchy, smartStretch, noSmartStretch,
                hollow, outline, roundStart, roundEnd;
        final Object[] values;
        final List<List<Key>> keys = new ArrayList<List<Key>>();

        NodeState(Node n) {
            parent = n.parent;
            shape = n.shape;
            drawOrder = n.drawOrder;
            rightTriangle = n.rightTriangle;
            halfArc = n.halfArc;
            polygonSides = n.polygonSides;
            angleLock = n.angleLock;
            dragLock = n.dragLock;
            curve = n.curve;
            restLength = n.restLength;
            trapezoidRatio = n.trapezoidRatio;
            upsideDown = n.triangleUpsideDown;
            gradient = n.gradient;
            gradientReverse = n.gradientReverse;
            gradientSideways = n.gradientSideways;
            gradientLinear = n.gradientLinear;
            pinned = n.pinned;
            isStatic = n.isStatic;
            stretchy = n.stretchy;
            smartStretch = n.smartStretch;
            noSmartStretch = n.noSmartStretch;
            hollow = n.circleHollow;
            outline = n.circleOutline;
            roundStart = n.trapezoidRoundStart;
            roundEnd = n.trapezoidRoundEnd;
            Property[] ps = n.properties();
            values = new Object[ps.length];
            for (int i = 0; i < ps.length; i++) {
                values[i] = ps[i].value();
                keys.add(new ArrayList<Key>(ps[i].track().keys()));
            }
        }

        void restore(Node n) {
            n.parent = parent;
            n.shape = shape;
            n.drawOrder = drawOrder;
            n.rightTriangle = rightTriangle;
            n.halfArc = halfArc;
            n.polygonSides = polygonSides;
            n.angleLock = angleLock;
            n.dragLock = dragLock;
            n.curve = curve;
            n.restLength = restLength;
            n.trapezoidRatio = trapezoidRatio;
            n.triangleUpsideDown = upsideDown;
            n.gradient = gradient;
            n.gradientReverse = gradientReverse;
            n.gradientSideways = gradientSideways;
            n.gradientLinear = gradientLinear;
            n.pinned = pinned;
            n.isStatic = isStatic;
            n.stretchy = stretchy;
            n.smartStretch = smartStretch;
            n.noSmartStretch = noSmartStretch;
            n.circleHollow = hollow;
            n.circleOutline = outline;
            n.trapezoidRoundStart = roundStart;
            n.trapezoidRoundEnd = roundEnd;
            Property[] ps = n.properties();
            for (int i = 0; i < ps.length; i++) {
                ps[i].setValue(values[i]);
                ps[i].track().clear();
                for (Key k : keys.get(i)) ps[i].track().put(k);
            }
        }
    }

    private static final class FigureState {
        final List<Node> order;
        final Map<Node, NodeState> states = new HashMap<Node, NodeState>();
        final List<Polyfill> fills;

        FigureState(Layer.FigureLayer l) {
            order = l.figure.nodeList();
            for (Node n : order) states.put(n, new NodeState(n));
            fills = new ArrayList<Polyfill>(l.figure.polyfills);
        }

        void restore(Layer.FigureLayer l) {
            for (Map.Entry<Node, NodeState> e : states.entrySet()) e.getValue().restore(e.getKey());
            l.figure.setNodes(order);
            l.figure.polyfills.clear();
            l.figure.polyfills.addAll(fills);
        }
    }

    /** A change to a figure as one undo step (the change runs once; redo restores its result). */
    public static Command edit(final String label, final Layer.FigureLayer l, final Runnable change) {
        return new Command() {
            FigureState before, after;

            public void apply() {
                if (after != null) {
                    after.restore(l);
                    return;
                }
                before = new FigureState(l);
                try {
                    change.run();
                } catch (RuntimeException e) {
                    before.restore(l);
                    throw e;
                }
                after = new FigureState(l);
            }

            public void revert() {
                before.restore(l);
            }

            public String label() {
                return label;
            }
        };
    }

    // ---- helpers ----

    private static PropertyResolver resolver(Project p) {
        return new PropertyResolver(p, new ArrayList<Diagnostic>());
    }

    private static void copyProperty(Property from, Property to) {
        to.setValue(from.value());
        to.track().clear();
        for (Key k : from.track().keys()) to.track().put(k);
    }

    /** Copies looks and options (not identity, parent, rotation or length). */
    private static void copyLooks(Node from, Node to) {
        to.shape = from.shape;
        to.curve = from.curve;
        to.isStatic = from.isStatic;
        to.stretchy = from.stretchy;
        to.gradient = from.gradient;
        to.gradientReverse = from.gradientReverse;
        to.gradientSideways = from.gradientSideways;
        to.gradientLinear = from.gradientLinear;
        to.circleHollow = from.circleHollow;
        to.circleOutline = from.circleOutline;
        to.halfArc = from.halfArc;
        to.polygonSides = from.polygonSides;
        to.trapezoidRatio = from.trapezoidRatio;
        to.trapezoidRoundStart = from.trapezoidRoundStart;
        to.trapezoidRoundEnd = from.trapezoidRoundEnd;
        to.triangleUpsideDown = from.triangleUpsideDown;
        to.rightTriangle = from.rightTriangle;
        copyProperty(from.thickness, to.thickness);
        copyProperty(from.color, to.color);
        copyProperty(from.gradientColor, to.gradientColor);
        copyProperty(from.outlineColor, to.outlineColor);
        copyProperty(from.scale, to.scale);
        copyProperty(from.shown, to.shown);
    }

    // ---- operations ----

    /** The node created by the last addSegment / duplicateBranch (for selecting it). */
    public static final class Made {
        public Node node;
    }

    /**
     * A new segment from a node, inheriting its static / stretchy setting,
     * thickness and colour (Stick Nodes 1.2.5 "inherit node properties").
     */
    public static Command addSegment(final Layer.FigureLayer l, final String parentId, final float rotation, final float length, final Made made) {
        final Node parent = l.figure.node(parentId);
        if (parent == null) throw new IllegalArgumentException("no node " + parentId);
        return edit("Add segment", l, new Runnable() {
            public void run() {
                Node n = new Node(l.id, l.figure.freeId("n"), parentId, parent.parent == null ? Node.Shape.LINE : parent.shape);
                if (parent.parent != null) {
                    n.isStatic = parent.isStatic;
                    n.stretchy = parent.stretchy;
                    copyProperty(parent.thickness, n.thickness);
                    copyProperty(parent.color, n.color);
                    n.thickness.track().clear();
                    n.color.track().clear();
                }
                n.rotation.setValue(rotation);
                n.length.setValue(length);
                n.restLength = length;
                l.figure.add(n);
                if (made != null) made.node = n;
            }
        });
    }

    /**
     * Splits a segment into pieces of equal length. A curved segment is split
     * along its arc: each piece is a smaller arc with the same curvature, so the
     * drawn shape does not change. Length keys are divided between the pieces;
     * rotation keys stay on the first piece.
     */
    public static Command split(final Layer.FigureLayer l, final String nodeId, final int pieces) {
        final Node n = l.figure.node(nodeId);
        if (n == null || n.parent == null) throw new IllegalArgumentException("cannot split " + nodeId);
        if (pieces < 2 || pieces > 64) throw new IllegalArgumentException("pieces 2..64");
        return edit("Split segment", l, new Runnable() {
            public void run() {
                float len = n.length.floatValue();
                // arc geometry (figure units, scale 1): bulge h, turn theta
                double h = n.curve * Shape.CURVE_SAGITTA, chord = len;
                double theta = chord < 1e-9 ? 0 : 4 * Math.atan(2 * h / chord);
                double sub = theta / pieces;
                double pieceChord = Math.abs(theta) < 1e-9 ? chord / pieces : chord * Math.sin(sub / 2) / Math.sin(theta / 2);
                float pieceCurve = Math.abs(theta) < 1e-9 ? 0 : (float) (pieceChord / 2 * Math.tan(sub / 4) / Shape.CURVE_SAGITTA);
                float ratio = (float) (pieceChord / Math.max(1e-9, chord));
                // a positive curve bulges left: the first piece leaves turned left by half
                // the arc (less half a piece), each next piece turns one piece back right
                float firstTurn = (float) Math.toDegrees(theta / 2 - sub / 2);
                float stepTurn = (float) Math.toDegrees(-sub);
                int at = l.figure.indexOf(nodeId);
                String prev = n.parent;
                List<Node> made = new ArrayList<Node>();
                for (int i = 0; i < pieces - 1; i++) {
                    Node piece = new Node(l.id, l.figure.freeId("n"), prev, n.shape);
                    copyLooks(n, piece);
                    piece.curve = pieceCurve;
                    piece.drawOrder = n.drawOrder;
                    if (i == 0) {
                        copyProperty(n.rotation, piece.rotation);
                        offsetAngles(piece.rotation, firstTurn);
                    } else {
                        piece.rotation.setValue(stepTurn);
                    }
                    copyProperty(n.length, piece.length);
                    scaleFloats(piece.length, ratio);
                    piece.restLength = n.restLength * ratio;
                    l.figure.insert(at + i, piece);
                    made.add(piece);
                    prev = piece.id;
                }
                l.figure.reparent(nodeId, prev);
                n.rotation.setValue(stepTurn);
                n.rotation.track().clear();
                scaleFloats(n.length, ratio);
                n.restLength *= ratio;
                n.curve = pieceCurve;
            }
        });
    }

    private static void offsetAngles(Property p, float delta) {
        p.setValue(p.floatValue() + delta);
        List<Key> ks = new ArrayList<Key>(p.track().keys());
        p.track().clear();
        for (Key k : ks) p.track().put(new Key(k.time, ((Float) k.value) + delta, k.interp, k.ease));
    }

    private static void scaleFloats(Property p, float k) {
        p.setValue(p.floatValue() * k);
        List<Key> ks = new ArrayList<Key>(p.track().keys());
        p.track().clear();
        for (Key key : ks) p.track().put(new Key(key.time, ((Float) key.value) * k, key.interp, key.ease));
    }

    /** Copies a node and its branch next to the original (same parent), drawn just above it. */
    public static Command duplicateBranch(final Layer.FigureLayer l, final String nodeId, final Made made) {
        final Node top = l.figure.node(nodeId);
        if (top == null || top.parent == null) throw new IllegalArgumentException("cannot duplicate " + nodeId);
        return edit("Duplicate branch", l, new Runnable() {
            public void run() {
                List<Node> sub = l.figure.subtree(nodeId);
                Map<String, String> ids = new HashMap<String, String>();
                int maxOrder = Integer.MIN_VALUE;
                for (Node x : l.figure.nodes()) maxOrder = Math.max(maxOrder, x.drawOrder);
                // copies keep their relative layering, above everything (pasted on top)
                int base = maxOrder + 1, minSub = Integer.MAX_VALUE;
                for (Node x : sub) minSub = Math.min(minSub, x.drawOrder);
                for (Node x : sub) {
                    String parent = x == top ? top.parent : ids.get(x.parent);
                    Node c = new Node(l.id, l.figure.freeId("n"), parent, x.shape);
                    copyLooks(x, c);
                    copyProperty(x.rotation, c.rotation);
                    copyProperty(x.length, c.length);
                    c.restLength = x.restLength;
                    c.smartStretch = x.smartStretch;
                    c.noSmartStretch = x.noSmartStretch;
                    c.angleLock = x.angleLock;
                    c.dragLock = x.dragLock;
                    c.drawOrder = base + (x.drawOrder - minSub);
                    l.figure.add(c);
                    ids.put(x.id, c.id);
                    if (x == top && made != null) made.node = c;
                }
                // polyfills entirely inside the branch come along
                List<Polyfill> extra = new ArrayList<Polyfill>();
                for (Polyfill pf : l.figure.polyfills) {
                    if (!ids.containsKey(pf.anchor)) continue;
                    boolean inside = true;
                    for (String x : pf.nodes) inside &= ids.containsKey(x);
                    if (!inside) continue;
                    Polyfill c = new Polyfill(l.id, pf.id + "c" + l.figure.polyfills.size());
                    c.anchor = ids.get(pf.anchor);
                    for (String x : pf.nodes) c.nodes.add(ids.get(x));
                    c.useOwnColor = pf.useOwnColor;
                    copyProperty(pf.color, c.color);
                    extra.add(c);
                }
                l.figure.polyfills.addAll(extra);
            }
        });
    }

    /**
     * Mirrors a branch (horizontal: left-right, else up-down) about its
     * parent's joint. The branch's first node gets its world angle mirrored
     * (using the parent's angle at each key's time); nodes below it just turn
     * the other way. Curves, right triangles and half arcs flip sides too.
     */
    public static Command flipBranch(final Project p, final Layer.FigureLayer l, final String nodeId, final boolean horizontal, final double t) {
        final Node top = l.figure.node(nodeId);
        if (top == null || top.parent == null) throw new IllegalArgumentException("cannot flip " + nodeId);
        return edit(horizontal ? "Flip horizontally" : "Flip vertically", l, new Runnable() {
            public void run() {
                PropertyResolver r = resolver(p);
                int pi = l.figure.indexOf(top.parent);
                double pwNow = FigurePose.compute(l, t, r).angle[pi];
                double base = horizontal ? 180 : 0;
                // first node: r' = base - 2 pw - r
                top.rotation.setValue((float) (base - 2 * pwNow - top.rotation.floatValue()));
                List<Key> ks = new ArrayList<Key>(top.rotation.track().keys());
                top.rotation.track().clear();
                for (Key k : ks) {
                    double pw = FigurePose.compute(l, k.time, resolver(p)).angle[pi];
                    top.rotation.track().put(new Key(k.time, (float) (base - 2 * pw - (Float) k.value), k.interp, k.ease));
                }
                for (Node x : l.figure.subtree(nodeId)) {
                    if (x != top) negate(x.rotation);
                    x.curve = -x.curve;
                    x.rightTriangle = -x.rightTriangle;
                    x.halfArc = -x.halfArc;
                }
            }
        });
    }

    private static void negate(Property p) {
        p.setValue(-p.floatValue());
        List<Key> ks = new ArrayList<Key>(p.track().keys());
        p.track().clear();
        for (Key k : ks) p.track().put(new Key(k.time, -(Float) k.value, k.interp, k.ease));
    }

    /** Mirrors a whole figure (its layer's x or y scale changes sign, keys included). */
    public static Command flipFigure(final Layer l, final boolean horizontal) {
        final Property s = horizontal ? l.transform.scaleX : l.transform.scaleY;
        final Object before = s.value();
        final List<Key> keys = new ArrayList<Key>(s.track().keys());
        return new Command() {
            public void apply() {
                s.setValue(-((Float) before));
                s.track().clear();
                for (Key k : keys) s.track().put(new Key(k.time, -(Float) k.value, k.interp, k.ease));
            }

            public void revert() {
                s.setValue(before);
                s.track().clear();
                for (Key k : keys) s.track().put(k);
            }

            public String label() {
                return horizontal ? "Flip figure horizontally" : "Flip figure vertically";
            }
        };
    }

    /** A change applied to every node of a branch ("Modify Branch"). */
    public interface NodeChange {
        void apply(Node n);
    }

    public static Command modifyBranch(final Layer.FigureLayer l, final String nodeId, final String label, final NodeChange change) {
        if (l.figure.node(nodeId) == null) throw new IllegalArgumentException("no node " + nodeId);
        return edit(label, l, new Runnable() {
            public void run() {
                for (Node x : l.figure.subtree(nodeId)) if (x.parent != null) change.apply(x);
            }
        });
    }

    /** Ready-made branch changes. */
    public static NodeChange setStatic(final boolean on) {
        return new NodeChange() {
            public void apply(Node n) {
                n.isStatic = on;
            }
        };
    }

    public static NodeChange setThickness(final float w) {
        return new NodeChange() {
            public void apply(Node n) {
                n.thickness.track().clear();
                n.thickness.setValue(w);
            }
        };
    }

    public static NodeChange setColor(final app.barkfart.model.Color c) {
        return new NodeChange() {
            public void apply(Node n) {
                n.color.track().clear();
                n.color.setValue(c);
            }
        };
    }

    public static NodeChange setShape(final Node.Shape s) {
        return new NodeChange() {
            public void apply(Node n) {
                n.shape = s;
            }
        };
    }

    public static NodeChange scaleLength(final float k) {
        return new NodeChange() {
            public void apply(Node n) {
                scaleFloats(n.length, k);
                n.restLength *= k;
            }
        };
    }

    /**
     * Moves a whole branch one step back (behind the next node below it that is
     * not in the branch) or forward, keeping the branch's own layering.
     */
    public static Command pushBranch(final Layer.FigureLayer l, final String nodeId, final boolean forward) {
        if (l.figure.node(nodeId) == null) throw new IllegalArgumentException("no node " + nodeId);
        return edit(forward ? "Bring branch forward" : "Push branch back", l, new Runnable() {
            public void run() {
                java.util.HashSet<Node> branch = new java.util.HashSet<Node>(l.figure.subtree(nodeId));
                List<Node> nodes = l.figure.nodes();
                List<Node> seq = new ArrayList<Node>();
                for (int i : l.figure.drawSequence()) seq.add(nodes.get(i));
                List<Node> inBranch = new ArrayList<Node>(), rest = new ArrayList<Node>();
                for (Node x : seq) (branch.contains(x) ? inBranch : rest).add(x);
                // where the block sits among the others: count others drawn before its first node
                int before = 0;
                for (Node x : seq) {
                    if (branch.contains(x)) break;
                    before++;
                }
                int to = Math.max(0, Math.min(rest.size(), before + (forward ? 1 : -1)));
                List<Node> out = new ArrayList<Node>(rest.subList(0, to));
                out.addAll(inBranch);
                out.addAll(rest.subList(to, rest.size()));
                for (int i = 0; i < out.size(); i++) out.get(i).drawOrder = i;
            }
        });
    }

    /** Puts a segment back to its rest length at time t (keyed like any pose edit), with smart-stretch. */
    public static boolean resetLength(app.barkfart.canvas.CanvasTools tools, Editor editor, Project p, Layer.FigureLayer l, Node n, double t) {
        if (n.restLength <= 0) return false;
        editor.doc.begin("Reset length");
        try {
            editor.change(n.length, n.restLength, t);
            if (n.smartStretch) tools.smartStretch(l, FigurePose.compute(l, t, resolver(p)), l.figure.indexOf(n.id), n.restLength, t);
        } finally {
            editor.doc.commit();
        }
        return true;
    }

    /** Pins or unpins one joint (it keeps its stage position while others are dragged). One undo step. */
    public static Command setPinned(final Layer.FigureLayer l, final String nodeId, final boolean on) {
        return edit(on ? "Pin joint" : "Unpin joint", l, new Runnable() {
            public void run() {
                Node n = l.figure.node(nodeId);
                if (n != null) n.pinned = on;
            }
        });
    }

}
