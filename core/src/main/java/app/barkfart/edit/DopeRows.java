package app.barkfart.edit;

import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/**
 * Rows of the dope sheet (UI-SPEC "Dope sheet"): a summary row, then per
 * figure a header row and one row per joint (rotation + length), a Move row
 * (layer position) and a Camera row. Headless so it can be tested.
 */
public final class DopeRows {
    public enum Kind { SUMMARY, FIGURE, JOINT, MOVE, CAMERA, SOUND }

    public static final class Row {
        public final Kind kind;
        public final String label;
        public final int depth;
        public final Layer layer;
        public final List<Property> props;
        /** SOUND rows: the clip drawn as a bar. */
        public final app.barkfart.model.Clip clip;

        Row(Kind kind, String label, int depth, Layer layer, List<Property> props) {
            this(kind, label, depth, layer, props, null);
        }

        Row(Kind kind, String label, int depth, Layer layer, List<Property> props, app.barkfart.model.Clip clip) {
            this.kind = kind;
            this.label = label;
            this.depth = depth;
            this.layer = layer;
            this.props = props;
            this.clip = clip;
        }

        /** Distinct key times across this row's properties, ascending. */
        public List<Double> times() {
            TreeSet<Double> t = new TreeSet<Double>();
            for (Property p : props) for (Key k : p.track().keys()) t.add(k.time);
            return new ArrayList<Double>(t);
        }

        /** All keys of this row at time t (within eps). */
        public List<KeyRef> keysAt(double t, double eps) {
            List<KeyRef> out = new ArrayList<KeyRef>();
            for (Property p : props) for (Key k : p.track().keys()) if (Math.abs(k.time - t) <= eps) out.add(new KeyRef(p, k.time));
            return out;
        }
    }

    private DopeRows() {}

    /**
     * Builds rows. If only is non-null, just that figure is listed (plus the
     * camera). collapsed holds layer ids whose joint rows are hidden.
     */
    public static List<Row> build(Project p, Layer.FigureLayer only, Set<String> collapsed) {
        List<Row> rows = new ArrayList<Row>();
        List<Property> all = new ArrayList<Property>();
        List<Row> body = new ArrayList<Row>();
        List<Row> sounds = new ArrayList<Row>();
        for (Layer l : p.root().layers) {
            if (l instanceof Layer.FigureLayer) {
                if (only != null && l != only) continue;
                Layer.FigureLayer f = (Layer.FigureLayer) l;
                List<Property> figProps = l.properties();
                all.addAll(figProps);
                body.add(new Row(Kind.FIGURE, l.name, 0, l, figProps));
                if (collapsed != null && collapsed.contains(l.id)) continue;
                List<Property> move = new ArrayList<Property>();
                move.add(l.transform.x);
                move.add(l.transform.y);
                move.add(l.transform.rotation);
                move.add(l.transform.scaleX);
                move.add(l.transform.scaleY);
                body.add(new Row(Kind.MOVE, "Move", 1, l, move));
                List<Node> nodes = f.figure.nodes();
                for (int i = 0; i < nodes.size(); i++) {
                    Node n = nodes.get(i);
                    if (n.parent == null) continue;
                    List<Property> jp = new ArrayList<Property>();
                    jp.add(n.rotation);
                    jp.add(n.length);
                    jp.add(n.thickness);
                    jp.add(n.color);
                    jp.add(n.shown);
                    body.add(new Row(Kind.JOINT, "Joint " + i, 1, l, jp));
                }
            } else if (l instanceof Layer.CameraLayer) {
                Layer.CameraLayer c = (Layer.CameraLayer) l;
                List<Property> cp = new ArrayList<Property>();
                cp.add(c.x);
                cp.add(c.y);
                cp.add(c.zoom);
                cp.add(c.rotation);
                cp.add(c.background);
                cp.add(c.background2);
                all.addAll(cp);
                body.add(new Row(Kind.CAMERA, "Camera", 0, l, cp));
            } else if (l instanceof Layer.AudioLayer) {
                // sounds are listed whatever the figure filter: they are timing context
                for (app.barkfart.model.Clip c : ((Layer.AudioLayer) l).clips) {
                    List<Property> sp = new ArrayList<Property>();
                    sp.add(c.volume);
                    sp.add(c.pan);
                    all.addAll(sp);
                    sounds.add(new Row(Kind.SOUND, l.name, 0, l, sp, c));
                }
            }
        }
        rows.add(new Row(Kind.SUMMARY, "All keys", 0, null, all));
        rows.addAll(sounds);   // timing context, right under the summary
        rows.addAll(body);
        return rows;
    }

    /** Selection helper: keys of the given rows whose time lies in [t0, t1]. */
    public static Set<KeyRef> keysIn(List<Row> rows, double t0, double t1) {
        Set<KeyRef> out = new HashSet<KeyRef>();
        for (Row r : rows) {
            for (Property p : r.props) for (Key k : p.track().keys()) if (k.time >= t0 - 1e-9 && k.time <= t1 + 1e-9) out.add(new KeyRef(p, k.time));
        }
        return out;
    }

    /** The selection after moving it by delta seconds (for keeping it selected after a retime). */
    public static Set<KeyRef> shifted(Set<KeyRef> sel, double delta) {
        Set<KeyRef> out = new HashSet<KeyRef>();
        for (KeyRef k : sel) out.add(new KeyRef(k.property, k.time + delta));
        return Collections.unmodifiableSet(out);
    }
}
