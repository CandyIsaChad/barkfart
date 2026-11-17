package app.barkfart.audio;

/**
 * Mixes a plan into stereo float frames: clip timing, trims, loops, fades,
 * volume and pan (constant power for mono sources, balance for stereo),
 * linear-interpolated rate conversion, and a final clamp. Pure; safe to call
 * from an audio thread.
 */
public final class Mixer {
    private Mixer() {}

    /**
     * Fills out[0 .. 2 * frames) (interleaved L, R) with the plan at timeline
     * time t0, at the given output rate. Overwrites out.
     */
    public static void mix(AudioPlan plan, double t0, int frames, int rate, float[] out) {
        java.util.Arrays.fill(out, 0, frames * 2, 0f);
        if (plan == null || rate <= 0 || !(t0 == t0) || Math.abs(t0) > 1e7) return;
        double dt = 1.0 / rate, t1 = t0 + frames * dt;
        for (AudioPlan.Entry e : plan.entries) {
            if (e.end <= t0 || e.start >= t1) continue;
            Pcm p = e.pcm;
            double len = e.out - e.in;
            int i0 = (int) Math.max(0, Math.ceil((e.start - t0) * rate - 1e-9));
            int i1 = (int) Math.min(frames, Math.ceil((e.end - t0) * rate - 1e-9));
            for (int i = i0; i < i1; i++) {
                double t = t0 + i * dt, local = t - e.start, played = local * e.pitch;
                double srcT = e.loop ? e.in + (played % len) : e.in + played;
                if (srcT >= e.out || srcT < e.in) continue;
                double pos = srcT * p.rate;
                int k = (int) pos;
                float f = (float) (pos - k);
                if (k >= p.frames) continue;
                int k2 = Math.min(p.frames - 1, k + 1);
                float gain = gain(e, t, local);
                if (gain <= 0) continue;
                float pan = curve(e.pan, local);
                if (p.channels == 1) {
                    float s = (p.data[k] + (p.data[k2] - p.data[k]) * f) / 32768f * gain;
                    double a = (pan + 1) * Math.PI / 4;   // constant power: -3 dB each side at centre
                    out[i * 2] += s * (float) Math.cos(a) * 1.41421356f;
                    out[i * 2 + 1] += s * (float) Math.sin(a) * 1.41421356f;
                } else {
                    float l = (p.data[k * 2] + (p.data[k2 * 2] - p.data[k * 2]) * f) / 32768f * gain;
                    float r = (p.data[k * 2 + 1] + (p.data[k2 * 2 + 1] - p.data[k * 2 + 1]) * f) / 32768f * gain;
                    out[i * 2] += l * Math.min(1f, 1f - pan);
                    out[i * 2 + 1] += r * Math.min(1f, 1f + pan);
                }
            }
        }
        for (int i = 0; i < frames * 2; i++) {
            float v = out[i];
            out[i] = v > 1 ? 1 : (v < -1 ? -1 : (v == v ? v : 0));
        }
    }

    /** Volume times fade in / fade out at timeline time t (local = t - start). */
    static float gain(AudioPlan.Entry e, double t, double local) {
        float g = curve(e.volume, local);
        if (e.fadeIn > 0 && local < e.fadeIn) g *= (float) (local / e.fadeIn);
        double left = e.end - t;
        if (e.fadeOut > 0 && left < e.fadeOut) g *= (float) Math.max(0, left / e.fadeOut);
        return g;
    }

    /** Linear interpolation of a curve sampled at AudioPlan.CURVE_RATE. */
    static float curve(float[] c, double local) {
        double x = local * AudioPlan.CURVE_RATE;
        if (!(x > 0)) return c[0];
        int k = (int) x;
        if (k >= c.length - 1) return c[c.length - 1];
        float f = (float) (x - k);
        return c[k] + (c[k + 1] - c[k]) * f;
    }

    /** A sine tone (test tone), mono 16-bit. */
    public static Pcm tone(double hz, double seconds, int rate, float level) {
        int n = (int) Math.round(seconds * rate);
        short[] d = new short[n];
        for (int i = 0; i < n; i++) {
            double env = Math.min(1, Math.min(i, n - i) / (0.01 * rate));   // 10 ms ramps, no clicks
            d[i] = Pcm.s16((float) (level * env * Math.sin(2 * Math.PI * hz * i / rate)));
        }
        return new Pcm(rate, 1, d);
    }
}
