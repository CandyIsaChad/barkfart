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
            DrawList.Batch batch = out.batch(b, effects);
            Color c = s.color;
            int rgba = new Color(c.r, c.g, c.b, c.a * op).toRgba8888();
            if ((rgba & 0xff) == 0) continue;
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
                float[] q = ((Shape.Quad) s).corners;
                tri(batch, q[0], q[1], q[2], q[3], q[4], q[5], rgba);
                tri(batch, q[0], q[1], q[4], q[5], q[6], q[7], rgba);
            }
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
