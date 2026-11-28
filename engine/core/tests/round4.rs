//! Round 4 regression & undo tests covering:
//! 1. Whole-figure Blend Modes (all 13 W3C/Pro modes + opacity preservation + zero per-node overlap)
//! 2. Camera layer overhaul (sparse keyframes, no forced per-frame baking, full keyframe CRUD, global Widescreen)
//! 3. Modifier Scripts (stock Luau modifiers + Camera Shake on Camera X/Y + rich `modifierTargets`)
//! 4. Figure Creator (`create_figure`) & Isolated Base-Pose Edit (`build` / `build_apply` / `build_cancel` / `edit_copy`)

use barkfart_core::codec;
use barkfart_core::edit::Document;
use barkfart_core::eval::{evaluate, Resolver};
use barkfart_core::model::*;
use barkfart_core::plugins;
use barkfart_core::session::Session;
use barkfart_core::tess;
use serde_json::json;

fn saved_text(p: &Project) -> String {
    codec::encode(p)
        .lines()
        .filter(|l| !l.contains("idCounter"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fresh_session() -> Session {
    let mut s = Session::new(Document::new(barkfart_core::session::new_project()));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s
}

fn find_layer_id(s: &Session, pred: impl Fn(&Layer) -> bool) -> String {
    s.project()
        .root()
        .expect("root")
        .layers
        .iter()
        .find(|l| pred(l))
        .expect("layer")
        .id
        .clone()
}

#[test]
fn blend_13_modes_codec_catalog_and_whole_figure_isolation() {
    assert_eq!(Blend::ALL.len(), 13);
    // Every blend mode round-trips through name, from_name, and catalog JSON.
    let catalog_val = Blend::catalog_json();
    let catalog = catalog_val.as_array().expect("catalog array");
    assert_eq!(catalog.len(), 13);
    for (idx, b) in Blend::ALL.iter().enumerate() {
        assert_eq!(b.index(), idx as u32);
        assert_eq!(Blend::from_name(b.name()), Some(*b));
        assert_eq!(Blend::from_name(&b.name().to_lowercase()), Some(*b));
        assert_eq!(catalog[idx]["id"], b.name());
        assert_eq!(catalog[idx]["index"], idx);
    }

    let mut s = fresh_session();
    let fig_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Figure(_)));
    s.command(&json!({"op": "select", "layer": fig_id, "node": "armR1"}));
    let st = s.state();
    assert_eq!(st["blendCatalog"].as_array().expect("blendCatalog").len(), 13);

    // Every non-Normal blend mode routes the whole figure through a single DrawList::Isolate
    // carrying BOTH blend and opacity, with inner batches using Blend::Normal (zero per-node overlap),
    // at opacity = 1.0, 0.99, and 0.5!
    for &b in &Blend::ALL[1..] {
        s.command(&json!({"op": "blend", "layer": fig_id, "v": b.name()}));
        assert_eq!(s.state()["selected"]["blend"], b.name());
        // Codec round-trip preserves all 13 blend variants
        let encoded = codec::encode(s.project());
        let decoded = codec::decode(&encoded).expect("decode");
        assert_eq!(decoded.root().unwrap().layer(&fig_id).unwrap().blend, b);

        for op in [1.0f32, 0.99, 0.5] {
            s.command(&json!({"op": "filter", "opacity": op}));
            let scene = evaluate(s.project(), &s.project().root_symbol, 0.0);
            let list = tess::layered(&scene, None);
            let mut found_isolate = false;
            for iso in &list.isolates {
                if iso.source == fig_id || iso.source.ends_with(&format!("/{fig_id}")) {
                    assert_eq!(iso.blend, b, "Isolate must carry blend {:?} at opacity {op}", b);
                    assert!((iso.opacity - op).abs() < 0.02, "Isolate must carry opacity {op}, got {}", iso.opacity);
                    // Inner figure batches must draw with Blend::Normal so segments never self-overlap!
                    assert!(!iso.items.is_empty());
                    for sub in &iso.items {
                        if let tess::IsoItem::Batch(bi) = sub {
                            assert_eq!(list.batches[*bi].blend(), Blend::Normal, "inner batch must be Blend::Normal inside Isolate for {:?}", b);
                        }
                    }
                    found_isolate = true;
                }
            }
            assert!(found_isolate, "expected whole-figure Isolate for {:?} at opacity {op}", b);
        }
    }

    // Blend works on textfield and undoes cleanly
    s.command(&json!({"op": "add_textfield"}));
    let txt_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Text(_)));
    let before = codec::encode(s.project());
    s.command(&json!({"op": "blend", "layer": txt_id, "v": "COLOR_DODGE"}));
    assert_eq!(s.state()["textfield"]["blend"], "COLOR_DODGE");
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
}

