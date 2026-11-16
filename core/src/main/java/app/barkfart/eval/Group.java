package app.barkfart.eval;

import app.barkfart.model.Layer;
import java.util.ArrayList;
import java.util.List;

/** A node of the evaluated scene: shapes and nested groups with shared opacity, blend and effects. */
public final class Group {
    public final String source;
    public float opacity = 1;
    public Layer.Blend blend = Layer.Blend.NORMAL;
    /** True for adjustment layers: effects apply to everything drawn before this group in its parent. */
    public boolean adjustment;
    public final List<ResolvedEffect> effects = new ArrayList<ResolvedEffect>();
    /** Shape or Group, in draw order. */
    public final List<Object> children = new ArrayList<Object>();

    public Group(String source) {
        this.source = source;
    }
}
