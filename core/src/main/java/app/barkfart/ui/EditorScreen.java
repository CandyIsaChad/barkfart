package app.barkfart.ui;

import app.barkfart.model.Project;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.InputMultiplexer;
import com.badlogic.gdx.Preferences;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.GL20;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.Stage;
import com.badlogic.gdx.utils.Disposable;
import com.badlogic.gdx.utils.viewport.ScreenViewport;

/**
 * The editor screen: sidebar, dope sheet, play button, stage (UI-SPEC v2).
 * The UI layer is rebuilt when the UI size changes; the editor state persists.
 */
public final class EditorScreen implements Disposable {
    public static final float MIN_USER = 0.8f, MAX_USER = 1.8f;

    public final EditorState state;
    /** Logical screen units per point from the platform (1 on iOS; the desktop preview passes its own). */
    private final float base;
    private Theme theme;
    private Stage stage;
    private Sidebar sidebar;
    private DopeSheet sheet;
    private StageView view;
    private UiButton play;
    private float scale;
    private float insetL, insetR, insetB, insetT;
    private final Preferences prefs;

    public EditorScreen(Project p, float base) {
        this.base = base;
        state = new EditorState(p);
        prefs = Gdx.app.getPreferences("barkfart.ui");
        state.uiUserScale = clampUser(prefs.getFloat("uiSize", 1f));
        try {
            state.tools.keyScope = app.barkfart.canvas.CanvasTools.KeyScope.valueOf(prefs.getString("keyScope", "JOINT"));
        } catch (IllegalArgumentException ignored) {
            // unknown value from a newer build: keep the default
        }
        state.onUiScaleChanged = new Runnable() {
            public void run() {
                state.uiUserScale = clampUser(state.uiUserScale);
                prefs.putFloat("uiSize", state.uiUserScale);
                prefs.flush();
                build();
                resize(Gdx.graphics.getWidth(), Gdx.graphics.getHeight());
            }
        };
        build();
    }

    private static float clampUser(float v) {
        return Math.max(MIN_USER, Math.min(MAX_USER, Math.round(v * 20) / 20f));
    }

    /**
     * Automatic size from the screen height in points: phones 1.0; tablets
     * scale with height (iPad mini ~1.13, Air/11-inch ~1.25, 12.9-inch 1.5).
     */
    public static float autoScale(float heightPt) {
        if (heightPt <= 500) return 1f;
        return Math.max(1f, Math.min(1.5f, heightPt / 656f));
    }

    private void build() {
        if (stage != null) {
            stage.dispose();
            view.dispose();
            theme.dispose();
        }
        state.clearListeners();
        float heightPt = Gdx.graphics.getHeight() / base;
        scale = base * autoScale(heightPt) * state.uiUserScale;
        float backbuffer = Gdx.graphics.getBackBufferWidth() / (float) Math.max(1, Gdx.graphics.getWidth());
        theme = new Theme(scale * backbuffer);
        ScreenViewport vp = new ScreenViewport();
        vp.setUnitsPerPixel(1f / scale);
        stage = new Stage(vp);
        view = new StageView(theme, state);
        sidebar = new Sidebar(theme, state);
        sheet = new DopeSheet(theme, state, new DopeSheet.Resized() {
            public void resized() {
                layout();
            }
        });
        final Theme th = theme;
        play = new UiButton(theme, "", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                if (state.playing) state.stop();
                else state.play();
            }
        }) {
            @Override
            public void draw(Batch batch, float a) {
                super.draw(batch, a);
                // play triangle or stop square (icons only for play/stop, per spec)
                float cx = getX() + getWidth() / 2, cy = getY() + getHeight() / 2;
                Color old = batch.getColor().cpy();
                batch.setColor(Theme.TEXT);
                if (state.playing) {
                    batch.draw(th.white(), cx - 7, cy - 7, 14, 14);
                } else {
                    for (int i = 0; i < 16; i++) batch.draw(th.white(), cx - 6 + i, cy - 8 + i * 0.5f, 1, 16 - i);
                }
                batch.setColor(old);
            }
        };
        stage.addActor(view);
        stage.addActor(sheet);
        stage.addActor(play);
        stage.addActor(sidebar);
        final EasingMenu easing = new EasingMenu(theme);
        stage.addActor(easing);
        state.easingOpener = new EditorState.EasingOpener() {
            public void open(float sx, float sy, EasingMenu.Preset cur, EasingMenu.Chosen chosen) {
                easing.setBounds(0, 0, stage.getWidth(), stage.getHeight());
                easing.open(sx, sy, cur, chosen);
            }
        };
        state.addListener(new Runnable() {
            public void run() {
                if (Math.abs(sheet.getHeight() - Math.max(DopeSheet.COLLAPSED, Math.min(state.sheetHeight, stage.getHeight() * 0.6f))) > 0.5f) {
                    layout();
                }
            }
        });
        Gdx.input.setInputProcessor(new InputMultiplexer(stage));
        state.fire();
    }

    public void resize(int width, int height) {
        stage.getViewport().update(width, height, true);
        float k = 1f / scale;
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
        float sheetH = Math.max(DopeSheet.COLLAPSED, Math.min(state.sheetHeight, h * 0.6f));
        float sheetY = h - insetT - sheetH;
        float playX = w - insetR - Theme.PLAY - Theme.GAP;
        sheet.setBounds(sbw, sheetY, playX - Theme.GAP - sbw, sheetH);
        play.setBounds(playX, h - insetT - DopeSheet.COLLAPSED + (DopeSheet.COLLAPSED - Theme.PLAY) / 2, Theme.PLAY, Theme.PLAY);
        view.setBounds(sbw, 0, w - sbw, sheetY);
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
