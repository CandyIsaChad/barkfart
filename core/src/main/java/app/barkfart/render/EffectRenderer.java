package app.barkfart.render;

import app.barkfart.eval.ResolvedEffect;
import app.barkfart.fx.EffectTypes;
import app.barkfart.fx.Kernels;
import app.barkfart.geom.DrawList;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.Mesh;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.VertexAttribute;
import com.badlogic.gdx.graphics.VertexAttributes;
import com.badlogic.gdx.graphics.glutils.FrameBuffer;
import com.badlogic.gdx.graphics.glutils.ShaderProgram;
import com.badlogic.gdx.math.Matrix4;
import com.badlogic.gdx.utils.BufferUtils;
import com.badlogic.gdx.utils.Disposable;
import java.nio.IntBuffer;

/**
 * Draws a layered DrawList (Tessellator.layered) with GL: each isolate renders
 * into a pooled framebuffer covering its bounds plus effect padding
 * (premultiplied alpha), its effects run as shader passes, and the result is
 * composited into its parent. This mirrors app.barkfart.fx.Compositor, the
 * reference definition. If shaders fail to compile or a pass throws, effects
 * switch off and the list is drawn flat, as before effects existed.
 */
public final class EffectRenderer implements Disposable {
    private static final String VERT =
            "attribute vec2 a_position;\n"
            + "attribute vec2 a_texCoord0;\n"
            + "varying vec2 v_uv;\n"
            + "void main() {\n"
            + "  v_uv = a_texCoord0;\n"
            + "  gl_Position = vec4(a_position, 0.0, 1.0);\n"
            + "}\n";
    private static final String HEAD = "#ifdef GL_ES\nprecision mediump float;\n#endif\nvarying vec2 v_uv;\nuniform sampler2D u_tex;\n";
    /** Premultiplied copy scaled by an opacity. */
    private static final String COPY = HEAD + "uniform float u_opacity;\nvoid main() { gl_FragColor = texture2D(u_tex, v_uv) * u_opacity; }\n";
    /** One direction of a Gaussian: 49 taps spaced u_step apart (stride grows with sigma). */
    private static final String BLUR = HEAD
            + "uniform vec2 u_step;\nuniform float u_sigma;\n"
            + "void main() {\n"
            + "  vec4 acc = vec4(0.0); float ws = 0.0;\n"
            + "  for (int i = -24; i <= 24; i++) {\n"
            + "    float x = float(i);\n"
            + "    float w = exp(-x * x / (2.0 * u_sigma * u_sigma));\n"
            + "    vec2 uv = v_uv + u_step * x;\n"
            + "    vec4 s = texture2D(u_tex, uv);\n"
            + "    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > u_max.x || uv.y > u_max.y) s = vec4(0.0);\n"
            + "    acc += s * w; ws += w;\n"
            + "  }\n"
            + "  gl_FragColor = acc / ws;\n"
            + "}\n";
    /** The source's alpha, moved by u_offset (texture units). */
    private static final String SHIFT = HEAD
            + "uniform vec2 u_offset;\n"
            + "void main() {\n"
            + "  vec2 uv = v_uv - u_offset;\n"
            + "  float a = texture2D(u_tex, uv).a;\n"
            + "  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > u_max.x || uv.y > u_max.y) a = 0.0;\n"
            + "  gl_FragColor = vec4(0.0, 0.0, 0.0, a);\n"
            + "}\n";
    /** Source over a tinted (blurred) alpha: glow and drop shadow. */
    private static final String UNDER = HEAD
            + "uniform sampler2D u_tex2;\nuniform vec4 u_color;\nuniform float u_strength;\n"
            + "void main() {\n"
            + "  vec4 src = texture2D(u_tex, v_uv);\n"
            + "  float sa = min(1.0, texture2D(u_tex2, v_uv).a * u_strength) * u_color.a;\n"
            + "  gl_FragColor = vec4(src.rgb + u_color.rgb * sa * (1.0 - src.a), src.a + sa * (1.0 - src.a));\n"
            + "}\n";
    private static final String INVERT = HEAD + "void main() { vec4 c = texture2D(u_tex, v_uv); gl_FragColor = vec4(c.a - c.rgb, c.a); }\n";
    private static final String ADJUST = HEAD
            + "uniform mat3 u_m;\nuniform float u_bright;\n"
            + "void main() {\n"
            + "  vec4 c = texture2D(u_tex, v_uv);\n"
            + "  if (c.a <= 0.0) { gl_FragColor = c; return; }\n"
            + "  vec3 rgb = clamp(u_m * (c.rgb / c.a) + vec3(u_bright), 0.0, 1.0);\n"
            + "  gl_FragColor = vec4(rgb * c.a, c.a);\n"
            + "}\n";
    private static final String TINT = HEAD
            + "uniform vec4 u_color;\nuniform float u_amount;\n"
            + "void main() { vec4 c = texture2D(u_tex, v_uv); gl_FragColor = vec4(c.rgb + (u_color.rgb * c.a - c.rgb) * u_amount, c.a); }\n";
    private static final String LERP = HEAD
            + "uniform sampler2D u_tex2;\nuniform float u_t;\n"
            + "void main() { gl_FragColor = mix(texture2D(u_tex, v_uv), texture2D(u_tex2, v_uv), u_t); }\n";

