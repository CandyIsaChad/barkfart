package app.barkfart.edit;

import app.barkfart.model.Layer;
import app.barkfart.model.Marker;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Polyfill;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;

/**
 * Undoable structural edits. All operate in place, so object identities (and
 * any selection holding them) survive undo/redo, and undo restores list
 * positions exactly.
 */
public final class StructureEdits {
    private StructureEdits() {}

    private abstract static class Named implements Command {
        private final String label;

        Named(String label) {
            this.label = label;
        }

        public String label() {
            return label;
        }
    }

    // ---- layers ----

    public static Command addLayer(final Symbol s, final Layer l, final int index) {
        return new Named("Add layer " + l.name) {
            public void apply() {
                s.layers.add(Math.min(Math.max(0, index), s.layers.size()), l);
            }

            public void revert() {
                s.layers.remove(l);
            }
        };
    }

    public static Command removeLayer(final Symbol s, final Layer l) {
        final int index = s.layers.indexOf(l);
        if (index < 0) throw new IllegalArgumentException("layer not in symbol");
        return new Named("Delete layer " + l.name) {
            public void apply() {
                s.layers.remove(l);
            }

            public void revert() {
                s.layers.add(index, l);
            }
        };
    }

    /** Moves a layer to a new index (0 = bottom). */
    public static Command moveLayer(final Symbol s, final Layer l, final int to) {
        final int from = s.layers.indexOf(l);
        if (from < 0) throw new IllegalArgumentException("layer not in symbol");
        final int target = Math.min(Math.max(0, to), s.layers.size() - 1);
        return new Named("Move layer " + l.name) {
            public void apply() {
                s.layers.remove(l);
                s.layers.add(target, l);
            }

            public void revert() {
                s.layers.remove(l);
                s.layers.add(from, l);
            }
        };
    }

    public enum Flag { VISIBLE, LOCKED, SOLO, ONION }

    public static Command setFlag(final Layer l, final Flag f, final boolean on) {
        final boolean before = get(l, f);
        return new Named((on ? "Turn on " : "Turn off ") + f.name().toLowerCase() + " " + l.name) {
            public void apply() {
                set(l, f, on);
            }

            public void revert() {
                set(l, f, before);
            }
        };
    }

    private static boolean get(Layer l, Flag f) {
        return f == Flag.VISIBLE ? l.visible : f == Flag.LOCKED ? l.locked : f == Flag.SOLO ? l.solo : l.onion;
    }

    private static void set(Layer l, Flag f, boolean v) {
        if (f == Flag.VISIBLE) l.visible = v;
        else if (f == Flag.LOCKED) l.locked = v;
        else if (f == Flag.SOLO) l.solo = v;
        else l.onion = v;
    }

    public static Command setBlend(final Layer l, final Layer.Blend blend) {
        final Layer.Blend before = l.blend;
        return new Named("Blend " + l.name) {
            public void apply() {
                l.blend = blend;
            }

            public void revert() {
                l.blend = before;
            }
        };
    }

    public static Command rename(final Layer l, final String name) {
        final String before = l.name;
        return new Named("Rename layer") {
            public void apply() {
                l.name = name;
            }

            public void revert() {
                l.name = before;
            }
        };
    }

    /** Sets the parent layer; refuses a parent that would create a cycle. */
    public static Command setParent(final Symbol s, final Layer l, final String parentId) {
        String p = parentId;
        int guard = s.layers.size() + 1;
        while (p != null && guard-- > 0) {
            if (p.equals(l.id)) throw new IllegalArgumentException("parenting would create a cycle");
            Layer pl = s.layer(p);
            p = pl == null ? null : pl.parent;
        }
        final String before = l.parent;
        return new Named("Parent " + l.name) {
            public void apply() {
                l.parent = parentId;
            }

            public void revert() {
                l.parent = before;
            }
        };
    }

    // ---- figure structure ----

    public static Command addNode(final Layer.FigureLayer l, final Node n) {
        if (l.figure.node(n.parent) == null) throw new IllegalArgumentException("unknown parent " + n.parent);
        return new Named("Add node") {
            int[] pos;
            final List<Node> one = new ArrayList<Node>();

            public void apply() {
                if (pos == null) {
                    l.figure.add(n);
                    one.clear();
                    one.add(n);
                    pos = new int[] {l.figure.indexOf(n.id)};
                } else {
                    l.figure.restore(one, pos);
                }
            }

            public void revert() {
                l.figure.removeSubtree(n.id);
            }
        };
    }

