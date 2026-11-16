package app.barkfart.edit;

import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Property;
import app.barkfart.model.Track;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Editing operations that the timeline and canvas call. Every operation is
 * one undo step on the Document. DESIGN.md §8 and the M4 brief.
 */
public final class Editor {
    public enum Result { CHANGED, REFUSED_NEEDS_AUTOKEY, NOTHING }

    public final Document doc;
    public boolean autoKey = true;
    /** Interpolation for new keys when nothing better is known. */
    public Interp defaultInterp = Interp.LINEAR;

    public Editor(Document doc) {
        this.doc = doc;
    }

    public double fps() {
        return doc.project.settings.fps;
    }

    /** Rounds a time to the nearest frame at the project fps. */
    public double snap(double t) {
        double f = fps();
        return Math.round(t * f) / f;
    }

    /**
     * Changes one property at the playhead. Auto-key on: keys only this
     * property. Off: edits the static value if the track is empty, else refuses.
     */
    public Result change(Property p, Object value, double t) {
        if (autoKey) {
            doc.apply(Edits.putKey(p, new Key(t, value, interpAt(p, t))));
            return Result.CHANGED;
        }
        if (p.track().isEmpty()) {
            doc.apply(Edits.setValue(p, value));
            return Result.CHANGED;
        }
        return Result.REFUSED_NEEDS_AUTOKEY;
    }

    /** New keys inherit the interpolation of the key they replace or follow. */
    private Interp interpAt(Property p, double t) {
        Track tr = p.track();
        if (tr.isEmpty()) return defaultInterp;
        int i = tr.indexOf(t);
        if (i >= 0) return tr.get(i).interp;
        int ins = -i - 1;
        return ins > 0 ? tr.get(ins - 1).interp : tr.get(0).interp;
    }

    /**
     * Frame-strip posing: keys every node of the figure (rotation and length)
     * at the frame time, using the given overrides and the current evaluated
     * pose for the rest. One undo step.
     */
    public void pose(Layer.FigureLayer layer, Map<String, Float> rotations, Map<String, Float> lengths, double frameTime) {
        double t = snap(frameTime);
        doc.begin("Pose " + layer.name);
        try {
            for (Node n : layer.figure.nodes()) {
                keyPose(n.rotation, rotations == null ? null : rotations.get(n.id), t);
                keyPose(n.length, lengths == null ? null : lengths.get(n.id), t);
            }
        } finally {
            doc.commit();
        }
    }

    /** Keys the override, or the current keys-only value (modifiers are never baked into keys). */
    private void keyPose(Property p, Float override, double t) {
        Object v = override != null ? override : p.keyValue(t);
        doc.apply(Edits.putKey(p, new Key(t, v, interpAt(p, t))));
    }

    /** Moves selected keys by delta seconds. Keys landing on other keys replace them. */
    public void retime(List<KeyRef> sel, double delta) {
        transformTimes(sel, 1, 0, delta, "Move keys");
    }

    /** Scales selected key times around a pivot (factor > 0). */
    public void scaleTime(List<KeyRef> sel, double pivot, double factor) {
        if (!(factor > 0)) throw new IllegalArgumentException("factor must be > 0");
        transformTimes(sel, factor, pivot, 0, "Scale keys");
    }

    /**
     * Offset tool: along a node chain (in order), shifts each node's selected
     * keys by index x frames. Properties are grouped per node in the given order.
     */
    public void stagger(List<List<KeyRef>> chain, double frames) {
        doc.begin("Offset keys");
        try {
            for (int i = 0; i < chain.size(); i++) {
                if (i == 0 || chain.get(i).isEmpty()) continue;
                retimeNoGroup(chain.get(i), i * frames / fps());
            }
        } finally {
            doc.commit();
        }
    }

    private void retimeNoGroup(List<KeyRef> sel, double delta) {
        transformTimes(sel, 1, 0, delta, "Move keys");
    }

