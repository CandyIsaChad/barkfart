package app.barkfart.edit;

import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import app.barkfart.model.Track;
import java.util.ArrayList;
import java.util.List;

/**
 * The frame-strip view (brief decision 8): frames are slots at the project fps
 * over a symbol's keyed tracks. Every operation is one undo step.
 */
public final class FrameOps {
    private final Editor editor;

    public FrameOps(Editor editor) {
        this.editor = editor;
    }

    private Symbol sym() {
        return editor.doc.project.root();
    }

    private double fps() {
        return editor.fps();
    }

    public int frameCount() {
        return Math.max(1, (int) Math.round(sym().duration * fps()));
    }

    public double time(int frame) {
        return frame / fps();
    }

    public int frameAt(double t) {
        return (int) Math.max(0, Math.min(frameCount() - 1, Math.round(t * fps())));
    }

    private List<Property> props() {
        List<Property> out = new ArrayList<Property>();
        for (Layer l : sym().layers) out.addAll(l.properties());
        return out;
    }

    /** True when any property of the symbol has a key on this frame. */
    public boolean hasKeys(int frame) {
        double t = time(frame), eps = 0.25 / fps();
        for (Property p : props()) {
            for (Key k : p.track().keys()) if (Math.abs(k.time - t) < eps) return true;
        }
        return false;
    }

    /** Inserts a frame after `frame`, shifting later keys; the new frame repeats the pose of `frame`. */
    public void insertAfter(int frame, boolean copyPose) {
        double t = time(frame), dt = 1 / fps(), eps = 0.25 / fps();
        editor.doc.begin("Insert frame");
        try {
            editor.doc.apply(StructureEdits.setDuration(sym(), (frameCount() + 1) / fps()));
            for (Property p : props()) {
                if (p.track().isEmpty()) continue;
                Track out = new Track(p.type);
                for (Key k : p.track().keys()) out.put(k.time > t + eps ? k.withTime(k.time + dt) : k);
                if (copyPose) out.put(new Key(t + dt, p.keyValue(t), interpAt(p.track(), t)));
                editor.doc.apply(Edits.setKeys(p, out.keys(), "Insert frame"));
            }
        } finally {
            editor.doc.commit();
        }
    }

    /** Deletes a frame: its keys go, later keys move one frame earlier. The last frame cannot go. */
    public boolean delete(int frame) {
        if (frameCount() <= 1) return false;
        double t = time(frame), dt = 1 / fps(), eps = 0.25 / fps();
        editor.doc.begin("Delete frame");
        try {
            for (Property p : props()) {
                if (p.track().isEmpty()) continue;
                Track out = new Track(p.type);
                for (Key k : p.track().keys()) {
                    if (Math.abs(k.time - t) < eps) continue;
                    out.put(k.time > t ? k.withTime(k.time - dt) : k);
                }
                editor.doc.apply(Edits.setKeys(p, out.keys(), "Delete frame"));
            }
            editor.doc.apply(StructureEdits.setDuration(sym(), (frameCount() - 1) / fps()));
        } finally {
            editor.doc.commit();
        }
        return true;
    }

    /** Sets how every key on this frame moves to the next: HOLD (snap) or LINEAR (tween). */
    public void setTween(int frame, boolean tween) {
        double t = time(frame), eps = 0.25 / fps();
        List<KeyRef> sel = new ArrayList<KeyRef>();
        for (Property p : props()) for (Key k : p.track().keys()) if (Math.abs(k.time - t) < eps) sel.add(new KeyRef(p, k.time));
        if (!sel.isEmpty()) editor.setInterp(sel, tween ? Interp.LINEAR : Interp.HOLD, null);
    }

    /**
     * "Add tweened frame": keys every animated property (all layers, or one)
     * at time t with the value it already has there, so the in-between pose
     * becomes a key you can edit. Nothing moves. Returns the keys added.
     */
    public int keyPose(Layer only, double t) {
        int added = 0;
        editor.doc.begin("Add tweened frame");
        try {
            for (Layer l : sym().layers) {
                if (only != null && l != only) continue;
                for (Property p : l.properties()) {
                    if (p.track().isEmpty() || p.track().indexOf(t) >= 0) continue;
                    Track out = new Track(p.type);
                    for (Key k : p.track().keys()) out.put(k);
                    out.put(new Key(t, p.keyValue(t), interpAt(p.track(), t)));
                    editor.doc.apply(Edits.setKeys(p, out.keys(), "Add tweened frame"));
                    added++;
                }
            }
        } finally {
            editor.doc.commit();
        }
        return added;
    }

