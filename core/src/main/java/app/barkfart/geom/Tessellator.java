package app.barkfart.geom;

import app.barkfart.eval.Affine;
import app.barkfart.eval.Group;
import app.barkfart.eval.Scene;
import app.barkfart.eval.Shape;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;

/**
 * Turns a Scene into a DrawList. Group opacity multiplies into vertex alpha
 * (v1 approximation: overlapping shapes inside a translucent group show their
 * overlaps; M6 renders such groups offscreen instead).
 */
public final class Tessellator {
    /** Target maximum chord error in pixels for round shapes. */
    private static final float TOLERANCE = 0.35f;

    private Tessellator() {}

    /** Glyphs for text shapes during one tessellation (none: text is skipped). */
    private static final ThreadLocal<GlyphSource> GLYPHS = new ThreadLocal<GlyphSource>();

    public static DrawList tessellate(Scene scene) {
        DrawList out = new DrawList();
        walk(scene.root, 1f, Layer.Blend.NORMAL, null, out);
        return out;
    }

    public static DrawList tessellate(Scene scene, GlyphSource glyphs) {
        GLYPHS.set(glyphs);
        try {
            return tessellate(scene);
        } finally {
            GLYPHS.remove();
        }
    }

    public static DrawList layered(Scene scene, GlyphSource glyphs) {
        GLYPHS.set(glyphs);
        try {
            return layered(scene);
        } finally {
            GLYPHS.remove();
        }
    }

    /** Lines centred on the origin; each glyph a textured quad in the text's colour. */
    private static void text(Shape.Text tx, float op, Layer.Blend b, String effects, DrawList out) {
        GlyphSource gs = GLYPHS.get();
        if (gs == null || !(gs.basePx() > 0)) return;
        Color c = tx.color;
        int rgba = new Color(c.r, c.g, c.b, c.a * op).toRgba8888();
        if ((rgba & 0xff) == 0) return;
        float k = tx.size / gs.basePx(), lh = gs.lineHeight();
        String[] lines = tx.text.split("\n", -1);
        float total = lines.length * lh;
        for (int li = 0; li < lines.length && li < 500; li++) {
            String line = lines[li];
            float width = 0;
            for (int i = 0; i < line.length() && i < 10000; i++) {
                GlyphSource.Glyph g = gs.glyph(line.charAt(i));
                width += g == null ? lh * 0.3f : g.advance;
            }
            float pen = -width / 2, base = total / 2 - (li + 1) * lh + lh * 0.25f;
            for (int i = 0; i < line.length() && i < 10000; i++) {
                GlyphSource.Glyph g = gs.glyph(line.charAt(i));
                if (g == null) {
                    pen += lh * 0.3f;
                    continue;
                }
                if (g.w > 0 && g.h > 0) {
                    float x0 = (pen + g.x) * k, y0 = (base + g.y) * k, x1 = x0 + g.w * k, y1 = y0 + g.h * k;
                    DrawList.Batch batch = out.batch(b, effects, g.texture);
                    Affine m = tx.xf;
                    float ax = (float) m.mapX(x0, y0), ay = (float) m.mapY(x0, y0), bx = (float) m.mapX(x1, y0), by = (float) m.mapY(x1, y0);
                    float cx = (float) m.mapX(x1, y1), cy = (float) m.mapY(x1, y1), dx = (float) m.mapX(x0, y1), dy = (float) m.mapY(x0, y1);
                    batch.add(ax, ay, rgba, g.u0, g.v0);
                    batch.add(bx, by, rgba, g.u1, g.v0);
                    batch.add(cx, cy, rgba, g.u1, g.v1);
                    batch.add(ax, ay, rgba, g.u0, g.v0);
                    batch.add(cx, cy, rgba, g.u1, g.v1);
                    batch.add(dx, dy, rgba, g.u0, g.v1);
                }
                pen += g.advance;
            }
        }
    }

    /**
     * Like tessellate, plus the isolate tree (DrawList.root): every group with an
     * enabled effect, an adjustment group, or opacity below 1 becomes an isolate,
     * drawn offscreen and composited once (no overlap seams). Shapes inside an
     * isolate carry no group opacity in their vertex alpha.
     */
    public static DrawList layered(Scene scene) {
        DrawList out = new DrawList();
        out.open(scene.root.source, 1f, Layer.Blend.NORMAL, false, java.util.Collections.<app.barkfart.eval.ResolvedEffect>emptyList());
        layeredWalk(scene.root, 1f, Layer.Blend.NORMAL, out, true);
        out.close();
        return out;
    }

