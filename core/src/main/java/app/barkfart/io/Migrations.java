package app.barkfart.io;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * Upgrades project JSON one version at a time (FORMAT.md). Step i turns
 * version i into version i + 1. Version 1 is the first released format, so
 * there are no steps yet; tests exercise the mechanism with their own steps.
 */
final class Migrations {
    interface Step {
        Map<String, Object> apply(Map<String, Object> project);
    }

    /** STEPS.get(i) upgrades version (i + 1) to (i + 2). */
    static final List<Step> STEPS = new ArrayList<Step>();

    private Migrations() {}

    static Map<String, Object> upgrade(Map<String, Object> m, int fromVersion) {
        return upgrade(m, fromVersion, 1 + STEPS.size(), STEPS);
    }

    static Map<String, Object> upgrade(Map<String, Object> m, int fromVersion, int toVersion, List<Step> steps) {
        for (int v = fromVersion; v < toVersion; v++) {
            int i = v - 1;
            if (i < 0 || i >= steps.size()) throw new IllegalArgumentException("no migration from version " + v);
            m = steps.get(i).apply(m);
        }
        return m;
    }
}
