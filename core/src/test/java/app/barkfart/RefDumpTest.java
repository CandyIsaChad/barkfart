package app.barkfart;

import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.GlyphSource;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.Json;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Polyfill;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import app.barkfart.model.ValueType;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.lang.reflect.Field;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

/**
 * Reference data for the Rust port (docs/barkfart/PORT.md). Only runs with
 * -Dbarkfart.refdump=<dir>; for each case writes the project (as .bark
 * project.json), and at a few times the evaluated scene and the triangles,
 * so the Rust engine can be checked number for number.
 */
class RefDumpTest {
    static final double[] TIMES = {0, 0.37, 1.1};

    /** Deterministic glyphs, so text tessellates the same in Java and Rust. */
    static final GlyphSource GLYPHS = new GlyphSource() {
        public float basePx() {
            return 20;
        }

        public float lineHeight() {
            return 24;
        }

        public Glyph glyph(char c) {
            if (c == ' ') return new Glyph("glyphs:0", 0, 0, 0, 0, 0, 0, 0, 0, 8);
            if (c > 126) return null;
            float u = (c % 16) / 16f, v = (c / 16) / 8f;
            return new Glyph("glyphs:0", u, v + 0.125f, u + 0.0625f, v, 1, -4, 10, 16, 12);
        }
    };

    @Test
    void dump() throws Exception {
        String dir = System.getProperty("barkfart.refdump", "");
        Assumptions.assumeTrue(!dir.isEmpty(), "reference dump not requested");
        File out = new File(dir);
        out.mkdirs();
        List<Object[]> cases = new ArrayList<Object[]>();
        cases.add(new Object[] {"kitchen-sink", Fixtures.kitchenSink()});
        Project man = Fixtures.empty();
        man.root().layers.add(Fixtures.stickman(man, "man"));
        man.invalidateIndex();
        cases.add(new Object[] {"stickman", man});
        Random r = new Random(20261119);
        for (int i = 0; i < 80; i++) cases.add(new Object[] {String.format("random-%03d", i), random(r)});
        cases.add(new Object[] {"fx-scene", app.barkfart.fx.EffectScene.build()});
        Random fr = new Random(4242);
        for (int i = 0; i < 24; i++) cases.add(new Object[] {String.format("fx-%02d", i), effects(fr, i)});
        List<Object> index = new ArrayList<Object>();
        for (Object[] c : cases) {
            String name = (String) c[0];
            Project p = (Project) c[1];
            File d = new File(out, name);
            d.mkdirs();
            write(new File(d, "project.json"), ProjectCodec.encode(p));
            List<Object> times = new ArrayList<Object>();
            for (double t : TIMES) {
                Scene s = Evaluator.evaluate(p, p.rootSymbol, t, false, true);
                Scene e = Evaluator.evaluate(p, p.rootSymbol, t, false, false);
                Map<String, Object> m = new LinkedHashMap<String, Object>();
                m.put("time", Json.num(t));
                m.put("scene", scene(s));
                m.put("stageScene", scene(e));
                m.put("draw", draw(Tessellator.tessellate(s, GLYPHS)));
                DrawList layered = Tessellator.layered(s, GLYPHS);
                m.put("layered", layered(layered));
                app.barkfart.geom.OnionSkin onion = new app.barkfart.geom.OnionSkin();
                m.put("onion", layered(onion.buildLayered(p, p.rootSymbol, t)));
                // CPU reference render (Compositor), premultiplied RGBA8, rows top-down:
                // effect cases at scale 1 around the origin, others the whole stage at 200 px wide
                boolean fxCase = name.startsWith("fx");
                int iw = fxCase ? app.barkfart.fx.EffectScene.W : 200;
                float scale = fxCase ? 1 : 200f / p.settings.width;
                int ih = fxCase ? app.barkfart.fx.EffectScene.H : Math.max(1, Math.round(p.settings.height * scale));
                app.barkfart.fx.Image im = new app.barkfart.fx.Image(iw, ih, -iw / 2f / scale, ih / 2f / scale, scale);
                app.barkfart.fx.Compositor.render(layered, im);
                byte[] px = new byte[iw * ih * 4];
                for (int k = 0; k < px.length; k++) px[k] = (byte) Math.round(Math.max(0f, Math.min(1f, im.px[k])) * 255);
                Map<String, Object> cpu = new LinkedHashMap<String, Object>();
                cpu.put("w", Json.num(iw));
                cpu.put("h", Json.num(ih));
                cpu.put("x0", Json.num(im.x0));
                cpu.put("y0", Json.num(im.y0));
                cpu.put("scale", Json.num(scale));
                m.put("cpu", cpu);
                java.util.zip.GZIPOutputStream cz = new java.util.zip.GZIPOutputStream(new FileOutputStream(new File(d, "t" + t + ".cpu.rgba.gz")));
                cz.write(px);
                cz.close();
                String tag = String.valueOf(t);
                java.util.zip.GZIPOutputStream gz = new java.util.zip.GZIPOutputStream(new FileOutputStream(new File(d, "t" + tag + ".json.gz")));
                gz.write(Json.write(m).getBytes("UTF-8"));
                gz.close();
                times.add(tag);
            }
            Map<String, Object> entry = new LinkedHashMap<String, Object>();
            entry.put("name", name);
            entry.put("times", times);
            index.add(entry);
        }
        write(new File(out, "index.json"), Json.write(index));
        // audio: projects with test tones, their plans' soundtrack bounced to WAV
        final app.barkfart.audio.Pcm mono = app.barkfart.audio.Mixer.tone(440, 1.0, 22050, 0.5f);
        final app.barkfart.audio.Pcm high = app.barkfart.audio.Mixer.tone(660, 0.6, 44100, 0.4f);
        app.barkfart.audio.AudioPlan.Sounds sounds = new app.barkfart.audio.AudioPlan.Sounds() {
            public app.barkfart.audio.Pcm pcm(String id) {
                return "tone440".equals(id) ? mono : "tone660".equals(id) ? high : null;
            }
        };
        File ad = new File(out, "audio");
        ad.mkdirs();
        Random ar = new Random(777);
        for (int i = 0; i < 6; i++) {
            Project p = audio(ar);
            write(new File(ad, "audio-" + i + ".json"), ProjectCodec.encode(p));
            byte[] wav = app.barkfart.audio.Bounce.wav(app.barkfart.audio.AudioPlan.build(p, p.root(), sounds), 0.1 * i, 2.5);
            java.util.zip.GZIPOutputStream gz = new java.util.zip.GZIPOutputStream(new FileOutputStream(new File(ad, "audio-" + i + ".wav.gz")));
            gz.write(wav == null ? new byte[0] : wav);
            gz.close();
        }
    }

