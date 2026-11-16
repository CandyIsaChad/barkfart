package app.barkfart.eval;

/** Deterministic 1D value noise in [-1, 1], smooth between integer lattice points. */
final class Noise {
    private Noise() {}

    static double fbm(double x, int seed, int octaves) {
        double sum = 0, amp = 1, norm = 0, f = 1;
        for (int o = 0; o < octaves; o++) {
            sum += amp * value(x * f, seed + o * 1013);
            norm += amp;
            amp *= 0.5;
            f *= 2;
        }
        return sum / norm;
    }

    static double value(double x, int seed) {
        double fl = Math.floor(x);
        long i = (long) fl;
        double u = x - fl;
        u = u * u * (3 - 2 * u);
        double a = hash(i, seed), b = hash(i + 1, seed);
        return a + (b - a) * u;
    }

    private static double hash(long i, int seed) {
        long h = i * 0x9E3779B97F4A7C15L + seed * 0xBF58476D1CE4E5B9L;
        h ^= h >>> 31;
        h *= 0x94D049BB133111EBL;
        h ^= h >>> 29;
        return ((h >>> 11) * (1.0 / (1L << 53))) * 2 - 1;
    }
}
