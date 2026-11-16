package app.barkfart.eval;

import app.barkfart.model.Color;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Figure;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayDeque;
import java.util.HashSet;
import java.util.List;

/**
 * evaluate(project, symbol, t) builds a Scene. Pure: reads the model, never
 * writes it, keeps no state between calls. Symbol nesting uses an explicit
 * work stack (never Java recursion), so self-referencing symbols cannot
 * overflow the call stack. See DESIGN.md §4.
 */
public final class Evaluator {
    /** Nesting levels allowed in total, whatever instances say. */
    public static final int DEPTH_CAP = 64;
    /** Maximum scene objects (shapes plus groups) per scene; beyond it, content is dropped with a diagnostic. */
    public static final int PRIMITIVE_BUDGET = 250_000;

    private Evaluator() {}

    /** Immutable linked list of the symbols on the current nesting path. */
    private static final class Path {
        final String symbol;
        final Path up;
        final int depth;

        Path(String symbol, Path up) {
            this.symbol = symbol;
            this.up = up;
            this.depth = up == null ? 0 : up.depth + 1;
        }

        int occurrences(String s) {
            int n = 0;
            for (Path p = this; p != null; p = p.up) if (p.symbol.equals(s)) n++;
            return n;
        }
    }

    private static final class Work {
        final Symbol symbol;
        final double time;
        final Affine xf;
        final Group into;
        final Path path;

        Work(Symbol symbol, double time, Affine xf, Group into, Path path) {
            this.symbol = symbol;
            this.time = time;
            this.xf = xf;
            this.into = into;
            this.path = path;
        }
    }