#[test]
fn camera_sparse_keyframes_crud_and_global_widescreen() {
    let mut s = fresh_session();
    s.command(&json!({"op": "add_camera"}));
    let cam_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Camera(_)));

    // Inserting 5 frames must NOT bake keys onto the Camera layer!
    for _ in 0..5 {
        s.command(&json!({"op": "insert_frame"}));
    }
    let cam_key_count = |s: &Session| -> usize {
        let l = s.project().root().unwrap().layer(&cam_id).unwrap();
        let LayerKind::Camera(c) = &l.kind else { unreachable!() };
        c.zoom.track.len()
    };
    assert_eq!(cam_key_count(&s), 0, "insert_frame must never bake camera keys");

    let cam_has_key = |s: &Session, fr: i64| -> bool {
        s.state()["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == cam_id)
            .and_then(|r| r["blocks"].as_array())
            .is_some_and(|blocks| blocks.iter().any(|b| b[0].as_i64() == Some(fr)))
    };

    // Keying camera on frame 3 anchors frame 0 at default (1.0) and puts a key ONLY on frame 3
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "camera", "zoom": 2.0, "x": 120.0}));
    assert_eq!(cam_key_count(&s), 2, "only frame 0 anchor + frame 3 key");
    assert!(cam_has_key(&s, 0) && cam_has_key(&s, 3));

    // Frame 1 & 2 smoothly interpolate without having keys of their own
    s.command(&json!({"op": "frame", "i": 1}));
    assert!(!cam_has_key(&s, 1));
    let z1 = s.state()["cam"]["zoom"].as_f64().unwrap();
    assert!((z1 - 1.333).abs() < 0.05, "expected smooth tween ~1.333 on frame 1, got {z1}");

    // Inserting a frame after frame 1 shifts frame 3's key to frame 4 and keeps frame 2 sparse
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(cam_key_count(&s), 2, "insert_frame still keeps camera sparse");
    s.command(&json!({"op": "frame", "i": 4}));
    assert!(cam_has_key(&s, 4) && !cam_has_key(&s, 2));
    assert!((s.state()["cam"]["zoom"].as_f64().unwrap() - 2.0).abs() < 0.01);

    // Move camera key from frame 4 -> frame 5, then undo & redo
    let before_move = codec::encode(s.project());
    s.command(&json!({"op": "move_pose_key", "layer": cam_id, "from": 4, "to": 5}));
    s.command(&json!({"op": "frame", "i": 5}));
    assert!(cam_has_key(&s, 5) && !cam_has_key(&s, 4));
    assert!((s.state()["cam"]["zoom"].as_f64().unwrap() - 2.0).abs() < 0.01);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before_move);
    s.command(&json!({"op": "redo"}));

    // Duplicate camera key from frame 5 -> frame 6
    s.command(&json!({"op": "duplicate_pose_key", "layer": cam_id, "from": 5, "to": 6}));
    assert_eq!(cam_key_count(&s), 3);

    // Copy & paste camera pose from frame 5 to frame 2
    s.command(&json!({"op": "frame", "i": 5}));
    s.command(&json!({"op": "select", "layer": cam_id}));
    s.command(&json!({"op": "copy_pose"}));
    assert_eq!(s.state()["cameraPose"], true);
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "paste_pose"}));
    assert_eq!(cam_key_count(&s), 4);
    assert!((s.state()["cam"]["zoom"].as_f64().unwrap() - 2.0).abs() < 0.01);

    // Delete camera key via delete_camera_key and verify clean 1-step undo
    let before_del = codec::encode(s.project());
    s.command(&json!({"op": "delete_camera_key", "frame": 2}));
    assert_eq!(cam_key_count(&s), 3);
    s.command(&json!({"op": "frame", "i": 2}));
    assert!(!cam_has_key(&s, 2));
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before_del);

    // Ease camera keys between frame 0 and 5
    let before_ease = saved_text(s.project());
    s.command(&json!({"op": "ease_keys", "poses": [[cam_id, 0], [cam_id, 5]], "v": "smooth"}));
    assert_ne!(saved_text(s.project()), before_ease);
    s.command(&json!({"op": "undo"}));
    assert_eq!(saved_text(s.project()), before_ease);

    // Global Widescreen: turning widescreen ON affects ALL frames (including newly inserted frames!)
    s.command(&json!({"op": "frame", "i": 2}));
    let before_ws = codec::encode(s.project());
    s.command(&json!({"op": "camera_fx", "widescreen": true}));
    let total = s.state()["frames"].as_i64().unwrap();
    for i in 0..total {
        s.command(&json!({"op": "frame", "i": i}));
        assert_eq!(s.state()["camFx"]["widescreen"], true, "widescreen must be on at frame {i}");
    }
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(s.state()["camFx"]["widescreen"], true, "widescreen persists onto newly inserted frames");
    // Undo back to before widescreen
    s.command(&json!({"op": "undo"})); // undo insert_frame
    s.command(&json!({"op": "undo"})); // undo camera_fx
    assert_eq!(codec::encode(s.project()), before_ws);
}

