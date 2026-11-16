package app.barkfart.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * An animatable value: a static value used when the track is empty, a sparse
 * track, and modifiers applied after the keys. The id is a path unique within
 * the project, for example "L3/n7/rotation", and is what modifiers link to.
 */
public final class Property {
    public final String id;
    public final ValueType type;
    private Object value;
    private final Track track;
    private final ArrayList<Modifier> modifiers = new ArrayList<Modifier>();

    public Property(String id, ValueType type, Object value) {
        this.id = id;
        this.type = type;
        this.value = type.checked(value);
        this.track = new Track(type);
    }

    public Object value() {
        return value;
    }

    public void setValue(Object v) {
        value = type.checked(v);
    }

    public Track track() {
        return track;
    }

    public List<Modifier> modifiers() {
        return Collections.unmodifiableList(modifiers);
    }

    /** Adds a modifier. A Loop modifier must come first because it remaps time. */
    public void addModifier(Modifier m) {
        if (m instanceof Modifier.Loop) {
            if (!modifiers.isEmpty() && modifiers.get(0) instanceof Modifier.Loop) {
                throw new IllegalStateException("only one Loop modifier per property");
            }
            modifiers.add(0, m);
        } else {
            modifiers.add(m);
        }
    }

    /** Inserts at a position (for undo). A Loop must stay first. */
    public void insertModifier(int i, Modifier m) {
        if (m instanceof Modifier.Loop) {
            if (i != 0 || (!modifiers.isEmpty() && modifiers.get(0) instanceof Modifier.Loop)) {
                throw new IllegalStateException("Loop modifier must be first and unique");
            }
        } else if (i == 0 && !modifiers.isEmpty() && modifiers.get(0) instanceof Modifier.Loop) {
            throw new IllegalStateException("only a Loop modifier can be first");
        }
        modifiers.add(i, m);
    }

    public Modifier removeModifier(int i) {
        return modifiers.remove(i);
    }

    /** Keys-only value at t: the track if it has keys, else the static value. */
    public Object keyValue(double t) {
        return track.isEmpty() ? value : track.sample(t);
    }

    public float floatValue() {
        return (Float) value;
    }
}
