package app.barkfart.edit;

import app.barkfart.eval.Affine;
import app.barkfart.eval.Evaluator;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.KeyReducer;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.List;

/** Camera helpers: follow a layer (auto-camera), lock a layer to the camera. One undo step each. */
public final class CameraOps {
    private CameraOps() {}

    /**
     * Keys the camera's x / y on every frame from f0 to f1 so it follows the
     * layer's origin, keeping the offset it has at f0, then thins the keys
     * (KeyReducer) so they stay editable. Existing x / y keys in the range are replaced.
     */
    public static Command follow(final Project p, final Symbol s, final Layer.CameraLayer cam, final Layer target, final int f0, final int f1) {
        final double fps = p.settings.fps > 0 ? p.settings.fps : 30;
        final Property[] xy = {cam.x, cam.y};
        return new Command() {
            final List<List<Key>> before = new ArrayList<List<Key>>(), after = new ArrayList<List<Key>>();
            final Object[] beforeV = new Object[2];

            public void apply() {
                if (!after.isEmpty()) {
                    set(after);
                    return;
                }
                for (int i = 0; i < 2; i++) {
                    before.add(new ArrayList<Key>(xy[i].track().keys()));
                    beforeV[i] = xy[i].value();
                }
                int a = Math.min(f0, f1), b = Math.max(f0, f1);
                double ta = a / fps, tb = b / fps;
                double[] start = origin(p, s, target, ta);
                float ox = (Float) cam.x.keyValue(ta) - (float) start[0], oy = (Float) cam.y.keyValue(ta) - (float) start[1];
                for (int i = 0; i < 2; i++) {
                    List<Key> keep = new ArrayList<Key>();
                    for (Key k : xy[i].track().keys()) if (k.time < ta - 1e-9 || k.time > tb + 1e-9) keep.add(k);
                    xy[i].track().clear();
                    for (Key k : keep) xy[i].track().put(k);
                }
                for (int f = a; f <= b; f++) {
                    double t = f / fps;
                    double[] o = origin(p, s, target, t);
                    cam.x.track().put(new Key(t, (float) o[0] + ox, Interp.LINEAR));
                    cam.y.track().put(new Key(t, (float) o[1] + oy, Interp.LINEAR));
                }
                for (Property q : xy) KeyReducer.reduce(q.track(), KeyReducer.defaultEpsilon(q.type));
                for (Property q : xy) after.add(new ArrayList<Key>(q.track().keys()));
            }

            private void set(List<List<Key>> k) {
                for (int i = 0; i < 2; i++) {
                    xy[i].track().clear();
                    for (Key key : k.get(i)) xy[i].track().put(key);
                }
            }

            public void revert() {
                set(before);
                for (int i = 0; i < 2; i++) xy[i].setValue(beforeV[i]);
            }

            public String label() {
                return "Camera follows " + target.name;
            }
        };
    }

    private static double[] origin(Project p, Symbol s, Layer l, double t) {
        Affine a = Evaluator.layerStage(p, s, l, t);
        return new double[] {a.mapX(0, 0), a.mapY(0, 0)};
    }

    /** Locks a layer to the camera (it stays put in the shot, like a caption), or unlocks it; no jump. */
    public static Command lock(Project p, Symbol s, Layer l, Layer.CameraLayer cam, boolean on, double t) {
        return JoinOps.reparent(p, s, l, on ? cam.id : null, t, on ? "Lock to camera" : "Unlock from camera");
    }

    /**
     * Lock text to a corner of the shot (Stick Nodes 2.2.0, a name or
     * watermark): locks it to the camera, aligns the text block to that
     * corner and places it there, margin in from the edges, unturned. One undo step.
     */
    public static void pinToCorner(Editor editor, Symbol s, final Layer.TextLayer l, Layer.CameraLayer cam, final int corner, float margin, double t) {
        if (corner < 1 || corner > 4) throw new IllegalArgumentException("corner 1..4");
        Project p = editor.doc.project;
        editor.doc.begin("Lock text to corner");
        try {
            if (!cam.id.equals(l.parent)) editor.doc.apply(lock(p, s, l, cam, true, t));
            final int old = l.corner;
            final float oldPar = l.parallax;
            editor.doc.apply(new Command() {
                public void apply() {
                    l.corner = corner;
                    l.parallax = 0;
                }

                public void revert() {
                    l.corner = old;
                    l.parallax = oldPar;
                }

                public String label() {
                    return "Align text";
                }
            });
            float w = p.settings.width / 2f - margin, h = p.settings.height / 2f - margin;
            boolean leftSide = corner == Layer.TextLayer.TOP_LEFT || corner == Layer.TextLayer.BOTTOM_LEFT;
            boolean topSide = corner == Layer.TextLayer.TOP_LEFT || corner == Layer.TextLayer.TOP_RIGHT;
            editor.change(l.transform.x, leftSide ? -w : w, t);
            editor.change(l.transform.y, topSide ? h : -h, t);
            editor.change(l.transform.rotation, 0f, t);
        } finally {
            editor.doc.commit();
        }
    }

}
