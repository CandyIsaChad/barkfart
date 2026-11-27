//! Wave 1 timeline, keyframe, live drag, batch selection, and layer row ops.

use barkfart_core::codec;
use barkfart_core::edit::block_ops::pose_frames;
use barkfart_core::edit::Document;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::json;

fn canonical(s: &Session) -> String {
    codec::encode(s.project())
        .lines()
        .filter(|l| !l.contains("idCounter"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn l2_frames(s: &Session) -> Vec<i32> {
    let p = s.project();
    pose_frames(p.root().unwrap().layer("L2").unwrap(), p.settings.fps as f64)
}

fn arm_angle(s: &Session, frame: i32) -> f32 {
    let p = s.project();
    let q = &p.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation;
    q.key_value(frame as f64 / p.settings.fps as f64).f()
}

fn three_distinct_poses() -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "node", "angle": 10.0}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "node", "angle": 50.0}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "node", "angle": 90.0}));
    s
}

#[test]
fn last_block_shortens_and_respects_other_layers() {
    let mut s = three_distinct_poses();
    assert_eq!(s.frame_count(), 3);
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "block_length", "layer": "L2", "frames": 4}));
    assert_eq!(s.frame_count(), 6);
    let before_shrink = canonical(&s);
    s.command(&json!({"op": "block_length", "layer": "L2", "frames": 2}));
    assert_eq!(s.frame_count(), 4);
    let after_shrink = canonical(&s);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before_shrink);
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after_shrink);

    // Another figure with a pose at frame 4 stops L2 from shrinking past frame 5
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "block_length", "layer": "L2", "frames": 5})); // 7 frames total
    assert_eq!(s.frame_count(), 7);
    s.command(&json!({"op": "frame", "i": 4}));
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    s.command(&json!({"op": "new_pose", "layer": ball, "frame": 4}));
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "block_length", "layer": "L2", "frames": 1}));
    assert_eq!(s.frame_count(), 5, "cannot chop off ball's pose on frame 4");
}

#[test]
fn live_timeline_drag_updates_stage_in_flight_commits_once_and_cancels_cleanly() {
    let mut s = three_distinct_poses();
    // Grow pose 2 -> frames 0, 1, 2 (total 5)
    s.command(&json!({"op": "frame", "i": 0}));
    let orig = canonical(&s);
    let undo0 = s.state()["undo"].clone();

    // Zero-delta drag leaves zero undo steps
    s.command(&json!({"op": "timeline_drag_start", "kind": "block_end", "layer": "L2", "frame": 0}));
    s.command(&json!({"op": "timeline_drag_move", "frames": 1}));
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], undo0);
    assert_eq!(canonical(&s), orig);

    // Multi-step live drag previews immediately on every move, commits as 1 undo step
    s.command(&json!({"op": "timeline_drag_start", "kind": "block_end", "layer": "L2", "frame": 0}));
    let mid1 = s.command(&json!({"op": "timeline_drag_move", "frames": 3}));
    assert_eq!(mid1["frames"], 5, "live preview grew immediately");
    assert_eq!(l2_frames(&s), vec![0, 3, 4]);
    let mid2 = s.command(&json!({"op": "timeline_drag_move", "frames": 2}));
    assert_eq!(mid2["frames"], 4, "live preview shrank immediately without drift");
    assert_eq!(l2_frames(&s), vec![0, 2, 3]);
    let done = s.command(&json!({"op": "up"}));
    assert_eq!(done["undo"], "Pose length");
    let after = canonical(&s);
    // Single undo returns to exact pre-drag project
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), orig);
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after);

    // Cancel rolls back live changes and restores original frame
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "timeline_drag_start", "kind": "pose_key", "layer": "L2", "frame": 2}));
    // Cross over occupied frame 3 -> silently hold previous valid state (no toast spam), then succeed on 5
    let over = s.command(&json!({"op": "timeline_drag_move", "to": 3}));
    assert_eq!(over["message"], "", "no mid-drag toast spam over occupied frames");
    let across = s.command(&json!({"op": "timeline_drag_move", "to": 5}));
    assert_eq!(across["frames"], 6);
    assert_eq!(l2_frames(&s), vec![0, 3, 5]);
    assert_eq!(s.frame, 5);
    s.command(&json!({"op": "cancel"}));
    assert_eq!(canonical(&s), after);
    assert_eq!(s.frame, 2);
}

