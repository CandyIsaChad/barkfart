package app.barkfart.io;

import app.barkfart.model.Asset;
import app.barkfart.model.Clip;
import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.EffectNode;
import app.barkfart.model.EffectStack;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Marker;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.NodeSet;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Scope;
import app.barkfart.model.Symbol;
import app.barkfart.model.ValueType;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Project <-> JSON tree (DESIGN.md §6). Deterministic: the same project always
 * produces the same text, and save(load(save(p))) == save(p). Colours are
 * stored as 8-bit hex, so non-8-bit colour values are rounded on first save.
 */
public final class ProjectCodec {
    private ProjectCodec() {}

    /** Thrown for structurally invalid or too-new project data. */
    public static final class FormatException extends Exception {
        public FormatException(String m, Throwable cause) {
            super(m, cause);
        }
    }

    // ================= encode =================

    public static String encode(Project p) {
        return Json.write(project(p));
    }

    private static Map<String, Object> obj() {
        return new LinkedHashMap<String, Object>();
    }

    private static Map<String, Object> project(Project p) {
        Map<String, Object> m = obj();
        m.put("format", "barkfart");
        m.put("formatVersion", Json.num(Project.FORMAT_VERSION));
        m.put("idCounter", Json.num(p.idCounter()));
        Map<String, Object> s = obj();
        s.put("fps", Json.num(p.settings.fps));
        s.put("width", Json.num(p.settings.width));
        s.put("height", Json.num(p.settings.height));
        s.put("background", color(p.settings.background));
        if (p.settings.letterbox != 0) s.put("letterbox", Json.num(p.settings.letterbox));
        m.put("settings", s);
        if (!p.view.isDefault()) {
            Map<String, Object> v = obj();
            v.put("zoom", Json.num(p.view.zoom));
            v.put("panX", Json.num(p.view.panX));
            v.put("panY", Json.num(p.view.panY));
            v.put("frame", Json.num(p.view.frame));
            v.put("onionBefore", Json.num(p.view.onionBefore));
            v.put("onionAfter", Json.num(p.view.onionAfter));
            if (p.view.onionInFront) v.put("onionInFront", true);
            m.put("view", v);
        }
        m.put("rootSymbol", p.rootSymbol);
        List<Object> lib = new ArrayList<Object>();
        for (Symbol sym : p.library.values()) lib.add(symbol(sym));
        m.put("library", lib);
        List<Object> assets = new ArrayList<Object>();
        for (Asset a : p.assets.values()) {
            Map<String, Object> am = obj();
            am.put("id", a.id);
            am.put("kind", a.kind.name());
            am.put("path", a.path);
            assets.add(am);
        }
        m.put("assets", assets);
        List<Object> sets = new ArrayList<Object>();
        for (NodeSet ns : p.nodeSets.values()) {
            Map<String, Object> nm = obj();
            nm.put("id", ns.id);
            nm.put("name", ns.name);
            List<Object> mem = new ArrayList<Object>();
            for (String[] pair : ns.members) {
                List<Object> pl = new ArrayList<Object>();
                pl.add(pair[0]);
                pl.add(pair[1]);
                mem.add(pl);
            }
            nm.put("members", mem);
            sets.add(nm);
        }
        m.put("nodeSets", sets);
        return m;
    }

    private static Map<String, Object> symbol(Symbol s) {
        Map<String, Object> m = obj();
        m.put("id", s.id);
        m.put("name", s.name);
        m.put("duration", Json.num(s.duration));
        List<Object> layers = new ArrayList<Object>();
        for (Layer l : s.layers) layers.add(layer(l));
        m.put("layers", layers);
        List<Object> markers = new ArrayList<Object>();
        for (Marker mk : s.markers) {
            Map<String, Object> mm = obj();
            mm.put("time", Json.num(mk.time));
            mm.put("name", mk.name);
            mm.put("color", color(mk.color));
            markers.add(mm);
        }
        m.put("markers", markers);
        List<Object> scoped = new ArrayList<Object>();
        for (Symbol.ScopedStack ss : s.scopedEffects) {
            Map<String, Object> sm = obj();
            sm.put("scope", scope(ss.scope));
            sm.put("stack", stack(ss.stack));
            scoped.add(sm);
        }
        m.put("scopedEffects", scoped);
        return m;
    }