    private static void write(File f, String s) throws Exception {
        Writer w = new OutputStreamWriter(new FileOutputStream(f), "UTF-8");
        w.write(s);
        w.close();
    }

    // ---- generic dump of evaluator output (public instance fields, by reflection) ----

    private static Map<String, Object> scene(Scene s) throws Exception {
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        m.put("primitives", Json.num(s.primitives));
        List<Object> diags = new ArrayList<Object>();
        for (app.barkfart.eval.Diagnostic d : s.diagnostics) diags.add(d.kind.name() + " " + d.subject);
        m.put("diagnostics", diags);
        m.put("root", value(s.root));
        return m;
    }

    static Object value(Object o) throws Exception {
        if (o == null) return null;
        if (o instanceof Float) return Json.num((Float) o);
        if (o instanceof Double) return Json.num((Double) o);
        if (o instanceof Integer) return Json.num((Integer) o);
        if (o instanceof Boolean || o instanceof String) return o;
        if (o instanceof Enum) return ((Enum<?>) o).name();
        if (o instanceof float[]) {
            List<Object> l = new ArrayList<Object>();
            for (float f : (float[]) o) l.add(Json.num(f));
            return l;
        }
        if (o instanceof List) {
            List<Object> l = new ArrayList<Object>();
            for (Object x : (List<?>) o) l.add(value(x));
            return l;
        }
        if (o instanceof Map) {
            Map<String, Object> m = new LinkedHashMap<String, Object>();
            for (Map.Entry<?, ?> e : ((Map<?, ?>) o).entrySet()) m.put(String.valueOf(e.getKey()), value(e.getValue()));
            return m;
        }
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        m.put("kind", o.getClass().getSimpleName());
        List<Field> fields = new ArrayList<Field>();
        for (Class<?> c = o.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
            List<Field> mine = new ArrayList<Field>();
            for (Field f : c.getDeclaredFields()) {
                int mod = f.getModifiers();
                if (java.lang.reflect.Modifier.isStatic(mod) || !java.lang.reflect.Modifier.isPublic(mod)) continue;
                mine.add(f);
            }
            fields.addAll(0, mine);
        }
        for (Field f : fields) m.put(f.getName(), value(f.get(o)));
        return m;
    }

