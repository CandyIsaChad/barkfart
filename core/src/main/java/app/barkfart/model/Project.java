package app.barkfart.model;

import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/** Top of the model. */
public final class Project {
    public static final int FORMAT_VERSION = 1;

    public static final class Settings {
        public float fps = 30;
        public int width = 1280, height = 720;
        public Color background = Color.WHITE;
    }

    public final Settings settings = new Settings();
    public final Map<String, Symbol> library = new LinkedHashMap<String, Symbol>();
    public String rootSymbol;
    public final Map<String, Asset> assets = new LinkedHashMap<String, Asset>();
    public final Map<String, NodeSet> nodeSets = new LinkedHashMap<String, NodeSet>();
    private int nextId = 1;
    private Map<String, Property> propertyIndex;

    /** A new id unique within this project: prefix + base-36 counter. */
    public String newId(String prefix) {
        String id;
        do {
            id = prefix + Integer.toString(nextId++, 36);
        } while (taken(id));
        return id;
    }

    private boolean taken(String id) {
        if (library.containsKey(id) || assets.containsKey(id) || nodeSets.containsKey(id)) return true;
        for (Symbol s : library.values()) if (s.layer(id) != null) return true;
        return false;
    }

    /** Counter for newId, saved with the project so ids never repeat. */
    public int idCounter() {
        return nextId;
    }

    public void setIdCounter(int v) {
        nextId = Math.max(1, v);
    }

    public Symbol root() {
        return library.get(rootSymbol);
    }

    public Symbol addSymbol(Symbol s) {
        library.put(s.id, s);
        invalidateIndex();
        return s;
    }

    /** Looks up a property by id. The index rebuilds after invalidateIndex(). */
    public Property property(String id) {
        if (propertyIndex == null) {
            HashMap<String, Property> m = new HashMap<String, Property>();
            for (Symbol s : library.values()) {
                for (Layer l : s.layers) for (Property p : l.properties()) m.put(p.id, p);
                for (Symbol.ScopedStack ss : s.scopedEffects) {
                    for (EffectNode e : ss.stack.nodes.values()) {
                        m.put(e.mix.id, e.mix);
                        for (Property p : e.params.values()) m.put(p.id, p);
                    }
                }
            }
            propertyIndex = m;
        }
        return propertyIndex.get(id);
    }

    /** Call after adding or removing layers, nodes, clips or effects. */
    public void invalidateIndex() {
        propertyIndex = null;
    }
}
