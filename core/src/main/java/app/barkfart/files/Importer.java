package app.barkfart.files;

import app.barkfart.edit.Document;
import app.barkfart.edit.StructureEdits;
import app.barkfart.legacy.FigureMapper;
import app.barkfart.legacy.MovieclipFormat;
import app.barkfart.legacy.MovieclipMapper;
import app.barkfart.legacy.NodesFormat;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.ByteArrayOutputStream;
import java.util.List;

/** Imports legacy files into the open project as undoable edits. */
public final class Importer {
    /** What happened, for the status line. */
    public static final class Result {
        public final Layer layer;
        public final String message;
        public final List<String> approximations;

        Result(Layer layer, String message, List<String> approximations) {
            this.layer = layer;
            this.message = message;
            this.approximations = approximations;
        }
    }

    /** Thrown with a user-facing message for files that cannot be imported (yet). */
    public static final class Unsupported extends Exception {
        public Unsupported(String m) {
            super(m);
        }
    }

    private Importer() {}

    public static byte[] read(File f) throws IOException {
        if (f.length() > 512L * 1024 * 1024) throw new IOException("file too large");
        FileInputStream in = new FileInputStream(f);
        try {
            ByteArrayOutputStream out = new ByteArrayOutputStream((int) Math.max(16, f.length()));
            byte[] b = new byte[65536];
            int n;
            while ((n = in.read(b)) > 0) out.write(b, 0, n);
            return out.toByteArray();
        } finally {
            in.close();
        }
    }

    /** Imports a file into doc's root symbol (one undo step). Sounds: WAV only (no platform decoder), at time 0. */
    public static Result importFile(Document doc, File f) throws IOException, Unsupported {
        return importFile(doc, f, app.barkfart.audio.AudioBackend.NONE, 0);
    }