    static boolean isolates(Group g) {
        if (g.adjustment) return true;
        if (g.opacity < 1f) return true;
        for (app.barkfart.eval.ResolvedEffect e : g.effects) if (e.mix > 0) return true;
        return false;
    }

    private static void layeredWalk(Group g, float opacity, Layer.Blend blend, DrawList out, boolean isRoot) {
        if (!isRoot && isolates(g)) {
            float gop = g.opacity;
            if (!(gop > 0)) return;
            java.util.List<app.barkfart.eval.ResolvedEffect> fx = new java.util.ArrayList<app.barkfart.eval.ResolvedEffect>();
            for (app.barkfart.eval.ResolvedEffect e : g.effects) if (e.mix > 0) fx.add(e);
            out.open(g.source, Math.min(1f, gop * opacity), g.blend, g.adjustment, fx);
            for (Object o : g.children) {
                if (o instanceof Group) layeredWalk((Group) o, 1f, Layer.Blend.NORMAL, out, false);
                else emit((Shape) o, 1f, Layer.Blend.NORMAL, null, out);
            }
            out.close();
            return;
        }
        float op = opacity * (isRoot ? 1f : g.opacity);
        if (op <= 0) return;
        Layer.Blend b = g.blend != Layer.Blend.NORMAL ? g.blend : blend;
        for (Object o : g.children) {
            if (o instanceof Group) layeredWalk((Group) o, op, b, out, false);
            else emit((Shape) o, op, b, null, out);
        }
    }

    private static void walk(Group g, float opacity, Layer.Blend blend, String fx, DrawList out) {
        float op = opacity * g.opacity;
        if (op <= 0) return;
        Layer.Blend b = g.blend != Layer.Blend.NORMAL ? g.blend : blend;
        String effects = !g.effects.isEmpty() ? g.source : fx;
        for (Object o : g.children) {
            if (o instanceof Group) {
                walk((Group) o, op, b, effects, out);
                continue;
            }
            emit((Shape) o, op, b, effects, out);
        }
    }

    /** Appends one shape's triangles to the current batch for (blend, effect group, texture). */
    private static void emit(Shape s, float op, Layer.Blend b, String effects, DrawList out) {
        if (s instanceof Shape.Text) {
            text((Shape.Text) s, op, b, effects, out);
            return;
        }
        DrawList.Batch batch = s instanceof Shape.Quad ? out.batch(b, effects, ((Shape.Quad) s).asset) : out.batch(b, effects);
        int first = batch.vertices;
        Color c = s.color;
        int rgba = new Color(c.r, c.g, c.b, c.a * op).toRgba8888();
        if ((rgba & 0xff) == 0 && s.gradient == null) return;   // invisible unless a gradient fades in
        if (s instanceof Shape.Line) line((Shape.Line) s, rgba, batch);
        else if (s instanceof Shape.Circle) {
            Shape.Circle ci = (Shape.Circle) s;
            fan(ci.cx, ci.cy, ci.r, ci.start, ci.span, rgba, batch);
        } else if (s instanceof Shape.Ring) {
            Shape.Ring ri = (Shape.Ring) s;
            ring(ri, rgba, batch);
        } else if (s instanceof Shape.Polygon) {
            Shape.Polygon pg = (Shape.Polygon) s;
            float[] q = pg.points;
            if (pg.convex && s.gradient != null && s.gradient.radial) {
                // fan from the centre so the ramp has an interior vertex (a rim fan is flat)
                float cx = 0, cy = 0;
                int n = q.length / 2;
                for (int i = 0; i < n; i++) {
                    cx += q[i * 2];
                    cy += q[i * 2 + 1];
                }
                cx /= n;
                cy /= n;
                for (int i = 0; i < n; i++) {
                    int j = (i + 1) % n;
                    tri(batch, cx, cy, q[i * 2], q[i * 2 + 1], q[j * 2], q[j * 2 + 1], rgba);
                }
            } else if (pg.convex) {
                for (int i = 2; i + 3 < q.length; i += 2) tri(batch, q[0], q[1], q[i], q[i + 1], q[i + 2], q[i + 3], rgba);
            } else {
                int[] idx = EarClip.triangulate(q);
                for (int i = 0; i + 2 < idx.length; i += 3) {
                    tri(batch, q[idx[i] * 2], q[idx[i] * 2 + 1], q[idx[i + 1] * 2], q[idx[i + 1] * 2 + 1],
                            q[idx[i + 2] * 2], q[idx[i + 2] * 2 + 1], rgba);
                }
            }
        } else if (s instanceof Shape.Quad) {
            // corners BL, BR, TR, TL; image v runs downward
            float[] q = ((Shape.Quad) s).corners;
            // a parallelogram maps exactly with two triangles; a distorted
            // quad is split into a grid with bilinear corners (no diagonal kink)
            float ex = q[0] + q[4] - q[2] - q[6], ey = q[1] + q[5] - q[3] - q[7];
            float span = Math.abs(q[4] - q[0]) + Math.abs(q[5] - q[1]) + Math.abs(q[6] - q[2]) + Math.abs(q[7] - q[3]);
            int n = Math.abs(ex) + Math.abs(ey) <= 1e-4f * Math.max(1, span) ? 1 : 12;
            for (int j = 0; j < n; j++) {
                for (int i = 0; i < n; i++) {
                    float u0 = (float) i / n, u1 = (float) (i + 1) / n, v0 = (float) j / n, v1 = (float) (j + 1) / n;
                    quadVertex(batch, q, u0, v0, rgba);
                    quadVertex(batch, q, u1, v0, rgba);
                    quadVertex(batch, q, u1, v1, rgba);
                    quadVertex(batch, q, u0, v0, rgba);
                    quadVertex(batch, q, u1, v1, rgba);
                    quadVertex(batch, q, u0, v1, rgba);
                }
            }
        }
        if (s.gradient != null) recolor(batch, first, c, s.gradient, op);
    }

