package app.barkfart.ui;

import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.Touchable;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.scenes.scene2d.utils.ClickListener;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;

/** Easing dropdown: presets with a curve preview each (UI feedback 2026-11-17). Modal. */
public final class EasingMenu extends WidgetGroup {
    public static final class Preset {
        public final String name;
        public final Interp interp;
        public final Ease ease;

        Preset(String name, Interp interp, Ease ease) {
            this.name = name;
            this.interp = interp;
            this.ease = ease;
        }

        /** Value fraction at time fraction u (for drawing). */
        public float at(float u) {
            if (interp == Interp.HOLD) return u < 1 ? 0 : 1;
            if (interp == Interp.LINEAR) return u;
            return ease.apply(u);
        }
    }

    public static final Preset[] PRESETS = {
        new Preset("Hold", Interp.HOLD, Ease.LINEAR),
        new Preset("Linear", Interp.LINEAR, Ease.LINEAR),
        new Preset("Ease in-out", Interp.BEZIER, Ease.EASE_IN_OUT),
        new Preset("Ease in", Interp.BEZIER, Ease.EASE_IN),
        new Preset("Ease out", Interp.BEZIER, Ease.EASE_OUT),
        new Preset("Sine in-out", Interp.BEZIER, Ease.SINE_IN_OUT),
        new Preset("Strong in-out", Interp.BEZIER, Ease.STRONG_IN_OUT),
        new Preset("Anticipate", Interp.BEZIER, Ease.ANTICIPATE),
        new Preset("Overshoot", Interp.BEZIER, Ease.OVERSHOOT),
        new Preset("Anticipate + overshoot", Interp.BEZIER, Ease.ANTICIPATE_OVERSHOOT),
        new Preset("Bounce", Interp.BEZIER, Ease.BOUNCE_OUT),
        new Preset("Bounce in", Interp.BEZIER, Ease.BOUNCE_IN),
        new Preset("Elastic", Interp.BEZIER, Ease.ELASTIC_OUT),
        new Preset("Elastic in", Interp.BEZIER, Ease.ELASTIC_IN),
    };

    /** Which preset a key's interpolation matches, or null. */
    public static Preset match(Interp in, Ease e) {
        for (Preset p : PRESETS) {
            if (p.interp != in) continue;
            if (in != Interp.BEZIER || p.ease.equals(e)) return p;
        }
        return null;
    }

    public interface Chosen {
        void chosen(Preset p);
    }

    static final float ROW_H = 44, GRAPH_W = 48, MENU_W = 232;

    private final Theme theme;
    private final NinePatchDrawable bg;
    private float mx, my, mh;
    private Chosen chosen;
    private Preset current;

    public EasingMenu(Theme theme) {
        this.theme = theme;
        bg = theme.box(Theme.CONTROL_DOWN);
        for (final Preset p : PRESETS) {
            Actor row = new Actor() {
                final ClickListener click = new ClickListener() {
                    @Override
                    public void clicked(InputEvent event, float x, float y) {
                        if (chosen != null) chosen.chosen(p);
                        close();
                    }
                };

                {
                    setTouchable(Touchable.enabled);
                    addListener(click);
                }

                @Override
                public void draw(Batch batch, float a) {
                    Color old = batch.getColor().cpy();
                    if (click.isPressed() || p == current) {
                        batch.setColor(p == current ? Theme.SEGMENT_ON : Theme.CONTROL);
                        batch.draw(EasingMenu.this.theme.white(), getX(), getY(), getWidth(), getHeight());
                    }
                    drawCurve(batch, EasingMenu.this.theme, p, getX() + 8, getY() + 6, GRAPH_W, getHeight() - 12,
                            p == current ? Theme.ACCENT : Theme.TEXT);
                    EasingMenu.this.theme.body.setColor(Theme.TEXT);
                    EasingMenu.this.theme.body.draw(batch, p.name, getX() + GRAPH_W + 20, getY() + getHeight() / 2 + 6);
                    batch.setColor(old);
                }
            };
            row.setUserObject(p);
            addActor(row);
        }
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (x < mx || x > mx + MENU_W || y < my || y > my + mh) close();
                return true;   // modal
            }
        });
        setVisible(false);
    }

    /** Draws a preset's curve as a dotted polyline in a box (y range -0.7 .. 1.7 to fit overshoot). */
    static void drawCurve(Batch batch, Theme theme, Preset p, float x, float y, float w, float h, Color c) {
        Color old = batch.getColor().cpy();
        float lo = -0.7f, hi = 1.7f;
        batch.setColor(Theme.SEGMENT_EDGE);
        float y0 = y + (0 - lo) / (hi - lo) * h, y1 = y + (1 - lo) / (hi - lo) * h;
        batch.draw(theme.white(), x, y0, w, 1);
        batch.draw(theme.white(), x, y1, w, 1);
        batch.setColor(c);
        int n = 48;
        float px = x, py = y0;
        for (int i = 1; i <= n; i++) {
            float u = i / (float) n;
            float qx = x + u * w, qy = y + (p.at(u) - lo) / (hi - lo) * h;
            // segment as a chain of 1.5 pt dots
            float d = (float) Math.hypot(qx - px, qy - py);
            int steps = Math.max(1, (int) (d / 0.75f));
            for (int k = 0; k < steps; k++) {
                float t = k / (float) steps;
                batch.draw(theme.white(), px + (qx - px) * t - 0.75f, py + (qy - py) * t - 0.75f, 1.5f, 1.5f);
            }
            px = qx;
            py = qy;
        }
        batch.setColor(old);
    }

    public void open(float stageX, float stageY, Preset current, Chosen chosen) {
        this.current = current;
        this.chosen = chosen;
        mh = PRESETS.length * ROW_H + 8;
        mx = Math.max(8, Math.min(stageX, getWidth() - MENU_W - 8));
        my = Math.max(8, Math.min(stageY - mh / 2, getHeight() - mh - 8));
        float y = my + mh - 4 - ROW_H;
        for (Actor a : getChildren()) {
            a.setBounds(mx + 4, y, MENU_W - 8, ROW_H);
            y -= ROW_H;
        }
        setVisible(true);
        toFront();
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
        bg.draw(batch, getX() + mx, getY() + my, MENU_W, mh);
        super.draw(batch, parentAlpha);
    }
}
