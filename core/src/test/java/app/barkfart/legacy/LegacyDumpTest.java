package app.barkfart.legacy;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.files.Importer;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import java.io.File;
import java.io.FileOutputStream;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

/**
 * Reference data for the Rust legacy importers (docs/barkfart/PORT.md). Only
 * runs with -Dbarkfart.legacydump=<dir>. For each sample: the input file,
 * the project before and after Importer.importFile, and every imported
 * figure re-exported as .nodes. Synthetic samples go to <dir>/synthetic
 * (committed); samples from the local reference bundle go to <dir>/local
 * (never committed: derived from the original app's files).
 */
class LegacyDumpTest {
    @Test
    void dump() throws Exception {
        String dir = System.getProperty("barkfart.legacydump", "");
        Assumptions.assumeTrue(!dir.isEmpty(), "legacy dump not requested");
        File syn = new File(dir, "synthetic");
        syn.mkdirs();
        int[][] versions = {{100, 0}, {200, 0}, {230, 0}, {300, 0}, {403, 38}, {403, 41}, {420, 62}, {431, 117}};
        for (int[] v : versions) {
            byte[] bytes = NodesFormat.write(Synthetic.figure(v[0], v[1]));
            File f = new File(syn, "fig-v" + v[0] + "b" + v[1] + ".nodes");
            write(f, bytes);
            dumpImport(f, syn);
        }
        // media: a sound, a picture, an animated GIF and a one-frame GIF
        File wav = new File(syn, "beep.wav");
        write(wav, app.barkfart.audio.Wav.encode(app.barkfart.audio.Mixer.tone(660, 0.25, 22050, 0.4f)));
        dumpImport(wav, syn);
        byte[] px = new byte[12 * 9 * 4];
        for (int k = 0; k < 12 * 9; k++) {
            px[k * 4] = (byte) (k * 7);
            px[k * 4 + 1] = (byte) 200;
            px[k * 4 + 2] = (byte) (k * 3);
            px[k * 4 + 3] = (byte) 255;
        }
        File png = new File(syn, "picture.png");
        write(png, app.barkfart.export.Png.encode(px, 12, 9, true));
        dumpImport(png, syn);
        for (int frames : new int[] {1, 5}) {
            java.io.ByteArrayOutputStream gb = new java.io.ByteArrayOutputStream();
            app.barkfart.export.Gif g = new app.barkfart.export.Gif(gb, 12, 9, 12, true);
            for (int f = 0; f < frames; f++) {
                byte[] q = px.clone();
                for (int k = 0; k < 12 * 9; k++) q[k * 4 + 1] = (byte) (f * 50);
                g.frame(q);
            }
            g.finish();
            File gf = new File(syn, "anim" + frames + ".gif");
            write(gf, gb.toByteArray());
            dumpImport(gf, syn);
        }
        File ref = new File(System.getProperty("barkfart.root", ".."), "main");
        if (!ref.isDirectory()) return;
        File local = new File(dir, "local");
        local.mkdirs();
        java.util.List<File> found = new java.util.ArrayList<File>();
        collect(ref, found);
        java.util.Collections.sort(found);
        for (File f : found) {
            File copy = new File(local, f.getName());
            write(copy, java.nio.file.Files.readAllBytes(f.toPath()));
            dumpImport(copy, local);
        }
    }

    private static void collect(File d, java.util.List<File> out) {
        File[] fs = d.listFiles();
        if (fs == null) return;
        for (File f : fs) {
            if (f.isDirectory()) collect(f, out);
            else {
                String n = f.getName().toLowerCase();
                if (n.endsWith(".nodes") || n.endsWith(".nodemc") || n.endsWith(".stknds")) out.add(f);
            }
        }
    }

    private static void dumpImport(File f, File dir) throws Exception {
        Project p = Fixtures.empty();
        String base = f.getName();
        write(new File(dir, base + ".before.json"), ProjectCodec.encode(p).getBytes("UTF-8"));
        try {
            Importer.importFile(new Document(p), f);
            write(new File(dir, base + ".after.json"), ProjectCodec.encode(p).getBytes("UTF-8"));
            int k = 0;
            for (app.barkfart.model.Symbol s : p.library.values()) {
                for (Layer l : s.layers) {
                    if (!(l instanceof Layer.FigureLayer)) continue;
                    write(new File(dir, base + ".export" + (k++) + ".nodes"), NodesFormat.write(FigureMapper.toLegacy((Layer.FigureLayer) l)));
                }
            }
        } catch (Importer.Unsupported e) {
            write(new File(dir, base + ".unsupported.txt"), String.valueOf(e.getMessage()).getBytes("UTF-8"));
        } catch (java.io.IOException e) {
            write(new File(dir, base + ".error.txt"), String.valueOf(e.getMessage()).getBytes("UTF-8"));
        }
    }

    private static void write(File f, byte[] b) throws Exception {
        FileOutputStream o = new FileOutputStream(f);
        o.write(b);
        o.close();
    }
}
