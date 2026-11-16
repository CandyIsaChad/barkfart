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

    private static Interp interpAt(Track tr, double t) {
        if (tr.isEmpty()) return Interp.LINEAR;
        int i = tr.indexOf(t);
        if (i >= 0) return tr.get(i).interp;
        int ins = -i - 1;
        return ins > 0 ? tr.get(ins - 1).interp : tr.get(0).interp;
    }
}
