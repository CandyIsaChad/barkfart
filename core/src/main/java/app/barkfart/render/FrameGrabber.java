package app.barkfart.render;

import app.barkfart.eval.Evaluator;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Project;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.glutils.FrameBuffer;
import com.badlogic.gdx.math.Matrix4;
import com.badlogic.gdx.utils.BufferUtils;
import com.badlogic.gdx.utils.Disposable;
import java.nio.ByteBuffer;

/**
 * Renders the project's shot (camera applied, effects included) at a time
 * into an offscreen buffer of the export size and reads it back as straight
 * RGBA8, rows top-down: over the project background, or transparent.
 * Drawn at twice the size and averaged down by default, so edges are smooth
 * (offscreen buffers have no multisampling).
 * GL thread only.
 */
public final class FrameGrabber implements Disposable {
    public final int w, h;
    private final MeshRenderer mesh = new MeshRenderer();
    private final EffectRenderer fx = new EffectRenderer(mesh);
    private final FrameBuffer fb;
    private final AssetTextures textures;
    private final GlyphAtlas glyphs = new GlyphAtlas();
    private final ByteBuffer buf;
    private final byte[] raw;

    /** Rendering scale: frames are drawn this many times larger and averaged down (smooth edges). */
    public final int ss;
    private final byte[] big;

    public FrameGrabber(Project p, int w, int h) {
        this(p, w, h, 2);
    }

    /** supersample: 1 = as drawn, 2 = 4 samples per pixel (capped at 4096 px wide / high). */
    public FrameGrabber(Project p, int w, int h, int supersample) {
        this.w = w;
        this.h = h;
        int k = Math.max(1, Math.min(4, supersample));
        while (k > 1 && (w * k > 4096 || h * k > 4096)) k--;
        ss = k;
        fb = new FrameBuffer(Pixmap.Format.RGBA8888, w * ss, h * ss, false);
        textures = new AssetTextures(p);
        buf = BufferUtils.newByteBuffer(w * ss * h * ss * 4);
        big = new byte[w * ss * h * ss * 4];
        raw = new byte[w * h * 4];
    }

    /** Problem with the effect renderer, if any (frames then draw without effects). */
    public String problem() {
        return fx.problem();
    }

    public byte[] grab(Project p, double t, boolean transparent) {
        int bw = w * ss, bh = h * ss;
        fb.bind();
        Gdx.gl.glViewport(0, 0, bw, bh);
        Gdx.gl.glDisable(GL20.GL_SCISSOR_TEST);
        Gdx.gl.glClearColor(0, 0, 0, 0);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
        float sw = p.settings.width, sh = p.settings.height;
        Matrix4 proj = new Matrix4().setToOrtho2D(-sw / 2, -sh / 2, sw, sh);
        fx.draw(Tessellator.layered(Evaluator.evaluate(p, p.rootSymbol, t, false, true), glyphs), proj, glyphs.with(textures), true);
        fb.bind();   // the renderer restores the binding it found; be explicit before reading
        Gdx.gl.glPixelStorei(GL20.GL_PACK_ALIGNMENT, 1);
        buf.clear();
        Gdx.gl.glReadPixels(0, 0, bw, bh, GL20.GL_RGBA, GL20.GL_UNSIGNED_BYTE, buf);
        buf.get(big);
        FrameBuffer.unbind();
        if (ss == 1) System.arraycopy(big, 0, raw, 0, raw.length);
        else {   // box filter over ss x ss samples (premultiplied, so edges average correctly)
            int n = ss * ss;
            for (int y = 0; y < h; y++) {
                for (int x = 0; x < w; x++) {
                    int r = 0, g = 0, b = 0, a = 0;
                    for (int dy = 0; dy < ss; dy++) {
                        int row = ((y * ss + dy) * bw + x * ss) * 4;
                        for (int dx = 0; dx < ss; dx++) {
                            int o = row + dx * 4;
                            r += big[o] & 0xff;
                            g += big[o + 1] & 0xff;
                            b += big[o + 2] & 0xff;
                            a += big[o + 3] & 0xff;
                        }
                    }
                    int o = (y * w + x) * 4;
                    raw[o] = (byte) ((r + n / 2) / n);
                    raw[o + 1] = (byte) ((g + n / 2) / n);
                    raw[o + 2] = (byte) ((b + n / 2) / n);
                    raw[o + 3] = (byte) ((a + n / 2) / n);
                }
            }
        }
        byte[] out = new byte[w * h * 4];
        app.barkfart.model.Color bg = p.settings.background;
        int br = Math.round(bg.r * 255), bgG = Math.round(bg.g * 255), bb = Math.round(bg.b * 255);
        for (int y = 0; y < h; y++) {
            int src = (h - 1 - y) * w * 4, dst = y * w * 4;   // GL rows are bottom-up
            for (int x = 0; x < w * 4; x += 4) {
                int r = raw[src + x] & 0xff, g = raw[src + x + 1] & 0xff, b = raw[src + x + 2] & 0xff, a = raw[src + x + 3] & 0xff;
                if (transparent) {   // premultiplied -> straight
                    if (a > 0) {
                        r = Math.min(255, r * 255 / a);
                        g = Math.min(255, g * 255 / a);
                        b = Math.min(255, b * 255 / a);
                    }
                } else {             // over the background
                    r = Math.min(255, r + br * (255 - a) / 255);
                    g = Math.min(255, g + bgG * (255 - a) / 255);
                    b = Math.min(255, b + bb * (255 - a) / 255);
                    a = 255;
                }
                out[dst + x] = (byte) r;
                out[dst + x + 1] = (byte) g;
                out[dst + x + 2] = (byte) b;
                out[dst + x + 3] = (byte) a;
            }
        }
        return out;
    }

    @Override
    public void dispose() {
        fb.dispose();
        fx.dispose();
        mesh.dispose();
        textures.dispose();
        glyphs.dispose();
    }
}
