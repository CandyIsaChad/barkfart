package app.barkfart.image;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.export.Gif;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.util.Random;
import javax.imageio.ImageIO;
import javax.imageio.ImageReader;
import javax.imageio.stream.ImageInputStream;
import org.junit.jupiter.api.Test;

class GifDecoderTest {
    static byte[] picture(int w, int h, long seed) {
        byte[] px = new byte[w * h * 4];
        Random r = new Random(seed);
        for (int i = 0; i < w * h; i++) {
            px[i * 4] = (byte) r.nextInt(256);
            px[i * 4 + 1] = (byte) ((i / w) * 255 / h);
            px[i * 4 + 2] = (byte) ((i % w) * 255 / w);
            px[i * 4 + 3] = (byte) (r.nextInt(10) == 0 ? 0 : 255);
        }
        return px;
    }

    /** Our decoder's frames against Java's reader, pixel for pixel. */
    static void sameAsImageIO(byte[] gif, GifDecoder.Result res) throws Exception {
        ImageReader rd = ImageIO.getImageReadersByFormatName("gif").next();
        ImageInputStream in = ImageIO.createImageInputStream(new ByteArrayInputStream(gif));
        rd.setInput(in);
        assertEquals(rd.getNumImages(true), res.frames.size());
        for (int f = 0; f < res.frames.size(); f++) {
            BufferedImage im = rd.read(f);
            byte[] px = res.frames.get(f).rgba;
            for (int y = 0; y < Math.min(res.h, im.getHeight()); y++) {
                for (int x = 0; x < Math.min(res.w, im.getWidth()); x++) {
                    int c = im.getRGB(x, y), o = (y * res.w + x) * 4;
                    if ((c >>> 24) == 0) {
                        assertEquals(0, px[o + 3] & 0xff, "transparent at " + x + "," + y);
                        continue;
                    }
                    assertEquals(c & 0xffffff, ((px[o] & 0xff) << 16) | ((px[o + 1] & 0xff) << 8) | (px[o + 2] & 0xff), "frame " + f + " at " + x + "," + y);
                }
            }
        }
    }

    @Test
    void decodesOurAnimatedGifsExactlyWithTiming() throws Exception {
        int w = 150, h = 70;
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        Gif g = new Gif(bytes, w, h, 30, true);
        for (int f = 0; f < 4; f++) g.frame(picture(w, h, f));
        g.finish();
        GifDecoder.Result res = GifDecoder.decode(bytes.toByteArray());
        assertNotNull(res);
        assertEquals(w, res.w);
        assertEquals(4, res.frames.size());
        assertEquals(0.03, res.frames.get(0).delay, 1e-9);
        assertEquals(0.04, res.frames.get(2).delay, 1e-9);
        sameAsImageIO(bytes.toByteArray(), res);
    }

    @Test
    void decodesJavasInterlacedGifs() throws Exception {
        BufferedImage im = new BufferedImage(61, 37, BufferedImage.TYPE_INT_RGB);
        Random r = new Random(5);
        for (int y = 0; y < 37; y++) for (int x = 0; x < 61; x++) im.setRGB(x, y, r.nextInt(6) * 0x330000 + (y * 7 % 256) * 0x100 + x * 4);
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        ImageIO.write(im, "gif", o);
        GifDecoder.Result res = GifDecoder.decode(o.toByteArray());
        assertNotNull(res);
        assertEquals(1, res.frames.size());
        sameAsImageIO(o.toByteArray(), res);
    }

    @Test
    void badInputGivesNullOrPartialNeverThrows() {
        assertNull(GifDecoder.decode(null));
        assertNull(GifDecoder.decode("GIF89a".getBytes()));
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        try {
            Gif g = new Gif(bytes, 40, 30, 10, true);
            g.frame(picture(40, 30, 1));
            g.finish();
        } catch (java.io.IOException e) {
            throw new AssertionError(e);
        }
        byte[] ok = bytes.toByteArray();
        Random r = new Random(9);
        for (int i = 0; i < 2000; i++) {
            byte[] b = java.util.Arrays.copyOf(ok, r.nextInt(ok.length + 1));
            if (b.length > 0) b[r.nextInt(b.length)] = (byte) r.nextInt();
            GifDecoder.decode(b);
        }
        assertEquals(1, GifDecoder.deinterlace(4, 8), "row order of interlace pass 2");
    }

    @Test
    void importingAnAnimatedGifMakesAMovieclipShowingOneFrameAtATime() throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        Gif g = new Gif(bytes, 30, 20, 10, true);
        for (int f = 0; f < 3; f++) g.frame(picture(30, 20, f));
        g.finish();
        java.io.File file = java.io.File.createTempFile("anim", ".gif");
        java.nio.file.Files.write(file.toPath(), bytes.toByteArray());
        app.barkfart.model.Project p = app.barkfart.Fixtures.empty();
        app.barkfart.edit.Document d = new app.barkfart.edit.Document(p);
        app.barkfart.files.Importer.Result r = app.barkfart.files.Importer.importFile(d, file, app.barkfart.audio.AudioBackend.NONE, 0);
        assertTrue(r.layer instanceof app.barkfart.model.Layer.SymbolLayer);
        for (double t : new double[] {0.05, 0.15, 0.25, 0.35}) {
            int quads = 0;
            for (Object o : flatten(app.barkfart.eval.Evaluator.evaluate(p, p.rootSymbol, t).root)) if (o instanceof app.barkfart.eval.Shape.Quad) quads++;
            assertEquals(1, quads, "one frame at " + t);
        }
        d.undo();
        assertTrue(p.root().layers.isEmpty());
        file.delete();
    }

    static java.util.List<Object> flatten(app.barkfart.eval.Group g) {
        java.util.List<Object> out = new java.util.ArrayList<Object>();
        for (Object o : g.children) {
            if (o instanceof app.barkfart.eval.Group) out.addAll(flatten((app.barkfart.eval.Group) o));
            else out.add(o);
        }
        return out;
    }
}
