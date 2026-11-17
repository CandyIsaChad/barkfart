package app.barkfart.io;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Project;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.util.Random;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class RoundTripTest {
    @TempDir
    File tmp;

    @Test
    void encodeDecodeEncodeIsIdentical() throws Exception {
        Project p = Fixtures.kitchenSink();
        String a = ProjectCodec.encode(p);
        String b = ProjectCodec.encode(ProjectCodec.decode(a));
        assertEquals(a, b);
        String c = ProjectCodec.encode(ProjectCodec.decode(b));
        assertEquals(b, c);
    }

    @Test
    void containerBytesAreIdentical() throws Exception {
        Project p = Fixtures.kitchenSink();
        byte[] a = bytes(p);
        byte[] b = bytes(BarkFile.read(new ByteArrayInputStream(a)));
        assertArrayEquals(a, b);
    }

    @Test
    void reloadedProjectRendersIdentically() throws Exception {
        Project p = Fixtures.kitchenSink();
        Project q = ProjectCodec.decode(ProjectCodec.encode(p));
        for (double t : new double[] {0, 0.3, 0.75, 1.0, 1.49, 2.5}) {
            DrawList a = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, t));
            DrawList b = Tessellator.tessellate(Evaluator.evaluate(q, q.rootSymbol, t));
            assertEquals(a.batches.size(), b.batches.size(), "batches at " + t);
            for (int i = 0; i < a.batches.size(); i++) {
                DrawList.Batch x = a.batches.get(i), y = b.batches.get(i);
                assertEquals(x.vertexCount(), y.vertexCount());
                for (int v = 0; v < x.vertexCount(); v++) {
                    assertEquals(x.x(v), y.x(v), 1e-4, "x at " + t);
                    assertEquals(x.y(v), y.y(v), 1e-4, "y at " + t);
                    assertEquals(x.rgba(v), y.rgba(v), "colour at " + t);
                }
            }
        }
    }

    @Test
    void atomicSaveKeepsBackup() throws Exception {
        File f = new File(tmp, "a.bark");
        Project p = Fixtures.kitchenSink();
        BarkFile.save(p, f);
        p.settings.fps = 60;
        BarkFile.save(p, f);
        assertEquals(60f, BarkFile.load(f).settings.fps);
        assertEquals(24f, BarkFile.load(new File(tmp, "a.bark.bak")).settings.fps);
        assertFalse(new File(tmp, "a.bark.tmp").exists());
    }

    @Test
    void newerFormatIsRefused() {
        String s = ProjectCodec.encode(Fixtures.empty()).replace("\"formatVersion\": 1", "\"formatVersion\": 999");
        ProjectCodec.FormatException e = assertThrows(ProjectCodec.FormatException.class, () -> ProjectCodec.decode(s));
        assertTrue(e.getMessage().contains("newer"));
    }

    /** Corrupted text must fail with FormatException (or load), never another exception. */
    @Test
    void corruptInputFailsCleanly() {
        String good = ProjectCodec.encode(Fixtures.kitchenSink());
        Random r = new Random(1234);
        String alphabet = "{}[]\",:0123456789.-eE ntrufalsx#\\";
        int loaded = 0, rejected = 0;
        for (int i = 0; i < 3000; i++) {
            StringBuilder sb = new StringBuilder(good);
            int edits = 1 + r.nextInt(4);
            for (int e = 0; e < edits; e++) {
                int pos = r.nextInt(sb.length());
                switch (r.nextInt(3)) {
                    case 0: sb.deleteCharAt(pos); break;
                    case 1: sb.insert(pos, alphabet.charAt(r.nextInt(alphabet.length()))); break;
                    default: sb.setCharAt(pos, alphabet.charAt(r.nextInt(alphabet.length())));
                }
            }
            try {
                Project p = ProjectCodec.decode(sb.toString());
                Evaluator.evaluate(p, p.rootSymbol, 0.5);
                loaded++;
            } catch (ProjectCodec.FormatException ex) {
                rejected++;
            }
        }
        assertTrue(rejected > 0 && loaded + rejected == 3000);
    }

    @Test
    void notAZipOrMissingEntryFailsCleanly() {
        assertThrows(Exception.class, () -> BarkFile.read(new ByteArrayInputStream(new byte[] {1, 2, 3})));
    }

    private static byte[] bytes(Project p) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        BarkFile.write(p, out);
        return out.toByteArray();
    }

    @Test
    void assetsLiveInTheContainerAndSurviveRepeatedSaves() throws Exception {
        File f = new File(tmp, "assets.bark");
        Project p = Fixtures.kitchenSink();
        byte[] img = new byte[50000], snd = new byte[1234];
        new Random(1).nextBytes(img);
        new Random(2).nextBytes(snd);
        java.util.List<app.barkfart.model.Asset> as = new java.util.ArrayList<app.barkfart.model.Asset>(p.assets.values());
        p.assetData.put(as.get(0).id, img);
        p.assetData.put(as.get(1).id, snd);
        BarkFile.save(p, f);
        Project q = BarkFile.load(f);
        assertArrayEquals(img, q.assetData.bytes(as.get(0).id));
        // Save the loaded project twice (assets come from the file lazily), then reload.
        BarkFile.save(q, f);
        BarkFile.save(q, f);
        Project r = BarkFile.load(f);
        assertArrayEquals(img, r.assetData.bytes(as.get(0).id));
        assertArrayEquals(snd, r.assetData.bytes(as.get(1).id));
        // Streams load assets eagerly.
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        BarkFile.write(r, out);
        Project s = BarkFile.read(new ByteArrayInputStream(out.toByteArray()));
        assertArrayEquals(snd, s.assetData.bytes(as.get(1).id));
        // Deterministic bytes with assets too.
        ByteArrayOutputStream again = new ByteArrayOutputStream();
        BarkFile.write(s, again);
        assertArrayEquals(out.toByteArray(), again.toByteArray());
    }
}
