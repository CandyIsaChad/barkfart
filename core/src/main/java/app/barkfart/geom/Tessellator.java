package app.barkfart.geom;

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

    public static DrawList tessellate(Scene scene) {
        DrawList out = new DrawList();
        walk(scene.root, 1f, Layer.Blend.NORMAL, null, out);
        return out;
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
            Shape s = (Shape) o;
            DrawList.Batch batch = s instanceof Shape.Quad ? out.batch(b, effects, ((Shape.Quad) s).asset) : out.batch(b, effects);
            int first = batch.vertices;
            Color c = s.color;
            int rgba = new Color(c.r, c.g, c.b, c.a * op).toRgba8888();
            if ((rgba & 0xff) == 0 && s.gradient == null) continue;   // invisible unless a gradient fades in
            if (s instanceof Shape.Line) line((Shape.Line) s, rgba, batch);
            else if (s instanceof Shape.Circle) {
                Shape.Circle ci = (Shape.Circle) s;
                fan(ci.cx, ci.cy, ci.r, 0, (float) (2 * Math.PI), rgba, batch);
            } else if (s instanceof Shape.Polygon) {
                Shape.Polygon pg = (Shape.Polygon) s;
                float[] q = pg.points;
                if (pg.convex) {
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
                batch.add(q[0], q[1], rgba, 0, 1);
                batch.add(q[2], q[3], rgba, 1, 1);
                batch.add(q[4], q[5], rgba, 1, 0);
                batch.add(q[0], q[1], rgba, 0, 1);
                batch.add(q[4], q[5], rgba, 1, 0);
                batch.add(q[6], q[7], rgba, 0, 0);
            }
            if (s.gradient != null) recolor(batch, first, c, s.gradient, op);
        }
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
