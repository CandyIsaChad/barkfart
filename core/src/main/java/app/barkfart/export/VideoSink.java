package app.barkfart.export;

/** A platform video encoder (iOS: H.264 + AAC in .mp4). */
public interface VideoSink {
    /** Result of offering a frame. */
    enum Accept { OK, BUSY, FAILED }

    /** Offers frame i (straight RGBA8, rows top-down); BUSY means try again on a later tick. */
    Accept frame(byte[] rgba, int index);

    /** Finishes the file; done gets null or a reason, on any thread. */
    void finish(Done done);

    void cancel();

    /** Why the last call failed. */
    String problem();

    interface Done {
        void done(String problem);
    }
}