    private static Map<String, Object> draw(DrawList dl) {
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        List<Object> batches = new ArrayList<Object>();
        for (DrawList.Batch b : dl.batches) {
            Map<String, Object> bm = new LinkedHashMap<String, Object>();
            bm.put("blend", b.blend.name());
            bm.put("effectGroup", b.effectGroup);
            bm.put("texture", b.texture);
            List<Object> xy = new ArrayList<Object>(), uv = new ArrayList<Object>(), rgba = new ArrayList<Object>();
            for (int i = 0; i < b.vertexCount(); i++) {
                xy.add(Json.num(b.x(i)));
                xy.add(Json.num(b.y(i)));
                uv.add(Json.num(b.u(i)));
                uv.add(Json.num(b.v(i)));
                rgba.add(String.format("%08x", b.rgba(i)));
            }
            bm.put("xy", xy);
            bm.put("uv", uv);
            bm.put("rgba", rgba);
            batches.add(bm);
        }
        m.put("batches", batches);
        return m;
    }

    /** The isolate tree: isolates with their fields and items (batch indices into "batches", or nested isolates). */
    private static Map<String, Object> layered(DrawList dl) throws Exception {
        java.util.IdentityHashMap<DrawList.Batch, Integer> ids = new java.util.IdentityHashMap<DrawList.Batch, Integer>();
        for (int i = 0; i < dl.batches.size(); i++) ids.put(dl.batches.get(i), i);
        Map<String, Object> m = draw(dl);
        m.put("root", isolate(dl.root, ids));
        return m;
    }

    private static Object isolate(DrawList.Isolate iso, java.util.IdentityHashMap<DrawList.Batch, Integer> ids) throws Exception {
        if (iso == null) return null;
        Map<String, Object> m = new LinkedHashMap<String, Object>();
        m.put("source", iso.source);
        m.put("opacity", Json.num(iso.opacity));
        m.put("blend", iso.blend.name());
        m.put("adjustment", iso.adjustment);
        m.put("effects", value(iso.effects));
        boolean empty = iso.empty();
        m.put("bounds", empty ? null : java.util.Arrays.<Object>asList(Json.num(iso.minX), Json.num(iso.minY), Json.num(iso.maxX), Json.num(iso.maxY)));
        List<Object> items = new ArrayList<Object>();
        for (Object o : iso.items) items.add(o instanceof DrawList.Batch ? (Object) Json.num(ids.get(o)) : isolate((DrawList.Isolate) o, ids));
        m.put("items", items);
        return m;
    }

    // ---- effect corpus: squares with every effect and parameter, at scale 1 ----

