package app.barkfart.model;

/** One keyframe. interp and ease govern the segment to the next key. */
public final class Key {
    public final double time;
    public final Object value;
    public final Interp interp;
    public final Ease ease;

    public Key(double time, Object value, Interp interp, Ease ease) {
        if (Double.isNaN(time) || Double.isInfinite(time)) throw new IllegalArgumentException("bad key time " + time);
        this.time = time;
        this.value = value;
        this.interp = interp;
        this.ease = ease == null ? Ease.LINEAR : ease;
    }

    public Key(double time, Object value, Interp interp) {
        this(time, value, interp, Ease.LINEAR);
    }

    public Key withTime(double t) {
        return new Key(t, value, interp, ease);
    }

    public Key withValue(Object v) {
        return new Key(time, v, interp, ease);
    }
}