#[test]
fn stock_luau_modifiers_camera_shake_and_target_picker() {
    let plugin_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/assets/plugins");
    for file in ["spring_overshoot.luau", "stutter_step.luau", "flicker_noise.luau", "camera_shake.luau"] {
        let src = std::fs::read_to_string(plugin_dir.join(file)).expect(file);
        plugins::install(&src, file).unwrap_or_else(|e| panic!("failed to install {file}: {e}"));
    }

    let mut s = fresh_session();
    s.command(&json!({"op": "add_camera"}));
    let fig_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Figure(_)));
    let cam_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Camera(_)));

    // Verify modifierTargets exposes Camera X/Y/Zoom/Rotation at top + figure transforms/joints
    let st = s.state();
    let targets = st["modifierTargets"].as_array().expect("modifierTargets");
    assert!(targets.len() >= 10);
    assert_eq!(targets[0]["layerKind"], "camera");
    assert_eq!(targets[0]["id"], format!("{cam_id}/cam/x"));
    assert_eq!(targets[1]["id"], format!("{cam_id}/cam/y"));
    assert_eq!(targets[2]["id"], format!("{cam_id}/cam/zoom"));
    assert_eq!(targets[3]["id"], format!("{cam_id}/cam/rotation"));
    assert!(targets.iter().any(|t| t["id"] == format!("{fig_id}/x") && t["layerKind"] == "figure"));

    // Animate figure X from 10 -> 210 across frames 0..3
    s.command(&json!({"op": "mode", "v": "all"}));
    for _ in 0..4 {
        s.command(&json!({"op": "insert_frame"}));
    }
    let root_node = s.project().root().unwrap().layer(&fig_id).unwrap().as_figure().unwrap().nodes()[0].id.clone();
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": fig_id, "node": root_node}));
    s.command(&json!({"op": "nudge", "dx": 10.0, "dy": 0.0}));
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "select", "layer": fig_id, "node": root_node}));
    s.command(&json!({"op": "nudge", "dx": 200.0, "dy": 0.0}));
    let before_x = s.project().property(&format!("{fig_id}/x")).unwrap().key_value(0.0).f();
    let end_x = s.project().property(&format!("{fig_id}/x")).unwrap().key_value(3.0 / s.project().settings.fps as f64).f();
    assert!((end_x - before_x).abs() > 100.0, "figure x should animate across frames: start={before_x}, end={end_x}");

    let prop_id = format!("{fig_id}/x");
    let fps = s.project().settings.fps as f64;
    let base_at = |s: &Session, fr: i32| -> f32 {
        let p = s.project();
        let mut r = Resolver::new(p);
        let q = p.property(&prop_id).unwrap();
        r.resolve(q, fr as f64 / fps).f()
    };

    // Test all 3 stock modifier plugins evaluate live and undo cleanly
    for (plugin_id, test_frame) in [
        ("barkfart.spring_overshoot", 4),
        ("barkfart.stutter_step", 1),
        ("barkfart.flicker_noise", 2),
    ] {
        let before = saved_text(s.project());
        let base_val = base_at(&s, test_frame);
        s.command(&json!({"op": "modifier_add", "property": prop_id, "kind": "plugin", "id": plugin_id}));
        if plugin_id == "barkfart.flicker_noise" {
            s.command(&json!({"op": "modifier_set", "property": prop_id, "index": 0, "props": {"amount": 25.0}}));
        }
        let mod_val = base_at(&s, test_frame);
        assert!(
            (mod_val - base_val).abs() > 1e-3,
            "modifier {plugin_id} did not change value at frame {test_frame}: base={base_val}, got={mod_val}"
        );
        // Must not have tripped Luau budget / switch_off
        let p = s.project();
        let q = p.property(&prop_id).unwrap();
        assert!(!q.modifiers.is_empty());
        // Undo back cleanly
        while saved_text(s.project()) != before {
            assert!(s.command(&json!({"op": "undo"}))["canUndo"].is_boolean());
        }
    }

    // Test Camera Shake script targeting Camera X & Y
    let before_shake = saved_text(s.project());
    s.command(&json!({"op": "script_add", "id": "barkfart.camera_shake"}));
    let script_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Script(_)));
    s.command(&json!({
        "op": "script_set",
        "layer": script_id,
        "target": {"name": "x", "to": format!("{cam_id}/cam/x")},
        "props": {"amount": 40.0}
    }));
    s.command(&json!({
        "op": "script_set",
        "layer": script_id,
        "target": {"name": "y", "to": format!("{cam_id}/cam/y")}
    }));
    let p = s.project();
    let mut r = Resolver::new(p);
    let cam_layer = p.root().unwrap().layer(&cam_id).unwrap();
    let LayerKind::Camera(cam_c) = &cam_layer.kind else { unreachable!() };
    let (cx, cy, _, _) = barkfart_core::eval::camera_values(cam_c, 4.0 / fps, &mut r);
    assert!(
        cx.abs() > 0.1 || cy.abs() > 0.1,
        "camera_shake must shake camera X/Y live, got ({cx}, {cy})"
    );
    // Clean undo
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(saved_text(s.project()), before_shake);
}

