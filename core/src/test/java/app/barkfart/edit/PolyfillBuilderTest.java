package app.barkfart.edit;

import static org.junit.jupiter.api.Assertions.*;

import app.barkfart.Fixtures;
import app.barkfart.eval.Evaluator;
import app.barkfart.geom.Tessellator;
import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import org.junit.jupiter.api.Test;

class PolyfillBuilderTest {
    @Test
    void pickingNodesMakesADrawnFillInOneUndoStep() {
        Project p = Fixtures.empty();
        Layer.FigureLayer man = Fixtures.stickman(p, "man");
        p.root().layers.add(man);
        p.invalidateIndex();
        Document d = new Document(p);
        int before = Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0)).triangleCount();
        PolyfillBuilder b = new PolyfillBuilder(man, "torso");
        assertFalse(b.add("nope"));
        assertFalse(b.add("torso"), "not the anchor first");
        assertTrue(b.add("armL1"));
        assertFalse(b.add("armL1"), "not the same node twice in a row");
        assertFalse(b.canFinish());
        assertTrue(b.add("armR1"));
        assertTrue(b.add("legL1"));
        assertTrue(b.undoLast());
        assertTrue(b.canFinish());
        d.apply(b.finish(PolyfillBuilder.freeId(man), Color.rgba8888(0x30d158ff)));
        assertEquals(1, man.figure.polyfills.size());
        assertTrue(Tessellator.tessellate(Evaluator.evaluate(p, p.rootSymbol, 0)).triangleCount() > before, "the fill is drawn");
        d.undo();
        assertTrue(man.figure.polyfills.isEmpty());
    }
}
