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
}
