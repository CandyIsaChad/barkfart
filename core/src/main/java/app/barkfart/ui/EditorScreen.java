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

    public EditorState state;
    private final app.barkfart.platform.Platform platform;
    private final app.barkfart.files.Session session;
    private FilePanel filePanel;
    private float autosaveTimer;
    public static final float AUTOSAVE_SECONDS = 30;
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

    public EditorScreen(app.barkfart.platform.Platform platform, float base) {
        this.base = base;
        this.platform = platform;
        prefs = Gdx.app.getPreferences("barkfart.ui");
        session = new app.barkfart.files.Session(new app.barkfart.files.ProjectStore(platform.documents()));
        float user = clampUser(prefs.getFloat("uiSize", 1f));
        app.barkfart.canvas.CanvasTools.KeyScope scope = app.barkfart.canvas.CanvasTools.KeyScope.JOINT;
        try {
            scope = app.barkfart.canvas.CanvasTools.KeyScope.valueOf(prefs.getString("keyScope", "JOINT"));
        } catch (IllegalArgumentException ignored) {
            // unknown value from a newer build: keep the default
        }
        // Reopen the last project, or start a new one.
        // Stored by file name: iOS app container paths change between installs/updates.
        String lastName = prefs.getString("lastProject", "");
        java.io.File last = lastName.isEmpty() ? new java.io.File("") : new java.io.File(platform.documents(), new java.io.File(lastName).getName());
        Project p = null;
        if (last.isFile()) {
            try {
                p = session.open(last);
            } catch (Exception e) {
                Gdx.app.error("Barkfart", "could not reopen " + last, e);
                last = null;
            }
        } else {
            last = null;
        }
        attach(p != null ? p : app.barkfart.BarkfartApp.newProject(), p != null ? last : null, p != null ? nameOf(last) : "Untitled", user, scope);
        build();
    }

    private static String nameOf(java.io.File f) {
        String n = f.getName();
        return n.endsWith(app.barkfart.files.ProjectStore.EXT) ? n.substring(0, n.length() - app.barkfart.files.ProjectStore.EXT.length()) : n;
    }

    private void attach(Project p, java.io.File file, String name, float user, app.barkfart.canvas.CanvasTools.KeyScope scope) {
        EditorState old = state;
        state = new EditorState(p);
        if (old != null) {   // checks belong to the device, not the project
            state.diagnostics = old.diagnostics;
            state.diagnosticsRunner = old.diagnosticsRunner;
        }
        state.audio = platform.audio();
        state.uiUserScale = user;
        state.tools.keyScope = scope;
        state.onUiScaleChanged = new Runnable() {
            public void run() {
                state.uiUserScale = clampUser(state.uiUserScale);
                prefs.putFloat("uiSize", state.uiUserScale);
                prefs.flush();
                build();
                resize(Gdx.graphics.getWidth(), Gdx.graphics.getHeight());
            }
        };
        session.attach(state.doc, file, name);
        prefs.putString("lastProject", file == null ? "" : file.getName()).flush();
    }

    /** The Diagnostics screen's "try it yourself" actions. */
    private void diagnosticsTry(int i) {
        if (i == 0) {
            String err = state.audio.play(app.barkfart.audio.AudioPlan.single(app.barkfart.audio.Mixer.tone(440, 0.6, 44100, 0.4f)), 0, 0);
            state.flash(err == null ? "Playing a test tone (440 Hz)" : "Sound off: " + err, null, 6);
        } else if (i == 1) {
            app.barkfart.model.Symbol root = state.project().root();
            int before = root.layers.size();
            app.barkfart.fx.EffectScene.addTo(state.project(), root, 3);
            java.util.List<app.barkfart.model.Layer> added = new java.util.ArrayList<app.barkfart.model.Layer>(root.layers.subList(before, root.layers.size()));
            root.layers.subList(before, root.layers.size()).clear();
            state.doc.begin("Add effects demo");
            try {
                for (app.barkfart.model.Layer l : added) state.doc.apply(app.barkfart.edit.StructureEdits.addLayer(root, l, root.layers.size()));
            } finally {
                state.doc.commit();
            }
            state.project().invalidateIndex();
            state.flash("Added the effects demo (Undo removes it)", null, 6);
        } else if (i == 2) {
            try {
                java.io.File f = java.io.File.createTempFile("test-tone", ".wav");
                java.io.FileOutputStream out = new java.io.FileOutputStream(f);
                try {
                    out.write(app.barkfart.audio.Wav.encode(app.barkfart.audio.Mixer.tone(330, 1.5, 44100, 0.3f)));
                } finally {
                    out.close();
                }
                app.barkfart.files.Importer.Result r = app.barkfart.files.Importer.importFile(state.doc, f, state.audio, state.frames.time(state.frame));
                f.delete();
                state.flash(r.message, null, 6);
            } catch (Exception e) {
                state.flash("Could not add the test sound: " + e.getMessage(), null, 6);
            }
        }
        state.fire();
    }

    /** Saves the current project if it changed; returns false (and reports) on failure. */
    public boolean saveNow() {
        try {
            boolean saved = session.saveIfDirty();
            if (saved) prefs.putString("lastProject", session.file().getName()).flush();
            return true;
        } catch (Exception e) {
            if (state != null) state.flash("Could not save: " + e.getMessage(), null);
            return false;
        }
    }

    private void switchTo(Project p, java.io.File f, String name) {
        if (!saveNow()) return;
        attach(p, f, name, state.uiUserScale, state.tools.keyScope);
        build();
        resize(Gdx.graphics.getWidth(), Gdx.graphics.getHeight());
    }

    public void openFile(java.io.File f) {
        try {
            switchTo(session.open(f), f, nameOf(f));
            state.flash("Opened " + nameOf(f), null);
        } catch (Exception e) {
            state.flash("Could not open " + f.getName() + ": " + e.getMessage(), null);
        }
    }

    public void newProject() {
        switchTo(app.barkfart.BarkfartApp.newProject(), null, "Untitled");
        state.flash("New project", null);
    }

    /** Called when the app goes to the background: never lose work. */
    public void pause() {
        saveNow();
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
        state.fileOpener = new Runnable() {
            public void run() {
                filePanel.setBounds(0, 0, stage.getWidth(), stage.getHeight());
                filePanel.open();
            }
        };
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
        filePanel = new FilePanel(theme, new FilePanel.Host() {
            public EditorState state() {
                return state;
            }

            public app.barkfart.files.Session session() {
                return session;
            }

            public app.barkfart.platform.Platform platform() {
                return platform;
            }

            public void openFile(java.io.File f) {
                EditorScreen.this.openFile(f);
            }

            public void newProject() {
                EditorScreen.this.newProject();
            }

            public void afterDelete() {
                // the open project's file is gone: continue in a fresh project
                attach(app.barkfart.BarkfartApp.newProject(), null, "Untitled", state.uiUserScale, state.tools.keyScope);
                build();
                resize(Gdx.graphics.getWidth(), Gdx.graphics.getHeight());
            }
        });
        stage.addActor(filePanel);
        final DiagnosticsPanel diag = new DiagnosticsPanel(theme, new DiagnosticsPanel.Host() {
            public EditorState state() {
                return state;
            }

            public void rerun() {
                if (state.diagnosticsRunner != null) state.diagnosticsRunner.run();
            }

            public String[][] tryItLabels() {
                return new String[][] {
                    {"Play a test tone", "You should hear a short beep. Nothing? Check the volume and the silent switch."},
                    {"Add the effects demo", "Adds six squares: blurred, glowing, shadowed, recoloured, see-through, and an inverting layer over all of it. Undo removes them."},
                    {"Add a test sound", "Adds a 1.5 s tone at the current frame. Press Play to hear it with the animation; it shows in the dope sheet."},
                };
            }

            public void tryIt(int i) {
                diagnosticsTry(i);
            }
        });
        stage.addActor(diag);
        state.diagnosticsOpener = new Runnable() {
            public void run() {
                diag.setBounds(0, 0, stage.getWidth(), stage.getHeight());
                diag.open();
            }
        };
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
        autosaveTimer += dt;
        if (autosaveTimer >= AUTOSAVE_SECONDS && !state.playing && !state.doc.inTransaction()) {
            autosaveTimer = 0;
            saveNow();
        }
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
