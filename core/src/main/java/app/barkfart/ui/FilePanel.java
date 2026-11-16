package app.barkfart.ui;

import app.barkfart.files.Importer;
import app.barkfart.files.ProjectStore;
import app.barkfart.model.Layer;
import app.barkfart.platform.Platform;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.Input;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.Touchable;
import com.badlogic.gdx.scenes.scene2d.ui.Label;
import com.badlogic.gdx.scenes.scene2d.ui.ScrollPane;
import com.badlogic.gdx.scenes.scene2d.ui.Table;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.scenes.scene2d.utils.ClickListener;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;
import java.io.File;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.List;

/** The File panel: projects, import, export (PARITY "Files" phase). Modal. */
public final class FilePanel extends WidgetGroup {
    /** What the panel asks the screen to do. */
    public interface Host {
        EditorState state();

        app.barkfart.files.Session session();

        Platform platform();

        void openFile(File f);

        void newProject();

        void afterDelete();
    }

    private final Theme theme;
    private final Host host;
    private final NinePatchDrawable bg;
    private final Table projects = new Table(), imports = new Table();
    private final ScrollPane projScroll, impScroll;
    private final Label title, projTitle, impTitle;
    private final UiButton close, newB, save, rename, duplicate, delete, pick, export, share;
    private float px, py, pw, ph;
    private boolean deleteArmed;
    private final SimpleDateFormat fmt = new SimpleDateFormat("d MMM, HH:mm");

