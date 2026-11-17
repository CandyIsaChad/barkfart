package app.barkfart.audio;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.model.Clip;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import java.util.HashMap;
import java.util.Map;
import org.junit.jupiter.api.Test;

class AudioTest {
    static final int RATE = 44100;

    /** Project with one audio layer and the given clip; sounds by asset id. */
    static Project project(Clip c, double duration) {
        Project p = Fixtures.empty();
        p.root().duration = duration;
        Layer.AudioLayer a = new Layer.AudioLayer(p.newId("L"), "audio");
        a.clips.add(c);
        p.root().layers.add(a);
        p.invalidateIndex();
        return p;
    }

    static float[] mix(AudioPlan plan, double t0, int frames) {
        float[] out = new float[frames * 2];
        Mixer.mix(plan, t0, frames, RATE, out);
        return out;
    }

    static double rms(float[] st, int ch, int from, int to) {
        double s = 0;
        for (int i = from; i < to; i++) s += st[i * 2 + ch] * st[i * 2 + ch];
        return Math.sqrt(s / Math.max(1, to - from));
    }

    static AudioPlan.Sounds sounds(final Map<String, Pcm> m) {
        return new AudioPlan.Sounds() {
            public Pcm pcm(String id) {
                return m.get(id);
            }
        };
    }

    @Test
    void wavRoundTripsAndOtherDepthsDecode() {
        Pcm tone = Mixer.tone(440, 0.1, RATE, 0.5f);
        Pcm back = Wav.decode(Wav.encode(tone));
        assertNotNull(back);
        assertEquals(RATE, back.rate);
        assertEquals(1, back.channels);
        assertArrayEquals(tone.data, back.data);
        short[] st = {100, -100, 2000, -2000};
        Pcm stereo = Wav.decode(Wav.encode(new Pcm(22050, 2, st)));
        assertEquals(2, stereo.channels);
        assertEquals(2, stereo.frames);
        assertArrayEquals(st, stereo.data);
        // 32-bit float, 3 channels (downmixed to stereo: first two kept)
        byte[] f = Wav.encode(new Pcm(8000, 2, new short[] {0, 0}));
        f[20] = 3;           // format float
        f[22] = 3;           // channels
        f[34] = 32;          // bits
        byte[] body = new byte[12];
        java.nio.ByteBuffer.wrap(body).order(java.nio.ByteOrder.LITTLE_ENDIAN).putFloat(0.5f).putFloat(-0.25f).putFloat(1f);
        byte[] g = new byte[44 + 12];
        System.arraycopy(f, 0, g, 0, 44);
        java.nio.ByteBuffer.wrap(g, 40, 4).order(java.nio.ByteOrder.LITTLE_ENDIAN).putInt(12);
        System.arraycopy(body, 0, g, 44, 12);
        Pcm fl = Wav.decode(g);
        assertNotNull(fl);
        assertEquals(2, fl.channels);
        assertEquals(Pcm.s16(0.5f), fl.data[0]);
        assertEquals(Pcm.s16(-0.25f), fl.data[1]);
    }

    @Test
    void garbageNeverThrows() {
        assertNull(Wav.decode(null));
        assertNull(Wav.decode(new byte[3]));
        assertNull(Wav.decode("RIFF____WAVEjunkjunkjunk".getBytes()));
        byte[] w = Wav.encode(Mixer.tone(440, 0.05, RATE, 0.5f));
        java.util.Random r = new java.util.Random(7);
        for (int i = 0; i < 2000; i++) {
            byte[] b = java.util.Arrays.copyOf(w, r.nextInt(w.length + 1));
            if (b.length > 0) b[r.nextInt(b.length)] = (byte) r.nextInt();
            Wav.decode(b);   // must not throw
        }
    }

