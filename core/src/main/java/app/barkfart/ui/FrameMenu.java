package app.barkfart.ui;

import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;

/** Frame menu (UI-SPEC): Insert after, Duplicate, Delete, Hold/Tween to next. Tap outside closes. */
public final class FrameMenu extends WidgetGroup {
    private final Theme theme;
    private final EditorState st;
    private final NinePatchDrawable bg;
    private final UiButton insert, duplicate, delete, tween;
    private int frame;
    private float mx, my, mw = 168, mh;

    public FrameMenu(Theme theme, final EditorState st) {
        this.theme = theme;
        this.st = st;
        bg = theme.box(Theme.CONTROL_DOWN);
        insert = new UiButton(theme, "Insert after", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.frames.insertAfter(frame, false);
                st.setFrame(frame + 1);
                close();
            }
        });
        duplicate = new UiButton(theme, "Duplicate", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                st.frames.insertAfter(frame, true);
                st.setFrame(frame + 1);
                close();
            }
        });
        tween = new UiButton(theme, "Hold to next", UiButton.Style.NORMAL, null);
        delete = new UiButton(theme, "Delete frame", UiButton.Style.DANGER, new Runnable() {
            public void run() {
                final int f = frame;
                if (st.frames.delete(f)) {
                    st.setFrame(Math.min(f, st.frames.frameCount() - 1));
                    st.flash("Deleted frame " + (f + 1) + " · Undo", new Runnable() {
                        public void run() {
                            st.doc.undo();
                        }
                    });
                }
                close();
            }
        });
        for (UiButton b : new UiButton[] {insert, duplicate, tween, delete}) addActor(b);
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (x < mx || x > mx + mw || y < my || y > my + mh) close();
                return true;   // modal: swallow touches
            }
        });
        setVisible(false);
    }

    public void open(int frame, float stageX, float stageY) {
        this.frame = frame;
        final boolean tweening = st.frames.hasKeys(frame) && isTween(frame);
        tween.setText(tweening ? "Hold to next" : "Tween to next");
        tween.enabled = st.frames.hasKeys(frame);
        tween.setAction(new Runnable() {
            public void run() {
                st.frames.setTween(FrameMenu.this.frame, !tweening);
                close();
            }
        });
        delete.enabled = st.frames.frameCount() > 1;
        mh = 4 * Theme.CONTROL_H + 3 * 4 + 2 * 8;
        mx = Math.max(8, Math.min(stageX - mw / 2, getWidth() - mw - 8));
        my = Math.max(8, stageY - mh - 8);
        float y = my + mh - 8 - Theme.CONTROL_H;
        for (UiButton b : new UiButton[] {insert, duplicate, tween, delete}) {
            b.setBounds(mx + 8, y, mw - 16, Theme.CONTROL_H);
            y -= Theme.CONTROL_H + 4;
        }
        setVisible(true);
        toFront();
    }

    private boolean isTween(int f) {
        double t = st.frames.time(f);
        for (app.barkfart.model.Layer l : st.project().root().layers) {
            for (app.barkfart.model.Property p : l.properties()) {
                for (app.barkfart.model.Key k : p.track().keys()) {
                    if (Math.abs(k.time - t) < 1e-6) return k.interp != app.barkfart.model.Interp.HOLD;
                }
            }
        }
        return false;
    }

    public void close() {
        setVisible(false);
    }

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Color c = batch.getColor().cpy();
        batch.setColor(0, 0, 0, 0.35f);
        batch.draw(theme.white(), getX(), getY(), getWidth(), getHeight());
        batch.setColor(c);
        bg.draw(batch, getX() + mx, getY() + my, mw, mh);
        super.draw(batch, parentAlpha);
    }
}
