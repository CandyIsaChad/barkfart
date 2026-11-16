package app.barkfart;

import com.badlogic.gdx.ApplicationAdapter;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.glutils.ShapeRenderer;
import com.badlogic.gdx.utils.viewport.ExtendViewport;

/** Placeholder entry point: draws a static stick figure until the editor exists. */
public class BarkfartApp extends ApplicationAdapter {
    private ShapeRenderer shapes;
    private ExtendViewport viewport;

    @Override
    public void create() {
        shapes = new ShapeRenderer();
        viewport = new ExtendViewport(800, 450);
    }

    @Override
    public void resize(int width, int height) {
        viewport.update(width, height, true);
    }

    @Override
    public void render() {
        Gdx.gl.glClearColor(0.93f, 0.93f, 0.93f, 1f);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
        viewport.apply();
        shapes.setProjectionMatrix(viewport.getCamera().combined);
        float cx = viewport.getWorldWidth() / 2f, cy = viewport.getWorldHeight() / 2f;
        shapes.begin(ShapeRenderer.ShapeType.Filled);
        shapes.setColor(Color.BLACK);
        float w = 10f;
        shapes.circle(cx, cy + 95, 25);
        shapes.rectLine(cx, cy + 70, cx, cy - 20, w);          // torso
        shapes.rectLine(cx, cy + 50, cx - 45, cy + 10, w);     // arms
        shapes.rectLine(cx, cy + 50, cx + 45, cy + 10, w);
        shapes.rectLine(cx, cy - 20, cx - 35, cy - 100, w);    // legs
        shapes.rectLine(cx, cy - 20, cx + 35, cy - 100, w);
        shapes.end();
    }

    @Override
    public void dispose() {
        shapes.dispose();
    }
}
