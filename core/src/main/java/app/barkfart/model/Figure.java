package app.barkfart.model;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * A node hierarchy. nodes is hierarchy order (a parent always comes before its
 * children); each node's drawOrder sets layering. add() gives new nodes the
 * next draw order (in front) unless one was set.
 */
public final class Figure {
    private final ArrayList<Node> nodes = new ArrayList<Node>();
    /** Filled shapes through node joints. */
    public final List<Polyfill> polyfills = new ArrayList<Polyfill>();
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
        if (n.drawOrder == Node.UNASSIGNED) n.drawOrder = nodes.isEmpty() ? 0 : maxDrawOrder() + 1;
        index.put(n.id, nodes.size());
        nodes.add(n);
    }

    private int maxDrawOrder() {
        int m = Integer.MIN_VALUE;
        for (Node x : nodes) m = Math.max(m, x.drawOrder);
        return m;
    }

    /** Node indices sorted by drawOrder (stable, back to front). */
    public int[] drawSequence() {
        int n = nodes.size();
        Integer[] idx = new Integer[n];
        for (int i = 0; i < n; i++) idx[i] = i;
        java.util.Arrays.sort(idx, new java.util.Comparator<Integer>() {
            public int compare(Integer a, Integer b) {
                int da = nodes.get(a).drawOrder, db = nodes.get(b).drawOrder;
                return da < db ? -1 : (da > db ? 1 : a.compareTo(b));
            }
        });
        int[] out = new int[n];
        for (int i = 0; i < n; i++) out[i] = idx[i];
        return out;
    }

    /** The node and all its descendants, in hierarchy order. */
    public List<Node> subtree(String id) {
        List<Node> out = new ArrayList<Node>();
        Node n = node(id);
        if (n == null) return out;
        java.util.HashSet<String> ids = new java.util.HashSet<String>();
        ids.add(id);
        for (Node x : nodes) {
            if (x == n || (x.parent != null && ids.contains(x.parent))) {
                ids.add(x.id);
                out.add(x);
            }
        }
        return out;
    }

    /**
     * Removes a node and its descendants (not the root). Returns their former
     * list positions, ascending, parallel to the removed nodes, for exact undo.
     */
    public int[] removeSubtree(String id) {
        Node n = node(id);
        if (n == null || n.parent == null) throw new IllegalArgumentException("cannot remove " + id);
        List<Node> sub = subtree(id);
        int[] pos = new int[sub.size()];
        for (int i = 0; i < sub.size(); i++) pos[i] = indexOf(sub.get(i).id);
        nodes.removeAll(sub);
        reindex();
        return pos;
    }

    /** Reinserts nodes at the given ascending positions (inverse of removeSubtree). */
    public void restore(List<Node> sub, int[] pos) {
        for (int i = 0; i < sub.size(); i++) nodes.add(Math.min(pos[i], nodes.size()), sub.get(i));
        reindex();
    }

    private void reindex() {
        index.clear();
        for (int i = 0; i < nodes.size(); i++) index.put(nodes.get(i).id, i);
    }

    /** A copy of the node list in hierarchy order. */
    public List<Node> nodeList() {
        return new ArrayList<Node>(nodes);
    }

    /** Replaces the node list (undo snapshots). Parents must precede children. */
    public void setNodes(List<Node> list) {
        java.util.HashSet<String> seen = new java.util.HashSet<String>();
        for (int i = 0; i < list.size(); i++) {
            Node n = list.get(i);
            if (i == 0 ? n.parent != null : (n.parent == null || !seen.contains(n.parent))) {
                throw new IllegalArgumentException("node order: " + n.id);
            }
            if (!seen.add(n.id)) throw new IllegalArgumentException("duplicate node id " + n.id);
        }
        nodes.clear();
        nodes.addAll(list);
        reindex();
    }

    /** Inserts a node at a list position; its parent must come earlier. */
    public void insert(int pos, Node n) {
        List<Node> l = nodeList();
        if (n.drawOrder == Node.UNASSIGNED) n.drawOrder = maxDrawOrder() + 1;
        l.add(Math.max(1, Math.min(pos, l.size())), n);
        setNodes(l);
    }

    /** Gives a node a new parent that already precedes it in the list. */
    public void reparent(String id, String parent) {
        Node n = node(id), p = node(parent);
        if (n == null || p == null || indexOf(parent) >= indexOf(id)) throw new IllegalArgumentException("reparent " + id + " -> " + parent);
        for (Node x : subtree(id)) if (x == p) throw new IllegalArgumentException("cycle");
        n.parent = parent;
    }

    /** An id not used by any node, from a prefix. */
    public String freeId(String prefix) {
        for (int i = nodes.size(); ; i++) if (!index.containsKey(prefix + i)) return prefix + i;
    }

    public Map<String, Integer> indexView() {
        return Collections.unmodifiableMap(index);
    }
}
