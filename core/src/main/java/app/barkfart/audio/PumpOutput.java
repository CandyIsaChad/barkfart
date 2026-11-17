package app.barkfart.audio;

/**
 * Shared playback loop: a thread mixes blocks from the plan and hands them to
 * the platform (write blocks while the device is full). Platforms implement
 * open, write and close only.
 */
public abstract class PumpOutput implements AudioBackend {
    public static final int RATE = 44100, BLOCK = 2048;

    private Thread thread;
    private volatile boolean running;
    private volatile String problem;

    /** Prepares the device; null or why not. */
    protected abstract String open();

    /** Queues frames (interleaved L, R floats), blocking while the device is full; false = stop. */
    protected abstract boolean write(float[] lr, int frames);

    protected abstract void close();

    /** Decodes; WAV always works (core), other types are the platform's. */
    public Pcm decode(String name, byte[] bytes) {
        if (Wav.looksLike(bytes)) return Wav.decode(bytes);
        return decodeOther(name, bytes);
    }

    protected Pcm decodeOther(String name, byte[] bytes) {
        return null;
    }

    public synchronized String play(final AudioPlan plan, final double t, final double loopEnd) {
        stop();
        String err;
        try {
            err = open();
        } catch (RuntimeException e) {
            err = "audio: " + e;
        }
        if (err != null) return problem = err;
        running = true;
        thread = new Thread(new Runnable() {
            public void run() {
                float[] buf = new float[BLOCK * 2];
                double time = t;
                double last = 0;   // without looping, stop once every clip has ended
                for (AudioPlan.Entry e : plan.entries) last = Math.max(last, e.end);
                try {
                    while (running) {
                        if (loopEnd <= 0 && time > last + 0.3) break;
                        if (loopEnd > 0 && time >= loopEnd) time %= loopEnd;
                        int n = BLOCK;
                        if (loopEnd > 0) n = (int) Math.max(1, Math.min(BLOCK, Math.ceil((loopEnd - time) * RATE)));   // split at the loop point
                        Mixer.mix(plan, time, n, RATE, buf);
                        if (!running || !write(buf, n)) break;
                        time += n / (double) RATE;
                    }
                } catch (RuntimeException e) {
                    problem = "audio: " + e;
                }
            }
        }, "audio");
        thread.setDaemon(true);
        thread.start();
        return null;
    }

    public synchronized void stop() {
        running = false;
        Thread th = thread;
        thread = null;
        if (th != null) {
            th.interrupt();
            try {
                th.join(500);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }
        try {
            close();
        } catch (RuntimeException e) {
            problem = "audio: " + e;
        }
    }

    /** The last playback problem, or null. */
    public String problem() {
        return problem;
    }

    public synchronized String check() {
        String err;
        try {
            err = open();
            if (err == null) {
                float[] silence = new float[BLOCK * 2];
                if (!write(silence, BLOCK) || !write(silence, BLOCK)) err = "audio output did not accept data";
                else err = drained(1000);
            }
        } catch (RuntimeException e) {
            err = "audio: " + e;
        } finally {
            try {
                close();
            } catch (RuntimeException e) {
                // reported below if nothing else was
            }
        }
        return err;
    }

    /** Waits until queued audio has played (platforms that can tell); null when it did. */
    protected String drained(long millis) {
        return null;
    }
}
