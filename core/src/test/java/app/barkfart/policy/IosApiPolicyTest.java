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
 * Core runs on iOS through MobiVM, whose runtime is older than desktop Java.
 * Keep core main code to APIs it is known to have. JVM tests cannot catch a
 * missing class (it would fail at runtime on the phone), so ban the risky ones.
 */
class IosApiPolicyTest {
    private static final Pattern FORBIDDEN = Pattern.compile(
            "\\bjava\\.util\\.stream\\b|\\.stream\\(\\)|\\bjava\\.time\\b|\\bjava\\.nio\\.file\\b|"
                    + "\\bjava\\.awt\\b|\\bjavax\\.imageio\\b|\\bjavax\\.swing\\b|\\bjava\\.lang\\.invoke\\b");

    @Test
    void coreMainAvoidsApisMissingOnIos() throws IOException {
        Path root = Paths.get(System.getProperty("barkfart.root", "..")).toAbsolutePath().normalize();
        Path src = root.resolve("core/src/main/java");
        List<String> hits = new ArrayList<>();
        try (Stream<Path> files = Files.walk(src)) {
            for (Path f : (Iterable<Path>) files.filter(p -> p.toString().endsWith(".java"))::iterator) {
                List<String> lines = Files.readAllLines(f);
                for (int i = 0; i < lines.size(); i++) {
                    if (FORBIDDEN.matcher(lines.get(i)).find()) {
                        hits.add(root.relativize(f) + ":" + (i + 1) + ": " + lines.get(i).trim());
                    }
                }
            }
        }
        assertTrue(hits.isEmpty(), "APIs that may be missing on iOS:\n" + String.join("\n", hits));
    }
}
