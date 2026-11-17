package app.barkfart.edit;

import app.barkfart.eval.Affine;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Polyfill;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Movieclip operations (Stick Nodes 3.x / 4.0): turn layers into a symbol
 * with one instance in their place (the animation looks the same), and
 * break an instance apart into plain layers posed as it shows them now.
 * One undo step each.
 */
public final class SymbolOps {
    private SymbolOps() {}

    /** The symbol / layers made by the last call (for selecting them). */
    public static final class Made {
        public Symbol symbol;
        public Layer instance;
        public final List<Layer> layers = new ArrayList<Layer>();
    }

    /** Snapshot of a symbol's layer list and the library, restored on undo. */
    private abstract static class ListEdit implements Command {
        private final Project p;
        private final Symbol s;
        private final String label;
        private List<Layer> before, after;
        private final List<String> libBefore = new ArrayList<String>();
        private Symbol added;

        ListEdit(Project p, Symbol s, String label) {
            this.p = p;
            this.s = s;
            this.label = label;
        }

        abstract Symbol change();

        public void apply() {
            if (after != null) {
                s.layers.clear();
                s.layers.addAll(after);
                if (added != null) p.library.put(added.id, added);
                p.invalidateIndex();
                return;
            }
            before = new ArrayList<Layer>(s.layers);
            libBefore.addAll(p.library.keySet());
            added = change();
            after = new ArrayList<Layer>(s.layers);
            p.invalidateIndex();
        }

        public void revert() {
            s.layers.clear();
            s.layers.addAll(before);
            if (added != null) p.library.remove(added.id);
            p.invalidateIndex();
        }

        public String label() {
            return label;
        }
    }

