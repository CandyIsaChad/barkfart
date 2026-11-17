package app.barkfart.edit;

import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Polyfill;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Making a polyfill (Stick Nodes creation mode): pick the anchor (it decides
 * the layering: the fill is drawn just before that node), then the nodes
 * around the shape in order, then finish. Undoing the last pick and
 * cancelling are free; finishing is one undo step.
 */
public final class PolyfillBuilder {
    private final Layer.FigureLayer layer;
    private final String anchor;
    private final List<String> nodes = new ArrayList<String>();

    public PolyfillBuilder(Layer.FigureLayer layer, String anchor) {
        if (layer.figure.node(anchor) == null) throw new IllegalArgumentException("no node " + anchor);
        this.layer = layer;
        this.anchor = anchor;
    }

    /** Adds a node; false when it is unknown or already the last one picked. */
    public boolean add(String nodeId) {
        if (layer.figure.node(nodeId) == null) return false;
        if (!nodes.isEmpty() && nodes.get(nodes.size() - 1).equals(nodeId)) return false;
        if (nodes.isEmpty() && nodeId.equals(anchor)) return false;
        nodes.add(nodeId);
        return true;
    }

    public boolean undoLast() {
        if (nodes.isEmpty()) return false;
        nodes.remove(nodes.size() - 1);
        return true;
    }

    public List<String> nodes() {
        return Collections.unmodifiableList(nodes);
    }

    /** A fill needs the anchor and at least two more points. */
    public boolean canFinish() {
        return nodes.size() >= 2;
    }

    /** The finished polyfill as one undo step; colour null = the anchor segment's colour. */
    public Command finish(String id, Color color) {
        if (!canFinish()) throw new IllegalStateException("pick at least two nodes after the anchor");
        Polyfill pf = new Polyfill(layer.id, id);
        pf.anchor = anchor;
        pf.nodes.addAll(nodes);
        if (color != null) {
            pf.useOwnColor = true;
            pf.color.setValue(color);
        }
        return StructureEdits.addPolyfill(layer, pf);
    }

    /** A polyfill id not used in the figure. */
    public static String freeId(Layer.FigureLayer l) {
        for (int i = l.figure.polyfills.size(); ; i++) {
            boolean used = false;
            for (Polyfill p : l.figure.polyfills) used |= p.id.equals("pf" + i);
            if (!used) return "pf" + i;
        }
    }
}
