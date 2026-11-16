package app.barkfart.edit;

/** One undoable model change. apply() and revert() must be exact inverses. */
public interface Command {
    void apply();

    void revert();

    String label();
}
