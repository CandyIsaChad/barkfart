package app.barkfart.selftest;

import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class SelfTestTest {
    @Test
    void inAppSelfTestPassesOnTheJvm() {
        SelfTest.Result r = SelfTest.run();
        assertTrue(r.ok(), r.summary() + " " + r.failures);
        assertTrue(r.passed >= 9);
    }
}
