package app.barkfart.ui;

import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.Touchable;
import com.badlogic.gdx.scenes.scene2d.utils.ClickListener;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;
import com.badlogic.gdx.utils.Align;

/** A text button drawn from the theme. Styles per UI-SPEC "Visual language". */
public class UiButton extends Actor {
    public enum Style { NORMAL, SEGMENT, SEGMENT_SMALL, ACCENT_OUTLINE, DANGER }

    private final Theme theme;
    private String text;
    private final Style style;
    public boolean enabled = true, selected;
    private Runnable action;
    private final ClickListener click;
    private final GlyphLayout layout = new GlyphLayout();
    private final NinePatchDrawable bg, bgDown, segOn, segEdge, accent, accentInner;

    public UiButton(Theme theme, String text, Style style, Runnable action) {
        this.theme = theme;
        this.text = text;
        this.style = style;
        this.action = action;
        bg = theme.box(Theme.CONTROL);
        bgDown = theme.box(Theme.CONTROL_DOWN);
        segOn = theme.box(Theme.SEGMENT_ON);
        segEdge = theme.box(Theme.SEGMENT_EDGE);
        accent = theme.box(Theme.ACCENT);
        accentInner = theme.box(Theme.PANEL);
        setSize(68, Theme.CONTROL_H);
        setTouchable(Touchable.enabled);
        click = new ClickListener() {
            @Override
            public void clicked(InputEvent event, float x, float y) {
                if (enabled && UiButton.this.action != null) UiButton.this.action.run();
            }
        };
        addListener(click);
    }

    public void setText(String t) {
        text = t;
    }

    public void setAction(Runnable r) {
        action = r;
    }

    /** Touch target never smaller than 44 pt even when drawn smaller. */
    @Override
    public Actor hit(float x, float y, boolean touchable) {
        if (touchable && getTouchable() != Touchable.enabled) return null;
        float ex = Math.max(0, (Theme.TOUCH - getWidth()) / 2), ey = Math.max(0, (Theme.TOUCH - getHeight()) / 2);
        return x >= -ex && x < getWidth() + ex && y >= -ey && y < getHeight() + ey ? this : null;
    }

    @Override
    public void draw(Batch batch, float parentAlpha) {
        float a = parentAlpha * (enabled ? 1f : 0.4f);
        Color old = batch.getColor().cpy();
        batch.setColor(1, 1, 1, a);
        float x = getX(), y = getY(), w = getWidth(), h = getHeight();
        boolean down = enabled && click.isPressed();
        boolean segment = style == Style.SEGMENT || style == Style.SEGMENT_SMALL;
        if (segment && selected) {
            segEdge.draw(batch, x, y, w, h);
            segOn.draw(batch, x + 1, y + 1, w - 2, h - 2);
        } else if (style == Style.ACCENT_OUTLINE) {
            accent.draw(batch, x, y, w, h);
            (down ? bgDown : accentInner).draw(batch, x + 1, y + 1, w - 2, h - 2);
        } else {
            (down ? bgDown : bg).draw(batch, x, y, w, h);
        }
        BitmapFont f = style == Style.SEGMENT_SMALL ? theme.small : theme.body;
        Color c = style == Style.DANGER ? Theme.DANGER
                : (segment && !selected ? Theme.TEXT_DIM : Theme.TEXT);
        f.setColor(c.r, c.g, c.b, a);
        layout.setText(f, text, f.getColor(), w - 8, Align.center, false);
        f.draw(batch, layout, x + 4, y + h / 2 + layout.height / 2);
        batch.setColor(old);
    }
}
