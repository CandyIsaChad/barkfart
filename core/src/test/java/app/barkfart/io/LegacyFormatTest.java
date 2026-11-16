package app.barkfart.io;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.util.zip.DeflaterOutputStream;
import java.util.zip.GZIPOutputStream;
import org.junit.jupiter.api.Test;

class LegacyFormatTest {
    private static final byte[] BODY = "hello nodes".getBytes();

    @Test
    void unwrapsZlibStickfigure() throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (DeflaterOutputStream z = new DeflaterOutputStream(out)) { z.write(BODY); }
        byte[] file = out.toByteArray();
        assertEquals(LegacyFormat.Kind.STICKFIGURE, LegacyFormat.sniff(file));
        assertArrayEquals(BODY, LegacyFormat.unwrap(file));
    }

    @Test
    void unwrapsPackedProject() throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(LegacyFormat.PACKED_MAGIC);
        try (GZIPOutputStream z = new GZIPOutputStream(out)) { z.write(BODY); }
        byte[] file = out.toByteArray();
        assertEquals(LegacyFormat.Kind.PACKED, LegacyFormat.sniff(file));
        assertArrayEquals(BODY, LegacyFormat.unwrap(file));
    }

    @Test
    void passesThroughRawBigEndianFigure() throws IOException {
        byte[] file = {0, 0, 1, 0x4e, 0x3f, (byte) 0x80, 0, 0};
        assertEquals(LegacyFormat.Kind.RAW, LegacyFormat.sniff(file));
        assertArrayEquals(file, LegacyFormat.unwrap(file));
    }

    @Test
    void rejectsGarbage() {
        byte[] junk = {0, 1, 2, 3};
        assertEquals(LegacyFormat.Kind.UNKNOWN, LegacyFormat.sniff(junk));
        assertThrows(IOException.class, () -> LegacyFormat.unwrap(junk));
    }

    /** Local-only: runs against the reference bundle when it is present (never committed). */
    @Test
    void unwrapsReferenceSamples() throws IOException {
        File ref = new File(System.getProperty("barkfart.reference", "missing"));
        assumeTrue(ref.isDirectory(), "reference bundle not present");
        int checked = 0;
        for (String dir : new String[] {"stickfigures", "movieclips", "sample_projects", "snmc"}) {
            File[] files = new File(ref, dir).listFiles();
            if (files == null) continue;
            for (File f : files) {
                byte[] body = LegacyFormat.unwrap(Files.readAllBytes(f.toPath()));
                assertTrue(body.length > 4, f.getName());
                checked++;
            }
        }
        assertTrue(checked > 0);
    }
}