#[test]
fn figure_creator_and_isolated_rest_pose_edit_preserve_animation_and_undo() {
    let mut s = fresh_session();
    let fig_id = find_layer_id(&s, |l| matches!(l.kind, LayerKind::Figure(_)));

    // 1. Create Stick Figure (`create_figure`) -> isolated canvas, edit, .nodes export, Apply & 1-step Undo
    let before_create = saved_text(s.project());
    let st = s.command(&json!({"op": "create_figure", "name": "Hero"}));
    assert_eq!(st["building"]["mode"], "create");
    assert_eq!(st["copyEdit"]["isNew"], true);
    assert_eq!(st["copyEdit"]["isolated"], true);
    let visible_in_studio = s.project().root().unwrap().layers.iter().filter(|l| l.visible).count();
    assert_eq!(visible_in_studio, 1, "studio hides all other scene layers");
    assert_eq!(s.scene_project().root().unwrap().layers.len(), 1, "main scene unchanged until Apply");

    // Add a segment in the creator studio & verify .nodes export bytes
    s.command(&json!({"op": "add_segment", "len": 50}));
    let (nodes_name, nodes_bytes) = s.figure_file().expect("studio exports valid .nodes file");
    assert!(!nodes_name.is_empty() && !nodes_bytes.is_empty());

    // Cancel discards without touching the scene
    let st_cancel = s.command(&json!({"op": "build_cancel"}));
    assert!(st_cancel["building"].is_null());
    assert_eq!(saved_text(s.project()), before_create);

    // Create again, add a segment, and Apply -> adds new figure in 1 undo step
    s.command(&json!({"op": "create_figure", "name": "Hero"}));
    s.command(&json!({"op": "add_segment", "len": 55}));
    let st_apply = s.command(&json!({"op": "build_apply"}));
    assert!(st_apply["building"].is_null());
    assert_eq!(s.project().root().unwrap().layers.len(), 2, "new figure added to scene");
    s.command(&json!({"op": "undo"}));
    assert_eq!(saved_text(s.project()), before_create, "create_figure + build_apply undoes in 1 step");

    // 2. Isolated Base-Pose Edit (`build`) on a mid-animation posed/translated figure
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": fig_id, "node": "armR1"}));
    s.command(&json!({"op": "nudge", "dx": 220.0, "dy": -110.0}));
    s.command(&json!({"op": "node", "angle": 35.0}));
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": fig_id, "node": "armR1"}));
    s.command(&json!({"op": "node", "angle": 125.0}));

    let fps = s.project().settings.fps as f64;
    let arm_rot_at = |s: &Session, fr: i32| -> f32 {
        let p = s.project();
        let mut r = Resolver::new(p);
        let q = p.property(&format!("{fig_id}/armR1/rotation")).unwrap();
        r.resolve(q, fr as f64 / fps).f()
    };
    let x_at_0 = s.project().property(&format!("{fig_id}/x")).unwrap().key_value(0.0).f();
    let rot_f0 = arm_rot_at(&s, 0);
    let rot_f2 = arm_rot_at(&s, 2);
    assert!((rot_f0 - 35.0).abs() < 0.5 && (rot_f2 - 125.0).abs() < 0.5);

    let before_edit = saved_text(s.project());
    let orig_nodes = s.project().root().unwrap().layer(&fig_id).unwrap().as_figure().unwrap().nodes().len();

    // Enter isolated base-pose edit studio
    let st_build = s.command(&json!({"op": "build", "layer": fig_id}));
    assert_eq!(st_build["building"]["mode"], "edit");
    assert_eq!(s.project().root().unwrap().layers.iter().filter(|l| l.visible).count(), 1);
    // Studio figure is centered at (0, 0) in clean rest pose and all other layers hidden!
    let studio_id = st_build["building"]["layer"].as_str().unwrap().to_string();
    let studio_layer = s.project().root().unwrap().layer(&studio_id).unwrap();
    assert_eq!(studio_layer.transform.x.value.f(), 0.0);
    assert_eq!(studio_layer.transform.y.value.f(), 0.0);

    // Add a new segment (e.g. sword/antenna) on head & recolor head
    s.command(&json!({"op": "select", "layer": studio_id, "node": "head"}));
    s.command(&json!({"op": "segment", "color": 0x00ff88ffu32}));
    s.command(&json!({"op": "add_segment", "len": 42}));

    // Apply back to original figure
    let st_done = s.command(&json!({"op": "build_apply"}));
    assert!(st_done["building"].is_null());
    let new_nodes = s.project().root().unwrap().layer(&fig_id).unwrap().as_figure().unwrap().nodes().len();
    assert_eq!(new_nodes, orig_nodes + 1, "added segment merged into original figure");
    // Live animation pose keyframes on armR1 AND position x must be preserved!
    assert!((arm_rot_at(&s, 0) - rot_f0).abs() < 0.5, "frame 0 pose keyframe preserved");
    assert!((arm_rot_at(&s, 2) - rot_f2).abs() < 0.5, "frame 2 pose keyframe preserved");
    let x_after = s.project().property(&format!("{fig_id}/x")).unwrap().key_value(0.0).f();
    assert!((x_after - x_at_0).abs() < 0.5, "figure position preserved");

    // 1-step Undo reverts the entire studio edit!
    s.command(&json!({"op": "undo"}));
    assert_eq!(saved_text(s.project()), before_edit, "build_apply reverts cleanly in 1 undo step");

    // 3. Save-as-new-figure (`build_apply` with `asNew: true`) leaves original untouched
    let st_b3 = s.command(&json!({"op": "build", "layer": fig_id}));
    let studio_id = st_b3["building"]["layer"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": studio_id, "node": "head"}));
    s.command(&json!({"op": "add_segment", "len": 30}));
    s.command(&json!({"op": "build_apply", "asNew": true, "name": "Hero Variant"}));
    assert_eq!(s.project().root().unwrap().layers.len(), 2);
    assert_eq!(s.project().root().unwrap().layer(&fig_id).unwrap().as_figure().unwrap().nodes().len(), orig_nodes);
    s.command(&json!({"op": "undo"}));
    assert_eq!(saved_text(s.project()), before_edit);
}
