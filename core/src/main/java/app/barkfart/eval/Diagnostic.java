package app.barkfart.eval;

/** A problem found while evaluating. Evaluation always finishes; it records these instead of throwing. */
public final class Diagnostic {
    public enum Kind { CYCLE, MISSING_REF, RECURSION_LIMIT, DEPTH_CAP, PRIMITIVE_BUDGET, PARENT_CYCLE, BAD_VALUE, RESOLVE_BUDGET }

    public final Kind kind;
    public final String subject;

    public Diagnostic(Kind kind, String subject) {
        this.kind = kind;
        this.subject = subject;
    }

    @Override
    public String toString() {
        return kind + ":" + subject;
    }
}
