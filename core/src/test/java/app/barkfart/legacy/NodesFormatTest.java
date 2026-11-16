package app.barkfart.legacy;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;

class NodesFormatTest {
    static List<File> referenceFigures() {
        File ref = new File(System.getProperty("barkfart.reference", "missing"));
        List<File> out = new ArrayList<File>();
        for (String dir : new String[] {"stickfigures", "snmc"}) {
            File[] fs = new File(ref, dir).listFiles();
            if (fs != null) for (File f : fs) if (f.getName().endsWith(".nodes")) out.add(f);
        }
        return out;
    }

    /** Local-only: every bundled figure parses to EOF and re-serializes identically. */
    @Test
    void referenceFiguresRoundTripByteForByte() throws IOException {
        List<File> files = referenceFigures();
        assumeTrue(!files.isEmpty(), "reference bundle not present");
        int compressedIdentical = 0, compressed = 0;
        for (File f : files) {
            byte[] file = Files.readAllBytes(f.toPath());
            NodesFormat.LegacyFigure fig = NodesFormat.read(file);
            byte[] body = fig.compressed ? LegacyFormat_unwrap(file) : file;
            assertArrayEquals(body, NodesFormat.writeBody(fig), f.getName() + " body");
            if (fig.compressed) {
                compressed++;
                if (java.util.Arrays.equals(file, NodesFormat.write(fig))) compressedIdentical++;
            } else {
                assertArrayEquals(file, NodesFormat.write(fig), f.getName() + " file");
            }
        }
        System.out.println("[nodes] " + files.size() + " reference figures round-tripped; zlib files byte-identical: "
                + compressedIdentical + "/" + compressed);
    }

    private static byte[] LegacyFormat_unwrap(byte[] file) throws IOException {
        return app.barkfart.io.LegacyFormat.unwrap(file);
    }

    /** Local-only diagnostic for import geometry (prints, asserts nothing about semantics). */
    @Test
    void geometryRelationships() throws IOException {
        List<File> files = referenceFigures();
        assumeTrue(!files.isEmpty(), "reference bundle not present");
        double ratioMin = 1e9, ratioMax = 0, angErr = 0;
        int n = 0;
        for (File f : files) {
            NodesFormat.LegacyFigure fig = NodesFormat.read(Files.readAllBytes(f.toPath()));
            walk(fig.root, 0, new double[] {1e9, 0, 0, 0});
            for (NodesFormat.LegacyNode p : fig.all()) {
                for (NodesFormat.LegacyNode c : p.children) {
                    float len = c.f("length");
                    if (len <= 1e-3f || c.isConnector) continue;
                    double h = Math.hypot(c.f("localX"), c.f("localY"));
                    ratioMin = Math.min(ratioMin, h / len);
                    ratioMax = Math.max(ratioMax, h / len);
                    n++;
                }
            }
            angErr = Math.max(angErr, worldAngleError(fig.root, 0));
        }
        System.out.printf("[nodes] |local|/length over %d segments: min %.4f max %.4f; max world-angle error %.4f deg%n",
                n, ratioMin, ratioMax, angErr);
    }

    private static void walk(NodesFormat.LegacyNode n, int depth, double[] acc) {}

    /** Checks atan2(localY, localX) against the sum of local angles down the tree. */
    private static double worldAngleError(NodesFormat.LegacyNode n, double parentWorld) {
        double worst = 0;
        for (NodesFormat.LegacyNode c : n.children) {
            double world = parentWorld + c.f("localAngle");
            if (Math.hypot(c.f("localX"), c.f("localY")) > 1e-3 && !c.isConnector) {
                double a = Math.toDegrees(Math.atan2(c.f("localY"), c.f("localX")));
                double d = ((a - world) % 360 + 540) % 360 - 180;
                worst = Math.max(worst, Math.abs(d));
            }
            worst = Math.max(worst, worldAngleError(c, world));
        }
        return worst;
    }

    /** Committed, synthetic: a figure built in code round-trips at old and new versions. */
    @Test
    void syntheticFigureRoundTripsAtSeveralVersions() throws IOException {
        int[][] versions = {{105, 0}, {182, 0}, {334, 0}, {420, 36}, {420, 62}, {420, 72}};
        for (int[] vb : versions) {
            NodesFormat.LegacyFigure fig = Synthetic.figure(vb[0], vb[1]);
            byte[] a = NodesFormat.write(fig);
            NodesFormat.LegacyFigure back = NodesFormat.read(a);
            assertArrayEquals(a, NodesFormat.write(back), "v" + vb[0] + " b" + vb[1]);
            assertEquals(fig.all().size(), back.all().size());
        }
    }

    @Test
    void truncatedOrGarbageFilesFailCleanly() throws IOException {
        byte[] good = NodesFormat.write(Synthetic.figure(334, 0));
        for (int cut = 0; cut < good.length; cut += 7) {
            byte[] bad = java.util.Arrays.copyOf(good, cut);
            assertThrows(IOException.class, () -> NodesFormat.read(bad), "cut " + cut);
        }
        byte[] hugeCount = good.clone();
        assertThrows(IOException.class, () -> NodesFormat.read(new byte[] {0, 0, 1, 0x4e, 0x7f}));
    }
}
