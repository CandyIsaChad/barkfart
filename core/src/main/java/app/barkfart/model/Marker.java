package app.barkfart.model;

/** A named point in a symbol's time. */
public final class Marker {
    public final double time;
    public final String name;
    public final Color color;

    public Marker(double time, String name, Color color) {
        this.time = time;
        this.name = name;
        this.color = color;
    }
}
