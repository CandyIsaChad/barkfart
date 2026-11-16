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
    public static final String VERSION = "0.1.0-m4ui";

    private final float uiScale;
    private EditorScreen editor;

    public BarkfartApp() {
        this(1f);
    }

    /** @param uiScale logical screen units per UI point (1 on iOS). */
    public BarkfartApp(float uiScale) {
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
        editor = new EditorScreen(newProject(), uiScale);
        Thread st = new Thread(new Runnable() {
            public void run() {
                final SelfTest.Result r = SelfTest.run();
                System.out.println("[Barkfart] self-test " + r.summary());
                for (String f : r.failures) System.out.println("[Barkfart] FAIL " + f);
                Gdx.app.postRunnable(new Runnable() {
                    public void run() {
                        editor.state.flash("Barkfart " + VERSION + " · self-test " + r.summary()
                                + (r.ok() ? "" : " · " + r.failures.get(0)), null);
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
    public void dispose() {
        editor.dispose();
    }
}
