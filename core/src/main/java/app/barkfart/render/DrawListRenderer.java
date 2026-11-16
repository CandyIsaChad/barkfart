package app.barkfart.render;

import app.barkfart.geom.DrawList;
import app.barkfart.model.Layer;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.glutils.ShapeRenderer;
import com.badlogic.gdx.math.Matrix4;

/**
 * Fills a DrawList with GL. All geometry comes from core; this only uploads
 * triangles and sets blend modes. (v1 uses ShapeRenderer; a mesh path and the
 * effect pipeline arrive in M6.)
 */
public final class DrawListRenderer {
    private final ShapeRenderer shapes = new ShapeRenderer(15000);
    private final Color c = new Color();

    public void draw(DrawList dl, Matrix4 projection) {
        shapes.setProjectionMatrix(projection);
        Gdx.gl.glEnable(GL20.GL_BLEND);
        for (DrawList.Batch b : dl.batches) {
            blend(b.blend);
            shapes.begin(ShapeRenderer.ShapeType.Filled);
            int n = b.vertexCount();
            for (int i = 0; i + 2 < n; i += 3) {
                Color.rgba8888ToColor(c, b.rgba(i));
                shapes.setColor(c);
                shapes.triangle(b.x(i), b.y(i), b.x(i + 1), b.y(i + 1), b.x(i + 2), b.y(i + 2));
            }
            shapes.end();
        }
        Gdx.gl.glBlendFunc(GL20.GL_SRC_ALPHA, GL20.GL_ONE_MINUS_SRC_ALPHA);
    }

    private static void blend(Layer.Blend mode) {
        switch (mode) {
            case ADD:
                Gdx.gl.glBlendFunc(GL20.GL_SRC_ALPHA, GL20.GL_ONE);
                break;
            case MULTIPLY:
                Gdx.gl.glBlendFunc(GL20.GL_DST_COLOR, GL20.GL_ONE_MINUS_SRC_ALPHA);
                break;
            case SCREEN:
                Gdx.gl.glBlendFunc(GL20.GL_ONE, GL20.GL_ONE_MINUS_SRC_COLOR);
                break;
            default:
                Gdx.gl.glBlendFunc(GL20.GL_SRC_ALPHA, GL20.GL_ONE_MINUS_SRC_ALPHA);
        }
    }

    public void dispose() {
        shapes.dispose();
    }
}
