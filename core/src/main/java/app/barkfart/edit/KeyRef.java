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
}
