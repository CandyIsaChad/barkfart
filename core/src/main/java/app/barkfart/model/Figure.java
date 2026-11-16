package app.barkfart.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** A node hierarchy. nodes is draw order; a parent always comes before its children. */
public final class Figure {
    private final ArrayList<Node> nodes = new ArrayList<Node>();
    private final HashMap<String, Integer> index = new HashMap<String, Integer>();

    public List<Node> nodes() {
        return Collections.unmodifiableList(nodes);
    }

    public Node root() {
        return nodes.isEmpty() ? null : nodes.get(0);
    }

    public Node node(String id) {
        Integer i = index.get(id);
        return i == null ? null : nodes.get(i);
    }

    public int indexOf(String id) {
        Integer i = index.get(id);
        return i == null ? -1 : i;
    }

    /** Appends a node. The first node must be the root; others need an existing parent. */
    public void add(Node n) {
        if (index.containsKey(n.id)) throw new IllegalArgumentException("duplicate node id " + n.id);
        if (nodes.isEmpty()) {
            if (n.parent != null) throw new IllegalArgumentException("first node must be the root");
        } else if (n.parent == null || !index.containsKey(n.parent)) {
            throw new IllegalArgumentException("parent " + n.parent + " must be added before " + n.id);
        }
        index.put(n.id, nodes.size());
        nodes.add(n);
    }

    public Map<String, Integer> indexView() {
        return Collections.unmodifiableMap(index);
    }
}
