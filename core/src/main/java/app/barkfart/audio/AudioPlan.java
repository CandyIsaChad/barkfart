package app.barkfart.audio;

import app.barkfart.eval.Diagnostic;
import app.barkfart.eval.PropertyResolver;
import app.barkfart.model.Clip;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Everything the audio thread needs, frozen: one entry per audible clip with
 * its PCM and its volume and pan sampled over time. Built on the UI thread
 * (it reads the project); the mixer only reads this. Audio inside nested
 * symbols is not included yet.
 */
public final class AudioPlan {
    /** Volume and pan are sampled this many times per second. */
    public static final int CURVE_RATE = 100;

    public static final class Entry {
        public final Pcm pcm;
        /** Timeline seconds the clip starts and stops (stop = start + length, or the plan's end when looping). */
        public final double start, end;
        /** Source seconds played: [in, out). */
        public final double in, out;
        public final boolean loop;
        public final double fadeIn, fadeOut;
        /** Source seconds played per timeline second. */
        public final double pitch;
        /** Samples at start + k / CURVE_RATE. */
        public final float[] volume, pan;

        Entry(Pcm pcm, double start, double end, double in, double out, boolean loop, double fadeIn, double fadeOut, float[] volume, float[] pan) {
            this(pcm, start, end, in, out, loop, fadeIn, fadeOut, volume, pan, 1);
        }

        Entry(Pcm pcm, double start, double end, double in, double out, boolean loop, double fadeIn, double fadeOut, float[] volume, float[] pan, double pitch) {
            this.pitch = pitch;
            this.pcm = pcm;
            this.start = start;
            this.end = end;
            this.in = in;
            this.out = out;
            this.loop = loop;
            this.fadeIn = fadeIn;
            this.fadeOut = fadeOut;
            this.volume = volume;
            this.pan = pan;
        }
    }

    /** Supplies decoded audio for an asset id (null when missing or undecodable). */
    public interface Sounds {
        Pcm pcm(String assetId);
    }

    public final List<Entry> entries;
    /** Timeline length (looping clips stop here). */
    public final double end;
    public final List<Diagnostic> diagnostics;

    private AudioPlan(List<Entry> entries, double end, List<Diagnostic> diagnostics) {
        this.entries = Collections.unmodifiableList(entries);
        this.end = end;
        this.diagnostics = Collections.unmodifiableList(diagnostics);
    }

    public static AudioPlan empty() {
        return new AudioPlan(new ArrayList<Entry>(), 0, new ArrayList<Diagnostic>());
    }

    /** A plan playing one sound once from time 0 (test tone). */
    public static AudioPlan single(Pcm pcm) {
        List<Entry> e = new ArrayList<Entry>();
        e.add(new Entry(pcm, 0, pcm.seconds(), 0, pcm.seconds(), false, 0, 0, new float[] {1}, new float[] {0}));
        return new AudioPlan(e, pcm.seconds(), new ArrayList<Diagnostic>());
    }

    public static AudioPlan build(Project p, Symbol s, Sounds sounds) {
        List<Diagnostic> diags = new ArrayList<Diagnostic>();
        List<Entry> out = new ArrayList<Entry>();
        double end = s.duration > 0 && s.duration < 1e6 ? s.duration : 0;
        PropertyResolver r = new PropertyResolver(p, diags);
        boolean anySolo = false;
        for (Layer l : s.layers) anySolo |= l.solo;
        for (Layer l : s.layers) {
            if (!(l instanceof Layer.AudioLayer) || !l.visible || (anySolo && !l.solo)) continue;
            for (Clip c : ((Layer.AudioLayer) l).clips) {
                Pcm pcm = c.asset == null ? null : sounds.pcm(c.asset);
                if (pcm == null || pcm.frames == 0) {
                    diags.add(new Diagnostic(Diagnostic.Kind.MISSING_REF, l.id + "/" + c.id + " (sound)"));
                    continue;
                }
                double src = pcm.seconds();
                double in = finite(c.in) ? Math.max(0, Math.min(c.in, src)) : 0;
                double o = finite(c.out) && c.out > 0 ? Math.min(c.out, src) : src;
                if (!(o > in + 1e-6) || !finite(c.start) || Math.abs(c.start) > 1e6) continue;
                double start = c.start;
                double pitch = finite(c.pitch) ? Math.max(0.25, Math.min(4, c.pitch)) : 1;
                double stop = c.loop ? Math.max(start, end) : start + (o - in) / pitch;
                if (!(stop > start)) continue;
                int n = (int) Math.min(CURVE_RATE * 3600L, (long) Math.ceil((stop - start) * CURVE_RATE) + 1);
                float[] vol = new float[Math.max(1, n)], pan = new float[Math.max(1, n)];
                for (int k = 0; k < vol.length; k++) {
                    double t = start + k / (double) CURVE_RATE;
                    float v = r.f(c.volume, t) * r.f(l.opacity, t);   // a layer's opacity is its volume
                    float pn = r.f(c.pan, t);
                    vol[k] = v == v ? Math.max(0, Math.min(4, v)) : 0;
                    pan[k] = pn == pn ? Math.max(-1, Math.min(1, pn)) : 0;
                }
                out.add(new Entry(pcm, start, stop, in, o, c.loop, finite(c.fadeIn) ? Math.max(0, c.fadeIn) : 0,
                        finite(c.fadeOut) ? Math.max(0, c.fadeOut) : 0, vol, pan, pitch));
            }
        }
        return new AudioPlan(out, end, diags);
    }

    private static boolean finite(double v) {
        return v == v && !Double.isInfinite(v);
    }
}
