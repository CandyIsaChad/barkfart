package app.barkfart.eval;

import app.barkfart.model.Color;

/** A drawable primitive in view space (camera applied). y is up, units are project pixels. */
public abstract class Shape {
    public final Color color;
    /** Source path for picking and debugging, e.g. "L1/n3". */
    public final String source;
    /** Optional gradient from color to Gradient.end (vertex colours are recomputed from it). */
    public Gradient gradient;

    /** Linear (from p0 to p1) or radial (centre p0, radius r) colour ramp, view space. */
    public static final class Gradient {
        public final boolean radial;
        /** Linear: ramp from (x0, y0) to (x1, y1). Radial: centre (x0, y0), (x1, y1) the end of the first axis. */
        public final float x0, y0, x1, y1;
        /** Radial: radius across the first axis. */
        public final float r2;
        public final Color end;

        private Gradient(boolean radial, float x0, float y0, float x1, float y1, float r2, Color end) {
            this.radial = radial;
            this.x0 = x0;
            this.y0 = y0;
            this.x1 = x1;
            this.y1 = y1;
            this.r2 = r2;
            this.end = end;
        }

        public static Gradient linear(float x0, float y0, float x1, float y1, Color end) {
            return new Gradient(false, x0, y0, x1, y1, 0, end);
        }

        public static Gradient radial(float cx, float cy, float r, Color end) {
            return new Gradient(true, cx, cy, cx + r, cy, r, end);
        }

        /** Elliptical: (ax, ay) is the vector from the centre to the end of the first axis; rb the other radius. */
        public static Gradient elliptical(float cx, float cy, float ax, float ay, float rb, Color end) {
            return new Gradient(true, cx, cy, cx + ax, cy + ay, rb, end);
        }

        /** Ramp position 0..1 at a point. */
        public float at(float x, float y) {
            float t;
            float dx = x1 - x0, dy = y1 - y0, l2 = dx * dx + dy * dy;
            if (radial) {
                if (l2 <= 0 || r2 <= 0) {
                    t = 0;
                } else {
                    double u = ((x - x0) * dx + (y - y0) * dy) / l2;                       // along, in first-axis radii
                    double v = (-(x - x0) * dy + (y - y0) * dx) / Math.sqrt(l2) / r2;      // across, in second-axis radii
                    t = (float) Math.hypot(u, v);
                }
            } else {
                t = l2 <= 0 ? 0 : ((x - x0) * dx + (y - y0) * dy) / l2;
            }
            return t < 0 ? 0 : (t > 1 ? 1 : t);
        }
    }

    private Shape(Color color, String source) {
        this.color = color;
        this.source = source;
    }

    /**
     * Bulge height (sagitta) of a curved segment per unit of Node.curve, in
     * figure units. Fitted against the original's movieclip previews
     * (NOTES.md): Sparks IoU 0.82 -> 0.91, Hit 0.81 -> 0.85; positive bulges
     * to the left of the parent-to-node direction.
     */
    public static final float CURVE_SAGITTA = 0.9f;

    /** A segment with round caps; bend (degrees, + = counter-clockwise bulge) makes it a circular arc. */
    public static final class Line extends Shape {
        public final float x1, y1, x2, y2, width, bend;

        public Line(float x1, float y1, float x2, float y2, float width, Color color, String source) {
            this(x1, y1, x2, y2, width, 0, color, source);
        }

        public Line(float x1, float y1, float x2, float y2, float width, float bend, Color color, String source) {
            super(color, source);
            this.x1 = x1;
            this.y1 = y1;
            this.x2 = x2;
            this.y2 = y2;
            this.width = width;
            this.bend = bend;
        }
    }

    /** A filled circle. */
    public static final class Circle extends Shape {
        public final float cx, cy, r;
        /** Filled sector from start over span (radians); a full circle by default. */
        public final float start, span;

        public Circle(float cx, float cy, float r, Color color, String source) {
            this(cx, cy, r, 0, (float) (2 * Math.PI), color, source);
        }

        public Circle(float cx, float cy, float r, float start, float span, Color color, String source) {
            super(color, source);
            this.cx = cx;
            this.cy = cy;
            this.r = r;
            this.start = start;
            this.span = span;
        }
    }

    /** A circular band of the given width centred on radius r, from start over span (radians). */
    public static final class Ring extends Shape {
        public final float cx, cy, r, width, start, span;

        public Ring(float cx, float cy, float r, float width, float start, float span, Color color, String source) {
            super(color, source);
            this.cx = cx;
            this.cy = cy;
            this.r = r;
            this.width = width;
            this.start = start;
            this.span = span;
        }
    }

    /** A filled polygon, points as x0,y0,x1,y1,... in either winding. */
    public static final class Polygon extends Shape {
        public final float[] points;
        /** True when known convex (fan triangulation); false means ear clipping. */
        public final boolean convex;

        public Polygon(float[] points, Color color, String source) {
            this(points, color, source, true);
        }

        public Polygon(float[] points, Color color, String source, boolean convex) {
            super(color, source);
            this.points = points;
            this.convex = convex;
        }
    }

    /** A textured quad (sprites). Corners in order: bottom-left, bottom-right, top-right, top-left. */
    public static final class Quad extends Shape {
        public final String asset;
        public final float[] corners;

        public Quad(String asset, float[] corners, Color tint, String source) {
            super(tint, source);
            this.asset = asset;
            this.corners = corners;
        }
    }

    /**
     * Text, laid out by the tessellator from a glyph source: lines centred on
     * the layer origin, size = line height in layer units, mapped by xf.
     */
    public static final class Text extends Shape {
        public final String text, font;
        public final float size;
        public final Affine xf;

        public Text(String text, String font, float size, Affine xf, Color color, String source) {
            super(color, source);
            this.text = text;
            this.font = font;
            this.size = size;
            this.xf = xf;
        }
    }
}