    @Test
    void clipsStartOnTimeTrimAndLoop() {
        Map<String, Pcm> m = new HashMap<String, Pcm>();
        m.put("A", Mixer.tone(440, 1.0, RATE, 0.5f));
        Clip c = new Clip("L", "c", "A", 0.5, 0.25, 0.5);   // 0.25 s of source from 0.5 s
        AudioPlan plan = AudioPlan.build(project(c, 3), project(c, 3).root(), sounds(m));
        float[] a = mix(plan, 0, RATE);
        assertEquals(0, rms(a, 0, 0, RATE / 2 - 10), 1e-6, "silent before the start");
        assertTrue(rms(a, 0, RATE / 2 + 500, RATE * 3 / 4 - 500) > 0.2, "plays");
        assertEquals(0, rms(a, 0, RATE * 3 / 4 + 10, RATE), 1e-6, "stops after 0.25 s");
        c.loop = true;
        Project p = project(c, 3);
        plan = AudioPlan.build(p, p.root(), sounds(m));
        float[] b = mix(plan, 2.0, RATE / 2);
        assertTrue(rms(b, 0, 0, RATE / 2) > 0.2, "loops until the end of the timeline");
        assertEquals(0, rms(mix(plan, 3.0, 1000), 0, 0, 1000), 1e-6, "and stops there");
    }

    @Test
    void fadesVolumeKeysAndPan() {
        Map<String, Pcm> m = new HashMap<String, Pcm>();
        m.put("A", Mixer.tone(440, 2.0, RATE, 0.5f));
        Clip c = new Clip("L", "c", "A", 0, 0, 0);
        c.fadeIn = 0.5;
        Project p = project(c, 3);
        float[] a = mix(AudioPlan.build(p, p.root(), sounds(m)), 0, RATE);
        assertTrue(rms(a, 0, 0, 2205) < rms(a, 0, RATE / 2, RATE / 2 + 2205) / 4, "fades in");
        c.fadeIn = 0;
        c.volume.track().put(new Key(0, 1f, Interp.LINEAR));
        c.volume.track().put(new Key(1, 0.5f, Interp.HOLD));
        p = project(c, 3);
        float[] b = mix(AudioPlan.build(p, p.root(), sounds(m)), 1.2, 4410);
        c.volume.track().clear();
        p = project(c, 3);
        float[] full = mix(AudioPlan.build(p, p.root(), sounds(m)), 1.2, 4410);
        assertEquals(0.5, rms(b, 0, 0, 4410) / rms(full, 0, 0, 4410), 0.03, "volume key halves the level");
        c.volume.track().clear();
        c.pan.setValue(-1f);
        p = project(c, 3);
        float[] left = mix(AudioPlan.build(p, p.root(), sounds(m)), 0.5, 4410);
        assertTrue(rms(left, 0, 0, 4410) > 0.3);
        assertEquals(0, rms(left, 1, 0, 4410), 1e-4, "hard left: nothing on the right");
        c.pan.setValue(0f);
        float[] centre = mix(AudioPlan.build(project(c, 3), project(c, 3).root(), sounds(m)), 0.5, 4410);
        assertEquals(rms(centre, 0, 0, 4410), rms(centre, 1, 0, 4410), 1e-4);
        assertEquals(0.5 / Math.sqrt(2), rms(centre, 0, 0, 4410), 0.02, "constant power: centre keeps the mono level");
    }

    @Test
    void rateConversionKeepsPitch() {
        Map<String, Pcm> m = new HashMap<String, Pcm>();
        m.put("A", Mixer.tone(440, 1.0, 22050, 0.5f));
        Clip c = new Clip("L", "c", "A", 0, 0, 0);
        Project p = project(c, 3);
        float[] a = mix(AudioPlan.build(p, p.root(), sounds(m)), 0.1, RATE / 2);
        int crossings = 0;
        for (int i = 1; i < RATE / 2; i++) if ((a[(i - 1) * 2] < 0) != (a[i * 2] < 0)) crossings++;
        assertEquals(440, crossings, 4, "440 Hz stays 440 Hz (two crossings per cycle over 0.5 s)");
    }