    private static Project effects(Random r, int index) {
        Project p = Fixtures.empty();
        p.settings.width = app.barkfart.fx.EffectScene.W;
        p.settings.height = app.barkfart.fx.EffectScene.H;
        String[] all = app.barkfart.fx.EffectTypes.ALL;
        int n = 2 + r.nextInt(4);
        for (int i = 0; i < n; i++) {
            Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "sq" + i);
            l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
            for (int k = 0; k < 1 + r.nextInt(3); k++) {
                Node s = new Node(l.id, "s" + k, "r", Node.Shape.values()[1 + r.nextInt(4)]);
                s.length.setValue(10f + r.nextInt(30));
                s.thickness.setValue(4f + r.nextInt(24));
                s.rotation.setValue((float) r.nextInt(360));
                s.color.setValue(color(r));
                l.figure.add(s);
            }
            l.transform.x.setValue((float) (r.nextInt(120) - 60));
            l.transform.y.setValue((float) (r.nextInt(140) - 70));
            if (r.nextInt(3) == 0) l.opacity.setValue(0.3f + r.nextFloat() * 0.6f);
            if (r.nextInt(4) == 0) l.blend = Layer.Blend.values()[1 + r.nextInt(3)];
            int fx = r.nextInt(3);
            for (int q = 0; q < fx; q++) {
                // every type comes up across the corpus: index-driven first, then random
                String type = q == 0 ? all[(index + i) % all.length] : all[r.nextInt(all.length)];
                EffectNode e = new EffectNode(l.id, "fx" + q, type);
                e.param(l.id, "radius", ValueType.FLOAT, 1f + r.nextInt(14));
                e.param(l.id, "strength", ValueType.FLOAT, 0.5f + r.nextFloat() * 2);
                e.param(l.id, "dx", ValueType.FLOAT, (float) (r.nextInt(21) - 10));
                e.param(l.id, "dy", ValueType.FLOAT, (float) (r.nextInt(21) - 10));
                e.param(l.id, "hue", ValueType.FLOAT, (float) r.nextInt(360));
                e.param(l.id, "saturation", ValueType.FLOAT, r.nextFloat() * 2);
                e.param(l.id, "brightness", ValueType.FLOAT, r.nextFloat() - 0.5f);
                e.param(l.id, "size", ValueType.FLOAT, 2f + r.nextInt(10));
                e.param(l.id, "width", ValueType.FLOAT, 1f + r.nextInt(6));
                e.param(l.id, "angle", ValueType.FLOAT, (float) r.nextInt(360));
                e.param(l.id, "distance", ValueType.FLOAT, 4f + r.nextInt(24));
                e.param(l.id, "amount", ValueType.FLOAT, r.nextFloat());
                e.param(l.id, "color", ValueType.COLOR, color(r));
                if (r.nextInt(3) == 0) e.mix.setValue(r.nextFloat());
                l.effects.append(e);
            }
            p.root().layers.add(l);
        }
        if (r.nextBoolean()) {   // an adjustment layer over everything
            Layer.AdjustmentLayer adj = new Layer.AdjustmentLayer(p.newId("L"), "adj");
            EffectNode e = new EffectNode(adj.id, "fx0", r.nextBoolean() ? app.barkfart.fx.EffectTypes.INVERT : app.barkfart.fx.EffectTypes.BLUR);
            e.param(adj.id, "radius", ValueType.FLOAT, 2f + r.nextInt(6));
            adj.effects.append(e);
            adj.opacity.setValue(0.4f + r.nextFloat() * 0.6f);
            p.root().layers.add(adj);
        }
        if (r.nextInt(3) == 0) {   // nested: a translucent movieclip holding a glowing figure
            Symbol inner = p.addSymbol(new Symbol(p.newId("S"), "inner", 1));
            Layer.FigureLayer f = new Layer.FigureLayer(p.newId("L"), "in");
            f.figure.add(new Node(f.id, "r", null, Node.Shape.NONE));
            Node s = new Node(f.id, "s", "r", Node.Shape.LINE);
            s.length.setValue(30f);
            s.thickness.setValue(8f);
            s.color.setValue(color(r));
            f.figure.add(s);
            EffectNode e = new EffectNode(f.id, "fx0", app.barkfart.fx.EffectTypes.GLOW);
            e.param(f.id, "radius", ValueType.FLOAT, 6f);
            f.effects.append(e);
            inner.layers.add(f);
            Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), "inst", inner.id);
            inst.opacity.setValue(0.7f);
            inst.transform.x.setValue((float) (r.nextInt(60) - 30));
            p.root().layers.add(inst);
        }
        p.invalidateIndex();
        return p;
    }

    private static Project audio(Random r) {
        Project p = Fixtures.empty();
        p.root().duration = 2 + r.nextInt(2);
        for (int li = 0; li < 1 + r.nextInt(3); li++) {
            Layer.AudioLayer al = new Layer.AudioLayer(p.newId("L"), "a" + li);
            for (int ci = 0; ci < 1 + r.nextInt(3); ci++) {
                app.barkfart.model.Clip c = new app.barkfart.model.Clip(al.id, "c" + ci, r.nextInt(6) == 0 ? "missing" : (r.nextBoolean() ? "tone440" : "tone660"),
                        r.nextDouble() * 1.5 - 0.2, r.nextInt(3) == 0 ? r.nextDouble() * 0.3 : 0, r.nextInt(3) == 0 ? 0.3 + r.nextDouble() * 0.5 : 0);
                c.loop = r.nextInt(3) == 0;
                c.fadeIn = r.nextBoolean() ? r.nextDouble() * 0.3 : 0;
                c.fadeOut = r.nextBoolean() ? r.nextDouble() * 0.3 : 0;
                c.pitch = new double[] {1, 1, 0.5, 2, 1.37}[r.nextInt(5)];
                if (r.nextBoolean()) {
                    c.volume.track().put(new Key(0.2, 1f, Interp.LINEAR));
                    c.volume.track().put(new Key(1.4, 0.2f + r.nextFloat(), Interp.BEZIER, Ease.EASE_IN_OUT));
                } else c.volume.setValue(0.3f + r.nextFloat());
                if (r.nextBoolean()) {
                    c.pan.track().put(new Key(0.0, -1f, Interp.LINEAR));
                    c.pan.track().put(new Key(1.8, 1f, Interp.LINEAR));
                } else c.pan.setValue(r.nextFloat() * 2 - 1);
                al.clips.add(c);
            }
            if (r.nextInt(3) == 0) al.opacity.setValue(0.5f);
            if (r.nextInt(5) == 0) al.solo = true;
            if (r.nextInt(6) == 0) al.visible = false;
            p.root().layers.add(al);
        }
        p.invalidateIndex();
        return p;
    }

    // ---- random corpus ----

    private static final Ease[] EASES = {Ease.LINEAR, Ease.EASE_IN_OUT, Ease.OVERSHOOT, Ease.ANTICIPATE, Ease.BOUNCE_OUT, Ease.ELASTIC_IN};

    private static Color color(Random r) {
        return new Color(r.nextInt(256) / 255f, r.nextInt(256) / 255f, r.nextInt(256) / 255f, r.nextInt(4) == 0 ? r.nextInt(256) / 255f : 1f);
    }

    private static void keys(Random r, Property p, float spread) {
        int n = r.nextInt(4);
        for (int i = 0; i < n; i++) {
            Object v;
            if (p.type == ValueType.COLOR) v = color(r);
            else if (p.type == ValueType.BOOL) v = r.nextBoolean();
            else if (p.type == ValueType.DOUBLE) v = r.nextDouble() * spread;
            else v = (float) (r.nextGaussian() * spread);
            Interp in = Interp.values()[r.nextInt(3)];
            p.track().put(new Key(Math.round(r.nextDouble() * 2 * 24) / 24.0, v, in, in == Interp.BEZIER ? EASES[r.nextInt(EASES.length)] : Ease.LINEAR));
        }
        if (p.type != ValueType.BOOL && p.type != ValueType.COLOR && r.nextInt(12) == 0) {
            p.addModifier(r.nextBoolean() ? new Modifier.Sine(0.5f + r.nextFloat() * 3, spread * r.nextFloat(), r.nextFloat())
                    : new Modifier.Wiggle(0.5f + r.nextFloat() * 4, spread * r.nextFloat(), r.nextInt(1000), 1 + r.nextInt(3)));
        }
    }

    private static Layer.FigureLayer figure(Random r, Project p) {
        Layer.FigureLayer fl = Fixtures.stickman(p, "m" + r.nextInt(100));
        List<Node> ns = fl.figure.nodes();
        for (Node n : ns) {
            if (n.parent == null) continue;
            if (r.nextInt(3) == 0) n.shape = Node.Shape.values()[1 + r.nextInt(Node.Shape.values().length - 1)];
            if (r.nextInt(4) == 0) n.color.setValue(color(r));
            if (r.nextInt(5) == 0) n.thickness.setValue(1 + r.nextFloat() * 30);
            if (r.nextInt(6) == 0) n.curve = (float) (r.nextGaussian() * 15);
            if (r.nextInt(6) == 0) {
                n.gradient = true;
                n.gradientColor.setValue(color(r));
                n.gradientReverse = r.nextBoolean();
                n.gradientSideways = r.nextBoolean();
                n.gradientLinear = r.nextInt(3) == 0;
            }
            if (r.nextInt(8) == 0) n.circleHollow = r.nextBoolean();
            if (r.nextInt(8) == 0) n.circleOutline = true;
            if (r.nextInt(8) == 0) n.halfArc = r.nextInt(3) - 1;
            if (r.nextInt(8) == 0) n.polygonSides = 3 + r.nextInt(10);
            if (r.nextInt(8) == 0) n.trapezoidRatio = r.nextFloat() * 2;
            if (r.nextInt(8) == 0) n.trapezoidRoundStart = true;
            if (r.nextInt(8) == 0) n.trapezoidRoundEnd = true;
            if (r.nextInt(8) == 0) n.triangleUpsideDown = true;
            if (r.nextInt(8) == 0) n.rightTriangle = r.nextInt(3) - 1;
            if (r.nextInt(10) == 0) n.angleLock = r.nextInt(3);
            if (r.nextInt(10) == 0) n.isStatic = true;
            if (r.nextInt(10) == 0) n.scale.setValue(0.2f + r.nextFloat() * 2);
            if (r.nextInt(12) == 0) n.drawOrder = r.nextInt(20) - 5;
            if (r.nextInt(3) == 0) keys(r, n.rotation, 90);
            if (r.nextInt(6) == 0) keys(r, n.length, 20);
            if (r.nextInt(10) == 0) keys(r, n.shown, 1);
            if (r.nextInt(10) == 0) keys(r, n.color, 1);
        }
        int fills = r.nextInt(3);
        for (int q = 0; q < fills; q++) {
            Polyfill pf = new Polyfill(fl.id, "pf" + q);
            pf.anchor = ns.get(r.nextInt(ns.size())).id;
            int m = 3 + r.nextInt(4);
            for (int z = 0; z < m; z++) pf.nodes.add(ns.get(r.nextInt(ns.size())).id);
            pf.useOwnColor = r.nextBoolean();
            pf.color.setValue(color(r));
            fl.figure.polyfills.add(pf);
        }
        return fl;
    }

    private static Project random(Random r) {
        Project p = Fixtures.empty();
        p.settings.width = 320 + r.nextInt(3) * 160;
        p.settings.height = 240 + r.nextInt(3) * 120;
        p.settings.fps = new float[] {12, 24, 30, 60}[r.nextInt(4)];
        if (r.nextInt(5) == 0) p.settings.letterbox = 2.39f;
        List<Symbol> syms = new ArrayList<Symbol>();
        syms.add(p.root());
        int extra = r.nextInt(3);
        for (int i = 0; i < extra; i++) syms.add(p.addSymbol(new Symbol(p.newId("S"), "s" + i, 0.5 + r.nextDouble() * 2)));
        for (int si = syms.size() - 1; si >= 0; si--) {
            Symbol s = syms.get(si);
            int layers = 1 + r.nextInt(5);
            for (int k = 0; k < layers; k++) {
                Layer l;
                int kind = r.nextInt(10);
                if (kind < 5) l = figure(r, p);
                else if (kind == 5 && si + 1 < syms.size()) {
                    Layer.SymbolLayer sl = new Layer.SymbolLayer(p.newId("L"), "i", syms.get(si + 1 + r.nextInt(syms.size() - si - 1)).id);
                    sl.loop = Layer.SymbolLayer.LoopMode.values()[r.nextInt(3)];
                    if (r.nextBoolean()) keys(r, sl.timeOffset, 1);
                    if (r.nextInt(4) == 0) sl.speed.setValue(0.25 + r.nextDouble() * 2);
                    l = sl;
                } else if (kind == 6) {
                    Layer.TextLayer tl = new Layer.TextLayer(p.newId("L"), "t");
                    tl.text = r.nextBoolean() ? "Hi there" : "two\nlines!";
                    tl.size = 10 + r.nextInt(40);
                    tl.corner = r.nextInt(5);
                    tl.color.setValue(color(r));
                    l = tl;
                } else if (kind == 7) {
                    Layer.SpriteLayer sp = new Layer.SpriteLayer(p.newId("L"), "sp");
                    sp.asset = "img" + r.nextInt(3);
                    sp.width = 20 + r.nextInt(100);
                    sp.height = 20 + r.nextInt(100);
                    sp.tint.setValue(color(r));
                    if (r.nextBoolean()) for (int c = 0; c < 8; c++) if (r.nextInt(3) == 0) sp.distort[c].setValue((float) (r.nextGaussian() * 15));
                    l = sp;
                } else if (kind == 8 && si == 0) {
                    Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
                    keys(r, cam.x, 40);
                    keys(r, cam.y, 40);
                    if (r.nextBoolean()) cam.zoom.setValue(0.5f + r.nextFloat() * 2);
                    keys(r, cam.rotation, 20);
                    if (r.nextBoolean()) cam.background.setValue(color(r));
                    if (r.nextBoolean()) cam.background2.setValue(color(r));
                    l = cam;
                } else {
                    Layer.AdjustmentLayer al = new Layer.AdjustmentLayer(p.newId("L"), "adj");
                    l = al;
                }
                keys(r, l.transform.x, 60);
                keys(r, l.transform.y, 60);
                if (r.nextInt(3) == 0) keys(r, l.transform.rotation, 45);
                if (r.nextInt(4) == 0) l.transform.scaleX.setValue(0.5f + r.nextFloat());
                if (r.nextInt(5) == 0) l.transform.scaleY.setValue(r.nextBoolean() ? -1f : 0.5f + r.nextFloat());
                if (r.nextInt(6) == 0) l.transform.anchorX.setValue((float) (r.nextGaussian() * 20));
                if (r.nextInt(5) == 0) keys(r, l.opacity, 1);
                if (r.nextInt(8) == 0) l.blend = Layer.Blend.values()[r.nextInt(4)];
                if (r.nextInt(10) == 0) keys(r, l.shown, 1);
                if (r.nextInt(12) == 0) l.visible = false;
                if (r.nextInt(5) == 0) {
                    String type = app.barkfart.fx.EffectTypes.ALL[r.nextInt(app.barkfart.fx.EffectTypes.ALL.length)];
                    EffectNode e = new EffectNode(l.id, p.newId("FX"), type);
                    for (String name : new String[] {"radius", "strength", "amount"}) if (r.nextBoolean()) e.param(l.id, name, ValueType.FLOAT, r.nextFloat() * 10);
                    l.effects.append(e);
                }
                // joins: to an earlier figure in the same symbol, sometimes at a joint
                if (r.nextInt(4) == 0 && !(l instanceof Layer.CameraLayer)) {
                    for (Layer o : s.layers) {
                        if (o instanceof Layer.FigureLayer && r.nextBoolean()) {
                            l.parent = o.id;
                            if (r.nextBoolean()) {
                                List<Node> on = ((Layer.FigureLayer) o).figure.nodes();
                                l.parentNode = on.get(r.nextInt(on.size())).id;
                                l.parentNodeRotate = r.nextBoolean();
                            }
                            break;
                        }
                        if (o instanceof Layer.CameraLayer && r.nextBoolean()) {
                            l.parent = o.id;
                            l.parallax = r.nextInt(3) * 0.5f;
                            break;
                        }
                    }
                }
                s.layers.add(l);
            }
        }
        p.invalidateIndex();
        return p;
    }
}