    /** Imports a file; sounds are decoded with audio (to refuse what cannot play) and placed at time at. */
    public static Result importFile(Document doc, File f, app.barkfart.audio.AudioBackend audio, double at) throws IOException, Unsupported {
        ProjectStore.Kind k = ProjectStore.kindOf(f.getName());
        String name = new ProjectStore.Entry(f, k).name;
        byte[] bytes = read(f);
        Project p = doc.project;
        Symbol root = p.root();
        switch (k) {
            case FIGURE: {
                NodesFormat.LegacyFigure fig = NodesFormat.read(bytes);
                FigureMapper.Report rep = new FigureMapper.Report();
                Layer.FigureLayer l = FigureMapper.toLayer(p, fig, name, rep);
                doc.apply(StructureEdits.addLayer(root, l, root.layers.size()));
                return new Result(l, "Imported figure " + name, rep.approximations);
            }
            case MOVIECLIP: {
                MovieclipFormat.LegacyMovieclip mc;
                try {
                    mc = MovieclipFormat.read(bytes);
                } catch (MovieclipFormat.UnsupportedLayoutException e) {
                    throw new Unsupported(name + ": " + e.getMessage());
                }
                FigureMapper.Report rep = new FigureMapper.Report();
                doc.begin("Import " + name);
                try {
                    // Build the symbol in a scratch project first, then add it undoably.
                    Symbol s = MovieclipMapper.toSymbol(p, mc, rep);
                    p.library.remove(s.id);
                    doc.apply(StructureEdits.addSymbol(p, s));
                    Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), name, s.id);
                    inst.loop = Layer.SymbolLayer.LoopMode.LOOP;
                    doc.apply(StructureEdits.addLayer(root, inst, root.layers.size()));
                    return new Result(inst, "Imported movieclip " + name + " (" + mc.frames.size() + " frames)", rep.approximations);
                } finally {
                    doc.commit();
                }
            }
            case LEGACY_PROJECT: {
                // Partial: the project's figure library comes in; its animation does not yet.
                app.barkfart.legacy.StkndsFormat.Header h;
                try {
                    h = app.barkfart.legacy.StkndsFormat.readHeader(bytes);
                } catch (MovieclipFormat.UnsupportedLayoutException e) {
                    throw new Unsupported(name + ": " + e.getMessage());
                }
                if (h.figures.isEmpty()) throw new Unsupported(name + " has no figures to import");
                FigureMapper.Report rep = new FigureMapper.Report();
                Layer last = null;
                doc.begin("Import figures from " + name);
                try {
                    int i = 0;
                    for (MovieclipFormat.NamedFigure nf : h.figures) {
                        Layer.FigureLayer l = FigureMapper.toLayer(p, nf.figure, nf.name, rep);
                        l.transform.x.setValue((i - (h.figures.size() - 1) / 2f) * 160f);   // side by side
                        doc.apply(StructureEdits.addLayer(root, l, root.layers.size()));
                        last = l;
                        i++;
                    }
                } finally {
                    doc.commit();
                }
                return new Result(last, "Imported " + h.figures.size() + " figures from " + name
                        + " (its animation cannot be imported yet)", rep.approximations);
            }
            case SOUND: {
                app.barkfart.audio.Pcm pcm = audio.decode(f.getName(), bytes);
                if (pcm == null) {
                    throw new Unsupported("Can't play " + f.getName() + " on this device. Use mp3, m4a or wav"
                            + (f.getName().toLowerCase().endsWith(".ogg") ? " (ogg is not supported by iOS)" : "") + ".");
                }
                String ext = f.getName().substring(f.getName().lastIndexOf('.')).toLowerCase();
                String aid = p.newId("A");
                app.barkfart.model.Asset a = new app.barkfart.model.Asset(aid, app.barkfart.model.Asset.Kind.AUDIO, "assets/" + aid + ext);
                Layer.AudioLayer al = new Layer.AudioLayer(p.newId("L"), name);
                double start = at == at && at >= 0 ? at : 0;
                al.clips.add(new app.barkfart.model.Clip(al.id, p.newId("C"), a.id, start, 0, pcm.seconds()));
                doc.begin("Add sound " + name);
                try {
                    doc.apply(StructureEdits.addAsset(p, a, bytes));
                    doc.apply(StructureEdits.addLayer(root, al, root.layers.size()));
                } finally {
                    doc.commit();
                }
                return new Result(al, String.format("Added sound %s (%.1f s) at %.2f s", name, pcm.seconds(), start), new java.util.ArrayList<String>());
            }
            case IMAGE: {
                if (bytes.length > 6 && bytes[0] == 'G' && bytes[1] == 'I' && bytes[2] == 'F') {
                    app.barkfart.image.GifDecoder.Result gif = app.barkfart.image.GifDecoder.decode(bytes);
                    if (gif == null) throw new Unsupported("Can't read " + f.getName() + ".");
                    if (gif.frames.size() > 1) return importGif(doc, name, gif);
                    // one frame: an ordinary picture, stored as PNG
                    bytes = app.barkfart.export.Png.encode(gif.frames.get(0).rgba, gif.w, gif.h, true);
                }
                int[] size = app.barkfart.image.ImageInfo.size(bytes);
                if (size == null || size[0] <= 0 || size[1] <= 0) throw new Unsupported("Can't read " + f.getName() + " (use png or jpg).");
                String ext = f.getName().toLowerCase().endsWith(".gif") ? ".png" : f.getName().substring(f.getName().lastIndexOf('.')).toLowerCase();
                String aid = p.newId("A");
                app.barkfart.model.Asset a = new app.barkfart.model.Asset(aid, app.barkfart.model.Asset.Kind.IMAGE, "assets/" + aid + ext);
                Layer.SpriteLayer sl = new Layer.SpriteLayer(p.newId("L"), name);
                sl.asset = aid;
                // keep the picture's pixel size, but fit it within 60% of the stage
                float fit = Math.min(1f, Math.min(p.settings.width * 0.6f / size[0], p.settings.height * 0.6f / size[1]));
                sl.width = size[0] * fit;
                sl.height = size[1] * fit;
                sl.transform.anchorX.setValue(sl.width / 2);
                sl.transform.anchorY.setValue(sl.height / 2);
                doc.begin("Add image " + name);
                try {
                    doc.apply(StructureEdits.addAsset(p, a, bytes));
                    doc.apply(StructureEdits.addLayer(root, sl, root.layers.size()));
                } finally {
                    doc.commit();
                }
                return new Result(sl, "Added image " + name + " (" + size[0] + " × " + size[1] + ")", new java.util.ArrayList<String>());
            }
            case PIVOT:
                throw new Unsupported("Pivot figures (.stk) cannot be imported yet");
            default:
                throw new Unsupported(f.getName() + " is not a file Barkfart can import");
        }
    }

    /**
     * An animated GIF as a movieclip (Stick Nodes 4.2.7): one sprite per
     * frame, each shown only during its own time, looping with the GIF's
     * timing. One undo step.
     */
    static Result importGif(Document doc, String name, app.barkfart.image.GifDecoder.Result gif) {
        Project p = doc.project;
        Symbol root = p.root();
        float fit = Math.min(1f, Math.min(p.settings.width * 0.6f / gif.w, p.settings.height * 0.6f / gif.h));
        double total = 0;
        for (app.barkfart.image.GifDecoder.Frame fr : gif.frames) total += fr.delay;
        Symbol sym = new Symbol(p.newId("S"), name, total);
        doc.begin("Add GIF " + name);
        try {
            double at = 0;
            int k = 0;
            for (app.barkfart.image.GifDecoder.Frame fr : gif.frames) {
                String aid = p.newId("A");
                app.barkfart.model.Asset a = new app.barkfart.model.Asset(aid, app.barkfart.model.Asset.Kind.IMAGE, "assets/" + aid + ".png");
                doc.apply(StructureEdits.addAsset(p, a, app.barkfart.export.Png.encode(fr.rgba, gif.w, gif.h, true)));
                Layer.SpriteLayer sl = new Layer.SpriteLayer(p.newId("L"), name + " " + (++k));
                sl.asset = aid;
                sl.width = gif.w * fit;
                sl.height = gif.h * fit;
                sl.transform.anchorX.setValue(sl.width / 2);
                sl.transform.anchorY.setValue(sl.height / 2);
                sl.shown.setValue(Boolean.FALSE);
                if (at > 0) sl.shown.track().put(new app.barkfart.model.Key(0, Boolean.FALSE, app.barkfart.model.Interp.HOLD));
                sl.shown.track().put(new app.barkfart.model.Key(at, Boolean.TRUE, app.barkfart.model.Interp.HOLD));
                if (at + fr.delay < total - 1e-9) sl.shown.track().put(new app.barkfart.model.Key(at + fr.delay, Boolean.FALSE, app.barkfart.model.Interp.HOLD));
                sym.layers.add(sl);
                at += fr.delay;
            }
            doc.apply(StructureEdits.addSymbol(p, sym));
            Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), name, sym.id);
            inst.loop = Layer.SymbolLayer.LoopMode.LOOP;
            doc.apply(StructureEdits.addLayer(root, inst, root.layers.size()));
            return new Result(inst, String.format("Added GIF %s (%d frames, %.2f s, as a movieclip)", name, gif.frames.size(), total), new java.util.ArrayList<String>());
        } finally {
            doc.commit();
        }
    }

    /** Writes a figure layer as a Stick Nodes .nodes file into the folder. */
    public static File exportFigure(ProjectStore store, Layer.FigureLayer l) throws IOException {
        File f = store.unique(l.name, ".nodes");
        byte[] bytes = NodesFormat.write(FigureMapper.toLegacy(l));
        java.io.FileOutputStream out = new java.io.FileOutputStream(f);
        try {
            out.write(bytes);
        } finally {
            out.close();
        }
        return f;
    }
}
