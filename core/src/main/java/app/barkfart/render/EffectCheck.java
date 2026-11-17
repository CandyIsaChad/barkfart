package app.barkfart.render;

import app.barkfart.eval.Evaluator;
import app.barkfart.fx.Compositor;
import app.barkfart.fx.EffectScene;
import app.barkfart.fx.Image;
import app.barkfart.geom.DrawList;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Project;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.glutils.FrameBuffer;
import com.badlogic.gdx.math.Matrix4;
import com.badlogic.gdx.utils.BufferUtils;
import java.nio.ByteBuffer;

/**
 * Renders EffectScene with GL and with the reference compositor and compares
 * them. Runs on the GL thread (launch self-test, desktop --fxcheck).
 */
public final class EffectCheck {
    private EffectCheck() {}

    /** Last GL readback (RGBA8, rows bottom-up), for saving a picture. */
    public static byte[] lastGl;
    public static Image lastRef;

    /** Null when GL effects match the reference; otherwise the reason. */
    public static String run() {
        MeshRenderer mesh = null;
        EffectRenderer fx = null;
        FrameBuffer target = null;
        try {
            int w = EffectScene.W, h = EffectScene.H;
            Project p = EffectScene.build();
            DrawList dl = Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, 0));
            Image ref = new Image(w, h, -w / 2f, h / 2f, 1);
            Compositor.render(dl, ref);
            lastRef = ref;
            mesh = new MeshRenderer();
            fx = new EffectRenderer(mesh);
            if (fx.problem() != null) return fx.problem();
            target = new FrameBuffer(Pixmap.Format.RGBA8888, w, h, false);
            target.bind();
            Gdx.gl.glViewport(0, 0, w, h);
            Gdx.gl.glDisable(GL20.GL_SCISSOR_TEST);
            Gdx.gl.glClearColor(0, 0, 0, 0);
            Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
            Matrix4 proj = new Matrix4().setToOrtho2D(-w / 2f, -h / 2f, w, h);
            fx.draw(dl, proj, null, 0, 0, w, h);
            if (fx.problem() != null) return fx.problem();
            ByteBuffer buf = BufferUtils.newByteBuffer(w * h * 4);
            Gdx.gl.glPixelStorei(GL20.GL_PACK_ALIGNMENT, 1);
            Gdx.gl.glReadPixels(0, 0, w, h, GL20.GL_RGBA, GL20.GL_UNSIGNED_BYTE, buf);
            byte[] px = new byte[w * h * 4];
            buf.get(px);
            lastGl = px;
            return EffectScene.compare(px, ref);
        } catch (RuntimeException e) {
            return "effect check: " + e;
        } finally {
            FrameBuffer.unbind();
            if (target != null) target.dispose();
            if (fx != null) fx.dispose();
            if (mesh != null) mesh.dispose();
            if (Gdx.graphics != null) Gdx.gl.glViewport(0, 0, Gdx.graphics.getBackBufferWidth(), Gdx.graphics.getBackBufferHeight());
        }
    }
}
