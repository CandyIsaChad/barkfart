package app.barkfart.legacy;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.geom.Raster;
import app.barkfart.geom.Tessellator;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.util.List;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

class FigureMapperTest {
    /** Local-only acceptance test: import, save as .bark, load, export: identical bytes. */
    @Test
    void referenceFiguresSurviveImportBarkAndExport() throws Exception {
        List<File> files = NodesFormatTest.referenceFigures();
        assumeTrue(!files.isEmpty(), "reference bundle not present");
        File renders = new File(System.getProperty("barkfart.root", ".."), "core/build/legacy-renders");
        renders.mkdirs();
        for (File f : files) {
            byte[] file = Files.readAllBytes(f.toPath());
            Project p = Fixtures.empty();
            FigureMapper.Report report = new FigureMapper.Report();
            Layer.FigureLayer layer = FigureMapper.toLayer(p, NodesFormat.read(file), f.getName(), report);
            p.root().layers.add(layer);
            p.invalidateIndex();
            Project q = ProjectCodec.decode(ProjectCodec.encode(p));
            Layer.FigureLayer back = (Layer.FigureLayer) q.root().layers.get(0);
            byte[] out = NodesFormat.write(FigureMapper.toLegacy(back));
            assertArrayEquals(file, out, f.getName());
            // Renders for eyeballing; written under build/, never committed.
            ImageIO.write(Raster.fill(Tessellator.tessellate(Evaluator.evaluate(q, q.rootSymbol, 0)), 480, 480, 0xffffffff),
                    "png", new File(renders, f.getName().replaceAll("[^A-Za-z0-9]+", "_") + ".png"));
            if (!report.approximations.isEmpty()) System.out.println("[import] " + f.getName() + " approximates " + report.approximations);
        }
    }

    /** Committed: edits made in Barkfart reach the exported file. */
    @Test
    void editsAreExported() throws IOException {
        NodesFormat.LegacyFigure fig = Synthetic.figure(420, 62);
        Project p = Fixtures.empty();
        Layer.FigureLayer layer = FigureMapper.toLayer(p, fig, "syn", null);
        Node arm = layer.figure.node("n3");
        arm.rotation.setValue(45f);
        arm.length.setValue(80f);
        arm.color.setValue(Color.rgba8888(0x00ff00ff));
        NodesFormat.LegacyFigure out = NodesFormat.read(NodesFormat.write(FigureMapper.toLegacy(layer)));
        NodesFormat.LegacyNode a = null;
        for (NodesFormat.LegacyNode n : out.all()) if (n.i("drawIndex") == 3) a = n;
        assertNotNull(a);
        assertEquals(45f, a.f("localAngle"));
        assertEquals(80f, a.f("length"));
        assertEquals(1, a.i("useSegmentColor"));
        assertEquals(Color.rgba8888(0x00ff00ff), FigureMapper.color(a.i("color")));
        // World angle = torso 90 + 45 = 135; the offset keeps the file's 0.5x convention? torso stored 1x here.
        double ang = Math.toDegrees(Math.atan2(a.f("localY"), a.f("localX")));
        assertEquals(135, ang, 0.01);
    }

    /** Committed: unedited synthetic figures export identically at every version. */
    @Test
    void syntheticRoundTripThroughModel() throws IOException {
        int[][] versions = {{105, 0}, {182, 0}, {334, 0}, {420, 36}, {420, 62}, {420, 72}};
        for (int[] vb : versions) {
            byte[] file = NodesFormat.write(Synthetic.figure(vb[0], vb[1]));
            Project p = Fixtures.empty();
            Layer.FigureLayer layer = FigureMapper.toLayer(p, NodesFormat.read(file), "syn", null);
            assertArrayEquals(file, NodesFormat.write(FigureMapper.toLegacy(layer)), "v" + vb[0] + " b" + vb[1]);
        }
    }
}
