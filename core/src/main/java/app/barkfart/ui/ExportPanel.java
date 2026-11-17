package app.barkfart.ui;

import app.barkfart.export.ExportJob;
import app.barkfart.model.Project;
import app.barkfart.render.FrameGrabber;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.g2d.Batch;
import com.badlogic.gdx.scenes.scene2d.Actor;
import com.badlogic.gdx.scenes.scene2d.InputEvent;
import com.badlogic.gdx.scenes.scene2d.InputListener;
import com.badlogic.gdx.scenes.scene2d.ui.Label;
import com.badlogic.gdx.scenes.scene2d.ui.WidgetGroup;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;
import java.io.File;

/**
 * Export: video (MP4 with sound, iOS), GIF or PNG frames; size; transparent
 * background. Runs frame by frame from tick() (render thread), shows
 * progress, can be cancelled, and offers the file to the share sheet when
 * done. Modal.
 */
public final class ExportPanel extends WidgetGroup {
    public interface Host {
        EditorState state();

        app.barkfart.files.Session session();

        app.barkfart.platform.Platform platform();
    }

    private final Theme theme;
    private final Host host;
    private final NinePatchDrawable bg;
    private final Label title, info, progress;
    private final UiButton close, go, fVideo, fGif, fPng, sFull, sHalf, sSmall, transparent;
    private ExportJob.Format format;
    private int size = 0;   // 0 full, 1 half, 2 small (480 wide)
    private boolean clear;
    private ExportJob job;
    private float px, py, pw, ph;

    public ExportPanel(Theme theme, Host host) {
        this.theme = theme;
        this.host = host;
        bg = theme.box(Theme.PANEL);
        title = new Label("Export", new Label.LabelStyle(theme.title, Theme.TEXT));
        info = new Label("", new Label.LabelStyle(theme.small, Theme.TEXT_DIM));
        info.setWrap(true);
        progress = new Label("", new Label.LabelStyle(theme.body, Theme.TEXT));
        progress.setWrap(true);
        format = host.platform().canEncodeVideo() ? ExportJob.Format.VIDEO : ExportJob.Format.GIF;
        close = new UiButton(theme, "Close", UiButton.Style.NORMAL, new Runnable() {
            public void run() {
                if (job != null && (job.state() == ExportJob.State.RUNNING || job.state() == ExportJob.State.FINISHING)) {
                    job.cancel();
                    progress.setText("Cancelled");
                    job = null;
                    refresh();
                    return;
                }
                setVisible(false);
            }
        });
        go = new UiButton(theme, "Export", UiButton.Style.ACCENT_OUTLINE, new Runnable() {
            public void run() {
                start();
            }
        });
        fVideo = seg("Video", new Runnable() {
            public void run() {
                format = ExportJob.Format.VIDEO;
                refresh();
            }
        });
        fGif = seg("GIF", new Runnable() {
            public void run() {
                format = ExportJob.Format.GIF;
                if (size == 0) size = 1;   // GIFs get big fast
                refresh();
            }
        });
        fPng = seg("PNG frames", new Runnable() {
            public void run() {
                format = ExportJob.Format.PNG;
                refresh();
            }
        });
        sFull = seg("Full", sizer(0));
        sHalf = seg("Half", sizer(1));
        sSmall = seg("Small", sizer(2));
        transparent = seg("Transparent background", new Runnable() {
            public void run() {
                clear = !clear;
                refresh();
            }
        });
        for (Actor a : new Actor[] {title, info, progress, close, go, fVideo, fGif, fPng, sFull, sHalf, sSmall, transparent}) addActor(a);
        addListener(new InputListener() {
            @Override
            public boolean touchDown(InputEvent e, float x, float y, int pointer, int button) {
                return true;   // modal; closing is explicit so a stray tap cannot cancel an export
            }
        });
        setVisible(false);
    }

    private UiButton seg(String s, Runnable r) {
        return new UiButton(theme, s, UiButton.Style.SEGMENT, r);
    }

    private Runnable sizer(final int s) {
        return new Runnable() {
            public void run() {
                size = s;
                refresh();
            }
        };
    }

    public void open() {
        setVisible(true);
        toFront();
        if (job == null) progress.setText("");
        refresh();
    }

    /** Export size, even numbers (video encoders need them). */
    private int[] dims() {
        Project p = host.state().project();
        int w = p.settings.width, h = p.settings.height;
        if (size == 1) {
            w /= 2;
            h /= 2;
        } else if (size == 2) {
            h = Math.round(h * 480f / w);
            w = 480;
        }
        return new int[] {Math.max(16, w & ~1), Math.max(16, h & ~1)};
    }

    private void refresh() {
        EditorState st = host.state();
        boolean busy = job != null && (job.state() == ExportJob.State.RUNNING || job.state() == ExportJob.State.FINISHING);
        boolean canVideo = host.platform().canEncodeVideo();
        if (!canVideo && format == ExportJob.Format.VIDEO) format = ExportJob.Format.GIF;
        fVideo.enabled = canVideo && !busy;
        fGif.enabled = fPng.enabled = sFull.enabled = sHalf.enabled = sSmall.enabled = !busy;
        fVideo.selected = format == ExportJob.Format.VIDEO;
        fGif.selected = format == ExportJob.Format.GIF;
        fPng.selected = format == ExportJob.Format.PNG;
        sFull.selected = size == 0;
        sHalf.selected = size == 1;
        sSmall.selected = size == 2;
        transparent.enabled = format != ExportJob.Format.VIDEO && !busy;
        transparent.selected = clear && format != ExportJob.Format.VIDEO;
        go.enabled = !busy;
        close.setText(busy ? "Cancel" : "Close");
        int[] d = dims();
        int frames = st.frames.frameCount();
        info.setText(frames + " frames · " + String.format("%.2f s", frames / st.editor.fps()) + " at " + Math.round(st.editor.fps()) + " fps · "
                + d[0] + " × " + d[1] + " · camera shot"
                + (format == ExportJob.Format.VIDEO ? " · with sound" : "")
                + (canVideo ? "" : " · Video export needs the iPad app (this build: GIF and PNG)"));
    }

