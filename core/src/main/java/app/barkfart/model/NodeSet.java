package app.barkfart.model;

import java.util.ArrayList;
import java.util.List;

/** A named, reusable selection of nodes, possibly across figures. */
public final class NodeSet {
    public final String id;
    public String name;
    /** Pairs of {layer id, node id}. */
    public final List<String[]> members = new ArrayList<String[]>();

    public NodeSet(String id, String name) {
        this.id = id;
        this.name = name;
    }
}
