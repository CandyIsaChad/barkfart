package app.barkfart.ui;

import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Color;
import com.badlogic.gdx.graphics.Pixmap;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.NinePatch;
import com.badlogic.gdx.graphics.g2d.freetype.FreeTypeFontGenerator;
import com.badlogic.gdx.scenes.scene2d.utils.NinePatchDrawable;
import com.badlogic.gdx.utils.Disposable;

/**
 * The single source of visual constants (UI-SPEC.md "Visual language").
 * Sizes are points; fonts are rasterized at the screen's pixel density so
 * text stays crisp.
 */
public final class Theme implements Disposable {
    // Colours
    public static final Color PANEL = rgb(0x1e2024), CONTROL = rgb(0x2b2e33), CONTROL_DOWN = rgb(0x363a40),
            SEGMENT_ON = rgb(0x40454c), SEGMENT_EDGE = rgb(0x5a6068), SURROUND = rgb(0x17191c),
            TEXT = rgb(0xe6e6e6), TEXT_DIM = rgb(0x9aa0a6), ACCENT = rgb(0x0a84ff), DANGER = rgb(0xff453a),
            STAGE = rgb(0xffffff), ROOT_HANDLE = rgb(0xff9f0a), CELL = rgb(0x26292e), CELL_TWEEN = rgb(0x1f2226);

    // Metrics (points), phone; see UI-SPEC.md for the iPad set.
    public static final float GRID = 4, PAD = 12, GAP = 8, CONTROL_H = 40, TOUCH = 44, RADIUS = 4;
    public static final float SIDEBAR_W = 168, STRIP_H = 72, CELL_W = 96, CELL_H = 54, CELL_GAP = 4, PLAY = 56;
    public static final float TEXT_S = 11, TEXT_M = 13, TEXT_L = 15;
    public static final float DRAG_SLOP = 8, HANDLE_HIT = 22, JOINT_R = 3.5f, ROOT_HALF = 4.5f;

    /** Pixels per point for rasterizing fonts and shapes. */
    public final float px;
    public final BitmapFont small, body, title;
    private final Texture roundTex;
    private final Texture whiteTex;

    /** @param pixelsPerPoint backbuffer pixels per UI point (3 on recent iPhones). */
    public Theme(float pixelsPerPoint) {
        px = Math.max(1f, pixelsPerPoint);
        FreeTypeFontGenerator reg = new FreeTypeFontGenerator(Gdx.files.internal("fonts/Inter-Regular.ttf"));
        FreeTypeFontGenerator semi = new FreeTypeFontGenerator(Gdx.files.internal("fonts/Inter-SemiBold.ttf"));
        small = font(reg, TEXT_S);
        body = font(reg, TEXT_M);
        title = font(semi, TEXT_M);
        reg.dispose();
        semi.dispose();
        roundTex = roundedRect((int) Math.ceil(RADIUS * px));
        Pixmap w = new Pixmap(1, 1, Pixmap.Format.RGBA8888);
        w.setColor(Color.WHITE);
        w.fill();
        whiteTex = new Texture(w);
        w.dispose();
    }

    private BitmapFont font(FreeTypeFontGenerator gen, float pt) {
        FreeTypeFontGenerator.FreeTypeFontParameter p = new FreeTypeFontGenerator.FreeTypeFontParameter();
        p.size = Math.round(pt * px);
        p.minFilter = Texture.TextureFilter.Linear;
        p.magFilter = Texture.TextureFilter.Linear;
        p.hinting = FreeTypeFontGenerator.Hinting.AutoMedium;
        p.characters = FreeTypeFontGenerator.DEFAULT_CHARS + "\u00b7\u2026\u2013\u2014\u00b0\u2212";
        BitmapFont f = gen.generateFont(p);
        f.getData().setScale(1f / px);   // draw in points, rasterized in pixels
        f.setUseIntegerPositions(false);
        tabularDigits(f);
        return f;
    }

    /** Gives every digit the widest digit's advance so numbers do not jitter. */
    private static void tabularDigits(BitmapFont f) {
        int max = 0;
        for (char c = '0'; c <= '9'; c++) {
            BitmapFont.Glyph g = f.getData().getGlyph(c);
            if (g != null) max = Math.max(max, g.xadvance);
        }
        for (char c = '0'; c <= '9'; c++) {
            BitmapFont.Glyph g = f.getData().getGlyph(c);
            if (g == null) continue;
            int extra = max - g.xadvance;
            g.xoffset += extra / 2;
            g.xadvance = max;
        }
    }

    /** White rounded-rect texture for tinted nine-patches, corner radius r px. */
    private static Texture roundedRect(int r) {
        int size = r * 2 + 2;
        Pixmap pm = new Pixmap(size, size, Pixmap.Format.RGBA8888);
        pm.setBlending(Pixmap.Blending.None);
        for (int y = 0; y < size; y++) {
            for (int x = 0; x < size; x++) {
                float cx = x < r ? r : (x > size - r - 1 ? size - r - 1 : x);
                float cy = y < r ? r : (y > size - r - 1 ? size - r - 1 : y);
                float d = (float) Math.hypot(x + 0.5f - (cx + 0.5f), y + 0.5f - (cy + 0.5f));
                float a = Math.max(0, Math.min(1, r - d + 0.5f));
                pm.drawPixel(x, y, Color.rgba8888(1, 1, 1, a));
            }
        }
        Texture t = new Texture(pm);
        t.setFilter(Texture.TextureFilter.Linear, Texture.TextureFilter.Linear);
        pm.dispose();
        return t;
    }

    /** A rounded rectangle drawable in the given colour, sized in points. */
    public NinePatchDrawable box(Color c) {
        int r = (roundTex.getWidth() - 2) / 2;
        NinePatch np = new NinePatch(roundTex, r, r, r, r);
        np.setColor(c);
        np.scale(1f / px, 1f / px);
        return new NinePatchDrawable(np);
    }

    public Texture white() {
        return whiteTex;
    }

    private static Color rgb(int v) {
        return new Color(((v >> 16) & 0xff) / 255f, ((v >> 8) & 0xff) / 255f, (v & 0xff) / 255f, 1f);
    }

    @Override
    public void dispose() {
        small.dispose();
        body.dispose();
        title.dispose();
        roundTex.dispose();
        whiteTex.dispose();
    }
}
