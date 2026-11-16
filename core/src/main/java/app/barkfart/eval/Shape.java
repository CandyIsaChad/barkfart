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

    /** A segment with round caps. */
    public static final class Line extends Shape {
        public final float x1, y1, x2, y2, width;

        public Line(float x1, float y1, float x2, float y2, float width, Color color, String source) {
            super(color, source);
            this.x1 = x1;
            this.y1 = y1;
            this.x2 = x2;
            this.y2 = y2;
            this.width = width;
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
