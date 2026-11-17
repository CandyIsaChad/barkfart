package app.barkfart.ui;

import app.barkfart.selftest.SelfTest;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.ui.Label;
import com.badlogic.gdx.scenes.scene2d.ui.ScrollPane;
import com.badlogic.gdx.scenes.scene2d.ui.Table;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;

/**
 * Every automated check, run on this device, grouped by feature with PASS or
 * FAIL and the reason; plus things only a person can judge ("try it"). Opened
 * from the launch message or the sidebar. Modal.
 */
public final class DiagnosticsPanel extends WidgetGroup {
    /** What the panel needs from the app. */
    public interface Host {
        EditorState state();

        /** Runs every check again (results arrive in state().diagnostics). */
        void rerun();

        /** Things to try by hand: label and action. */
        String[][] tryItLabels();

        void tryIt(int index);
    }

    private final Theme theme;
    private final Host host;
    private final NinePatchDrawable bg;
    private final Table list = new Table();
    private final ScrollPane scroll;
    private final Label title;
    private final UiButton close, rerun;
    private float px, py, pw, ph;

    public DiagnosticsPanel(Theme theme, Host host) {
        this.theme = theme;
        this.host = host;
        bg = theme.box(Theme.PANEL);
        title = new Label("Diagnostics", new Label.LabelStyle(theme.title, Theme.TEXT));
        title.setEllipsis("…");
        list.top().left();
        scroll = new ScrollPane(list);
        scroll.setScrollingDisabled(true, false);
        close = new UiButton(theme, "Close", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                close();
            }
        });
        rerun = new UiButton(theme, "Run again", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                DiagnosticsPanel.this.host.state().diagnostics = null;
                refresh();
                DiagnosticsPanel.this.host.rerun();
            }
        });
        for (Actor a : new Actor[] {title, scroll, close, rerun}) addActor(a);
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (x < px || x > px + pw || y < py || y > py + ph) close();
                return true;   // modal
            }
        });
        host.state().addListener(new Runnable() {
            public void run() {
                refresh();
            }
        });
        setVisible(false);
    }

    public void open() {
        setVisible(true);
        toFront();
        refresh();
    }

    public void close() {
        setVisible(false);
    }

    @Override
    public void layout() {
        float w = getWidth(), h = getHeight();
        pw = Math.min(w - 32, 760);
        ph = Math.min(h - 32, 620);
        px = (w - pw) / 2;
        py = (h - ph) / 2;
        float pad = 16, top = py + ph - pad;
        title.setBounds(px + pad, top - 22, pw - 2 * pad - 2 * 120, 22);
        close.setBounds(px + pw - pad - 100, top - 32, 100, Theme.CONTROL_H - 4);
        rerun.setBounds(px + pw - pad - 208, top - 32, 100, Theme.CONTROL_H - 4);
        scroll.setBounds(px + pad, py + pad, pw - 2 * pad, top - 44 - (py + pad));
        refresh();
    }

    private Label label(String s, com.badlogic.gdx.graphics.g2d.BitmapFont f, Color c) {
        Label l = new Label(s, new Label.LabelStyle(f, c));
        l.setWrap(true);
        return l;
    }

    private void refresh() {
        if (!isVisible()) return;
        float lw = scroll.getWidth();
        list.clearChildren();
        SelfTest.Result r = host.state().diagnostics;
        if (r == null) {
            title.setText("Diagnostics · running…");
            list.add(label("Running every check on this device…", theme.body, Theme.TEXT_DIM)).width(lw).row();
        } else {
            int failed = r.entries.size() - r.passed;
            title.setText("Diagnostics · " + (failed == 0 ? "all " + r.entries.size() + " passed" : failed + " of " + r.entries.size() + " FAILED"));
            list.add(label(failed == 0
                    ? "Everything below was checked on this device just now. Nothing to report."
                    : "Failures are listed first. Send a screenshot of them.", theme.small, failed == 0 ? Theme.TEXT_DIM : Theme.DANGER))
                    .width(lw).padBottom(8).row();
            java.util.List<String> groups = new java.util.ArrayList<String>();
            for (SelfTest.Entry e : r.entries) if (!groups.contains(e.group)) groups.add(e.group);
            java.util.List<SelfTest.Entry> ordered = new java.util.ArrayList<SelfTest.Entry>();
            for (String g : groups) for (SelfTest.Entry e : r.entries) if (e.group.equals(g)) ordered.add(e);
            for (int pass = 0; pass < 2; pass++) {   // failures first, then everything, by feature
                String group = null;
                for (SelfTest.Entry e : ordered) {
                    if (pass == 0 && e.ok) continue;
                    if (pass == 1 && !e.ok) continue;
                    if (!e.group.equals(group)) {
                        group = e.group;
                        list.add(label(pass == 0 ? "Failed · " + group : group, theme.title, pass == 0 ? Theme.DANGER : Theme.TEXT))
                                .width(lw).padTop(10).padBottom(4).row();
                    }
                    String line = (e.ok ? "PASS   " : "FAIL   ") + e.name + (e.millis > 0 ? "   " + e.millis + " ms" : "");
                    list.add(label(line, theme.body, e.ok ? Theme.TEXT : Theme.DANGER)).width(lw).padBottom(2).row();
                    if (!e.ok && e.detail.length() > 0) list.add(label(e.detail, theme.small, Theme.TEXT_DIM)).width(lw).padBottom(6).row();
                }
            }
        }
        list.add(label("Try it yourself (things only you can judge)", theme.title, Theme.TEXT)).width(lw).padTop(16).padBottom(4).row();
        String[][] items = host.tryItLabels();
        for (int i = 0; i < items.length; i++) {
            final int k = i;
            list.add(label(items[i][1], theme.small, Theme.TEXT_DIM)).width(lw).padBottom(4).row();
            list.add(new UiButton(theme, items[i][0], UiButton.Style.NORMAL, new Runnable() {
                public void run() {
                    host.tryIt(k);
                }
            })).width(Math.min(lw, 320)).height(Theme.CONTROL_H).left().padBottom(10).row();
        }
    }

    @Override
    public void draw(Batch batch, float parentAlpha) {
        Color c = batch.getColor().cpy();
        batch.setColor(0, 0, 0, 0.45f);
        batch.draw(theme.white(), getX(), getY(), getWidth(), getHeight());
        batch.setColor(c);
        bg.draw(batch, getX() + px, getY() + py, pw, ph);
        super.draw(batch, parentAlpha);
    }
}
