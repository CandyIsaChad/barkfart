package app.barkfart.export;

import static org.junit.jupiter.api.Assertions.*;

import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.util.Random;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.stream.ImageInputStream;
import org.junit.jupiter.api.Test;

class EncodersTest {
    static byte[] picture(int w, int h, long seed, boolean holes) {
        byte[] px = new byte[w * h * 4];
        Random r = new Random(seed);
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                int i = (y * w + x) * 4;
                // gradients plus noise: exercises long LZW runs and code-table resets
                px[i] = (byte) (x * 255 / Math.max(1, w - 1));
                px[i + 1] = (byte) (y * 255 / Math.max(1, h - 1));
                px[i + 2] = (byte) r.nextInt(256);
                px[i + 3] = (byte) (holes && (x / 7 + y / 5) % 3 == 0 ? 0 : 255);
            }
        }
        return px;
    }

    @Test
    void pngDecodesToTheSamePixels() throws Exception {
        int w = 97, h = 61;
        byte[] px = picture(w, h, 1, true);
        BufferedImage im = ImageIO.read(new ByteArrayInputStream(Png.encode(px, w, h, true)));
        assertEquals(w, im.getWidth());
        for (int y = 0; y < h; y += 7) {
            for (int x = 0; x < w; x += 5) {
                int i = (y * w + x) * 4, c = im.getRGB(x, y);
                assertEquals(px[i + 3] & 0xff, c >>> 24);
                if ((px[i + 3] & 0xff) > 0) assertEquals(px[i + 2] & 0xff, c & 0xff);
            }
        }
        BufferedImage opaque = ImageIO.read(new ByteArrayInputStream(Png.encode(px, w, h, false)));
        assertEquals(0xff, opaque.getRGB(0, 0) >>> 24);
    }

    @Test
    void gifFramesDecodeCloseToTheSourceWithTransparencyAndTiming() throws Exception {
        int w = 160, h = 90, frames = 7;
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        Gif g = new Gif(bytes, w, h, 30, true);
        byte[][] src = new byte[frames][];
        for (int f = 0; f < frames; f++) {
            src[f] = picture(w, h, f, true);
            g.frame(src[f]);
        }
        g.finish();
        ImageReader rd = ImageIO.getImageReadersByFormatName("gif").next();
        ImageInputStream in = ImageIO.createImageInputStream(new ByteArrayInputStream(bytes.toByteArray()));
        rd.setInput(in);
        assertEquals(frames, rd.getNumImages(true));
        for (int f = 0; f < frames; f++) {
            BufferedImage im = rd.read(f);
            long err = 0;
            int n = 0;
            for (int y = 0; y < h; y++) {
                for (int x = 0; x < w; x++) {
                    int i = (y * w + x) * 4, c = im.getRGB(x, y);
                    boolean clear = (src[f][i + 3] & 0xff) < 128;
                    assertEquals(clear, (c >>> 24) == 0, "transparency at " + x + "," + y);
                    if (clear) continue;
                    err += Math.abs(((c >> 16) & 0xff) - (src[f][i] & 0xff)) + Math.abs(((c >> 8) & 0xff) - (src[f][i + 1] & 0xff));
                    n += 2;
                }
            }
            assertTrue(err / (double) n < 40, "colour error " + err / (double) n);
        }
        Gif timing = new Gif(new ByteArrayOutputStream(), 1, 1, 30, false);
        int total = 0;
        for (int i = 0; i < 30; i++) total += timing.nextDelay();
        assertEquals(100, total, "30 frames at 30 fps last exactly one second");
        Gif fast = new Gif(new ByteArrayOutputStream(), 1, 1, 120, false);
        assertEquals(2, fast.nextDelay(), "never under 2 hundredths");
    }
}