    public FilePanel(Theme theme, Host host) {
        this.theme = theme;
        this.host = host;
        bg = theme.box(Theme.PANEL);
        title = new Label("", new Label.LabelStyle(theme.title, Theme.TEXT));
        title.setEllipsis("…");
        projTitle = new Label("Projects", new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        impTitle = new Label("Import into this project", new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        projects.top().left();
        imports.top().left();
        projScroll = new ScrollPane(projects);
        impScroll = new ScrollPane(imports);
        projScroll.setScrollingDisabled(true, false);
        impScroll.setScrollingDisabled(true, false);
        close = new UiButton(theme, "Close", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                close();
            }
        });
        newB = new UiButton(theme, "New", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                close();
                FilePanel.this.host.newProject();
            }
        });
        save = new UiButton(theme, "Save", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                try {
                    File f = FilePanel.this.host.session().save();
                    status("Saved " + f.getName());
                    refresh();
                } catch (Exception e) {
                    status("Could not save: " + e.getMessage());
                }
            }
        });
        rename = new UiButton(theme, "Rename", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                renameCurrent();
            }
        });
        duplicate = new UiButton(theme, "Duplicate", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                try {
                    File f = FilePanel.this.host.session().save();
                    File d = FilePanel.this.host.session().store.duplicate(f);
                    status("Duplicated as " + d.getName());
                    refresh();
                } catch (Exception e) {
                    status("Could not duplicate: " + e.getMessage());
                }
            }
        });
        delete = new UiButton(theme, "Delete", UiButton.Style.DANGER, new Runnable() {
            public void run() {
                deleteCurrent();
            }
        });
        pick = new UiButton(theme, "From Files…", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                pickFiles();
            }
        });
        export = new UiButton(theme, "Export figure", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                exportFigure();
            }
        });
        share = new UiButton(theme, "Share project", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                try {
                    FilePanel.this.host.platform().share(FilePanel.this.host.session().save());
                } catch (Exception e) {
                    status("Could not share: " + e.getMessage());
                }
            }
        });
        for (Actor a : new Actor[] {title, projTitle, impTitle, projScroll, impScroll, close, newB, save, rename, duplicate, delete, pick, export, share}) {
            addActor(a);
        }
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                if (x < px || x > px + pw || y < py || y > py + ph) close();
                return true;   // modal
            }
        });
        setVisible(false);
    }

    private void status(String s) {
        host.state().flash(s, null);
    }

    public void open() {
        deleteArmed = false;
        delete.setText("Delete");
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
        ph = Math.min(h - 32, 520);
        px = (w - pw) / 2;
        py = (h - ph) / 2;
        float pad = 16, top = py + ph - pad;
        title.setBounds(px + pad, top - 22, pw - 2 * pad - 92, 22);
        close.setBounds(px + pw - pad - 84, top - 32, 84, Theme.CONTROL_H - 4);
        float colTop = top - 44, footerH = 2 * Theme.CONTROL_H + Theme.GAP + pad;
        float colW = (pw - 3 * pad) / 2, colH = colTop - 18 - (py + footerH);
        projTitle.setBounds(px + pad, colTop - 14, colW, 14);
        impTitle.setBounds(px + 2 * pad + colW, colTop - 14, colW, 14);
        projScroll.setBounds(px + pad, py + footerH, colW, colH);
        impScroll.setBounds(px + 2 * pad + colW, py + footerH, colW, colH);
        float bw = (pw - 2 * pad - 3 * Theme.GAP) / 4, y1 = py + pad + Theme.CONTROL_H + Theme.GAP, y2 = py + pad;
        newB.setBounds(px + pad, y1, bw, Theme.CONTROL_H);
        save.setBounds(px + pad + (bw + Theme.GAP), y1, bw, Theme.CONTROL_H);
        rename.setBounds(px + pad + 2 * (bw + Theme.GAP), y1, bw, Theme.CONTROL_H);
        duplicate.setBounds(px + pad + 3 * (bw + Theme.GAP), y1, bw, Theme.CONTROL_H);
        pick.setBounds(px + pad, y2, bw, Theme.CONTROL_H);
        export.setBounds(px + pad + (bw + Theme.GAP), y2, bw, Theme.CONTROL_H);
        share.setBounds(px + pad + 2 * (bw + Theme.GAP), y2, bw, Theme.CONTROL_H);
        delete.setBounds(px + pad + 3 * (bw + Theme.GAP), y2, bw, Theme.CONTROL_H);
        refresh();
    }

    private void refresh() {
        if (!isVisible()) return;
        app.barkfart.files.Session s = host.session();
        title.setText(s.name() + (s.dirty() ? " · unsaved changes" : ""));
        pick.enabled = host.platform().canPick();
        share.enabled = host.platform().canShare();
        export.enabled = host.state().selectedLayer != null;
        rename.enabled = true;
        float colW = projScroll.getWidth();
        projects.clearChildren();
        List<ProjectStore.Entry> ps = s.store.projects();
        if (ps.isEmpty()) projects.add(note("No saved projects yet. Your work is saved automatically.", colW)).width(colW).row();
        for (final ProjectStore.Entry e : ps) {
            boolean current = e.file.equals(s.file());
            projects.add(new Row(e.name, (current ? "Open · " : "") + fmt.format(new Date(e.modified)), current, new Runnable() {
                public void run() {
                    if (e.file.equals(host.session().file())) {
                        close();
                        return;
                    }
                    close();
                    host.openFile(e.file);
                }
            })).width(colW).height(44).padBottom(2).row();
        }
        imports.clearChildren();
        List<ProjectStore.Entry> is = s.store.importables();
        if (is.isEmpty()) {
            imports.add(note("Put .nodes or .nodemc files in the Barkfart folder (Files app), or use From Files…", colW))
                    .width(colW).row();
        }
        for (final ProjectStore.Entry e : is) {
            String kind = e.kind == ProjectStore.Kind.FIGURE ? "Stickfigure" : e.kind == ProjectStore.Kind.MOVIECLIP ? "Movieclip"
                    : e.kind == ProjectStore.Kind.LEGACY_PROJECT ? "Stick Nodes project: figures only for now" : "Pivot figure (not yet)";
            imports.add(new Row(e.name, kind, false, new Runnable() {
                public void run() {
                    importFile(e.file);
                }
            })).width(colW).height(44).padBottom(2).row();
        }
    }

    private Label note(String s, float w) {
        Label l = new Label(s, new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        l.setWrap(true);
        return l;
    }

    private void importFile(File f) {
        try {
            Importer.Result r = Importer.importFile(host.state().doc, f);
            close();
            if (r.layer instanceof Layer.FigureLayer) host.state().select((Layer.FigureLayer) r.layer, null);
            status(r.message + (r.approximations.isEmpty() ? "" : " · approximated: " + join(r.approximations)));
        } catch (Importer.Unsupported e) {
            status(e.getMessage());
        } catch (Exception e) {
            status("Could not import " + f.getName() + ": " + e.getMessage());
        }
    }

    private static String join(List<String> l) {
        StringBuilder b = new StringBuilder();
        for (String s : l) b.append(b.length() == 0 ? "" : ", ").append(s);
        return b.toString();
    }

    private void pickFiles() {
        host.platform().pickFiles(new Platform.PickCallback() {
            public void picked(List<Platform.Picked> files) {
                File openMe = null;
                int imported = 0;
                for (Platform.Picked p : files) {
                    try {
                        File f = host.session().store.addCopy(p.name, p.bytes);
                        ProjectStore.Kind k = ProjectStore.kindOf(f.getName());
                        if (k == ProjectStore.Kind.PROJECT) openMe = f;
                        else if (k == ProjectStore.Kind.FIGURE || k == ProjectStore.Kind.MOVIECLIP) {
                            Importer.importFile(host.state().doc, f);
                            imported++;
                        }
                    } catch (Exception e) {
                        status("Could not import " + p.name + ": " + e.getMessage());
                    }
                }
                if (openMe != null) {
                    close();
                    host.openFile(openMe);
                } else if (imported > 0) {
                    close();
                    status("Imported " + imported + (imported == 1 ? " file" : " files"));
                } else {
                    refresh();
                }
            }
        });
    }

    private void exportFigure() {
        Layer.FigureLayer l = host.state().selectedLayer;
        if (l == null) return;
        try {
            File f = Importer.exportFigure(host.session().store, l);
            status("Exported " + f.getName() + " to the Barkfart folder");
            if (host.platform().canShare()) host.platform().share(f);
            refresh();
        } catch (Exception e) {
            status("Could not export: " + e.getMessage());
        }
    }

    private void renameCurrent() {
        final app.barkfart.files.Session s = host.session();
        Gdx.input.getTextInput(new Input.TextInputListener() {
            public void input(String text) {
                try {
                    if (s.file() == null) s.save();
                    s.renamed(s.store.rename(s.file(), text));
                    status("Renamed to " + s.name());
                } catch (Exception e) {
                    status("Could not rename: " + e.getMessage());
                }
                refresh();
            }

            public void canceled() {
            }
        }, "Rename project", s.name(), "Project name");
    }

    private void deleteCurrent() {
        if (!deleteArmed) {
            deleteArmed = true;
            delete.setText("Tap to confirm");
            return;
        }
        final app.barkfart.files.Session s = host.session();
        try {
            if (s.file() == null) {
                close();
                host.newProject();
                return;
            }
            final String name = s.file().getName();
            final File trashed = s.store.delete(s.file());
            close();
            host.afterDelete();
            host.state().flash("Deleted " + name + " · Undo", new Runnable() {
                public void run() {
                    try {
                        File back = host.session().store.restore(trashed, name);
                        host.openFile(back);
                    } catch (Exception e) {
                        status("Could not restore: " + e.getMessage());
                    }
                }
            });
        } catch (Exception e) {
            status("Could not delete: " + e.getMessage());
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

    /** A list row: name on the left, detail below, tappable. */
    final class Row extends Actor {
        private final String name, detail;
        private final boolean current;
        private final ClickListener click;

        Row(String name, String detail, boolean current, final Runnable action) {
            this.name = name;
            this.detail = detail;
            this.current = current;
            setTouchable(Touchable.enabled);
            click = new ClickListener() {
                @Override
                public void clicked(InputEvent event, float x, float y) {
                    action.run();
                }
            };
            addListener(click);
        }

        @Override
        public void draw(Batch batch, float a) {
            Color old = batch.getColor().cpy();
            batch.setColor(click.isPressed() ? Theme.CONTROL_DOWN : (current ? Theme.SEGMENT_ON : Theme.CONTROL));
            batch.draw(theme.white(), getX(), getY(), getWidth(), getHeight());
            if (current) {
                batch.setColor(Theme.ACCENT);
                batch.draw(theme.white(), getX(), getY(), 3, getHeight());
            }
            theme.body.setColor(Theme.TEXT);
            theme.body.draw(batch, name, getX() + 12, getY() + getHeight() - 8, getWidth() - 20, com.badlogic.gdx.utils.Align.left, false);
            theme.small.setColor(Theme.TEXT_DIM);
            theme.small.draw(batch, detail, getX() + 12, getY() + 16, getWidth() - 20, com.badlogic.gdx.utils.Align.left, false);
            batch.setColor(old);
        }
    }
}