    /**
     * Moves the layers (in their order) into a new symbol and puts one looping
     * instance where the first of them was. Layers joined across the edge of
     * the selection are refused (the join would break).
     */
    public static Command convertToSymbol(final Project p, final Symbol s, final List<Layer> layers, final String name, final Made made) {
        if (layers.isEmpty()) throw new IllegalArgumentException("nothing selected");
        final Set<String> ids = new HashSet<String>();
        for (Layer l : layers) {
            if (!s.layers.contains(l)) throw new IllegalArgumentException(l.name + " is not in this symbol");
            ids.add(l.id);
        }
        for (Layer l : s.layers) {
            boolean in = ids.contains(l.id);
            if (l.parent != null && in != ids.contains(l.parent)) {
                throw new IllegalArgumentException(l.name + " is joined across the selection; include both or unjoin first");
            }
        }
        return new ListEdit(p, s, "Make movieclip " + name) {
            Symbol change() {
                Symbol sym = new Symbol(p.newId("S"), name, s.duration);
                int at = s.layers.size();
                List<Layer> ordered = new ArrayList<Layer>();
                for (Layer l : s.layers) {
                    if (ids.contains(l.id)) {
                        at = Math.min(at, s.layers.indexOf(l));
                        ordered.add(l);
                    }
                }
                s.layers.removeAll(ordered);
                sym.layers.addAll(ordered);
                p.library.put(sym.id, sym);
                Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), name, sym.id);
                inst.loop = Layer.SymbolLayer.LoopMode.LOOP;
                s.layers.add(Math.min(at, s.layers.size()), inst);
                if (made != null) {
                    made.symbol = sym;
                    made.instance = inst;
                }
                return sym;
            }
        };
    }

    /**
     * Replaces an instance by copies of its symbol's figure and sprite layers,
     * posed as the instance shows them at time t (static values: the
     * movieclip's motion is not carried over), with the instance's placement
     * and opacity baked in. Other layer types inside are skipped.
     */
    public static Command breakApart(final Project p, final Symbol s, final Layer.SymbolLayer inst, final double t, final Made made) {
        final Symbol target = p.library.get(inst.symbol);
        if (target == null) throw new IllegalArgumentException("the movieclip's symbol is missing");
        if (!s.layers.contains(inst)) throw new IllegalArgumentException("not in this symbol");
        return new ListEdit(p, s, "Break apart " + inst.name) {
            Symbol change() {
                PropertyResolver r = new PropertyResolver(p, new ArrayList<Diagnostic>());
                double inner = Evaluator.instanceTime(inst, target, t, r);
                Affine place = Evaluator.layerStage(p, s, inst, t);
                float opacity = r.f(inst.opacity, t);
                int at = s.layers.indexOf(inst);
                s.layers.remove(inst);
                List<Layer> out = new ArrayList<Layer>();
                for (Layer l : target.layers) {
                    Layer c;
                    if (l instanceof Layer.FigureLayer) c = copyFigure(p, (Layer.FigureLayer) l, inner, r);
                    else if (l instanceof Layer.SpriteLayer) c = copySprite(p, (Layer.SpriteLayer) l, inner, r);
                    else continue;
                    Affine m = place.mul(Evaluator.layerStage(p, target, l, inner));
                    setTransform(c, m);
                    c.opacity.setValue(Math.max(0, Math.min(1, opacity * r.f(l.opacity, inner))));
                    c.shown.setValue(r.b(l.shown, inner));
                    c.blend = l.blend;
                    c.visible = l.visible;
                    copyEffects(l, c, inner, r);
                    out.add(c);
                }
                s.layers.addAll(Math.min(at, s.layers.size()), out);
                if (made != null) made.layers.addAll(out);
                return null;
            }
        };
    }

    private static Layer.FigureLayer copyFigure(Project p, Layer.FigureLayer src, double t, PropertyResolver r) {
        Layer.FigureLayer c = new Layer.FigureLayer(p.newId("L"), src.name);
        for (Node n : src.figure.nodes()) {
            Node m = new Node(c.id, n.id, n.parent, n.shape);
            m.drawOrder = n.drawOrder;
            m.curve = n.curve;
            m.triangleUpsideDown = n.triangleUpsideDown;
            m.rightTriangle = n.rightTriangle;
            m.gradient = n.gradient;
            m.gradientReverse = n.gradientReverse;
            m.gradientSideways = n.gradientSideways;
            m.isStatic = n.isStatic;
            m.stretchy = n.stretchy;
            m.smartStretch = n.smartStretch;
            m.noSmartStretch = n.noSmartStretch;
            m.restLength = n.restLength;
            m.circleHollow = n.circleHollow;
            m.circleOutline = n.circleOutline;
            m.halfArc = n.halfArc;
            m.polygonSides = n.polygonSides;
            m.trapezoidRatio = n.trapezoidRatio;
            m.trapezoidRoundStart = n.trapezoidRoundStart;
            m.trapezoidRoundEnd = n.trapezoidRoundEnd;
            m.angleLock = n.angleLock;
            m.dragLock = n.dragLock;
            m.pinned = n.pinned;
            Property[] from = n.properties(), to = m.properties();
            for (int i = 0; i < from.length; i++) to[i].setValue(r.resolve(from[i], t));
            c.figure.add(m);
        }
        for (Polyfill pf : src.figure.polyfills) {
            Polyfill q = new Polyfill(c.id, pf.id);
            q.anchor = pf.anchor;
            q.nodes.addAll(pf.nodes);
            q.useOwnColor = pf.useOwnColor;
            q.color.setValue(r.resolve(pf.color, t));
            c.figure.polyfills.add(q);
        }
        return c;
    }

    private static Layer.SpriteLayer copySprite(Project p, Layer.SpriteLayer src, double t, PropertyResolver r) {
        Layer.SpriteLayer c = new Layer.SpriteLayer(p.newId("L"), src.name);
        c.asset = src.asset;
        c.width = src.width;
        c.height = src.height;
        c.tint.setValue(r.resolve(src.tint, t));
        return c;
    }

    private static void copyEffects(Layer from, Layer to, double t, PropertyResolver r) {
        for (EffectNode e : from.effects.chain()) {
            EffectNode n = new EffectNode(to.id, e.id, e.type);
            n.enabled = e.enabled;
            n.mix.setValue(r.resolve(e.mix, t));
            for (java.util.Map.Entry<String, Property> q : e.params.entrySet()) {
                n.param(to.id, q.getKey(), q.getValue().type, r.resolve(q.getValue(), t));
            }
            to.effects.append(n);
        }
    }

    /** Sets x, y, rotation and scale (anchor 0) from a matrix without skew. */
    static void setTransform(Layer c, Affine m) {
        double sx = Math.hypot(m.a, m.b);
        double rot = Math.toDegrees(Math.atan2(m.b, m.a));
        double sy = sx < 1e-12 ? 0 : (m.a * m.d - m.b * m.c) / sx;
        c.transform.anchorX.setValue(0f);
        c.transform.anchorY.setValue(0f);
        c.transform.x.setValue((float) m.tx);
        c.transform.y.setValue((float) m.ty);
        c.transform.rotation.setValue((float) rot);
        c.transform.scaleX.setValue((float) sx);
        c.transform.scaleY.setValue((float) sy);
    }
}
