package app.barkfart.render;

import app.barkfart.geom.GlyphSource;
import com.badlogic.gdx.Gdx;
import com.badlogic.gdx.graphics.Texture;
import com.badlogic.gdx.graphics.g2d.BitmapFont;
import com.badlogic.gdx.graphics.g2d.freetype.FreeTypeFontGenerator;
import com.badlogic.gdx.utils.Disposable;

/**
 * Text glyphs for the renderer: the bundled Inter font rasterized once at
 * BASE pixels (Latin, Latin-1 and common punctuation). Text quads sample its
 * pages, named "glyphs:<page>". GL thread.
 */
public final class GlyphAtlas implements GlyphSource, Disposable {
    public static final int BASE = 64;
    private static final String EXTRA = "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ"
            + "ŒœŠšŸŽžƒ€‘’“”•…–—™¡¿«»°±²³µ¶·¹º¼½¾§©®";
    private final BitmapFont font;
    private final java.util.Map<Character, Glyph> cache = new java.util.HashMap<Character, Glyph>();

    public GlyphAtlas() {
        FreeTypeFontGenerator gen = new FreeTypeFontGenerator(Gdx.files.internal("fonts/Inter-Regular.ttf"));
        try {
            FreeTypeFontGenerator.FreeTypeFontParameter p = new FreeTypeFontGenerator.FreeTypeFontParameter();
            p.size = BASE;
            p.characters = FreeTypeFontGenerator.DEFAULT_CHARS + EXTRA;
            p.minFilter = Texture.TextureFilter.Linear;
            p.magFilter = Texture.TextureFilter.Linear;
            p.genMipMaps = false;
            font = gen.generateFont(p);
        } finally {
            gen.dispose();
        }
    }

    public float basePx() {
        return BASE;
    }

    public float lineHeight() {
        return font.getLineHeight();
    }

    public Glyph glyph(char c) {
        Glyph g = cache.get(c);
        if (g != null || cache.containsKey(c)) return g;
        BitmapFont.Glyph b = font.getData().getGlyph(c);
        if (b == null) b = font.getData().missingGlyph;
        if (b != null) {
            // BitmapFontCache convention (unflipped): quad bottom at baseline + yoffset; v at the bottom, v2 at the top
            g = new Glyph("glyphs:" + b.page, b.u, b.v, b.u2, b.v2, b.xoffset, b.yoffset, b.width, b.height, b.xadvance);
        }
        cache.put(c, g);
        return g;
    }

    /** The texture for a "glyphs:<page>" id, or null. */
    public Texture texture(String id) {
        if (id == null || !id.startsWith("glyphs:")) return null;
        try {
            int page = Integer.parseInt(id.substring(7));
            return page < font.getRegions().size ? font.getRegions().get(page).getTexture() : null;
        } catch (NumberFormatException e) {
            return null;
        }
    }

    /** Textures for a draw: glyph pages first, then the project's images. */
    public MeshRenderer.Textures with(final MeshRenderer.Textures images) {
        return new MeshRenderer.Textures() {
            public Texture get(String id) {
                Texture t = texture(id);
                return t != null ? t : (images == null ? null : images.get(id));
            }
        };
    }

    @Override
    public void dispose() {
        font.dispose();
    }
}
