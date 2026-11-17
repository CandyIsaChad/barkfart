package app.barkfart.image;

/** Image size from the file header (PNG, JPEG, GIF), without decoding pixels. Null when unknown. */
public final class ImageInfo {
    private ImageInfo() {}

    public static int[] size(byte[] b) {
        try {
            if (b == null || b.length < 24) return null;
            if ((b[0] & 0xff) == 0x89 && b[1] == 'P' && b[2] == 'N' && b[3] == 'G') return new int[] {be32(b, 16), be32(b, 20)};
            if (b[0] == 'G' && b[1] == 'I' && b[2] == 'F') return new int[] {(b[6] & 0xff) | ((b[7] & 0xff) << 8), (b[8] & 0xff) | ((b[9] & 0xff) << 8)};
            if ((b[0] & 0xff) == 0xFF && (b[1] & 0xff) == 0xD8) {
                int p = 2;
                while (p + 9 < b.length) {
                    if ((b[p] & 0xff) != 0xFF) return null;
                    int marker = b[p + 1] & 0xff, len = ((b[p + 2] & 0xff) << 8) | (b[p + 3] & 0xff);
                    boolean sof = marker >= 0xC0 && marker <= 0xCF && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
                    if (sof) return new int[] {((b[p + 7] & 0xff) << 8) | (b[p + 8] & 0xff), ((b[p + 5] & 0xff) << 8) | (b[p + 6] & 0xff)};
                    if (len < 2) return null;
                    p += 2 + len;
                }
            }
            return null;
        } catch (RuntimeException e) {
            return null;
        }
    }

    private static int be32(byte[] b, int o) {
        return ((b[o] & 0xff) << 24) | ((b[o + 1] & 0xff) << 16) | ((b[o + 2] & 0xff) << 8) | (b[o + 3] & 0xff);
    }
}
