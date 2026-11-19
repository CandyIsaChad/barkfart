package app.barkfart.edit;

import app.barkfart.Fixtures;
import app.barkfart.io.ProjectCodec;
import app.barkfart.model.Ease;
import app.barkfart.model.Interp;
import app.barkfart.model.Key;
import app.barkfart.model.Layer;
import app.barkfart.model.Project;
import app.barkfart.model.Property;
import java.io.File;
import java.io.FileOutputStream;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;

/**
 * Reference data for the Rust editing operations (docs/barkfart/PORT.md).
 * Only runs with -Dbarkfart.editdump=<dir>. Each scenario applies one
 * operation with fixed arguments to the kitchen-sink project and writes the
 * project before, after, and after undo. The Rust test runs the same
 * scenario by name.
 */
class EditDumpTest {
    interface Op {
        void run(Document d, Editor e, Project p) throws Exception;
    }

    private File dir;

    private void scenario(String name, Op op) throws Exception {
        Project p = Fixtures.kitchenSink();
        Document d = new Document(p);
        Editor e = new Editor(d);
        write(name + ".before.json", ProjectCodec.encode(p));
        op.run(d, e, p);
        write(name + ".after.json", ProjectCodec.encode(p));
        d.undo();
        write(name + ".undo.json", ProjectCodec.encode(p));
    }

    private void write(String name, String text) throws Exception {
        FileOutputStream o = new FileOutputStream(new File(dir, name));
        o.write(text.getBytes("UTF-8"));
        o.close();
    }

    private static List<KeyRef> keys(Project p, String id) {
        List<KeyRef> out = new ArrayList<KeyRef>();
        Property q = p.property(id);
        for (Key k : q.track().keys()) out.add(new KeyRef(q, k.time));
        return out;
    }

    private static Layer.FigureLayer man(Project p) {
        return (Layer.FigureLayer) p.root().layers.get(0);
    }

    private void figureScenarios() throws Exception {
        scenario("add-segment", (d, e, p) -> d.apply(FigureOps.addSegment(man(p), "armR2", 30f, 20f, null)));
        scenario("add-segment-root", (d, e, p) -> d.apply(FigureOps.addSegment(man(p), "hip", -45f, 33f, null)));
        scenario("split-3", (d, e, p) -> d.apply(FigureOps.split(man(p), "armR1", 3)));
        scenario("split-curved", (d, e, p) -> {
            d.apply(FigureOps.edit("Curve", man(p), () -> man(p).figure.node("legL1").curve = 12f));
            d.apply(FigureOps.split(man(p), "legL1", 4));
        });
        scenario("duplicate-branch", (d, e, p) -> d.apply(FigureOps.duplicateBranch(man(p), "armL1", null)));
        scenario("duplicate-branch-fill", (d, e, p) -> d.apply(FigureOps.duplicateBranch(man(p), "torso", null)));
        scenario("flip-branch-h", (d, e, p) -> d.apply(FigureOps.flipBranch(p, man(p), "armR1", true, 0.6)));
        scenario("flip-branch-v", (d, e, p) -> d.apply(FigureOps.flipBranch(p, man(p), "legL1", false, 0)));
        scenario("flip-figure", (d, e, p) -> d.apply(FigureOps.flipFigure(man(p), true)));
        scenario("modify-thickness", (d, e, p) -> d.apply(FigureOps.modifyBranch(man(p), "torso", "Thickness", FigureOps.setThickness(7f))));
        scenario("modify-color", (d, e, p) -> d.apply(FigureOps.modifyBranch(man(p), "armL1", "Colour", FigureOps.setColor(app.barkfart.model.Color.rgba8888(0x336699ff)))));
        scenario("modify-shape", (d, e, p) -> d.apply(FigureOps.modifyBranch(man(p), "legR1", "Shape", FigureOps.setShape(app.barkfart.model.Node.Shape.TRIANGLE))));
        scenario("modify-scale", (d, e, p) -> d.apply(FigureOps.modifyBranch(man(p), "hip", "Scale", FigureOps.scaleLength(1.5f))));
        scenario("modify-static", (d, e, p) -> d.apply(FigureOps.modifyBranch(man(p), "armR1", "Static", FigureOps.setStatic(true))));
        scenario("push-back", (d, e, p) -> d.apply(FigureOps.pushBranch(man(p), "armL1", false)));
        scenario("push-forward", (d, e, p) -> d.apply(FigureOps.pushBranch(man(p), "legL1", true)));
        scenario("pin", (d, e, p) -> d.apply(FigureOps.setPinned(man(p), "legR2", true)));
        // structure
        scenario("remove-node", (d, e, p) -> d.apply(StructureEdits.removeNode(man(p), "armL1")));
        scenario("remove-layer", (d, e, p) -> d.apply(StructureEdits.removeLayer(p.root(), p.root().layers.get(2))));
        scenario("move-layer", (d, e, p) -> d.apply(StructureEdits.moveLayer(p.root(), p.root().layers.get(0), 4)));
        scenario("set-parent", (d, e, p) -> d.apply(StructureEdits.setParent(p.root(), p.root().layers.get(3), "L2")));
        scenario("remove-effect", (d, e, p) -> d.apply(StructureEdits.removeEffect(man(p), man(p).effects.nodes.get("fx1"))));
        scenario("clear-effects", (d, e, p) -> d.apply(StructureEdits.clearEffects(man(p))));
        scenario("paste-clip", (d, e, p) -> {
            Layer.AudioLayer a = (Layer.AudioLayer) p.root().layer("Lb");
            d.apply(StructureEdits.pasteClip(p, a, a.clips.get(0), 1.3));
        });
        scenario("canvas-size", (d, e, p) -> d.apply(StructureEdits.setCanvasSize(p, 720, 1280)));
        scenario("parallax", (d, e, p) -> d.apply(StructureEdits.setParallax(p.root().layers.get(3), 7f)));
        scenario("copy-layer", (d, e, p) -> {
            Map<String, String> ids = new LinkedHashMap<String, String>();
            ids.put("L2", p.newId("L"));
            Layer c = app.barkfart.io.ProjectCodec.copyLayer(man(p), ids);
            d.apply(StructureEdits.addLayer(p.root(), c, 1));
        });
    }

