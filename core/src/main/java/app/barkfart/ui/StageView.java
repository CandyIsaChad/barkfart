package app.barkfart.ui;

import app.barkfart.canvas.CanvasTools;
import app.barkfart.edit.StructureEdits;
import app.barkfart.eval.Evaluator;
import app.barkfart.eval.Scene;
import app.barkfart.geom.OnionSkin;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.render.DrawListRenderer;
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
    private final DrawListRenderer renderer = new DrawListRenderer();
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
    private float camStartX, camStartY, camStartZoom;
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
                    pending = st.playing ? null : pick(x, y);
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
                if (!dragging && !panning && start.dst(x, y) > Theme.DRAG_SLOP) {
                    if (pending != null) beginDrag();
                    else beginPan();
                }
                if (dragging) {
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
                    boolean tap = gestureTime < 0.3f && !dragging && !panning && !pinching
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
        if (st.mode == EditorState.Mode.CAMERA) {
            st.doc.begin("Move camera");
            camDrag = camera(true);
            camStartX = (Float) camDrag.x.keyValue(editTime());
            camStartY = (Float) camDrag.y.keyValue(editTime());
        }
    }

    private void updatePan(float x, float y) {
        if (st.mode == EditorState.Mode.CAMERA && camDrag != null) {
            float s = scale(), z = (Float) camDrag.zoom.keyValue(editTime());
            double rot = Math.toRadians((Float) camDrag.rotation.keyValue(editTime()));
            float dx = (x - start.x) / s / z, dy = (y - start.y) / s / z;
            float wx = (float) (dx * Math.cos(rot) - dy * Math.sin(rot)), wy = (float) (dx * Math.sin(rot) + dy * Math.cos(rot));
            st.editor.change(camDrag.x, camStartX - wx, editTime());
            st.editor.change(camDrag.y, camStartY - wy, editTime());
        } else {
            st.panX = panBaseX + (x - start.x);
            st.panY = panBaseY + (y - start.y);
            st.fire();
        }
    }

    private float panBaseX, panBaseY;

    private void endSingle(boolean cancelled) {
        if (dragging) {
            if (cancelled) st.tools.cancel();
            else st.tools.end();
        }
        if (panning && st.mode == EditorState.Mode.CAMERA && camDrag != null) st.doc.commit();
        dragging = panning = false;
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
        if (st.mode == EditorState.Mode.CAMERA) {
            st.doc.begin("Zoom camera");
            camDrag = camera(true);
            camStartZoom = (Float) camDrag.zoom.keyValue(editTime());
        }
    }

    private void updatePinch() {
        float k = pts[0].dst(pts[1]) / pinchDist;
        if (st.mode == EditorState.Mode.CAMERA && camDrag != null) {
            st.editor.change(camDrag.zoom, Math.max(0.05f, Math.min(20f, camStartZoom * k)), editTime());
            return;
        }
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
        if (st.mode == EditorState.Mode.CAMERA && camDrag != null) {
            st.doc.commit();
            camDrag = null;
        }
        panBaseX = st.panX;
        panBaseY = st.panY;
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
        if (st.onion && !st.playing) renderer.draw(onion.build(p, p.rootSymbol, t), m);
        Scene scene = Evaluator.evaluate(p, p.rootSymbol, t);
        renderer.draw(Tessellator.tessellate(scene), m);
        if (!st.playing && st.mode != EditorState.Mode.CAMERA) drawHandles(o);
        if (st.mode == EditorState.Mode.CAMERA) drawCameraBorder(o);

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

    private void drawCameraBorder(Vector2 o) {
        shapes.setProjectionMatrix(getStage().getCamera().combined);
        shapes.begin(ShapeRenderer.ShapeType.Filled);
        shapes.setColor(Theme.ACCENT);
        float t = 2, w = getWidth(), h = getHeight(), in = 2;
        shapes.rect(o.x + in, o.y + in, w - 2 * in, t);
        shapes.rect(o.x + in, o.y + h - in - t, w - 2 * in, t);
        shapes.rect(o.x + in, o.y + in, t, h - 2 * in);
        shapes.rect(o.x + w - in - t, o.y + in, t, h - 2 * in);
        shapes.end();
    }

    public void dispose() {
        renderer.dispose();
        shapes.dispose();
    }
}
