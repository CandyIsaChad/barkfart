package app.barkfart.audio;

/** Decoding and output for one platform. Every method returns errors; none throws. */
public interface AudioBackend {
    /** Decodes a sound file (the name gives the type); null when it cannot. */
    Pcm decode(String name, byte[] bytes);

    /**
     * Plays the plan from timeline time t, looping back to loopStart at loopEnd
     * seconds when loopEnd > loopStart. Replaces anything playing. Returns null,
     * or why not.
     */
    String play(AudioPlan plan, double t, double loopStart, double loopEnd);

    void stop();

    /** Opens the output, plays a short silent block and waits for it (about a second at most; not on the UI thread). Null when output works. */
    String check();

    /** Silent stand-in (tests, platforms without audio). */
    AudioBackend NONE = new AudioBackend() {
        public Pcm decode(String name, byte[] bytes) {
            return Wav.decode(bytes);
        }

        public String play(AudioPlan plan, double t, double loopStart, double loopEnd) {
            return "no audio output on this platform";
        }

        public void stop() {
        }

        public String check() {
            return "no audio output on this platform";
        }
    };
}
