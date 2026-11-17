package app.barkfart.edit;

import app.barkfart.eval.Affine;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;

/**
 * Joining (Stick Nodes 1.7.4): attach a layer to a joint of a figure so it
 * follows that joint (position, and its angle unless told otherwise), or
 * detach it. Either way the layer stays exactly where it is on screen: its
 * transform (static values and every key) is rewritten in the new frame.
 * One undo step.
 */
public final class JoinOps {
    private JoinOps() {}

    /** Attach child to target's node (nodeId null = to the figure itself). */
    public static Command join(Project p, Symbol s, Layer child, Layer.FigureLayer target, String nodeId, boolean rotate, double t) {
        if (target == child) throw new IllegalArgumentException("a figure cannot join itself");
        for (Layer cur = target; cur != null; cur = cur.parent == null ? null : s.layer(cur.parent)) {
            if (cur == child) throw new IllegalArgumentException("that would join a figure to its own joined figure");
        }
        if (nodeId != null && target.figure.node(nodeId) == null) throw new IllegalArgumentException("no node " + nodeId);
        return change(p, s, child, target.id, nodeId, rotate, t, "Join to " + target.name);
    }

    /** Detach a layer from whatever it is joined to. */
    public static Command unjoin(Project p, Symbol s, Layer child, double t) {
        return change(p, s, child, null, null, true, t, "Unjoin " + child.name);
    }

    private static Command change(final Project p, final Symbol s, final Layer l, final String parent, final String node, final boolean rotate,
                                  final double t, final String label) {
        final Property[] props = {l.transform.x, l.transform.y, l.transform.rotation, l.transform.scaleX, l.transform.scaleY};
        return new Command() {
            Object[] beforeV, afterV;
            List<List<Key>> beforeK, afterK;
            String bParent, bNode;
            boolean bRotate;

            public void apply() {
                if (afterV != null) {
                    set(afterV, afterK);
                    l.parent = parent;
                    l.parentNode = node;
                    l.parentNodeRotate = rotate;
                    return;
                }
                bParent = l.parent;
                bNode = l.parentNode;
                bRotate = l.parentNodeRotate;
                beforeV = new Object[props.length];
                beforeK = new ArrayList<List<Key>>();
                for (int i = 0; i < props.length; i++) {
                    beforeV[i] = props[i].value();
                    beforeK.add(new ArrayList<Key>(props[i].track().keys()));
                }
                // where the layer is on screen at every time that matters, before the change
                TreeSet<Double> times = new TreeSet<Double>();
                times.add(t);
                for (Property q : props) for (Key k : q.track().keys()) times.add(k.time);
                java.util.Map<Double, Affine> world = new java.util.HashMap<Double, Affine>();
                for (double tt : times) world.put(tt, Evaluator.parentFrame(p, s, l, tt).mul(local(p, l, tt)));
                l.parent = parent;
                l.parentNode = node;
                l.parentNodeRotate = rotate;
                // the same placement, expressed in the new frame. A rotated joint mixes the
                // axes (motion keyed on x may become x and y), so when anything was keyed,
                // every transform property gets a key at every key time.
                boolean keyed = times.size() > 1 || anyKeys(props);
                java.util.Map<Double, Key> template = new java.util.HashMap<Double, Key>();
                for (Property q : props) for (Key k : q.track().keys()) if (!template.containsKey(k.time)) template.put(k.time, k);
                java.util.Map<Double, Affine> local = new java.util.HashMap<Double, Affine>();
                for (double tt : times) local.put(tt, Evaluator.parentFrame(p, s, l, tt).inverse().mul(world.get(tt)));
                for (int i = 0; i < props.length; i++) {
                    props[i].track().clear();
                    if (keyed) {
                        for (Key k : template.values()) props[i].track().put(new Key(k.time, part(i, l, local.get(k.time)), k.interp, k.ease));
                    }
                    props[i].setValue(part(i, l, local.get(t)));
                }
                afterV = new Object[props.length];
                afterK = new ArrayList<List<Key>>();
                for (int i = 0; i < props.length; i++) {
                    afterV[i] = props[i].value();
                    afterK.add(new ArrayList<Key>(props[i].track().keys()));
                }
            }

            public void revert() {
                set(beforeV, beforeK);
                l.parent = bParent;
                l.parentNode = bNode;
                l.parentNodeRotate = bRotate;
            }

            private void set(Object[] v, List<List<Key>> k) {
                for (int i = 0; i < props.length; i++) {
                    props[i].setValue(v[i]);
                    props[i].track().clear();
                    for (Key key : k.get(i)) props[i].track().put(key);
                }
            }

            public String label() {
                return label;
            }
        };
    }

    private static boolean anyKeys(Property[] ps) {
        for (Property q : ps) if (!q.track().isEmpty()) return true;
        return false;
    }

    private static Affine local(Project p, Layer l, double t) {
        PropertyResolver r = new PropertyResolver(p, new ArrayList<Diagnostic>());
        return Affine.translate(r.f(l.transform.x, t), r.f(l.transform.y, t))
                .mul(Affine.rotateDeg(r.f(l.transform.rotation, t)))
                .mul(Affine.scale(r.f(l.transform.scaleX, t), r.f(l.transform.scaleY, t)))
                .mul(Affine.translate(-r.f(l.transform.anchorX, t), -r.f(l.transform.anchorY, t)));
    }

    /**
     * One of x, y, rotation, scaleX, scaleY from a local matrix (no skew; a
     * mirror shows up as a negative scaleY), with the layer's anchor accounted for.
     */
    private static Float part(int i, Layer l, Affine m) {
        double sx = Math.hypot(m.a, m.b);
        double rot = Math.toDegrees(Math.atan2(m.b, m.a));
        double sy = sx < 1e-12 ? 0 : (m.a * m.d - m.b * m.c) / sx;
        double ax = l.transform.anchorX.floatValue(), ay = l.transform.anchorY.floatValue();
        double x = m.tx + m.a * ax + m.c * ay, y = m.ty + m.b * ax + m.d * ay;
        // keep rotation continuous with the old value
        double old = l.transform.rotation.floatValue();
        rot += 360 * Math.round((old - rot) / 360);
        switch (i) {
            case 0: return (float) x;
            case 1: return (float) y;
            case 2: return (float) rot;
            case 3: return (float) sx;
            default: return (float) sy;
        }
    }
}
