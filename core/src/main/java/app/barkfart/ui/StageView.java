package app.barkfart.ui;

import app.barkfart.canvas.CanvasTools;
import app.barkfart.edit.StructureEdits;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.geom.OnionSkin;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.render.AssetTextures;
import app.barkfart.render.MeshRenderer;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.glutils.ShapeRenderer;
import com.badlogic.gdx.math.Matrix4;
import com.badlogic.gdx.math.Rectangle;
import com.badlogic.gdx.math.Vector2;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.Touchable;
import com.badlogic.gdx.scenes.scene2d.utils.ScissorStack;
import java.util.List;

/** The stage (UI-SPEC "Stage"): scene, onion skin, handles and all canvas gestures. */
public final class StageView extends Actor {
    private final Theme theme;
    private final EditorState st;
    private final MeshRenderer renderer = new MeshRenderer();
    private final app.barkfart.render.EffectRenderer effects = new app.barkfart.render.EffectRenderer(renderer);
    private final app.barkfart.render.GlyphAtlas glyphs = new app.barkfart.render.GlyphAtlas();
    private AssetTextures textures;
    private Project texturesFor;
    private final ShapeRenderer shapes = new ShapeRenderer();
    private final OnionSkin onion = new OnionSkin();

    // gesture state
    private final Vector2[] pts = {new Vector2(), new Vector2(), new Vector2()};
    private final boolean[] down = new boolean[3];
    private int maxPointers;
    private float gestureTime, moved;
    private Vector2 start = new Vector2();
    private CanvasTools.Hit pending;
    private boolean dragging, panning, pinching;
    private float pinchDist, pinchZoom;
    private Vector2 pinchMid = new Vector2();
    private float lastTapTime = -10;
    private Layer.CameraLayer camDrag;
    private float clock;

    public StageView(Theme theme, EditorState st) {
        this.theme = theme;
        this.st = st;
        setTouchable(Touchable.enabled);
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (pointer > 2) return false;
                pts[pointer].set(x, y);
                down[pointer] = true;
                int n = count();
                if (n == 1) {
                    maxPointers = 1;
                    gestureTime = 0;
                    moved = 0;
                    start.set(x, y);
                    boolean shot = st.playing || st.previewCamera;
                    pending = shot ? null : pick(x, y);
                    camOp = shot || st.mode != EditorState.Mode.CAMERA ? null : pickCamera(x, y);
                } else {
                    maxPointers = Math.max(maxPointers, n);
                    endSingle(true);
                    if (n == 2) beginPinch();
                }
                return true;
            }

            @Override
            public void touchDragged(InputEvent e, float x, float y, int pointer) {
                if (pointer > 2) return;
                moved = Math.max(moved, pts[pointer].dst(x, y) + 0);
                pts[pointer].set(x, y);
                if (count() >= 2) {
                    if (pinching) updatePinch();
                    return;
                }
                if (pointer != 0 || st.playing) return;
                if (!dragging && !panning && !camming && start.dst(x, y) > Theme.DRAG_SLOP) {
                    if (camOp != null) beginCamera();
                    else if (pending != null) beginDrag();
                    else beginPan();
                }
                if (camming) {
                    updateCamera(x, y);
                } else if (dragging) {
                    Vector2 v = toView(x, y);
                    st.tools.move(v.x, v.y, editTime());
                } else if (panning) {
                    updatePan(x, y);
                }
            }

