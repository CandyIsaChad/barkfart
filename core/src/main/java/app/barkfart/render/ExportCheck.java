package app.barkfart.render;

import app.barkfart.audio.AudioPlan;
import app.barkfart.audio.Bounce;
import app.barkfart.audio.Mixer;
import app.barkfart.audio.Pcm;
import app.barkfart.export.ExportJob;
import app.barkfart.export.VideoSink;
import app.barkfart.fx.EffectScene;
import app.barkfart.model.Clip;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.platform.Platform;
import java.io.File;
import java.io.FileOutputStream;

/**
 * Exports the effect scene with the real exporter (GL thread) and checks the
 * files: GIF and PNG frames right away, the video when its encoder finishes.
 */
public final class ExportCheck {
    private ExportCheck() {}

    static final int W = 128, H = 72, FRAMES = 6;
    static final double FPS = 12;

    private static Project scene() {
        Project p = EffectScene.build();
        p.settings.width = EffectScene.W;
        p.settings.height = EffectScene.H;
        p.root().duration = FRAMES / FPS;
        return p;
    }

    private static ExportJob.Frames frames(final Project p, final boolean clear) {
        final FrameGrabber g = new FrameGrabber(p, W, H);
        return new ExportJob.Frames() {
            private boolean disposed;

            public byte[] grab(double t) {
                return g.grab(p, t, clear);
            }

            public void dispose() {
                if (!disposed) g.dispose();
                disposed = true;
            }
        };
    }

    /** GIF and PNG frames; null when both files check out. */
    public static String files(File dir) {
        File gif = new File(dir, ".check.gif"), zip = new File(dir, ".check.zip");
        try {
            Project p = scene();
            ExportJob g = new ExportJob(ExportJob.Format.GIF, gif, W, H, FRAMES, FPS, false, frames(p, false), null);
            while (g.state() == ExportJob.State.RUNNING) g.step();
            if (g.state() != ExportJob.State.DONE) return "GIF: " + g.problem();
            byte[] b = java.nio.ByteBuffer.allocate(6).array();
            java.io.FileInputStream in = new java.io.FileInputStream(gif);
            try {
                if (in.read(b) != 6 || !new String(b, "US-ASCII").equals("GIF89a")) return "GIF: bad header";
            } finally {
                in.close();
            }
            ExportJob z = new ExportJob(ExportJob.Format.PNG, zip, W, H, FRAMES, FPS, true, frames(p, true), null);
            while (z.state() == ExportJob.State.RUNNING) z.step();
            if (z.state() != ExportJob.State.DONE) return "PNG: " + z.problem();
            java.util.zip.ZipFile zf = new java.util.zip.ZipFile(zip);
            try {
                if (zf.size() != FRAMES) return "PNG: " + zf.size() + " frames, expected " + FRAMES;
            } finally {
                zf.close();
            }
            return null;
        } catch (Exception e) {
            return String.valueOf(e);
        } finally {
            gif.delete();
            zip.delete();
        }
    }

    public interface Result {
        void result(String problem);
    }

    /**
     * Video with a tone, then reopened and checked; result arrives later on
     * another thread. Platforms without video report so at once.
     */
    public static void video(final Platform platform, final File dir, final Result result) {
        if (!platform.canEncodeVideo()) {
            result.result(null);   // nothing to check here (desktop)
            return;
        }
        try {
            Project p = scene();
            Layer.AudioLayer a = new Layer.AudioLayer(p.newId("L"), "tone");
            a.clips.add(new Clip(a.id, "c", "T", 0, 0, 0));
            p.root().layers.add(a);
            p.invalidateIndex();
            final Pcm tone = Mixer.tone(440, FRAMES / FPS, 44100, 0.2f);
            byte[] wav = Bounce.wav(AudioPlan.build(p, p.root(), new AudioPlan.Sounds() {
                public Pcm pcm(String id) {
                    return tone;
                }
            }), FRAMES / FPS);
            final File wf = new File(dir, ".check.wav"), mp4 = new File(dir, ".check.mp4");
            FileOutputStream o = new FileOutputStream(wf);
            try {
                o.write(wav);
            } finally {
                o.close();
            }
            VideoSink v = platform.openVideo(mp4, W, H, FPS, wf);
            if (v == null) {
                wf.delete();
                result.result("could not open the video encoder");
                return;
            }
            final ExportJob job = new ExportJob(ExportJob.Format.VIDEO, mp4, W, H, FRAMES, FPS, false, frames(p, false), v);
            long deadline = System.currentTimeMillis() + 10000;
            while (job.state() == ExportJob.State.RUNNING && System.currentTimeMillis() < deadline) job.step();
            if (job.state() == ExportJob.State.RUNNING) {
                job.cancel();
                wf.delete();
                result.result("encoder stayed busy for 10 s");
                return;
            }
            Thread t = new Thread(new Runnable() {
                public void run() {
                    long end = System.currentTimeMillis() + 20000;
                    while (job.state() == ExportJob.State.FINISHING && System.currentTimeMillis() < end) {
                        try {
                            Thread.sleep(20);
                        } catch (InterruptedException e) {
                            break;
                        }
                    }
                    String r;
                    if (job.state() == ExportJob.State.DONE) r = platform.verifyVideo(mp4, W, H, FRAMES / FPS, true);
                    else if (job.state() == ExportJob.State.FINISHING) r = "the encoder did not finish in 20 s";
                    else r = job.problem();
                    mp4.delete();
                    wf.delete();
                    result.result(r);
                }
            }, "video-check");
            t.setDaemon(true);
            t.start();
        } catch (Exception e) {
            result.result(String.valueOf(e));
        }
    }
}
