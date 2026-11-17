package app.barkfart;

import app.barkfart.demo.Demo;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;
import app.barkfart.selftest.SelfTest;
import app.barkfart.ui.EditorScreen;
import com.badlogic.gdx.ApplicationAdapter;
import com.badlogic.gdx.Gdx;

/** Barkfart: opens straight into the editor with a new project (UI-SPEC "Missing states"). */
public class BarkfartApp extends ApplicationAdapter {
    public static final String VERSION = "0.1.0-export";

    private final float uiScale;
    private final app.barkfart.platform.Platform platform;
    private EditorScreen editor;

    /** @param uiScale logical screen units per UI point (1 on iOS). */
    public BarkfartApp(app.barkfart.platform.Platform platform, float uiScale) {
        this.platform = platform;
        this.uiScale = uiScale;
    }

    /** New project: one frame, one default figure centred. */
    public static Project newProject() {
        Project p = new Project();
        Symbol root = p.addSymbol(new Symbol(p.newId("S"), "Untitled", 1.0 / 30));
        p.rootSymbol = root.id;
        Layer.FigureLayer man = Demo.stickman(p, "Figure 1");
        man.transform.y.setValue(-40f);
        man.transform.scaleX.setValue(2f);
        man.transform.scaleY.setValue(2f);
        root.layers.add(man);
        p.invalidateIndex();
        return p;
    }

    @Override
    public void create() {
        editor = new EditorScreen(platform, uiScale);
        editor.state.diagnosticsRunner = new Runnable() {
            public void run() {
                runDiagnostics(false);
            }
        };
        runDiagnostics(true);
    }

    // The video check finishes on its own schedule; it joins the results whichever arrives last.
    private String videoResult;
    private long videoMs;
    private boolean videoDone;
    private SelfTest.Result videoAddedTo;

    private void addVideoResult() {
        SelfTest.Result r = editor.state.diagnostics;
        if (!videoDone || r == null || videoAddedTo == r) return;
        videoAddedTo = r;
        if (platform.canEncodeVideo()) r.add("Export", "video (MP4 with sound) export", videoResult, videoMs);
        editor.state.fire();
        if (videoResult != null && platform.canEncodeVideo()) {
            editor.state.flash("Video export check FAILED · tap for Diagnostics", editor.state.diagnosticsOpener, 12);
        }
    }

    /**
     * Every check, on this device: core and feature checks and the audio output
     * on a worker thread, the GPU effects check here (GL thread). Results go to
     * the Diagnostics screen; at launch a summary shows (tap it for details).
     */
    private void runDiagnostics(final boolean atLaunch) {
        long g0 = System.nanoTime();
        final String fx = app.barkfart.render.EffectCheck.run();
        final long fxMs = (System.nanoTime() - g0) / 1000000;
        long e0 = System.nanoTime();
        final String files = app.barkfart.render.ExportCheck.files(platform.documents());
        final long filesMs = (System.nanoTime() - e0) / 1000000;
        final long v0 = System.nanoTime();
        videoResult = null;
        videoDone = false;
        app.barkfart.render.ExportCheck.video(platform, platform.documents(), new app.barkfart.render.ExportCheck.Result() {
            public void result(final String problem) {
                Gdx.app.postRunnable(new Runnable() {
                    public void run() {
                        videoResult = problem;
                        videoMs = (System.nanoTime() - v0) / 1000000;
                        videoDone = true;
                        addVideoResult();
                    }
                });
            }
        });
        System.out.println("[Barkfart] effects " + (fx == null ? "OK" : fx));
        Thread st = new Thread(new Runnable() {
            public void run() {
                final SelfTest.Result r = SelfTest.run(platform.documents());
                long a0 = System.nanoTime();
                final String au = platform.audio().check();   // opens the output, plays silence, waits for it
                r.add("Device", "GPU draws effects like the reference", fx, fxMs);
                r.add("Export", "GIF and PNG frames export", files, filesMs);
                r.add("Device", "audio output plays", au, (System.nanoTime() - a0) / 1000000);
                System.out.println("[Barkfart] self-test " + r.summary());
                for (String f : r.failures) System.out.println("[Barkfart] FAIL " + f);
                Gdx.app.postRunnable(new Runnable() {
                    public void run() {
                        editor.state.diagnostics = r;
                        videoAddedTo = null;
                        addVideoResult();
                        if (atLaunch || !r.ok()) {
                            int failed = r.entries.size() - r.passed;
                            editor.state.flash((failed == 0 ? "All " + r.entries.size() + " checks passed" : failed + " of " + r.entries.size() + " checks FAILED")
                                    + " · tap for Diagnostics · v" + VERSION, editor.state.diagnosticsOpener, 12);
                        }
                        editor.state.fire();
                    }
                });
            }
        }, "self-test");
        st.setDaemon(true);
        st.start();
    }

    /** For scripted desktop screenshots: the editor's state. */
    public app.barkfart.ui.EditorState state() {
        return editor.state;
    }

    @Override
    public void resize(int width, int height) {
        editor.resize(width, height);
    }

    @Override
    public void render() {
        editor.render(Math.min(Gdx.graphics.getDeltaTime(), 0.1f));
    }

    @Override
    public void pause() {
        editor.pause();
    }

    @Override
    public void dispose() {
        editor.pause();
        editor.dispose();
    }
}