#[test]
fn free_pose_key_move_duplicate_and_copy_paste_with_transform() {
    let mut s = three_distinct_poses();
    // Move pose 1 (angle 50) across pose 2 (angle 90) to frame 4
    let before = canonical(&s);
    let st = s.command(&json!({"op": "move_pose_key", "layer": "L2", "from": 1, "to": 4}));
    assert_eq!(st["undo"], "Move pose");
    assert_eq!(l2_frames(&s), vec![0, 2, 4]);
    assert_eq!(s.frame_count(), 5);
    assert!((arm_angle(&s, 2) - 90.0).abs() < 0.01);
    assert!((arm_angle(&s, 4) - 50.0).abs() < 0.01);
    let after = canonical(&s);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before);
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after);

    // Duplicate pose 2 -> lands on first free frame (3)
    let st = s.command(&json!({"op": "duplicate_pose_key", "layer": "L2", "from": 2}));
    assert_eq!(st["undo"], "Duplicate pose");
    assert_eq!(l2_frames(&s), vec![0, 2, 3, 4]);
    assert!((arm_angle(&s, 3) - 90.0).abs() < 0.01);

    // Copy pose with transform & paste onto frame 5
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "nudge", "dx": 85.0, "dy": -35.0}));
    let x2 = s.project().root().unwrap().layer("L2").unwrap().transform.x.key_value(0.2).f();
    s.command(&json!({"op": "copy_pose", "layer": "L2", "frame": 2, "withTransform": true}));
    let st = s.command(&json!({"op": "paste_pose", "layer": "L2", "frame": 5}));
    assert_eq!(st["undo"], "Paste pose");
    assert_eq!(s.frame_count(), 6);
    let x5 = s.project().root().unwrap().layer("L2").unwrap().transform.x.key_value(0.5).f();
    assert!((x5 - x2).abs() < 0.1, "transform copied: {x5} vs {x2}");
    assert!((arm_angle(&s, 5) - 90.0).abs() < 0.01);

    // blockEases reflects easing while blocks stays 3 elements
    let st = s.command(&json!({"op": "easing", "layer": "L2", "frame": 2, "v": "bounce"}));
    let l2 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    assert_eq!(l2["blocks"][1].as_array().unwrap().len(), 3);
    assert_eq!(l2["blockEases"][1], "bounce");
}

