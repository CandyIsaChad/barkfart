package app.barkfart.model;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** One effect in an effect graph. */
public final class EffectNode {
    public final String id;
    /** Effect type id, e.g. "blur.gaussian". */
    public final String type;
    public boolean enabled = true;
    public final Property mix;
    public final Map<String, Property> params = new LinkedHashMap<String, Property>();
    /** Input node ids; empty means the stack's source image. */
    public final List<String> inputs = new ArrayList<String>();
    public Scope mask;

    public EffectNode(String ownerPath, String id, String type) {
        this.id = id;
        this.type = type;
        this.mix = new Property(ownerPath + "/" + id + "/mix", ValueType.FLOAT, 1f);
    }

    public Property param(String ownerPath, String name, ValueType t, Object value) {
        Property p = new Property(ownerPath + "/" + id + "/" + name, t, value);
        params.put(name, p);
        return p;
    }
}
