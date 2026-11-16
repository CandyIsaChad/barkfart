package app.barkfart.legacy;

/** Public access to the synthetic legacy figures for tests in other packages. */
public final class SyntheticAccess {
    private SyntheticAccess() {}

    public static NodesFormat.LegacyFigure figure(int version, int build) {
        return Synthetic.figure(version, build);
    }
}
