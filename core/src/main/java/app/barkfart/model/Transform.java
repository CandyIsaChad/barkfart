package app.barkfart.model;

/** Animatable 2D transform of a layer. Rotation in degrees. */
public final class Transform {
    public final Property x, y, rotation, scaleX, scaleY, anchorX, anchorY;

    public Transform(String owner) {
        x = new Property(owner + "/x", ValueType.FLOAT, 0f);
        y = new Property(owner + "/y", ValueType.FLOAT, 0f);
        rotation = new Property(owner + "/rotation", ValueType.ANGLE, 0f);
        scaleX = new Property(owner + "/scaleX", ValueType.FLOAT, 1f);
        scaleY = new Property(owner + "/scaleY", ValueType.FLOAT, 1f);
        anchorX = new Property(owner + "/anchorX", ValueType.FLOAT, 0f);
        anchorY = new Property(owner + "/anchorY", ValueType.FLOAT, 0f);
    }

    public Property[] all() {
        return new Property[] {x, y, rotation, scaleX, scaleY, anchorX, anchorY};
    }
}
