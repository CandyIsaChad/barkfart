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
     *   --screenshot out.png N  render N frames (default 45), save, exit
     *   --script steps          (with --screenshot) comma list of actions run before capture
     */
    public static void main(String[] args) {
        boolean phone = false;
        String shot = null;
        int frames = 45;
        String script = null;
        for (int i = 0; i < args.length; i++) {
            if ("--phone".equals(args[i])) phone = true;
            else if ("--screenshot".equals(args[i])) {
                shot = args[++i];
                if (i + 1 < args.length && args[i + 1].matches("\\d+")) frames = Integer.parseInt(args[++i]);
            } else if ("--script".equals(args[i])) script = args[++i];
        }
        Lwjgl3ApplicationConfiguration config = new Lwjgl3ApplicationConfiguration();
        config.setTitle("Barkfart");
        config.setWindowedMode(phone ? 1688 : 1280, phone ? 780 : 720);
        config.useVsync(true);
        config.setBackBufferConfig(8, 8, 8, 8, 16, 0, 4);
        float scale = phone ? 2f : 1.5f;
        if (shot == null) {
            new Lwjgl3Application(new BarkfartApp(scale), config);
            return;
        }
        final String out = shot;
        final int n = frames;
        final String steps = script;
        new Lwjgl3Application(new BarkfartApp(scale) {
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
