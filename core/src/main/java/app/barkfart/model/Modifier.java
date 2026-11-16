package app.barkfart.model;

/** Procedural modifiers stacked on a property after its keys (DESIGN.md §3). */
public abstract class Modifier {
    public boolean enabled = true;

    private Modifier() {}

    /** Smooth noise around the value: v + amplitude * noise(seed, t * frequency). */
    public static final class Wiggle extends Modifier {
        public float frequency, amplitude;
        public int seed, octaves;

        public Wiggle(float frequency, float amplitude, int seed, int octaves) {
            this.frequency = frequency;
            this.amplitude = amplitude;
            this.seed = seed;
            this.octaves = Math.max(1, Math.min(octaves, 8));
        }
    }

    /** v + amplitude * sin(2 pi (frequency t + phase)). */
    public static final class Sine extends Modifier {
        public float frequency, amplitude, phase;

        public Sine(float frequency, float amplitude, float phase) {
            this.frequency = frequency;
            this.amplitude = amplitude;
            this.phase = phase;
        }
    }

    /** Remaps time into [start, end] before the keys are sampled. Always first. */
    public static final class Loop extends Modifier {
        public enum Mode { CYCLE, PING_PONG }

        public Mode mode;
        public double start, end;

        public Loop(Mode mode, double start, double end) {
            this.mode = mode;
            this.start = start;
            this.end = end;
        }

        public double remap(double t) {
            double len = end - start;
            if (!(len > 0) || t <= start) return t < start ? t : start;
            double u = (t - start) / len;
            if (mode == Mode.CYCLE) return start + (u - Math.floor(u)) * len;
            double k = u - 2 * Math.floor(u / 2);           // 0..2
            return start + (k <= 1 ? k : 2 - k) * len;
        }
    }

    /** Replaces the value with source(t - delay) * scale + offset. Delay in seconds. */
    public static final class DelayFollow extends Modifier {
        public String source;
        public double delay;
        public float scale, offset;

        public DelayFollow(String source, double delay, float scale, float offset) {
            this.source = source;
            this.delay = delay;
            this.scale = scale;
            this.offset = offset;
        }
    }

    /** Replaces the value with source(t) * scale + offset. */
    public static final class Link extends Modifier {
        public String source;
        public float scale, offset;

        public Link(String source, float scale, float offset) {
            this.source = source;
            this.scale = scale;
            this.offset = offset;
        }
    }
}
