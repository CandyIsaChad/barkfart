package app.barkfart.edit;

import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Figure tools on whole layers: duplicate (with the figures joined to it),
 * push back / bring forward together with joined figures, and copy / paste
 * all of a figure's properties. One undo step each.
 */
public final class LayerOps {
    private LayerOps() {}

    /** The layer and every layer joined to it (directly or through others), in stack order. */
    public static List<Layer> withJoined(Symbol s, Layer l) {
        List<Layer> out = new ArrayList<Layer>();
        for (Layer x : s.layers) {
            for (Layer cur = x; cur != null; cur = cur.parent == null ? null : s.layer(cur.parent)) {
                if (cur == l) {
                    out.add(x);
                    break;
                }
                if (out.size() > s.layers.size()) break;
            }
        }
        return out;
    }

    private abstract static class StackEdit implements Command {
        private final Symbol s;
        private final Project p;
        private final String label;
        private List<Layer> before, after;

        StackEdit(Project p, Symbol s, String label) {
            this.p = p;
            this.s = s;
            this.label = label;
        }

        abstract void change() throws Exception;

        public void apply() {
            if (after != null) {
                s.layers.clear();
                s.layers.addAll(after);
                p.invalidateIndex();
                return;
            }
            before = new ArrayList<Layer>(s.layers);
            try {
                change();
            } catch (Exception e) {
                s.layers.clear();
                s.layers.addAll(before);
                throw new IllegalStateException(e.getMessage(), e);
            }
            after = new ArrayList<Layer>(s.layers);
            p.invalidateIndex();
        }

        public void revert() {
            s.layers.clear();
            s.layers.addAll(before);
            p.invalidateIndex();
        }

        public String label() {
            return label;
        }
    }

    /** Copies made by the last duplicate (the first one is the copy of the layer itself). */
    public static final class Made {
        public final List<Layer> layers = new ArrayList<Layer>();
    }

    /**
     * Duplicates a layer (and, with joined, the figures joined to it, still
     * joined to the copy), placed above the originals and moved by (dx, dy).
     */
    public static Command duplicate(final Project p, final Symbol s, final Layer l, final boolean joined, final float dx, final float dy, final Made made) {
        return new StackEdit(p, s, "Duplicate " + l.name) {
            void change() throws Exception {
                List<Layer> block = joined ? withJoined(s, l) : java.util.Collections.singletonList(l);
                Map<String, String> ids = new HashMap<String, String>();
                for (Layer x : block) ids.put(x.id, p.newId("L"));
                int at = 0;
                for (Layer x : block) at = Math.max(at, s.layers.indexOf(x) + 1);
                List<Layer> copies = new ArrayList<Layer>();
                for (Layer x : block) {
                    Layer c = ProjectCodec.copyLayer(x, ids);
                    if (x == l) {
                        if (!joined) c.parent = l.parent;   // a lone copy stays joined where the original is
                        offset(c.transform.x, dx);
                        offset(c.transform.y, dy);
                    }
                    copies.add(c);
                }
                s.layers.addAll(Math.min(at, s.layers.size()), copies);
                if (made != null) made.layers.addAll(copies);
            }
        };
    }

    private static void offset(Property q, float d) {
        if (d == 0) return;
        q.setValue(q.floatValue() + d);
        List<Key> ks = new ArrayList<Key>(q.track().keys());
        q.track().clear();
        for (Key k : ks) q.track().put(new Key(k.time, (Float) k.value + d, k.interp, k.ease));
    }

    /**
     * Push back / bring forward (Stick Nodes 4.0.0): the layer and the figures
     * joined to it move together, past the next layer that is not part of them.
     */
    public static Command move(final Project p, final Symbol s, final Layer l, final boolean forward) {
        return new StackEdit(p, s, forward ? "Bring forward" : "Push back") {
            void change() {
                List<Layer> block = withJoined(s, l);
                List<Layer> rest = new ArrayList<Layer>(s.layers);
                rest.removeAll(block);
                int before = 0;
                for (Layer x : s.layers) {
                    if (block.contains(x)) break;
                    before++;
                }
                int to = Math.max(0, Math.min(rest.size(), before + (forward ? 1 : -1)));
                List<Layer> out = new ArrayList<Layer>(rest.subList(0, to));
                out.addAll(block);
                out.addAll(rest.subList(to, rest.size()));
                s.layers.clear();
                s.layers.addAll(out);
            }
        };
    }

    /** A figure's properties at one time ("Copy all figure properties", 2.5.9). */
    public static final class Props {
        final float[] transform = new float[5];
        float opacity;
        Layer.Blend blend;
        final List<EffectNode> effects = new ArrayList<EffectNode>();
        String sourceId;
        int stackIndex;
    }

    public static Props copyProps(Project p, Symbol s, Layer l, double t) {
        PropertyResolver r = new PropertyResolver(p, new ArrayList<Diagnostic>());
        Props c = new Props();
        Property[] tr = {l.transform.x, l.transform.y, l.transform.rotation, l.transform.scaleX, l.transform.scaleY};
        for (int i = 0; i < 5; i++) c.transform[i] = r.f(tr[i], t);
        c.opacity = r.f(l.opacity, t);
        c.blend = l.blend;
        for (EffectNode e : l.effects.chain()) {
            EffectNode n = new EffectNode("clip", e.id, e.type);
            n.enabled = e.enabled;
            n.mix.setValue(r.resolve(e.mix, t));
            for (Map.Entry<String, Property> q : e.params.entrySet()) n.param("clip", q.getKey(), q.getValue().type, r.resolve(q.getValue(), t));
            c.effects.add(n);
        }
        c.sourceId = l.id;
        c.stackIndex = s.layers.indexOf(l);
        return c;
    }

    /**
     * Pastes properties onto a layer at time t (keyed like any pose edit):
     * position, rotation, scale, opacity, blend and effects; with layer,
     * also its place in the stack (2.6.1). One undo step.
     */
    public static void pasteProps(Editor editor, Symbol s, Layer l, Props c, boolean position, boolean layer, double t) {
        Document doc = editor.doc;
        doc.begin("Paste properties");
        try {
            Property[] tr = {l.transform.x, l.transform.y, l.transform.rotation, l.transform.scaleX, l.transform.scaleY};
            for (int i = position ? 0 : 2; i < 5; i++) editor.change(tr[i], c.transform[i], t);
            editor.change(l.opacity, c.opacity, t);
            if (l.blend != c.blend) doc.apply(StructureEdits.setBlend(l, c.blend));
            for (EffectNode e : new ArrayList<EffectNode>(l.effects.chain())) doc.apply(StructureEdits.removeEffect(l, e));
            for (EffectNode e : c.effects) {
                EffectNode n = new EffectNode(l.id, doc.project.newId("FX"), e.type);
                n.enabled = e.enabled;
                n.mix.setValue(e.mix.value());
                for (Map.Entry<String, Property> q : e.params.entrySet()) n.param(l.id, q.getKey(), q.getValue().type, q.getValue().value());
                doc.apply(StructureEdits.addEffect(l, n));
            }
            if (layer) {
                int to = Math.max(0, Math.min(s.layers.size() - 1, c.stackIndex));
                if (s.layers.indexOf(l) != to) doc.apply(StructureEdits.moveLayer(s, l, to));
            }
            doc.project.invalidateIndex();
        } finally {
            doc.commit();
        }
    }
}
