package app.barkfart.model;

/** An image, audio or font file stored inside the project container. */
public final class Asset {
    public enum Kind { IMAGE, AUDIO, FONT }

    public final String id;
    public final Kind kind;
    /** Path inside the container, e.g. "assets/a3.png". */
    public final String path;

    public Asset(String id, Kind kind, String path) {
        this.id = id;
        this.kind = kind;
        this.path = path;
    }
}
