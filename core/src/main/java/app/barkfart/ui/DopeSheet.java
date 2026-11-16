package app.barkfart.ui;

import app.barkfart.edit.DopeRows;
import app.barkfart.edit.KeyRef;
import app.barkfart.edit.StructureEdits;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.math.Rectangle;
import com.badlogic.gdx.math.Vector2;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.Touchable;
import com.badlogic.gdx.scenes.scene2d.utils.ScissorStack;
import java.util.ArrayList;
import java.util.List;

/** The dope sheet (UI-SPEC "Dope sheet"): ruler, playhead, rows of keys, selection, retime. */
public final class DopeSheet extends Actor {
    public static final float RULER = 18, ROW = 22, LABEL_W = 72, GRIP = 12, COLLAPSED = 72;

    public interface Resized {
        void resized();
    }

    private final Theme theme;
    private final EditorState st;
    private final Resized onResize;
    private final GlyphLayout gl = new GlyphLayout();
    private List<DopeRows.Row> rows = new ArrayList<DopeRows.Row>();

    private float ppf = 14;          // points per frame
    private float scrollF;           // first visible frame (fractional)
    private float scrollRows;        // vertical scroll in points

    // gesture state
    private enum Mode { NONE, SCRUB, LENGTH, RESIZE, KEY, SCROLL, MARQUEE, LABEL, PINCH }

    private Mode mode = Mode.NONE;
    private final Vector2 down = new Vector2(), last = new Vector2(), p1 = new Vector2();
    private boolean moved, longArmed;
    private float held;
    private float pinchDist, pinchPpf, pinchFrame;
    private int dragDelta;
    private float startHeight;
    private DopeRows.Row pressRow;
    private double pressTime;
    private final Rectangle marquee = new Rectangle();
    private float clock, lastEmptyTap = -10;

    public DopeSheet(Theme theme, EditorState st, Resized onResize) {
        this.theme = theme;
        this.st = st;
        this.onResize = onResize;
        setTouchable(Touchable.enabled);
        st.addListener(new Runnable() {
            public void run() {
                rebuild();
            }
        });
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (pointer == 1 && mode != Mode.KEY && mode != Mode.MARQUEE) {
                    p1.set(x, y);
                    mode = Mode.PINCH;
                    pinchDist = Math.max(1, last.dst(p1));
                    pinchPpf = ppf;
                    pinchFrame = frameAtX((last.x + p1.x) / 2);
                    return true;
                }
                if (pointer != 0) return false;
                down.set(x, y);
                last.set(x, y);
                moved = false;
                held = 0;
                longArmed = false;
                dragDelta = 0;
                mode = classify(x, y);
                if (mode == Mode.SCRUB) scrubTo(x);
                if (mode == Mode.RESIZE) startHeight = st.sheetHeight;
                if (mode == Mode.LENGTH) st.doc.begin("Length");
                return true;
            }

            @Override
            public void touchDragged(InputEvent e, float x, float y, int pointer) {
                if (pointer == 1) {
                    p1.set(x, y);
                    if (mode == Mode.PINCH) pinch();
                    return;
                }
                if (pointer != 0) return;
                if (!moved && down.dst(x, y) > Theme.DRAG_SLOP) {
                    moved = true;
                    if (mode == Mode.NONE) mode = longArmed ? Mode.MARQUEE : Mode.SCROLL;
                    if (mode == Mode.KEY) beginKeyDrag();
                }
                float dx = x - last.x, dy = y - last.y;
                switch (mode) {
                    case SCRUB: scrubTo(x); break;
                    case LENGTH: setLength(x); break;
                    case RESIZE:
                        st.sheetHeight = Math.max(COLLAPSED, Math.min(maxHeight(), startHeight + (down.y - y)));
                        onResize.resized();
                        break;
                    case SCROLL:
                        scrollF = Math.max(0, scrollF - dx / ppf);
                        scrollRows = Math.max(0, Math.min(maxRowScroll(), scrollRows + dy));
                        break;
                    case KEY:
                        if (moved) dragDelta = Math.round((x - down.x) / ppf);
                        break;
                    case MARQUEE:
                        marquee.set(Math.min(down.x, x), Math.min(down.y, y), Math.abs(x - down.x), Math.abs(y - down.y));
                        break;
                    default:
                }
                last.set(x, y);
            }