#[test]
fn property_key_move_copy_paste_duplicate_ease_and_delete() {
    let mut s = three_distinct_poses();
    s.command(&json!({"op": "set", "k": "keying", "v": true}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "filter", "glowAmount": 0.2}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "filter", "glowAmount": 0.8}));
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    let prop = row["props"].as_array().unwrap().iter().find(|p| p["name"] == "Glow amount").unwrap();
    let pid = prop["id"].as_str().unwrap().to_string();
    assert_eq!(prop["keys"], json!([0, 1]));
    assert_eq!(prop["eases"].as_array().unwrap().len(), 2);

    // Move property key 1 -> 3 (grows duration)
    let before = canonical(&s);
    let st = s.command(&json!({"op": "move_prop_key", "property": pid, "from": 1, "to": 3}));
    assert_eq!(st["undo"], "Move key");
    assert_eq!(st["frames"], 4);
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    let prop = row["props"].as_array().unwrap().iter().find(|p| p["id"] == pid).unwrap();
    assert_eq!(prop["keys"], json!([0, 3]));
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before);
    s.command(&json!({"op": "redo"}));

    // Duplicate property key 3 -> lands on 4, equal value stays visible in keys!
    let st = s.command(&json!({"op": "duplicate_prop_key", "property": pid, "from": 3}));
    assert_eq!(st["undo"], "Duplicate key");
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    let prop = row["props"].as_array().unwrap().iter().find(|p| p["id"] == pid).unwrap();
    assert_eq!(prop["keys"], json!([0, 3, 4]));

    // Copy & paste property key onto frame 2
    let st = s.command(&json!({"op": "copy_prop_key", "property": pid, "frame": 3}));
    assert_eq!(st["propKeyCopied"], true);
    let st = s.command(&json!({"op": "paste_prop_key", "property": pid, "frame": 2}));
    assert_eq!(st["undo"], "Paste key");
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    let prop = row["props"].as_array().unwrap().iter().find(|p| p["id"] == pid).unwrap();
    assert_eq!(prop["keys"], json!([0, 2, 3, 4]));

    // Ease single property key
    let st = s.command(&json!({"op": "easing", "property": pid, "frame": 2, "v": "hold"}));
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    let prop = row["props"].as_array().unwrap().iter().find(|p| p["id"] == pid).unwrap();
    assert_eq!(prop["eases"][1], "hold");

    // Delete down to 1 key -> becomes static at the surviving value (0.2)
    s.command(&json!({"op": "delete_prop_key", "property": pid, "frame": 4}));
    s.command(&json!({"op": "delete_prop_key", "property": pid, "frame": 3}));
    let st = s.command(&json!({"op": "delete_prop_key", "property": pid, "frame": 2}));
    assert_eq!(st["undo"], "Delete key");
    let q = s.project().property(&pid).unwrap();
    assert!(q.track.is_empty());
    assert!((q.value.f() - 0.2).abs() < 1e-4);
}

#[test]
fn batch_key_ops_shift_ease_and_delete_atomically() {
    let mut s = three_distinct_poses();
    // Shift poses 1 and 2 by +2 -> frames [0, 3, 4] (frame 0 skipped if included)
    let before = canonical(&s);
    let st = s.command(&json!({"op": "shift_keys", "poses": [["L2", 0], ["L2", 1], ["L2", 2]], "props": [], "by": 2}));
    assert_eq!(st["undo"], "Shift keys");
    assert_eq!(l2_frames(&s), vec![0, 3, 4]);
    assert_eq!(s.frame_count(), 5);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before);
    s.command(&json!({"op": "redo"}));
    assert_eq!(l2_frames(&s), vec![0, 3, 4]);

    // Ease batch
    let st = s.command(&json!({"op": "ease_keys", "poses": [["L2", 0], ["L2", 3]], "props": [], "v": "hold"}));
    let l2 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    assert_eq!(l2["blockEases"][0], "hold");
    assert_eq!(l2["blockEases"][1], "hold");

    // Delete batch (frame 0 protected!)
    let st = s.command(&json!({"op": "delete_keys", "poses": [["L2", 0], ["L2", 3], ["L2", 4]], "props": []}));
    assert_eq!(st["undo"], "Delete keys");
    assert_eq!(l2_frames(&s), vec![0]);
}