    private final MeshRenderer meshes;
    private final BufferPool pool = new BufferPool();
    private ShaderProgram copy, blur, shift, under, invert, adjust, tint, lerp;
    private Mesh quad;
    private final float[] q = new float[16];
    /** False once anything failed: draw() then falls back to flat drawing. */
    private boolean ok;
    private String problem = "not started";

    public EffectRenderer(MeshRenderer meshes) {
        this.meshes = meshes;
        try {
            ShaderProgram.pedantic = false;
            copy = compile(COPY, false);
            blur = compile(BLUR, true);
            shift = compile(SHIFT, true);
            under = compile(UNDER, false);
            invert = compile(INVERT, false);
            adjust = compile(ADJUST, false);
            tint = compile(TINT, false);
            lerp = compile(LERP, false);
            quad = new Mesh(false, 4, 0,
                    new VertexAttribute(VertexAttributes.Usage.Position, 2, ShaderProgram.POSITION_ATTRIBUTE),
                    new VertexAttribute(VertexAttributes.Usage.TextureCoordinates, 2, ShaderProgram.TEXCOORD_ATTRIBUTE + "0"));
            ok = true;
            problem = null;
        } catch (RuntimeException e) {
            ok = false;
            problem = String.valueOf(e.getMessage());
        }
    }

    private static ShaderProgram compile(String frag, boolean bounded) {
        String f = bounded ? frag.replace("uniform sampler2D u_tex;\n", "uniform sampler2D u_tex;\nuniform vec2 u_max;\n") : frag;
        ShaderProgram p = new ShaderProgram(VERT, f);
        if (!p.isCompiled()) throw new IllegalStateException("effect shader: " + p.getLog());
        return p;
    }

    /** Null when effects work; otherwise why they are off. */
    public String problem() {
        return problem;
    }

    // ---- targets ----

    /** An offscreen buffer holding a rectangle of the viewport (pixels, viewport-absolute). */
    private static final class Target {
        final FrameBuffer fb;
        final int x, y, w, h;

        Target(FrameBuffer fb, int x, int y, int w, int h) {
            this.fb = fb;
            this.x = x;
            this.y = y;
            this.w = w;
            this.h = h;
        }
    }

    private int vx, vy, vw, vh;
    private Matrix4 proj;
    private MeshRenderer.Textures textures;
    private float pxPerUnit;

