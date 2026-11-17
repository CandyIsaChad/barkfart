package app.barkfart.render;

import app.barkfart.geom.DrawList;
import app.barkfart.model.Layer;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.Mesh;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.VertexAttribute;
import com.badlogic.gdx.graphics.VertexAttributes;
import com.badlogic.gdx.graphics.glutils.ShaderProgram;
import com.badlogic.gdx.math.Matrix4;
import com.badlogic.gdx.utils.Disposable;
import com.badlogic.gdx.utils.NumberUtils;

/**
 * Fills a DrawList with one dynamic mesh and one shader: per-vertex colour
 * times an optional texture, with the four blend modes. All geometry comes
 * from core (Tessellator); this only uploads it.
 */
public final class MeshRenderer implements Disposable {
    /** Supplies textures for textured batches (asset id to texture), or null when missing. */
    public interface Textures {
        Texture get(String assetId);
    }

    private static final int FLOATS = 5;              // x, y, packed colour, u, v
    private static final int MAX_VERTS = 3 * 10000;
    private static final String VERT =
            "attribute vec4 a_position;\n"
            + "attribute vec4 a_color;\n"
            + "attribute vec2 a_texCoord0;\n"
            + "uniform mat4 u_projTrans;\n"
            + "varying vec4 v_color;\n"
            + "varying vec2 v_uv;\n"
            + "void main() {\n"
            + "  v_color = a_color;\n"
            + "  v_color.a = v_color.a * (255.0 / 254.0);\n"
            + "  v_uv = a_texCoord0;\n"
            + "  gl_Position = u_projTrans * a_position;\n"
            + "}\n";
    private static final String FRAG =
            "#ifdef GL_ES\n"
            + "precision mediump float;\n"
            + "#endif\n"
            + "varying vec4 v_color;\n"
            + "varying vec2 v_uv;\n"
            + "uniform sampler2D u_texture;\n"
            + "uniform float u_premul;\n"
            + "void main() {\n"
            + "  vec4 c = v_color * texture2D(u_texture, v_uv);\n"
            + "  if (u_premul > 0.5) c.rgb *= c.a;\n"
            + "  gl_FragColor = c;\n"
            + "}\n";

    private final ShaderProgram shader;
    private final Mesh mesh;
    private final float[] verts = new float[MAX_VERTS * FLOATS];
    private final Texture white;

    public MeshRenderer() {
        ShaderProgram.pedantic = false;
        shader = new ShaderProgram(VERT, FRAG);
        if (!shader.isCompiled()) throw new IllegalStateException("shader: " + shader.getLog());
        mesh = new Mesh(false, MAX_VERTS, 0,
                new VertexAttribute(VertexAttributes.Usage.Position, 2, ShaderProgram.POSITION_ATTRIBUTE),
                new VertexAttribute(VertexAttributes.Usage.ColorPacked, 4, ShaderProgram.COLOR_ATTRIBUTE),
                new VertexAttribute(VertexAttributes.Usage.TextureCoordinates, 2, ShaderProgram.TEXCOORD_ATTRIBUTE + "0"));
        Pixmap pm = new Pixmap(1, 1, Pixmap.Format.RGBA8888);
        pm.setColor(Color.WHITE);
        pm.fill();
        white = new Texture(pm);
        pm.dispose();
    }

    public void draw(DrawList dl, Matrix4 projection, Textures textures) {
        begin(projection, false);
        for (DrawList.Batch b : dl.batches) batch(b, textures, false);
        end();
    }

    /** Starts drawing batches; premultiplied = into an offscreen (premultiplied) buffer. */
    void begin(Matrix4 projection, boolean premultiplied) {
        Gdx.gl.glEnable(GL20.GL_BLEND);
        shader.bind();
        shader.setUniformMatrix("u_projTrans", projection);
        shader.setUniformi("u_texture", 0);
        shader.setUniformf("u_premul", premultiplied ? 1f : 0f);
    }

    void batch(DrawList.Batch b, Textures textures, boolean premultiplied) {
        Texture t = b.texture == null || textures == null ? null : textures.get(b.texture);
        (t == null ? white : t).bind(0);
        if (premultiplied) premultipliedBlend(b.blend);
        else blend(b.blend);
        int n = b.vertexCount(), at = 0;
        while (at < n) {
            int count = Math.min(MAX_VERTS, n - at);
            int k = 0;
            for (int i = at; i < at + count; i++) {
                int c = b.rgba(i);
                int abgr = ((c & 0xff) << 24) | (((c >>> 8) & 0xff) << 16) | (((c >>> 16) & 0xff) << 8) | ((c >>> 24) & 0xff);
                verts[k++] = b.x(i);
                verts[k++] = b.y(i);
                verts[k++] = NumberUtils.intToFloatColor(abgr);
                verts[k++] = b.u(i);
                verts[k++] = b.v(i);
            }
            mesh.setVertices(verts, 0, k);
            mesh.render(shader, GL20.GL_TRIANGLES, 0, count);
            at += count;
        }
    }

    void end() {
        Gdx.gl.glBlendFunc(GL20.GL_SRC_ALPHA, GL20.GL_ONE_MINUS_SRC_ALPHA);
    }

    /** Blending for a premultiplied source into a premultiplied buffer (matches fx.Compositor). */
    static void premultipliedBlend(Layer.Blend mode) {
        switch (mode == null ? Layer.Blend.NORMAL : mode) {
            case ADD:
                Gdx.gl.glBlendFunc(GL20.GL_ONE, GL20.GL_ONE);
                break;
            case MULTIPLY:
                Gdx.gl.glBlendFuncSeparate(GL20.GL_DST_COLOR, GL20.GL_ONE_MINUS_SRC_ALPHA, GL20.GL_ONE, GL20.GL_ONE_MINUS_SRC_ALPHA);
                break;
            case SCREEN:
                Gdx.gl.glBlendFuncSeparate(GL20.GL_ONE, GL20.GL_ONE_MINUS_SRC_COLOR, GL20.GL_ONE, GL20.GL_ONE_MINUS_SRC_ALPHA);
                break;
            default:
                Gdx.gl.glBlendFunc(GL20.GL_ONE, GL20.GL_ONE_MINUS_SRC_ALPHA);
        }
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

    @Override
    public void dispose() {
        shader.dispose();
        mesh.dispose();
        white.dispose();
    }
}