    private static Map<String, Object> layer(Layer l) {
        Map<String, Object> m = obj();
        m.put("id", l.id);
        m.put("name", l.name);
        m.put("type", l.type().name());
        m.put("visible", l.visible);
        m.put("locked", l.locked);
        m.put("solo", l.solo);
        m.put("onion", l.onion);
        m.put("blend", l.blend.name());
        m.put("parent", l.parent);
        if (l.parentNode != null) m.put("parentNode", l.parentNode);
        if (!l.parentNodeRotate) m.put("parentNodeRotate", false);
        if (l.inheritEffects) m.put("inheritEffects", true);
        if (l.parallax != 0) m.put("parallax", Json.num(l.parallax));
        m.put("shown", prop(l.shown, false));
        m.put("opacity", prop(l.opacity, false));
        Map<String, Object> t = obj();
        t.put("x", prop(l.transform.x, false));
        t.put("y", prop(l.transform.y, false));
        t.put("rotation", prop(l.transform.rotation, false));
        t.put("scaleX", prop(l.transform.scaleX, false));
        t.put("scaleY", prop(l.transform.scaleY, false));
        t.put("anchorX", prop(l.transform.anchorX, false));
        t.put("anchorY", prop(l.transform.anchorY, false));
        m.put("transform", t);
        m.put("effects", stack(l.effects));
        if (l instanceof Layer.FigureLayer) {
            List<Object> nodes = new ArrayList<Object>();
            for (Node n : ((Layer.FigureLayer) l).figure.nodes()) nodes.add(node(n));
            m.put("nodes", nodes);
            List<Object> fills = new ArrayList<Object>();
            for (app.barkfart.model.Polyfill pf : ((Layer.FigureLayer) l).figure.polyfills) {
                Map<String, Object> pm = obj();
                pm.put("id", pf.id);
                pm.put("anchor", pf.anchor);
                pm.put("nodes", new ArrayList<Object>(pf.nodes));
                pm.put("ownColor", pf.useOwnColor);
                pm.put("color", prop(pf.color, false));
                fills.add(pm);
            }
            m.put("polyfills", fills);
        } else if (l instanceof Layer.SpriteLayer) {
            Layer.SpriteLayer s = (Layer.SpriteLayer) l;
            m.put("asset", s.asset);
            m.put("width", Json.num(s.width));
            m.put("height", Json.num(s.height));
            m.put("tint", prop(s.tint, false));
            if (s.distorted()) {
                List<Object> dl = new ArrayList<Object>();
                for (app.barkfart.model.Property q : s.distort) dl.add(prop(q, false));
                m.put("distort", dl);
            }
        } else if (l instanceof Layer.TextLayer) {
            Layer.TextLayer s = (Layer.TextLayer) l;
            m.put("text", s.text);
            m.put("font", s.font);
            m.put("size", Json.num(s.size));
            m.put("color", prop(s.color, false));
            if (s.corner != 0) m.put("corner", Json.num(s.corner));
            if (s.aboveBars) m.put("aboveBars", true);
        } else if (l instanceof Layer.SymbolLayer) {
            Layer.SymbolLayer s = (Layer.SymbolLayer) l;
            m.put("symbol", s.symbol);
            m.put("timeOffset", prop(s.timeOffset, false));
            m.put("speed", prop(s.speed, false));
            m.put("loop", s.loop.name());
            m.put("recursionLimit", Json.num(s.recursionLimit));
        } else if (l instanceof Layer.AudioLayer) {
            List<Object> clips = new ArrayList<Object>();
            for (Clip c : ((Layer.AudioLayer) l).clips) {
                Map<String, Object> cm = obj();
                cm.put("id", c.id);
                cm.put("asset", c.asset);
                cm.put("start", Json.num(c.start));
                cm.put("in", Json.num(c.in));
                cm.put("out", Json.num(c.out));
                cm.put("loop", c.loop);
                cm.put("fadeIn", Json.num(c.fadeIn));
                cm.put("fadeOut", Json.num(c.fadeOut));
                if (c.pitch != 1) cm.put("pitch", Json.num(c.pitch));
                cm.put("volume", prop(c.volume, false));
                cm.put("pan", prop(c.pan, false));
                clips.add(cm);
            }
            m.put("clips", clips);
        } else if (l instanceof Layer.CameraLayer) {
            Layer.CameraLayer c = (Layer.CameraLayer) l;
            m.put("camX", prop(c.x, false));
            m.put("camY", prop(c.y, false));
            m.put("zoom", prop(c.zoom, false));
            m.put("camRotation", prop(c.rotation, false));
            if (!c.background.track().isEmpty() || ((Color) c.background.value()).a > 0) m.put("background", prop(c.background, false));
            if (!c.background2.track().isEmpty() || ((Color) c.background2.value()).a > 0) m.put("background2", prop(c.background2, false));
        }
        return m;
    }

