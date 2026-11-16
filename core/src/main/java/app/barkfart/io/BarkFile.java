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
        zip.finish();
        zip.flush();
    }

    public static Project read(InputStream in) throws IOException, ProjectCodec.FormatException {
        ZipInputStream zip = new ZipInputStream(in);
        ZipEntry e;
        while ((e = zip.getNextEntry()) != null) {
            if (!PROJECT_ENTRY.equals(e.getName())) continue;
            ByteArrayOutputStream buf = new ByteArrayOutputStream();
            byte[] b = new byte[8192];
            int n;
            while ((n = zip.read(b)) > 0) {
                buf.write(b, 0, n);
                if (buf.size() > MAX_JSON_BYTES) throw new IOException("project.json too large");
            }
            return ProjectCodec.decode(buf.toString("UTF-8"));
        }
        throw new ProjectCodec.FormatException("no " + PROJECT_ENTRY + " in file", null);
    }

    /**
     * Atomic save: write name.tmp, sync, keep the previous file as name.bak,
     * rename tmp into place. A crash leaves either the old or the new file.
     */
    public static void save(Project p, File file) throws IOException {
        File tmp = new File(file.getPath() + ".tmp");
        File bak = new File(file.getPath() + ".bak");
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
    }

    public static Project load(File file) throws IOException, ProjectCodec.FormatException {
        FileInputStream in = new FileInputStream(file);
        try {
            return read(in);
        } finally {
            in.close();
        }
    }
}
