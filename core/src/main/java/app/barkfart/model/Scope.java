package app.barkfart.model;

/** Where an effect stack (or effect mask) applies. */
public final class Scope {
    public enum Kind { NODE, NODE_SET, FIGURE, LAYER, SYMBOL_INSTANCE, COMPOSITION }

    public final Kind kind;
    /** Layer id, node-set id or symbol id depending on kind. */
    public final String target;
    /** Node id for NODE scope, else null. */
    public final String node;

    public Scope(Kind kind, String target, String node) {
        this.kind = kind;
        this.target = target;
        this.node = node;
    }
}
