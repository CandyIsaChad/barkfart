package app.barkfart.legacy;

import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.io.IOException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Maps legacy stickfigures to figure layers and back.
 *
 * Geometry: the length field and the local angle (relative to the parent,
 * degrees, counter-clockwise, y up) are authoritative. Stored local x/y
 * offsets are a cached value at 0.5x or 1x of the length depending on the
 * saving device, so they are kept only for faithful re-export. The figure
 * scale becomes the layer's scale.
 *
 * Every legacy field is kept in Node.legacyExtras ("L." prefix; connector
 * fields "C."; figure header on the root as "fig.*"), so an unedited figure
 * exports byte for byte.
 */
public final class FigureMapper {
    private FigureMapper() {}

    /** Shapes Barkfart cannot draw yet are approximated; this lists them per import. */
    public static final class Report {
        public final List<String> approximations = new ArrayList<String>();
    }

    public static Layer.FigureLayer toLayer(Project p, NodesFormat.LegacyFigure fig, String name, Report report) {
        Layer.FigureLayer layer = new Layer.FigureLayer(p.newId("L"), name);
        layer.transform.scaleX.setValue(fig.scale);
        layer.transform.scaleY.setValue(fig.scale);
        Color figColor = color(fig.color);
        // Depth-first with parents first; explicit stack.
        ArrayList<Object[]> stack = new ArrayList<Object[]>();
        stack.add(new Object[] {fig.root, null});
        while (!stack.isEmpty()) {
            Object[] top = stack.remove(stack.size() - 1);
            NodesFormat.LegacyNode ln = (NodesFormat.LegacyNode) top[0];
            String parent = (String) top[1];
            int type = ln.i("type");
            Node n = new Node(layer.id, "n" + ln.i("drawIndex"), parent, shape(type, report));
            n.drawOrder = ln.i("drawIndex");
            n.rotation.setValue(ln.f("localAngle"));
            n.length.setValue(ln.f("length"));
            n.thickness.setValue((float) ln.i("thickness"));
            n.curve = ln.i("curveRadius");
            n.triangleUpsideDown = ln.i("triangleUpsideDown") != 0;
            n.rightTriangle = Integer.signum(ln.i("rightTriangleDirection"));
            n.gradient = ln.i("gradient") != 0;
            n.gradientReverse = ln.i("reverseGradient") != 0;
            n.gradientSideways = ln.i("gradientMode") == 1;
            // Segments without their own colour follow the figure colour at both gradient ends
            // (fitted against the original's previews, NOTES.md); the stored value is kept for export.
            if (ln.fields.containsKey("gradientColor")) {
                n.gradientColor.setValue(ln.i("useSegmentColor") != 0 ? color(ln.i("gradientColor")) : figColor);
            }
            n.color.setValue(ln.i("useSegmentColor") != 0 ? color(ln.i("color")) : figColor);
            for (Map.Entry<String, Object> e : ln.fields.entrySet()) {
                if (!e.getKey().startsWith("~")) n.legacyExtras.put("L." + e.getKey(), e.getValue());
            }
            if (ln.isConnector) {
                n.legacyExtras.put("connector", Boolean.TRUE);
                for (Map.Entry<String, Object> e : ln.connector.entrySet()) n.legacyExtras.put("C." + e.getKey(), e.getValue());
            }
            if (parent == null) putHeader(n, fig);
            layer.figure.add(n);
            for (int i = ln.children.size() - 1; i >= 0; i--) stack.add(new Object[] {ln.children.get(i), n.id});
        }
        int k = 0;
        for (NodesFormat.Polyfill lp : fig.polyfills) {
            app.barkfart.model.Polyfill pf = new app.barkfart.model.Polyfill(layer.id, "pf" + (k++));
            pf.anchor = "n" + lp.anchor;
            for (int nid : lp.nodes) pf.nodes.add("n" + nid);
            pf.useOwnColor = lp.useColor != 0;
            pf.color.setValue(color(lp.color));
            layer.figure.polyfills.add(pf);
        }
        p.invalidateIndex();
        return layer;
    }

    private static Node.Shape shape(int type, Report report) {
        switch (type) {
            case -1: return Node.Shape.NONE;
            case 0: return Node.Shape.LINE;
            case 2:
            case 4: return Node.Shape.CIRCLE;
            case 1: return Node.Shape.SQUARE_LINE;
            case 3: return Node.Shape.TRIANGLE;
            case 5: return Node.Shape.ELLIPSE;
            default:
                String what = type == 6 ? "trapezoid" : type == 7 ? "polygon" : "unknown type " + type;
                if (report != null && !report.approximations.contains(what)) report.approximations.add(what);
                return Node.Shape.LINE;
        }
    }

    /** Legacy colour int: bytes A, B, G, R from the high byte down. */
    static Color color(int c) {
        int a = (c >>> 24) & 0xff, b = (c >>> 16) & 0xff, g = (c >>> 8) & 0xff, r = c & 0xff;
        return Color.rgba8888((r << 24) | (g << 16) | (b << 8) | a);
    }

