package app.barkfart.eval;

import app.barkfart.model.Color;

/** A drawable primitive in view space (camera applied). y is up, units are project pixels. */
public abstract class Shape {
    public final Color color;
    /** Source path for picking and debugging, e.g. "L1/n3". */
    public final String source;

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

        public Circle(float cx, float cy, float r, Color color, String source) {
            super(color, source);
            this.cx = cx;
            this.cy = cy;
            this.r = r;
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
}
