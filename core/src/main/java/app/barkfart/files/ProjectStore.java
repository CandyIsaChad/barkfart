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

    public enum Kind { PROJECT, FIGURE, MOVIECLIP, LEGACY_PROJECT, PIVOT, SOUND, IMAGE, ZIP, OTHER }

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

    /** Sound files the import accepts (ogg is listed so it gets a clear "not supported" message). */
    public static final String[] SOUND_EXTENSIONS = {".wav", ".mp3", ".m4a", ".aac", ".caf", ".aif", ".aiff", ".ogg"};

    public static Kind kindOf(String fileName) {
        String n = fileName.toLowerCase();
        if (n.endsWith(EXT)) return Kind.PROJECT;
        if (n.endsWith(".nodes")) return Kind.FIGURE;
        if (n.endsWith(".nodemc")) return Kind.MOVIECLIP;
        if (n.endsWith(".stknds")) return Kind.LEGACY_PROJECT;
        if (n.endsWith(".stk")) return Kind.PIVOT;
        for (String e : SOUND_EXTENSIONS) if (n.endsWith(e)) return Kind.SOUND;
        if (n.endsWith(".png") || n.endsWith(".jpg") || n.endsWith(".jpeg")) return Kind.IMAGE;
        if (n.endsWith(".zip")) return Kind.ZIP;
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

    /** List orders (Stick Nodes: A-Z 1.6.0, natural numbers 3.1.0, new/old 2.3.0). */
    public enum Sort { NEWEST, OLDEST, NAME }

    /**
     * Projects or importable files, filtered by a case-insensitive part of the
     * name (null or empty: all) and sorted; names compare numbers by value
     * ("fig 2" before "fig 10").
     */
    public List<Entry> list(boolean projects, Sort sort, String filter) {
        List<Entry> all = list(projects);
        List<Entry> out = new ArrayList<Entry>();
        String f = filter == null ? "" : filter.trim().toLowerCase();
        for (Entry e : all) if (f.isEmpty() || e.name.toLowerCase().contains(f)) out.add(e);
        final Sort by = sort == null ? Sort.NEWEST : sort;
        java.util.Collections.sort(out, new Comparator<Entry>() {
            public int compare(Entry a, Entry b) {
                if (by == Sort.NAME || a.modified == b.modified) {
                    int c = natural(a.name, b.name);
                    return by == Sort.NAME || c != 0 ? c : 0;
                }
                return by == Sort.NEWEST ? (a.modified > b.modified ? -1 : 1) : (a.modified < b.modified ? -1 : 1);
            }
        });
        return out;
    }

    /** Natural order: case-insensitive, runs of digits compared by value. */
    public static int natural(String a, String b) {
        int i = 0, j = 0;
        while (i < a.length() && j < b.length()) {
            char ca = a.charAt(i), cb = b.charAt(j);
            if (Character.isDigit(ca) && Character.isDigit(cb)) {
                int si = i, sj = j;
                while (i < a.length() && a.charAt(i) == '0') i++;
                while (j < b.length() && b.charAt(j) == '0') j++;
                int ni = i, nj = j;
                while (ni < a.length() && Character.isDigit(a.charAt(ni))) ni++;
                while (nj < b.length() && Character.isDigit(b.charAt(nj))) nj++;
                int la = ni - i, lb = nj - j;
                if (la != lb) return la < lb ? -1 : 1;
                for (int k = 0; k < la; k++) {
                    char da = a.charAt(i + k), db = b.charAt(j + k);
                    if (da != db) return da < db ? -1 : 1;
                }
                if ((ni - si) != (nj - sj)) return (ni - si) < (nj - sj) ? -1 : 1;   // fewer leading zeros first
                i = ni;
                j = nj;
                continue;
            }
            int c = Character.compare(Character.toLowerCase(ca), Character.toLowerCase(cb));
            if (c != 0) return c;
            i++;
            j++;
        }
        return (a.length() - i) - (b.length() - j);
    }

    /**
     * Opens a ZIP pack (Stick Nodes 2.0.1 / 4.0.0): every importable file in it
     * is copied into the folder (projects, figures, movieclips, sounds,
     * images; folders inside are flattened). Unsafe names, nested ZIPs and
     * oversized archives are skipped. Returns the files added.
     */
    public List<File> extractPack(byte[] zip) throws IOException {
        List<File> out = new ArrayList<File>();
        java.util.zip.ZipInputStream in = new java.util.zip.ZipInputStream(new java.io.ByteArrayInputStream(zip));
        long total = 0;
        int entries = 0;
        try {
            java.util.zip.ZipEntry e;
            byte[] buf = new byte[65536];
            while ((e = in.getNextEntry()) != null) {
                if (++entries > 5000) break;
                if (e.isDirectory()) continue;
                String name = e.getName().replace('\\', '/');
                name = name.substring(name.lastIndexOf('/') + 1);
                if (name.isEmpty() || name.startsWith(".") || name.startsWith("__MACOSX")) continue;
                Kind k = kindOf(name);
                if (k == Kind.OTHER || k == Kind.ZIP) continue;
                java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
                int n;
                while ((n = in.read(buf)) > 0) {
                    total += n;
                    if (total > 512L * 1024 * 1024) throw new IOException("pack is too large (over 512 MB unpacked)");
                    bytes.write(buf, 0, n);
                }
                out.add(addCopy(sanitize(name), bytes.toByteArray()));
            }
        } finally {
            in.close();
        }
        return out;
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
