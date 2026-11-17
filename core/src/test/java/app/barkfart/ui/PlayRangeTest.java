package app.barkfart.ui;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class PlayRangeTest {
    @Test
    void playbackLoopsInsideTheRange() {
        Project p = Fixtures.empty();
        p.root().duration = 2;
        EditorState st = new EditorState(p);
        st.rangeStart = 5;
        st.rangeEnd = 9;
        double fps = st.editor.fps(), a = 5 / fps, b = 10 / fps;
        st.setFrame(20);   // outside: play starts at the range start
        st.play();
        assertEquals(a, st.playTime, 1e-9);
        boolean wrapped = false;
        double last = st.playTime;
        for (int i = 0; i < 200; i++) {
            st.update(1 / 60f);
            assertTrue(st.playTime >= a - 1e-9 && st.playTime < b + 1e-9, "inside the range: " + st.playTime);
            wrapped |= st.playTime < last;
            last = st.playTime;
        }
        assertTrue(wrapped, "it loops");
        st.stop();
        st.rangeStart = st.rangeEnd = -1;
        assertArrayEquals(new double[] {0, 2}, st.playRange(), 1e-9);
    }
}
