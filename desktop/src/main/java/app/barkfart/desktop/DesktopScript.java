package app.barkfart.desktop;

import app.barkfart.BarkfartApp;
import app.barkfart.edit.DopeRows;
import app.barkfart.edit.StructureEdits;
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
                st.doc.apply(StructureEdits.setDuration(st.project().root(), 24 / st.editor.fps()));
                Map<String, Float> rot = new HashMap<String, Float>();
                for (int i = 0; i <= 18; i += 6) {
                    rot.put("armR1", -70f + i * 5);
                    rot.put("armL1", -150f + i * 3);
                    st.editor.pose(f, rot, null, st.frames.time(i));
                }
                st.setFrame(9);
            } else if (step.equals("camera")) {
                st.mode = EditorState.Mode.CAMERA;
                st.fire();
            } else if (step.equals("deselect")) st.select(null, null);
            else if (step.equals("expand")) {
                st.sheetHeight = 200;
                st.fire();
            } else if (step.equals("keys") && f != null) {
                st.selectKeys(DopeRows.keysIn(DopeRows.build(st.project(), f, null).subList(4, 6), 0.15, 0.45), false);
            }
        }
    }
}
