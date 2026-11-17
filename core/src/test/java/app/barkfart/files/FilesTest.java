package app.barkfart.files;

import static org.junit.jupiter.api.Assertions.*;
import static org.junit.jupiter.api.Assumptions.assumeTrue;

import app.barkfart.Fixtures;
import app.barkfart.edit.Document;
import app.barkfart.io.ProjectCodec;
import app.barkfart.legacy.NodesFormat;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.file.Files;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

class FilesTest {
    @TempDir
    File dir;

    private static void write(File f, byte[] b) throws Exception {
        FileOutputStream o = new FileOutputStream(f);
        o.write(b);
        o.close();
    }

    @Test
    void projectsLifecycle() throws Exception {
        ProjectStore st = new ProjectStore(new File(dir, "Barkfart"));
        Project p = Fixtures.kitchenSink();
        File a = st.unique("My anim", ProjectStore.EXT);
        st.save(p, a);
        File b = st.unique("My anim", ProjectStore.EXT);
        assertEquals("My anim 2.bark", b.getName(), "never overwrites");
        st.save(Fixtures.empty(), b);
        assertEquals(2, st.projects().size());
        assertEquals(ProjectCodec.encode(p), ProjectCodec.encode(st.open(a)));
        File renamed = st.rename(a, "Fight scene");
        assertEquals("Fight scene.bark", renamed.getName());
        assertFalse(a.exists());
        File dup = st.duplicate(renamed);
        assertEquals("Fight scene copy.bark", dup.getName());
        File trashed = st.delete(dup);
        assertEquals(2, st.projects().size(), "trash is hidden");
        st.restore(trashed, "Fight scene copy.bark");
        assertEquals(3, st.projects().size());
    }

    @Test
    void sanitizeNames() {
        assertEquals("ab", ProjectStore.sanitize("a/b"));
        assertEquals("Untitled", ProjectStore.sanitize("  "));
        assertEquals("Untitled", ProjectStore.sanitize("..."));
        assertEquals("x", ProjectStore.sanitize(".x"));
        assertEquals(80, ProjectStore.sanitize(new String(new char[200]).replace('\0', 'a')).length());
    }

    @Test
    void figureImportIsOneUndoStepAndExportRoundTrips() throws Exception {
        ProjectStore st = new ProjectStore(dir);
        byte[] fig = NodesFormat.write(app.barkfart.legacy.SyntheticAccess.figure(334, 0));
        File f = new File(dir, "Hero.nodes");
        write(f, fig);
        List<ProjectStore.Entry> imp = st.importables();
        assertEquals(1, imp.size());
        assertEquals(ProjectStore.Kind.FIGURE, imp.get(0).kind);
        Document d = new Document(Fixtures.empty());
        Importer.Result r = Importer.importFile(d, f);
        assertTrue(r.layer instanceof Layer.FigureLayer);
        assertEquals("Hero", r.layer.name);
        assertEquals(1, d.project.root().layers.size());
        File out = Importer.exportFigure(st, (Layer.FigureLayer) r.layer);
        assertEquals("Hero 2.nodes", out.getName());
        assertArrayEquals(fig, Files.readAllBytes(out.toPath()), "unedited figure exports byte for byte");
        assertTrue(d.undo());
        assertEquals(0, d.project.root().layers.size());
    }

    @Test
    void unsupportedFilesGiveAClearMessage() throws Exception {
        File f = new File(dir, "Old.stk");
        write(f, new byte[] {1, 2, 3});
        Document d = new Document(Fixtures.empty());
        Importer.Unsupported e = assertThrows(Importer.Unsupported.class, () -> Importer.importFile(d, f));
        assertTrue(e.getMessage().contains("cannot be imported yet"));
    }

    /** Local-only: every sample project's figure library imports (animation not yet). */
    @Test
    void legacyProjectFiguresImport() throws Exception {
        File dirRef = new File(System.getProperty("barkfart.reference", "missing"), "sample_projects");
        assumeTrue(dirRef.isDirectory(), "reference bundle not present");
        int[] expected = {3, 6, 15};
        for (int i = 1; i <= 3; i++) {
            File f = new File(dirRef, "sample" + i + ".stknds");
            app.barkfart.legacy.StkndsFormat.Header h = app.barkfart.legacy.StkndsFormat.readHeader(Files.readAllBytes(f.toPath()));
            assertEquals(431, h.version);
            assertEquals(expected[i - 1], h.figures.size(), f.getName());
            Document d = new Document(Fixtures.empty());
            Importer.Result r = Importer.importFile(d, f);
            assertEquals(expected[i - 1], d.project.root().layers.size());
            assertTrue(r.message.contains("animation cannot be imported yet"));
            assertTrue(d.undo());
            assertEquals(0, d.project.root().layers.size());
        }
    }

