package app.barkfart.fx;

import app.barkfart.model.Color;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import app.barkfart.model.ValueType;

/**
 * A small scene using every effect type plus a translucent overlap and an
 * adjustment layer, 160 x 180 view units around the origin. Used to compare
 * the GL renderer with the reference (desktop --fxcheck and the launch
 * self-test on the device).
 */
public final class EffectScene {
    private EffectScene() {}

    public static final int W = 160, H = 180;

    public static Project build() {
        Project p = new Project();
        Symbol root = p.addSymbol(new Symbol(p.newId("S"), "fx", 1));
        p.rootSymbol = root.id;
        addTo(p, root, 1);
        return p;
    }

    /** Adds the scene's layers to a symbol, ids from p, positions and sizes times k. */
    public static void addTo(Project p, Symbol root, float k) {
        int first = root.layers.size();
        Layer.FigureLayer blur = square(p, root, -50, 60, 0x0a84ffff, 1f);
        fx(blur, EffectTypes.BLUR).param(blur.id, "radius", ValueType.FLOAT, 8f);
        Layer.FigureLayer glow = square(p, root, 0, 60, 0x000000ff, 1f);
        EffectNode g = fx(glow, EffectTypes.GLOW);
        g.param(glow.id, "radius", ValueType.FLOAT, 8f);
        g.param(glow.id, "color", ValueType.COLOR, Color.rgba8888(0xffd60aff));
        Layer.FigureLayer shadow = square(p, root, 50, 60, 0xff3b30ff, 1f);
        EffectNode sh = fx(shadow, EffectTypes.SHADOW);
        sh.param(shadow.id, "dx", ValueType.FLOAT, 6f);
        sh.param(shadow.id, "dy", ValueType.FLOAT, -6f);
        sh.param(shadow.id, "radius", ValueType.FLOAT, 4f);
        Layer.FigureLayer see = square(p, root, -50, 0, 0x30d158ff, 0.5f);
        Node extra = new Node(see.id, "x2", "r", Node.Shape.SQUARE_LINE);
        extra.rotation.setValue(90f);
        extra.length.setValue(30f);
        extra.thickness.setValue(24f);
        extra.color.setValue(Color.rgba8888(0x30d158ff));
        see.figure.add(extra);
        Layer.FigureLayer adj = square(p, root, 0, 0, 0xff9f0aff, 1f);
        EffectNode a = fx(adj, EffectTypes.ADJUST);
        a.param(adj.id, "hue", ValueType.FLOAT, 180f);
        a.param(adj.id, "saturation", ValueType.FLOAT, 0.5f);
        EffectNode t = fx(adj, EffectTypes.TINT);
        t.param(adj.id, "color", ValueType.COLOR, Color.rgba8888(0x5e5ce6ff));
        t.param(adj.id, "amount", ValueType.FLOAT, 0.3f);
        t.mix.setValue(0.5f);
        square(p, root, 50, 0, 0xffffffff, 1f);
        Layer.FigureLayer px = square(p, root, -50, -60, 0x5e5ce6ff, 1f);
        fx(px, EffectTypes.PIXELATE).param(px.id, "size", ValueType.FLOAT, 7f);
        Node stripe = new Node(px.id, "x2", "r", Node.Shape.SQUARE_LINE);
        stripe.rotation.setValue(45f);
        stripe.length.setValue(20f);
        stripe.thickness.setValue(6f);
        stripe.color.setValue(Color.rgba8888(0xffd60aff));
        px.figure.add(stripe);
        Layer.FigureLayer ol = square(p, root, 0, -60, 0xffffffff, 1f);
        EffectNode o = fx(ol, EffectTypes.OUTLINE);
        o.param(ol.id, "width", ValueType.FLOAT, 4f);
        o.param(ol.id, "color", ValueType.COLOR, Color.rgba8888(0x30d158ff));
        Layer.FigureLayer mb = square(p, root, 50, -60, 0xff2d55ff, 1f);
        EffectNode m = fx(mb, EffectTypes.MOTION);
        m.param(mb.id, "angle", ValueType.FLOAT, 30f);
        m.param(mb.id, "distance", ValueType.FLOAT, 16f);
        Layer.AdjustmentLayer inv = new Layer.AdjustmentLayer(p.newId("L"), "invert");
        EffectNode i = new EffectNode(inv.id, "fx", EffectTypes.INVERT);
        inv.effects.append(i);
        inv.opacity.setValue(0.8f);
        root.layers.add(inv);
        for (int li = first; li < root.layers.size(); li++) {
            Layer l = root.layers.get(li);
            l.transform.x.setValue(l.transform.x.floatValue() * k);
            l.transform.y.setValue(l.transform.y.floatValue() * k);
            l.transform.scaleX.setValue(k);
            l.transform.scaleY.setValue(k);
        }
        p.invalidateIndex();
    }

    private static Layer.FigureLayer square(Project p, Symbol root, float cx, float cy, int rgba, float opacity) {
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), "sq");
        l.figure.add(new Node(l.id, "r", null, Node.Shape.NONE));
        Node s = new Node(l.id, "s", "r", Node.Shape.SQUARE_LINE);
        s.length.setValue(24f);
        s.thickness.setValue(24f);
        s.color.setValue(Color.rgba8888(rgba));
        l.figure.add(s);
        l.transform.x.setValue(cx - 12);
        l.transform.y.setValue(cy);
        l.opacity.setValue(opacity);
        root.layers.add(l);
        return l;
    }

    private static EffectNode fx(Layer l, String type) {
        EffectNode e = new EffectNode(l.id, "fx" + l.effects.nodes.size(), type);
        l.effects.append(e);
        return e;
    }

    /**
     * Compares premultiplied RGBA8 pixels (rows bottom-up, as glReadPixels gives
     * them) with the reference image. Returns null when they agree: mean channel
     * difference under 2 and under 1% of pixels off by more than 48 (edge
     * rasterization differs slightly between GL and the reference).
     */
    public static String compare(byte[] glRgba, Image ref) {
        long sum = 0;
        int bad = 0, n = ref.w * ref.h;
        for (int y = 0; y < ref.h; y++) {
            for (int x = 0; x < ref.w; x++) {
                int go = ((ref.h - 1 - y) * ref.w + x) * 4, ro = (y * ref.w + x) * 4;
                int worst = 0;
                for (int c = 0; c < 4; c++) {
                    int g = glRgba[go + c] & 0xff;
                    int r = Math.round(Kernels.clamp01(ref.px[ro + c]) * 255);
                    int d = Math.abs(g - r);
                    sum += d;
                    worst = Math.max(worst, d);
                }
                if (worst > 48) bad++;
            }
        }
        double mean = sum / (4.0 * n);
        if (mean < 2 && bad < n / 100) return null;
        return String.format("effects differ from reference: mean %.1f, %d of %d pixels off", mean, bad, n);
    }
}