    /**
     * Draws dl with the given projection (view space to clip space of the
     * current viewport). clipX..clipH (viewport pixels) bounds the offscreen
     * work, e.g. the visible stage area.
     */
    public void draw(DrawList dl, Matrix4 projection, MeshRenderer.Textures tex, int clipX, int clipY, int clipW, int clipH) {
        if (!ok || dl.root == null || !hasIsolates(dl.root)) {
            meshes.draw(dl, projection, tex);
            return;
        }
        IntBuffer vp = BufferUtils.newIntBuffer(16);
        Gdx.gl.glGetIntegerv(GL20.GL_VIEWPORT, vp);
        vx = vp.get(0);
        vy = vp.get(1);
        vw = vp.get(2);
        vh = vp.get(3);
        boolean scissor = Gdx.gl.glIsEnabled(GL20.GL_SCISSOR_TEST);
        IntBuffer fbo = BufferUtils.newIntBuffer(16);
        Gdx.gl.glGetIntegerv(GL20.GL_FRAMEBUFFER_BINDING, fbo);
        int outer = fbo.get(0);   // whatever was bound: the screen (not 0 on iOS) or a caller's buffer
        proj = projection;
        textures = tex;
        float[] m = projection.val;
        pxPerUnit = (float) Math.hypot(m[Matrix4.M00], m[Matrix4.M10]) * vw / 2f;
        int cx0 = Math.max(vx, clipX), cy0 = Math.max(vy, clipY);
        int cx1 = Math.min(vx + vw, clipX + clipW), cy1 = Math.min(vy + vh, clipY + clipH);
        Target root = null;
        try {
            Gdx.gl.glDisable(GL20.GL_SCISSOR_TEST);
            if (cx1 > cx0 && cy1 > cy0) root = render(dl.root, cx0, cy0, cx1 - cx0, cy1 - cy0, true);
            Gdx.gl.glBindFramebuffer(GL20.GL_FRAMEBUFFER, outer);
            Gdx.gl.glViewport(vx, vy, vw, vh);
            if (scissor) Gdx.gl.glEnable(GL20.GL_SCISSOR_TEST);
            if (root != null) {
                Gdx.gl.glEnable(GL20.GL_BLEND);
                MeshRenderer.premultipliedBlend(Layer.Blend.NORMAL);
                blit(copy, root, vx, vy, vw, vh, 1f);
            }
        } catch (RuntimeException e) {
            ok = false;
            problem = "effects off: " + e;
            System.out.println("[Barkfart] " + problem);
            Gdx.gl.glBindFramebuffer(GL20.GL_FRAMEBUFFER, outer);
            Gdx.gl.glViewport(vx, vy, vw, vh);
            if (scissor) Gdx.gl.glEnable(GL20.GL_SCISSOR_TEST);
            meshes.draw(dl, projection, tex);
        } finally {
            if (root != null) pool.release(root.fb);
            Gdx.gl.glBlendFunc(GL20.GL_SRC_ALPHA, GL20.GL_ONE_MINUS_SRC_ALPHA);
        }
    }

    private static boolean hasIsolates(DrawList.Isolate iso) {
        for (Object o : iso.items) if (o instanceof DrawList.Isolate) return true;
        return false;
    }

    /** Padded view-space bounds of an isolate (its effects and its children's). */
    private static float[] bounds(DrawList.Isolate iso) {
        float x0 = iso.minX, y0 = iso.minY, x1 = iso.maxX, y1 = iso.maxY;
        for (Object o : iso.items) {
            if (!(o instanceof DrawList.Isolate)) continue;
            float[] b = bounds((DrawList.Isolate) o);
            if (b == null) continue;
            x0 = Math.min(x0, b[0]);
            y0 = Math.min(y0, b[1]);
            x1 = Math.max(x1, b[2]);
            y1 = Math.max(y1, b[3]);
        }
        if (!(x0 <= x1 && y0 <= y1)) return null;
        float pad = 0;
        for (ResolvedEffect e : iso.effects) pad += EffectTypes.padding(e);
        return new float[] {x0 - pad, y0 - pad, x1 + pad, y1 + pad};
    }

