package app.barkfart.audio;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.audio.AudioDevice;

/** Output through LibGDX's AudioDevice (desktop; iOS returns none, see IOSAudio). WAV decoding only. */
public final class GdxAudio extends PumpOutput {
    private AudioDevice device;

    protected String open() {
        if (Gdx.audio == null) return "no audio";
        if (device == null) device = Gdx.audio.newAudioDevice(RATE, false);
        return device == null ? "this platform has no audio device" : null;
    }

    protected boolean write(float[] lr, int frames) {
        AudioDevice d = device;
        if (d == null) return false;
        d.writeSamples(lr, 0, frames * 2);
        return true;
    }

    protected void close() {
        // keep the device open between plays (opening is slow); disposed with the app
    }
}
