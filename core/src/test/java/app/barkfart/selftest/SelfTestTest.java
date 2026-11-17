package app.barkfart.selftest;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class SelfTestTest {
    @Test
    void inAppSelfTestPassesOnTheJvm() {
        java.io.File dir = new java.io.File(System.getProperty("java.io.tmpdir"), "barkfart-selftest-" + System.nanoTime());
        dir.mkdirs();
        SelfTest.Result r = SelfTest.run(dir);
        assertTrue(r.ok(), r.summary() + " " + r.failures);
        assertTrue(r.passed >= 13);
        assertEquals(0, dir.listFiles().length, "the documents check cleans up after itself");
    }
}