#[test]
fn layer_select_visibility_lock_and_duplicate_copy_naming() {
    let mut s = three_distinct_poses();
    // Duplicate figure gets "(copy)", then "(copy 2)", and offset (60, -20)
    let x0 = s.project().root().unwrap().layer("L2").unwrap().transform.x.key_value(0.0).f();
    let st1 = s.command(&json!({"op": "duplicate_figure", "layer": "L2"}));
    let id1 = st1["selected"]["layer"].as_str().unwrap().to_string();
    assert_eq!(st1["selected"]["name"], "Figure 1 (copy)");
    let x1 = s.project().root().unwrap().layer(&id1).unwrap().transform.x.key_value(0.0).f();
    assert!((x1 - x0 - 60.0).abs() < 0.1, "{x1} vs {x0}");

    let st2 = s.command(&json!({"op": "duplicate_figure", "layer": "L2"}));
    assert_eq!(st2["selected"]["name"], "Figure 1 (copy 2)");

    // Layer lock & visibility on explicit layer + undo
    let before = canonical(&s);
    let st = s.command(&json!({"op": "layer_visible", "layer": id1, "v": false}));
    assert_eq!(st["undo"], "Hide layer");
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == id1).unwrap();
    assert_eq!(row["visible"], false);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before);

    let st = s.command(&json!({"op": "layer_lock", "layer": id1, "v": true}));
    assert_eq!(st["undo"], "Lock layer");
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == id1).unwrap();
    assert_eq!(row["locked"], true);

    // Selecting text, camera, and back to figure works by layer id alone
    let st = s.command(&json!({"op": "add_textfield"}));
    let tid = st["textfield"]["layer"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": "L2"}));
    assert!(s.state()["textfield"].is_null());
    let st = s.command(&json!({"op": "select", "layer": tid}));
    assert_eq!(st["textfield"]["layer"], tid);

    let st = s.command(&json!({"op": "add_camera"}));
    let cid = st["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "camera").unwrap()["id"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "select", "layer": cid}));
    assert_eq!(st["mode"], "camera");
    let st = s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(st["mode"], "figure");
    assert_eq!(st["selected"]["layer"], "L2");

    // Delete explicit layer by id
    let st = s.command(&json!({"op": "delete_everywhere", "layer": id1}));
    assert!(st["rows"].as_array().unwrap().iter().all(|r| r["id"] != id1));
}

#[test]
fn group_blend_saves_evaluates_and_undoes_cleanly() {
    use barkfart_core::{eval, model::Blend};
    let mut s = three_distinct_poses();
    let st = s.command(&json!({"op": "group", "layers": ["L2"]}));
    let gid = st["group"]["id"].as_str().unwrap().to_string();
    assert_eq!(st["group"]["blend"], "NORMAL");

    let before = canonical(&s);
    let st = s.command(&json!({"op": "group_set", "group": gid, "blend": "MULTIPLY"}));
    assert_eq!(st["undo"], "Group blend");
    assert_eq!(st["group"]["blend"], "MULTIPLY");

    // Evaluated scene carries Blend::Multiply on the group node
    let scene = eval::evaluate(s.project(), &s.project().root_symbol, 0.0);
    let grp = scene.root.children.iter().find_map(|i| match i {
        eval::Item::Group(g) if g.source.ends_with(&gid) => Some(g),
        _ => None,
    }).expect("evaluated group");
    assert_eq!(grp.blend, Blend::Multiply);

    // Save / load round-trips blend
    let encoded = codec::encode(s.project());
    let decoded = codec::decode(&encoded).unwrap();
    assert_eq!(decoded.root().unwrap().groups[0].blend, Blend::Multiply);

    // Undo / redo exact
    let after = canonical(&s);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before);
    assert_eq!(s.state()["group"]["blend"], "NORMAL");
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after);
}

#[test]
fn reverse_range_and_ping_pong_range_mirror_poses_and_properties() {
    let mut s = three_distinct_poses(); // frames 0: 10, 1: 50, 2: 90
    let orig = canonical(&s);
    let st = s.command(&json!({"op": "reverse_range", "from": 0, "to": 2}));
    assert_eq!(st["undo"], "Reverse range");
    assert!((arm_angle(&s, 0) - 90.0).abs() < 0.01);
    assert!((arm_angle(&s, 1) - 50.0).abs() < 0.01);
    assert!((arm_angle(&s, 2) - 10.0).abs() < 0.01);

    // Reversing a second time restores original poses
    s.command(&json!({"op": "reverse_range", "from": 0, "to": 2}));
    assert!((arm_angle(&s, 0) - 10.0).abs() < 0.01);
    assert!((arm_angle(&s, 1) - 50.0).abs() < 0.01);
    assert!((arm_angle(&s, 2) - 90.0).abs() < 0.01);

    // Undo twice returns to exact original project
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), orig);

    // Ping-pong [0..=2] -> [0:10, 1:50, 2:90, 3:50, 4:10] (5 frames)
    let st = s.command(&json!({"op": "ping_pong_range", "from": 0, "to": 2}));
    assert_eq!(st["undo"], "Ping-pong range");
    assert_eq!(st["frames"], 5);
    assert!((arm_angle(&s, 0) - 10.0).abs() < 0.01);
    assert!((arm_angle(&s, 1) - 50.0).abs() < 0.01);
    assert!((arm_angle(&s, 2) - 90.0).abs() < 0.01);
    assert!((arm_angle(&s, 3) - 50.0).abs() < 0.01);
    assert!((arm_angle(&s, 4) - 10.0).abs() < 0.01);

    let after_pp = canonical(&s);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), orig);
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after_pp);
}