    /** Point at (u, v) of corners BL, BR, TR, TL (v up), bilinear; texture v runs downward. */
    private static void quadVertex(DrawList.Batch b, float[] q, float u, float v, int rgba) {
        float bx = q[0] + (q[2] - q[0]) * u, by = q[1] + (q[3] - q[1]) * u;
        float tx = q[6] + (q[4] - q[6]) * u, ty = q[7] + (q[5] - q[7]) * u;
        b.add(bx + (tx - bx) * v, by + (ty - by) * v, rgba, u, 1 - v);
    }

    /** Recomputes vertex colours [first, end) from a gradient (straight RGBA, group opacity applied). */
    private static void recolor(DrawList.Batch b, int first, Color c0, Shape.Gradient g, float op) {
        Color c1 = g.end;
        for (int i = first; i < b.vertices; i++) {
            float t = g.at(b.x(i), b.y(i));
            Color m = c0.lerp(c1, t);
            b.rgba[i] = new Color(m.r, m.g, m.b, m.a * op).toRgba8888();
        }
    }

    /** Segments for an arc of radius r and angle span, from the chord tolerance. */
    static int segments(float r, float span) {
        if (r <= TOLERANCE) return 3;
        double step = 2 * Math.acos(1 - TOLERANCE / r);
        int n = (int) Math.ceil(span / step);
        return Math.max(3, Math.min(n, 256));
    }

    private static void line(Shape.Line l, int c, DrawList.Batch b) {
        float hw = l.width / 2;
        if (hw <= 0) return;
        if (Math.abs(l.bend) > 0.01f) {
            arc(l, hw, c, b);
            return;
        }
        float dx = l.x2 - l.x1, dy = l.y2 - l.y1;
        float len = (float) Math.sqrt(dx * dx + dy * dy);
        if (len < 1e-6f) {
            fan(l.x1, l.y1, hw, 0, (float) (2 * Math.PI), c, b);
            return;
        }
        float nx = -dy / len * hw, ny = dx / len * hw;
        tri(b, l.x1 + nx, l.y1 + ny, l.x2 + nx, l.y2 + ny, l.x2 - nx, l.y2 - ny, c);
        tri(b, l.x1 + nx, l.y1 + ny, l.x2 - nx, l.y2 - ny, l.x1 - nx, l.y1 - ny, c);
        float a = (float) Math.atan2(ny, nx);
        fan(l.x1, l.y1, hw, a, (float) Math.PI, c, b);              // cap at start
        fan(l.x2, l.y2, hw, a + (float) Math.PI, (float) Math.PI, c, b);  // cap at end
    }

