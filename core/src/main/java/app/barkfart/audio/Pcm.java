package app.barkfart.audio;

/** Decoded audio: 16-bit samples, interleaved when stereo. */
public final class Pcm {
    /** Longest sound kept in memory (seconds); longer sources are cut (recorded in a diagnostic by the caller). */
    public static final int MAX_SECONDS = 600;

    public final int rate, channels, frames;
    public final short[] data;

    public Pcm(int rate, int channels, short[] data) {
        if (rate <= 0 || (channels != 1 && channels != 2)) throw new IllegalArgumentException("pcm " + rate + " Hz, " + channels + " ch");
        this.rate = rate;
        this.channels = channels;
        this.frames = data.length / channels;
        this.data = data;
    }

    public double seconds() {
        return frames / (double) rate;
    }

    /** Converts a float sample (-1..1) to 16 bits, clamped. */
    public static short s16(float v) {
        if (!(v == v)) return 0;
        float c = v < -1 ? -1 : (v > 1 ? 1 : v);
        return (short) Math.round(c * 32767f);
    }

    /** Builds from float channels (each its own array, same length), downmixing beyond two channels. */
    public static Pcm fromFloat(int rate, float[][] ch, int frames) {
        int n = Math.min(frames, rate * MAX_SECONDS);
        int out = ch.length >= 2 ? 2 : 1;
        short[] d = new short[n * out];
        for (int i = 0; i < n; i++) {
            if (out == 1) d[i] = s16(ch[0][i]);
            else {
                d[i * 2] = s16(ch[0][i]);
                d[i * 2 + 1] = s16(ch[1][i]);
            }
        }
        return new Pcm(rate, out, d);
    }
}
