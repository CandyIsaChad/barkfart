package app.barkfart.eval;

import java.util.ArrayList;
import java.util.List;

/** Result of evaluate(): a render tree in view space plus diagnostics. */
public final class Scene {
    public final Group root;
    public final double time;
    public final List<Diagnostic> diagnostics = new ArrayList<Diagnostic>();
    public int primitives;

    public Scene(Group root, double time) {
        this.root = root;
        this.time = time;
    }

    public boolean has(Diagnostic.Kind k) {
        for (Diagnostic d : diagnostics) if (d.kind == k) return true;
        return false;
    }
}