    private void transformTimes(List<KeyRef> sel, double factor, double pivot, double delta, String label) {
        Map<Property, List<Double>> byProp = group(sel);
        doc.begin(label);
        try {
            for (Map.Entry<Property, List<Double>> e : byProp.entrySet()) {
                Property p = e.getKey();
                Track t = p.track();
                List<Key> moving = new ArrayList<Key>(), staying = new ArrayList<Key>();
                for (Key k : t.keys()) (e.getValue().contains(k.time) ? moving : staying).add(k);
                Track out = new Track(p.type);
                for (Key k : staying) out.put(k);
                for (Key k : moving) out.put(k.withTime(pivot + (k.time - pivot) * factor + delta));
                doc.apply(Edits.setKeys(p, out.keys(), label));
            }
        } finally {
            doc.commit();
        }
    }

    private static Map<Property, List<Double>> group(List<KeyRef> sel) {
        Map<Property, List<Double>> m = new LinkedHashMap<Property, List<Double>>();
        for (KeyRef k : sel) {
            List<Double> l = m.get(k.property);
            if (l == null) m.put(k.property, l = new ArrayList<Double>());
            if (!l.contains(k.time)) l.add(k.time);
        }
        return m;
    }

    /** Deletes the selected keys. */
    public void delete(List<KeyRef> sel) {
        doc.begin("Delete keys");
        try {
            for (Map.Entry<Property, List<Double>> e : group(sel).entrySet()) {
                Track t = e.getKey().track().copy();
                for (double time : e.getValue()) {
                    int i = t.indexOf(time);
                    if (i >= 0) t.removeAt(i);
                }
                doc.apply(Edits.setKeys(e.getKey(), t.keys(), "Delete keys"));
            }
        } finally {
            doc.commit();
        }
    }

    /** Copied keys: property id -> keys with times relative to the earliest copied key. */
    public static final class Clip {
        public final Map<String, List<Key>> keys = new LinkedHashMap<String, List<Key>>();
    }

    public Clip copy(List<KeyRef> sel) {
        double t0 = Double.POSITIVE_INFINITY;
        for (KeyRef k : sel) t0 = Math.min(t0, k.time);
        Clip c = new Clip();
        for (Map.Entry<Property, List<Double>> e : group(sel).entrySet()) {
            List<Key> out = new ArrayList<Key>();
            for (Key k : e.getKey().track().keys()) if (e.getValue().contains(k.time)) out.add(k.withTime(k.time - t0));
            c.keys.put(e.getKey().id, out);
        }
        return c;
    }

    /**
     * Pastes at time t. Each copied property id is mapped through idMap (for
     * pasting onto another figure); unmapped ids paste onto the same property.
     * Missing or type-mismatched targets are skipped. Returns keys pasted.
     */
    public int paste(Clip c, double t, Map<String, String> idMap) {
        int n = 0;
        doc.begin("Paste keys");
        try {
            for (Map.Entry<String, List<Key>> e : c.keys.entrySet()) {
                String target = idMap != null && idMap.containsKey(e.getKey()) ? idMap.get(e.getKey()) : e.getKey();
                Property p = doc.project.property(target);
                if (p == null || e.getValue().isEmpty()) continue;
                if (!sameType(p, e.getValue().get(0).value)) continue;
                Track out = p.track().copy();
                for (Key k : e.getValue()) {
                    out.put(k.withTime(t + k.time));
                    n++;
                }
                doc.apply(Edits.setKeys(p, out.keys(), "Paste keys"));
            }
        } finally {
            doc.commit();
        }
        return n;
    }

    private static boolean sameType(Property p, Object v) {
        try {
            p.type.checked(v);
            return true;
        } catch (IllegalArgumentException ex) {
            return false;
        }
    }

    /** Sets interpolation/easing on the selected keys (easing presets, bezier editor). */
    public void setInterp(List<KeyRef> sel, Interp interp, app.barkfart.model.Ease ease) {
        doc.begin("Easing");
        try {
            for (Map.Entry<Property, List<Double>> e : group(sel).entrySet()) {
                Track out = new Track(e.getKey().type);
                for (Key k : e.getKey().track().keys()) {
                    out.put(e.getValue().contains(k.time) ? new Key(k.time, k.value, interp, ease) : k);
                }
                doc.apply(Edits.setKeys(e.getKey(), out.keys(), "Easing"));
            }
        } finally {
            doc.commit();
        }
    }
}
