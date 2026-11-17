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

        public SpriteLayer(String id, String name) {
            super(id, name);
            tint = new Property(id + "/tint", ValueType.COLOR, Color.WHITE);
        }

        @Override
        public Type type() {
            return Type.SPRITE;
        }

        @Override
        public List<Property> properties() {
            List<Property> out = super.properties();
            out.add(tint);
            return out;
        }
    }

    public static final class TextLayer extends Layer {
        public String text = "";
        public String font;
        public float size = 32;
        public final Property color;

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

        public CameraLayer(String id, String name) {
            super(id, name);
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
            return out;
        }
    }
}