    private static Map<String, Object> node(Node n) {
        Map<String, Object> m = obj();
        m.put("id", n.id);
        m.put("parent", n.parent);
        m.put("shape", n.shape.name());
        m.put("draw", Json.num(n.drawOrder));
        if (n.curve != 0) m.put("curve", Json.num(n.curve));
        if (n.triangleUpsideDown) m.put("upsideDown", true);
        if (n.rightTriangle != 0) m.put("rightTriangle", Json.num(n.rightTriangle));
        if (n.isStatic) m.put("static", true);
        if (n.stretchy) m.put("stretchy", true);
        if (n.smartStretch) m.put("smartStretch", true);
        if (n.noSmartStretch) m.put("noSmartStretch", true);
        if (n.restLength != 0) m.put("restLength", Json.num(n.restLength));
        if (n.circleHollow) m.put("hollow", true);
        if (n.circleOutline) m.put("outline", true);
        if (!n.outlineColor.track().isEmpty() || !app.barkfart.model.Color.BLACK.equals(n.outlineColor.value())) m.put("outlineColor", prop(n.outlineColor, false));
        if (n.halfArc != 0) m.put("halfArc", Json.num(n.halfArc));
        if (n.polygonSides != 0) m.put("sides", Json.num(n.polygonSides));
        if (n.trapezoidRatio != 1f) m.put("trapezoidRatio", Json.num(n.trapezoidRatio));
        if (n.trapezoidRoundStart) m.put("roundStart", true);
        if (n.trapezoidRoundEnd) m.put("roundEnd", true);
        if (n.angleLock != 0) m.put("angleLock", Json.num(n.angleLock));
        if (n.dragLock != 0) m.put("dragLock", Json.num(n.dragLock));
        if (n.pinned) m.put("pinned", true);
        // Saved whenever it carries information, even with the gradient off (exact legacy export).
        boolean gradColourSet = !n.gradientColor.track().isEmpty() || !app.barkfart.model.Color.WHITE.equals(n.gradientColor.value());
        if (n.gradient || n.gradientReverse || n.gradientSideways || gradColourSet) {
            Map<String, Object> gm = obj();
            gm.put("on", n.gradient);
            gm.put("reverse", n.gradientReverse);
            gm.put("sideways", n.gradientSideways);
            gm.put("color", prop(n.gradientColor, false));
            m.put("gradient", gm);
        }
        m.put("rotation", prop(n.rotation, false));
        m.put("length", prop(n.length, false));
        m.put("thickness", prop(n.thickness, false));
        m.put("scale", prop(n.scale, false));
        m.put("color", prop(n.color, false));
        m.put("shown", prop(n.shown, false));
        if (!n.legacyExtras.isEmpty()) {
            Map<String, Object> ex = obj();
            for (Map.Entry<String, Object> e : n.legacyExtras.entrySet()) ex.put(e.getKey(), extra(e.getValue()));
            m.put("legacy", ex);
        }
        return m;
    }

    /** Typed scalar for legacy extras: "i:21", "f:1.5", "b:true", "s:text". */
    private static String extra(Object v) {
        if (v instanceof Integer) return "i:" + v;
        if (v instanceof Float) return "f:" + Float.toString((Float) v);
        if (v instanceof Boolean) return "b:" + v;
        if (v instanceof String) return "s:" + v;
        throw new IllegalArgumentException("unsupported legacy extra " + v.getClass());
    }

    private static Map<String, Object> stack(EffectStack st) {
        Map<String, Object> m = obj();
        m.put("output", st.output);
        List<Object> nodes = new ArrayList<Object>();
        for (EffectNode e : st.nodes.values()) {
            Map<String, Object> em = obj();
            em.put("id", e.id);
            em.put("type", e.type);
            em.put("enabled", e.enabled);
            em.put("mix", prop(e.mix, false));
            Map<String, Object> pm = obj();
            for (Map.Entry<String, Property> pe : e.params.entrySet()) pm.put(pe.getKey(), prop(pe.getValue(), true));
            em.put("params", pm);
            em.put("inputs", new ArrayList<Object>(e.inputs));
            em.put("mask", e.mask == null ? null : scope(e.mask));
            nodes.add(em);
        }
        m.put("nodes", nodes);
        return m;
    }

    private static Map<String, Object> scope(Scope s) {
        Map<String, Object> m = obj();
        m.put("kind", s.kind.name());
        m.put("target", s.target);
        m.put("node", s.node);
        return m;
    }

