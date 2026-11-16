package app.barkfart.files;

import app.barkfart.io.BarkFile;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Project;
import java.io.File;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;

/**
 * The user's Barkfart folder (iOS: the app's Documents, visible in the Files
 * app). Projects are *.bark; *.nodes, *.nodemc, *.stknds and *.stk files in
 * the same folder are offered for import. Plain java.io (MobiVM-safe).
 */
public final class ProjectStore {
    public static final String EXT = ".bark";

    public enum Kind { PROJECT, FIGURE, MOVIECLIP, LEGACY_PROJECT, PIVOT, OTHER }

    public static final class Entry {
        public final File file;
        public final Kind kind;
        public final String name;
        public final long modified;

        Entry(File file, Kind kind) {
            this.file = file;
            this.kind = kind;
            String n = file.getName();
            int dot = n.lastIndexOf('.');
            this.name = dot > 0 ? n.substring(0, dot) : n;
            this.modified = file.lastModified();
        }
    }

    public final File root;

    public ProjectStore(File root) {
        this.root = root;
        if (!root.exists() && !root.mkdirs()) throw new IllegalStateException("cannot create " + root);
    }

    public static Kind kindOf(String fileName) {
        String n = fileName.toLowerCase();
        if (n.endsWith(EXT)) return Kind.PROJECT;
        if (n.endsWith(".nodes")) return Kind.FIGURE;
        if (n.endsWith(".nodemc")) return Kind.MOVIECLIP;
        if (n.endsWith(".stknds")) return Kind.LEGACY_PROJECT;
        if (n.endsWith(".stk")) return Kind.PIVOT;
        return Kind.OTHER;
    }

    private List<Entry> list(boolean projects) {
        List<Entry> out = new ArrayList<Entry>();
        File[] fs = root.listFiles();
        if (fs == null) return out;
        for (File f : fs) {
            if (!f.isFile() || f.getName().startsWith(".")) continue;
            Kind k = kindOf(f.getName());
            if (k == Kind.OTHER) continue;
            if ((k == Kind.PROJECT) == projects) out.add(new Entry(f, k));
        }
        Entry[] arr = out.toArray(new Entry[0]);
        Arrays.sort(arr, new Comparator<Entry>() {
            public int compare(Entry a, Entry b) {
                return a.modified == b.modified ? a.name.compareToIgnoreCase(b.name) : (a.modified > b.modified ? -1 : 1);
            }
        });
        return new ArrayList<Entry>(Arrays.asList(arr));
    }

    /** Barkfart projects, newest first. */
    public List<Entry> projects() {
        return list(true);
    }

    /** Importable legacy files, newest first. */
    public List<Entry> importables() {
        return list(false);
    }

    /** A file name that does not exist yet: "name.ext", "name 2.ext", ... */
    public File unique(String name, String ext) {
        String base = sanitize(name);
        File f = new File(root, base + ext);
        for (int i = 2; f.exists(); i++) f = new File(root, base + " " + i + ext);
        return f;
    }

    /** Strips characters that are not allowed in file names; never empty. */
    public static String sanitize(String name) {
        String s = name == null ? "" : name.replaceAll("[\\\\/:*?\"<>|\\x00-\\x1f]", "").trim();
        s = s.replaceAll("^\\.+", "").trim();   // no hidden files, no "." or ".."

        if (s.length() > 80) s = s.substring(0, 80).trim();
        return s.isEmpty() ? "Untitled" : s;
    }

    public void save(Project p, File f) throws IOException {
        BarkFile.save(p, f);
    }

    public Project open(File f) throws IOException, ProjectCodec.FormatException {
        return BarkFile.load(f);
    }

    /** Renames a project; returns the new file (unique if the name is taken). */
    public File rename(File f, String newName) throws IOException {
        File to = new File(root, sanitize(newName) + EXT);
        if (to.equals(f)) return f;
        if (to.exists()) to = unique(newName, EXT);
        if (!f.renameTo(to)) throw new IOException("cannot rename " + f.getName());
        File bak = new File(f.getPath() + ".bak");
        if (bak.exists()) bak.renameTo(new File(to.getPath() + ".bak"));
        return to;
    }

    public File duplicate(File f) throws IOException, ProjectCodec.FormatException {
        Entry e = new Entry(f, Kind.PROJECT);
        File to = unique(e.name + " copy", EXT);
        BarkFile.save(BarkFile.load(f), to);
        return to;
    }

    /**
     * Deletes softly: moves the file into ".trash" (hidden from lists) so a
     * mistaken delete can be undone with restore().
     */
    public File delete(File f) throws IOException {
        File trash = new File(root, ".trash");
        if (!trash.exists() && !trash.mkdirs()) throw new IOException("cannot create trash");
        File to = new File(trash, System.currentTimeMillis() + "-" + f.getName());
        if (!f.renameTo(to)) throw new IOException("cannot delete " + f.getName());
        return to;
    }

    public File restore(File trashed, String originalName) throws IOException {
        File to = new File(root, originalName);
        if (to.exists()) {
            String n = originalName;
            int dot = n.lastIndexOf('.');
            to = unique(dot > 0 ? n.substring(0, dot) : n, dot > 0 ? n.substring(dot) : "");
        }
        if (!trashed.renameTo(to)) throw new IOException("cannot restore " + originalName);
        return to;
    }

    /** Copies bytes into the folder (picked files); returns the new file. */
    public File addCopy(String fileName, byte[] bytes) throws IOException {
        String n = fileName;
        int dot = n.lastIndexOf('.');
        File to = unique(dot > 0 ? n.substring(0, dot) : n, dot > 0 ? n.substring(dot).toLowerCase() : "");
        java.io.FileOutputStream out = new java.io.FileOutputStream(to);
        try {
            out.write(bytes);
        } finally {
            out.close();
        }
        return to;
    }
}
