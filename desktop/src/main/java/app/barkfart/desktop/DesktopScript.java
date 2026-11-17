package app.barkfart.desktop;

import app.barkfart.BarkfartApp;
import app.barkfart.edit.DopeRows;
import app.barkfart.files.Importer;
import app.barkfart.fx.EffectScene;
import app.barkfart.edit.StructureEdits;
import app.barkfart.model.Asset;
import app.barkfart.model.Layer;
import app.barkfart.model.Node;
import app.barkfart.ui.EasingMenu;
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
                Layer.CameraLayer cam = new Layer.CameraLayer(st.project().newId("L"), "Camera");
                cam.zoom.setValue(1.6f);
                cam.rotation.setValue(8f);
                cam.x.setValue(60f);
                cam.y.setValue(40f);
                st.doc.apply(StructureEdits.addLayer(st.project().root(), cam, st.project().root().layers.size()));
                st.fire();
            } else if (step.equals("fxscene")) {
                // the effect check scene, three times larger, added with this project's ids
                EffectScene.addTo(st.project(), st.project().root(), 3);
                st.fire();
            } else if (step.equals("noonion")) st.onion = false;
            else if (step.equals("deselect")) st.select(null, null);
            else if (step.equals("easing")) {
                EasingMenu.Preset cur = EasingMenu.PRESETS[2];
                st.easingOpener.open(200, 300, cur, null);
            } else if (step.equals("importall")) {
                java.io.File docs = new java.io.File(System.getProperty("barkfart.docs"));
                for (java.io.File fl : docs.listFiles()) {
                    try {
                        if (fl.getName().endsWith(".nodes") || fl.getName().endsWith(".nodemc")) {
                            Importer.importFile(st.doc, fl);
                        }
                    } catch (Exception e) {
                        System.out.println("[script] " + e);
                    }
                }
                st.fire();
            } else if (step.equals("sprite")) {
                com.badlogic.gdx.graphics.Pixmap pm = new com.badlogic.gdx.graphics.Pixmap(64, 64, com.badlogic.gdx.graphics.Pixmap.Format.RGBA8888);
                for (int y = 0; y < 64; y++) for (int x = 0; x < 64; x++) {
                    boolean c = ((x / 8) + (y / 8)) % 2 == 0;
                    pm.drawPixel(x, y, c ? 0xff9f0aff : (y < 32 ? 0x0a84ffff : 0x30d158ff));
                }
                java.io.ByteArrayOutputStream bo = new java.io.ByteArrayOutputStream();
                try {
                    com.badlogic.gdx.graphics.PixmapIO.PNG png = new com.badlogic.gdx.graphics.PixmapIO.PNG();
                    png.setFlipY(false);   // the encoder flips by default (meant for screenshots)
                    png.write(bo, pm);
                } catch (java.io.IOException e) {
                    throw new RuntimeException(e);
                }
                pm.dispose();
                String id = st.project().newId("A");
                st.project().assets.put(id, new Asset(id, Asset.Kind.IMAGE, "assets/" + id + ".png"));
                st.project().assetData.put(id, bo.toByteArray());
                Layer.SpriteLayer sp = new Layer.SpriteLayer(st.project().newId("L"), "Checker");
                sp.asset = id;
                sp.width = 160;
                sp.height = 160;
                sp.transform.x.setValue(-420f);
                sp.transform.y.setValue(-100f);
                sp.transform.rotation.setValue(15f);
                st.doc.apply(StructureEdits.addLayer(st.project().root(), sp, 0));
            } else if (step.equals("files")) {
                st.fileOpener.run();
            } else if (step.equals("preview")) {
                st.previewCamera = true;
                st.fire();
            } else if (step.equals("expand")) {
                st.sheetHeight = 200;
                st.fire();
            } else if (step.equals("keys") && f != null) {
                st.selectKeys(DopeRows.keysIn(DopeRows.build(st.project(), f, null).subList(4, 6), 0.15, 0.45), false);
            }
        }
    }
}
