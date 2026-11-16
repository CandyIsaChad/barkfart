package app.barkfart;

import app.barkfart.model.Color;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;

/** Synthetic test projects built in code (never from legacy files). */
public final class Fixtures {
    private Fixtures() {}

    public static Project empty() {
        Project p = new Project();
        Symbol root = new Symbol(p.newId("S"), "Root", 2.0);
        p.addSymbol(root);
        p.rootSymbol = root.id;
        return p;
    }

    /** Classic-style stick figure: root at hip, torso, head, two arms, two legs. */
    public static Layer.FigureLayer stickman(Project p, String name) {
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), name);
        String id = l.id;
        Node root = new Node(id, "hip", null, Node.Shape.NONE);
        l.figure.add(root);
        add(l, "torso", "hip", Node.Shape.LINE, 90, 60);
        add(l, "head", "torso", Node.Shape.CIRCLE, 0, 50);
        add(l, "armL1", "torso", Node.Shape.LINE, -150, 45);
        add(l, "armL2", "armL1", Node.Shape.LINE, 20, 45);
        add(l, "armR1", "torso", Node.Shape.LINE, 150, 45);
        add(l, "armR2", "armR1", Node.Shape.LINE, -20, 45);
        add(l, "legL1", "hip", Node.Shape.LINE, -110, 55);
        add(l, "legL2", "legL1", Node.Shape.LINE, 10, 55);
        add(l, "legR1", "hip", Node.Shape.LINE, -70, 55);
        add(l, "legR2", "legR1", Node.Shape.LINE, -10, 55);
        return l;
    }

    public static Node add(Layer.FigureLayer l, String id, String parent, Node.Shape shape, float rot, float len) {
        Node n = new Node(l.id, id, parent, shape);
        n.rotation.setValue(rot);
        n.length.setValue(len);
        n.thickness.setValue(12f);
        n.color.setValue(Color.BLACK);
        l.figure.add(n);
        return n;
    }
}