    static int legacyColor(Color c) {
        int v = c.toRgba8888();
        int r = (v >>> 24) & 0xff, g = (v >>> 16) & 0xff, b = (v >>> 8) & 0xff, a = v & 0xff;
        return (a << 24) | (b << 16) | (g << 8) | r;
    }

    private static void putHeader(Node root, NodesFormat.LegacyFigure fig) {
        root.legacyExtras.put("fig.version", fig.version);
        root.legacyExtras.put("fig.build", fig.build);
        root.legacyExtras.put("fig.color", fig.color);
        root.legacyExtras.put("fig.scale", fig.scale);
        root.legacyExtras.put("fig.little", fig.littleEndian);
        root.legacyExtras.put("fig.compressed", fig.compressed);
        StringBuilder cn = new StringBuilder();
        for (int[] c : fig.connections) cn.append(cn.length() == 0 ? "" : ";").append(c[0]).append(',').append(c[1]);
        root.legacyExtras.put("fig.connections", cn.toString());
    }

    /**
     * Builds a legacy figure from the layer's static values (keys are ignored).
     * Nodes made in Barkfart get defaults for legacy-only fields.
     */
    public static NodesFormat.LegacyFigure toLegacy(Layer.FigureLayer layer) throws IOException {
        List<Node> nodes = layer.figure.nodes();
        if (nodes.isEmpty()) throw new IOException("empty figure");
        Node root = nodes.get(0);
        NodesFormat.LegacyFigure fig = new NodesFormat.LegacyFigure();
        fig.version = intExtra(root, "fig.version", 420);
        fig.build = intExtra(root, "fig.build", fig.version >= 403 ? 62 : 0);
        fig.littleEndian = boolExtra(root, "fig.little", fig.version < 160);
        fig.compressed = boolExtra(root, "fig.compressed", fig.version < 160);
        float scale = layer.transform.scaleX.floatValue();
        Object oldScale = root.legacyExtras.get("fig.scale");
        fig.scale = oldScale instanceof Float && (Float) oldScale == scale ? (Float) oldScale : scale;
        int figColor = intExtra(root, "fig.color", legacyColor(Color.BLACK));
        fig.color = figColor;
        Color figColorModel = color(figColor);

        Map<String, NodesFormat.LegacyNode> byId = new HashMap<String, NodesFormat.LegacyNode>();
        Map<String, Double> worldAngle = new HashMap<String, Double>();
        for (Node n : nodes) {
            NodesFormat.LegacyNode ln = new NodesFormat.LegacyNode();
            boolean isRoot = n.parent == null;
            for (NodesFormat.Field f : NodesFormat.NODE) {
                if (!f.present(fig.version, fig.build)) continue;
                Object v = n.legacyExtras.get("L." + f.name);
                if (v == null) v = defaultValue(f);
                ln.fields.put(f.name, v);
            }
            ln.isConnector = Boolean.TRUE.equals(n.legacyExtras.get("connector"));
            if (ln.isConnector) {
                for (NodesFormat.Field f : NodesFormat.CONNECTOR) {
                    if (!f.present(fig.version, fig.build)) continue;
                    Object v = n.legacyExtras.get("C." + f.name);
                    ln.connector.put(f.name, v == null ? defaultValue(f) : v);
                }
            }
            ln.fields.put("type", isRoot ? -1 : legacyType(n, ln.i("type")));
            ln.fields.put("drawIndex", isRoot ? 0 : n.drawOrder);
            setIfChanged(ln, "localAngle", n.rotation.floatValue());
            setIfChanged(ln, "length", n.length.floatValue());
            if (ln.fields.containsKey("curveRadius") && ln.i("curveRadius") != Math.round(n.curve)) {
                ln.fields.put("curveRadius", Math.round(n.curve));
            }
            if (ln.fields.containsKey("triangleUpsideDown") && (ln.i("triangleUpsideDown") != 0) != n.triangleUpsideDown) {
                ln.fields.put("triangleUpsideDown", n.triangleUpsideDown ? 1 : 0);
            }
            if (ln.fields.containsKey("rightTriangleDirection") && Integer.signum(ln.i("rightTriangleDirection")) != n.rightTriangle) {
                ln.fields.put("rightTriangleDirection", n.rightTriangle);
            }
            if (ln.fields.containsKey("gradient") && (ln.i("gradient") != 0) != n.gradient) ln.fields.put("gradient", n.gradient ? 1 : 0);
            if (ln.fields.containsKey("reverseGradient") && (ln.i("reverseGradient") != 0) != n.gradientReverse) {
                ln.fields.put("reverseGradient", n.gradientReverse ? 1 : 0);
            }
            if (ln.fields.containsKey("gradientMode") && (ln.i("gradientMode") == 1) != n.gradientSideways) {
                ln.fields.put("gradientMode", n.gradientSideways ? 1 : 0);
            }
            if (ln.fields.containsKey("gradientColor")) {
                Color imported = ln.i("useSegmentColor") != 0 ? color(ln.i("gradientColor")) : figColorModel;
                if (!imported.equals(n.gradientColor.value())) ln.fields.put("gradientColor", legacyColor((Color) n.gradientColor.value()));
            }
            int thick = Math.round(n.thickness.floatValue());
            if (ln.i("thickness") != thick) ln.fields.put("thickness", thick);
            Color c = (Color) n.color.value();
            boolean segColor = ln.i("useSegmentColor") != 0;
            if (segColor) {
                if (!color(ln.i("color")).equals(c)) ln.fields.put("color", legacyColor(c));
            } else if (!c.equals(figColorModel)) {
                ln.fields.put("useSegmentColor", 1);
                ln.fields.put("color", legacyColor(c));
            }
            double world = (isRoot ? 0 : worldAngle.get(n.parent)) + n.rotation.floatValue();
            worldAngle.put(n.id, world);
            if (!isRoot) refreshOffset(ln, n, world);
            byId.put(n.id, ln);
            if (isRoot) fig.root = ln;
            else byId.get(n.parent).children.add(ln);
        }
        for (app.barkfart.model.Polyfill mp : layer.figure.polyfills) {
            NodesFormat.Polyfill p = new NodesFormat.Polyfill();
            p.anchor = drawIndexOf(layer, mp.anchor);
            p.color = legacyColor((Color) mp.color.value());
            p.useColor = mp.useOwnColor ? 1 : 0;
            for (String nid : mp.nodes) p.nodes.add(drawIndexOf(layer, nid));
            fig.polyfills.add(p);
        }
        String cn = strExtra(root, "fig.connections");
        if (cn != null && !cn.isEmpty()) {
            for (String part : cn.split(";")) {
                String[] q = part.split(",");
                fig.connections.add(new int[] {Integer.parseInt(q[0]), Integer.parseInt(q[1])});
            }
        }
        return fig;
    }

