package app.barkfart.desktop;

import app.barkfart.BarkfartApp;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3Application;
import com.badlogic.gdx.backends.lwjgl3.Lwjgl3ApplicationConfiguration;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.PixmapIO;
import com.badlogic.gdx.utils.ScreenUtils;

public class DesktopLauncher {
    /**
     * Args: [--screenshot out.png [frames]] renders that many frames (default 45),
     * saves the last one and exits. Used to check visuals from the command line.
     */
    public static void main(String[] args) {
        Lwjgl3ApplicationConfiguration config = new Lwjgl3ApplicationConfiguration();
        config.setTitle("Barkfart");
        config.setWindowedMode(1280, 720);
        config.useVsync(true);
        if (args.length >= 2 && "--screenshot".equals(args[0])) {
            final String out = args[1];
            final int frames = args.length >= 3 ? Integer.parseInt(args[2]) : 45;
            new Lwjgl3Application(new BarkfartApp() {
                int n;

                @Override
                public void render() {
                    super.render();
                    if (++n == frames) {
                        int w = Gdx.graphics.getBackBufferWidth(), h = Gdx.graphics.getBackBufferHeight();
                        Pixmap pm = Pixmap.createFromFrameBuffer(0, 0, w, h);
                        Pixmap flipped = new Pixmap(w, h, Pixmap.Format.RGBA8888);
                        for (int y = 0; y < h; y++) flipped.drawPixmap(pm, 0, y, 0, h - 1 - y, w, 1);
                        PixmapIO.writePNG(Gdx.files.absolute(out), flipped);
                        pm.dispose();
                        flipped.dispose();
                        Gdx.app.exit();
                    }
                }
            }, config);
            return;
        }
        new Lwjgl3Application(new BarkfartApp(), config);
    }
}
