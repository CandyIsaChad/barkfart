package app.barkfart.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/** Sparse, time-sorted keys with unique times. */
public final class Track {
    private final ValueType type;
    private final ArrayList<Key> keys = new ArrayList<Key>();

    public Track(ValueType type) {
        this.type = type;
    }

    public ValueType type() {
        return type;
    }

    public int size() {
        return keys.size();
    }

    public boolean isEmpty() {
        return keys.isEmpty();
    }

    public Key get(int i) {
        return keys.get(i);
    }

    public List<Key> keys() {
        return Collections.unmodifiableList(keys);
    }

    /** Inserts or replaces the key at the same time. Returns the replaced key or null. */
    public Key put(Key k) {
        type.checked(k.value);
        Key stored = type.continuous() ? k : new Key(k.time, k.value, Interp.HOLD, k.ease);
        int i = indexOf(k.time);
        if (i >= 0) return keys.set(i, stored);
        keys.add(-i - 1, stored);
        return null;
    }

    public Key removeAt(int i) {
        return keys.remove(i);
    }

    public void clear() {
        keys.clear();
    }

    /** Binary search by exact time: index if found, else -(insertion point) - 1. */
    public int indexOf(double time) {
        int lo = 0, hi = keys.size() - 1;
        while (lo <= hi) {
            int mid = (lo + hi) >>> 1;
            double t = keys.get(mid).time;
            if (t < time) lo = mid + 1;
            else if (t > time) hi = mid - 1;
            else return mid;
        }
        return -(lo + 1);
    }

    /** Value at time t. Before the first key and after the last, the end values hold. */
    public Object sample(double t) {
        int n = keys.size();
        if (n == 0) throw new IllegalStateException("empty track");
        Key first = keys.get(0);
        if (t <= first.time || n == 1) return first.value;
        Key last = keys.get(n - 1);
        if (t >= last.time) return last.value;
        int i = indexOf(t);
        if (i >= 0) return keys.get(i).value;
        int hi = -i - 1;
        Key a = keys.get(hi - 1), b = keys.get(hi);
        switch (a.interp) {
            case HOLD:
                return a.value;
            case LINEAR:
                return type.lerp(a.value, b.value, (float) ((t - a.time) / (b.time - a.time)));
            default:
                float u = (float) ((t - a.time) / (b.time - a.time));
                return type.lerp(a.value, b.value, a.ease.apply(u));
        }
    }

    public Track copy() {
        Track t = new Track(type);
        t.keys.addAll(keys);
        return t;
    }
}
