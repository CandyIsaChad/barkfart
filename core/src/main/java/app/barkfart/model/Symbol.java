package app.barkfart.model;

import java.util.ArrayList;
import java.util.List;

/** A composition with its own timeline and layer stack. The project root is a Symbol. */
public final class Symbol {
    public static final class ScopedStack {
        public final Scope scope;
        public final EffectStack stack;

        public ScopedStack(Scope scope, EffectStack stack) {
            this.scope = scope;
            this.stack = stack;
        }
    }

    public final String id;
    public String name;
    public double duration;
    /** index 0 = bottom. */
    public final List<Layer> layers = new ArrayList<Layer>();
    public final List<Marker> markers = new ArrayList<Marker>();
    public final List<ScopedStack> scopedEffects = new ArrayList<ScopedStack>();

    public Symbol(String id, String name, double duration) {
        this.id = id;
        this.name = name;
        this.duration = duration;
    }

    public Layer layer(String layerId) {
        for (Layer l : layers) if (l.id.equals(layerId)) return l;
        return null;
    }
}
