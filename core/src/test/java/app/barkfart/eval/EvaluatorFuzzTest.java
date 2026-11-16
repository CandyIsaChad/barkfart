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
                    case 1:
                        l = Fixtures.stickman(p, "m");
                        break;
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
