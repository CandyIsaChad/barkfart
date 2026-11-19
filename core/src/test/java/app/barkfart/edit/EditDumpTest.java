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

    @Test
    void dump() throws Exception {
        String d0 = System.getProperty("barkfart.editdump", "");
        Assumptions.assumeTrue(!d0.isEmpty(), "edit dump not requested");
        dir = new File(d0, "editor");
        dir.mkdirs();
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
