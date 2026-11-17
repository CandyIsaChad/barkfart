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
    /** Opens the File panel (set by the screen). */
    public Runnable fileOpener;
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

    /** Latest results of every check (null while running); opener and runner set by the screen / app. */
    public app.barkfart.selftest.SelfTest.Result diagnostics;
    public Runnable diagnosticsOpener, diagnosticsRunner, exportOpener;

    /** Sound output (set by the screen from the platform). */
    public app.barkfart.audio.AudioBackend audio = app.barkfart.audio.AudioBackend.NONE;
    private final java.util.Map<String, app.barkfart.audio.Pcm> soundCache = new java.util.HashMap<String, app.barkfart.audio.Pcm>();
    private final java.util.Set<String> undecodable = new java.util.HashSet<String>();
    private Project soundCacheFor;

    /** Decoded sounds of the open project (cached; undecodable ones remembered). */
    public app.barkfart.audio.AudioPlan.Sounds sounds() {
        final Project p = project();
        if (p != soundCacheFor) {
            soundCache.clear();
            undecodable.clear();
            soundCacheFor = p;
        }
        return new app.barkfart.audio.AudioPlan.Sounds() {
            public app.barkfart.audio.Pcm pcm(String id) {
                app.barkfart.audio.Pcm c = soundCache.get(id);
                if (c != null || undecodable.contains(id)) return c;
                app.barkfart.model.Asset a = p.assets.get(id);
                try {
                    byte[] b = a == null ? null : p.assetData.bytes(id);
                    c = b == null ? null : audio.decode(a.path, b);
                } catch (java.io.IOException e) {
                    c = null;
                }
                if (c == null) undecodable.add(id);
                else soundCache.put(id, c);
                return c;
            }
        };
    }

    /** Playback range in frames (inclusive), or -1 for the whole timeline (Stick Nodes 4.1.6). */
    public int rangeStart = -1, rangeEnd = -1;

    /** The range played, in seconds: {start, end}. Also used by export. */
    public double[] playRange() {
        int n = frames.frameCount();
        int a = rangeStart < 0 ? 0 : Math.min(rangeStart, n - 1);
        int b = rangeEnd < 0 ? n - 1 : Math.max(a, Math.min(rangeEnd, n - 1));
        return new double[] {frames.time(a), frames.time(b + 1)};
    }

    public void play() {
        if (playing) return;
        playStartFrame = frame;
        double[] range = playRange();
        playTime = frames.time(frame);
        if (playTime < range[0] || playTime >= range[1]) playTime = range[0];
        playing = true;
        app.barkfart.audio.AudioPlan plan = app.barkfart.audio.AudioPlan.build(project(), project().root(), sounds());
        if (!plan.entries.isEmpty()) {
            String err = audio.play(plan, playTime, range[0], range[1]);
            if (err != null) flash("Sound off: " + err, null, 6);
        }
        fire();
    }

    public void stop() {
        if (!playing) return;
        playing = false;
        audio.stop();
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
        double[] range = playRange();
        playTime += dt;
        if (playTime >= range[1] || playTime < range[0]) playTime = range[0] + Math.max(0, playTime - range[0]) % Math.max(1e-6, range[1] - range[0]);
    }

    public int playingFrame() {
        return frames.frameAt(playTime);
    }

    /** Shows a transient status message (4 s); action runs if the user taps it. */
    public void flash(String text, Runnable action) {
        flash(text, action, 4);
    }

    public void flash(String text, Runnable action, float seconds) {
        message = text;
        messageAction = action;
        messageSeconds = seconds;
        fire();
    }

    public String message() {
        return message;
    }

    public Runnable messageAction() {
        return messageAction;
    }
}
