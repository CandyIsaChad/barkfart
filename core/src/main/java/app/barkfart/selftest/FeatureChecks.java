package app.barkfart.selftest;

import app.barkfart.audio.AudioPlan;
import app.barkfart.audio.Mixer;
import app.barkfart.audio.Pcm;
import app.barkfart.audio.Wav;
import app.barkfart.canvas.CanvasTools;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.FigurePose;
import app.barkfart.eval.Group;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.eval.Scene;
import app.barkfart.eval.Shape;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.EffectTypes;
import app.barkfart.fx.Image;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Clip;
import app.barkfart.model.Color;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import app.barkfart.model.ValueType;
import java.util.ArrayList;
import java.util.List;

/**
 * One check per feature, run on the device by the Diagnostics screen (and at
 * launch). They use only app code, so they exercise the same runtime the
 * user has. Unit tests cover the same ground in more depth.
 */
final class FeatureChecks {
    private FeatureChecks() {}

    // ---- tiny builders ----

    private static Project project() {
        Project p = new Project();
        Symbol s = p.addSymbol(new Symbol(p.newId("S"), "check", 2));
        p.rootSymbol = s.id;
        return p;
    }

    private static Layer.FigureLayer figure(Project p) {
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "f");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        p.root().layers.add(l);
        return l;
    }

    private static Node node(Layer.FigureLayer l, String id, String parent, Node.Shape shape, float rot, float len, float thick) {
        Node n = new Node(l.id, id, parent, shape);
        n.rotation.setValue(rot);
        n.length.setValue(len);
        n.thickness.setValue(thick);
        l.figure.add(n);
        return n;
    }

    private static List<Shape> shapes(Project p) {
        p.invalidateIndex();
        List<Shape> out = new ArrayList<Shape>();
        collect(Evaluator.evaluate(p, p.rootSymbol, 0).root, out);
        return out;
    }

    private static void collect(Group g, List<Shape> out) {
        for (Object o : g.children) {
            if (o instanceof Group) collect((Group) o, out);
            else out.add((Shape) o);
        }
    }

    private static Image render(Project p, int w, int h) {
        p.invalidateIndex();
        Image im = new Image(w, h, -w / 2f, h / 2f, 1);
        Compositor.render(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0)), im);
        return im;
    }

    private static int alpha(Image im, float x, float y) {
        return Compositor.rgba(im, (int) (x - im.x0), (int) (im.y0 - y)) & 0xff;
    }

    private static PropertyResolver resolver(Project p) {
        return new PropertyResolver(p, new ArrayList<Diagnostic>());
    }

    static void run(SelfTest.Result r) {
        r.group = "Shapes";
        SelfTest.check(r, "curved segments bend", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "a", "r", Node.Shape.LINE, 0, 100, 8).curve = 30;
                Shape s = shapes(p).get(0);
                SelfTest.expect(s instanceof Shape.Line && Math.abs(((Shape.Line) s).bend) > 1, "no bend");
            }
        });
        SelfTest.check(r, "ellipse glow has a centre colour", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                Node e = node(l, "e", "r", Node.Shape.ELLIPSE, 0, 160, 60);
                e.color.setValue(Color.rgba8888(0xffffffff));
                e.gradient = true;
                e.gradientColor.setValue(Color.rgba8888(0x0000ffff));
                p.invalidateIndex();
                DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0));
                boolean centre = false;
                for (DrawList.Batch b : dl.batches) for (int i = 0; i < b.vertexCount(); i++) centre |= (b.rgba(i) >>> 24) >= 0xf0;
                SelfTest.expect(centre, "flat rim colour");
            }
        });
        SelfTest.check(r, "hollow circles, polygons, trapezoids", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "c", "r", Node.Shape.CIRCLE, 0, 60, 6).circleHollow = true;
                Node poly = node(l, "p", "r", Node.Shape.POLYGON, 90, 60, 6);
                poly.polygonSides = 7;
                Node trap = node(l, "t", "r", Node.Shape.TRAPEZOID, 180, 60, 40);
                trap.trapezoidRatio = 0.25f;
                List<Shape> s = shapes(p);
                SelfTest.expect(s.get(0) instanceof Shape.Ring, "hollow circle is not a ring");
                SelfTest.expect(((Shape.Polygon) s.get(1)).points.length == 14, "polygon corners");
                float[] q = ((Shape.Polygon) s.get(2)).points;
                double wA = Math.hypot(q[0] - q[2], q[1] - q[3]), wB = Math.hypot(q[4] - q[6], q[5] - q[7]);
                SelfTest.expect(Math.abs(wB / wA - 0.25) < 0.01, "trapezoid ratio " + wB / wA);
            }
        });
        SelfTest.check(r, "polyfills follow curves", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "c", "r", Node.Shape.LINE, 0, 140, 2).curve = 60;
                node(l, "d", "r", Node.Shape.NONE, -60, 100, 0);
                app.barkfart.model.Polyfill pf = new app.barkfart.model.Polyfill(l.id, "pf");
                pf.anchor = "r";
                pf.nodes.add("c");
                pf.nodes.add("d");
                l.figure.polyfills.add(pf);
                for (Shape s : shapes(p)) {
                    if (s instanceof Shape.Polygon) {
                        SelfTest.expect(((Shape.Polygon) s).points.length > 8, "fill edge is straight");
                        return;
                    }
                }
                throw new AssertionError("no fill");
            }
        });

        r.group = "Rigging";
        SelfTest.check(r, "node scale stretches its own segment", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "a", "r", Node.Shape.LINE, 0, 100, 4).scale.setValue(0.5f);
                node(l, "b", "a", Node.Shape.LINE, 0, 100, 4);
                p.invalidateIndex();
                FigurePose ps = FigurePose.compute(l, 0, resolver(p));
                SelfTest.expect(Math.abs(ps.x[1] - 50) < 1e-6 && Math.abs(ps.x[2] - 150) < 1e-6, "positions " + ps.x[1] + ", " + ps.x[2]);
            }
        });
        SelfTest.check(r, "smart-stretch matches the original", new SelfTest.Check() {
            public void run() {
                // The original's own stored result (Rain's stretched circle): 132.3 -> 701.2.
                Project p = project();
                Layer.FigureLayer l = figure(p);
                Node s = node(l, "s", "r", Node.Shape.LINE, 90, 132.3f, 4);
                s.smartStretch = true;
                s.restLength = 132.3f;
                Node back = node(l, "back", "s", Node.Shape.LINE, 180, 132.3f, 4);
                back.isStatic = true;
                back.restLength = 132.3f;
                Node d = node(l, "d", "back", Node.Shape.LINE, 85, 132.3f, 4);
                d.isStatic = true;
                d.restLength = 132.3f;
                p.invalidateIndex();
                CanvasTools tools = new CanvasTools(new Editor(new Document(p)));
                tools.smartStretch(l, FigurePose.compute(l, 0, resolver(p)), 1, 701.2f, 0);
                float got = resolver(p).f(d.length, 0);
                SelfTest.expect(Math.abs(got - 145.3f) < 0.3f, "detail length " + got + " (original: 145.3)");
            }
        });
        SelfTest.check(r, "angle locks and static nodes", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "arm", "r", Node.Shape.LINE, 40, 100, 4);
                Node plate = node(l, "plate", "arm", Node.Shape.LINE, 30, 50, 4);
                plate.isStatic = true;
                node(l, "abs", "plate", Node.Shape.LINE, 10, 30, 4).angleLock = Node.LOCK_ABSOLUTE;
                p.invalidateIndex();
                FigurePose ps = FigurePose.compute(l, 0, resolver(p));
                SelfTest.expect(Math.abs(ps.angle[3] - 10) < 1e-9, "absolute lock " + ps.angle[3]);
                CanvasTools tools = new CanvasTools(new Editor(new Document(p)));
                for (CanvasTools.Handle h : tools.handles(0)) SelfTest.expect(!h.node.isStatic, "static node has a handle");
            }
        });

        r.group = "Effects (reference)";
        SelfTest.check(r, "translucent figures composite once", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "a", "r", Node.Shape.SQUARE_LINE, 0, 40, 40);
                node(l, "b", "r", Node.Shape.SQUARE_LINE, 0, 40, 40);
                l.opacity.setValue(0.5f);
                Image im = render(p, 80, 60);
                int a = alpha(im, 20, 0);
                SelfTest.expect(Math.abs(a - 128) <= 2, "overlap alpha " + a);
            }
        });
        SelfTest.check(r, "blur, glow, shadow, invert", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "s", "r", Node.Shape.SQUARE_LINE, 0, 20, 20).color.setValue(Color.rgba8888(0xffffffff));
                EffectNode sh = new EffectNode(l.id, "FXs", EffectTypes.SHADOW);
                sh.param(l.id, "dx", ValueType.FLOAT, 10f);
                sh.param(l.id, "dy", ValueType.FLOAT, -10f);
                sh.param(l.id, "radius", ValueType.FLOAT, 0f);
                sh.param(l.id, "color", ValueType.COLOR, Color.rgba8888(0x000000ff));
                l.effects.append(sh);
                Image im = render(p, 80, 60);
                SelfTest.expect(alpha(im, 26, -16) == 255 && alpha(im, -6, 16) == 0, "shadow direction");
                l.effects.nodes.clear();
                l.effects.output = null;
                EffectNode b = new EffectNode(l.id, "FXb", EffectTypes.BLUR);
                b.param(l.id, "radius", ValueType.FLOAT, 8f);
                l.effects.append(b);
                im = render(p, 80, 60);
                SelfTest.expect(alpha(im, 24, 0) > 10, "blur does not spread");
                l.effects.nodes.clear();
                l.effects.output = null;
                l.effects.append(new EffectNode(l.id, "FXi", EffectTypes.INVERT));
                im = render(p, 80, 60);
                SelfTest.expect((Compositor.rgba(im, 50, 30) >>> 8) == 0, "invert");
            }
        });

        r.group = "Audio (mixer)";
        SelfTest.check(r, "wav decode", new SelfTest.Check() {
            public void run() {
                Pcm t = Mixer.tone(440, 0.05, 44100, 0.5f);
                Pcm b = Wav.decode(Wav.encode(t));
                SelfTest.expect(b != null && java.util.Arrays.equals(t.data, b.data), "round trip");
            }
        });
        SelfTest.check(r, "sound timing, trim, loop, pan", new SelfTest.Check() {
            public void run() {
                final Pcm tone = Mixer.tone(440, 1.0, 44100, 0.5f);
                Project p = project();
                Layer.AudioLayer a = new Layer.AudioLayer(p.newId("L"), "a");
                Clip c = new Clip(a.id, "c", "A", 0.5, 0, 0.25);
                c.pan.setValue(-1f);
                a.clips.add(c);
                p.root().layers.add(a);
                p.invalidateIndex();
                AudioPlan plan = AudioPlan.build(p, p.root(), new AudioPlan.Sounds() {
                    public Pcm pcm(String id) {
                        return tone;
                    }
                });
                float[] out = new float[44100 * 2];
                Mixer.mix(plan, 0, 44100, 44100, out);
                double before = 0, during = 0, after = 0, right = 0;
                for (int i = 0; i < 44100; i++) {
                    double v = out[i * 2] * out[i * 2];
                    if (i < 22000) before += v;
                    else if (i > 22600 && i < 32500) during += v;
                    else if (i > 33200) after += v;
                    right += out[i * 2 + 1] * out[i * 2 + 1];
                }
                SelfTest.expect(before == 0 && after == 0 && during > 100 && right < 1e-6, "timing " + before + " " + during + " " + after + " pan " + right);
            }
        });

        r.group = "Safety limits";
        SelfTest.check(r, "runaway delayed links stop", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                Node prev = null;
                for (int i = 0; i < 40; i++) prev = node(l, "n" + i, prev == null ? "r" : prev.id, Node.Shape.LINE, 1, 1, 1);
                p.invalidateIndex();
                List<Node> ns = l.figure.nodes();
                for (int i = 1; i + 2 < ns.size(); i++) {
                    // a different delay per link: every path lands on its own time (2^38 of them)
                    ns.get(i).rotation.addModifier(new Modifier.DelayFollow(ns.get(i + 1).rotation.id, 0.013f + i * 1.3e-4f, 1, 0));
                    ns.get(i).rotation.addModifier(new Modifier.DelayFollow(ns.get(i + 2).rotation.id, 0.029f + i * 2.9e-4f, 1, 0));
                }
                long t0 = System.currentTimeMillis();
                Scene s = Evaluator.evaluate(p, p.rootSymbol, 1);
                SelfTest.expect(System.currentTimeMillis() - t0 < 20000, "too slow");
                boolean hit = false;
                for (Diagnostic d : s.diagnostics) hit |= d.kind == Diagnostic.Kind.RESOLVE_BUDGET;
                SelfTest.expect(hit, "no budget diagnostic");
            }
        });
        SelfTest.check(r, "bad effect values render", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "s", "r", Node.Shape.SQUARE_LINE, 0, 20, 20);
                EffectNode e = new EffectNode(l.id, "FXn", EffectTypes.GLOW);
                e.param(l.id, "radius", ValueType.FLOAT, Float.NaN);
                e.param(l.id, "strength", ValueType.FLOAT, 1e30f);
                e.mix.setValue(Float.NaN);
                l.effects.append(e);
                render(p, 40, 30);
            }
        });

        r.group = "Figure building";
        SelfTest.check(r, "split a curved segment along its arc", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                Node a = node(l, "a", "r", Node.Shape.LINE, 30, 120, 6);
                a.curve = 20;
                p.invalidateIndex();
                FigurePose b = FigurePose.compute(l, 0, resolver(p));
                double ex = b.x[1], ey = b.y[1];
                Document d = new Document(p);
                d.apply(app.barkfart.edit.FigureOps.split(l, "a", 3));
                FigurePose c = FigurePose.compute(l, 0, resolver(p));
                int ai = l.figure.indexOf("a");
                SelfTest.expect(Math.hypot(c.x[ai] - ex, c.y[ai] - ey) < 1e-3, "end moved");
                SelfTest.expect(l.figure.nodes().size() == 4, "pieces " + l.figure.nodes().size());
                d.undo();
                SelfTest.expect(l.figure.nodes().size() == 2, "undo");
            }
        });
        SelfTest.check(r, "duplicate and flip a branch", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "a", "r", Node.Shape.LINE, 30, 100, 6);
                node(l, "b", "a", Node.Shape.LINE, 20, 50, 6);
                p.invalidateIndex();
                Document d = new Document(p);
                app.barkfart.edit.FigureOps.Made m = new app.barkfart.edit.FigureOps.Made();
                d.apply(app.barkfart.edit.FigureOps.duplicateBranch(l, "a", m));
                SelfTest.expect(l.figure.nodes().size() == 5, "copies");
                d.apply(app.barkfart.edit.FigureOps.flipBranch(p, l, "a", true, 0));
                FigurePose ps = FigurePose.compute(l, 0, resolver(p));
                SelfTest.expect(ps.x[l.figure.indexOf("b")] < 0, "flipped to the left");
            }
        });

        r.group = "Joining, movieclips, camera";
        SelfTest.check(r, "join a figure to a joint without a jump", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer a = figure(p), b = figure(p);
                node(a, "arm", "r", Node.Shape.LINE, 0, 100, 6).rotation.track().put(new app.barkfart.model.Key(1, 90f, app.barkfart.model.Interp.LINEAR));
                a.figure.node("arm").rotation.track().put(new app.barkfart.model.Key(0, 0f, app.barkfart.model.Interp.LINEAR));
                b.transform.x.setValue(150f);
                p.invalidateIndex();
                double before = Evaluator.layerStage(p, p.root(), b, 0).tx;
                new Document(p).apply(app.barkfart.edit.JoinOps.join(p, p.root(), b, a, "arm", true, 0));
                SelfTest.expect(Math.abs(Evaluator.layerStage(p, p.root(), b, 0).tx - before) < 1e-3, "jumped");
                SelfTest.expect(Math.abs(Evaluator.layerStage(p, p.root(), b, 1).tx - before) > 10, "does not follow");
            }
        });
        SelfTest.check(r, "make a movieclip, break it apart", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer a = figure(p);
                node(a, "arm", "r", Node.Shape.LINE, 30, 100, 6);
                p.invalidateIndex();
                int tris = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0.3)).triangleCount();
                Document d = new Document(p);
                app.barkfart.edit.SymbolOps.Made m = new app.barkfart.edit.SymbolOps.Made();
                d.apply(app.barkfart.edit.SymbolOps.convertToSymbol(p, p.root(), java.util.Arrays.<Layer>asList(a), "clip", m));
                SelfTest.expect(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0.3)).triangleCount() == tris, "frame changed");
                d.apply(app.barkfart.edit.SymbolOps.breakApart(p, p.root(), (Layer.SymbolLayer) m.instance, 0.3, null));
                SelfTest.expect(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0.3)).triangleCount() == tris, "broken apart differs");
            }
        });
        SelfTest.check(r, "camera lock, follow, widescreen bars", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer a = figure(p), cap = figure(p);
                a.transform.x.track().put(new app.barkfart.model.Key(0, 0f, app.barkfart.model.Interp.LINEAR));
                a.transform.x.track().put(new app.barkfart.model.Key(2, 300f, app.barkfart.model.Interp.LINEAR));
                Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
                p.root().layers.add(cam);
                p.invalidateIndex();
                Document d = new Document(p);
                d.apply(app.barkfart.edit.CameraOps.follow(p, p.root(), cam, a, 0, 60));
                double s0 = Evaluator.layerView(p, p.root(), a, 0).tx, s1 = Evaluator.layerView(p, p.root(), a, 1.5).tx;
                SelfTest.expect(Math.abs(s0 - s1) < 3, "follow drifts " + s0 + " " + s1);
                d.apply(app.barkfart.edit.CameraOps.lock(p, p.root(), cap, cam, true, 0));
                double c0 = Evaluator.layerView(p, p.root(), cap, 0).tx, c1 = Evaluator.layerView(p, p.root(), cap, 1.5).tx;
                SelfTest.expect(Math.abs(c0 - c1) < 1e-3, "locked layer moves in the shot");
                p.settings.letterbox = 2.39f;
                Group root = Evaluator.evaluate(p, p.rootSymbol, 0, false, true).root;
                SelfTest.expect("letterbox".equals(((Group) root.children.get(root.children.size() - 1)).source), "no bars");
            }
        });

        r.group = "Frames, images, text";
        SelfTest.check(r, "tweened frame and hold", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer a = figure(p);
                Node arm = node(a, "arm", "r", Node.Shape.LINE, 0, 100, 6);
                arm.rotation.track().put(new app.barkfart.model.Key(0, 0f, app.barkfart.model.Interp.LINEAR));
                arm.rotation.track().put(new app.barkfart.model.Key(1, 100f, app.barkfart.model.Interp.LINEAR));
                p.invalidateIndex();
                app.barkfart.edit.FrameOps ops = new app.barkfart.edit.FrameOps(new Editor(new Document(p)));
                ops.keyPoseBetween(null, 0, 1, 0.5);
                SelfTest.expect(arm.rotation.track().indexOf(0.5) >= 0 && Math.abs((Float) arm.rotation.keyValue(0.5) - 50) < 1e-3, "tweened key");
                float at = (Float) arm.rotation.keyValue(0.2);
                ops.hold(6, 3);
                SelfTest.expect(Math.abs((Float) arm.rotation.keyValue(0.2 + 2 / 30.0) - at) < 1e-3, "not held");
            }
        });
        SelfTest.check(r, "remove a background colour", new SelfTest.Check() {
            public void run() {
                byte[] px = new byte[4 * 4 * 4];
                for (int i = 0; i < 16; i++) {
                    boolean edge = i % 4 == 0 || i % 4 == 3 || i < 4 || i >= 12;
                    px[i * 4] = px[i * 4 + 1] = px[i * 4 + 2] = (byte) (edge ? 255 : 0);
                    px[i * 4 + 3] = (byte) 255;
                }
                byte[] out = app.barkfart.image.ImageOps.removeColor(px, 4, 4, 0xffffff, 5, 0, true);
                SelfTest.expect(out[3] == 0 && (out[(5) * 4 + 3] & 0xff) == 255, "background / subject");
                SelfTest.expect(java.util.Arrays.equals(new int[] {4, 4}, app.barkfart.image.ImageInfo.size(app.barkfart.export.Png.encode(px, 4, 4, true))), "png size");
            }
        });
        SelfTest.check(r, "text layout", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.TextLayer t = new Layer.TextLayer(p.newId("L"), "t");
                t.text = "Hi there";
                t.size = 20;
                p.root().layers.add(t);
                p.invalidateIndex();
                app.barkfart.geom.GlyphSource g = new app.barkfart.geom.GlyphSource() {
                    public float basePx() {
                        return 20;
                    }

                    public float lineHeight() {
                        return 20;
                    }

                    public Glyph glyph(char c) {
                        return new Glyph("glyphs:0", 0, 0, 1, 1, 0, 0, c == ' ' ? 0 : 10, 10, 10);
                    }
                };
                SelfTest.expect(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0), g).triangleCount() == 14, "glyph quads");
            }
        });

        r.group = "Effects (reference)";
        SelfTest.check(r, "pixelate, outline, motion blur", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer l = figure(p);
                node(l, "s", "r", Node.Shape.SQUARE_LINE, 0, 20, 20).color.setValue(Color.rgba8888(0xffffffff));
                EffectNode o = new EffectNode(l.id, "FXo", EffectTypes.OUTLINE);
                o.param(l.id, "width", ValueType.FLOAT, 4f);
                l.effects.append(o);
                Image im = render(p, 80, 60);
                SelfTest.expect(alpha(im, 22, 0) == 255 && alpha(im, 26, 0) == 0, "outline width");
                l.effects.nodes.clear();
                l.effects.output = null;
                EffectNode m = new EffectNode(l.id, "FXm", EffectTypes.MOTION);
                m.param(l.id, "distance", ValueType.FLOAT, 16f);
                l.effects.append(m);
                im = render(p, 80, 60);
                SelfTest.expect(alpha(im, 26, 0) > 10 && alpha(im, 0, 16) == 0, "motion direction");
            }
        });

        r.group = "Files and figure tools";
        SelfTest.check(r, "natural sort, zip packs, saved view", new SelfTest.Check() {
            public void run() throws Exception {
                SelfTest.expect(app.barkfart.files.ProjectStore.natural("fig 2", "fig 10") < 0, "natural sort");
                Project p = project();
                p.view.zoom = 2;
                p.view.frame = 3;
                Project q = app.barkfart.io.ProjectCodec.decode(app.barkfart.io.ProjectCodec.encode(p));
                SelfTest.expect(q.view.zoom == 2 && q.view.frame == 3, "view state");
                java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
                java.util.zip.ZipOutputStream z = new java.util.zip.ZipOutputStream(bytes);
                z.putNextEntry(new java.util.zip.ZipEntry("../x/a.nodes"));
                z.write(new byte[] {1});
                z.closeEntry();
                z.close();
                java.io.File dir = new java.io.File(System.getProperty("java.io.tmpdir"), "bf-pack-" + System.nanoTime());
                dir.mkdirs();
                try {
                    java.util.List<java.io.File> got = new app.barkfart.files.ProjectStore(dir).extractPack(bytes.toByteArray());
                    SelfTest.expect(got.size() == 1 && got.get(0).getParentFile().getCanonicalPath().equals(dir.getCanonicalPath()), "pack unpacked inside the folder");
                } finally {
                    java.io.File[] fs = dir.listFiles();
                    if (fs != null) for (java.io.File f : fs) f.delete();
                    dir.delete();
                }
            }
        });
        SelfTest.check(r, "duplicate with joins, copy properties", new SelfTest.Check() {
            public void run() throws Exception {
                Project p = project();
                Layer.FigureLayer a = figure(p), b = figure(p);
                node(a, "arm", "r", Node.Shape.LINE, 0, 100, 6);
                p.invalidateIndex();
                Document d = new Document(p);
                d.apply(app.barkfart.edit.JoinOps.join(p, p.root(), b, a, "arm", true, 0));
                app.barkfart.edit.LayerOps.Made m = new app.barkfart.edit.LayerOps.Made();
                d.apply(app.barkfart.edit.LayerOps.duplicate(p, p.root(), a, true, 30, 0, m));
                SelfTest.expect(m.layers.size() == 2 && m.layers.get(0).id.equals(m.layers.get(1).parent), "joins kept");
                a.transform.rotation.setValue(33f);
                app.barkfart.edit.LayerOps.Props c = app.barkfart.edit.LayerOps.copyProps(p, p.root(), a, 0);
                Editor ed = new Editor(d);
                ed.autoKey = true;
                app.barkfart.edit.LayerOps.pasteProps(ed, p.root(), m.layers.get(0), c, false, false, 0);
                SelfTest.expect(Math.abs(resolver(p).f(m.layers.get(0).transform.rotation, 0) - 33) < 1e-3, "properties pasted");
            }
        });
        SelfTest.check(r, "polyfill creation, node digging", new SelfTest.Check() {
            public void run() {
                Project p = project();
                Layer.FigureLayer a = figure(p);
                node(a, "x", "r", Node.Shape.LINE, 0, 100, 6);
                node(a, "y", "r", Node.Shape.LINE, 90, 100, 6);
                p.invalidateIndex();
                app.barkfart.edit.PolyfillBuilder pb = new app.barkfart.edit.PolyfillBuilder(a, "r");
                pb.add("x");
                pb.add("y");
                new Document(p).apply(pb.finish("pf0", Color.rgba8888(0xff0000ff)));
                SelfTest.expect(a.figure.polyfills.size() == 1, "fill made");
                Layer.FigureLayer twin = figure(p);
                node(twin, "x", "r", Node.Shape.LINE, 0, 100, 6);
                p.invalidateIndex();
                CanvasTools tools = new CanvasTools(new Editor(new Document(p)));
                tools.showStatic = true;
                CanvasTools.Hit h1 = tools.pickCycling(0, 0, 4, 0, null, null);
                CanvasTools.Hit h2 = tools.pickCycling(0, 0, 4, 0, null, h1.node);
                SelfTest.expect(h1.node != h2.node, "digging");
            }
        });

        r.group = "Sound, GIF, onion";
        SelfTest.check(r, "pitch, gif round trip, seam-free ghosts", new SelfTest.Check() {
            public void run() throws Exception {
                final Pcm tone = Mixer.tone(440, 1.0, 44100, 0.5f);
                Project p = project();
                Layer.AudioLayer al = new Layer.AudioLayer(p.newId("L"), "a");
                Clip c = new Clip(al.id, "c", "A", 0, 0, 0);
                c.pitch = 2;
                al.clips.add(c);
                p.root().layers.add(al);
                p.invalidateIndex();
                AudioPlan plan = AudioPlan.build(p, p.root(), new AudioPlan.Sounds() {
                    public Pcm pcm(String id) {
                        return tone;
                    }
                });
                SelfTest.expect(Math.abs(plan.entries.get(0).end - 0.5) < 1e-9, "pitch 2 halves the length");
                java.io.ByteArrayOutputStream gb = new java.io.ByteArrayOutputStream();
                app.barkfart.export.Gif g = new app.barkfart.export.Gif(gb, 8, 6, 10, true);
                byte[] px = new byte[8 * 6 * 4];
                for (int i = 0; i < 48; i++) {
                    px[i * 4] = (byte) 255;
                    px[i * 4 + 3] = (byte) 255;
                }
                g.frame(px);
                g.frame(px);
                g.finish();
                app.barkfart.image.GifDecoder.Result res = app.barkfart.image.GifDecoder.decode(gb.toByteArray());
                SelfTest.expect(res != null && res.frames.size() == 2 && (res.frames.get(1).rgba[0] & 0xff) == 255, "gif decode");
                Layer.FigureLayer f = figure(p);
                node(f, "a", "r", Node.Shape.SQUARE_LINE, 0, 20, 20);
                node(f, "b", "r", Node.Shape.SQUARE_LINE, 0, 20, 20);
                p.invalidateIndex();
                app.barkfart.geom.OnionSkin o = new app.barkfart.geom.OnionSkin();
                o.before = 1;
                o.after = 0;
                Image im = new Image(60, 40, -30, 20, 1);
                Compositor.render(o.buildLayered(p, p.rootSymbol, 0.5), im);
                SelfTest.expect(Math.abs(alpha(im, 10, 0) - Math.round(o.opacity * 255)) <= 2, "ghost overlap");
            }
        });
    }
}