#[test]
fn extend_default_figure_then_add_or_import_new_figure_and_extend_shorten_move() {
    let mut s = Session::new(Document::new(new_editor_project()));
    // Fresh unposed figure immediately reports a 1-frame visible block and selected.pose!
    let init_st = s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(init_st["selected"]["pose"]["start"], 0);
    assert_eq!(init_st["selected"]["pose"]["length"], 1);
    let init_row = init_st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap();
    assert_eq!(init_row["blocks"], json!([[0, 1, false]]));

    // 1. Extend unposed default stick figure from 1 frame to 8 frames
    let st = s.command(&json!({"op": "block_length", "layer": "L2", "frame": 0, "frames": 8}));
    assert_eq!(st["frames"], 8);

    // 2. Add a second stick figure on frame 0
    let st = s.command(&json!({"op": "add_figure", "kind": "stickman"}));
    let fig2 = st["selected"]["layer"].as_str().unwrap().to_string();
    assert_eq!(st["selected"]["pose"]["start"], 0);
    assert_eq!(st["selected"]["pose"]["length"], 1, "newly added figure starts as 1-frame visible block");
    let row2 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(row2["blocks"], json!([[0, 1, false]]));
    assert_eq!(row2["on"], json!([true, false, false, false, false, false, false, false]));

    // 3. Live-drag right grip to extend fig2 into existing timeline (5 frames), past end (10 frames), then shorten to 6 frames
    let before_drag = canonical(&s);
    s.command(&json!({"op": "timeline_drag_start", "kind": "block_end", "layer": fig2, "frame": 0}));
    let mid5 = s.command(&json!({"op": "timeline_drag_move", "frames": 5}));
    assert_eq!(mid5["frames"], 8, "default figure still holds 8 frames");
    let r5 = mid5["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(r5["blocks"], json!([[0, 5, false]]));
    assert_eq!(r5["on"], json!([true, true, true, true, true, false, false, false]));

    let mid10 = s.command(&json!({"op": "timeline_drag_move", "frames": 10}));
    assert_eq!(mid10["frames"], 10);
    let r10 = mid10["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(r10["blocks"], json!([[0, 10, false]]));

    let mid6 = s.command(&json!({"op": "timeline_drag_move", "frames": 6}));
    assert_eq!(mid6["frames"], 8, "shrunk back to L2's 8 frames without chopping L2");
    let done = s.command(&json!({"op": "up"}));
    assert_eq!(done["undo"], "Pose length");
    let r6 = done["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(r6["blocks"], json!([[0, 6, false]]));
    assert_eq!(r6["on"], json!([true, true, true, true, true, true, false, false]));

    // Exact 1-step undo and redo
    let after_drag = canonical(&s);
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before_drag);
    s.command(&json!({"op": "redo"}));
    assert_eq!(canonical(&s), after_drag);

    // 4. Hold longer grows visible length by +1 (6 -> 7), and extend_to extends onto empty/runway cells
    let st = s.command(&json!({"op": "hold_longer", "layer": fig2, "frame": 0}));
    let r7 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(r7["blocks"], json!([[0, 7, false]]));

    let before_ext_to = canonical(&s);
    let st = s.command(&json!({"op": "extend_to", "layer": fig2, "frame": 9}));
    assert_eq!(st["undo"], "Extend layer");
    assert_eq!(st["frames"], 10);
    assert_eq!(st["frame"], 9);
    let r_ext = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig2).unwrap();
    assert_eq!(r_ext["blocks"], json!([[0, 10, false]]));
    s.command(&json!({"op": "undo"}));
    assert_eq!(canonical(&s), before_ext_to);

    // Textfield added mid-timeline also emits a single extendable/movable block
    s.command(&json!({"op": "frame", "i": 2}));
    let st_txt = s.command(&json!({"op": "add_textfield"}));
    let tid = st_txt["textfield"]["layer"].as_str().unwrap().to_string();
    let r_txt = st_txt["rows"].as_array().unwrap().iter().find(|r| r["id"] == tid).unwrap();
    assert_eq!(r_txt["blocks"], json!([[2, 1, false]]));
    let st_txt = s.command(&json!({"op": "extend_to", "layer": tid, "frame": 5}));
    let r_txt = st_txt["rows"].as_array().unwrap().iter().find(|r| r["id"] == tid).unwrap();
    assert_eq!(r_txt["blocks"], json!([[2, 4, false]]));

    // 5. Import a stock .nodes figure mid-timeline (frame 3), extend, move, and add pose on runway cell
    let stock_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Stickfigure.nodes");
    if let Ok(bytes) = std::fs::read(stock_path) {
        s.command(&json!({"op": "frame", "i": 3}));
        let st = s.import("Stickfigure.nodes", &bytes);
        let fig3 = st["selected"]["layer"].as_str().unwrap().to_string();
        assert_eq!(st["selected"]["pose"]["start"], 3);
        assert_eq!(st["selected"]["pose"]["length"], 1);
        let r3 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig3).unwrap();
        assert_eq!(r3["blocks"], json!([[3, 1, false]]));

        // Extend mid-timeline imported figure to 3 frames (frames 3, 4, 5)
        let before_ext3 = canonical(&s);
        let st = s.command(&json!({"op": "block_length", "layer": fig3, "frame": 3, "frames": 3}));
        let r3 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig3).unwrap();
        assert_eq!(r3["blocks"], json!([[3, 3, false]]));
        assert_eq!(r3["on"], json!([false, false, false, true, true, true, false, false]));
        s.command(&json!({"op": "undo"}));
        assert_eq!(canonical(&s), before_ext3);
        s.command(&json!({"op": "redo"}));

        // Move mid-timeline block from frame 3 to frame 1
        let st = s.command(&json!({"op": "move_pose_key", "layer": fig3, "from": 3, "to": 1}));
        let r3 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig3).unwrap();
        assert_eq!(r3["blocks"], json!([[1, 3, false]]));
        assert_eq!(r3["on"], json!([false, true, true, true, false, false, false, false]));

        // Add new pose on a runway cell (frame 10 > current 8 frames)
        let st = s.command(&json!({"op": "new_pose", "layer": fig3, "frame": 10}));
        assert_eq!(st["frames"], 11);
        let r3 = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig3).unwrap();
        assert_eq!(r3["blocks"], json!([[1, 3, false], [10, 1, false]]));
    }
}