    /** Renders an isolate into a new target within the clip rectangle; null when nothing is visible. */
    private Target render(DrawList.Isolate iso, int clipX, int clipY, int clipW, int clipH, boolean isRoot) {
        int rx = clipX, ry = clipY, rw = clipW, rh = clipH;
        if (!isRoot) {
            float[] b = bounds(iso);
            if (b == null) return null;
            float[] p0 = toPixels(b[0], b[1]), p1 = toPixels(b[2], b[3]), p2 = toPixels(b[0], b[3]), p3 = toPixels(b[2], b[1]);
            float minx = Math.min(Math.min(p0[0], p1[0]), Math.min(p2[0], p3[0])), maxx = Math.max(Math.max(p0[0], p1[0]), Math.max(p2[0], p3[0]));
            float miny = Math.min(Math.min(p0[1], p1[1]), Math.min(p2[1], p3[1])), maxy = Math.max(Math.max(p0[1], p1[1]), Math.max(p2[1], p3[1]));
            int x0 = Math.max(clipX, (int) Math.floor(minx) - 2), y0 = Math.max(clipY, (int) Math.floor(miny) - 2);
            int x1 = Math.min(clipX + clipW, (int) Math.ceil(maxx) + 2), y1 = Math.min(clipY + clipH, (int) Math.ceil(maxy) + 2);
            if (!(x1 > x0 && y1 > y0)) return null;
            rx = x0;
            ry = y0;
            rw = x1 - x0;
            rh = y1 - y0;
        }
        if (Boolean.getBoolean("barkfart.fxdebug")) System.out.println("[fx] isolate " + iso.source + " root=" + isRoot + " rect " + rx + "," + ry + " " + rw + "x" + rh + " effects " + iso.effects.size() + " op " + iso.opacity + " vp " + vx + "," + vy + " " + vw + "x" + vh + " px/unit " + pxPerUnit);
        Target t = new Target(pool.acquire(rw, rh), rx, ry, rw, rh);
        bindClear(t);
        Matrix4 local = localProjection(t);
        boolean drawing = false;
        for (Object o : iso.items) {
            if (o instanceof DrawList.Batch) {
                if (!drawing) {
                    meshes.begin(local, true);
                    drawing = true;
                }
                meshes.batch((DrawList.Batch) o, textures, true);
                continue;
            }
            drawing = false;
            DrawList.Isolate child = (DrawList.Isolate) o;
            if (child.adjustment) {
                Target before = copyOf(t);
                Target fx = effects(before, child.effects, false);
                bind(t);
                Gdx.gl.glDisable(GL20.GL_BLEND);
                lerp.bind();
                lerp.setUniformi("u_tex", 0);
                lerp.setUniformi("u_tex2", 1);
                lerp.setUniformf("u_t", Math.max(0, Math.min(1, child.opacity)));
                fx.fb.getColorBufferTexture().bind(1);
                before.fb.getColorBufferTexture().bind(0);
                drawQuad(lerp, t, before, t);
                Gdx.gl.glEnable(GL20.GL_BLEND);
                if (fx != before) pool.release(fx.fb);
                pool.release(before.fb);
                continue;
            }
            Target c = render(child, t.x, t.y, t.w, t.h, false);
            bind(t);
            if (c == null) continue;
            Target done = effects(c, child.effects, true);
            bind(t);
            Gdx.gl.glEnable(GL20.GL_BLEND);
            MeshRenderer.premultipliedBlend(child.blend);
            blitInto(t, done, child.opacity);
            pool.release(done.fb);
        }
        return t;
    }

    /** Runs effects in order; consumes src when owned (returns a new target, or src when there are none). */
    private Target effects(Target src, java.util.List<ResolvedEffect> list, boolean owned) {
        Target cur = src;
        for (ResolvedEffect e : list) {
            float mix = e.mix == e.mix ? Math.max(0, Math.min(1, e.mix)) : 1;
            if (mix <= 0 || !EffectTypes.known(e.type)) continue;
            Target out = one(e, cur);
            if (mix < 1) {
                Target m = blank(cur);
                Gdx.gl.glDisable(GL20.GL_BLEND);
                lerp.bind();
                lerp.setUniformi("u_tex", 0);
                lerp.setUniformi("u_tex2", 1);
                lerp.setUniformf("u_t", mix);
                out.fb.getColorBufferTexture().bind(1);
                cur.fb.getColorBufferTexture().bind(0);
                drawQuad(lerp, m, cur, m);
                pool.release(out.fb);
                out = m;
            }
            if (cur != src || owned) pool.release(cur.fb);
            cur = out;
        }
        Gdx.gl.glEnable(GL20.GL_BLEND);
        return cur;
    }

