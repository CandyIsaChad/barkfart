package app.barkfart.ui;

import app.barkfart.canvas.CanvasTools;
import app.barkfart.edit.Document;
import app.barkfart.edit.Editor;
import app.barkfart.edit.FrameOps;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import java.util.ArrayList;
import java.util.List;

/** Everything the editor panels share: document, selection, mode, frame, playback, view. */
public final class EditorState {
    public enum Mode { FIGURE, ALL, CAMERA }

    public final Document doc;
    public final Editor editor;
    public final FrameOps frames;
    public final CanvasTools tools;

    public Mode mode = Mode.FIGURE;
    public int frame;
    public Layer.FigureLayer selectedLayer;
    public Node selectedNode;
    public boolean onion = true;
    /** Look through the camera while editing (handles hidden). */
    public boolean previewCamera;

    // Dope sheet
    public final java.util.LinkedHashSet<app.barkfart.edit.KeyRef> selectedKeys = new java.util.LinkedHashSet<app.barkfart.edit.KeyRef>();
    public final java.util.HashSet<String> collapsedFigures = new java.util.HashSet<String>();
    public boolean sheetShowAll;
    /** Sheet height in points (72 = collapsed). */
    public float sheetHeight = 72;
    public app.barkfart.edit.Editor.Clip keyClipboard;
    /** Multi-select mode in the dope sheet (double-tap toggles): taps add/remove keys. */
    public boolean multiSelect;
    /** Opens the easing dropdown at a stage position (set by the screen). */
    public EasingOpener easingOpener;

    public interface EasingOpener {
        void open(float stageX, float stageY, EasingMenu.Preset current, EasingMenu.Chosen chosen);
    }

    public boolean playing;
    public double playTime;
    private int playStartFrame;

    /** View (not the camera layer): pan in points, user zoom multiplier on top of fit. */
    public float panX, panY, zoom = 1;

    private String message;
    private float messageSeconds;
    private Runnable messageAction;

    private final List<Runnable> listeners = new ArrayList<Runnable>();

    public EditorState(Project p) {
        doc = new Document(p);
        editor = new Editor(doc);
        editor.autoKey = true;   // always on (UI-SPEC: no toggle)
        frames = new FrameOps(editor);
        tools = new CanvasTools(editor);
        doc.addListener(new Document.Listener() {
            public void changed(Document d) {
                validateSelection();
                fire();
            }
        });
    }

    public Project project() {
        return doc.project;
    }

    /** Time being shown: the playhead while playing, else the current frame. */
    public double time() {
        return playing ? playTime : frames.time(frame);
    }

    public void addListener(Runnable r) {
        listeners.add(r);
    }

    /** Drops UI listeners (the UI is being rebuilt, e.g. at a new size). */
    public void clearListeners() {
        listeners.clear();
    }

    /** User UI size multiplier on top of the automatic size (0.8 .. 1.8), and how to apply a change. */
    public float uiUserScale = 1f;
    public Runnable onUiScaleChanged;

    public void fire() {
        for (Runnable r : listeners) r.run();
    }

    public void setFrame(int f) {
        frame = Math.max(0, Math.min(frames.frameCount() - 1, f));
        fire();
    }

    public void select(Layer.FigureLayer layer, Node node) {
        selectedLayer = layer;
        selectedNode = node;
        fire();
    }

    public List<Layer.FigureLayer> figures() {
        List<Layer.FigureLayer> out = new ArrayList<Layer.FigureLayer>();
        for (Layer l : project().root().layers) if (l instanceof Layer.FigureLayer) out.add((Layer.FigureLayer) l);
        return out;
    }

    /** Steps the selected figure through the figure layers (wraps). */
    public void cycleFigure(int dir) {
        List<Layer.FigureLayer> fs = figures();
        if (fs.isEmpty()) return;
        int i = selectedLayer == null ? -1 : fs.indexOf(selectedLayer);
        i = ((i + dir) % fs.size() + fs.size()) % fs.size();
        select(fs.get(i), null);
    }

    private void validateSelection() {
        if (selectedLayer != null && !project().root().layers.contains(selectedLayer)) {
            selectedLayer = null;
            selectedNode = null;
        }
        if (selectedNode != null && (selectedLayer == null || selectedLayer.figure.node(selectedNode.id) != selectedNode)) {
            selectedNode = null;
        }
        if (frame >= frames.frameCount()) frame = frames.frameCount() - 1;
        // drop selected keys that no longer exist (after undo, delete, retime)
        java.util.Iterator<app.barkfart.edit.KeyRef> it = selectedKeys.iterator();
        while (it.hasNext()) {
            app.barkfart.edit.KeyRef k = it.next();
            if (k.property.track().indexOf(k.time) < 0) it.remove();
        }
    }

    public void selectKeys(java.util.Collection<app.barkfart.edit.KeyRef> keys, boolean add) {
        if (!add) selectedKeys.clear();
        selectedKeys.addAll(keys);
        fire();
    }

    public void play() {
        if (playing) return;
        playStartFrame = frame;
        playTime = frames.time(frame);
        playing = true;
        fire();
    }

    public void stop() {
        if (!playing) return;
        playing = false;
        frame = playStartFrame;
        fire();
    }

    /** Advances playback; loops over the symbol's duration. */
    public void update(float dt) {
        if (messageSeconds > 0 && (messageSeconds -= dt) <= 0) {
            message = null;
            messageAction = null;
            fire();
        }
        if (!playing) return;
        double dur = frames.frameCount() / editor.fps();
        playTime += dt;
        if (playTime >= dur) playTime %= dur;
    }

    public int playingFrame() {
        return frames.frameAt(playTime);
    }

    /** Shows a transient status message (4 s); action runs if the user taps it. */
    public void flash(String text, Runnable action) {
        message = text;
        messageAction = action;
        messageSeconds = 4;
        fire();
    }

    public String message() {
        return message;
    }

    public Runnable messageAction() {
        return messageAction;
    }
}
