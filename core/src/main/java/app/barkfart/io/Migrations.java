package app.barkfart.io;

import java.util.Map;

/** Upgrades older project JSON one version at a time. Version 1 is the first; none exist yet. */
final class Migrations {
    private Migrations() {}

    static Map<String, Object> upgrade(Map<String, Object> m, int fromVersion) {
        // Add "if (v == N) m = vNtoN1(m);" steps here as the format evolves.
        return m;
    }
}
