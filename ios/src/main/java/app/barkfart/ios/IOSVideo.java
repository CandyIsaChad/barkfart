package app.barkfart.ios;

import app.barkfart.export.VideoSink;
import java.io.File;
import org.robovm.apple.avfoundation.AVAssetReader;
import org.robovm.apple.avfoundation.AVAssetReaderStatus;
import org.robovm.apple.avfoundation.AVAssetReaderTrackOutput;
import org.robovm.apple.avfoundation.AVAssetTrack;
import org.robovm.apple.avfoundation.AVAssetWriter;
import org.robovm.apple.avfoundation.AVAssetWriterInput;
import org.robovm.apple.avfoundation.AVAssetWriterInputPixelBufferAdaptor;
import org.robovm.apple.avfoundation.AVAssetWriterStatus;
import org.robovm.apple.avfoundation.AVAudioSettings;
import org.robovm.apple.avfoundation.AVMediaType;
import org.robovm.apple.avfoundation.AVURLAsset;
import org.robovm.apple.avfoundation.AVVideoCodecType;
import org.robovm.apple.avfoundation.AVVideoSettings;
import org.robovm.apple.coreaudio.AudioFormat;
import org.robovm.apple.coremedia.CMSampleBuffer;
import org.robovm.apple.coremedia.CMTime;
import org.robovm.apple.corevideo.CVPixelBuffer;
import org.robovm.apple.corevideo.CVPixelBufferAttributes;
import org.robovm.apple.corevideo.CVPixelBufferLockFlags;
import org.robovm.apple.corevideo.CVPixelBufferPool;
import org.robovm.apple.corevideo.CVPixelFormatType;
import org.robovm.apple.foundation.NSArray;
import org.robovm.apple.foundation.NSError;
import org.robovm.apple.foundation.NSErrorException;
import org.robovm.apple.foundation.NSURL;
import org.robovm.rt.bro.ptr.BytePtr;

/**
 * H.264 video (+ AAC audio read from a WAV of the mixed timeline) into an
 * .mp4 with AVAssetWriter. Frames come from the GL thread one at a time and
 * never block it: when the encoder is not ready the frame is offered again
 * later. Audio is interleaved: before each frame, audio up to half a second
 * past it is appended.
 */
final class IOSVideo implements VideoSink {
    private final int w, h;
    private final double fps;
    private final File out;
    private AVAssetWriter writer;
    private AVAssetWriterInput video, audio;
    private AVAssetWriterInputPixelBufferAdaptor adaptor;
    private AVAssetReader reader;
    private AVAssetReaderTrackOutput audioOut;
    private boolean audioDone = true;
    private double audioTime;
    private volatile String problem;
    private byte[] row;

    IOSVideo(File out, int w, int h, double fps, File wav) throws Exception {
        this.out = out;
        this.w = w;
        this.h = h;
        this.fps = fps > 0 ? fps : 30;
        out.delete();
        writer = new AVAssetWriter(new NSURL(out), "public.mpeg-4");
        video = new AVAssetWriterInput(AVMediaType.Video, new AVVideoSettings()
                .setCodec(AVVideoCodecType.AVVideoCodecTypeH264).setWidth(w).setHeight(h));
        video.setExpectsMediaDataInRealTime(false);
        adaptor = new AVAssetWriterInputPixelBufferAdaptor(video, new CVPixelBufferAttributes()
                .setPixelFormatType(CVPixelFormatType._32BGRA).setWidth(w).setHeight(h));
        writer.addInput(video);
        if (wav != null) {
            AVURLAsset asset = new AVURLAsset(new NSURL(wav), null);
            NSArray<AVAssetTrack> tracks = asset.getTracksWithType(AVMediaType.Audio);
            if (tracks != null && tracks.size() > 0) {
                reader = new AVAssetReader(asset);
                audioOut = new AVAssetReaderTrackOutput(tracks.get(0), new AVAudioSettings().setFormat(AudioFormat.LinearPCM));
                reader.addOutput(audioOut);
                audio = new AVAssetWriterInput(AVMediaType.Audio, new AVAudioSettings()
                        .setFormat(AudioFormat.MPEG4AAC).setSampleRate(44100).setNumberOfChannels(2));
                audio.setExpectsMediaDataInRealTime(false);
                writer.addInput(audio);
                if (!reader.startReading()) throw new IllegalStateException("could not read the soundtrack");
                audioDone = false;
            }
        }
        if (!writer.startWriting()) throw new IllegalStateException("could not start writing: " + error());
        writer.startSession(CMTime.Zero());
        row = new byte[w * 4];
    }

