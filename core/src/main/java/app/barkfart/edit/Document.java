package app.barkfart.edit;

import app.barkfart.model.Project;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

/**
 * Owns a project and its history. Every model mutation goes through
 * apply(Command). A transaction (one gesture, e.g. a drag) groups every
 * command until commit() into a single undo step. DESIGN.md §8.
 */
public final class Document {
    public static final int DEFAULT_HISTORY = 500;

    public interface Listener {
        void changed(Document d);
    }

    public final Project project;
    private final ArrayDeque<Command> undo = new ArrayDeque<Command>();
    private final ArrayDeque<Command> redo = new ArrayDeque<Command>();
    private final List<Listener> listeners = new ArrayList<Listener>();
    private final int historyLimit;
    private Compound open;
    private long revision;

    public Document(Project project) {
        this(project, DEFAULT_HISTORY);
    }

    public Document(Project project, int historyLimit) {
        this.project = project;
        this.historyLimit = Math.max(1, historyLimit);
    }

    public long revision() {
        return revision;
    }

    public void addListener(Listener l) {
        listeners.add(l);
    }

    public void apply(Command c) {
        c.apply();
        if (open != null) {
            open.parts.add(c);
        } else {
            push(c);
        }
        changed();
    }

    private void push(Command c) {
        undo.push(c);
        while (undo.size() > historyLimit) undo.removeLast();
        redo.clear();
    }

    /** Starts grouping commands into one undo step. Nested calls join the open one. */
    public void begin(String label) {
        if (open == null) open = new Compound(label);
        else open.depth++;
    }

    /** Ends the group; an empty group leaves no history entry. */
    public void commit() {
        if (open == null) throw new IllegalStateException("no open transaction");
        if (open.depth > 0) {
            open.depth--;
            return;
        }
        Compound c = open;
        open = null;
        if (!c.parts.isEmpty()) push(c);
    }

    /** Reverts everything in the open group and discards it (cancelled gesture). */
    public void cancel() {
        if (open == null) return;
        Compound c = open;
        open = null;
        c.revert();
        changed();
    }

    public boolean inTransaction() {
        return open != null;
    }

    public boolean canUndo() {
        return open == null && !undo.isEmpty();
    }

    public boolean canRedo() {
        return open == null && !redo.isEmpty();
    }

    public String undoLabel() {
        return undo.isEmpty() ? null : undo.peek().label();
    }

    public boolean undo() {
        if (!canUndo()) return false;
        Command c = undo.pop();
        c.revert();
        redo.push(c);
        changed();
        return true;
    }

    public boolean redo() {
        if (!canRedo()) return false;
        Command c = redo.pop();
        c.apply();
        undo.push(c);
        changed();
        return true;
    }

    private void changed() {
        revision++;
        project.invalidateIndex();
        for (Listener l : listeners) l.changed(this);
    }

    static final class Compound implements Command {
        final String label;
        final List<Command> parts = new ArrayList<Command>();
        int depth;

        Compound(String label) {
            this.label = label;
        }

        public void apply() {
            for (Command c : parts) c.apply();
        }

        public void revert() {
            for (int i = parts.size() - 1; i >= 0; i--) parts.get(i).revert();
        }

        public String label() {
            return label;
        }
    }
}
