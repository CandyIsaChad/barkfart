package app.barkfart;

import app.barkfart.demo.Demo;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Project;
import app.barkfart.render.DrawListRenderer;
import app.barkfart.selftest.SelfTest;
import com.badlogic.gdx.ApplicationAdapter;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.OrthographicCamera;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.SpriteBatch;
import com.badlogic.gdx.utils.viewport.ExtendViewport;
import com.badlogic.gdx.utils.viewport.ScreenViewport;

/**
 * Until the editor exists (M4): plays the built-in demo through the real
 * evaluator, tessellator and GL renderer, and shows the core self-test result.
 */
public class BarkfartApp extends ApplicationAdapter {
    public static final String VERSION = "0.0.1-m3a";

    private ExtendViewport stage;
    private ScreenViewport hud;
    private DrawListRenderer renderer;
    private SpriteBatch batch;
    private BitmapFont font;
    private Project demo;
    private volatile SelfTest.Result selfTest;
    private double time;

    @Override
    public void create() {
        // Runs off the render thread: the budget checks take a moment on a phone.
        Thread st = new Thread(new Runnable() {
            public void run() {
                SelfTest.Result r = SelfTest.run();
                selfTest = r;
                System.out.println("[Barkfart] self-test " + r.summary());
                for (String f : r.failures) System.out.println("[Barkfart] FAIL " + f);
            }
        }, "self-test");
        st.setDaemon(true);
        st.start();
        OrthographicCamera cam = new OrthographicCamera();
        stage = new ExtendViewport(640, 360, cam);
        hud = new ScreenViewport();
        renderer = new DrawListRenderer();
        batch = new SpriteBatch();
        font = new BitmapFont();
        demo = Demo.waving();
    }

    @Override
    public void resize(int width, int height) {
        stage.update(width, height, false);
        stage.getCamera().position.set(0, 0, 0);
        stage.getCamera().update();
        hud.setUnitsPerPixel(1f / Math.max(1f, Math.round(Gdx.graphics.getDensity())));
        hud.update(width, height, true);
    }

    @Override
    public void render() {
        time += Math.min(Gdx.graphics.getDeltaTime(), 0.1f);
        double t = time % 2.0;
        Gdx.gl.glClearColor(0.95f, 0.95f, 0.95f, 1f);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
        stage.apply();
        Scene scene = Evaluator.evaluate(demo, demo.rootSymbol, t);
        renderer.draw(Tessellator.tessellate(scene), stage.getCamera().combined);

        hud.apply();
        batch.setProjectionMatrix(hud.getCamera().combined);
        batch.begin();
        float top = hud.getWorldHeight() - 10;
        SelfTest.Result st = selfTest;
        String status = st == null ? "running..." : st.summary();
        font.setColor(st == null ? Color.GRAY : (st.ok() ? new Color(0.1f, 0.5f, 0.15f, 1) : Color.RED));
        font.draw(batch, "Barkfart " + VERSION + "   core self-test " + status, 10, top);
        font.setColor(Color.DARK_GRAY);
        font.draw(batch, Gdx.graphics.getFramesPerSecond() + " fps   " + scene.primitives + " objects", 10, top - 20);
        float y = top - 40;
        if (st != null) for (String f : st.failures) {
            font.setColor(Color.RED);
            font.draw(batch, f, 10, y);
            y -= 18;
        }
        batch.end();
    }

    @Override
    public void dispose() {
        renderer.dispose();
        batch.dispose();
        font.dispose();
    }
}