    private void start() {
        EditorState st = host.state();
        if (st.playing) st.stop();
        Project p = st.project();
        int[] d = dims();
        String ext = format == ExportJob.Format.VIDEO ? ".mp4" : format == ExportJob.Format.GIF ? ".gif" : " frames.zip";
        File out = host.session().store.unique(host.session().name(), ext);
        try {
            final FrameGrabber grabber = new FrameGrabber(p, d[0], d[1]);
            final Project proj = p;
            final boolean tr = clear && format != ExportJob.Format.VIDEO;
            ExportJob.Frames frames = new ExportJob.Frames() {
                private boolean disposed;

                public byte[] grab(double t) {
                    return grabber.grab(proj, t, tr);
                }

                public void dispose() {
                    if (!disposed) grabber.dispose();
                    disposed = true;
                }
            };
            app.barkfart.export.VideoSink video = null;
            if (format == ExportJob.Format.VIDEO) {
                double seconds = st.frames.frameCount() / st.editor.fps();
                byte[] wav = app.barkfart.audio.Bounce.wav(app.barkfart.audio.AudioPlan.build(p, p.root(), st.sounds()), seconds);
                File wf = null;
                if (wav != null) {
                    wf = File.createTempFile("soundtrack", ".wav");
                    java.io.FileOutputStream o = new java.io.FileOutputStream(wf);
                    try {
                        o.write(wav);
                    } finally {
                        o.close();
                    }
                    wf.deleteOnExit();
                }
                video = host.platform().openVideo(out, d[0], d[1], st.editor.fps(), wf);
                if (video == null) {
                    frames.dispose();
                    progress.setText("Video export could not start on this device.");
                    return;
                }
            }
            job = new ExportJob(format, out, d[0], d[1], st.frames.frameCount(), st.editor.fps(), tr, frames, video);
            progress.setText("Starting…");
        } catch (Exception e) {
            progress.setText("Could not start: " + e.getMessage());
            job = null;
        }
        refresh();
    }

    /** Called every render tick: works on the export for a few milliseconds. */
    public void tick() {
        if (job == null) return;
        ExportJob.State s = job.state();
        if (s == ExportJob.State.RUNNING) {
            long until = System.nanoTime() + 25000000L;
            do {
                job.step();
            } while (job.state() == ExportJob.State.RUNNING && System.nanoTime() < until);
            progress.setText("Frame " + job.done() + " of " + job.frameCount + "…");
        } else if (s == ExportJob.State.FINISHING) {
            progress.setText("Finishing the video…");
        } else if (s == ExportJob.State.DONE) {
            File f = job.out;
            job = null;
            progress.setText("Saved " + f.getName() + " (" + Math.max(1, f.length() / 1024) + " KB) in your Barkfart folder.");
            if (host.platform().canShare()) host.platform().share(f);
            refresh();
        } else if (s == ExportJob.State.FAILED) {
            progress.setText("Export failed: " + job.problem());
            job = null;
            refresh();
        } else {
            job = null;
            refresh();
        }
    }

    public boolean busy() {
        return job != null;
    }

    /** Stops a running export and deletes its partial file. */
    public void cancel() {
        if (job != null) job.cancel();
        job = null;
    }

    @Override
    public void layout() {
        float w = getWidth(), h = getHeight();
        pw = Math.min(w - 32, 620);
        ph = Math.min(h - 32, 420);
        px = (w - pw) / 2;
        py = (h - ph) / 2;
        float pad = 16, top = py + ph - pad, inner = pw - 2 * pad;
        title.setBounds(px + pad, top - 22, inner - 110, 22);
        close.setBounds(px + pw - pad - 100, top - 32, 100, Theme.CONTROL_H - 4);
        float y = top - 44 - Theme.CONTROL_H;
        float b3 = (inner - 2 * Theme.GAP) / 3;
        fVideo.setBounds(px + pad, y, b3, Theme.CONTROL_H);
        fGif.setBounds(px + pad + b3 + Theme.GAP, y, b3, Theme.CONTROL_H);
        fPng.setBounds(px + pad + 2 * (b3 + Theme.GAP), y, b3, Theme.CONTROL_H);
        y -= Theme.GAP + Theme.CONTROL_H;
        sFull.setBounds(px + pad, y, b3, Theme.CONTROL_H);
        sHalf.setBounds(px + pad + b3 + Theme.GAP, y, b3, Theme.CONTROL_H);
        sSmall.setBounds(px + pad + 2 * (b3 + Theme.GAP), y, b3, Theme.CONTROL_H);
        y -= Theme.GAP + Theme.CONTROL_H;
        transparent.setBounds(px + pad, y, inner, Theme.CONTROL_H);
        y -= Theme.GAP + 36;
        info.setBounds(px + pad, y, inner, 36);
        go.setBounds(px + pad, py + pad, inner, Theme.CONTROL_H);
        progress.setBounds(px + pad, py + pad + Theme.CONTROL_H + Theme.GAP, inner, Math.max(20, y - (py + pad + Theme.CONTROL_H + 2 * Theme.GAP)));
        refresh();
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
