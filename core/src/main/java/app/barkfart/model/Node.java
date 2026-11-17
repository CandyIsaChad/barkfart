package app.barkfart.model;

import java.util.LinkedHashMap;
import java.util.Map;

/** One node of a figure: a joint plus the shape drawn from its parent to it. */
public final class Node {
    public enum Shape {
        NONE,
        /** Segment with round ends. */
        LINE,
        /** Filled circle spanning the segment (diameter = segment length). */
        CIRCLE,
        /** Segment with square ends. */
        SQUARE_LINE,
        /** Ellipse spanning the segment; width = thickness. */
        ELLIPSE,
        /** Isosceles triangle: base of width = thickness at the parent, apex at the node. */
        TRIANGLE,
        /** Quad of width = thickness at the parent and thickness x trapezoidRatio at the node. */
        TRAPEZOID,
        /** Regular polygon (polygonSides) inscribed in the circle spanning the segment, a corner at the node. */
        POLYGON
    }

    /** Angle lock: what the rotation is measured against. */
    public static final int LOCK_NONE = 0, LOCK_ABSOLUTE = 1, LOCK_RELATIVE = 2;
    /** Drag lock: the axis a drag of this joint is held to. */
    public static final int DRAG_FREE = 0, DRAG_HORIZONTAL = 1, DRAG_VERTICAL = 2;

    public final String id;
    /** Parent node id, or null for the root. Parents precede children in Figure.nodes. */
    public String parent;   // changed only through Figure (setNodes / reparent keep parents first)
    public Shape shape;
    /** drawOrder value meaning "not set": Figure.add then puts the node in front. */
    public static final int UNASSIGNED = Integer.MIN_VALUE;
    /** Draw order within the figure: lower draws behind. Independent of hierarchy. */
    public int drawOrder = UNASSIGNED;
    public final Property rotation, length, thickness, scale, color, shown;
    /**
     * Bend of the segment (legacy "curve radius" units; 0 = straight): a
     * circular arc whose bulge height is curve x Shape.CURVE_SAGITTA.
     */
    public float curve;
    /** Triangle options (legacy): apex at the parent instead of the node; right-angled to one side (-1 / +1). */
    public boolean triangleUpsideDown;
    public int rightTriangle;
    /** Gradient from color to gradientColor: along the segment, across it (sideways), or radial for round shapes. */
    public boolean gradient, gradientReverse, gradientSideways;
    /**
     * Circles, polygons and ellipses (Stick Nodes 4.2.4): a linear gradient
     * instead of the radial one: along the segment, or across it with sideways.
     */
    public boolean gradientLinear;
    public final Property gradientColor;
    /** A detail node: drawn, but not dragged while animating; moves only with its parent. */
    public boolean isStatic;
    /** Its length may change while animating. */
    public boolean stretchy;
    /**
     * Stretching this node reshapes its static details: each keeps its angle and
     * its length follows the stretch along this node's direction (NOTES.md).
     */
    public boolean smartStretch;
    /** Smart-stretch leaves this node and its branch alone. */
    public boolean noSmartStretch;
    /** Length smart-stretch measures from; 0 = not set (the length at the start of a drag). */
    public float restLength;
    /** Circles: no fill; outline ring of width = thickness in outlineColor. */
    public boolean circleHollow, circleOutline;
    public final Property outlineColor;
    /** Circles: only the half on one side of the segment (1 = left, -1 = right, 0 = whole). */
    public int halfArc;
    /** Polygon corner count (4-16); 0 = 5. */
    public int polygonSides;
    /** Trapezoid width at the node as a fraction of the width at the parent; round ends. */
    public float trapezoidRatio = 1f;
    public boolean trapezoidRoundStart, trapezoidRoundEnd;
    /** LOCK_ABSOLUTE: rotation is the world angle; LOCK_RELATIVE: relative to the nearest non-static ancestor. */
    public int angleLock;
    /** DRAG_HORIZONTAL / DRAG_VERTICAL: drags of this joint move along one stage axis only. */
    public int dragLock;
    /**
     * Pinned (Stick Nodes 2.4.8 individual node lock): while other joints or
     * the whole figure are dragged, this joint keeps its place on the stage
     * (its own and its parent's angles turn to reach it, like a planted foot).
     */
    public boolean pinned;
    /** Legacy fields not yet understood, kept for faithful re-export. */
    public final Map<String, Object> legacyExtras = new LinkedHashMap<String, Object>();

    public Node(String layerId, String id, String parent, Shape shape) {
        this.id = id;
        this.parent = parent;
        this.shape = shape;
        String p = layerId + "/" + id + "/";
        rotation = new Property(p + "rotation", ValueType.ANGLE, 0f);
        length = new Property(p + "length", ValueType.FLOAT, 0f);
        thickness = new Property(p + "thickness", ValueType.FLOAT, 10f);
        scale = new Property(p + "scale", ValueType.FLOAT, 1f);
        color = new Property(p + "color", ValueType.COLOR, Color.BLACK);
        shown = new Property(p + "shown", ValueType.BOOL, Boolean.TRUE);
        gradientColor = new Property(p + "gradientColor", ValueType.COLOR, Color.WHITE);
        outlineColor = new Property(p + "outlineColor", ValueType.COLOR, Color.BLACK);
    }

    public int sides() {
        return polygonSides <= 0 ? 5 : Math.max(3, Math.min(16, polygonSides));
    }

    public Property[] properties() {
        return new Property[] {rotation, length, thickness, scale, color, shown, gradientColor, outlineColor};
    }
}
