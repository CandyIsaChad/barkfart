package app.barkfart.desktop;

import app.barkfart.BarkfartApp;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.ui.EditorState;
import java.util.HashMap;
import java.util.Map;

/** Scripted state changes for command-line screenshots (desktop only). */
final class DesktopScript {
    private DesktopScript() {}

    static void run(BarkfartApp app, String steps) {
        EditorState st = app.state();
        for (String step : steps.split(",")) {
            Layer.FigureLayer f = st.figures().isEmpty() ? null : st.figures().get(0);
            if (step.equals("select") && f != null) st.select(f, f.figure.node("armR1"));
            else if (step.equals("frames") && f != null) {
                for (int i = 0; i < 6; i++) st.frames.insertAfter(st.frames.frameCount() - 1, true);
                Map<String, Float> rot = new HashMap<String, Float>();
                for (int i = 0; i <= 6; i += 2) {
                    rot.put("armR1", -70f + i * 15);
                    rot.put("armL1", -150f + i * 8);
                    st.editor.pose(f, rot, null, st.frames.time(i));
                }
                st.setFrame(3);
            } else if (step.equals("camera")) {
                st.mode = EditorState.Mode.CAMERA;
                st.fire();
            } else if (step.equals("deselect")) st.select(null, null);
        }
    }
}
