package app.barkfart.geom;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class TextLayoutTest {
    /** Every glyph a 10x10 box with advance 10, line height 20, at base 20 px. */
    static final GlyphSource BOXES = new GlyphSource() {
        public float basePx() {
            return 20;
        }

        public float lineHeight() {
            return 20;
        }

        public Glyph glyph(char c) {
            return c == ' ' ? new Glyph("glyphs:0", 0, 0, 0, 0, 0, 0, 0, 0, 10) : new Glyph("glyphs:0", 0, 0, 1, 1, 0, 0, 10, 10, 10);
        }
    };

    @Test
    void linesAreCentredScaledAndTransformed() {
        Project p = Fixtures.empty();
        Layer.TextLayer t = new Layer.TextLayer(p.newId("L"), "t");
        t.text = "ab\nabcd";
        t.size = 40;   // twice the base: boxes become 20 x 20
        t.color.setValue(Color.rgba8888(0x00ff00ff));
        t.transform.x.setValue(100f);
        p.root().layers.add(t);
        p.invalidateIndex();
        DrawList dl = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0), BOXES);
        DrawList.Batch b = dl.batches.get(0);
        assertEquals("glyphs:0", b.texture);
        assertEquals(6 * 6, b.vertexCount(), "six glyph quads");
        float minX = 1e9f, maxX = -1e9f;
        for (int i = 0; i < b.vertexCount(); i++) {
            minX = Math.min(minX, b.x(i));
            maxX = Math.max(maxX, b.x(i));
            assertEquals(0x00ff00ff, b.rgba(i));
        }
        assertEquals(100 - 40, minX, 1e-3, "the longer line (4 glyphs x 20) is centred on x = 100");
        assertEquals(100 + 40, maxX, 1e-3);
        // without glyphs (tests, the reference renderer) text is simply not drawn
        assertEquals(0, Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0)).triangleCount());
    }
}
