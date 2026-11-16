package app.barkfart.model;

import java.util.LinkedHashMap;
import java.util.Map;

/** One node of a figure: a joint plus the shape drawn from its parent to it. */
public final class Node {
    public enum Shape { NONE, LINE, CIRCLE }

    public final String id;
    /** Parent node id, or null for the root. Parents precede children in Figure.nodes. */
    public final String parent;
    public Shape shape;
    public final Property rotation, length, thickness, scale, color, shown;
    /** Legacy fields not yet understood, kept for faithful re-export. */
    public final Map<String, Object> legacyExtras = new LinkedHashMap<String, Object>();

    public Node(String layerId, String id, String parent, Shape shape) {
        this.id = id;
        this.parent = parent;
        this.shape = shape;
        String p = layerId + "/" + id + "/";
        rotation = new Property(p + "rotation", ValueType.ANGLE, 0f);
        length = new Property(p + "length", ValueType.FLOAT, 0f);
        thickness = new Property(p + "thickness", ValueType.FLOAT, 10f);
        scale = new Property(p + "scale", ValueType.FLOAT, 1f);
        color = new Property(p + "color", ValueType.COLOR, Color.BLACK);
        shown = new Property(p + "shown", ValueType.BOOL, Boolean.TRUE);
    }

    public Property[] properties() {
        return new Property[] {rotation, length, thickness, scale, color, shown};
    }
}
