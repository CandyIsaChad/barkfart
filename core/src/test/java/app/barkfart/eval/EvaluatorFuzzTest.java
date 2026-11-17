package app.barkfart.eval;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import org.junit.jupiter.api.Test;

/**
 * Random projects with self- and mutually-referencing symbols, link and delay
 * cycles, missing references, parent cycles and extreme values. The evaluator
 * must always finish, never throw, and respect its budgets.
 */
class EvaluatorFuzzTest {
    private static final float[] EXTREMES = {0, -0f, 1e-30f, 1e30f, -1e30f, Float.NaN, Float.POSITIVE_INFINITY, 360000f};

    @Test
    void randomProjectsAlwaysEvaluate() {
        assertTimeoutPreemptively(Duration.ofSeconds(240), () -> {
            Random r = new Random(20261116);
            for (int i = 0; i < 400; i++) fuzzOnce(r, i);
        });
    }

    private static void fuzzOnce(Random r, int iter) {
        Project p = Fixtures.empty();
        List<Symbol> syms = new ArrayList<Symbol>();
        syms.add(p.root());
        int extra = r.nextInt(4);
        for (int i = 0; i < extra; i++) syms.add(p.addSymbol(new Symbol(p.newId("S"), "s" + i, r.nextInt(3) * r.nextDouble())));
        List<Property> props = new ArrayList<Property>();
        for (Symbol s : syms) {
            int layers = 1 + r.nextInt(5);
            for (int k = 0; k < layers; k++) {
                Layer l;
                switch (r.nextInt(4)) {
                    case 0:
                    case 1: {
                        Layer.FigureLayer fl = Fixtures.stickman(p, "m");
                        List<Node> ns = fl.figure.nodes();
                        for (Node nd : ns) if (nd.parent != null) nd.shape = Node.Shape.values()[1 + r.nextInt(Node.Shape.values().length - 1)];
                        int fills = r.nextInt(3);
                        for (int q = 0; q < fills; q++) {
                            app.barkfart.model.Polyfill pf = new app.barkfart.model.Polyfill(fl.id, "pf" + q);
                            pf.anchor = ns.get(r.nextInt(ns.size())).id;
                            int m = r.nextInt(8);
                            for (int z = 0; z < m; z++) pf.nodes.add(r.nextInt(10) == 0 ? "missing" : ns.get(r.nextInt(ns.size())).id);
                            pf.useOwnColor = r.nextBoolean();
                            fl.figure.polyfills.add(pf);
                        }
                        l = fl;
                        break;
                    }
                    case 2: {
                        Layer.SymbolLayer sl = new Layer.SymbolLayer(p.newId("L"), "i",
                                r.nextInt(8) == 0 ? "missing" : syms.get(r.nextInt(syms.size())).id);
                        sl.recursionLimit = r.nextInt(12) - 1;
                        sl.loop = Layer.SymbolLayer.LoopMode.values()[r.nextInt(3)];
                        sl.transform.scaleX.setValue(0.3f + r.nextFloat());
                        l = sl;
                        break;
                    }
                    default:
                        l = new Layer.CameraLayer(p.newId("L"), "c");
                }
                int nfx = r.nextInt(4) == 0 ? 1 + r.nextInt(3) : 0;
                for (int q = 0; q < nfx; q++) {
                    String type = r.nextInt(8) == 0 ? "no.such" : app.barkfart.fx.EffectTypes.ALL[r.nextInt(app.barkfart.fx.EffectTypes.ALL.length)];
                    app.barkfart.model.EffectNode e = new app.barkfart.model.EffectNode(l.id, "fx" + q, type);
                    for (String name : new String[] {"radius", "dx", "dy", "strength", "hue", "saturation", "brightness", "amount"}) {
                        if (r.nextBoolean()) e.param(l.id, name, app.barkfart.model.ValueType.FLOAT, r.nextInt(3) == 0 ? EXTREMES[r.nextInt(EXTREMES.length)] : (float) (r.nextGaussian() * 50));
                    }
                    if (r.nextBoolean()) e.mix.setValue(r.nextInt(3) == 0 ? EXTREMES[r.nextInt(EXTREMES.length)] : r.nextFloat());
                    l.effects.append(e);
                }
                if (r.nextInt(5) == 0) l.opacity.setValue(r.nextInt(3) == 0 ? EXTREMES[r.nextInt(EXTREMES.length)] : r.nextFloat());
                l.solo = r.nextInt(10) == 0;
                l.visible = r.nextInt(12) != 0;
                s.layers.add(l);
                props.addAll(l.properties());
            }
        }
        // Random parents, including cycles and dangling ids.
        for (Symbol s : syms) {
            for (Layer l : s.layers) {
                if (r.nextInt(4) == 0) {
                    l.parent = r.nextInt(5) == 0 ? "nope" : s.layers.get(r.nextInt(s.layers.size())).id;
                }
            }
        }
        p.invalidateIndex();
        // Random keys and modifiers, including link/delay cycles and extreme values.
        int edits = r.nextInt(40);
        for (int e = 0; e < edits && !props.isEmpty(); e++) {
            Property prop = props.get(r.nextInt(props.size()));
            Property other = props.get(r.nextInt(props.size()));
            switch (r.nextInt(6)) {
                case 0:
                    prop.addModifier(new Modifier.Link(r.nextInt(10) == 0 ? "missing" : other.id, r.nextFloat() * 4 - 2, r.nextFloat()));
                    break;
                case 1:
                    prop.addModifier(new Modifier.DelayFollow(other.id, r.nextDouble() * 2 - 0.5, 1, 0));
                    break;
                case 2:
                    prop.addModifier(new Modifier.Wiggle(r.nextFloat() * 100, EXTREMES[r.nextInt(EXTREMES.length)], r.nextInt(), r.nextInt(10)));
                    break;
                case 3:
                    if (prop.track().isEmpty() || r.nextBoolean()) {
                        try {
                            prop.addModifier(new Modifier.Loop(Modifier.Loop.Mode.values()[r.nextInt(2)], r.nextDouble(), r.nextDouble() * 2));
                        } catch (IllegalStateException alreadyHasLoop) {
                            // fine
                        }
                    }
                    break;
                default:
                    Object v = randomValue(prop, r);
                    if (v != null) prop.track().put(new Key(r.nextDouble() * 4 - 1, v, Interp.values()[r.nextInt(3)]));
            }
        }
        double[] times = {0, -5, r.nextDouble() * 3, 1e9, -1e-9};
        for (double t : times) {
            Scene s = Evaluator.evaluate(p, p.rootSymbol, t);
            assertTrue(s.primitives <= Evaluator.PRIMITIVE_BUDGET, "budget, iter " + iter);
            Tessellator.tessellate(s);
            app.barkfart.geom.DrawList dl = Tessellator.layered(s);
            app.barkfart.fx.Compositor.render(dl, new app.barkfart.fx.Image(48, 27, -960, 540, 0.025f));
        }
    }

    private static Object randomValue(Property p, Random r) {
        switch (p.type) {
            case FLOAT:
            case ANGLE:
                return r.nextInt(4) == 0 ? EXTREMES[r.nextInt(EXTREMES.length)] : (float) (r.nextGaussian() * 100);
            case DOUBLE:
                return r.nextInt(4) == 0 ? (double) EXTREMES[r.nextInt(EXTREMES.length)] : r.nextGaussian() * 3;
            case BOOL:
                return r.nextBoolean();
            default:
                return null;
        }
    }
}
