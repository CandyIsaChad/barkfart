package app.barkfart.export;

import java.io.BufferedOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

/**
 * Renders a project frame by frame into a file. step() renders and writes one
 * frame and returns quickly, so the UI calls it once per render tick (or a few
 * times within a time budget): the app stays responsive and can cancel.
 */
public final class ExportJob {
    public enum Format { VIDEO, GIF, PNG }

    public enum State { RUNNING, FINISHING, DONE, FAILED, CANCELLED }

    /** Renders frame times to straight RGBA8, rows top-down (GL thread). */
    public interface Frames {
        byte[] grab(double t);

        void dispose();
    }

    public final Format format;
    public final File out;
    public final int w, h, frameCount, startFrame;
    public final double fps;
    private final Frames frames;
    private final VideoSink video;
    private OutputStream stream;
    private ZipOutputStream zip;
    private Gif gif;
    private int next;
    private byte[] pending;   // a video frame the encoder was not ready for
    private volatile State state = State.RUNNING;
    private volatile String problem;

    public ExportJob(Format format, File out, int w, int h, int frameCount, double fps, boolean transparent, Frames frames, VideoSink video) throws IOException {
        this(format, out, w, h, 0, frameCount, fps, transparent, frames, video);
    }

    /** Exports frameCount frames starting at startFrame (an export range); the file starts at time 0. */
    public ExportJob(Format format, File out, int w, int h, int startFrame, int frameCount, double fps, boolean transparent, Frames frames, VideoSink video) throws IOException {
        this.startFrame = Math.max(0, startFrame);
        this.format = format;
        this.out = out;
        this.w = w;
        this.h = h;
        this.frameCount = Math.max(1, frameCount);
        this.fps = fps > 0 ? fps : 30;
        this.frames = frames;
        this.video = video;
        if (format == Format.VIDEO && video == null) throw new IOException("video export is not available on this device");
        // the video encoder already created (and owns) its file; other formats replace any old one
        if (format != Format.VIDEO && out.exists() && !out.delete()) throw new IOException("cannot replace " + out.getName());
        if (format == Format.PNG) {
            stream = new FileOutputStream(out);
            zip = new ZipOutputStream(new BufferedOutputStream(stream, 1 << 16));
        } else if (format == Format.GIF) {
            stream = new BufferedOutputStream(new FileOutputStream(out), 1 << 16);
            gif = new Gif(stream, w, h, this.fps, true);
        }
        this.transparent = transparent;
    }

    private final boolean transparent;

    public State state() {
        return state;
    }

    public String problem() {
        return problem;
    }

    /** Frames written so far. */
    public int done() {
        return next;
    }

    /** One unit of work. Call repeatedly while RUNNING (GL thread). */
    public void step() {
        if (state != State.RUNNING) return;
        try {
            if (next >= frameCount) {
                finish();
                return;
            }
            byte[] px = pending != null ? pending : frames.grab((startFrame + next) / fps);
            pending = null;
            switch (format) {
                case PNG: {
                    zip.putNextEntry(new ZipEntry(String.format("frame_%04d.png", next + 1)));
                    zip.write(Png.encode(px, w, h, transparent));
                    zip.closeEntry();
                    next++;
                    break;
                }
                case GIF:
                    gif.frame(px);
                    next++;
                    break;
                default: {
                    VideoSink.Accept a = video.frame(px, next);
                    if (a == VideoSink.Accept.BUSY) pending = px;
                    else if (a == VideoSink.Accept.FAILED) fail(video.problem());
                    else next++;
                }
            }
        } catch (IOException e) {
            fail(e.getMessage());
        } catch (RuntimeException e) {
            fail(String.valueOf(e));
        }
    }

    private void finish() throws IOException {
        if (format == Format.VIDEO) {
            state = State.FINISHING;
            frames.dispose();
            video.finish(new VideoSink.Done() {
                public void done(String p) {
                    if (p == null) state = State.DONE;
                    else {
                        problem = p;
                        state = State.FAILED;
                        out.delete();
                    }
                }
            });
            return;
        }
        if (zip != null) zip.close();
        if (gif != null) {
            gif.finish();
            stream.close();
        }
        frames.dispose();
        state = State.DONE;
    }

    private void fail(String why) {
        problem = why == null ? "export failed" : why;
        state = State.FAILED;
        cleanup();
    }

    public void cancel() {
        if (state == State.RUNNING || state == State.FINISHING) {
            state = State.CANCELLED;
            cleanup();
        }
    }

    private void cleanup() {
        try {
            if (zip != null) zip.close();
            else if (stream != null) stream.close();
        } catch (IOException e) {
            // already failing
        }
        if (video != null) video.cancel();
        frames.dispose();
        out.delete();
    }
}