            @Override
            public void touchUp(InputEvent e, float x, float y, int pointer, int button) {
                if (pointer == 1) {
                    if (mode == Mode.PINCH) mode = Mode.SCROLL;
                    return;
                }
                if (pointer != 0) return;
                switch (mode) {
                    case LENGTH: st.doc.commit(); break;
                    case KEY:
                        if (moved) endKeyDrag();
                        else tapKey();
                        break;
                    case MARQUEE: endMarquee(); break;
                    case LABEL: if (!moved) tapLabel(x, y); break;
                    case NONE:
                        if (!moved) {
                            if (clock - lastEmptyTap < 0.3f) {      // double-tap: toggle multi-select mode
                                st.multiSelect = !st.multiSelect;
                                lastEmptyTap = -10;
                                st.flash(st.multiSelect ? "Multi-select: tap keys to add or remove" : "Multi-select off", null);
                                break;
                            }
                            lastEmptyTap = clock;
                            if (!st.multiSelect) st.selectedKeys.clear();   // tap empty: clear, move the playhead
                            scrubTo(x);
                        }
                        break;
                    default:
                }
                mode = Mode.NONE;
                marquee.set(0, 0, 0, 0);
            }
        });
    }

    private float maxHeight() {
        return getStage() == null ? 300 : getStage().getHeight() * 0.6f;
    }

    private void rebuild() {
        rows = DopeRows.build(st.project(), st.sheetShowAll ? null : st.selectedLayer, st.collapsedFigures);
        scrollRows = Math.max(0, Math.min(maxRowScroll(), scrollRows));
    }

    private float maxRowScroll() {
        return Math.max(0, rows.size() * ROW - (getHeight() - RULER - GRIP));
    }

    // ---- geometry (local coords, y up) ----

    private float xOfFrame(float f) {
        return LABEL_W + (f - scrollF) * ppf + ppf / 2;
    }

    private float frameAtX(float x) {
        return (x - LABEL_W - ppf / 2) / ppf + scrollF;
    }

    private float rowTop(int i) {
        return getHeight() - RULER - i * ROW + scrollRows;
    }

    private int rowAt(float y) {
        float fromTop = getHeight() - RULER - y + scrollRows;
        if (fromTop < 0) return -1;
        int i = (int) (fromTop / ROW);
        return i < rows.size() ? i : -1;
    }

    private float endX() {
        return xOfFrame(st.frames.frameCount() - 0.5f);
    }

    private Mode classify(float x, float y) {
        if (y < GRIP) return Mode.RESIZE;
        if (y > getHeight() - RULER) {
            if (x < LABEL_W) return Mode.LABEL;
            if (Math.abs(x - endX()) < 12) return Mode.LENGTH;
            return Mode.SCRUB;
        }
        if (x < LABEL_W) return Mode.LABEL;
        int r = rowAt(y);
        if (r >= 0) {
            DopeRows.Row row = rows.get(r);
            double fps = st.editor.fps();
            float bestD = Math.max(ppf / 2, 11);
            Double best = null;
            for (double t : row.times()) {
                float d = Math.abs(xOfFrame((float) (t * fps)) - x);
                if (d <= bestD) {
                    bestD = d;
                    best = t;
                }
            }
            if (best != null) {
                pressRow = row;
                pressTime = best;
                return Mode.KEY;
            }
        }
        longArmed = false;
        return Mode.NONE;
    }

    private void scrubTo(float x) {
        int f = Math.max(0, Math.min(st.frames.frameCount() - 1, Math.round(frameAtX(x))));
        if (st.playing) st.stop();
        if (f != st.frame) st.setFrame(f);
        else st.fire();
    }

    private void setLength(float x) {
        int frames = Math.max(1, Math.round(frameAtX(x) + 0.5f));
        double dur = frames / st.editor.fps();
        if (Math.abs(dur - st.project().root().duration) > 1e-9) {
            st.doc.apply(StructureEdits.setDuration(st.project().root(), dur));
        }
    }

    private void pinch() {
        float k = last.dst(p1) / pinchDist;
        ppf = Math.max(3, Math.min(60, pinchPpf * k));
        float mid = (last.x + p1.x) / 2;
        scrollF = Math.max(0, pinchFrame - (mid - LABEL_W - ppf / 2) / ppf);
    }

    // ---- keys ----

    private List<KeyRef> pressKeys() {
        return pressRow.keysAt(pressTime, 1e-9);
    }

    private void tapKey() {
        List<KeyRef> ks = pressKeys();
        if (st.multiSelect) {
            if (st.selectedKeys.containsAll(ks)) {
                st.selectedKeys.removeAll(ks);
                st.fire();
            } else {
                st.selectKeys(ks, true);
            }
            return;
        }
        st.selectKeys(ks, false);
        st.setFrame(st.frames.frameAt(pressTime));
    }

    private void beginKeyDrag() {
        List<KeyRef> ks = pressKeys();
        if (!st.selectedKeys.containsAll(ks)) st.selectKeys(ks, false);
    }

    private void endKeyDrag() {
        if (dragDelta == 0) return;
        double delta = dragDelta / st.editor.fps();
        List<KeyRef> sel = new ArrayList<KeyRef>(st.selectedKeys);
        st.editor.retime(sel, delta);
        java.util.Set<KeyRef> moved = DopeRows.shifted(st.selectedKeys, delta);
        st.selectedKeys.clear();
        st.selectedKeys.addAll(moved);
        dragDelta = 0;
        st.fire();
    }

    private void endMarquee() {
        if (marquee.width <= 0) return;
        int r0 = rowAt(marquee.y + marquee.height), r1 = rowAt(marquee.y);
        if (r0 < 0) r0 = 0;
        if (r1 < 0) r1 = rows.size() - 1;
        double fps = st.editor.fps();
        double t0 = Math.ceil(frameAtX(marquee.x) - 0.5) / fps, t1 = Math.floor(frameAtX(marquee.x + marquee.width) + 0.5) / fps;
        st.selectKeys(DopeRows.keysIn(rows.subList(Math.min(r0, r1), Math.max(r0, r1) + 1), t0, t1), st.multiSelect);
    }

    private void tapLabel(float x, float y) {
        if (y > getHeight() - RULER) {           // filter toggle
            st.sheetShowAll = !st.sheetShowAll;
            st.fire();
            return;
        }
        int r = rowAt(y);
        if (r < 0) return;
        DopeRows.Row row = rows.get(r);
        if (row.kind == DopeRows.Kind.FIGURE) {
            if (!st.collapsedFigures.remove(row.layer.id)) st.collapsedFigures.add(row.layer.id);
            st.fire();
        } else {
            st.selectKeys(DopeRows.keysIn(rows.subList(r, r + 1), -1e9, 1e9), false);
        }
    }

    @Override
    public void act(float delta) {
        super.act(delta);
        clock += delta;
        if (Gdx.input.isTouched(0) && mode == Mode.NONE && !moved) {
            held += delta;
            if (held > 0.4f && !longArmed) {
                longArmed = true;
                Gdx.input.vibrate(10);
            }
        }
        if (st.playing) {
            float f = st.playingFrame();
            float visible = (getWidth() - LABEL_W) / ppf;
            if (f < scrollF || f > scrollF + visible - 1) scrollF = Math.max(0, f - visible / 2);
        }
    }

    // ---- drawing ----

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Color old = batch.getColor().cpy();
        float x0 = getX(), y0 = getY(), w = getWidth(), h = getHeight();
        batch.setColor(Theme.PANEL);
        batch.draw(theme.white(), x0, y0, w, h);
        batch.flush();
        Rectangle clip = new Rectangle();
        ScissorStack.calculateScissors(getStage().getCamera(), batch.getTransformMatrix(), new Rectangle(x0, y0, w, h), clip);
        boolean pushed = ScissorStack.pushScissors(clip);

        double fps = st.editor.fps();
        int n = st.frames.frameCount();
        float keyTop = y0 + h - RULER, keyBottom = y0 + GRIP;
        // key area background, beyond-the-end shading
        batch.setColor(Theme.CELL_TWEEN);
        batch.draw(theme.white(), x0 + LABEL_W, keyBottom, w - LABEL_W, keyTop - keyBottom);
        float ex = x0 + endX() + ppf / 2;
        if (ex < x0 + w) {
            batch.setColor(Theme.SURROUND);
            batch.draw(theme.white(), Math.max(ex, x0 + LABEL_W), keyBottom, x0 + w - Math.max(ex, x0 + LABEL_W), keyTop - keyBottom + RULER);
        }
        // alternating rows
        for (int i = 0; i < rows.size(); i++) {
            float top = y0 + rowTop(i);
            if (top < keyBottom || top - ROW > keyTop) continue;
            if (i % 2 == 1) {
                batch.setColor(Theme.CELL);
                batch.draw(theme.white(), x0 + LABEL_W, top - ROW, Math.min(w - LABEL_W, ex - x0 - LABEL_W), ROW);
            }
        }
        // frame grid and ruler numbers
        int step = 1;
        for (int s : new int[] {1, 2, 5, 10, 20, 50, 100}) {
            step = s;
            if (s * ppf >= 28) break;
        }
        int f0 = Math.max(0, (int) scrollF), f1 = (int) (scrollF + (w - LABEL_W) / ppf) + 1;
        for (int f = f0; f <= f1; f++) {
            float fx = x0 + xOfFrame(f);
            if (fx < x0 + LABEL_W) continue;
            boolean major = f % step == 0;
            if (major) {
                batch.setColor(Theme.CONTROL);
                batch.draw(theme.white(), fx - 0.5f, keyBottom, 1, keyTop - keyBottom);
            }
            batch.setColor(major ? Theme.TEXT_DIM : Theme.SEGMENT_EDGE);
            batch.draw(theme.white(), fx - 0.5f, keyTop, 1, major ? 5 : 2);
            if (major) {
                theme.small.setColor(Theme.TEXT_DIM);
                gl.setText(theme.small, Integer.toString(f + 1));
                theme.small.draw(batch, gl, fx - gl.width / 2, y0 + h - 2);
            }
        }
        // length handle
        batch.setColor(Theme.TEXT_DIM);
        batch.draw(theme.white(), ex - 1, keyTop, 2, RULER);
        // labels column
        batch.setColor(Theme.PANEL);
        batch.draw(theme.white(), x0, y0, LABEL_W, h);
        theme.small.setColor(Theme.TEXT_DIM);
        theme.small.draw(batch, st.sheetShowAll ? "All ▾" : "Selected ▾", x0 + 6, y0 + h - 4, LABEL_W - 8,
                com.badlogic.gdx.utils.Align.left, true);
        for (int i = 0; i < rows.size(); i++) {
            float top = y0 + rowTop(i);
            if (top < keyBottom || top - ROW > keyTop) continue;
            DopeRows.Row r = rows.get(i);
            boolean header = r.kind != DopeRows.Kind.JOINT && r.kind != DopeRows.Kind.MOVE;
            String label = r.kind == DopeRows.Kind.FIGURE
                    ? (st.collapsedFigures.contains(r.layer.id) ? "▸ " : "▾ ") + r.label : r.label;
            (header ? theme.small : theme.small).setColor(header ? Theme.TEXT : Theme.TEXT_DIM);
            theme.small.draw(batch, label, x0 + 6 + r.depth * 8, top - 5, LABEL_W - 10 - r.depth * 8,
                    com.badlogic.gdx.utils.Align.left, false);
        }
        // keys
        float d = 8;
        for (int i = 0; i < rows.size(); i++) {
            float top = y0 + rowTop(i);
            if (top < keyBottom || top - ROW > keyTop) continue;
            DopeRows.Row r = rows.get(i);
            boolean dim = r.kind == DopeRows.Kind.SUMMARY || r.kind == DopeRows.Kind.FIGURE;
            for (double t : r.times()) {
                boolean sel = false;
                for (KeyRef k : r.keysAt(t, 1e-9)) {
                    if (st.selectedKeys.contains(k)) {
                        sel = true;
                        break;
                    }
                }
                float f = (float) (t * fps) + (sel && mode == Mode.KEY ? dragDelta : 0);
                float kx = x0 + xOfFrame(f);
                if (kx < x0 + LABEL_W - d || kx > x0 + w + d) continue;
                batch.setColor(sel ? Theme.ACCENT : (dim ? Theme.TEXT_DIM : Theme.TEXT));
                batch.draw(theme.diamond(), kx - d / 2, top - ROW / 2 - d / 2, d, d);
            }
        }
        // playhead
        int cur = st.playing ? st.playingFrame() : st.frame;
        float px = x0 + xOfFrame(cur);
        if (px >= x0 + LABEL_W) {
            batch.setColor(Theme.ACCENT);
            batch.draw(theme.white(), px - 1, keyBottom, 2, keyTop - keyBottom + RULER);
            batch.draw(theme.white(), px - 5, y0 + h - 6, 10, 6);
        }
        // marquee
        if (mode == Mode.MARQUEE && marquee.width > 0) {
            batch.setColor(Theme.ACCENT.r, Theme.ACCENT.g, Theme.ACCENT.b, 0.18f);
            batch.draw(theme.white(), x0 + marquee.x, y0 + marquee.y, marquee.width, marquee.height);
        }
        // grip
        batch.setColor(Theme.SEGMENT_EDGE);
        batch.draw(theme.white(), x0 + w / 2 - 18, y0 + 4, 36, 3);
        batch.flush();
        if (pushed) ScissorStack.popScissors();
        batch.setColor(old);
    }
}
