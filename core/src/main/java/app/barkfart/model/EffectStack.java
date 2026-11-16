package app.barkfart.model;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Effects stored as a DAG so a graph editor can come later. The UI builds only
 * linear chains through append(), and chain() walks one back from the output.
 */
public final class EffectStack {
    public final Map<String, EffectNode> nodes = new LinkedHashMap<String, EffectNode>();
    public String output;

    public boolean isEmpty() {
        return nodes.isEmpty();
    }

    /** Appends an effect after the current output. */
    public void append(EffectNode e) {
        if (nodes.containsKey(e.id)) throw new IllegalArgumentException("duplicate effect id " + e.id);
        e.inputs.clear();
        if (output != null) e.inputs.add(output);
        nodes.put(e.id, e);
        output = e.id;
    }

    /** Effects from first to last along input[0] links. Stops on a cycle or a missing node. */
    public List<EffectNode> chain() {
        ArrayList<EffectNode> rev = new ArrayList<EffectNode>();
        String cur = output;
        int guard = nodes.size();
        while (cur != null && guard-- >= 0) {
            EffectNode e = nodes.get(cur);
            if (e == null || rev.contains(e)) break;
            rev.add(e);
            cur = e.inputs.isEmpty() ? null : e.inputs.get(0);
        }
        ArrayList<EffectNode> out = new ArrayList<EffectNode>(rev.size());
        for (int i = rev.size() - 1; i >= 0; i--) out.add(rev.get(i));
        return out;
    }
}
