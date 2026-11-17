package app.barkfart.io;

import app.barkfart.model.Project;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import java.util.zip.ZipOutputStream;

/**
 * The .bark container: a ZIP holding project.json (and later assets/, thumb.png).
 * Entry times are fixed so identical projects give identical bytes.
 */
public final class BarkFile {
    public static final String PROJECT_ENTRY = "project.json";
    /** Refuse absurdly large project.json (corrupt or hostile files). */
    static final int MAX_JSON_BYTES = 256 * 1024 * 1024;
    private static final long FIXED_TIME = 1700000000000L;

    private BarkFile() {}

    public static void write(Project p, OutputStream out) throws IOException {
        ZipOutputStream zip = new ZipOutputStream(out);
        ZipEntry e = new ZipEntry(PROJECT_ENTRY);
        e.setTime(FIXED_TIME);
        zip.putNextEntry(e);
        zip.write(ProjectCodec.encode(p).getBytes("UTF-8"));
        zip.closeEntry();
        // Assets: STORED (images/audio are already compressed), in id order.
        for (app.barkfart.model.Asset a : p.assets.values()) {
            if (!p.assetData.has(a.id)) continue;   // declared but no data: skipped, reported on load
            byte[] b = p.assetData.bytes(a.id);
            ZipEntry ae = new ZipEntry(a.path);
            ae.setTime(FIXED_TIME);
            ae.setMethod(ZipEntry.STORED);
            ae.setSize(b.length);
            ae.setCompressedSize(b.length);
            java.util.zip.CRC32 crc = new java.util.zip.CRC32();
            crc.update(b);
            ae.setCrc(crc.getValue());
            zip.putNextEntry(ae);
            zip.write(b);
            zip.closeEntry();
        }
        zip.finish();
        zip.flush();
    }

    /** Reads a project; asset entries are loaded into memory (streams cannot be revisited). */
    public static Project read(InputStream in) throws IOException, ProjectCodec.FormatException {
        ZipInputStream zip = new ZipInputStream(in);
        ZipEntry e;
        Project p = null;
        java.util.Map<String, byte[]> entries = new java.util.HashMap<String, byte[]>();
        while ((e = zip.getNextEntry()) != null) {
            ByteArrayOutputStream buf = new ByteArrayOutputStream();
            byte[] b = new byte[8192];
            int n;
            while ((n = zip.read(b)) > 0) {
                buf.write(b, 0, n);
                if (buf.size() > MAX_JSON_BYTES) throw new IOException(e.getName() + " too large");
            }
            if (PROJECT_ENTRY.equals(e.getName())) p = ProjectCodec.decode(buf.toString("UTF-8"));
            else entries.put(e.getName(), buf.toByteArray());
        }
        if (p == null) throw new ProjectCodec.FormatException("no " + PROJECT_ENTRY + " in file", null);
        for (app.barkfart.model.Asset a : p.assets.values()) {
            byte[] b = entries.get(a.path);
            if (b != null) p.assetData.put(a.id, b);
        }
        return p;
    }

    /**
     * Atomic save: write name.tmp, sync, keep the previous file as name.bak,
     * rename tmp into place. A crash leaves either the old or the new file.
     */
    public static void save(Project p, File file) throws IOException {
        File tmp = new File(file.getPath() + ".tmp");
        File bak = new File(file.getPath() + ".bak");
        // write() reads lazy assets from the current file before it is replaced below.
        FileOutputStream fos = new FileOutputStream(tmp);
        try {
            write(p, fos);
            fos.flush();
            fos.getFD().sync();
        } finally {
            fos.close();
        }
        if (file.exists()) {
            if (bak.exists() && !bak.delete()) throw new IOException("cannot replace backup " + bak);
            if (!file.renameTo(bak)) throw new IOException("cannot back up " + file);
        }
        if (!tmp.renameTo(file)) {
            if (bak.exists()) bak.renameTo(file);
            throw new IOException("cannot move " + tmp + " to " + file);
        }
        p.assetData.repoint(file);   // the new file holds every asset now
    }

    /** Loads a project; assets stay in the file and are read when first needed. */
    public static Project load(File file) throws IOException, ProjectCodec.FormatException {
        java.util.zip.ZipFile z;
        try {
            z = new java.util.zip.ZipFile(file);
        } catch (java.util.zip.ZipException ex) {
            throw new ProjectCodec.FormatException("not a Barkfart project: " + ex.getMessage(), ex);
        }
        try {
            ZipEntry pe = z.getEntry(PROJECT_ENTRY);
            if (pe == null) throw new ProjectCodec.FormatException("no " + PROJECT_ENTRY + " in file", null);
            if (pe.getSize() > MAX_JSON_BYTES) throw new IOException("project.json too large");
            InputStream in = z.getInputStream(pe);
            ByteArrayOutputStream buf = new ByteArrayOutputStream();
            byte[] b = new byte[8192];
            int n;
            while ((n = in.read(b)) > 0) {
                buf.write(b, 0, n);
                if (buf.size() > MAX_JSON_BYTES) throw new IOException("project.json too large");
            }
            in.close();
            Project p = ProjectCodec.decode(buf.toString("UTF-8"));
            for (app.barkfart.model.Asset a : p.assets.values()) {
                if (z.getEntry(a.path) != null) p.assetData.putLazy(a.id, file, a.path);
            }
            return p;
        } finally {
            z.close();
        }
    }
}
