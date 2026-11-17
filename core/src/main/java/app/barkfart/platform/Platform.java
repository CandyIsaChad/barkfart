package app.barkfart.platform;

import java.io.File;
import java.util.List;

/** What the editor needs from the OS. iOS and desktop provide their own. */
public interface Platform {
    /** One picked file: its name and contents. */
    final class Picked {
        public final String name;
        public final byte[] bytes;

        public Picked(String name, byte[] bytes) {
            this.name = name;
            this.bytes = bytes;
        }
    }

    interface PickCallback {
        /** Called on the render thread; empty when cancelled. */
        void picked(List<Picked> files);
    }

    /** The user's Barkfart folder (iOS: Documents, visible in the Files app). */
    File documents();

    /** True when a system file picker exists (iOS). */
    boolean canPick();

    /** Shows the system picker for importable files. */
    void pickFiles(PickCallback cb);

    /** True when files can be shared/exported through the OS (iOS share sheet). */
    boolean canShare();

    /** Offers the file to other apps / Save to Files. */
    void share(File f);

    /** Sound decoding and output. */
    app.barkfart.audio.AudioBackend audio();

    /** Default: no picker or sharing, documents in ~/Barkfart (desktop, tests). */
    class Basic implements Platform {
        private final File docs;

        public Basic(File docs) {
            this.docs = docs;
        }

        public File documents() {
            return docs;
        }

        public boolean canPick() {
            return false;
        }

        public void pickFiles(PickCallback cb) {
            cb.picked(java.util.Collections.<Picked>emptyList());
        }

        public boolean canShare() {
            return false;
        }

        public void share(File f) {
        }

        private app.barkfart.audio.AudioBackend audio;

        public app.barkfart.audio.AudioBackend audio() {
            if (audio == null) audio = new app.barkfart.audio.GdxAudio();
            return audio;
        }
    }
}
