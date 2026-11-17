package app.barkfart.ios;

import app.barkfart.audio.Pcm;
import app.barkfart.audio.PumpOutput;
import java.io.File;
import java.io.FileOutputStream;
import java.util.concurrent.Semaphore;
import java.util.concurrent.TimeUnit;
import org.robovm.apple.avfoundation.AVAudioCommonFormat;
import org.robovm.apple.avfoundation.AVAudioEngine;
import org.robovm.apple.avfoundation.AVAudioFile;
import org.robovm.apple.avfoundation.AVAudioFormat;
import org.robovm.apple.avfoundation.AVAudioPCMBuffer;
import org.robovm.apple.avfoundation.AVAudioPlayerNode;
import org.robovm.apple.avfoundation.AVAudioSession;
import org.robovm.apple.avfoundation.AVAudioSessionCategory;
import org.robovm.apple.foundation.NSErrorException;
import org.robovm.apple.foundation.NSURL;
import org.robovm.rt.bro.ptr.FloatPtr;

/**
 * iOS sound: AVAudioEngine with one player node, fed by the shared pump
 * thread in blocks (at most three queued; each completion frees a slot).
 * Decoding (mp3, m4a, wav, caf, aiff) through AVAudioFile. Errors are
 * returned, never thrown.
 */
final class IOSAudio extends PumpOutput {
    private static final int QUEUED = 3;
    private AVAudioEngine engine;
    private AVAudioPlayerNode player;
    private AVAudioFormat format;
    private final Semaphore slots = new Semaphore(QUEUED);

    protected String open() {
        if (engine != null) return null;
        try {
            AVAudioSession.getSharedInstance().setCategory(AVAudioSessionCategory.Playback);
            AVAudioSession.getSharedInstance().setActive(true);
        } catch (NSErrorException e) {
            // keep going: playback works without the category, just obeys the silent switch
        }
        try {
            format = new AVAudioFormat(AVAudioCommonFormat.PCMFormatFloat32, RATE, 2, false);
            engine = new AVAudioEngine();
            player = new AVAudioPlayerNode();
            engine.attachNode(player);
            engine.connect(player, engine.getMainMixerNode(), format);
            engine.prepare();
            engine.start();
            player.play();
            slots.drainPermits();
            slots.release(QUEUED);
            return null;
        } catch (NSErrorException e) {
            close();
            return "audio engine: " + e.getError().getLocalizedDescription();
        } catch (RuntimeException e) {
            close();
            return "audio engine: " + e;
        }
    }

    protected boolean write(float[] lr, int frames) {
        AVAudioPlayerNode p = player;
        if (p == null) return false;
        try {
            if (!slots.tryAcquire(2, TimeUnit.SECONDS)) return false;   // device stalled (e.g. route change): stop
        } catch (InterruptedException e) {
            return false;
        }
        AVAudioPCMBuffer buf = new AVAudioPCMBuffer(format, frames);
        buf.setFrameLength(frames);
        float[] l = new float[frames], r = new float[frames];
        for (int i = 0; i < frames; i++) {
            l[i] = lr[i * 2];
            r[i] = lr[i * 2 + 1];
        }
        FloatPtr.FloatPtrPtr ch = buf.getFloatChannelData();
        ch.get().set(l);
        ch.next().get().set(r);
        p.scheduleBuffer(buf, new Runnable() {
            public void run() {
                slots.release();
            }
        });
        return true;
    }

    protected String drained(long millis) {
        try {
            if (!slots.tryAcquire(QUEUED, millis, TimeUnit.MILLISECONDS)) return "audio output did not play (no completion)";
            slots.release(QUEUED);
            return null;
        } catch (InterruptedException e) {
            return "interrupted";
        }
    }

    protected void close() {
        if (player != null) player.stop();
        if (engine != null) engine.stop();
        player = null;
        engine = null;
        slots.drainPermits();
        slots.release(QUEUED);
    }

    @Override
    protected Pcm decodeOther(String name, byte[] bytes) {
        File tmp = null;
        try {
            String ext = name.lastIndexOf('.') >= 0 ? name.substring(name.lastIndexOf('.')).toLowerCase() : ".dat";
            tmp = File.createTempFile("snd", ext);
            FileOutputStream out = new FileOutputStream(tmp);
            try {
                out.write(bytes);
            } finally {
                out.close();
            }
            AVAudioFile f = new AVAudioFile(new NSURL(tmp));
            AVAudioFormat pf = f.getProcessingFormat();   // float, deinterleaved
            int rate = (int) Math.round(pf.getSampleRate());
            int channels = (int) pf.getChannelCount();
            long len = f.getLength();
            int frames = (int) Math.min(len, (long) rate * Pcm.MAX_SECONDS);
            if (frames <= 0 || channels <= 0 || rate <= 0) return null;
            AVAudioPCMBuffer buf = new AVAudioPCMBuffer(pf, frames);
            f.readIntoBuffer(buf, frames);
            int got = buf.getFrameLength();
            FloatPtr.FloatPtrPtr ch = buf.getFloatChannelData();
            int use = Math.min(2, channels);
            float[][] data = new float[use][];
            for (int c = 0; c < use; c++) {
                data[c] = new float[got];
                ch.get().get(data[c]);
                ch = ch.next();
            }
            return Pcm.fromFloat(rate, data, got);
        } catch (NSErrorException e) {
            return null;
        } catch (Exception e) {
            return null;
        } finally {
            if (tmp != null) tmp.delete();
        }
    }
}