    @Test
    void missingHiddenAndBrokenClipsAreSilent() {
        Map<String, Pcm> m = new HashMap<String, Pcm>();
        m.put("A", Mixer.tone(440, 1.0, RATE, 0.5f));
        Clip missing = new Clip("L", "c", "nope", 0, 0, 0);
        Project p = project(missing, 3);
        AudioPlan plan = AudioPlan.build(p, p.root(), sounds(m));
        assertTrue(plan.entries.isEmpty());
        assertFalse(plan.diagnostics.isEmpty());
        Clip c = new Clip("L", "c", "A", Double.NaN, 0.8, 0.2);
        p = project(c, 3);
        assertTrue(AudioPlan.build(p, p.root(), sounds(m)).entries.isEmpty(), "NaN start, out before in");
        Clip ok = new Clip("L", "c", "A", 0, 0, 0);
        p = project(ok, 3);
        p.root().layers.get(0).visible = false;
        assertTrue(AudioPlan.build(p, p.root(), sounds(m)).entries.isEmpty(), "hidden layer is muted");
        float[] out = new float[20];
        Mixer.mix(AudioPlan.build(project(ok, 3), project(ok, 3).root(), sounds(m)), Double.NaN, 10, RATE, out);
        for (float v : out) assertEquals(0, v);
    }

    @Test
    void loudMixesClamp() {
        Map<String, Pcm> m = new HashMap<String, Pcm>();
        m.put("A", Mixer.tone(440, 1.0, RATE, 1f));
        Project p = Fixtures.empty();
        p.root().duration = 2;
        Layer.AudioLayer a = new Layer.AudioLayer(p.newId("L"), "audio");
        for (int i = 0; i < 5; i++) a.clips.add(new Clip(a.id, "c" + i, "A", 0, 0, 0));
        p.root().layers.add(a);
        float[] out = mix(AudioPlan.build(p, p.root(), sounds(m)), 0, 4410);
        for (float v : out) assertTrue(v >= -1 && v <= 1);
    }

    @Test
    void importingASoundIsOneUndoStepAndOggIsRefusedClearly() throws Exception {
        Project p = Fixtures.empty();
        app.barkfart.edit.Document d = new app.barkfart.edit.Document(p);
        String before = app.barkfart.io.ProjectCodec.encode(p);
        java.io.File dir = java.nio.file.Files.createTempDirectory("snd").toFile();
        java.io.File w = new java.io.File(dir, "boom.wav");
        java.nio.file.Files.write(w.toPath(), Wav.encode(Mixer.tone(220, 0.5, RATE, 0.5f)));
        app.barkfart.files.Importer.Result r = app.barkfart.files.Importer.importFile(d, w, AudioBackend.NONE, 1.25);
        Layer.AudioLayer a = (Layer.AudioLayer) r.layer;
        assertEquals(1.25, a.clips.get(0).start, 1e-9);
        assertEquals(0.5, a.clips.get(0).out, 1e-3);
        assertNotNull(p.assetData.bytes(a.clips.get(0).asset));
        AudioPlan plan = AudioPlan.build(p, p.root(), new AudioPlan.Sounds() {
            public Pcm pcm(String id) {
                try {
                    return Wav.decode(p.assetData.bytes(id));
                } catch (java.io.IOException e) {
                    return null;
                }
            }
        });
        assertEquals(1, plan.entries.size());
        d.undo();
        // ids are never handed back on undo (the counter only grows), so compare without it
        assertEquals(before.replaceAll("\"idCounter\": \\d+", ""), app.barkfart.io.ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
        assertFalse(p.assetData.has(a.clips.get(0).asset));
        java.io.File o = new java.io.File(dir, "x.ogg");
        java.nio.file.Files.write(o.toPath(), "OggS-not-really".getBytes());
        try {
            app.barkfart.files.Importer.importFile(d, o, AudioBackend.NONE, 0);
            fail("ogg should be refused");
        } catch (app.barkfart.files.Importer.Unsupported e) {
            assertTrue(e.getMessage().contains("ogg is not supported"), e.getMessage());
        }
    }
}