    private static Map<String, Object> prop(Property p, boolean withType) {
        Map<String, Object> m = obj();
        if (withType) m.put("t", p.type.name());
        m.put("v", value(p.type, p.value()));
        if (!p.track().isEmpty()) {
            List<Object> keys = new ArrayList<Object>();
            for (Key k : p.track().keys()) {
                List<Object> km = new ArrayList<Object>();
                km.add(Json.num(k.time));
                km.add(value(p.type, k.value));
                km.add(k.interp.name().substring(0, 1));
                if (k.interp == Interp.BEZIER) {
                    km.add(Json.num(k.ease.x1));
                    km.add(Json.num(k.ease.y1));
                    km.add(Json.num(k.ease.x2));
                    km.add(Json.num(k.ease.y2));
                    if (k.ease.kind != Ease.Kind.CUBIC) km.add(k.ease.kind.name());
                }
                keys.add(km);
            }
            m.put("k", keys);
        }
        if (!p.modifiers().isEmpty()) {
            List<Object> mods = new ArrayList<Object>();
            for (Modifier mod : p.modifiers()) mods.add(modifier(mod));
            m.put("m", mods);
        }
        return m;
    }

    private static Map<String, Object> modifier(Modifier mod) {
        Map<String, Object> m = obj();
        if (mod instanceof Modifier.Wiggle) {
            Modifier.Wiggle w = (Modifier.Wiggle) mod;
            m.put("type", "wiggle");
            m.put("frequency", Json.num(w.frequency));
            m.put("amplitude", Json.num(w.amplitude));
            m.put("seed", Json.num(w.seed));
            m.put("octaves", Json.num(w.octaves));
        } else if (mod instanceof Modifier.Sine) {
            Modifier.Sine s = (Modifier.Sine) mod;
            m.put("type", "sine");
            m.put("frequency", Json.num(s.frequency));
            m.put("amplitude", Json.num(s.amplitude));
            m.put("phase", Json.num(s.phase));
        } else if (mod instanceof Modifier.Loop) {
            Modifier.Loop l = (Modifier.Loop) mod;
            m.put("type", "loop");
            m.put("mode", l.mode.name());
            m.put("start", Json.num(l.start));
            m.put("end", Json.num(l.end));
        } else if (mod instanceof Modifier.DelayFollow) {
            Modifier.DelayFollow d = (Modifier.DelayFollow) mod;
            m.put("type", "delayFollow");
            m.put("source", d.source);
            m.put("delay", Json.num(d.delay));
            m.put("scale", Json.num(d.scale));
            m.put("offset", Json.num(d.offset));
        } else {
            Modifier.Link l = (Modifier.Link) mod;
            m.put("type", "link");
            m.put("source", l.source);
            m.put("scale", Json.num(l.scale));
            m.put("offset", Json.num(l.offset));
        }
        m.put("enabled", mod.enabled);
        return m;
    }

    private static Object value(ValueType t, Object v) {
        switch (t) {
            case FLOAT:
            case ANGLE: return Json.num((Float) v);
            case DOUBLE: return Json.num((Double) v);
            case COLOR: return color((Color) v);
            default: return v;
        }
    }

    private static String color(Color c) {
        String h = Integer.toHexString(c.toRgba8888());
        while (h.length() < 8) h = "0" + h;
        return "#" + h;
    }

    // ================= decode =================

    /**
     * A deep copy of a layer under a new id (copy / paste, duplicate).
     * ids maps old layer ids to new ones: the layer's own id, its parent, and
     * property references in links (old + "/...") are rewritten when listed.
     */
    @SuppressWarnings("unchecked")
    public static Layer copyLayer(Layer l, Map<String, String> ids) throws FormatException {
        try {
            Object m = Json.parse(Json.write(layer(l)));
            return layer(asMap(remap(m, ids), "layer"));
        } catch (RuntimeException e) {
            throw new FormatException("cannot copy layer: " + e.getMessage(), e);
        }
    }

    @SuppressWarnings("unchecked")
    private static Object remap(Object o, Map<String, String> ids) {
        if (o instanceof Map) {
            Map<String, Object> out = new LinkedHashMap<String, Object>();
            for (Map.Entry<String, Object> e : ((Map<String, Object>) o).entrySet()) {
                boolean words = e.getKey().equals("text") || e.getKey().equals("name");   // user text is never an id
                out.put(e.getKey(), words ? e.getValue() : remap(e.getValue(), ids));
            }
            return out;
        }
        if (o instanceof List) {
            List<Object> out = new ArrayList<Object>();
            for (Object x : (List<Object>) o) out.add(remap(x, ids));
            return out;
        }
        if (o instanceof String) {
            String str = (String) o;
            if (ids.containsKey(str)) return ids.get(str);
            int slash = str.indexOf('/');
            if (slash > 0 && ids.containsKey(str.substring(0, slash))) return ids.get(str.substring(0, slash)) + str.substring(slash);
        }
        return o;
    }