    /**
     * A thick circular arc from (x1,y1) to (x2,y2) turning `bend` degrees, as
     * one triangle strip (no overlaps, so translucent arcs blend once) plus
     * round caps. Positive bend bulges to the left of the travel direction.
     */
    private static void arc(Shape.Line l, float hw, int c, DrawList.Batch b) {
        double dx = l.x2 - l.x1, dy = l.y2 - l.y1, chord = Math.sqrt(dx * dx + dy * dy);
        if (chord < 1e-6) {
            fan(l.x1, l.y1, hw, 0, (float) (2 * Math.PI), c, b);
            return;
        }
        double theta = Math.toRadians(Math.max(-350, Math.min(350, l.bend)));
        double r = chord / (2 * Math.sin(Math.abs(theta) / 2));
        double mx = (l.x1 + l.x2) / 2, my = (l.y1 + l.y2) / 2;
        double nx = -dy / chord, ny = dx / chord;                    // left normal
        double h = r * Math.cos(theta / 2);                          // centre offset from the chord
        double sgn = theta > 0 ? -1 : 1;                             // centre is opposite the bulge
        double cx = mx + sgn * nx * h, cy = my + sgn * ny * h;
        double a0 = Math.atan2(l.y1 - cy, l.x1 - cx);
        // positive bend bulges left: the centre is on the right, so the sweep runs clockwise
        double sweep = theta > 0 ? -Math.abs(theta) : Math.abs(theta);
        int n = Math.max(6, Math.min(96, segments((float) (r + hw), (float) Math.abs(theta))));
        float px0 = 0, py0 = 0, px1 = 0, py1 = 0;
        for (int i = 0; i <= n; i++) {
            double a = a0 + sweep * i / n;
            double ux = Math.cos(a), uy = Math.sin(a);
            float ox = (float) (cx + (r + hw) * ux), oy = (float) (cy + (r + hw) * uy);
            float ix = (float) (cx + (r - hw) * ux), iy = (float) (cy + (r - hw) * uy);
            if (i > 0) {
                tri(b, px0, py0, ox, oy, ix, iy, c);
                tri(b, px0, py0, ix, iy, px1, py1, c);
            }
            px0 = ox;
            py0 = oy;
            px1 = ix;
            py1 = iy;
        }
        // caps: half discs facing away along the tangents
        double t0 = a0 + (sweep > 0 ? -Math.PI / 2 : Math.PI / 2);
        double a1 = a0 + sweep;
        double t1 = a1 + (sweep > 0 ? Math.PI / 2 : -Math.PI / 2);
        fan(l.x1, l.y1, hw, (float) (t0 - Math.PI / 2), (float) Math.PI, c, b);
        fan(l.x2, l.y2, hw, (float) (t1 - Math.PI / 2), (float) Math.PI, c, b);
    }

    /** Band between r - width/2 and r + width/2 (inner radius clamped at 0). */
    private static void ring(Shape.Ring ri, int c, DrawList.Batch b) {
        float hw = ri.width / 2;
        if (hw <= 0) return;
        float r0 = Math.max(0, ri.r - hw), r1 = ri.r + hw;
        if (ri.r <= 0 && r0 <= 0) {
            fan(ri.cx, ri.cy, r1, ri.start, ri.span, c, b);
            return;
        }
        int n = segments(r1, ri.span);
        for (int i = 0; i < n; i++) {
            double a0 = ri.start + ri.span * i / n, a1 = ri.start + ri.span * (i + 1) / n;
            float c0 = (float) Math.cos(a0), s0 = (float) Math.sin(a0), c1 = (float) Math.cos(a1), s1 = (float) Math.sin(a1);
            float ix0 = ri.cx + r0 * c0, iy0 = ri.cy + r0 * s0, ox0 = ri.cx + r1 * c0, oy0 = ri.cy + r1 * s0;
            float ix1 = ri.cx + r0 * c1, iy1 = ri.cy + r0 * s1, ox1 = ri.cx + r1 * c1, oy1 = ri.cy + r1 * s1;
            tri(b, ix0, iy0, ox0, oy0, ox1, oy1, c);
            tri(b, ix0, iy0, ox1, oy1, ix1, iy1, c);
        }
    }

    private static void fan(float cx, float cy, float r, float start, float span, int c, DrawList.Batch b) {
        if (r <= 0) return;
        int n = segments(r, span);
        float px = cx + r * (float) Math.cos(start), py = cy + r * (float) Math.sin(start);
        for (int i = 1; i <= n; i++) {
            float a = start + span * i / n;
            float x = cx + r * (float) Math.cos(a), y = cy + r * (float) Math.sin(a);
            tri(b, cx, cy, px, py, x, y, c);
            px = x;
            py = y;
        }
    }

    private static void tri(DrawList.Batch b, float x1, float y1, float x2, float y2, float x3, float y3, int c) {
        b.add(x1, y1, c);
        b.add(x2, y2, c);
        b.add(x3, y3, c);
    }
}
