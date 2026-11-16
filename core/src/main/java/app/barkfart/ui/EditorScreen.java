package app.barkfart.ui;

import app.barkfart.model.Project;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.InputMultiplexer;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.graphics.g2d.GlyphLayout;
import com.badlogic.gdx.scenes.scene2d.Stage;
import com.badlogic.gdx.utils.Disposable;
import com.badlogic.gdx.utils.viewport.ScreenViewport;

/** The editor screen: sidebar, frame strip, play button, stage, frame menu (UI-SPEC v2). */
public final class EditorScreen implements Disposable {
    public final EditorState state;
    private final Theme theme;
    private final Stage stage;
    private final Sidebar sidebar;
    private final FrameStrip strip;
    private final StageView view;
    private final UiButton play;
    private final FrameMenu menu;
    private final float uiScale;
    private float insetL, insetR, insetB, insetT;

    /** @param uiScale logical screen units per UI point (1 on iOS; 2 for the desktop phone preview). */
    public EditorScreen(Project p, float uiScale) {
        this.uiScale = uiScale;
        float backbuffer = Gdx.graphics.getBackBufferWidth() / (float) Math.max(1, Gdx.graphics.getWidth());
        theme = new Theme(uiScale * backbuffer);
        state = new EditorState(p);
        ScreenViewport vp = new ScreenViewport();
        vp.setUnitsPerPixel(1f / uiScale);
        stage = new Stage(vp);
        view = new StageView(theme, state);
        sidebar = new Sidebar(theme, state);
        menu = new FrameMenu(theme, state);
        strip = new FrameStrip(theme, state, new FrameStrip.MenuOpener() {
            public void open(int frame, float sx, float sy) {
                menu.open(frame, sx, sy);
            }
        });
        play = new UiButton(theme, "Play", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                if (state.playing) state.stop();
                else state.play();
            }
        }) {
            private final GlyphLayout g = new GlyphLayout();

            @Override
            public void draw(Batch batch, float a) {
                setText("");
                super.draw(batch, a);
                // play triangle or stop square, drawn (icons only for play/stop, per spec)
                float cx = getX() + getWidth() / 2, cy = getY() + getHeight() / 2;
                Color old = batch.getColor().cpy();
                batch.setColor(Theme.TEXT);
                if (state.playing) {
                    batch.draw(theme.white(), cx - 7, cy - 7, 14, 14);
                } else {
                    for (int i = 0; i < 16; i++) batch.draw(theme.white(), cx - 6 + i, cy - 8 + i * 0.5f, 1, 16 - i);
                }
                batch.setColor(old);
            }
        };
        stage.addActor(view);
        stage.addActor(strip);
        stage.addActor(play);
        stage.addActor(sidebar);
        stage.addActor(menu);
        state.addListener(new Runnable() {
            public void run() {
                if (!state.playing) strip.reveal(state.frame);
            }
        });
        Gdx.input.setInputProcessor(new InputMultiplexer(stage));
    }

    public void resize(int width, int height) {
        stage.getViewport().update(width, height, true);
        float k = 1f / uiScale;
        insetL = Gdx.graphics.getSafeInsetLeft() * k;
        insetR = Gdx.graphics.getSafeInsetRight() * k;
        insetB = Gdx.graphics.getSafeInsetBottom() * k;
        insetT = Gdx.graphics.getSafeInsetTop() * k;
        layout();
    }

    private void layout() {
        float w = stage.getWidth(), h = stage.getHeight();
        // Panels run under the safe insets; controls stay inside them.
        float sbw = insetL + Theme.SIDEBAR_W;
        sidebar.setBounds(0, 0, sbw, h);
        sidebar.setPadding(insetL, insetB);
        float stripY = h - insetT - Theme.STRIP_H;
        float playX = w - insetR - Theme.PLAY - Theme.GAP;
        strip.setBounds(sbw, stripY, playX - Theme.GAP - sbw, Theme.STRIP_H);
        play.setBounds(playX, stripY + (Theme.STRIP_H - Theme.PLAY) / 2, Theme.PLAY, Theme.PLAY);
        view.setBounds(sbw, 0, w - sbw, stripY);
        menu.setBounds(0, 0, w, h);
        sidebar.invalidate();
    }

    public void render(float dt) {
        state.update(dt);
        Gdx.gl.glClearColor(Theme.PANEL.r, Theme.PANEL.g, Theme.PANEL.b, 1);
        Gdx.gl.glClear(GL20.GL_COLOR_BUFFER_BIT);
        stage.act(dt);
        // panel behind the strip and play button (under the top inset too)
        stage.getViewport().apply();
        stage.draw();
    }

    @Override
    public void dispose() {
        stage.dispose();
        view.dispose();
        theme.dispose();
    }
}
