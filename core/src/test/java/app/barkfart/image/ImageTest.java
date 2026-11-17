package app.barkfart.image;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.export.Png;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import javax.imageio.ImageIO;
import org.junit.jupiter.api.Test;

class ImageTest {
    @Test
    void sizesFromPngJpegGifHeaders() throws Exception {
        assertArrayEquals(new int[] {37, 21}, ImageInfo.size(Png.encode(new byte[37 * 21 * 4], 37, 21, true)));
        BufferedImage im = new BufferedImage(123, 45, BufferedImage.TYPE_INT_RGB);
        for (String fmt : new String[] {"jpg", "gif"}) {
            ByteArrayOutputStream o = new ByteArrayOutputStream();
            ImageIO.write(im, fmt, o);
            assertArrayEquals(new int[] {123, 45}, ImageInfo.size(o.toByteArray()), fmt);
        }
        assertNull(ImageInfo.size(new byte[30]));
        assertNull(ImageInfo.size(null));
    }

    /** A 20x10 green image with a white border 3 px wide and a white dot inside. */
    static byte[] picture() {
        byte[] px = new byte[20 * 10 * 4];
        for (int y = 0; y < 10; y++) {
            for (int x = 0; x < 20; x++) {
                boolean white = x < 3 || y < 3 || x >= 17 || y >= 7 || (x == 10 && y == 5);
                int i = (y * 20 + x) * 4;
                px[i] = (byte) (white ? 250 : 20);
                px[i + 1] = (byte) (white ? 252 : 200);
                px[i + 2] = (byte) (white ? 251 : 30);
                px[i + 3] = (byte) 255;
            }
        }
        return px;
    }

    static int alpha(byte[] px, int x, int y) {
        return px[(y * 20 + x) * 4 + 3] & 0xff;
    }

    @Test
    void removingABackgroundColour() {
        byte[] all = ImageOps.removeColor(picture(), 20, 10, 0xffffff, 10, 0, false);
        assertEquals(0, alpha(all, 0, 0), "background gone");
        assertEquals(0, alpha(all, 10, 5), "everywhere: the dot inside goes too");
        assertEquals(255, alpha(all, 5, 5), "green stays");
        byte[] edge = ImageOps.removeColor(picture(), 20, 10, 0xffffff, 10, 0, true);
        assertEquals(0, alpha(edge, 0, 0));
        assertEquals(255, alpha(edge, 10, 5), "edges only: the dot inside the subject stays");
        byte[] soft = ImageOps.removeColor(picture(), 20, 10, 0xffffff, 2, 10, false);
        int a = alpha(soft, 0, 0);
        assertTrue(a > 0 && a < 255, "a near match fades instead of vanishing: " + a);
    }

    @Test
    void importingAnImageMakesASpriteInOneUndoStep() throws Exception {
        Project p = Fixtures.empty();
        app.barkfart.edit.Document d = new app.barkfart.edit.Document(p);
        String before = app.barkfart.io.ProjectCodec.encode(p);
        java.io.File f = java.io.File.createTempFile("pic", ".png");
        java.nio.file.Files.write(f.toPath(), Png.encode(picture(), 20, 10, true));
        app.barkfart.files.Importer.Result r = app.barkfart.files.Importer.importFile(d, f, app.barkfart.audio.AudioBackend.NONE, 0);
        Layer.SpriteLayer s = (Layer.SpriteLayer) r.layer;
        assertEquals(20, s.width, 1e-6);
        assertEquals(10, s.height, 1e-6);
        assertTrue(p.assetData.has(s.asset));
        d.undo();
        assertEquals(before.replaceAll("\"idCounter\": \\d+", ""), app.barkfart.io.ProjectCodec.encode(p).replaceAll("\"idCounter\": \\d+", ""));
        f.delete();
    }
}
