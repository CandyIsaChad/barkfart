package app.barkfart.edit;

import app.barkfart.model.Key;
import app.barkfart.model.Property;
import app.barkfart.model.Track;
import java.util.ArrayList;
import java.util.List;

/** Basic commands. Track edits store full before/after key lists, so undo is exact. */
public final class Edits {
    private Edits() {}

    /** Sets a property's static value (used when its track is empty). */
    public static Command setValue(final Property p, final Object value) {
        final Object before = p.value();
        p.type.checked(value);
        return new Command() {
            public void apply() {
                p.setValue(value);
            }

            public void revert() {
                p.setValue(before);
            }

            public String label() {
                return "Change " + p.id;
            }
        };
    }

    static List<Key> snapshot(Track t) {
        return new ArrayList<Key>(t.keys());
    }

    static void restore(Track t, List<Key> keys) {
        t.clear();
        for (Key k : keys) t.put(k);
    }

    /** Replaces a property's keys wholesale (the building block of all key edits). */
    public static Command setKeys(final Property p, final List<Key> after, String label) {
        final List<Key> before = snapshot(p.track());
        final List<Key> to = new ArrayList<Key>(after);
        for (Key k : to) p.type.checked(k.value);
        final String l = label;
        return new Command() {
            public void apply() {
                restore(p.track(), to);
            }

            public void revert() {
                restore(p.track(), before);
            }

            public String label() {
                return l;
            }
        };
    }

    /** Inserts or replaces one key. */
    public static Command putKey(Property p, Key k) {
        Track t = p.track().copy();
        t.put(k);
        return setKeys(p, t.keys(), "Key " + p.id);
    }

    /** Removes the key at exactly time t (no-op command if none). */
    public static Command removeKey(Property p, double time) {
        Track t = p.track().copy();
        int i = t.indexOf(time);
        if (i >= 0) t.removeAt(i);
        return setKeys(p, t.keys(), "Delete key " + p.id);
    }
}
