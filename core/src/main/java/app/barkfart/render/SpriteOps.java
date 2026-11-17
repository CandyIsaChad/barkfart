package app.barkfart.render;

import app.barkfart.edit.Document;
import app.barkfart.edit.StructureEdits;
import app.barkfart.export.Png;
import app.barkfart.image.ImageOps;
import app.barkfart.model.Asset;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import com.badlogic.gdx.graphics.Pixmap;

/** Sprite image edits that need pixels (decoded with LibGDX on the device). */
public final class SpriteOps {
    private SpriteOps() {}

    /** Straight RGBA8 pixels of an image file, rows top-down, plus its size; null if it cannot be decoded. */
    public static Object[] decode(byte[] bytes) {
        Pixmap pm = null, conv = null;
        try {
            pm = new Pixmap(bytes, 0, bytes.length);
            Pixmap src = pm;
            if (pm.getFormat() != Pixmap.Format.RGBA8888) {
                conv = new Pixmap(pm.getWidth(), pm.getHeight(), Pixmap.Format.RGBA8888);
                conv.setBlending(Pixmap.Blending.None);
                conv.drawPixmap(pm, 0, 0);
                src = conv;
            }
            int w = src.getWidth(), h = src.getHeight();
            byte[] px = new byte[w * h * 4];
            java.nio.ByteBuffer b = src.getPixels();
            b.position(0);
            b.get(px);
            b.position(0);
            return new Object[] {px, w, h};
        } catch (RuntimeException e) {
            return null;
        } finally {
            if (pm != null) pm.dispose();
            if (conv != null) conv.dispose();
        }
    }

    /**
     * Remove background colour: a new PNG asset with the colour made
     * transparent (see ImageOps.removeColor), and the sprite pointed at it.
     * One undo step; the original image stays in the project. Returns a
     * problem, or null.
     */
    public static String removeColor(Document doc, Layer.SpriteLayer l, int rgb, int tolerance, int soft, boolean edgesOnly) {
        Project p = doc.project;
        try {
            byte[] bytes = l.asset == null ? null : p.assetData.bytes(l.asset);
            Object[] img = bytes == null ? null : decode(bytes);
            if (img == null) return "could not read the image";
            int w = (Integer) img[1], h = (Integer) img[2];
            byte[] out = ImageOps.removeColor((byte[]) img[0], w, h, rgb, tolerance, soft, edgesOnly);
            String aid = p.newId("A");
            Asset a = new Asset(aid, Asset.Kind.IMAGE, "assets/" + aid + ".png");
            doc.begin("Remove background colour");
            try {
                doc.apply(StructureEdits.addAsset(p, a, Png.encode(out, w, h, true)));
                doc.apply(StructureEdits.setSpriteAsset(l, aid));
            } finally {
                doc.commit();
            }
            return null;
        } catch (java.io.IOException e) {
            return e.getMessage();
        }
    }
}
