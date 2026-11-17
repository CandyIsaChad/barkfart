package app.barkfart.files;

import app.barkfart.edit.Document;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Project;
import java.io.File;
import java.io.IOException;

/**
 * The open project's file and saved state. Autosave writes only when the
 * document changed since the last save; a new project gets a file on its
 * first save ("Untitled.bark", "Untitled 2.bark", ...).
 */
public final class Session {
    public final ProjectStore store;
    private File file;
    private Document doc;
    private long savedRevision = -1;
    private String name;

    public Session(ProjectStore store) {
        this.store = store;
    }

    public void attach(Document doc, File file, String name) {
        this.doc = doc;
        this.file = file;
        this.name = name;
        savedRevision = file == null ? -1 : doc.revision();
    }

    public File file() {
        return file;
    }

    public String name() {
        return name;
    }

    /** Unsaved changes (a never-saved project counts as dirty once edited). */
    public boolean dirty() {
        return doc != null && doc.revision() != savedRevision && (file != null || doc.revision() > 0);
    }

    /** Runs just before every save (the editor stores its view state here). */
    public Runnable beforeSave;

    public File save() throws IOException {
        if (beforeSave != null) beforeSave.run();
        if (file == null) file = store.unique(name == null ? "Untitled" : name, ProjectStore.EXT);
        store.save(doc.project, file);
        savedRevision = doc.revision();
        name = new ProjectStore.Entry(file, ProjectStore.Kind.PROJECT).name;
        return file;
    }

    public boolean saveIfDirty() throws IOException {
        if (!dirty()) return false;
        save();
        return true;
    }

    public Project open(File f) throws IOException, ProjectCodec.FormatException {
        return store.open(f);
    }

    public void renamed(File to) {
        file = to;
        name = new ProjectStore.Entry(to, ProjectStore.Kind.PROJECT).name;
    }
}
