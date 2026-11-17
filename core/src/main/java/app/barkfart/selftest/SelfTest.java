package app.barkfart.selftest;

import app.barkfart.demo.Demo;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Group;
import app.barkfart.eval.Scene;
import app.barkfart.eval.Shape;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.BarkFile;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.KeyReducer;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import app.barkfart.model.Track;
import app.barkfart.model.ValueType;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.List;

/**
 * Core checks that run inside the app at launch. Phones are the only place the
 * MobiVM runtime is exercised, so this mirrors key unit tests without JUnit.
 */
public final class SelfTest {
    public static final class Result {
        public final List<String> failures = new ArrayList<String>();
        public int passed;
        public long millis;

        public boolean ok() {
            return failures.isEmpty();
        }

        public String summary() {
            return (ok() ? "PASS " : "FAIL ") + passed + "/" + (passed + failures.size()) + " (" + millis + " ms)";
        }
    }

    private interface Check {
        void run() throws Exception;
    }

    private SelfTest() {}

    public static Result run() {
        return run(null);
    }

    /** @param documents the user's folder; when given, a save/reload there is checked too. */
    public static Result run(final java.io.File documents) {
        Result r = new Result();
        if (documents != null) {
            check(r, "save and reload in Documents", new Check() {
                public void run() throws Exception {
                    java.io.File f = new java.io.File(documents, ".selftest.bark");   // hidden from lists
                    Project p = Demo.waving();
                    p.assets.put("Aself", new app.barkfart.model.Asset("Aself", app.barkfart.model.Asset.Kind.IMAGE, "assets/Aself.bin"));
                    byte[] data = new byte[4096];
                    for (int i = 0; i < data.length; i++) data[i] = (byte) i;
                    p.assetData.put("Aself", data);
                    BarkFile.save(p, f);
                    Project q = BarkFile.load(f);
                    if (!ProjectCodec.encode(p).equals(ProjectCodec.encode(q))) throw new AssertionError("project differs");
                    if (!java.util.Arrays.equals(data, q.assetData.bytes("Aself"))) throw new AssertionError("asset differs");
                    f.delete();
                    new java.io.File(f.getPath() + ".bak").delete();
                }
            });
        }
        long t0 = System.currentTimeMillis();
        check(r, "float text round trip", new Check() {
            public void run() {
                float[] vs = {0.1f, -363.24f, 1e-30f, 3.4028235e38f, 1.0f / 3, 30.123457f};
                for (float v : vs) {
                    if (Float.parseFloat(Float.toString(v)) != v) throw new AssertionError("float " + v);
                }
                double d = 1.0 / 30;
                if (Double.parseDouble(Double.toString(d)) != d) throw new AssertionError("double");
            }
        });
        check(r, "track sampling", new Check() {
            public void run() {
                Track t = new Track(ValueType.FLOAT);
                t.put(new Key(0, 0f, Interp.LINEAR));
                t.put(new Key(1, 10f, Interp.HOLD));
                t.put(new Key(2, 20f, Interp.LINEAR));
                eq(2.5f, (Float) t.sample(0.25), "linear");
                eq(10f, (Float) t.sample(1.5), "hold");
                eq(20f, (Float) t.sample(9), "after end");
            }
        });
        check(r, "forward kinematics", new Check() {
            public void run() {
                Project p = single();
                Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "arm");
                l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
                Demo.add(l, "a", "r", Node.Shape.LINE, 0, 10, 4);
                Demo.add(l, "b", "a", Node.Shape.LINE, 90, 10, 4);
                p.root().layers.add(l);
                List<Shape> s = shapes(Evaluator.evaluate(p, p.rootSymbol, 0).root);
                Shape.Line b = (Shape.Line) s.get(1);
                eq(10f, b.x2, "x");
                eq(10f, b.y2, "y");
            }
        });
        check(r, "self recursion limit", new Check() {
            public void run() {
                Project p = single();
                p.root().layers.add(Demo.stickman(p, "m"));
                p.root().layers.add(new Layer.SymbolLayer(p.newId("L"), "self", p.rootSymbol));
                Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
                if (!s.has(Diagnostic.Kind.RECURSION_LIMIT)) throw new AssertionError("no limit diagnostic");
                if (shapes(s.root).size() != 90) throw new AssertionError("shapes " + shapes(s.root).size());
            }
        });
        check(r, "exponential nesting budget", new Check() {
            public void run() {
                Project p = single();
                for (int i = 0; i < 6; i++) {
                    Layer.SymbolLayer sl = new Layer.SymbolLayer(p.newId("L"), "s", p.rootSymbol);
                    sl.recursionLimit = 60;
                    p.root().layers.add(sl);
                }
                Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
                if (!s.has(Diagnostic.Kind.PRIMITIVE_BUDGET) && !s.has(Diagnostic.Kind.DEPTH_CAP)) {
                    throw new AssertionError("no guard hit");
                }
            }
        });
        check(r, "link cycle", new Check() {
            public void run() {
                Project p = single();
                Layer.FigureLayer l = Demo.stickman(p, "m");
                p.root().layers.add(l);
                Node a = l.figure.node("armL1"), b = l.figure.node("armR1");
                a.rotation.addModifier(new Modifier.Link(b.rotation.id, 1, 0));
                b.rotation.addModifier(new Modifier.Link(a.rotation.id, 1, 0));
                p.invalidateIndex();
                if (!Evaluator.evaluate(p, p.rootSymbol, 0).has(Diagnostic.Kind.CYCLE)) throw new AssertionError("no cycle");
            }
        });
        check(r, "save/load round trip", new Check() {
            public void run() throws Exception {
                Project p = Demo.waving();
                String a = ProjectCodec.encode(p);
                String b = ProjectCodec.encode(ProjectCodec.decode(a));
                if (!a.equals(b)) throw new AssertionError("json differs");
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                BarkFile.write(p, out);
                Project q = BarkFile.read(new ByteArrayInputStream(out.toByteArray()));
                if (!a.equals(ProjectCodec.encode(q))) throw new AssertionError("zip differs");
            }
        });
        check(r, "key reduction", new Check() {
            public void run() {
                Track t = new Track(ValueType.FLOAT);
                for (int i = 0; i <= 20; i++) t.put(new Key(i, i * 3f, Interp.LINEAR));
                KeyReducer.reduce(t, 0.01);
                if (t.size() != 2) throw new AssertionError("left " + t.size());
            }
        });
        check(r, "demo evaluates", new Check() {
            public void run() {
                Project p = Demo.waving();
                for (int i = 0; i < 30; i++) {
                    Scene s = Evaluator.evaluate(p, p.rootSymbol, i / 15.0);
                    for (Diagnostic d : s.diagnostics) {
                        if (d.kind != Diagnostic.Kind.RECURSION_LIMIT) throw new AssertionError(d.toString());
                    }
                    if (Tessellator.tessellate(s).triangleCount() == 0) throw new AssertionError("no triangles");
                }
            }
        });
        check(r, "legacy figure round trip", new Check() {
            public void run() throws Exception {
                Project p = single();
                Layer.FigureLayer l = Demo.stickman(p, "m");
                byte[] a = app.barkfart.legacy.NodesFormat.write(app.barkfart.legacy.FigureMapper.toLegacy(l));
                Layer.FigureLayer back = app.barkfart.legacy.FigureMapper.toLayer(p,
                        app.barkfart.legacy.NodesFormat.read(a), "back", null);
                byte[] b = app.barkfart.legacy.NodesFormat.write(app.barkfart.legacy.FigureMapper.toLegacy(back));
                if (!java.util.Arrays.equals(a, b)) throw new AssertionError("bytes differ");
                if (back.figure.nodes().size() != l.figure.nodes().size()) throw new AssertionError("node count");
            }
        });
        check(r, "polyfill ear clipping", new Check() {
            public void run() {
                Project p = single();
                Layer.FigureLayer l = Demo.stickman(p, "m");
                app.barkfart.model.Polyfill pf = new app.barkfart.model.Polyfill(l.id, "pf");
                pf.anchor = "hip";
                pf.nodes.add("legL2");
                pf.nodes.add("legL1");
                pf.nodes.add("legR1");
                pf.nodes.add("legR2");
                l.figure.polyfills.add(pf);
                p.root().layers.add(l);
                Scene s = Evaluator.evaluate(p, p.rootSymbol, 0);
                if (!s.diagnostics.isEmpty()) throw new AssertionError(s.diagnostics.toString());
                int tris = Tessellator.tessellate(s).triangleCount();
                if (tris < 10) throw new AssertionError("triangles " + tris);
            }
        });
        check(r, "edit, pose and undo", new Check() {
            public void run() {
                Project p = single();
                Layer.FigureLayer l = Demo.stickman(p, "m");
                p.root().layers.add(l);
                p.invalidateIndex();
                String before = ProjectCodec.encode(p);
                app.barkfart.edit.Document d = new app.barkfart.edit.Document(p);
                app.barkfart.edit.Editor e = new app.barkfart.edit.Editor(d);
                e.change(l.figure.node("armR1").rotation, 45f, 0.5);
                e.pose(l, null, null, 1.0);
                if (l.figure.node("legL1").rotation.track().size() != 1) throw new AssertionError("pose did not key");
                while (d.undo()) { }
                if (!before.equals(ProjectCodec.encode(p))) throw new AssertionError("undo did not restore");
            }
        });
        r.millis = System.currentTimeMillis() - t0;
        return r;
    }

    private static Project single() {
        Project p = new Project();
        Symbol s = p.addSymbol(new Symbol(p.newId("S"), "root", 1));
        p.rootSymbol = s.id;
        return p;
    }

    private static void eq(float want, float got, String what) {
        if (Math.abs(want - got) > 1e-3f) throw new AssertionError(what + ": want " + want + " got " + got);
    }

    private static List<Shape> shapes(Group g) {
        List<Shape> out = new ArrayList<Shape>();
        collect(g, out);
        return out;
    }

    private static void collect(Group g, List<Shape> out) {
        for (Object o : g.children) {
            if (o instanceof Shape) out.add((Shape) o);
            else collect((Group) o, out);
        }
    }

    private static void check(Result r, String name, Check c) {
        try {
            c.run();
            r.passed++;
        } catch (Throwable t) {
            r.failures.add(name + ": " + t);
        }
    }
}
