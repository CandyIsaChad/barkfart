package app.barkfart.ui;

import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Property;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.Touchable;

/**
 * Frame strip (UI-SPEC "Frame strip"): 16:9 cells, tap to go, tap current or
 * long-press for the frame menu, momentum scroll, "+" cell, scroll bar.
 */
public final class FrameStrip extends Actor {
    public interface MenuOpener {
        void open(int frame, float stageX, float stageY);
    }

    private final Theme theme;
    private final EditorState st;
    private final MenuOpener menu;
    private final GlyphLayout gl = new GlyphLayout();
    private float scroll, velocity;
    private float downX, lastX, downTime;
    private boolean dragging, longFired, pressed;
    private int pressFrame = -1;
    private boolean[] keyed = new boolean[0];
    private boolean[] tweenOut = new boolean[0];

    public FrameStrip(Theme theme, EditorState st, MenuOpener menu) {
        this.theme = theme;
        this.st = st;
        this.menu = menu;
        setTouchable(Touchable.enabled);
        st.addListener(new Runnable() {
            public void run() {
                recount();
            }
        });
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (pointer != 0) return false;
                downX = lastX = x;
                downTime = 0;
                dragging = longFired = false;
                pressed = true;
                velocity = 0;
                pressFrame = cellAt(x);
                return true;
            }

            @Override
            public void touchDragged(InputEvent e, float x, float y, int pointer) {
                if (!dragging && Math.abs(x - downX) > Theme.DRAG_SLOP) dragging = true;
                if (dragging) {
                    float dx = x - lastX;
                    scroll = clamp(scroll - dx);
                    velocity = -dx / Math.max(1e-3f, Gdx.graphics.getDeltaTime());
                    lastX = x;
                }
            }