    /** The tweened pose a fraction (0..1) of the way from time t0 to t1, as keys (Stick Nodes' 10-90% buttons). */
    public int keyPoseBetween(Layer only, double t0, double t1, double fraction) {
        double f = Math.max(0, Math.min(1, fraction));
        return keyPose(only, t0 + (t1 - t0) * f);
    }

    /**
     * Holds the pose of a frame for `frames` more frames: later keys move back
     * and the pose is keyed at both ends of the hold (Stick Nodes' frame delay).
     */
    public void hold(int frame, int frames) {
        hold(frame, frames, false);
    }

    /**
     * hold with cameraMoves (Stick Nodes 3.1.0 "move camera during delay"):
     * the camera is not held, so it keeps tweening through the delay.
     */
    public void hold(int frame, int frames, boolean cameraMoves) {
        if (frames <= 0) return;
        java.util.Set<Property> cam = new java.util.HashSet<Property>();
        if (cameraMoves) for (Layer l : sym().layers) if (l instanceof Layer.CameraLayer) cam.addAll(l.properties());
        double t = time(frame), dt = frames / fps(), eps = 0.25 / fps();
        editor.doc.begin("Hold frame");
        try {
            editor.doc.apply(StructureEdits.setDuration(sym(), (frameCount() + frames) / fps()));
            for (Property p : props()) {
                if (p.track().isEmpty()) continue;
                Object v = p.keyValue(t);
                Interp in = interpAt(p.track(), t);
                Track out = new Track(p.type);
                for (Key k : p.track().keys()) out.put(k.time > t + eps ? k.withTime(k.time + dt) : k);
                if (!cam.contains(p)) {
                    if (out.indexOf(t) < 0) out.put(new Key(t, v, in));
                    out.put(new Key(t + dt, v, in));
                }
                editor.doc.apply(Edits.setKeys(p, out.keys(), "Hold frame"));
            }
        } finally {
            editor.doc.commit();
        }
    }

/**
     * Adds tweened frames after a frame (Stick Nodes 3.1.0 long-press add):
     * later keys move back, so the tween from this frame to the next keyed
     * one runs over more frames. One undo step.
     */
    public void insertTweened(int frame, int frames) {
        if (frames <= 0) return;
        double t = time(frame), dt = frames / fps(), eps = 0.25 / fps();
        editor.doc.begin("Add " + frames + " frames");
        try {
            editor.doc.apply(StructureEdits.setDuration(sym(), (frameCount() + frames) / fps()));
            for (Property p : props()) {
                if (p.track().isEmpty()) continue;
                Track out = new Track(p.type);
                for (Key k : p.track().keys()) out.put(k.time > t + eps ? k.withTime(k.time + dt) : k);
                editor.doc.apply(Edits.setKeys(p, out.keys(), "Add frames"));
            }
        } finally {
            editor.doc.commit();
        }
    }

    /**
     * Slow motion (Stick Nodes 3.1.0): the tween from this frame to the next
     * keyed frame takes `factor` times as many frames. Returns the frames added.
     */
    public int slowMotion(int frame, int factor) {
        if (factor <= 1) return 0;
        double t = time(frame), eps = 0.25 / fps(), next = Double.POSITIVE_INFINITY;
        for (Property p : props()) for (Key k : p.track().keys()) if (k.time > t + eps) next = Math.min(next, k.time);
        if (Double.isInfinite(next)) return 0;
        int gap = (int) Math.round((next - t) * fps());
        int add = Math.min(gap * (factor - 1), 100000);
        insertTweened(frame, add);
        return add;
    }

        private static Interp interpAt(Track tr, double t) {
        if (tr.isEmpty()) return Interp.LINEAR;
        int i = tr.indexOf(t);
        if (i >= 0) return tr.get(i).interp;
        int ins = -i - 1;
        return ins > 0 ? tr.get(ins - 1).interp : tr.get(0).interp;
    }
}
