package app.barkfart.audio;

/** Mixes a whole timeline offline into a stereo 44.1 kHz WAV (the soundtrack of an export). */
public final class Bounce {
    private Bounce() {}

    /** The plan mixed from 0 to seconds; null when the plan is silent (no entries). */
    public static byte[] wav(AudioPlan plan, double seconds) {
        return wav(plan, 0, seconds);
    }

    /** The plan mixed from timeline time `from`, `seconds` long (an export range). */
    public static byte[] wav(AudioPlan plan, double from, double seconds) {
        if (plan == null || plan.entries.isEmpty() || !(seconds > 0)) return null;
        int rate = 44100, frames = (int) Math.min((long) rate * Pcm.MAX_SECONDS, Math.round(seconds * rate));
        short[] d = new short[frames * 2];
        float[] block = new float[4096 * 2];
        for (int at = 0; at < frames; at += 4096) {
            int n = Math.min(4096, frames - at);
            Mixer.mix(plan, from + at / (double) rate, n, rate, block);
            for (int i = 0; i < n * 2; i++) d[at * 2 + i] = Pcm.s16(block[i]);
        }
        return Wav.encode(new Pcm(rate, 2, d));
    }
}
