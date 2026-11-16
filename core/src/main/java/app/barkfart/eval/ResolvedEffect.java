package app.barkfart.eval;

import java.util.LinkedHashMap;
import java.util.Map;

/** An effect with its parameters resolved at the evaluated time. */
public final class ResolvedEffect {
    public final String type;
    public final float mix;
    public final Map<String, Object> params = new LinkedHashMap<String, Object>();

    public ResolvedEffect(String type, float mix) {
        this.type = type;
        this.mix = mix;
    }
}
