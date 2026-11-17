package app.barkfart.desktop;

import app.barkfart.BarkfartApp;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Application;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3ApplicationConfiguration;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.PixmapIO;

public class DesktopLauncher {
    /**
     * Args (any order):
     *   --phone                 iPhone landscape preview: 844 x 390 pt at 2x
     *   --ipad                  iPad Air landscape preview: 1180 x 820 pt at 1x
     *   --screenshot out.png N  render N frames (default 45), save, exit
     *   --script steps          (with --screenshot) comma list of actions run before capture
     *   --fxcheck out.png       render the effect scene with GL and the reference, compare, save both, exit
     */
    public static void main(String[] args) {
        boolean phone = false, ipad = false;
        String shot = null;
        int frames = 45;
        String script = null;
        String fxcheck = null;
        for (int i = 0; i < args.length; i++) {
            if ("--fxcheck".equals(args[i])) fxcheck = args[++i];
            if ("--phone".equals(args[i])) phone = true;
            else if ("--ipad".equals(args[i])) ipad = true;
            else if ("--screenshot".equals(args[i])) {
                shot = args[++i];
                if (i + 1 < args.length && args[i + 1].matches("\\d+")) frames = Integer.parseInt(args[++i]);
            } else if ("--script".equals(args[i])) script = args[++i];
        }
        Lwjgl3ApplicationConfiguration config = new Lwjgl3ApplicationConfiguration();
        config.setTitle("Barkfart");
        config.setWindowedMode(phone ? 1688 : (ipad ? 1180 : 1280), phone ? 780 : (ipad ? 820 : 720));
        config.useVsync(true);
        config.setBackBufferConfig(8, 8, 8, 8, 16, 0, 4);
        float scale = phone ? 2f : (ipad ? 1f : 1.5f);
        String docs = System.getProperty("barkfart.docs", System.getProperty("user.home") + "/Barkfart");
        final app.barkfart.platform.Platform platform = new app.barkfart.platform.Platform.Basic(new java.io.File(docs));
        if (fxcheck != null) {
            final String out = fxcheck;
            config.setWindowedMode(320, 240);
            new Lwjgl3Application(new com.badlogic.gdx.ApplicationAdapter() {
                @Override
                public void create() {
                    String r = app.barkfart.render.EffectCheck.run();
                    System.out.println("[fxcheck] " + (r == null ? "OK: GL effects match the reference" : r));
                    app.barkfart.fx.Image ref = app.barkfart.render.EffectCheck.lastRef;
                    byte[] gl = app.barkfart.render.EffectCheck.lastGl;
                    if (ref != null) {
                        int w = ref.w, h = ref.h;
                        Pixmap pm = new Pixmap(w * 2 + 4, h, Pixmap.Format.RGB888);
                        pm.setColor(1, 0, 0, 1);
                        pm.fill();
                        for (int y = 0; y < h; y++) {
                            for (int x = 0; x < w; x++) {
                                int o = (y * w + x) * 4;
                                float a = ref.px[o + 3];
                                pm.drawPixel(x, y, rgb(ref.px[o] + 1 - a, ref.px[o + 1] + 1 - a, ref.px[o + 2] + 1 - a));
                                if (gl != null) {
                                    int g = ((h - 1 - y) * w + x) * 4;
                                    float ga = (gl[g + 3] & 0xff) / 255f;
                                    pm.drawPixel(w + 4 + x, y, rgb((gl[g] & 0xff) / 255f + 1 - ga, (gl[g + 1] & 0xff) / 255f + 1 - ga, (gl[g + 2] & 0xff) / 255f + 1 - ga));
                                }
                            }
                        }
                        PixmapIO.writePNG(Gdx.files.absolute(out), pm);
                        pm.dispose();
                    }
                    Gdx.app.exit();
                }

                private int rgb(float r, float g, float b) {
                    int ri = Math.round(Math.min(1, r) * 255), gi = Math.round(Math.min(1, g) * 255), bi = Math.round(Math.min(1, b) * 255);
                    return (ri << 24) | (gi << 16) | (bi << 8) | 0xff;
                }
            }, config);
            return;
        }
        if (shot == null) {
            new Lwjgl3Application(new BarkfartApp(platform, scale), config);
            return;
        }
        final String out = shot;
        final int n = frames;
        final String steps = script;
        new Lwjgl3Application(new BarkfartApp(platform, scale) {
            int k;

            @Override
            public void render() {
                if (k == 2 && steps != null) DesktopScript.run(this, steps);
                super.render();
                if (++k == n) {
                    int w = Gdx.graphics.getBackBufferWidth(), h = Gdx.graphics.getBackBufferHeight();
                    Pixmap pm = Pixmap.createFromFrameBuffer(0, 0, w, h);
                    // RGB: what is on screen (the framebuffer alpha is not part of the image).
                    Pixmap flipped = new Pixmap(w, h, Pixmap.Format.RGB888);
                    for (int y = 0; y < h; y++) flipped.drawPixmap(pm, 0, y, 0, h - 1 - y, w, 1);
                    PixmapIO.writePNG(Gdx.files.absolute(out), flipped);
                    pm.dispose();
                    flipped.dispose();
                    Gdx.app.exit();
                }
            }
        }, config);
    }
}