    /** Local-only: a real movieclip imports as a symbol plus a looping instance layer. */
    @Test
    void movieclipImport() throws Exception {
        File clip = new File(System.getProperty("barkfart.reference", "missing"), "movieclips/_Fire.nodemc");
        assumeTrue(clip.isFile(), "reference bundle not present");
        Document d = new Document(Fixtures.empty());
        Importer.Result r = Importer.importFile(d, clip);
        assertTrue(r.layer instanceof Layer.SymbolLayer);
        assertEquals(2, d.project.library.size());
        assertTrue(d.undo());
        assertEquals(1, d.project.library.size());
        assertEquals(0, d.project.root().layers.size());
    }

    @Test
    void sessionSavesOnlyWhenDirtyAndNamesNewProjects() throws Exception {
        ProjectStore st = new ProjectStore(dir);
        Session s = new Session(st);
        Project p = Fixtures.empty();
        p.root().layers.add(Fixtures.stickman(p, "man"));
        Document d = new Document(p);
        s.attach(d, null, "Untitled");
        assertFalse(s.dirty(), "an untouched new project is not saved");
        assertFalse(s.saveIfDirty());
        d.apply(app.barkfart.edit.StructureEdits.setFps(p, 12));
        assertTrue(s.dirty());
        assertTrue(s.saveIfDirty());
        assertEquals("Untitled.bark", s.file().getName());
        assertFalse(s.dirty());
        assertFalse(s.saveIfDirty());
        assertEquals(12f, st.open(s.file()).settings.fps);
        d.undo();
        assertTrue(s.dirty(), "undo is a change too");
    }

    @Test
    void listsSortNaturallyAndFilter() throws Exception {
        java.io.File dir = java.nio.file.Files.createTempDirectory("lists").toFile();
        ProjectStore store = new ProjectStore(dir);
        String[] names = {"fig 10.nodes", "Fig 2.nodes", "fig 1.nodes", "walk.nodemc", "boom.wav"};
        long t = 1_700_000_000_000L;
        for (int i = 0; i < names.length; i++) {
            java.io.File f = new java.io.File(dir, names[i]);
            java.nio.file.Files.write(f.toPath(), new byte[] {1});
            f.setLastModified(t + i * 1000L);
        }
        java.util.List<String> byName = new java.util.ArrayList<String>();
        for (ProjectStore.Entry e : store.list(false, ProjectStore.Sort.NAME, null)) byName.add(e.name);
        assertEquals(java.util.Arrays.asList("boom", "fig 1", "Fig 2", "fig 10", "walk"), byName);
        assertEquals("boom", store.list(false, ProjectStore.Sort.NEWEST, null).get(0).name);
        assertEquals("fig 10", store.list(false, ProjectStore.Sort.OLDEST, null).get(0).name);
        assertEquals(3, store.list(false, ProjectStore.Sort.NAME, "FIG").size());
        assertTrue(ProjectStore.natural("a2", "a10") < 0 && ProjectStore.natural("a02", "a2") > 0 && ProjectStore.natural("b", "A") > 0);
    }

    @Test
    void zipPacksUnpackSafely() throws Exception {
        java.io.File dir = java.nio.file.Files.createTempDirectory("pack").toFile();
        ProjectStore store = new ProjectStore(dir);
        java.io.ByteArrayOutputStream bytes = new java.io.ByteArrayOutputStream();
        java.util.zip.ZipOutputStream z = new java.util.zip.ZipOutputStream(bytes);
        for (String n : new String[] {"pack/heroes/Hero.nodes", "../../evil.nodemc", "__MACOSX/._Hero.nodes", "readme.txt", "inner.zip", "sfx/boom.wav"}) {
            z.putNextEntry(new java.util.zip.ZipEntry(n));
            z.write(new byte[] {1, 2, 3});
            z.closeEntry();
        }
        z.close();
        java.util.List<java.io.File> got = store.extractPack(bytes.toByteArray());
        java.util.Set<String> names = new java.util.TreeSet<String>();
        for (java.io.File f : got) {
            names.add(f.getName());
            assertEquals(dir.getCanonicalFile(), f.getParentFile().getCanonicalFile(), "nothing escapes the folder");
        }
        assertEquals(new java.util.TreeSet<String>(java.util.Arrays.asList("Hero.nodes", "evil.nodemc", "boom.wav")), names);
    }

    @Test
    void viewStateIsSavedWithTheProject() throws Exception {
        app.barkfart.model.Project p = app.barkfart.Fixtures.empty();
        p.view.zoom = 2.5f;
        p.view.panX = -40;
        p.view.frame = 7;
        String s = app.barkfart.io.ProjectCodec.encode(p);
        app.barkfart.model.Project q = app.barkfart.io.ProjectCodec.decode(s);
        assertEquals(2.5f, q.view.zoom);
        assertEquals(-40f, q.view.panX);
        assertEquals(7, q.view.frame);
        assertFalse(app.barkfart.io.ProjectCodec.encode(app.barkfart.Fixtures.empty()).contains("\"view\""), "default view not written");
    }
}