#[test]
fn gradient_ball_and_legacy_gradient_modes_evaluate_and_tessellate_smoothly() {
    use barkfart_core::{eval, figures, model::Shape, tess};
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Gradient Ball.nodes");
    let bytes = std::fs::read(path).expect("Gradient Ball.nodes");

    // Preview project (figure picker) and Session::import both render smooth radial pink->white
    let prev = figures::preview_file("Gradient Ball.nodes", &bytes).expect("preview_file");
    let scene = eval::evaluate(&prev, &prev.root_symbol, 0.0);
    let dl = tess::tessellate(&scene, None);
    assert!(dl.triangle_count() > 100);

    // Ball center in stage coords (node n12 circle center ~ (7.9, -54.1), clear_spot keeps x=0 on empty preview)
    let (cx, cy) = (7.9f32, -54.1f32);
    let mut inner_pink = 0usize;
    let mut outer_white = 0usize;
    for b in &dl.batches {
        for i in 0..b.vertex_count() {
            let (x, y) = (b.xy[i * 2], b.xy[i * 2 + 1]);
            let d = (x - cx).hypot(y - cy);
            let rgba = b.rgba[i];
            let (r, g, bl) = ((rgba >> 24) & 0xff, (rgba >> 16) & 0xff, (rgba >> 8) & 0xff);
            if d < 56.0 {
                // Every inner vertex (center disc + inner ends of both SquareLine & Triangle spokes) is pink!
                assert!(r > 230 && g < 110, "inner vertex at r={d:.1} was not pink: rgba=({r},{g},{bl})");
                inner_pink += 1;
            } else if d > 315.0 {
                // Every outer rim vertex of both SquareLine & Triangle spokes is white!
                assert!(r > 245 && g > 245 && bl > 245, "outer vertex at r={d:.1} was not white: rgba=({r},{g},{bl})");
                outer_white += 1;
            }
        }
    }
    assert!(inner_pink >= 50 && outer_white >= 100, "inner={inner_pink}, outer={outer_white}");

    // Toggling gradientReverse on an imported SquareLine spoke flips cleanly and resets to defaults
    let mut s = Session::new(Document::new(new_editor_project()));
    let st = s.import("Gradient Ball.nodes", &bytes);
    let lid = st["selected"]["layer"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "select", "layer": lid, "node": "n81"}));
    assert_eq!(st["selected"]["shape"], "square");
    assert_eq!(st["selected"]["gradientReverse"], false);
    assert_eq!(st["selected"]["defaults"]["gradientReverse"], false);
    let before = canonical(&s);
    let st = s.command(&json!({"op": "segment", "gradientReverse": true}));
    assert_eq!(st["selected"]["gradientReverse"], true);
    let st = s.command(&json!({"op": "reset_segment"}));
    assert_eq!(st["selected"]["gradientReverse"], false);
    assert_eq!(canonical(&s), before);

    // Zero-length circle with legacy L.gradientMode = i:1 (vertical linear, like Snowman Head)
    let mut p = new_editor_project();
    {
        let fig = p.root_mut().unwrap().layers[0].clone();
        let mut layer = (*fig).clone();
        let barkfart_core::model::LayerKind::Figure(f) = &mut layer.kind else { unreachable!() };
        let head = f.node_mut("head").unwrap();
        head.shape = Shape::Circle;
        head.length.value = barkfart_core::model::Value::F(0.0);
        head.thickness.value = barkfart_core::model::Value::F(200.0);
        head.gradient = true;
        head.color.value = barkfart_core::model::Value::C(barkfart_core::model::Color::rgba8888(0xff0000ff));
        head.gradient_color.value = barkfart_core::model::Value::C(barkfart_core::model::Color::rgba8888(0x0000ffff));
        head.legacy_extras.insert("L.type".into(), "i:2".into());
        head.legacy_extras.insert("L.gradientMode".into(), "i:1".into());
        p.root_mut().unwrap().layers[0] = std::sync::Arc::new(layer);
    }
    let sc = eval::evaluate(&p, &p.root_symbol, 0.0);
    let dl2 = tess::tessellate(&sc, None);
    let has_red = dl2.batches.iter().any(|b| b.rgba.iter().any(|&c| ((c >> 24) & 0xff) > 220 && ((c >> 8) & 0xff) < 40));
    let has_blue = dl2.batches.iter().any(|b| b.rgba.iter().any(|&c| ((c >> 8) & 0xff) > 220 && ((c >> 24) & 0xff) < 40));
    assert!(has_red && has_blue, "zero-length linear gradient circle spans red to blue");
}