    private static Layer L(Project p, String id) {
        return p.root().layer(id);
    }

    private void opsScenarios() throws Exception {
        scenario("join-joint", (d, e, p) -> d.apply(JoinOps.join(p, p.root(), L(p, "L7"), man(p), "armR2", true, 0.6)));
        scenario("join-no-rotate", (d, e, p) -> d.apply(JoinOps.join(p, p.root(), L(p, "L8"), man(p), "head", false, 0.6)));
        scenario("join-figure", (d, e, p) -> d.apply(JoinOps.join(p, p.root(), L(p, "L7"), man(p), null, true, 0)));
        scenario("unjoin", (d, e, p) -> d.apply(JoinOps.unjoin(p, p.root(), L(p, "L5"), 0.3)));
        scenario("flip-joined", (d, e, p) -> {
            d.apply(JoinOps.join(p, p.root(), L(p, "L7"), man(p), "armR2", true, 0.6));
            d.apply(JoinOps.flipJoined(p.root(), man(p), "armR2"));
        });
        scenario("lock-camera", (d, e, p) -> d.apply(CameraOps.lock(p, p.root(), L(p, "L7"), (Layer.CameraLayer) L(p, "Lc"), true, 0.2)));
        scenario("duplicate-joined", (d, e, p) -> d.apply(LayerOps.duplicate(p, p.root(), man(p), true, 30, -5, null)));
        scenario("duplicate-alone", (d, e, p) -> d.apply(LayerOps.duplicate(p, p.root(), L(p, "L5"), false, 0, 12, null)));
        scenario("move-joined-forward", (d, e, p) -> d.apply(LayerOps.move(p, p.root(), man(p), true)));
        scenario("move-joined-back", (d, e, p) -> d.apply(LayerOps.move(p, p.root(), L(p, "L8"), false)));
        scenario("paste-props", (d, e, p) -> {
            LayerOps.Props c = LayerOps.copyProps(p, p.root(), man(p), 0.6);
            LayerOps.pasteProps(e, p.root(), L(p, "L7"), c, true, true, 0.3);
        });
        scenario("paste-props-pose-only", (d, e, p) -> {
            LayerOps.Props c = LayerOps.copyProps(p, p.root(), man(p), 0.2);
            LayerOps.pasteProps(e, p.root(), L(p, "L8"), c, false, false, 0.9);
        });
        scenario("tweening-off", (d, e, p) -> LayerOps.setTweening(e, man(p), false));
        scenario("convert-symbol", (d, e, p) -> d.apply(SymbolOps.convertToSymbol(p, p.root(), Arrays.asList(L(p, "L7"), L(p, "L8")), "clip", null)));
        scenario("break-apart", (d, e, p) -> d.apply(SymbolOps.breakApart(p, p.root(), (Layer.SymbolLayer) L(p, "L5"), 0.4, null)));
        scenario("camera-follow", (d, e, p) -> d.apply(CameraOps.follow(p, p.root(), (Layer.CameraLayer) L(p, "Lc"), man(p), 0, 12)));
        scenario("pin-corner", (d, e, p) -> CameraOps.pinToCorner(e, p.root(), (Layer.TextLayer) L(p, "L8"), (Layer.CameraLayer) L(p, "Lc"), 2, 10, 0));
        scenario("polyfill", (d, e, p) -> {
            PolyfillBuilder b = new PolyfillBuilder(man(p), "torso");
            b.add("armR1");
            b.add("armR1");
            b.add("armR2");
            b.add("head");
            d.apply(b.finish(PolyfillBuilder.freeId(man(p)), app.barkfart.model.Color.rgba8888(0xff000080)));
        });
    }