    public static Scene evaluate(Project project, String symbolId, double t) {
        Symbol sym = project.library.get(symbolId);
        Group root = new Group(symbolId);
        Scene scene = new Scene(root, t);
        if (sym == null) {
            scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.MISSING_REF, symbolId));
            return scene;
        }
        PropertyResolver r = new PropertyResolver(project, scene.diagnostics);
        Affine view = cameraView(sym, t, r);
        ArrayDeque<Work> stack = new ArrayDeque<Work>();
        stack.push(new Work(sym, t, view, root, new Path(sym.id, null)));
        while (!stack.isEmpty()) {
            Work w = stack.pop();
            evalSymbol(project, w, r, scene, stack);
        }
        return scene;
    }

    /** View transform from the topmost visible camera layer of the evaluated symbol. */
    static Affine cameraView(Symbol sym, double t, PropertyResolver r) {
        for (int i = sym.layers.size() - 1; i >= 0; i--) {
            Layer l = sym.layers.get(i);
            if (!(l instanceof Layer.CameraLayer) || !l.visible || !r.b(l.shown, t)) continue;
            Layer.CameraLayer c = (Layer.CameraLayer) l;
            double zoom = r.f(c.zoom, t);
            if (!(zoom > 1e-6)) zoom = 1e-6;
            return Affine.scale(zoom, zoom).mul(Affine.rotateDeg(-r.f(c.rotation, t)))
                    .mul(Affine.translate(-r.f(c.x, t), -r.f(c.y, t)));
        }
        return Affine.IDENTITY;
    }

    private static void evalSymbol(Project project, Work w, PropertyResolver r, Scene scene, ArrayDeque<Work> stack) {
        Symbol sym = w.symbol;
        double t = w.time;
        boolean anySolo = false;
        for (Layer l : sym.layers) anySolo |= l.solo;
        for (Layer l : sym.layers) {
            if (!l.visible) continue;
            Layer.Type type = l.type();
            if (type == Layer.Type.CAMERA || type == Layer.Type.AUDIO) continue;
            if (anySolo && !l.solo && type != Layer.Type.ADJUSTMENT) continue;
            if (!r.b(l.shown, t)) continue;
            // Every group costs one budget unit, so scene size is bounded even for
            // symbols that nest themselves many times.
            if (!spend(scene, l.id)) break;
            Group g = new Group(sym.id + "/" + l.id);
            float op = r.f(l.opacity, t);
            g.opacity = op < 0 ? 0 : (op > 1 ? 1 : op);
            g.blend = l.blend;
            resolveEffects(l, t, r, g);
            Affine xf = w.xf.mul(layerChain(sym, l, t, r, scene));
            if (!xf.isFinite()) {
                scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.BAD_VALUE, l.id + "/transform"));
                continue;
            }
            switch (type) {
                case ADJUSTMENT:
                    g.adjustment = true;
                    w.into.children.add(g);
                    break;
                case FIGURE:
                    w.into.children.add(g);
                    emitFigure(((Layer.FigureLayer) l), xf, t, r, scene, g);
                    break;
                case SPRITE:
                    w.into.children.add(g);
                    emitSprite((Layer.SpriteLayer) l, xf, t, r, scene, g);
                    break;
                case SYMBOL:
                    w.into.children.add(g);
                    pushInstance(project, (Layer.SymbolLayer) l, xf, t, r, scene, g, w.path, stack);
                    break;
                default:
                    // TEXT renders in a later milestone; it contributes no shapes yet.
                    break;
            }
        }
    }

    private static void resolveEffects(Layer l, double t, PropertyResolver r, Group g) {
        if (l.effects.isEmpty()) return;
        for (EffectNode e : l.effects.chain()) {
            if (!e.enabled) continue;
            ResolvedEffect re = new ResolvedEffect(e.type, r.f(e.mix, t));
            for (java.util.Map.Entry<String, Property> p : e.params.entrySet()) {
                re.params.put(p.getKey(), r.resolve(p.getValue(), t));
            }
            g.effects.add(re);
        }
    }

    /** Local transform of a layer composed with its parent chain (parents applied outermost). */
    private static Affine layerChain(Symbol sym, Layer l, double t, PropertyResolver r, Scene scene) {
        Affine acc = local(l, t, r);
        HashSet<String> seen = new HashSet<String>();
        seen.add(l.id);
        Layer cur = l;
        while (cur.parent != null) {
            Layer p = sym.layer(cur.parent);
            if (p == null) {
                scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.MISSING_REF, cur.id + "/parent"));
                break;
            }
            if (!seen.add(p.id)) {
                scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.PARENT_CYCLE, l.id));
                break;
            }
            acc = local(p, t, r).mul(acc);
            cur = p;
        }
        return acc;
    }

    private static Affine local(Layer l, double t, PropertyResolver r) {
        return Affine.translate(r.f(l.transform.x, t), r.f(l.transform.y, t))
                .mul(Affine.rotateDeg(r.f(l.transform.rotation, t)))
                .mul(Affine.scale(r.f(l.transform.scaleX, t), r.f(l.transform.scaleY, t)))
                .mul(Affine.translate(-r.f(l.transform.anchorX, t), -r.f(l.transform.anchorY, t)));
    }

    private static boolean spend(Scene scene, String what) {
        if (scene.primitives >= PRIMITIVE_BUDGET) {
            if (!scene.has(Diagnostic.Kind.PRIMITIVE_BUDGET)) {
                scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.PRIMITIVE_BUDGET, what));
            }
            return false;
        }
        scene.primitives++;
        return true;
    }

    /** Forward kinematics in hierarchy order, then shapes emitted in draw order. */
    static void emitFigure(Layer.FigureLayer layer, Affine xf, double t, PropertyResolver r, Scene scene, Group g) {
        Figure f = layer.figure;
        List<Node> nodes = f.nodes();
        int n = nodes.size();
        double[] px = new double[n], py = new double[n], ang = new double[n];
        int[] parent = new int[n];
        for (int i = 0; i < n; i++) {
            Node node = nodes.get(i);
            double rot = r.f(node.rotation, t);
            if (node.parent == null) {
                parent[i] = -1;
                ang[i] = rot;
                continue;
            }
            int pi = f.indexOf(node.parent);
            parent[i] = pi;
            ang[i] = ang[pi] + rot;
            double len = r.f(node.length, t) * r.f(node.scale, t);
            double rad = Math.toRadians(ang[i]);
            px[i] = px[pi] + len * Math.cos(rad);
            py[i] = py[pi] + len * Math.sin(rad);
        }
        double sf = xf.scaleFactor();
        for (int i : f.drawSequence()) {
            Node node = nodes.get(i);
            int pi = parent[i];
            if (pi < 0 || node.shape == Node.Shape.NONE || !r.b(node.shown, t)) continue;
            if (!spend(scene, layer.id)) return;
            Color c = (Color) r.resolve(node.color, t);
            String src = layer.id + "/" + node.id;
            float ax = (float) xf.mapX(px[pi], py[pi]), ay = (float) xf.mapY(px[pi], py[pi]);
            float bx = (float) xf.mapX(px[i], py[i]), by = (float) xf.mapY(px[i], py[i]);
            if (node.shape == Node.Shape.LINE) {
                float w = (float) (Math.abs(r.f(node.thickness, t)) * sf);
                g.children.add(new Shape.Line(ax, ay, bx, by, w, c, src));
            } else {
                float cx = (ax + bx) / 2, cy = (ay + by) / 2;
                float rad2 = (float) (Math.hypot(bx - ax, by - ay) / 2);
                g.children.add(new Shape.Circle(cx, cy, rad2, c, src));
            }
        }
    }

    private static void emitSprite(Layer.SpriteLayer l, Affine xf, double t, PropertyResolver r, Scene scene, Group g) {
        if (l.asset == null || !spend(scene, l.id)) return;
        float w = l.width, h = l.height;
        double[][] pts = {{0, 0}, {w, 0}, {w, h}, {0, h}};
        float[] c = new float[8];
        for (int i = 0; i < 4; i++) {
            c[i * 2] = (float) xf.mapX(pts[i][0], pts[i][1]);
            c[i * 2 + 1] = (float) xf.mapY(pts[i][0], pts[i][1]);
        }
        g.children.add(new Shape.Quad(l.asset, c, (Color) r.resolve(l.tint, t), l.id));
    }

    private static void pushInstance(Project project, Layer.SymbolLayer l, Affine xf, double t, PropertyResolver r,
            Scene scene, Group g, Path path, ArrayDeque<Work> stack) {
        Symbol target = project.library.get(l.symbol);
        if (target == null) {
            scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.MISSING_REF, l.id + "->" + l.symbol));
            return;
        }
        if (path.depth + 1 >= DEPTH_CAP) {
            scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.DEPTH_CAP, l.id));
            return;
        }
        // Recursion depth = how many times the target is already on the path.
        if (path.occurrences(target.id) > Math.max(0, l.recursionLimit)) {
            scene.diagnostics.add(new Diagnostic(Diagnostic.Kind.RECURSION_LIMIT, l.id));
            return;
        }
        double local = localTime(r.d(l.timeOffset, t) + t * r.d(l.speed, t), target.duration, l.loop);
        stack.push(new Work(target, local, xf, g, new Path(target.id, path)));
    }

    /** Applies the instance loop mode against the symbol duration. */
    public static double localTime(double lt, double duration, Layer.SymbolLayer.LoopMode mode) {
        if (Double.isNaN(lt) || Double.isInfinite(lt)) return 0;
        if (!(duration > 0)) return Math.max(0, Math.min(lt, 0));
        switch (mode) {
            case ONCE:
                return lt < 0 ? 0 : (lt > duration ? duration : lt);
            case LOOP: {
                double m = lt % duration;
                return m < 0 ? m + duration : m;
            }
            default: {
                double p = lt % (2 * duration);
                if (p < 0) p += 2 * duration;
                return p <= duration ? p : 2 * duration - p;
            }
        }
    }
}
