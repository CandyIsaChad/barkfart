package app.barkfart.ui;

import static org.junit.jupiter.api.Assertions.*;

import org.junit.jupiter.api.Test;

class ScaleTest {
    @Test
    void autoScaleByScreenHeight() {
        assertEquals(1f, EditorScreen.autoScale(390), 1e-6);        // iPhone landscape
        assertEquals(1f, EditorScreen.autoScale(430), 1e-6);        // iPhone Pro Max landscape
        assertEquals(1.134f, EditorScreen.autoScale(744), 1e-3);    // iPad mini
        assertEquals(1.25f, EditorScreen.autoScale(820), 1e-3);     // iPad Air / 10.9
        assertEquals(1.271f, EditorScreen.autoScale(834), 1e-3);    // iPad Pro 11
        assertEquals(1.5f, EditorScreen.autoScale(1024), 1e-6);     // iPad Pro 12.9 (capped)
    }
}
