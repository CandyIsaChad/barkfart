package app.barkfart.model;

import java.util.ArrayList;
import java.util.List;

/**
 * A filled shape through node joints, drawn just before its anchor node.
 * Colour is its own when useOwnColor, else the anchor node's colour.
 */
public final class Polyfill {
    public final String id;
    public String anchor;
    /** Node ids after the anchor, in outline order. */
    public final List<String> nodes = new ArrayList<String>();
    public boolean useOwnColor;
    public final Property color;

    public Polyfill(String layerId, String id) {
        this.id = id;
        color = new Property(layerId + "/" + id + "/color", ValueType.COLOR, Color.BLACK);
    }
}
