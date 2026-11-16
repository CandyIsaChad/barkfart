package app.barkfart.demo;

import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Modifier;
import app.barkfart.model.Node;
import app.barkfart.model.Project;
import app.barkfart.model.Symbol;

/** Built-in demo content, made in code (no legacy assets). */
public final class Demo {
    private Demo() {}

    public static Node add(Layer.FigureLayer l, String id, String parent, Node.Shape shape, float rot, float len, float thick) {
        Node n = new Node(l.id, id, parent, shape);
        n.rotation.setValue(rot);
        n.length.setValue(len);
        n.thickness.setValue(thick);
        n.color.setValue(Color.BLACK);
        l.figure.add(n);
        return n;
    }

    public static Layer.FigureLayer stickman(Project p, String name) {
        Layer.FigureLayer l = new Layer.FigureLayer(p.newId("L"), name);
        l.figure.add(new Node(l.id, "hip", null, Node.Shape.NONE));
        add(l, "torso", "hip", Node.Shape.LINE, 90, 60, 12);
        add(l, "head", "torso", Node.Shape.CIRCLE, 0, 50, 12);
        add(l, "armL1", "torso", Node.Shape.LINE, -150, 45, 12);
        add(l, "armL2", "armL1", Node.Shape.LINE, 20, 45, 12);
        add(l, "armR1", "torso", Node.Shape.LINE, 150, 45, 12);
        add(l, "armR2", "armR1", Node.Shape.LINE, -20, 45, 12);
        add(l, "legL1", "hip", Node.Shape.LINE, -110, 55, 12);
        add(l, "legL2", "legL1", Node.Shape.LINE, 10, 55, 12);
        add(l, "legR1", "hip", Node.Shape.LINE, -70, 55, 12);
        add(l, "legR2", "legR1", Node.Shape.LINE, -10, 55, 12);
        return l;
    }

    /**
     * A waving figure whose other arm follows two frames late, a nested
     * self-referencing symbol, and a gently wiggling camera. Loops every 2 s.
     */
    public static Project waving() {
        Project p = new Project();
        Symbol root = p.addSymbol(new Symbol(p.newId("S"), "Demo", 2.0));
        p.rootSymbol = root.id;
        Symbol buddy = p.addSymbol(new Symbol(p.newId("S"), "Buddy", 1.0));

        Layer.FigureLayer man = stickman(p, "waver");
        man.transform.y.setValue(-20f);
        Node arm = man.figure.node("armR1");
        // Torso points up (90 degrees), so -70..-30 relative swings the arm
        // between 20 and 60 degrees: out to the right and up, clear of the head.
        arm.rotation.track().put(new Key(0, -70f, Interp.BEZIER, Ease.EASE_IN_OUT));
        arm.rotation.track().put(new Key(0.5, -30f, Interp.BEZIER, Ease.EASE_IN_OUT));
        arm.rotation.track().put(new Key(1.0, -70f, Interp.LINEAR));
        arm.rotation.addModifier(new Modifier.Loop(Modifier.Loop.Mode.CYCLE, 0, 1.0));
        Node fore = man.figure.node("armR2");
        fore.rotation.addModifier(new Modifier.Sine(2, 25, 0));
        Node other = man.figure.node("armL1");
        // The left arm echoes the wave two frames late, mirrored and lowered.
        other.rotation.addModifier(new Modifier.DelayFollow(arm.rotation.id, 2 / 30.0, -0.5f, 120));
        man.figure.node("head").color.setValue(Color.rgba8888(0xf2ac5cff));
        root.layers.add(man);

        Layer.FigureLayer small = stickman(p, "buddy");
        small.figure.node("head").color.setValue(Color.rgba8888(0x3a7bd5ff));
        small.figure.node("armL1").rotation.addModifier(new Modifier.Sine(1, 30, 0.25f));
        buddy.layers.add(small);
        Layer.SymbolLayer self = new Layer.SymbolLayer(p.newId("L"), "again", buddy.id);
        self.transform.x.setValue(90f);
        self.transform.scaleX.setValue(0.65f);
        self.transform.scaleY.setValue(0.65f);
        self.recursionLimit = 6;
        buddy.layers.add(self);
        Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), "buddies", buddy.id);
        inst.transform.x.setValue(120f);
        inst.transform.y.setValue(-40f);
        inst.transform.scaleX.setValue(0.7f);
        inst.transform.scaleY.setValue(0.7f);
        root.layers.add(inst);

        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "camera");
        cam.zoom.setValue(1.0f);
        cam.x.setValue(70f);
        cam.rotation.addModifier(new Modifier.Wiggle(0.5f, 2f, 3, 2));
        root.layers.add(cam);
        p.invalidateIndex();
        return p;
    }
}