    private String error() {
        NSError e = writer == null ? null : writer.getError();
        return e == null ? "unknown error" : e.getLocalizedDescription();
    }

    /** Appends audio until it is `until` seconds ahead, or the encoder is busy, or it runs out. */
    private void pumpAudio(double until) {
        while (!audioDone && audioTime < until && audio.isReadyForMoreMediaData()) {
            CMSampleBuffer sb = audioOut.getNextSampleBuffer();
            if (sb == null) {
                audioDone = true;
                audio.markAsFinished();
                break;
            }
            audio.appendSampleBuffer(sb);
            audioTime = sb.getPresentationTimeStamp().getSeconds() + sb.getDuration().getSeconds();
        }
    }

    public Accept frame(byte[] rgba, int index) {
        try {
            if (writer.getStatus() == AVAssetWriterStatus.Failed) {
                problem = "video encoder: " + error();
                return Accept.FAILED;
            }
            pumpAudio((index + 1) / fps + 0.5);
            if (!video.isReadyForMoreMediaData()) return Accept.BUSY;
            CVPixelBufferPool pool = adaptor.getPixelBufferPool();
            CVPixelBuffer pb = pool != null ? pool.createPixelBuffer()
                    : CVPixelBuffer.create(w, h, CVPixelFormatType._32BGRA, null);
            pb.lockBaseAddress(CVPixelBufferLockFlags.None);
            try {
                BytePtr base = pb.getBaseAddress().as(BytePtr.class);
                long bpr = pb.getBytesPerRow();
                for (int y = 0; y < h; y++) {
                    int s = y * w * 4;
                    for (int x = 0; x < w * 4; x += 4) {   // RGBA -> BGRA
                        row[x] = rgba[s + x + 2];
                        row[x + 1] = rgba[s + x + 1];
                        row[x + 2] = rgba[s + x];
                        row[x + 3] = (byte) 255;
                    }
                    base.next(y * bpr).set(row);
                }
            } finally {
                pb.unlockBaseAddress(CVPixelBufferLockFlags.None);
            }
            if (!adaptor.appendPixelBuffer(pb, CMTime.create(index * 1000L, (int) Math.round(fps * 1000)))) {
                problem = "video encoder: " + error();
                return Accept.FAILED;
            }
            return Accept.OK;
        } catch (RuntimeException e) {
            problem = "video: " + e;
            return Accept.FAILED;
        }
    }

    public void finish(final Done done) {
        Thread t = new Thread(new Runnable() {
            public void run() {
                try {
                    video.markAsFinished();
                    long deadline = System.currentTimeMillis() + 60000;
                    while (!audioDone && System.currentTimeMillis() < deadline) {
                        pumpAudio(Double.MAX_VALUE);
                        if (!audioDone) Thread.sleep(5);
                    }
                    if (reader != null && reader.getStatus() == AVAssetReaderStatus.Failed) {
                        done.done("could not read the soundtrack");
                        return;
                    }
                    writer.finishWriting(new Runnable() {
                        public void run() {
                            done.done(writer.getStatus() == AVAssetWriterStatus.Completed ? null : "video encoder: " + error());
                        }
                    });
                } catch (Exception e) {
                    done.done("video: " + e);
                }
            }
        }, "video-finish");
        t.setDaemon(true);
        t.start();
    }

    public void cancel() {
        try {
            if (reader != null) reader.cancelReading();
            if (writer != null && writer.getStatus() == AVAssetWriterStatus.Writing) writer.cancelWriting();
        } catch (RuntimeException e) {
            // cancelling anyway
        }
        out.delete();
    }

    public String problem() {
        return problem;
    }

    /** Opens a sink, or returns null and keeps the reason in why[0]. */
    static VideoSink open(File out, int w, int h, double fps, File wav, String[] why) {
        try {
            return new IOSVideo(out, w, h, fps, wav);
        } catch (NSErrorException e) {
            why[0] = e.getError().getLocalizedDescription();
        } catch (Exception e) {
            why[0] = String.valueOf(e);
        }
        return null;
    }
}