    @Test
    void dump() throws Exception {
        String d0 = System.getProperty("barkfart.editdump", "");
        Assumptions.assumeTrue(!d0.isEmpty(), "edit dump not requested");
        dir = new File(d0, "editor");
        dir.mkdirs();
        java.io.File editorDir = dir;
        dir = new File(d0, "figure");
        dir.mkdirs();
        figureScenarios();
        dir = new File(d0, "ops");
        dir.mkdirs();
        opsScenarios();
        dir = editorDir;
        scenario("change-autokey", (d, e, p) -> e.change(p.property("L2/armR1/rotation"), 33f, 0.5));
        scenario("change-static", (d, e, p) -> {
            e.autoKey = false;
            e.change(p.property("L2/head/thickness"), 14f, 0.5);
        });
        scenario("change-refused", (d, e, p) -> {
            e.autoKey = false;
            e.change(p.property("L2/armR1/rotation"), 1f, 0.5);
        });
        scenario("change-interp-follows", (d, e, p) -> {
            e.defaultInterp = Interp.HOLD;
            e.change(p.property("L2/legL1/rotation"), -100f, 0.25);
            e.change(p.property("L2/legL1/rotation"), -90f, 0.75);
        });
        scenario("change-replaces-key", (d, e, p) -> {
            e.defaultInterp = Interp.LINEAR;   // the key is HOLD: replacing it must keep HOLD
            Property q = p.property("L2/armR1/rotation");
            e.change(q, 10f, q.track().get(1).time);
        });
        scenario("pose", (d, e, p) -> {
            Map<String, Float> rot = new LinkedHashMap<String, Float>();
            rot.put("armL1", -120f);
            Map<String, Float> len = new LinkedHashMap<String, Float>();
            len.put("head", 44f);
            e.pose((Layer.FigureLayer) p.root().layers.get(0), rot, len, 0.61);
        });
        scenario("retime", (d, e, p) -> e.retime(keys(p, "L2/armR1/rotation").subList(1, 2), 0.2));
        scenario("retime-onto-key", (d, e, p) -> e.retime(keys(p, "L2/armR1/rotation").subList(0, 1), keys(p, "L2/armR1/rotation").get(1).time));
        scenario("scale-time", (d, e, p) -> {
            List<KeyRef> sel = keys(p, "L2/armR1/rotation");
            sel.addAll(keys(p, "L2/x"));
            e.scaleTime(sel, 0.5, 2);
        });
        scenario("stagger", (d, e, p) -> {
            List<List<KeyRef>> chain = new ArrayList<List<KeyRef>>();
            chain.add(keys(p, "L2/x"));
            chain.add(keys(p, "L2/armR1/rotation"));
            chain.add(new ArrayList<KeyRef>());
            chain.add(keys(p, "L2/head/color"));
            e.stagger(chain, 2);
        });
        scenario("delete", (d, e, p) -> {
            List<KeyRef> sel = keys(p, "L2/armR1/rotation").subList(0, 2);
            sel = new ArrayList<KeyRef>(sel);
            sel.addAll(keys(p, "L2/head/color"));
            e.delete(sel);
        });
        scenario("copy-paste", (d, e, p) -> {
            Editor.Clip c = e.copy(keys(p, "L2/armR1/rotation"));
            Map<String, String> map = new LinkedHashMap<String, String>();
            map.put("L2/armR1/rotation", "L2/armL1/rotation");
            e.paste(c, 1.25, map);
        });
        scenario("paste-type-mismatch", (d, e, p) -> {
            Editor.Clip c = e.copy(keys(p, "L2/armR1/rotation"));
            Map<String, String> map = new LinkedHashMap<String, String>();
            map.put("L2/armR1/rotation", "L2/head/color");
            e.paste(c, 0, map);
        });
        scenario("easing", (d, e, p) -> {
            List<KeyRef> sel = keys(p, "L2/armR1/rotation");
            sel.addAll(keys(p, "L2/shown"));
            e.setInterp(sel, Interp.BEZIER, Ease.BOUNCE_OUT);
        });
    }
}