    private static int legacyType(Node n, int previous) {
        switch (n.shape) {
            case CIRCLE: return previous == 2 || previous == 4 ? previous : 2;
            case SQUARE_LINE: return 1;
            case TRIANGLE: return 3;
            case ELLIPSE: return 5;
            case LINE: return previous == 0 || previous == 6 || previous == 7 ? previous : 0;
            default: return previous;
        }
    }

    /** Keeps the stored float bits unless the value really changed. */
    private static void setIfChanged(NodesFormat.LegacyNode ln, String k, float v) {
        if (ln.f(k) != v) ln.fields.put(k, v);
    }

    /** Recomputes the cached offset only when length or angle changed, keeping its 0.5x/1x scale. */
    private static void refreshOffset(NodesFormat.LegacyNode ln, Node n, double world) {
        float lx = ln.f("localX"), ly = ln.f("localY"), len = n.length.floatValue();
        double h = Math.hypot(lx, ly);
        double ratio = h > 1e-6 && len > 1e-6 ? (Math.abs(h / len - 1) < 0.01 ? 1 : 0.5) : 0.5;
        double want = ratio * len;
        double ang = Math.toDegrees(Math.atan2(ly, lx));
        double dAng = Math.abs(((ang - world) % 360 + 540) % 360 - 180);
        if (Math.abs(h - want) <= 0.01 * Math.max(1, want) && (h < 1e-6 || dAng < 0.05)) return;
        double rad = Math.toRadians(world);
        ln.fields.put("localX", (float) (want * Math.cos(rad)));
        ln.fields.put("localY", (float) (want * Math.sin(rad)));
    }

    private static int drawIndexOf(Layer.FigureLayer layer, String nodeId) throws IOException {
        Node n = layer.figure.node(nodeId);
        if (n == null) throw new IOException("polyfill refers to missing node " + nodeId);
        return n.parent == null ? 0 : n.drawOrder;
    }

    private static Object defaultValue(NodesFormat.Field f) {
        if (f.kind == NodesFormat.Kind.F32) {
            if (f.name.equals("scale") || f.name.equals("smartStretchMultiplier") || f.name.equals("trapezoidTopRatio")) return 1f;
            return 0f;
        }
        if (f.name.equals("useSegmentScale")) return 1;
        return 0;
    }

    private static int intExtra(Node n, String k, int def) {
        Object o = n.legacyExtras.get(k);
        return o instanceof Integer ? (Integer) o : def;
    }

    private static boolean boolExtra(Node n, String k, boolean def) {
        Object o = n.legacyExtras.get(k);
        return o instanceof Boolean ? (Boolean) o : def;
    }

    private static String strExtra(Node n, String k) {
        Object o = n.legacyExtras.get(k);
        return o instanceof String ? (String) o : null;
    }
}
