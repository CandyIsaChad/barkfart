package app.barkfart.legacy;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.File;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class MovieclipFormatTest {
    static List<File> referenceClips() {
        File dir = new File(System.getProperty("barkfart.reference", "missing"), "movieclips");
        List<File> out = new ArrayList<File>();
        File[] fs = dir.listFiles();
        if (fs != null) for (File f : fs) if (f.getName().endsWith(".nodemc")) out.add(f);
        return out;
    }

    /** Local-only: every bundled movieclip parses to EOF and its body re-serializes identically. */
    @Test
    void referenceClipsRoundTrip() throws Exception {
        List<File> files = referenceClips();
        assumeTrue(!files.isEmpty(), "reference bundle not present");
        int supported = 0;
        for (File f : files) {
            byte[] file = Files.readAllBytes(f.toPath());
            MovieclipFormat.LegacyMovieclip mc;
            try {
                mc = MovieclipFormat.read(file);
            } catch (MovieclipFormat.UnsupportedLayoutException e) {
                // Allowed only for the documented gap (multi-figure frames); anything else fails the test.
                assertTrue(e.getMessage().contains("several figures"), f.getName() + ": " + e.getMessage());
                System.out.println("[nodemc] " + f.getName() + ": not supported yet (" + e.getMessage() + ")");
                continue;
            }
            supported++;
            assertArrayEquals(MovieclipFormat.unpack(file), MovieclipFormat.writeBody(mc), f.getName());
            assertArrayEquals(MovieclipFormat.writeBody(mc), MovieclipFormat.writeBody(MovieclipFormat.read(MovieclipFormat.write(mc))));
            int instances = 0;
            for (MovieclipFormat.Frame fr : mc.frames) instances += fr.figures.size();
            System.out.printf("[nodemc] %s: %d frames @ %d fps, %d library figures, %d figure instances, tween %d%n",
                    f.getName(), mc.frames.size(), mc.fps, mc.library.size(), instances, mc.tweening);
        }
        assertEquals(files.size(), supported, "every local clip must parse");
    }
}