    /** One effect pass: a new target from src (same rectangle). */
    private Target one(ResolvedEffect e, Target src) {
        float s = pxPerUnit;
        Gdx.gl.glDisable(GL20.GL_BLEND);
        if (EffectTypes.BLUR.equals(e.type)) {
            return blurred(src, EffectTypes.sigma(EffectTypes.f(e, "radius", 6, 0, EffectTypes.MAX_LENGTH)) * s, false);
        }
        if (EffectTypes.GLOW.equals(e.type) || EffectTypes.SHADOW.equals(e.type)) {
            boolean glow = EffectTypes.GLOW.equals(e.type);
            Color c = glow ? EffectTypes.c(e, "color", Color.WHITE) : EffectTypes.c(e, "color", new Color(0, 0, 0, 0.6f));
            float radius = glow ? EffectTypes.f(e, "radius", 8, 0, EffectTypes.MAX_LENGTH) : EffectTypes.f(e, "radius", 6, 0, EffectTypes.MAX_LENGTH);
            float dx = glow ? 0 : EffectTypes.f(e, "dx", 8, -EffectTypes.MAX_LENGTH, EffectTypes.MAX_LENGTH) * s;
            float dy = glow ? 0 : EffectTypes.f(e, "dy", -8, -EffectTypes.MAX_LENGTH, EffectTypes.MAX_LENGTH) * s;   // GL y is up, like view space
            float strength = glow ? EffectTypes.f(e, "strength", 1, 0, 4) : 1;
            Target alpha = blank(src);
            shift.bind();
            shift.setUniformi("u_tex", 0);
            shift.setUniformf("u_offset", Math.round(dx) / (float) src.fb.getWidth(), Math.round(dy) / (float) src.fb.getHeight());
            setMax(shift, src);
            src.fb.getColorBufferTexture().bind(0);
            drawQuad(shift, alpha, src, alpha);
            Target soft = blurred(alpha, EffectTypes.sigma(radius) * s, true);
            Target out = blank(src);
            under.bind();
            under.setUniformi("u_tex", 0);
            under.setUniformi("u_tex2", 1);
            under.setUniformf("u_color", c.r, c.g, c.b, c.a);
            under.setUniformf("u_strength", strength);
            soft.fb.getColorBufferTexture().bind(1);
            src.fb.getColorBufferTexture().bind(0);
            drawQuad(under, out, src, out);
            pool.release(soft.fb);
            return out;
        }
        Target out = blank(src);
        ShaderProgram p;
        if (EffectTypes.INVERT.equals(e.type)) {
            p = invert;
            p.bind();
        } else if (EffectTypes.ADJUST.equals(e.type)) {
            p = adjust;
            p.bind();
            float[] m = Kernels.hueSaturation(EffectTypes.f(e, "hue", 0, -3600, 3600), EffectTypes.f(e, "saturation", 1, 0, 4));
            float[] colMajor = {m[0], m[3], m[6], m[1], m[4], m[7], m[2], m[5], m[8]};
            p.setUniformMatrix("u_m", new com.badlogic.gdx.math.Matrix3(colMajor));
            p.setUniformf("u_bright", EffectTypes.f(e, "brightness", 0, -1, 1));
        } else {
            p = tint;
            p.bind();
            Color c = EffectTypes.c(e, "color", new Color(1, 0, 0, 1));
            p.setUniformf("u_color", c.r, c.g, c.b, c.a);
            p.setUniformf("u_amount", EffectTypes.f(e, "amount", 0.5f, 0, 1) * c.a);
        }
        p.setUniformi("u_tex", 0);
        src.fb.getColorBufferTexture().bind(0);
        drawQuad(p, out, src, out);
        return out;
    }

    /** Two-pass Gaussian; consumes src when owned. */
    private Target blurred(Target src, float sigma, boolean owned) {
        if (!(sigma > 0.05f)) {
            if (owned) return src;
            Target c = copyOf(src);
            return c;
        }
        sigma = Math.min(sigma, Kernels.MAX_SIGMA);
        float stride = Math.max(1f, 3 * sigma / 24f);   // 49 taps reach 3 sigma
        Target h = blank(src), v = blank(src);
        blur.bind();
        blur.setUniformi("u_tex", 0);
        blur.setUniformf("u_sigma", sigma / stride);
        setMax(blur, src);
        blur.setUniformf("u_step", stride / src.fb.getWidth(), 0);
        src.fb.getColorBufferTexture().bind(0);
        drawQuad(blur, h, src, h);
        blur.setUniformf("u_step", 0, stride / src.fb.getHeight());
        h.fb.getColorBufferTexture().bind(0);
        drawQuad(blur, v, h, v);
        pool.release(h.fb);
        if (owned) pool.release(src.fb);
        return v;
    }

    // ---- plumbing ----

    private float[] toPixels(float x, float y) {
        float[] m = proj.val;
        float nx = m[Matrix4.M00] * x + m[Matrix4.M01] * y + m[Matrix4.M03];
        float ny = m[Matrix4.M10] * x + m[Matrix4.M11] * y + m[Matrix4.M13];
        return new float[] {vx + (nx + 1) / 2 * vw, vy + (ny + 1) / 2 * vh};
    }

