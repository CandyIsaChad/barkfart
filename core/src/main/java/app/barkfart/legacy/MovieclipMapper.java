package app.barkfart.legacy;

import app.barkfart.model.Color;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Legacy movieclip to Symbol (DESIGN §7): one figure layer per figure
 * instance; every frame keys the full pose of each present figure at
 * frame / fps, plus position, scale, opacity and hold keys on "shown".
 * Stage coordinates (1920x1080, origin bottom-left, y up) map to Barkfart
 * coordinates with the stage centre at 0,0.
 */
public final class MovieclipMapper {
    public static final float STAGE_W = 1920, STAGE_H = 1080;

    private MovieclipMapper() {}

    public static Symbol toSymbol(Project p, MovieclipFormat.LegacyMovieclip mc, FigureMapper.Report report) {
        double fps = mc.fps > 0 ? mc.fps : 10;
        Symbol sym = p.addSymbol(new Symbol(p.newId("S"), mc.name, mc.frames.size() / fps));
        Interp interp = mc.tweening != 0 ? Interp.LINEAR : Interp.HOLD;
        Map<Integer, Layer.FigureLayer> layers = new LinkedHashMap<Integer, Layer.FigureLayer>();
        // Pass 1: layers for every instance, in first-appearance order.
        for (MovieclipFormat.Frame fr : mc.frames) {
            for (MovieclipFormat.FrameFigure ff : fr.figures) {
                if (layers.containsKey(ff.instanceId())) continue;
                MovieclipFormat.NamedFigure nf = mc.library.get(ff.libraryId);
                Layer.FigureLayer l = FigureMapper.toLayer(p, nf.figure, nf.name, report);
                layers.put(ff.instanceId(), l);
            }
        }
        // Pass 2: keys.
        for (int f = 0; f < mc.frames.size(); f++) {
            double t = f / fps;
            MovieclipFormat.Frame fr = mc.frames.get(f);
            java.util.HashSet<Integer> present = new java.util.HashSet<Integer>();
            for (MovieclipFormat.FrameFigure ff : fr.figures) {
                Layer.FigureLayer l = layers.get(ff.instanceId());
                present.add(ff.instanceId());
                l.shown.track().put(new Key(t, Boolean.TRUE, Interp.HOLD));
                key(l.transform.x, t, ff.stageX() - STAGE_W / 2, interp);
                key(l.transform.y, t, ff.stageY() - STAGE_H / 2, interp);
                key(l.transform.scaleX, t, ff.scale(), interp);
                key(l.transform.scaleY, t, ff.scale(), interp);
                key(l.opacity, t, ff.opacity(), interp);
                Color figColor = FigureMapper.color(MovieclipFormat.be32(ff.header, 75));
                List<NodesFormat.LegacyNode> lib = mc.library.get(ff.libraryId).figure.all();
                List<Node> nodes = l.figure.nodes();
                for (int i = 0; i < nodes.size() && i < ff.nodes.size(); i++) {
                    byte[] r = ff.nodes.get(i);
                    Node n = nodes.get(i);
                    key(n.rotation, t, MovieclipFormat.FrameFigure.nodeLocalAngle(r), interp);
                    key(n.length, t, MovieclipFormat.FrameFigure.nodeLength(r), interp);
                    key(n.thickness, t, (float) MovieclipFormat.FrameFigure.nodeThickness(r), interp);
                    boolean own = lib.get(i).i("useSegmentColor") != 0;
                    Color c = own ? FigureMapper.color(MovieclipFormat.FrameFigure.nodeColor(r)) : figColor;
                    n.color.track().put(new Key(t, c, interp));
                }
            }
            for (Map.Entry<Integer, Layer.FigureLayer> e : layers.entrySet()) {
                if (!present.contains(e.getKey())) e.getValue().shown.track().put(new Key(t, Boolean.FALSE, Interp.HOLD));
            }
        }
        for (Layer.FigureLayer l : layers.values()) sym.layers.add(l);
        p.invalidateIndex();
        return sym;
    }

    private static void key(app.barkfart.model.Property prop, double t, float v, Interp interp) {
        prop.track().put(new Key(t, v, interp));
    }
}
