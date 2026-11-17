package app.barkfart.model;

/** An audio clip placed in time on an audio layer. Times in seconds. */
public final class Clip {
    public final String id;
    public String asset;
    public double start, in, out;
    public boolean loop;
    public double fadeIn, fadeOut;
    /** Playback rate: 2 = an octave up and twice as fast, 0.5 = an octave down and half speed (Stick Nodes 3.1.0). */
    public double pitch = 1;
    public final Property volume, pan;

    public Clip(String layerId, String id, String asset, double start, double in, double out) {
        this.id = id;
        this.asset = asset;
        this.start = start;
        this.in = in;
        this.out = out;
        volume = new Property(layerId + "/" + id + "/volume", ValueType.FLOAT, 1f);
        pan = new Property(layerId + "/" + id + "/pan", ValueType.FLOAT, 0f);
    }
}
