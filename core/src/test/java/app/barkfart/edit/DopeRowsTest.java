package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

class DopeRowsTest {
    @Test
    void rowsFilterToTheSelectedFigureAndCollapse() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a"), b = Fixtures.stickman(p, "b");
        p.root().layers.add(a);
        p.root().layers.add(b);
        p.root().layers.add(new Layer.CameraLayer(p.newId("L"), "cam"));
        List<DopeRows.Row> all = DopeRows.build(p, null, null);
        // summary + per figure (header + move + 10 joints) x2 + camera
        assertEquals(1 + 2 * 12 + 1, all.size());
        List<DopeRows.Row> onlyA = DopeRows.build(p, a, null);
        assertEquals(1 + 12 + 1, onlyA.size());
        Set<String> collapsed = new HashSet<String>(Collections.singleton(a.id));
        assertEquals(1 + 1 + 1, DopeRows.build(p, a, collapsed).size());
    }

    @Test
    void rowTimesAggregateTheirPropertiesAndSummaryHasEverything() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a");
        p.root().layers.add(a);
        a.figure.node("armR1").rotation.track().put(new Key(0.5, 1f, Interp.LINEAR));
        a.figure.node("armR1").length.track().put(new Key(1.0, 50f, Interp.LINEAR));
        a.figure.node("legL1").rotation.track().put(new Key(0.25, 2f, Interp.LINEAR));
        List<DopeRows.Row> rows = DopeRows.build(p, null, null);
        DopeRows.Row arm = null;
        for (DopeRows.Row r : rows) if (r.props.contains(a.figure.node("armR1").rotation)) if (r.kind == DopeRows.Kind.JOINT) arm = r;
        assertEquals(Arrays.asList(0.5, 1.0), arm.times());
        assertEquals(Arrays.asList(0.25, 0.5, 1.0), rows.get(0).times());
        assertEquals(2, DopeRows.keysIn(rows.subList(0, 1), 0.4, 1.1).size());
    }

    @Test
    void selectionSurvivesARetime() {
        Project p = Fixtures.empty();
        Layer.FigureLayer a = Fixtures.stickman(p, "a");
        p.root().layers.add(a);
        p.invalidateIndex();
        Document d = new Document(p);
        Editor e = new Editor(d);
        a.figure.node("armR1").rotation.track().put(new Key(0.5, 1f, Interp.LINEAR));
        Set<KeyRef> sel = new HashSet<KeyRef>(Collections.singleton(new KeyRef(a.figure.node("armR1").rotation, 0.5)));
        e.retime(new ArrayList<KeyRef>(sel), 0.25);
        Set<KeyRef> moved = DopeRows.shifted(sel, 0.25);
        assertTrue(moved.contains(new KeyRef(a.figure.node("armR1").rotation, 0.75)));
        assertEquals(0.75, a.figure.node("armR1").rotation.track().get(0).time, 1e-12);
    }
}
