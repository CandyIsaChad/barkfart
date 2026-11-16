package app.barkfart;

import app.barkfart.model.Asset;
import app.barkfart.model.Clip;
import app.barkfart.model.Color;
import app.barkfart.model.Ease;
import app.barkfart.model.EffectNode;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Marker;
import app.barkfart.model.Modifier;
import app.barkfart.model.NodeSet;
import app.barkfart.model.Scope;
import app.barkfart.model.ValueType;
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

    /** Every model feature at least once, for round-trip and fuzz tests. */
    public static Project kitchenSink() {
        Project p = empty();
        p.settings.fps = 24;
        p.settings.background = Color.rgba8888(0x101820ff);
        Symbol root = p.root();
        root.markers.add(new Marker(0.5, "hit", Color.rgba8888(0xff0000ff)));
        Layer.FigureLayer man = stickman(p, "man \"quoted\"\n");
        Node arm = man.figure.node("armR1");
        arm.rotation.track().put(new Key(0, 150f, Interp.BEZIER, new Ease(0.1f, -0.3f, 0.7f, 1.4f)));
        arm.rotation.track().put(new Key(0.75, 30.123457f, Interp.HOLD));
        arm.rotation.track().put(new Key(1.5, -363.24f, Interp.LINEAR));
        man.figure.node("head").color.track().put(new Key(0, Color.rgba8888(0x11223344), Interp.LINEAR));
        man.figure.node("head").color.track().put(new Key(1, Color.rgba8888(0xaabbccdd), Interp.LINEAR));
        man.figure.node("legL1").shown.track().put(new Key(1.0, false, Interp.HOLD));
        man.figure.node("legL1").legacyExtras.put("thicknessInt", 21);
        man.figure.node("legL1").legacyExtras.put("stretchy", true);
        man.figure.node("legL1").legacyExtras.put("defaultLength", 1.5f);
        man.figure.node("legL1").legacyExtras.put("note", "a:b");
        Node follow = man.figure.node("armL1");
        follow.rotation.addModifier(new Modifier.DelayFollow(arm.rotation.id, 1.0 / 24, 1, 0));
        follow.rotation.addModifier(new Modifier.Wiggle(2, 4, 7, 3));
        man.figure.node("head").rotation.addModifier(new Modifier.Loop(Modifier.Loop.Mode.PING_PONG, 0, 1));
        man.figure.node("head").rotation.addModifier(new Modifier.Sine(0.5f, 3, 0.25f));
        man.figure.node("torso").rotation.addModifier(new Modifier.Link(arm.rotation.id, 0.1f, 90));
        man.transform.x.track().put(new Key(0, -100f, Interp.LINEAR));
        man.transform.x.track().put(new Key(2, 100f, Interp.LINEAR));
        EffectNode glow = new EffectNode(man.id, "fx1", "glow");
        glow.param(man.id, "radius", ValueType.FLOAT, 8f).track().put(new Key(0, 2f, Interp.LINEAR));
        glow.param(man.id, "color", ValueType.COLOR, Color.rgba8888(0xffee00ff));
        man.effects.append(glow);
        EffectNode blur = new EffectNode(man.id, "fx2", "blur.gaussian");
        blur.mix.setValue(0.5f);
        blur.mask = new Scope(Scope.Kind.NODE, man.id, "head");
        man.effects.append(blur);
        root.layers.add(man);

        Symbol inner = p.addSymbol(new Symbol(p.newId("S"), "inner", 1.0));
        inner.layers.add(stickman(p, "tiny"));
        Layer.SymbolLayer inst = new Layer.SymbolLayer(p.newId("L"), "inst", inner.id);
        inst.speed.track().put(new Key(0, 1.0, Interp.LINEAR));
        inst.speed.track().put(new Key(2, 3.0, Interp.LINEAR));
        inst.timeOffset.setValue(0.125);
        inst.loop = Layer.SymbolLayer.LoopMode.PING_PONG;
        inst.recursionLimit = 3;
        inst.parent = man.id;
        inst.transform.scaleX.setValue(0.5f);
        inst.transform.scaleY.setValue(0.5f);
        root.layers.add(inst);

        Asset img = new Asset(p.newId("A"), Asset.Kind.IMAGE, "assets/img.png");
        p.assets.put(img.id, img);
        Layer.SpriteLayer sprite = new Layer.SpriteLayer(p.newId("L"), "sprite");
        sprite.asset = img.id;
        sprite.width = 64;
        sprite.height = 32;
        sprite.blend = Layer.Blend.MULTIPLY;
        root.layers.add(sprite);

        Layer.TextLayer text = new Layer.TextLayer(p.newId("L"), "caption");
        text.text = "Hi \u00e9\u2603";
        root.layers.add(text);

        Layer.AdjustmentLayer adj = new Layer.AdjustmentLayer(p.newId("L"), "adj");
        EffectNode inv = new EffectNode(adj.id, "fx1", "invert");
        adj.effects.append(inv);
        adj.opacity.setValue(0.75f);
        root.layers.add(adj);

        Asset snd = new Asset(p.newId("A"), Asset.Kind.AUDIO, "assets/boom.ogg");
        p.assets.put(snd.id, snd);
        Layer.AudioLayer audio = new Layer.AudioLayer(p.newId("L"), "audio");
        Clip clip = new Clip(audio.id, "c1", snd.id, 0.5, 0.0, 1.25);
        clip.fadeIn = 0.1;
        clip.volume.track().put(new Key(0.5, 1f, Interp.LINEAR));
        clip.volume.track().put(new Key(1.5, 0f, Interp.LINEAR));
        audio.clips.add(clip);
        root.layers.add(audio);

        Layer.CameraLayer cam = new Layer.CameraLayer(p.newId("L"), "cam");
        cam.zoom.track().put(new Key(0, 1f, Interp.BEZIER, Ease.EASE_IN_OUT));
        cam.zoom.track().put(new Key(2, 1.5f, Interp.LINEAR));
        cam.x.addModifier(new Modifier.Wiggle(6, 3, 99, 2));
        root.layers.add(cam);

        NodeSet set = new NodeSet(p.newId("N"), "arms");
        set.members.add(new String[] {man.id, "armL1"});
        set.members.add(new String[] {man.id, "armR1"});
        p.nodeSets.put(set.id, set);
        p.invalidateIndex();
        return p;
    }
}