            @Override
            public void touchUp(InputEvent e, float x, float y, int pointer, int button) {
                if (pointer > 2) return;
                down[pointer] = false;
                int n = count();
                if (n == 0) {
                    boolean tap = gestureTime < 0.3f && !dragging && !panning && !pinching && !camming
                            && start.dst(x, y) <= Theme.DRAG_SLOP;
                    endSingle(false);
                    endPinch();
                    if (tap && maxPointers == 2) st.doc.undo();
                    else if (tap && maxPointers == 3) st.doc.redo();
                    else if (tap && maxPointers == 1) tapped(x, y);
                } else if (n < 2) {
                    endPinch();
                }
            }
        });
    }

    private int count() {
        int c = 0;
        for (boolean b : down) if (b) c++;
        return c;
    }

    private double editTime() {
        return st.frames.time(st.frame);
    }

    // ---- view transform: view space (y up, stage centred) <-> actor-local points ----

    private float scale() {
        Project p = st.project();
        float fit = Math.min(getWidth() / p.settings.width, getHeight() / p.settings.height) * 0.94f;
        return fit * st.zoom;
    }

    private Vector2 toView(float lx, float ly) {
        float s = scale();
        return new Vector2((lx - getWidth() / 2 - st.panX) / s, (ly - getHeight() / 2 - st.panY) / s);
    }

    private float toLocalX(float vx) {
        return getWidth() / 2 + st.panX + vx * scale();
    }

    private float toLocalY(float vy) {
        return getHeight() / 2 + st.panY + vy * scale();
    }

    // ---- picking ----

    private CanvasTools.Hit pick(float lx, float ly) {
        if (st.mode == EditorState.Mode.CAMERA) return null;
        Vector2 v = toView(lx, ly);
        float r = Theme.HANDLE_HIT / scale();
        List<CanvasTools.Handle> hs = st.tools.handles(editTime());
        CanvasTools.Handle best = null;
        double bestD = Double.MAX_VALUE;
        for (int pass = 0; pass < 2 && best == null; pass++) {
            for (CanvasTools.Handle h : hs) {
                boolean selectedFig = h.layer == st.selectedLayer;
                if (pass == 0 && !selectedFig) continue;              // prefer the selected figure
                if (st.mode == EditorState.Mode.FIGURE && st.selectedLayer != null && !selectedFig) continue;
                double d = Math.hypot(h.x - v.x, h.y - v.y);
                if (d <= r && d <= bestD) {
                    bestD = d;
                    best = h;
                }
            }
        }
        return best == null ? null : new CanvasTools.Hit(best.layer, best.node, best.x, best.y);
    }

    private void tapped(float x, float y) {
        if (st.playing) {
            st.stop();
            return;
        }
        if (clock - lastTapTime < 0.3f) {      // double-tap empty stage: fit
            st.panX = st.panY = 0;
            st.zoom = 1;
            st.fire();
            lastTapTime = -10;
            return;
        }
        lastTapTime = clock;
        CanvasTools.Hit h = pick(x, y);
        if (h != null) st.select(h.layer, h.node);
        else if (st.mode != EditorState.Mode.CAMERA) st.select(null, null);
    }

    // ---- single-finger drag / pan ----

    private void beginDrag() {
        Vector2 v = toView(start.x, start.y);
        st.select(pending.layer, pending.node);
        st.tools.begin(pending, v.x, v.y, editTime());
        dragging = true;
    }

    private void beginPan() {
        panning = true;
    }

    private void updatePan(float x, float y) {
        st.panX = panBaseX + (x - start.x);
        st.panY = panBaseY + (y - start.y);
        st.fire();
    }

    private float panBaseX, panBaseY;

    private void endSingle(boolean cancelled) {
        if (dragging) {
            if (cancelled) st.tools.cancel();
            else st.tools.end();
        }
        if (camming) {
            st.doc.commit();
            camming = false;
        }
        dragging = panning = false;
        camOp = null;
        pending = null;
        camDrag = null;
        panBaseX = st.panX;
        panBaseY = st.panY;
    }

    // ---- pinch ----

    private void beginPinch() {
        pinching = true;
        pinchDist = Math.max(1, pts[0].dst(pts[1]));
        pinchMid.set(pts[0]).add(pts[1]).scl(0.5f);
        pinchZoom = st.zoom;
        panBaseX = st.panX;
        panBaseY = st.panY;
    }

    private void updatePinch() {
        float k = pts[0].dst(pts[1]) / pinchDist;
        float newZoom = Math.max(0.25f, Math.min(8f, pinchZoom * k));
        // keep the point under the fingers' midpoint fixed
        float cx = pinchMid.x - getWidth() / 2, cy = pinchMid.y - getHeight() / 2;
        float r = newZoom / pinchZoom;
        st.panX = cx - (cx - panBaseX) * r;
        st.panY = cy - (cy - panBaseY) * r;
        st.zoom = newZoom;
        st.fire();
    }

    private void endPinch() {
        if (!pinching) return;
        pinching = false;
        panBaseX = st.panX;
        panBaseY = st.panY;
    }

    // ---- camera stand-in: a frame on the stage (applied only when playing / exporting) ----

    private enum CamOp { MOVE, ZOOM, ROTATE }

    private CamOp camOp;
    private boolean camming;
    private float camX0, camY0, camZoom0, camRot0;
    private Vector2 fingerW0 = new Vector2();

    /** Camera frame corners in stage space (BL, BR, TR, TL), from the current camera or identity. */
    private float[] cameraFrame() {
        Project p = st.project();
        app.barkfart.eval.Affine inv = Evaluator.cameraView(p, p.root(), editTime()).inverse();
        float w = p.settings.width / 2f, h = p.settings.height / 2f;
        float[][] c = {{-w, -h}, {w, -h}, {w, h}, {-w, h}};
        float[] out = new float[8];
        for (int i = 0; i < 4; i++) {
            out[i * 2] = (float) inv.mapX(c[i][0], c[i][1]);
            out[i * 2 + 1] = (float) inv.mapY(c[i][0], c[i][1]);
        }
        return out;
    }

    /** Rotation knob: above the top edge's midpoint, 24 pt out. */
    private Vector2 knob(float[] f) {
        float mx = (f[4] + f[6]) / 2, my = (f[5] + f[7]) / 2;
        float cx = (f[0] + f[4]) / 2, cy = (f[1] + f[5]) / 2;
        float dx = mx - cx, dy = my - cy, len = (float) Math.max(1e-3, Math.hypot(dx, dy));
        float out = 24 / scale();
        return new Vector2(mx + dx / len * out, my + dy / len * out);
    }

    private CamOp pickCamera(float lx, float ly) {
        float[] f = cameraFrame();
        Vector2 v = toView(lx, ly);
        float r = Theme.HANDLE_HIT / scale();
        if (knob(f).dst(v) <= r) return CamOp.ROTATE;
        for (int i = 0; i < 4; i++) if (Math.hypot(f[i * 2] - v.x, f[i * 2 + 1] - v.y) <= r) return CamOp.ZOOM;
        return inside(f, v.x, v.y) ? CamOp.MOVE : null;
    }

    private static boolean inside(float[] f, float x, float y) {
        boolean in = false;
        for (int i = 0, j = 3; i < 4; j = i++) {
            float xi = f[i * 2], yi = f[i * 2 + 1], xj = f[j * 2], yj = f[j * 2 + 1];
            if ((yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) in = !in;
        }
        return in;
    }

    private void beginCamera() {
        camming = true;
        st.doc.begin("Camera");
        camDrag = camera(true);
        double t = editTime();
        camX0 = (Float) camDrag.x.keyValue(t);
        camY0 = (Float) camDrag.y.keyValue(t);
        camZoom0 = (Float) camDrag.zoom.keyValue(t);
        camRot0 = (Float) camDrag.rotation.keyValue(t);
        fingerW0.set(toView(start.x, start.y));
    }

    private void updateCamera(float lx, float ly) {
        Vector2 w = toView(lx, ly);
        double t = editTime();
        switch (camOp) {
            case MOVE:
                st.editor.change(camDrag.x, camX0 + (w.x - fingerW0.x), t);
                st.editor.change(camDrag.y, camY0 + (w.y - fingerW0.y), t);
                break;
            case ZOOM: {
                double d0 = Math.max(1e-3, Math.hypot(fingerW0.x - camX0, fingerW0.y - camY0));
                double d1 = Math.max(1e-3, Math.hypot(w.x - camX0, w.y - camY0));
                st.editor.change(camDrag.zoom, (float) Math.max(0.05, Math.min(20, camZoom0 * d0 / d1)), t);
                break;
            }
            default: {
                double a0 = Math.atan2(fingerW0.y - camY0, fingerW0.x - camX0), a1 = Math.atan2(w.y - camY0, w.x - camX0);
                double delta = Math.toDegrees(a1 - a0);
                delta -= 360 * Math.round(delta / 360);
                st.editor.change(camDrag.rotation, (float) (camRot0 + delta), t);
            }
        }
    }

    /** Topmost camera layer, created (undoably, inside the open gesture) if missing. */
    private Layer.CameraLayer camera(boolean create) {
        List<Layer> ls = st.project().root().layers;
        for (int i = ls.size() - 1; i >= 0; i--) if (ls.get(i) instanceof Layer.CameraLayer) return (Layer.CameraLayer) ls.get(i);
        if (!create) return null;
        Layer.CameraLayer c = new Layer.CameraLayer(st.project().newId("L"), "Camera");
        st.doc.apply(StructureEdits.addLayer(st.project().root(), c, ls.size()));
        return c;
    }

    @Override
    public void act(float delta) {
        super.act(delta);
        clock += delta;
        if (count() > 0) gestureTime += delta;
    }

    // ---- drawing ----

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Vector2 o = localToStageCoordinates(new Vector2(0, 0));
        Color old = batch.getColor().cpy();
        batch.setColor(Theme.SURROUND);
        batch.draw(theme.white(), o.x, o.y, getWidth(), getHeight());
        Project p = st.project();
        float s = scale(), sw = p.settings.width * s, sh = p.settings.height * s;
        float sx = o.x + getWidth() / 2 + st.panX - sw / 2, sy = o.y + getHeight() / 2 + st.panY - sh / 2;
        batch.setColor(Theme.STAGE);
        batch.draw(theme.white(), sx, sy, sw, sh);
        batch.setColor(old);
        batch.flush();

        Rectangle clip = new Rectangle(), area = new Rectangle(o.x, o.y, getWidth(), getHeight());
        ScissorStack.calculateScissors(getStage().getCamera(), batch.getTransformMatrix(), area, clip);
        boolean pushed = ScissorStack.pushScissors(clip);
        batch.end();

        Matrix4 m = new Matrix4(batch.getProjectionMatrix()).translate(o.x + getWidth() / 2 + st.panX, o.y + getHeight() / 2 + st.panY, 0).scale(s, s, 1);
        double t = st.time();
        if (texturesFor != p) {
            if (textures != null) textures.dispose();
            textures = new AssetTextures(p);
            texturesFor = p;
        }
        if (st.onion && !st.playing && !st.previewCamera) renderer.draw(onion.build(p, p.rootSymbol, t), m, null);
        // Editing shows the stage; the camera is a frame on it. Play shows the camera's shot.
        boolean shot = st.playing || st.previewCamera;
        Scene scene = Evaluator.evaluate(p, p.rootSymbol, t, false, shot);
        boolean shotClip = false;
        if (shot) {   // the shot is what export sees: clip to the stage rectangle
            Rectangle sc = new Rectangle();
            ScissorStack.calculateScissors(getStage().getCamera(), batch.getTransformMatrix(), new Rectangle(sx, sy, sw, sh), sc);
            shotClip = ScissorStack.pushScissors(sc);
        }
        // groups with effects or translucency draw offscreen; everything else draws as before
        effects.draw(Tessellator.layered(scene, glyphs), m, glyphs.with(textures));   // limited to the scissor set above
        if (shotClip) ScissorStack.popScissors();
        if (!shot) {
            drawCameraFrame(o);
            if (st.mode != EditorState.Mode.CAMERA) drawHandles(o);
        }

        if (pushed) ScissorStack.popScissors();
        batch.begin();
    }

    private void drawHandles(Vector2 o) {
        List<CanvasTools.Handle> hs = st.tools.handles(editTime());
        shapes.setProjectionMatrix(getStage().getCamera().combined);
        Gdx.gl.glEnable(GL20.GL_BLEND);
        shapes.begin(ShapeRenderer.ShapeType.Filled);
        for (CanvasTools.Handle h : hs) {
            if (st.mode == EditorState.Mode.FIGURE && st.selectedLayer != null && h.layer != st.selectedLayer) continue;
            float x = o.x + toLocalX(h.x), y = o.y + toLocalY(h.y);
            boolean sel = h.node == st.selectedNode;
            if (h.root) {
                float r = Theme.ROOT_HALF;
                shapes.setColor(0, 0, 0, 0.6f);
                shapes.rect(x - r - 1.5f, y - r - 1.5f, 2 * r + 3, 2 * r + 3);
                shapes.setColor(sel ? Theme.ACCENT : Theme.ROOT_HANDLE);
                shapes.rect(x - r, y - r, 2 * r, 2 * r);
                continue;
            }
            if (sel) {
                shapes.setColor(Theme.ACCENT.r, Theme.ACCENT.g, Theme.ACCENT.b, 0.35f);
                shapes.circle(x, y, Theme.JOINT_R + 4, 24);
                shapes.setColor(Theme.ACCENT);
                shapes.circle(x, y, Theme.JOINT_R, 20);
                continue;
            }
            // 7 pt white ring (1.5 pt wide) with a 1.5 pt black 60% outline, open centre
            ring(x, y, Theme.JOINT_R, Theme.JOINT_R + 1.5f, 0, 0, 0, 0.6f);
            ring(x, y, Theme.JOINT_R - 1.5f, Theme.JOINT_R, 1, 1, 1, 1);
            ring(x, y, Theme.JOINT_R - 3f, Theme.JOINT_R - 1.5f, 0, 0, 0, 0.6f);
        }
        shapes.end();
    }

    /** Filled annulus from radius r0 to r1. */
    private void ring(float x, float y, float r0, float r1, float r, float g, float b, float a) {
        shapes.setColor(r, g, b, a);
        int n = 20;
        r0 = Math.max(0, r0);
        for (int i = 0; i < n; i++) {
            double a0 = 2 * Math.PI * i / n, a1 = 2 * Math.PI * (i + 1) / n;
            float c0 = (float) Math.cos(a0), s0 = (float) Math.sin(a0), c1 = (float) Math.cos(a1), s1 = (float) Math.sin(a1);
            shapes.triangle(x + c0 * r0, y + s0 * r0, x + c0 * r1, y + s0 * r1, x + c1 * r1, y + s1 * r1);
            shapes.triangle(x + c0 * r0, y + s0 * r0, x + c1 * r1, y + s1 * r1, x + c1 * r0, y + s1 * r0);
        }
    }

    /** The camera stand-in: outline always (only when a camera exists), handles in Camera mode. */
    private void drawCameraFrame(Vector2 o) {
        boolean edit = st.mode == EditorState.Mode.CAMERA;
        if (camera(false) == null && !edit) return;
        float[] f = cameraFrame();
        float[] q = new float[8];
        for (int i = 0; i < 4; i++) {
            q[i * 2] = o.x + toLocalX(f[i * 2]);
            q[i * 2 + 1] = o.y + toLocalY(f[i * 2 + 1]);
        }
        shapes.setProjectionMatrix(getStage().getCamera().combined);
        Gdx.gl.glEnable(GL20.GL_BLEND);
        shapes.begin(ShapeRenderer.ShapeType.Filled);
        Color c = edit ? Theme.ACCENT : Theme.TEXT_DIM;
        shapes.setColor(c.r, c.g, c.b, edit ? 1f : 0.7f);
        float lw = edit ? 2 : 1.5f;
        for (int i = 0; i < 4; i++) {
            int j = (i + 1) % 4;
            shapes.rectLine(q[i * 2], q[i * 2 + 1], q[j * 2], q[j * 2 + 1], lw);
        }
        if (edit) {
            for (int i = 0; i < 4; i++) {
                shapes.setColor(Color.WHITE);
                shapes.rect(q[i * 2] - 5, q[i * 2 + 1] - 5, 10, 10);
                shapes.setColor(Theme.ACCENT);
                shapes.rect(q[i * 2] - 3.5f, q[i * 2 + 1] - 3.5f, 7, 7);
            }
            Vector2 k = knob(f);
            float kx = o.x + toLocalX(k.x), ky = o.y + toLocalY(k.y);
            float mx = (q[4] + q[6]) / 2, my = (q[5] + q[7]) / 2;
            shapes.setColor(Theme.ACCENT);
            shapes.rectLine(mx, my, kx, ky, 1.5f);
            shapes.circle(kx, ky, 6, 20);
            shapes.setColor(Color.WHITE);
            shapes.circle(kx, ky, 3, 16);
        }
        shapes.end();
        if (edit || camera(false) != null) {
            // label above the top-left corner
            com.badlogic.gdx.graphics.g2d.SpriteBatch b = labelBatch();
            b.setProjectionMatrix(getStage().getCamera().combined);
            b.begin();
            theme.small.setColor(c);
            theme.small.draw(b, "Camera", q[6] + 2, q[7] + 14);
            b.end();
        }
    }

    private com.badlogic.gdx.graphics.g2d.SpriteBatch labels;

    private com.badlogic.gdx.graphics.g2d.SpriteBatch labelBatch() {
        if (labels == null) labels = new com.badlogic.gdx.graphics.g2d.SpriteBatch(64);
        return labels;
    }

    public void dispose() {
        effects.dispose();
        glyphs.dispose();
        renderer.dispose();
        if (textures != null) textures.dispose();
        shapes.dispose();
        if (labels != null) labels.dispose();
    }
}
