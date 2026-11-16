package app.barkfart.edit;

import app.barkfart.model.Property;

/** A selected key: a property and the exact time of one of its keys. */
public final class KeyRef {
    public final Property property;
    public final double time;

    public KeyRef(Property property, double time) {
        this.property = property;
        this.time = time;
    }

    @Override
    public boolean equals(Object o) {
        if (!(o instanceof KeyRef)) return false;
        KeyRef k = (KeyRef) o;
        return k.property == property && Double.compare(k.time, time) == 0;
    }

    @Override
    public int hashCode() {
        return System.identityHashCode(property) * 31 + Double.valueOf(time).hashCode();
    }
}