    /** Projection mapping view space onto a target's rectangle (its own viewport). */
    private Matrix4 localProjection(Target t) {
        Matrix4 m = new Matrix4();
        float sx = (float) vw / t.w, sy = (float) vh / t.h;
        m.val[Matrix4.M00] = sx;
        m.val[Matrix4.M11] = sy;
        m.val[Matrix4.M03] = (2f * (vx - t.x) + vw) / t.w - 1;
        m.val[Matrix4.M13] = (2f * (vy - t.y) + vh) / t.h - 1;
        return m.mul(proj);
    }

    private void bind(Target t) {
        t.fb.bind();
        Gdx.gl.glViewport(0, 0, t.w, t.h);
    }

    private void bindClear(Target t) {
        bind(t);
        Gdx.gl.glClearColor(0, 0, 0, 0);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
    }

    private Target blank(Target like) {
        Target t = new Target(pool.acquire(like.w, like.h), like.x, like.y, like.w, like.h);
        bindClear(t);
        return t;
    }

    private Target copyOf(Target src) {
        Target t = blank(src);
        Gdx.gl.glDisable(GL20.GL_BLEND);
        copy.bind();
        copy.setUniformi("u_tex", 0);
        copy.setUniformf("u_opacity", 1f);
        src.fb.getColorBufferTexture().bind(0);
        drawQuad(copy, t, src, t);
        return t;
    }

    private static void setMax(ShaderProgram p, Target t) {
        p.setUniformf("u_max", (t.w - 0.5f) / t.fb.getWidth(), (t.h - 0.5f) / t.fb.getHeight());
    }

    /** Composites src into the bound target dst with the current blend and an opacity. */
    private void blitInto(Target dst, Target src, float opacity) {
        copy.bind();
        copy.setUniformi("u_tex", 0);
        copy.setUniformf("u_opacity", Math.max(0, Math.min(1, opacity)));
        src.fb.getColorBufferTexture().bind(0);
        drawQuad(copy, dst, src, src);
    }

    /** Composites a target onto the screen viewport. */
    private void blit(ShaderProgram p, Target src, int x, int y, int w, int h, float opacity) {
        p.bind();
        p.setUniformi("u_tex", 0);
        p.setUniformf("u_opacity", opacity);
        src.fb.getColorBufferTexture().bind(0);
        float x0 = (src.x - x) * 2f / w - 1, y0 = (src.y - y) * 2f / h - 1;
        float x1 = (src.x + src.w - x) * 2f / w - 1, y1 = (src.y + src.h - y) * 2f / h - 1;
        quadVerts(x0, y0, x1, y1, src);
        quad.render(p, GL20.GL_TRIANGLE_FAN);
    }

    /**
     * Draws a quad covering `area` (viewport rectangle) into the bound target
     * `into`, sampling texture coordinates of `tex`'s rectangle.
     */
    private void drawQuad(ShaderProgram p, Target into, Target tex, Target area) {
        bind(into);
        float x0 = (area.x - into.x) * 2f / into.w - 1, y0 = (area.y - into.y) * 2f / into.h - 1;
        float x1 = (area.x + area.w - into.x) * 2f / into.w - 1, y1 = (area.y + area.h - into.y) * 2f / into.h - 1;
        quadVerts(x0, y0, x1, y1, tex);
        quad.render(p, GL20.GL_TRIANGLE_FAN);
    }

    private void quadVerts(float x0, float y0, float x1, float y1, Target tex) {
        Texture t = tex.fb.getColorBufferTexture();
        float u1 = (float) tex.w / t.getWidth(), v1 = (float) tex.h / t.getHeight();
        q[0] = x0; q[1] = y0; q[2] = 0; q[3] = 0;
        q[4] = x1; q[5] = y0; q[6] = u1; q[7] = 0;
        q[8] = x1; q[9] = y1; q[10] = u1; q[11] = v1;
        q[12] = x0; q[13] = y1; q[14] = 0; q[15] = v1;
        quad.setVertices(q);
    }

    @Override
    public void dispose() {
        pool.dispose();
        for (ShaderProgram p : new ShaderProgram[] {copy, blur, shift, under, invert, adjust, tint, lerp}) if (p != null) p.dispose();
        if (quad != null) quad.dispose();
    }
}
