package app.barkfart.geom;

import app.barkfart.eval.Evaluator;
import app.barkfart.model.Project;

/**
 * Onion skin from the evaluator: tinted ghosts of the scene at earlier and
 * later frames, fading with distance. Layers with onion = false are left out.
 */
public final class OnionSkin {
    public int before = 2, after = 1;
    /** Ghost colours (RGB, alpha from opacity) for past and future frames. */
    public int pastRgb = 0xd04040, futureRgb = 0x3070d0;
    public float opacity = 0.35f, falloff = 0.6f;
    /** Ghosts in stage space (editing view) unless set. */
    public boolean applyCamera;

    /** Ghost draw list for time t at the project fps, farthest ghosts first. */
    public DrawList build(Project p, String symbolId, double t) {
        DrawList out = new DrawList();
        double fps = p.settings.fps > 0 ? p.settings.fps : 30;
        for (int k = Math.max(before, after); k >= 1; k--) {
            float a = opacity * (float) Math.pow(falloff, k - 1);
            if (k <= before) tint(Tessellator.tessellate(Evaluator.evaluate(p, symbolId, t - k / fps, true, applyCamera)), pastRgb, a, out);
            if (k <= after) tint(Tessellator.tessellate(Evaluator.evaluate(p, symbolId, t + k / fps, true, applyCamera)), futureRgb, a, out);
        }
        return out;
    }

    /**
     * The same ghosts as an isolate tree: each ghost drawn solid in its tint,
     * then faded as one piece, so a ghost's own overlaps do not show darker
     * (draw with the effect renderer).
     */
    public DrawList buildLayered(Project p, String symbolId, double t) {
        DrawList out = new DrawList();
        out.open(symbolId, 1f, app.barkfart.model.Layer.Blend.NORMAL, false, java.util.Collections.<app.barkfart.eval.ResolvedEffect>emptyList());
        double fps = p.settings.fps > 0 ? p.settings.fps : 30;
        for (int k = Math.max(before, after); k >= 1; k--) {
            float a = opacity * (float) Math.pow(falloff, k - 1);
            for (int side = 0; side < 2; side++) {
                if (side == 0 ? k > before : k > after) continue;
                double at = side == 0 ? t - k / fps : t + k / fps;
                DrawList ghost = Tessellator.tessellate(Evaluator.evaluate(p, symbolId, at, true, applyCamera));
                out.open("ghost", Math.min(1f, a), app.barkfart.model.Layer.Blend.NORMAL, false, java.util.Collections.<app.barkfart.eval.ResolvedEffect>emptyList());
                for (DrawList.Batch b : ghost.batches) {
                    DrawList.Batch o = out.batch(app.barkfart.model.Layer.Blend.NORMAL, null);
                    for (int i = 0; i < b.vertexCount(); i++) {
                        int srcA = b.rgba(i) & 0xff;
                        o.add(b.x(i), b.y(i), ((side == 0 ? pastRgb : futureRgb) << 8) | srcA);
                    }
                }
                out.close();
            }
        }
        out.close();
        return out;
    }

    private static void tint(DrawList src, int rgb, float alpha, DrawList out) {
        for (DrawList.Batch b : src.batches) {
            DrawList.Batch o = out.batch(app.barkfart.model.Layer.Blend.NORMAL, null);
            for (int i = 0; i < b.vertexCount(); i++) {
                int srcA = b.rgba(i) & 0xff;
                int a = Math.round(alpha * srcA);
                o.add(b.x(i), b.y(i), (rgb << 8) | a);
            }
        }
    }
}
