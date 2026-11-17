package app.barkfart.model;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.util.HashMap;
import java.util.Map;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * The bytes behind Project.assets (images, audio, fonts). Not serialized as
 * JSON: stored as entries of the .bark ZIP. Assets loaded from a file stay on
 * disk until needed (lazy), so big images do not sit in memory.
 */
public final class AssetData {
    private static final class Source {
        byte[] bytes;
        File zip;
        String entry;
    }

    private final Map<String, Source> sources = new HashMap<String, Source>();

    /** Adds or replaces an asset's bytes. */
    public void put(String assetId, byte[] bytes) {
        Source s = new Source();
        s.bytes = bytes;
        sources.put(assetId, s);
    }

    /** Points an asset at an entry of a .bark file (read when first needed). */
    public void putLazy(String assetId, File zip, String entry) {
        Source s = new Source();
        s.zip = zip;
        s.entry = entry;
        sources.put(assetId, s);
    }

    public boolean has(String assetId) {
        return sources.containsKey(assetId);
    }

    public void remove(String assetId) {
        sources.remove(assetId);
    }

    /** The asset's bytes, reading from its source file if needed. */
    public byte[] bytes(String assetId) throws IOException {
        Source s = sources.get(assetId);
        if (s == null) throw new IOException("asset " + assetId + " has no data");
        if (s.bytes != null) return s.bytes;
        ZipFile z = new ZipFile(s.zip);
        try {
            ZipEntry e = z.getEntry(s.entry);
            if (e == null) throw new IOException("asset entry " + s.entry + " missing from " + s.zip.getName());
            InputStream in = z.getInputStream(e);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[65536];
            int n;
            while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
            in.close();
            return out.toByteArray();
        } finally {
            z.close();
        }
    }

    /** Lazy sources pointing at a file are re-pointed after it was replaced by a save. */
    public void repoint(File zip) {
        for (Source s : sources.values()) if (s.zip != null) s.zip = zip;
    }
}
