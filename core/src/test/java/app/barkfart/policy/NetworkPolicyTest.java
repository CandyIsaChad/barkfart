package app.barkfart.policy;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.junit.jupiter.api.Test;

/**
 * Barkfart makes no network requests. This test fails if app source code starts
 * using a network API. A future user-initiated feature (for example "share")
 * must be added to ALLOWED with a written reason in docs/barkfart/NETWORK.md.
 */
class NetworkPolicyTest {
    private static final Pattern FORBIDDEN = Pattern.compile(
            "\\bjava\\.net\\b|\\bjavax\\.net\\b|Gdx\\.net\\b|\\bHttpURLConnection\\b|"
                    + "\\bSocket\\b|\\bServerSocket\\b|\\bDatagramSocket\\b|\\bnew URL\\(|"
                    + "\\bNSURLSession\\b|\\bNSURLConnection\\b|okhttp|firebase|crashlytics|admob");

    /** Relative paths of files allowed to use network APIs. Empty on purpose. */
    private static final List<String> ALLOWED = new ArrayList<>();

    @Test
    void appSourceUsesNoNetworkApis() throws IOException {
        Path root = Paths.get(System.getProperty("barkfart.root", "..")).toAbsolutePath().normalize();
        List<String> hits = new ArrayList<>();
        int scanned = 0;
        for (String module : new String[] {"core", "desktop", "ios"}) {
            Path src = root.resolve(module).resolve("src").resolve("main");
            if (!Files.isDirectory(src)) continue;
            try (Stream<Path> files = Files.walk(src)) {
                for (Path f : (Iterable<Path>) files.filter(p -> p.toString().endsWith(".java"))::iterator) {
                    scanned++;
                    String rel = root.relativize(f).toString();
                    if (ALLOWED.contains(rel)) continue;
                    List<String> lines = Files.readAllLines(f);
                    for (int i = 0; i < lines.size(); i++) {
                        if (FORBIDDEN.matcher(lines.get(i)).find()) hits.add(rel + ":" + (i + 1) + ": " + lines.get(i).trim());
                    }
                }
            }
        }
        assertTrue(scanned > 0, "no source files found under " + root);
        assertTrue(hits.isEmpty(), "Network API use found:\n" + String.join("\n", hits));
    }
}
