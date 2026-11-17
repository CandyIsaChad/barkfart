package app.barkfart.model;

import java.util.ArrayList;
import java.util.List;

/** Base layer. Subclasses hold the content for each layer type. */
public abstract class Layer {
    public enum Type { FIGURE, SPRITE, TEXT, SYMBOL, ADJUSTMENT, AUDIO, CAMERA }

    public enum Blend { NORMAL, ADD, MULTIPLY, SCREEN }

    public final String id;
    public String name;
    public boolean visible = true, locked = false, solo = false;
    /** Drawn in onion skin (turn off for backgrounds that hide the ghosts). */
    public boolean onion = true;
    public final Property shown;
    public final Property opacity;
    public Blend blend = Blend.NORMAL;
    /** Parent layer id for transform inheritance, or null. */
    public String parent;
    /**
     * Joining: with a figure parent, the node this layer hangs from (its joint
     * position, and its angle when parentNodeRotate), or null for the figure itself.
     */
    public String parentNode;
    public boolean parentNodeRotate = true;
    /** Also apply the parent layer's effects (Stick Nodes 3.1.0 "use parent's filters"), before its own. */
    public boolean inheritEffects;
    /**
     * For layers locked to the camera (Stick Nodes 2.3.7 / 2.4.4 parallax):
     * 0 stays fixed in the shot, 1 moves with the world, between follows the
     * camera's move, turn and zoom by that fraction (distant backgrounds).
     */
    public float parallax;
    public final Transform transform;
    public final EffectStack effects = new EffectStack();

    protected Layer(String id, String name) {
        this.id = id;
        this.name = name;
        shown = new Property(id + "/shown", ValueType.BOOL, Boolean.TRUE);
        opacity = new Property(id + "/opacity", ValueType.FLOAT, 1f);
        transform = new Transform(id);
    }

    public abstract Type type();

    /** All animatable properties owned by this layer (not nested content). */
    public List<Property> properties() {
        ArrayList<Property> out = new ArrayList<Property>();
        out.add(shown);
        out.add(opacity);
        for (Property p : transform.all()) out.add(p);
        for (EffectNode e : effects.nodes.values()) {
            out.add(e.mix);
            out.addAll(e.params.values());
        }
        return out;
    }

    public static final class FigureLayer extends Layer {
        public final Figure figure = new Figure();

        public FigureLayer(String id, String name) {
            super(id, name);
        }

        @Override
        public Type type() {
            return Type.FIGURE;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            for (Node n : figure.nodes()) for (Property p : n.properties()) out.add(p);
            for (Polyfill pf : figure.polyfills) out.add(pf.color);
            return out;
        }
    }

    public static final class SpriteLayer extends Layer {
        public String asset;
        public float width, height;
        public final Property tint;
        /**
         * Distort (Stick Nodes 4.3.0): offsets of the corners bottom-left,
         * bottom-right, top-right, top-left as x, y pairs in the sprite's own
         * units; all zero is the plain rectangle. Keyable.
         */
        public final Property[] distort = new Property[8];

        public SpriteLayer(String id, String name) {
            super(id, name);
            tint = new Property(id + "/tint", ValueType.COLOR, Color.WHITE);
            for (int i = 0; i < 8; i++) distort[i] = new Property(id + "/distort/" + (i / 2) + (i % 2 == 0 ? "x" : "y"), ValueType.FLOAT, 0f);
        }

        public boolean distorted() {
            for (Property q : distort) if (!q.track().isEmpty() || q.floatValue() != 0) return true;
            return false;
        }

        @Override
        public Type type() {
            return Type.SPRITE;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            out.add(tint);
            for (Property q : distort) out.add(q);
            return out;
        }
    }

    public static final class TextLayer extends Layer {
        public String text = "";
        public String font;
        public float size = 32;
        public final Property color;
        /**
         * Which point of the text block sits at the layer origin: 0 centre,
         * else a corner (Stick Nodes 2.2.0 lock to corner): lines align to
         * that side.
         */
        public int corner;
        public static final int CENTRE = 0, TOP_LEFT = 1, TOP_RIGHT = 2, BOTTOM_LEFT = 3, BOTTOM_RIGHT = 4;
        /** Drawn over the widescreen bars (Stick Nodes 3.1.3), e.g. a watermark. */
        public boolean aboveBars;

        public TextLayer(String id, String name) {
            super(id, name);
            color = new Property(id + "/color", ValueType.COLOR, Color.BLACK);
        }

        @Override
        public Type type() {
            return Type.TEXT;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            out.add(color);
            return out;
        }
    }

    public static final class SymbolLayer extends Layer {
        public enum LoopMode { ONCE, LOOP, PING_PONG }

        public String symbol;
        public final Property timeOffset, speed;
        public LoopMode loop = LoopMode.LOOP;
        public int recursionLimit = 8;

        public SymbolLayer(String id, String name, String symbol) {
            super(id, name);
            this.symbol = symbol;
            timeOffset = new Property(id + "/timeOffset", ValueType.DOUBLE, 0.0);
            speed = new Property(id + "/speed", ValueType.DOUBLE, 1.0);
        }

        @Override
        public Type type() {
            return Type.SYMBOL;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            out.add(timeOffset);
            out.add(speed);
            return out;
        }
    }

    public static final class AdjustmentLayer extends Layer {
        public AdjustmentLayer(String id, String name) {
            super(id, name);
        }

        @Override
        public Type type() {
            return Type.ADJUSTMENT;
        }
    }

    public static final class AudioLayer extends Layer {
        public final List<Clip> clips = new ArrayList<Clip>();

        public AudioLayer(String id, String name) {
            super(id, name);
        }

        @Override
        public Type type() {
            return Type.AUDIO;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            for (Clip c : clips) {
                out.add(c.volume);
                out.add(c.pan);
            }
            return out;
        }
    }

    public static final class CameraLayer extends Layer {
        public final Property x, y, zoom, rotation;
        /**
         * Shot background (Stick Nodes 1.0 frame background, 2.0.0 gradient):
         * transparent = the project background; with background2 opaque, a
         * gradient from background at the top to background2 at the bottom.
         */
        public final Property background, background2;

        public CameraLayer(String id, String name) {
            super(id, name);
            background = new Property(id + "/cam/bg", ValueType.COLOR, new Color(0, 0, 0, 0));
            background2 = new Property(id + "/cam/bg2", ValueType.COLOR, new Color(0, 0, 0, 0));
            x = new Property(id + "/cam/x", ValueType.FLOAT, 0f);
            y = new Property(id + "/cam/y", ValueType.FLOAT, 0f);
            zoom = new Property(id + "/cam/zoom", ValueType.FLOAT, 1f);
            rotation = new Property(id + "/cam/rotation", ValueType.ANGLE, 0f);
        }

        @Override
        public Type type() {
            return Type.CAMERA;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            out.add(x);
            out.add(y);
            out.add(zoom);
            out.add(rotation);
            out.add(background);
            out.add(background2);
            return out;
        }
    }
}