    /** Removes a node, its descendants, and polyfills that use any of them. */
    public static Command removeNode(final Layer.FigureLayer l, final String nodeId) {
        final List<Node> sub = l.figure.subtree(nodeId);
        if (sub.isEmpty() || sub.get(0).parent == null) throw new IllegalArgumentException("cannot remove " + nodeId);
        final java.util.HashSet<String> ids = new java.util.HashSet<String>();
        for (Node n : sub) ids.add(n.id);
        final List<Polyfill> fills = new ArrayList<Polyfill>();
        final List<Integer> fillPos = new ArrayList<Integer>();
        for (int i = 0; i < l.figure.polyfills.size(); i++) {
            Polyfill pf = l.figure.polyfills.get(i);
            boolean uses = ids.contains(pf.anchor);
            for (String x : pf.nodes) uses |= ids.contains(x);
            if (uses) {
                fills.add(pf);
                fillPos.add(i);
            }
        }
        return new Named("Delete node") {
            int[] pos;

            public void apply() {
                pos = l.figure.removeSubtree(nodeId);
                l.figure.polyfills.removeAll(fills);
            }

            public void revert() {
                l.figure.restore(sub, pos);
                for (int i = 0; i < fills.size(); i++) l.figure.polyfills.add(fillPos.get(i), fills.get(i));
            }
        };
    }

    public static Command setShape(final Node n, final Node.Shape shape) {
        final Node.Shape before = n.shape;
        return new Named("Segment shape") {
            public void apply() {
                n.shape = shape;
            }

            public void revert() {
                n.shape = before;
            }
        };
    }

    /** Layering inside a figure (push back / pull forward). */
    public static Command setDrawOrder(final Node n, final int order) {
        final int before = n.drawOrder;
        return new Named("Layering") {
            public void apply() {
                n.drawOrder = order;
            }

            public void revert() {
                n.drawOrder = before;
            }
        };
    }

    public static Command addPolyfill(final Layer.FigureLayer l, final Polyfill pf) {
        return new Named("Add polyfill") {
            public void apply() {
                l.figure.polyfills.add(pf);
            }

            public void revert() {
                l.figure.polyfills.remove(pf);
            }
        };
    }

    public static Command removePolyfill(final Layer.FigureLayer l, final Polyfill pf) {
        final int i = l.figure.polyfills.indexOf(pf);
        if (i < 0) throw new IllegalArgumentException("polyfill not in figure");
        return new Named("Delete polyfill") {
            public void apply() {
                l.figure.polyfills.remove(pf);
            }

            public void revert() {
                l.figure.polyfills.add(i, pf);
            }
        };
    }

    // ---- modifiers ----

    public static Command addModifier(final Property p, final Modifier m) {
        return new Named("Add modifier") {
            int at;

            public void apply() {
                p.addModifier(m);
                at = p.modifiers().indexOf(m);
            }

            public void revert() {
                p.removeModifier(at);
            }
        };
    }

    public static Command removeModifier(final Property p, final int index) {
        final Modifier m = p.modifiers().get(index);
        return new Named("Delete modifier") {
            public void apply() {
                p.removeModifier(index);
            }

            public void revert() {
                p.insertModifier(index, m);
            }
        };
    }

    public static Command setModifierEnabled(final Modifier m, final boolean on) {
        final boolean before = m.enabled;
        return new Named(on ? "Enable modifier" : "Disable modifier") {
            public void apply() {
                m.enabled = on;
            }

            public void revert() {
                m.enabled = before;
            }
        };
    }

    // ---- markers and symbols ----

    public static Command addMarker(final Symbol s, final Marker m) {
        return new Named("Add marker") {
            public void apply() {
                int i = 0;
                while (i < s.markers.size() && s.markers.get(i).time <= m.time) i++;
                s.markers.add(i, m);
            }

            public void revert() {
                s.markers.remove(m);
            }
        };
    }

    public static Command removeMarker(final Symbol s, final Marker m) {
        final int i = s.markers.indexOf(m);
        if (i < 0) throw new IllegalArgumentException("marker not in symbol");
        return new Named("Delete marker") {
            public void apply() {
                s.markers.remove(m);
            }

            public void revert() {
                s.markers.add(i, m);
            }
        };
    }

    public static Command addSymbol(final Project p, final Symbol s) {
        if (p.library.containsKey(s.id)) throw new IllegalArgumentException("duplicate symbol " + s.id);
        return new Named("Add symbol " + s.name) {
            public void apply() {
                p.library.put(s.id, s);
            }

            public void revert() {
                p.library.remove(s.id);
            }
        };
    }

    public static Command setDuration(final Symbol s, final double seconds) {
        final double before = s.duration;
        return new Named("Duration") {
            public void apply() {
                s.duration = seconds;
            }

            public void revert() {
                s.duration = before;
            }
        };
    }
}