            @Override
            public void touchUp(InputEvent e, float x, float y, int pointer, int button) {
                pressed = false;
                if (dragging || longFired) return;
                velocity = 0;
                int f = cellAt(x);
                if (f == -2) {
                    st.frames.insertAfter(st.frames.frameCount() - 1, true);
                    st.setFrame(st.frames.frameCount() - 1);
                } else if (f >= 0) {
                    if (st.playing) st.stop();
                    if (f == st.frame) menu.open(f, e.getStageX(), e.getStageY());
                    else st.setFrame(f);
                }
            }
        });
    }

    private float contentWidth() {
        return (st.frames.frameCount() + 1) * (Theme.CELL_W + Theme.CELL_GAP) + Theme.CELL_GAP;
    }

    private float clamp(float s) {
        return Math.max(0, Math.min(s, Math.max(0, contentWidth() - getWidth())));
    }

    /** Frame index under x, -2 for the "+" cell, -1 for nothing. */
    private int cellAt(float x) {
        float cx = x + scroll - Theme.CELL_GAP;
        if (cx < 0) return -1;
        int i = (int) (cx / (Theme.CELL_W + Theme.CELL_GAP));
        if (cx - i * (Theme.CELL_W + Theme.CELL_GAP) > Theme.CELL_W) return -1;
        int n = st.frames.frameCount();
        return i < n ? i : (i == n ? -2 : -1);
    }

    /** Which frames have keys, and whether their keys tween to the next. */
    private void recount() {
        int n = st.frames.frameCount();
        keyed = new boolean[n];
        tweenOut = new boolean[n];
        double fps = st.editor.fps();
        for (Layer l : st.project().root().layers) {
            for (Property p : l.properties()) {
                for (Key k : p.track().keys()) {
                    int f = (int) Math.round(k.time * fps);
                    if (f >= 0 && f < n) {
                        keyed[f] = true;
                        if (k.interp != Interp.HOLD) tweenOut[f] = true;
                    }
                }
            }
        }
    }

    /** Keeps a frame visible (centred when it was off screen). */
    public void reveal(int f) {
        float x0 = Theme.CELL_GAP + f * (Theme.CELL_W + Theme.CELL_GAP);
        if (x0 < scroll || x0 + Theme.CELL_W > scroll + getWidth()) scroll = clamp(x0 - (getWidth() - Theme.CELL_W) / 2);
    }

    @Override
    public void act(float delta) {
        super.act(delta);
        if (pressed && !dragging && !longFired) {
            downTime += delta;
            if (downTime >= 0.4f && pressFrame >= 0) {
                longFired = true;
                Gdx.input.vibrate(10);
                if (st.playing) st.stop();
                st.setFrame(pressFrame);
                float sx = localToStageCoordinates(new com.badlogic.gdx.math.Vector2(downX, getHeight() / 2)).x;
                menu.open(pressFrame, sx, getY());
            }
        }
        if (!pressed && Math.abs(velocity) > 1) {
            scroll = clamp(scroll + velocity * delta);
            velocity *= Math.pow(0.02, delta);   // momentum decay
        }
        if (st.playing) reveal(st.playingFrame());
    }

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Color old = batch.getColor().cpy();
        float x0 = getX(), y0 = getY(), w = getWidth(), h = getHeight();
        batch.setColor(Theme.PANEL);
        batch.draw(theme.white(), x0, y0, w, h);
        batch.flush();
        com.badlogic.gdx.math.Rectangle clip = new com.badlogic.gdx.math.Rectangle();
        com.badlogic.gdx.scenes.scene2d.utils.ScissorStack.calculateScissors(getStage().getCamera(), batch.getTransformMatrix(),
                new com.badlogic.gdx.math.Rectangle(x0, y0, w, h), clip);
        boolean clipped = com.badlogic.gdx.scenes.scene2d.utils.ScissorStack.pushScissors(clip);
        float cy = y0 + h - 6 - Theme.CELL_H;
        int n = st.frames.frameCount();
        int current = st.playing ? st.playingFrame() : st.frame;
        int first = Math.max(0, (int) ((scroll - Theme.CELL_GAP) / (Theme.CELL_W + Theme.CELL_GAP)));
        for (int i = first; i <= n; i++) {
            float cx = x0 + Theme.CELL_GAP + i * (Theme.CELL_W + Theme.CELL_GAP) - scroll;
            if (cx > x0 + w) break;
            boolean plus = i == n;
            boolean isKey = !plus && i < keyed.length && keyed[i];
            batch.setColor(plus ? Theme.CONTROL : (isKey ? Theme.CELL : Theme.CELL_TWEEN));
            batch.draw(theme.white(), cx, cy, Theme.CELL_W, Theme.CELL_H);
            if (plus) {
                theme.title.setColor(Theme.TEXT_DIM);
                gl.setText(theme.title, "+");
                theme.title.draw(batch, gl, cx + (Theme.CELL_W - gl.width) / 2, cy + Theme.CELL_H / 2 + gl.height / 2);
                continue;
            }
            theme.small.setColor(isKey ? Theme.TEXT : Theme.TEXT_DIM);
            theme.small.draw(batch, Integer.toString(i + 1), cx + 6, cy + Theme.CELL_H - 6);
            if (!isKey && i > 0) {
                // in-between frame: tween-arrow badge along the bottom (shaft + triangular head)
                batch.setColor(Theme.TEXT_DIM);
                float ax = cx + Theme.CELL_W - 28, ay = cy + 9;
                batch.draw(theme.white(), ax, ay - 0.75f, 13, 1.5f);
                for (int k = 0; k < 6; k++) {
                    float hh = 6 - k;
                    batch.draw(theme.white(), ax + 13 + k, ay - hh / 2, 1, hh);
                }
            }
            if (i == current) {
                batch.setColor(Theme.ACCENT);
                float t = 2;
                batch.draw(theme.white(), cx, cy, Theme.CELL_W, t);
                batch.draw(theme.white(), cx, cy + Theme.CELL_H - t, Theme.CELL_W, t);
                batch.draw(theme.white(), cx, cy, t, Theme.CELL_H);
                batch.draw(theme.white(), cx + Theme.CELL_W - t, cy, t, Theme.CELL_H);
            }
        }
        batch.flush();
        if (clipped) com.badlogic.gdx.scenes.scene2d.utils.ScissorStack.popScissors();
        // scroll indicator
        float cw = contentWidth();
        if (cw > w) {
            float bw = Math.max(24, w * w / cw), bx = x0 + (w - bw) * (scroll / (cw - w));
            batch.setColor(Theme.SEGMENT_EDGE);
            batch.draw(theme.white(), bx, y0 + 3, bw, 3);
        }
        batch.setColor(old);
    }
}
