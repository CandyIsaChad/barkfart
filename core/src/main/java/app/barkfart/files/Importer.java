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

    /** Imports a file into doc's root symbol (one undo step). */
    public static Result importFile(Document doc, File f) throws IOException, Unsupported {
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
            case PIVOT:
                throw new Unsupported("Pivot figures (.stk) cannot be imported yet");
            default:
                throw new Unsupported(f.getName() + " is not a file Barkfart can import");
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