    public static Project decode(String json) throws FormatException {
        try {
            Object root = Json.parse(json);
            return project(asMap(root, "project"));
        } catch (FormatException e) {
            throw e;
        } catch (RuntimeException e) {
            throw new FormatException("invalid project: " + e.getMessage(), e);
        }
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> asMap(Object o, String what) {
        if (!(o instanceof Map)) throw new IllegalArgumentException(what + " must be an object");
        return (Map<String, Object>) o;
    }

    @SuppressWarnings("unchecked")
    private static List<Object> list(Map<String, Object> m, String k) {
        Object o = m.get(k);
        if (o == null) return new ArrayList<Object>();
        if (!(o instanceof List)) throw new IllegalArgumentException(k + " must be a list");
        return (List<Object>) o;
    }

    private static String str(Map<String, Object> m, String k) {
        Object o = m.get(k);
        if (o == null) return null;
        if (!(o instanceof String)) throw new IllegalArgumentException(k + " must be a string");
        return (String) o;
    }

    private static String req(Map<String, Object> m, String k) {
        String s = str(m, k);
        if (s == null) throw new IllegalArgumentException("missing " + k);
        return s;
    }

    private static Json.Num num(Object o, String k) {
        if (!(o instanceof Json.Num)) throw new IllegalArgumentException(k + " must be a number");
        return (Json.Num) o;
    }

    private static float f(Map<String, Object> m, String k) {
        return num(m.get(k), k).f();
    }

    private static double d(Map<String, Object> m, String k) {
        return num(m.get(k), k).d();
    }

    private static int i(Map<String, Object> m, String k) {
        return num(m.get(k), k).i();
    }

    // Optional fields: absent means the model's default (format rule, see FORMAT.md).
    private static float f(Map<String, Object> m, String k, float def) {
        return m.containsKey(k) ? f(m, k) : def;
    }

    private static double d(Map<String, Object> m, String k, double def) {
        return m.containsKey(k) ? d(m, k) : def;
    }

    private static int i(Map<String, Object> m, String k, int def) {
        return m.containsKey(k) ? i(m, k) : def;
    }

    private static boolean b(Map<String, Object> m, String k, boolean def) {
        return m.containsKey(k) ? b(m, k) : def;
    }

    private static <E extends Enum<E>> E en(Map<String, Object> m, String k, Class<E> c, E def) {
        String v = str(m, k);
        return v == null ? def : Enum.valueOf(c, v);
    }

    private static boolean b(Map<String, Object> m, String k) {
        Object o = m.get(k);
        if (!(o instanceof Boolean)) throw new IllegalArgumentException(k + " must be a boolean");
        return (Boolean) o;
    }

    private static Project project(Map<String, Object> m) throws FormatException {
        if (!"barkfart".equals(m.get("format"))) throw new FormatException("not a Barkfart project", null);
        int v = i(m, "formatVersion");
        if (v > Project.FORMAT_VERSION) {
            throw new FormatException("project format " + v + " is newer than this app (" + Project.FORMAT_VERSION + ")", null);
        }
        m = Migrations.upgrade(m, v);
        Project p = new Project();
        Map<String, Object> s = m.containsKey("settings") ? asMap(m.get("settings"), "settings") : new LinkedHashMap<String, Object>();
        Project.Settings defaults = new Project.Settings();
        p.settings.fps = f(s, "fps", defaults.fps);
        p.settings.width = i(s, "width", defaults.width);
        p.settings.height = i(s, "height", defaults.height);
        p.settings.background = s.containsKey("background") ? parseColor(req(s, "background")) : defaults.background;
        p.settings.letterbox = f(s, "letterbox", 0);
        if (m.containsKey("view")) {
            Map<String, Object> vw = asMap(m.get("view"), "view");
            p.view.zoom = f(vw, "zoom", 1);
            p.view.panX = f(vw, "panX", 0);
            p.view.panY = f(vw, "panY", 0);
            p.view.frame = i(vw, "frame", 0);
            p.view.onionBefore = Math.max(0, Math.min(10, i(vw, "onionBefore", 2)));
            p.view.onionAfter = Math.max(0, Math.min(10, i(vw, "onionAfter", 1)));
            p.view.onionInFront = b(vw, "onionInFront", false);
        }
        p.rootSymbol = req(m, "rootSymbol");
        for (Object o : list(m, "assets")) {
            Map<String, Object> am = asMap(o, "asset");
            Asset a = new Asset(req(am, "id"), Asset.Kind.valueOf(req(am, "kind")), req(am, "path"));
            p.assets.put(a.id, a);
        }
        for (Object o : list(m, "library")) {
            Symbol sym = symbol(asMap(o, "symbol"));
            if (p.library.containsKey(sym.id)) throw new IllegalArgumentException("duplicate symbol " + sym.id);
            p.addSymbol(sym);
        }
        for (Object o : list(m, "nodeSets")) {
            Map<String, Object> nm = asMap(o, "nodeSet");
            NodeSet ns = new NodeSet(req(nm, "id"), str(nm, "name"));
            for (Object pair : list(nm, "members")) {
                @SuppressWarnings("unchecked")
                List<Object> pl = (List<Object>) pair;
                ns.members.add(new String[] {(String) pl.get(0), (String) pl.get(1)});
            }
            p.nodeSets.put(ns.id, ns);
        }
        if (!p.library.containsKey(p.rootSymbol)) throw new IllegalArgumentException("root symbol missing");
        p.setIdCounter(i(m, "idCounter", 1));
        p.invalidateIndex();
        return p;
    }

    private static Symbol symbol(Map<String, Object> m) {
        Symbol s = new Symbol(req(m, "id"), str(m, "name"), d(m, "duration", 1.0));
        for (Object o : list(m, "layers")) s.layers.add(layer(asMap(o, "layer")));
        for (Object o : list(m, "markers")) {
            Map<String, Object> mm = asMap(o, "marker");
            s.markers.add(new Marker(d(mm, "time", 0), str(mm, "name"),
                    mm.containsKey("color") ? parseColor(req(mm, "color")) : app.barkfart.model.Color.WHITE));
        }
        for (Object o : list(m, "scopedEffects")) {
            Map<String, Object> sm = asMap(o, "scopedEffect");
            EffectStack st = new EffectStack();
            Scope sc = scope(asMap(sm.get("scope"), "scope"));
            readStack(asMap(sm.get("stack"), "stack"), st, s.id + "/fx" + s.scopedEffects.size());
            s.scopedEffects.add(new Symbol.ScopedStack(sc, st));
        }
        return s;
    }

    private static Layer layer(Map<String, Object> m) {
        String id = req(m, "id"), name = str(m, "name");
        Layer.Type type = Layer.Type.valueOf(req(m, "type"));
        Layer l;
        switch (type) {
            case FIGURE: {
                Layer.FigureLayer fl = new Layer.FigureLayer(id, name);
                for (Object o : list(m, "nodes")) fl.figure.add(node(id, asMap(o, "node")));
                for (Object o : list(m, "polyfills")) {
                    Map<String, Object> pm = asMap(o, "polyfill");
                    app.barkfart.model.Polyfill pf = new app.barkfart.model.Polyfill(id, req(pm, "id"));
                    pf.anchor = req(pm, "anchor");
                    for (Object nid : list(pm, "nodes")) pf.nodes.add((String) nid);
                    pf.useOwnColor = b(pm, "ownColor", false);
                    readProp(pm, "color", pf.color);
                    fl.figure.polyfills.add(pf);
                }
                l = fl;
                break;
            }
            case SPRITE: {
                Layer.SpriteLayer s = new Layer.SpriteLayer(id, name);
                s.asset = str(m, "asset");
                s.width = f(m, "width", 0);
                s.height = f(m, "height", 0);
                readProp(m, "tint", s.tint);
                if (m.get("distort") instanceof List) {
                    List<?> dl = (List<?>) m.get("distort");
                    for (int k = 0; k < Math.min(8, dl.size()); k++) {
                        if (dl.get(k) instanceof Map) {
                            Map<String, Object> dm = asMap(dl.get(k), "distort");
                            s.distort[k].setValue(parseValue(s.distort[k].type, dm.get("v")));
                            fillProp(dm, s.distort[k]);
                        }
                    }
                }
                l = s;
                break;
            }
            case TEXT: {
                Layer.TextLayer s = new Layer.TextLayer(id, name);
                s.text = str(m, "text");
                s.font = str(m, "font");
                s.size = f(m, "size", s.size);
                readProp(m, "color", s.color);
                s.corner = Math.max(0, Math.min(4, i(m, "corner", 0)));
                s.aboveBars = b(m, "aboveBars", false);
                l = s;
                break;
            }
            case SYMBOL: {
                Layer.SymbolLayer s = new Layer.SymbolLayer(id, name, req(m, "symbol"));
                readProp(m, "timeOffset", s.timeOffset);
                readProp(m, "speed", s.speed);
                s.loop = en(m, "loop", Layer.SymbolLayer.LoopMode.class, s.loop);
                s.recursionLimit = i(m, "recursionLimit", s.recursionLimit);
                l = s;
                break;
            }
            case ADJUSTMENT:
                l = new Layer.AdjustmentLayer(id, name);
                break;
            case AUDIO: {
                Layer.AudioLayer a = new Layer.AudioLayer(id, name);
                for (Object o : list(m, "clips")) {
                    Map<String, Object> cm = asMap(o, "clip");
                    Clip c = new Clip(id, req(cm, "id"), str(cm, "asset"), d(cm, "start", 0), d(cm, "in", 0), d(cm, "out", 0));
                    c.pitch = d(cm, "pitch", 1);
                    c.loop = b(cm, "loop", false);
                    c.fadeIn = d(cm, "fadeIn", 0);
                    c.fadeOut = d(cm, "fadeOut", 0);
                    readProp(cm, "volume", c.volume);
                    readProp(cm, "pan", c.pan);
                    a.clips.add(c);
                }
                l = a;
                break;
            }
            default: {
                Layer.CameraLayer c = new Layer.CameraLayer(id, name);
                readProp(m, "camX", c.x);
                readProp(m, "camY", c.y);
                readProp(m, "zoom", c.zoom);
                readProp(m, "camRotation", c.rotation);
                readProp(m, "background", c.background);
                readProp(m, "background2", c.background2);
                l = c;
            }
        }
        l.visible = b(m, "visible", true);
        l.locked = b(m, "locked", false);
        l.solo = b(m, "solo", false);
        l.onion = b(m, "onion", true);
        l.blend = en(m, "blend", Layer.Blend.class, Layer.Blend.NORMAL);
        l.parent = str(m, "parent");
        l.parentNode = str(m, "parentNode");
        l.parentNodeRotate = b(m, "parentNodeRotate", true);
        l.inheritEffects = b(m, "inheritEffects", false);
        l.parallax = f(m, "parallax", 0f);
        readProp(m, "shown", l.shown);
        readProp(m, "opacity", l.opacity);
        Map<String, Object> t = m.containsKey("transform") ? asMap(m.get("transform"), "transform") : new LinkedHashMap<String, Object>();
        readProp(t, "x", l.transform.x);
        readProp(t, "y", l.transform.y);
        readProp(t, "rotation", l.transform.rotation);
        readProp(t, "scaleX", l.transform.scaleX);
        readProp(t, "scaleY", l.transform.scaleY);
        readProp(t, "anchorX", l.transform.anchorX);
        readProp(t, "anchorY", l.transform.anchorY);
        if (m.containsKey("effects")) readStack(asMap(m.get("effects"), "effects"), l.effects, id);
        return l;
    }

    private static Node node(String layerId, Map<String, Object> m) {
        Node n = new Node(layerId, req(m, "id"), str(m, "parent"), en(m, "shape", Node.Shape.class, Node.Shape.LINE));
        n.drawOrder = i(m, "draw", Node.UNASSIGNED);
        n.curve = f(m, "curve", 0);
        n.triangleUpsideDown = b(m, "upsideDown", false);
        n.rightTriangle = i(m, "rightTriangle", 0);
        n.isStatic = b(m, "static", false);
        n.stretchy = b(m, "stretchy", false);
        n.smartStretch = b(m, "smartStretch", false);
        n.noSmartStretch = b(m, "noSmartStretch", false);
        n.restLength = f(m, "restLength", 0);
        n.circleHollow = b(m, "hollow", false);
        n.circleOutline = b(m, "outline", false);
        if (m.containsKey("outlineColor")) readProp(m, "outlineColor", n.outlineColor);
        n.halfArc = i(m, "halfArc", 0);
        n.polygonSides = i(m, "sides", 0);
        n.trapezoidRatio = f(m, "trapezoidRatio", 1f);
        n.trapezoidRoundStart = b(m, "roundStart", false);
        n.trapezoidRoundEnd = b(m, "roundEnd", false);
        n.angleLock = i(m, "angleLock", 0);
        n.dragLock = i(m, "dragLock", 0);
        n.pinned = b(m, "pinned", false);
        if (m.containsKey("gradient")) {
            Map<String, Object> gm = asMap(m.get("gradient"), "gradient");
            n.gradient = b(gm, "on", true);
            n.gradientReverse = b(gm, "reverse", false);
            n.gradientSideways = b(gm, "sideways", false);
            readProp(gm, "color", n.gradientColor);
        }
        readProp(m, "rotation", n.rotation);
        readProp(m, "length", n.length);
        readProp(m, "thickness", n.thickness);
        readProp(m, "scale", n.scale);
        readProp(m, "color", n.color);
        readProp(m, "shown", n.shown);
        Object ex = m.get("legacy");
        if (ex != null) {
            for (Map.Entry<String, Object> e : asMap(ex, "legacy").entrySet()) {
                n.legacyExtras.put(e.getKey(), parseExtra((String) e.getValue()));
            }
        }
        return n;
    }

    private static Object parseExtra(String s) {
        String body = s.substring(2);
        switch (s.charAt(0)) {
            case 'i': return Integer.parseInt(body);
            case 'f': return Float.parseFloat(body);
            case 'b': return Boolean.parseBoolean(body);
            case 's': return body;
            default: throw new IllegalArgumentException("bad legacy extra " + s);
        }
    }

    private static void readStack(Map<String, Object> m, EffectStack st, String owner) {
        for (Object o : list(m, "nodes")) {
            Map<String, Object> em = asMap(o, "effect");
            EffectNode e = new EffectNode(owner, req(em, "id"), req(em, "type"));
            e.enabled = b(em, "enabled", true);
            readProp(em, "mix", e.mix);
            Map<String, Object> params = em.containsKey("params") ? asMap(em.get("params"), "params") : new LinkedHashMap<String, Object>();
            for (Map.Entry<String, Object> pe : params.entrySet()) {
                Map<String, Object> pm = asMap(pe.getValue(), "param");
                ValueType vt = ValueType.valueOf(req(pm, "t"));
                Property p = e.param(owner, pe.getKey(), vt, parseValue(vt, pm.get("v")));
                fillProp(pm, p);
            }
            for (Object in : list(em, "inputs")) e.inputs.add((String) in);
            Object mask = em.get("mask");
            if (mask != null) e.mask = scope(asMap(mask, "mask"));
            if (st.nodes.containsKey(e.id)) throw new IllegalArgumentException("duplicate effect " + e.id);
            st.nodes.put(e.id, e);
        }
        st.output = str(m, "output");
    }

    private static Scope scope(Map<String, Object> m) {
        return new Scope(Scope.Kind.valueOf(req(m, "kind")), str(m, "target"), str(m, "node"));
    }

    private static void readProp(Map<String, Object> owner, String k, Property p) {
        Object o = owner.get(k);
        if (o == null) return;  // absent: keep default
        Map<String, Object> m = asMap(o, k);
        p.setValue(parseValue(p.type, m.get("v")));
        fillProp(m, p);
    }

    @SuppressWarnings("unchecked")
    private static void fillProp(Map<String, Object> m, Property p) {
        for (Object ko : list(m, "k")) {
            List<Object> k = (List<Object>) ko;
            double time = num(k.get(0), "key time").d();
            Object v = parseValue(p.type, k.get(1));
            String code = (String) k.get(2);
            Interp in = "H".equals(code) ? Interp.HOLD : ("B".equals(code) ? Interp.BEZIER : Interp.LINEAR);
            Ease ease = Ease.LINEAR;
            if (in == Interp.BEZIER) {
                ease = new Ease(num(k.get(3), "x1").f(), num(k.get(4), "y1").f(), num(k.get(5), "x2").f(), num(k.get(6), "y2").f());
                if (k.size() > 7 && k.get(7) instanceof String) {   // bounce / elastic (optional; unknown names stay cubic)
                    for (Ease.Kind kind : Ease.Kind.values()) if (kind.name().equals(k.get(7))) ease = new Ease(kind);
                }
            }
            p.track().put(new Key(time, v, in, ease));
        }
        for (Object mo : list(m, "m")) p.addModifier(parseModifier(asMap(mo, "modifier")));
    }

    private static Modifier parseModifier(Map<String, Object> m) {
        String type = req(m, "type");
        Modifier mod;
        if ("wiggle".equals(type)) {
            mod = new Modifier.Wiggle(f(m, "frequency", 1), f(m, "amplitude", 0), i(m, "seed", 0), i(m, "octaves", 1));
        } else if ("sine".equals(type)) {
            mod = new Modifier.Sine(f(m, "frequency", 1), f(m, "amplitude", 0), f(m, "phase", 0));
        } else if ("loop".equals(type)) {
            mod = new Modifier.Loop(en(m, "mode", Modifier.Loop.Mode.class, Modifier.Loop.Mode.CYCLE), d(m, "start", 0), d(m, "end", 1));
        } else if ("delayFollow".equals(type)) {
            mod = new Modifier.DelayFollow(req(m, "source"), d(m, "delay", 0), f(m, "scale", 1), f(m, "offset", 0));
        } else if ("link".equals(type)) {
            mod = new Modifier.Link(req(m, "source"), f(m, "scale", 1), f(m, "offset", 0));
        } else {
            throw new IllegalArgumentException("unknown modifier " + type);
        }
        mod.enabled = b(m, "enabled", true);
        return mod;
    }

    private static Object parseValue(ValueType t, Object o) {
        switch (t) {
            case FLOAT:
            case ANGLE: return num(o, "value").f();
            case DOUBLE: return num(o, "value").d();
            case COLOR:
                if (!(o instanceof String)) throw new IllegalArgumentException("colour must be a string");
                return parseColor((String) o);
            default:
                if (!(o instanceof Boolean)) throw new IllegalArgumentException("bool value expected");
                return o;
        }
    }

    private static Color parseColor(String s) {
        if (s.length() != 9 || s.charAt(0) != '#') throw new IllegalArgumentException("bad colour " + s);
        return Color.rgba8888((int) Long.parseLong(s.substring(1), 16));
    }
}
