//! The editor session the app drives: gestures, frames, figures, saving.

use barkfart_core::codec;
use barkfart_core::edit::Document;
use barkfart_core::session::{new_project, Session};
use serde_json::{json, Value};

fn session() -> Session {
    Session::new(Document::new(new_project()))
}

fn handle(s: &Value, i: usize) -> (f32, f32) {
    let h = &s["handles"][i];
    (h[0].as_f64().unwrap() as f32, h[1].as_f64().unwrap() as f32)
}

#[test]
fn new_project_matches_java_and_has_one_figure() {
    let java = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/edit/starter/new-project.json")).unwrap();
    assert_eq!(codec::encode(&new_project()), java);
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    assert_eq!(st["frames"], 1);
    assert_eq!(st["figures"].as_array().unwrap().len(), 1);
    assert_eq!(st["handles"].as_array().unwrap().len(), 11);
    // no camera: the export frame is the stage rectangle
    let c: Vec<f64> = st["camera"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(c, vec![-640.0, -360.0, 640.0, -360.0, 640.0, 360.0, -640.0, 360.0]);
}

#[test]
fn a_tap_selects_without_history_and_a_drag_is_one_step() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    // the right hand (armR2) is the 7th node
    let (x, y) = handle(&st, 6);
    let st = s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    assert_eq!(st["hit"], true);
    assert_eq!(st["selected"]["node"], "armR2");
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["canUndo"], false, "a tap leaves no undo step");

    let before = codec::encode(s.project());
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    for k in 1..=5 {
        s.command(&json!({"op": "move", "x": x + 8.0 * k as f32, "y": y - 6.0 * k as f32}));
    }
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["canUndo"], true);
    assert_eq!(st["undo"], "Pose node");
    let (nx, ny) = handle(&st, 6);
    assert!((nx - (x + 40.0)).hypot(ny - (y - 30.0)) > 1.0, "the hand moved");
    assert_ne!(codec::encode(s.project()), before);
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
    assert_eq!(st["canRedo"], true);
}

#[test]
fn digging_steps_through_joints_under_the_finger() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let (x, y) = handle(&st, 0); // hip
    let mut seen = Vec::new();
    for _ in 0..3 {
        let st = s.command(&json!({"op": "down", "x": x, "y": y, "r": 120}));
        seen.push(st["selected"]["node"].as_str().unwrap().to_string());
        s.command(&json!({"op": "up"}));
    }
    assert_eq!(seen[0], "hip");
    assert_ne!(seen[1], seen[0]);
    assert_ne!(seen[2], seen[1]);
}

#[test]
fn frames_figures_and_saving() {
    let mut s = session();
    let st = s.command(&json!({"op": "insert_frame"}));
    assert_eq!(st["frames"], 2);
    assert_eq!(st["frame"], 1);
    let st = s.command(&json!({"op": "add_figure"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
    assert_eq!(st["selected"]["node"], "hip");
    let st = s.command(&json!({"op": "add_segment"}));
    assert_eq!(st["selected"]["root"], false);
    let st = s.command(&json!({"op": "segment", "thickness": 20, "color": 0xff0000ffu32}));
    assert_eq!(st["selected"]["thickness"], 20.0);
    assert_eq!(st["selected"]["color"], 0xff0000ffu32);
    let st = s.command(&json!({"op": "delete_figure"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 1);
    assert!(st["selected"].is_null());
    let st = s.command(&json!({"op": "add_camera"}));
    assert_eq!(st["hasCamera"], true);
    assert_eq!(st["mode"], "camera");
    // camera drag moves the export frame with the finger
    s.command(&json!({"op": "down", "x": 0, "y": 0, "r": 20}));
    s.command(&json!({"op": "move", "x": 50, "y": 20}));
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["camera"][0], -590.0);
    assert_eq!(st["camera"][1], -340.0);
    assert_eq!(st["dirty"], true);

    let bytes = s.save().unwrap();
    assert_eq!(s.state()["dirty"], false);
    let back = codec::read_bark(&bytes).unwrap();
    assert_eq!(codec::encode(&back.project), codec::encode(s.project()));
    // and every step undoes back to the new project
    while s.doc.can_undo() {
        s.doc.undo();
    }
    // (ids keep counting, so only the counter differs)
    let mut fresh = new_project();
    fresh.id_counter = s.project().id_counter;
    assert_eq!(codec::encode(s.project()), codec::encode(&fresh));
}

#[test]
fn moving_everything_is_one_step() {
    let mut s = session();
    s.command(&json!({"op": "add_figure"}));
    s.command(&json!({"op": "mode", "v": "all"}));
    let before = codec::encode(s.project());
    s.command(&json!({"op": "down", "x": 0, "y": 0, "r": 20}));
    for k in 1..=4 {
        s.command(&json!({"op": "move", "x": 5 * k, "y": 0}));
    }
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], "Move everything");
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
}

/// Four frames (snapshots): the last one has the right arm swung and the figure moved.
fn animated() -> Session {
    let mut s = session();
    for _ in 0..3 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "frame", "i": 3}));
    let st = s.command(&json!({"op": "state"}));
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 60.0, "y": y + 40.0}));
    s.command(&json!({"op": "up"}));
    let (x, y) = handle(&s.command(&json!({"op": "state"})), 0);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 120.0, "y": y}));
    s.command(&json!({"op": "up"}));
    s
}

/// Triangle vertices of the whole picture at a frame.
fn picture(s: &Session, frame: i32) -> Vec<f32> {
    let p = s.project();
    let t = frame as f64 / p.settings.fps as f64;
    let list = barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
    list.batches.iter().flat_map(|b| b.xy.clone()).collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}


#[test]
fn deleting_on_one_frame_leaves_the_others_alone() {
    for k in 0..4 {
        let mut s = animated();
        let before: Vec<Vec<f32>> = (0..4).map(|f| picture(&s, f)).collect();
        let text = codec::encode(s.project());
        s.command(&json!({"op": "frame", "i": k}));
        s.command(&json!({"op": "select", "layer": "L2"}));
        let st = s.command(&json!({"op": "delete_here"}));
        assert!(st["selected"].is_null(), "frame {k}: the hidden figure is not selected");
        for f in 0..4 {
            let now = picture(&s, f);
            if f == k {
                assert!(now.is_empty(), "frame {k}: the figure is gone on its frame");
            } else {
                assert!(same(&now, &before[f as usize]), "deleting on frame {k} changed frame {f}");
            }
        }
        assert_eq!(st["figures"].as_array().unwrap().len(), 1, "frame {k}: still one figure");
        s.command(&json!({"op": "undo"}));
        let mut back = codec::decode(&codec::encode(s.project())).unwrap();
        back.id_counter = codec::decode(&text).unwrap().id_counter;
        assert_eq!(codec::encode(&back), text, "frame {k}: undo");
    }
}




#[test]
fn importing_a_stick_nodes_figure_is_one_step() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/legacy/synthetic");
    let file = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "nodes")).expect("a synthetic .nodes sample");
    let mut s = session();
    let before = codec::encode(s.project());
    let st = s.import(file.file_name().unwrap().to_str().unwrap(), &std::fs::read(&file).unwrap());
    assert_eq!(st["figures"].as_array().unwrap().len(), 2, "{}", st["message"]);
    assert_eq!(st["undo"], "Import");
    assert!(!st["selected"].is_null());
    assert!(barkfart_core::figures::preview_file("x.nodes", &std::fs::read(&file).unwrap()).is_some());
    s.command(&json!({"op": "undo"}));
    let mut back = codec::decode(&codec::encode(s.project())).unwrap();
    back.id_counter = codec::decode(&before).unwrap().id_counter;
    assert_eq!(codec::encode(&back), before);
}

/// Two frames, the right arm swung and the figure moved on the second.
fn two_poses() -> Session {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "state"}));
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 60.0, "y": y + 40.0}));
    s.command(&json!({"op": "up"}));
    let (x, y) = handle(&s.state(), 0);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 120.0, "y": y}));
    s.command(&json!({"op": "up"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s
}

/// The picture at time t (not a frame).
fn picture_at(s: &Session, t: f64) -> Vec<f32> {
    let p = s.project();
    barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None).batches.iter().flat_map(|b| b.xy.clone()).collect()
}

#[test]
fn tweened_frames_are_real_frames_with_the_in_between_pose() {
    for (cmd, fracs) in [(json!({"op": "tween_add"}), vec![0.5]), (json!({"op": "tween_add", "at": [0.1, 0.9]}), vec![0.1, 0.9]), (json!({"op": "tween_add", "n": 3}), vec![0.25, 0.5, 0.75])] {
        let mut s = two_poses();
        assert_eq!(s.state()["fps"], 10.0);
        let (a, b) = (picture(&s, 0), picture(&s, 1));
        let want: Vec<Vec<f32>> = fracs.iter().map(|f| picture_at(&s, 0.1 * f)).collect();
        let st = s.command(&cmd);
        let k = fracs.len() as i64;
        assert_eq!(st["frames"], 2 + k, "{cmd}");
        assert_eq!(st["frame"], k, "{cmd}: the last new frame is shown");
        assert_eq!(st["undo"], "Add tweened frame");
        assert!(same(&picture(&s, 0), &a) && same(&picture(&s, 1 + k as i32), &b), "{cmd}: the two frames are unchanged");
        for (j, w) in want.iter().enumerate() {
            assert!(same(&picture(&s, 1 + j as i32), w), "{cmd}: new frame {} is the pose at {}", j + 1, fracs[j]);
        }
        // a real frame: every pose property is keyed on it
        assert!(st["keyed"].as_array().unwrap().iter().all(|v| v == true), "{cmd}: every frame is a key frame");
        s.command(&json!({"op": "undo"}));
        assert_eq!(s.state()["frames"], 2);
    }
    // only between two frames
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "tween_add"}));
    assert_eq!(st["frames"], 2);
    assert!(!st["message"].as_str().unwrap().is_empty());
}

#[test]
fn tweening_settings_save_and_change_the_picture_rate() {
    let mut s = two_poses();
    assert_eq!(s.state()["rate"], 10.0);
    let st = s.command(&json!({"op": "animation", "tweening": true, "tweened": 5}));
    assert_eq!(st["rate"], 60.0);
    let st = s.command(&json!({"op": "animation", "tweened": 2}));
    assert_eq!(st["rate"], 30.0);
    let st = s.command(&json!({"op": "fps", "v": 30}));
    assert_eq!(st["rate"], 60.0, "at most 60 pictures a second");
    s.command(&json!({"op": "undo"}));
    let text = codec::encode(s.project());
    assert!(text.contains("\"tweening\": true") && text.contains("\"tweenedFrames\": 2"));
    let back = codec::decode(&text).unwrap();
    assert_eq!(codec::encode(&back), text);
    // defaults are not written
    assert!(!codec::encode(&new_project()).contains("tween"));
}

#[test]
fn copy_paste_and_no_tween_frames() {
    let mut s = two_poses();
    let first = picture(&s, 0);
    s.command(&json!({"op": "copy_frame"}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "paste_frame"}));
    assert_eq!(st["frames"], 3);
    assert_eq!(st["frame"], 2);
    assert!(same(&picture(&s, 2), &first), "the pasted frame is the copied pose");
    let st = s.command(&json!({"op": "no_tween", "on": true}));
    assert_eq!(st["frameNoTween"], true);
}

#[test]
fn stickfigure_segment_and_camera_tools() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.command(&json!({"op": "figure", "scale": 3.0}));
    assert_eq!(st["selected"]["scale"], 3.0);
    let st = s.command(&json!({"op": "figure", "turn": 45.0}));
    assert_eq!(st["selected"]["figAngle"], 45.0);
    let st = s.command(&json!({"op": "figure", "color": 0x336699ffu32}));
    assert_eq!(st["selected"]["figColor"], 0x336699ffu32);
    let st = s.command(&json!({"op": "figure", "locked": true}));
    assert_eq!(st["selected"]["locked"], true);
    assert_eq!(st["handles"].as_array().unwrap().len(), 0, "a locked figure has no handles");
    s.command(&json!({"op": "figure", "locked": false}));
    // scale is keyed on this frame only
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(s.state()["selected"]["scale"], 2.0);

    s.command(&json!({"op": "select", "layer": "L2", "node": "armR2"}));
    let st = s.command(&json!({"op": "node", "stretchy": true, "length": 70.0}));
    assert_eq!(st["selected"]["stretchy"], true);
    assert_eq!(st["selected"]["length"], 70.0);

    s.command(&json!({"op": "copy_figure"}));
    let st = s.command(&json!({"op": "paste_figure"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
    assert_ne!(st["selected"]["layer"], "L2");
    // pasted on frame 2: not there on frame 1
    s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(s.state()["handles"].as_array().unwrap().len(), 11);

    let st = s.command(&json!({"op": "add_camera"}));
    assert!(st["cam"].is_object());
    let st = s.command(&json!({"op": "camera", "zoom": 1.5, "rotation": 10.0}));
    assert_eq!(st["cam"]["zoom"], 1.5);
    assert_eq!(st["undo"], "Camera");
}

/// "Modify branch" applies one property to the selected joint and everything
/// below it, one undo step per Set.
#[test]
fn modify_branch_changes_a_property_down_the_whole_branch() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let before = codec::encode(s.project());
    let read = |s: &Session, n: &str| {
        let p = s.project();
        let f = p.root().unwrap().layer("L2").unwrap().as_figure().unwrap();
        let node = f.node(n).unwrap();
        let mut r = barkfart_core::eval::Resolver::new(p);
        (r.f(&node.thickness, 0.0), r.f(&node.length, 0.0), node.stretchy, node.is_static)
    };
    let torso = read(&s, "torso");

    let st = s.command(&json!({"op": "segment", "branch": true, "thickness": 7.0}));
    assert_eq!(st["undo"], "Segment");
    assert_eq!(read(&s, "armR1").0, 7.0);
    assert_eq!(read(&s, "armR2").0, 7.0, "the child takes the new thickness too");
    assert_eq!(read(&s, "torso").0, torso.0, "a joint above the branch is untouched");
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before, "one undo step");

    s.command(&json!({"op": "segment", "branch": true, "stretchy": true}));
    assert!(read(&s, "armR1").2 && read(&s, "armR2").2);
    assert!(!read(&s, "torso").2);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before, "one undo step");

    s.command(&json!({"op": "segment", "branch": true, "scaleLength": 1.5}));
    assert_eq!(read(&s, "armR1").1, 67.5);
    assert_eq!(read(&s, "armR2").1, 67.5);
    assert_eq!(read(&s, "torso").1, torso.1);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before, "one undo step");

    s.command(&json!({"op": "segment", "branch": true, "static": true}));
    assert!(read(&s, "armR1").3 && read(&s, "armR2").3);
    assert!(!read(&s, "torso").3);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
}

/// The original's "Stickfigure (IK)" is a rig: stretchy limbs whose static
/// detail segments smart-stretch. Dragging a limb end in Barkfart must bend
/// it the same way (lengths of the static children follow).
#[test]
#[ignore = "superseded by ik_rig_matches_the_original (the IK leg is a connector rig, not the movieclip smart-stretch rule)"]
fn the_ik_stickfigure_bends_by_smart_stretch() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Stickfigure (IK).nodes");
    let Ok(bytes) = std::fs::read(path) else { return };
    let mut p = barkfart_core::session::new_editor_project();
    p.root_mut().unwrap().layers.clear();
    let mut s = Session::new(Document::new(p));
    let st = s.import("Stickfigure (IK).nodes", &bytes);
    let layer = st["selected"]["layer"].as_str().unwrap().to_string();
    let len = |s: &Session, n: &str| {
        let p = s.project();
        let f = p.root().unwrap().layer(&layer).unwrap().as_figure().unwrap();
        let mut r = barkfart_core::eval::Resolver::new(p);
        r.f(&f.node(n).unwrap().length, 0.0)
    };
    // n31: a stretchy leg straight from the hip; n2 its smart-stretch knee detail
    let (leg0, knee0) = (len(&s, "n31"), len(&s, "n2"));
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "n31").expect("the leg end has a handle");
    let (hx, hy) = (h.x, h.y);
    s.command(&json!({"op": "down", "x": hx, "y": hy, "r": 5}));
    // pull the foot halfway towards the hip: the leg gets shorter
    let hip = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "n0").unwrap();
    s.command(&json!({"op": "move", "x": (hx + hip.x) / 2.0, "y": (hy + hip.y) / 2.0}));
    s.command(&json!({"op": "up"}));
    let (leg1, knee1) = (len(&s, "n31"), len(&s, "n2"));
    assert!(leg1 < leg0 * 0.7, "the stretchy leg shortened: {leg0} -> {leg1}");
    assert!((knee1 - knee0).abs() > 1.0, "the smart-stretch detail followed: {knee0} -> {knee1}");
}

#[test]
fn tweened_motion_is_smooth_between_frames() {
    // three frames, the figure moving right on each; sampled at 60 a second
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    let (x, y) = handle(&s.state(), 0);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 120.0, "y": y}));
    s.command(&json!({"op": "up"}));
    let p = s.project();
    let l = p.root().unwrap().layer("L2").unwrap();
    let mut r = barkfart_core::eval::Resolver::new(p);
    let xs: Vec<f32> = (0..=12).map(|i| r.f(&l.transform.x, i as f64 / 60.0)).collect();
    for w in xs.windows(3) {
        let (a, b) = (w[1] - w[0], w[2] - w[1]);
        assert!(b > 0.0 && (a - b).abs() < 1e-3, "even steps while tweening: {xs:?}");
    }
}

/// Inserting frames shifts later keys by 1/fps; float error used to leave a
/// stale key a hair away from a new one on the same frame, so tweens
/// snapped back ("rubber-band"), in playback and export.
#[test]
fn keys_stay_on_their_frames_after_many_inserts() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "fps", "v": 12}));
    for i in 0..12 {
        // insert in the middle (shifting everything after), then pose the new frame
        s.command(&json!({"op": "frame", "i": i / 2}));
        s.command(&json!({"op": "insert_frame"}));
        let (x, y) = handle(&s.state(), 0);
        s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
        s.command(&json!({"op": "move", "x": x + 7.0 + i as f32, "y": y}));
        s.command(&json!({"op": "up"}));
        s.command(&json!({"op": "tween_add"}));
    }
    let p = s.project();
    let fps = p.settings.fps as f64;
    for l in &p.root().unwrap().layers {
        for q in l.properties() {
            for w in q.track.keys().windows(2) {
                assert!(w[1].time - w[0].time > 0.5 / fps, "{}: keys {} and {} are on the same frame", q.id, w[0].time, w[1].time);
            }
            for k in q.track.keys() {
                assert_eq!(k.time, (k.time * fps).round() / fps, "{}: key at {} is off its frame", q.id, k.time);
            }
        }
    }
}

#[test]
fn easing_shapes_the_tween_between_frames() {
    let mut s = two_poses();
    let mid = picture_at(&s, 0.05);
    let st = s.command(&json!({"op": "easing", "v": "smooth"}));
    assert_eq!(st["undo"], "Easing");
    assert!(!same(&picture_at(&s, 0.03), &mid) || !same(&picture_at(&s, 0.02), &picture_at(&{ let t = two_poses(); t }, 0.02)), "the in-between changed");
    assert!(same(&picture(&s, 0), &picture(&two_poses(), 0)) && same(&picture(&s, 1), &picture(&two_poses(), 1)), "the frames themselves did not");
}

#[test]
fn a_figure_added_pasted_or_deleted_on_one_frame_stays_on_that_frame() {
    let mut s = session();
    for _ in 0..3 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    let on = |s: &mut Session, f: i32| {
        s.command(&json!({"op": "frame", "i": f}));
        s.tools.handles(s.project(), s.time()).iter().filter(|h| h.layer == ball).count() > 0
    };
    assert_eq!((0..4).map(|f| on(&mut s, f)).collect::<Vec<_>>(), vec![false, true, false, false], "added on frame 2 only");
    // a new frame after frame 2 copies it, ball included
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!((0..5).map(|f| on(&mut s, f)).collect::<Vec<_>>(), vec![false, true, true, false, false]);
    // deleting on frame 3 leaves frame 2
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": ball}));
    s.command(&json!({"op": "delete_here"}));
    assert_eq!((0..5).map(|f| on(&mut s, f)).collect::<Vec<_>>(), vec![false, true, false, false, false]);
    // copy it and paste onto frame 5 (which does not have it): the same ball, now on 2 and 5
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": ball}));
    s.command(&json!({"op": "copy_figure"}));
    s.command(&json!({"op": "frame", "i": 4}));
    let st = s.command(&json!({"op": "paste_figure"}));
    assert_eq!(st["selected"]["layer"], ball.as_str());
    assert_eq!((0..5).map(|f| on(&mut s, f)).collect::<Vec<_>>(), vec![false, true, false, false, true]);
    // a figure added on one frame and deleted there is removed altogether
    s.command(&json!({"op": "frame", "i": 3}));
    let st = s.command(&json!({"op": "add_figure", "kind": "stick"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 3);
    let st = s.command(&json!({"op": "delete_here"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
}

#[test]
fn pasting_onto_a_frame_without_the_figure_brings_the_same_figure_back() {
    let mut s = two_poses();
    s.command(&json!({"op": "insert_frame"}));
    // frames 1..3; delete the figure on frame 2 by accident
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "delete_here"}));
    // copy it from frame 1 and paste it onto frame 2: the same figure (tweens), no new one
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "copy_figure"}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "paste_figure"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 1, "no new figure");
    assert_eq!(st["selected"]["layer"], "L2");
    assert!(same(&picture(&s, 1), &picture(&s, 0)), "frame 2 now has the frame 1 pose");
    // pasting where it already is: a new figure
    let st = s.command(&json!({"op": "paste_figure"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
    assert_ne!(st["selected"]["layer"], "L2");
}

#[test]
fn joining_a_figure_to_a_joint() {
    let mut s = session();
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    let ball_at = |s: &Session| s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == ball && h.root).map(|h| (h.x, h.y)).unwrap();
    // Join to... then tap the man's right hand
    s.command(&json!({"op": "join"}));
    assert_eq!(s.state()["joining"], true);
    let hand = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == "L2" && h.node == "armR2").unwrap();
    let before = ball_at(&s);
    let st = s.command(&json!({"op": "down", "x": hand.x, "y": hand.y, "r": 20}));
    s.command(&json!({"op": "up"}));
    assert_eq!(st["joining"], false);
    assert_eq!(st["undo"], "Join");
    assert_eq!(st["selected"]["joinedTo"], "Figure 1");
    let after = ball_at(&s);
    assert!((after.0 - before.0).abs() < 0.01 && (after.1 - before.1).abs() < 0.01, "joining keeps the ball in place");
    // move the man: the ball follows
    let (hx, hy) = (s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == "L2" && h.root).unwrap().x, 0.0);
    let hip = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == "L2" && h.root).unwrap();
    let _ = (hx, hy);
    s.command(&json!({"op": "down", "x": hip.x, "y": hip.y, "r": 10}));
    s.command(&json!({"op": "move", "x": hip.x + 80.0, "y": hip.y}));
    s.command(&json!({"op": "up"}));
    let moved = ball_at(&s);
    assert!((moved.0 - after.0 - 80.0).abs() < 0.5, "the ball moved with the man: {after:?} -> {moved:?}");
    s.command(&json!({"op": "select", "layer": ball}));
    let st = s.command(&json!({"op": "unjoin"}));
    assert!(st["selected"]["joinedTo"].is_null());
    let still = ball_at(&s);
    assert!((still.0 - moved.0).abs() < 0.01, "unjoin keeps it in place");
}

#[test]
fn copying_a_frame_and_the_ones_after_it() {
    let mut s = animated(); // 4 frames, the last one posed differently
    let pics: Vec<Vec<f32>> = (0..4).map(|f| picture(&s, f)).collect();
    s.command(&json!({"op": "frame", "i": 2}));
    let st = s.command(&json!({"op": "copy_frame", "n": 1}));
    assert_eq!(st["frameClipCount"], 2);
    s.command(&json!({"op": "frame", "i": 3}));
    let st = s.command(&json!({"op": "paste_frame"}));
    assert_eq!(st["frames"], 6);
    assert_eq!(st["frame"], 5);
    for (f, want) in [(4, 2), (5, 3)] {
        assert!(same(&picture(&s, f), &pics[want]), "frame {} is a copy of frame {}", f + 1, want + 1);
    }
    for f in 0..4 {
        assert!(same(&picture(&s, f), &pics[f as usize]), "frame {} unchanged", f + 1);
    }
    let st = s.command(&json!({"op": "no_tween", "on": true}));
    assert_eq!(st["noTweenFrames"][5], true);
    assert_eq!(st["noTweenFrames"][4], false);
}


/// Ground truth from the original (Stick Nodes Pro 4.3.1, 2026-11-21): a movieclip
/// of "Stickfigure (IK)" with one foot (stretchy n30) pulled to 176.7, 67.6
/// and 465.5 (past reach). The original stored, per frame, the lengths of the
/// construction segments n11, n12 (reset impulse x -1), n13 and the connectors
/// n14 (construction point to hip) and n15 (knee, the end of the visible thigh n27, to foot).
#[test]
fn ik_rig_matches_the_original() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Stickfigure (IK).nodes");
    let Ok(bytes) = std::fs::read(path) else { return };
    let mut p = new_project();
    p.root_mut().unwrap().layers.clear();
    barkfart_core::import::import_file(&mut p, "ik.nodes", &bytes).unwrap();
    let truth = [(176.7f32, [94.5f32, 197.3, 82.7, 141.0, 148.3]), (67.6, [36.2, 248.3, 31.7, 218.9, 146.5]), (465.5, [160.5, 139.5, 140.5, 139.5, 160.0])];
    for (raw, want) in truth {
        let mut q = p.clone();
        let root = q.root_symbol.clone();
        let lid = q.root().unwrap().layers[0].id.clone();
        let (clamped, sets) = {
            let f = q.root().unwrap().layers[0].as_figure().unwrap();
            barkfart_core::edit::canvas::connector_smart_stretch(f, f.index_of("n30").unwrap(), raw).expect("an IK leg")
        };
        {
            let l = q.layer_mut(&root, &lid).unwrap();
            let barkfart_core::model::LayerKind::Figure(f) = &mut l.kind else { unreachable!() };
            f.node_mut("n30").unwrap().length.value = barkfart_core::model::Value::F(clamped);
            for (id, v) in &sets {
                let nid = id.split('/').nth(1).unwrap();
                f.node_mut(nid).unwrap().length.value = barkfart_core::model::Value::F(*v);
            }
        }
        let f = q.root().unwrap().layers[0].as_figure().unwrap();
        let len = |n: &str| f.node(n).unwrap().length.value.f();
        let pose = barkfart_core::eval::FigurePose::compute(f, 0.0, &mut barkfart_core::eval::Resolver::new(&q));
        let at = |n: &str| { let i = f.index_of(n).unwrap(); (pose.x[i], pose.y[i]) };
        let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).hypot(a.1 - b.1)) as f32;
        let knee = at("n13");
        let got = [len("n11"), len("n12"), len("n13"), dist(knee, at("n0")), dist(at("n27"), at("n10"))];
        for (k, name) in ["n11", "n12", "n13", "knee to hip", "knee to foot"].iter().enumerate() {
            assert!((got[k] - want[k]).abs() < 0.25, "foot at {raw}: {name} is {} in Barkfart, {} in the original", got[k], want[k]);
        }
    }
}

fn text_layers(s: &Session) -> Vec<barkfart_core::model::Layer> {
    s.project().root().unwrap().layers.iter().filter(|l| matches!(l.kind, barkfart_core::model::LayerKind::Text(_))).map(|l| (**l).clone()).collect()
}

#[test]
fn textfields_are_added_on_this_frame_and_dragged_keyed() {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(s.state()["frames"], 2);
    let st = s.command(&json!({"op": "add_textfield"}));
    let tf = st["textfield"].clone();
    assert_eq!(tf["text"], "Hello");
    assert_eq!(tf["align"], "center");
    assert_eq!(tf["scale"], 1.0);
    assert_eq!(tf["locked"], false);
    let id = tf["layer"].as_str().unwrap().to_string();
    assert_eq!(text_layers(&s).len(), 1);

    // the textfield is on this frame only
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert!(st["textfield"].is_null());
    assert_eq!(text_layers(&s).len(), 1, "the layer still exists on the other frame");
    s.command(&json!({"op": "frame", "i": 1}));

    // tap its right end (away from the figure's hip at the centre)
    let st = s.command(&json!({"op": "down", "x": 40.0, "y": 10.0, "r": 10.0}));
    assert_eq!(st["hit"], true);
    assert_eq!(st["textfield"]["layer"], id);
    s.command(&json!({"op": "up"}));

    let st = s.command(&json!({"op": "textfield", "text": "Hi\nthere", "color": 0xff0000ffu32, "scale": 2.0, "align": 0, "locked": true}));
    assert_eq!(st["textfield"]["text"], "Hi\nthere");
    assert_eq!(st["textfield"]["color"], 0xff0000ffu32);
    assert_eq!(st["textfield"]["scale"], 2.0);
    assert_eq!(st["textfield"]["align"], "left");
    assert_eq!(st["textfield"]["locked"], true);
    // the text is really in the layer
    let l = s.project().root().unwrap().layer(&id).unwrap();
    let barkfart_core::model::LayerKind::Text(tl) = &l.kind else { panic!("not text") };
    assert_eq!(tl.text.as_deref(), Some("Hi\nthere"));

    // unlocked again: dragging moves it, keyed on this frame
    s.command(&json!({"op": "textfield", "locked": false}));
    let before = codec::encode(s.project());
    s.command(&json!({"op": "down", "x": 40.0, "y": 10.0, "r": 10.0}));
    for k in 1..=4 {
        s.command(&json!({"op": "move", "x": 40.0 + 10.0 * k as f32, "y": 10.0 - 5.0 * k as f32}));
    }
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], "Move textfield");
    assert_eq!(st["textfield"]["x"], 40.0);
    assert_eq!(st["textfield"]["y"], -20.0);
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
    assert_eq!(st["textfield"]["x"], 0.0);

    // copy and paste make a second textfield on top of the first
    s.command(&json!({"op": "down", "x": 40.0, "y": 10.0, "r": 10.0}));
    s.command(&json!({"op": "up"}));
    let st = s.command(&json!({"op": "copy_textfield"}));
    assert_eq!(st["textfieldCopied"], true);
    let st = s.command(&json!({"op": "paste_textfield"}));
    assert_eq!(text_layers(&s).len(), 2);
    let copy = st["textfield"]["layer"].as_str().unwrap().to_string();
    assert_ne!(copy, id);
    assert_eq!(st["textfield"]["text"], "Hi\nthere");

    // delete removes it from this frame; on no frame at all, the layer goes
    s.command(&json!({"op": "delete_textfield"}));
    assert_eq!(text_layers(&s).len(), 1);
    s.command(&json!({"op": "select"}));
    s.command(&json!({"op": "down", "x": 40.0, "y": 10.0, "r": 10.0}));
    s.command(&json!({"op": "up"}));
    s.command(&json!({"op": "delete_textfield"}));
    assert_eq!(text_layers(&s).len(), 0);
}

#[test]
fn stickfigure_filters_map_to_effect_nodes_on_this_frame() {
    let mut s = session();
    s.command(&json!({"op": "set", "k": "keying", "v": true})); // per-frame effects are keyed (D5)
    let st = s.command(&json!({"op": "state"}));
    let id = st["figures"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": id}));
    let st = s.command(&json!({"op": "filter", "opacity": 0.5, "invert": true, "hue": 30.0, "saturation": 0.5,
        "pixelate": 4.0, "tint": 0x00ff00ffu32, "tintAmount": 0.3, "outline": 0xff00ffffu32, "outlineAmount": 0.8,
        "outlineThickness": 3.0, "blur": 2.0, "motion": true, "motionDistance": 12.0, "glow": 0x0a84ffffu32,
        "glowAmount": 0.4, "glowIntensity": 2.0, "shadow": 0x000000ffu32, "shadowOpacity": 0.5,
        "shadowDistance": 20.0, "shadowAngle": 90.0, "shadowBlur": 3.0}));
    let f = &st["selected"]["filters"];
    assert_eq!(f["opacity"], 0.5);
    assert_eq!(f["invert"], true);
    assert_eq!(f["hue"], 30.0);
    assert_eq!(f["saturation"], 0.5);
    assert_eq!(f["pixelate"], 4.0);
    assert_eq!(f["tint"], 0x00ff00ffu32);
    assert_eq!(f["tintAmount"], 0.3);
    assert_eq!(f["outline"], 0xff00ffffu32);
    assert_eq!(f["outlineAmount"], 0.8);
    assert_eq!(f["outlineThickness"], 3.0);
    assert_eq!(f["blur"], 2.0);
    assert_eq!(f["motion"], true);
    assert_eq!(f["motionDistance"], 12.0);
    assert_eq!(f["glow"], 0x0a84ffffu32);
    assert_eq!(f["glowAmount"], 0.4);
    assert_eq!(f["glowIntensity"], 2.0);
    assert_eq!(f["shadow"], 0x000000ffu32);
    assert_eq!(f["shadowOpacity"], 0.5);
    assert_eq!(f["shadowDistance"], 20.0);
    assert_eq!(f["shadowAngle"], 90.0);
    assert_eq!(f["shadowBlur"], 3.0);
    {
        let l = s.project().root().unwrap().layer(&id).unwrap();
        let ids: Vec<&str> = l.effects.nodes.keys().map(String::as_str).collect();
        assert_eq!(ids, ["filter.invert", "filter.adjust", "filter.pixelate", "filter.tint", "filter.outline", "filter.blur", "filter.motion", "filter.glow", "filter.shadow"]);
        // the controls are keyed on this frame (the first one)
        let blur = &l.effects.nodes["filter.blur"];
        assert!(blur.params["radius"].track.index_of(0.0).is_ok());
        assert!(blur.mix.track.index_of(0.0).is_ok());
    }

    // undo takes every filter away again
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["selected"]["filters"]["opacity"], 1.0);
    assert_eq!(st["selected"]["filters"]["invert"], false);
    let l = s.project().root().unwrap().layer(&id).unwrap();
    assert!(l.effects.is_empty());

    // copy the filter set onto another figure
    s.command(&json!({"op": "filter", "blur": 2.0, "invert": true}));
    s.command(&json!({"op": "copy_filters"}));
    let st = s.command(&json!({"op": "add_figure"}));
    let second = st["selected"]["layer"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "paste_filters"}));
    assert_eq!(st["selected"]["filters"]["blur"], 2.0);
    assert_eq!(st["selected"]["filters"]["invert"], true);
    // clear all
    let st = s.command(&json!({"op": "clear_filters"}));
    assert_eq!(st["selected"]["filters"]["blur"], 0.0);
    assert_eq!(st["selected"]["filters"]["invert"], false);
    let l = s.project().root().unwrap().layer(&second).unwrap();
    assert!(l.effects.is_empty());
}

#[test]
fn frame_background_is_per_frame_and_undoable() {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "state"}));
    assert_eq!(st["frameBg"]["color"], 0xffffffffu32);
    assert_eq!(st["frameBg"]["useGradient"], false);
    let without_ids = |s: &Session| codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
    let before = without_ids(&s);

    // frame 2 gets a red background; frame 1 stays white
    let st = s.command(&json!({"op": "frame_bg", "color": 0xff0000ffu32}));
    assert_eq!(st["frameBg"]["color"], 0xff0000ffu32);
    assert_eq!(st["hasCamera"], true);
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(st["frameBg"]["color"], 0xffffffffu32);
    s.command(&json!({"op": "frame", "i": 1}));

    // a gradient keeps both colours, and turning it off brings back the plain colour
    let st = s.command(&json!({"op": "frame_bg", "useGradient": true, "gradient": 0x0000ffffu32}));
    assert_eq!(st["frameBg"]["color"], 0xff0000ffu32);
    assert_eq!(st["frameBg"]["gradient"], 0x0000ffffu32);
    assert_eq!(st["frameBg"]["useGradient"], true);
    let st = s.command(&json!({"op": "frame_bg", "useGradient": false}));
    assert_eq!(st["frameBg"]["color"], 0xff0000ffu32);
    assert_eq!(st["frameBg"]["gradient"], 0x0000ffffu32);
    assert_eq!(st["frameBg"]["useGradient"], false);

    // a new frame copies this frame's background
    let st = s.command(&json!({"op": "insert_frame"}));
    assert_eq!(st["frame"], 2);
    assert_eq!(st["frameBg"]["color"], 0xff0000ffu32);

    for _ in 0..4 {
        s.command(&json!({"op": "undo"}));
    }
    assert!(without_ids(&s) == before, "undo restores the project");
}

#[test]
fn frame_delay_and_repeat_follow_their_frames() {
    use barkfart_core::timing::Schedule;
    let mut s = session();
    let without_ids = |s: &Session| codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
    let before = without_ids(&s);
    assert_eq!(s.state()["canUndo"], false);
    for _ in 0..3 {
        s.command(&json!({"op": "insert_frame"}));
    }
    assert_eq!(s.state()["frames"], 4);
    let fps = s.project().settings.fps as f64;
    let plain = Schedule::build(s.project());
    assert!(plain.identity);
    assert!((plain.total - 4.0 / fps).abs() < 1e-9);

    // frame 2 (index 1) pauses half a second
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "frame_delay", "seconds": 0.5}));
    assert_eq!(st["frameDelay"], 0.5);
    assert_eq!(st["frameMarks"][1][0], true);
    assert_eq!(st["frameMarks"][0][0], false);
    let sch = Schedule::build(s.project());
    assert!(!sch.identity);
    assert!((sch.total - (4.0 / fps + 0.5)).abs() < 1e-6);
    // during the pause the picture holds frame 2, then frame 3 follows
    assert!((sch.time_at(1.0 / fps + 0.3, fps) - 1.0 / fps).abs() < 1e-9);
    assert!((sch.time_at(2.0 / fps + 0.5 + 1e-6, fps) - 2.0 / fps).abs() < 1e-9);
    // the file keeps it
    let back = codec::decode(&codec::encode(s.project())).unwrap();
    assert_eq!(barkfart_core::timing::frame_settings(&back, 1).0, 0.5);

    // a frame inserted before it moves the delay along; the new frame has none
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(barkfart_core::timing::frame_settings(s.project(), 2).0, 0.5);
    assert_eq!(barkfart_core::timing::frame_settings(s.project(), 1).0, 0.0);
    // a tweened frame after the delayed one does not inherit it
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "tween_add", "at": [0.5]}));
    assert_eq!(barkfart_core::timing::frame_settings(s.project(), 3).0, 0.0);
    // copy and paste carry it
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "copy_frame"}));
    s.command(&json!({"op": "frame", "i": 5}));
    s.command(&json!({"op": "paste_frame"}));
    assert_eq!(barkfart_core::timing::frame_settings(s.project(), 6).0, 0.5);

    // the last frame repeats the two before it twice: 7 frames + 2 * 3 slots
    let n = s.state()["frames"].as_i64().unwrap() as i32;
    s.command(&json!({"op": "frame", "i": n - 1}));
    let st = s.command(&json!({"op": "frame_repeat", "back": 2, "times": 2}));
    assert_eq!(st["frameRepeat"]["back"], 2);
    assert_eq!(st["frameMarks"][(n - 1) as usize][1], true);
    let sch = Schedule::build(s.project());
    let order: Vec<i32> = sch.slots.iter().map(|x| x.frame).collect();
    let k = n - 1;
    let mut want: Vec<i32> = (0..=k).collect();
    want.extend([k - 2, k - 1, k, k - 2, k - 1, k]);
    assert_eq!(order, want);
    // turning it off
    let st = s.command(&json!({"op": "frame_repeat", "back": 0}));
    assert_eq!(st["frameRepeat"]["back"], 0);
    assert_eq!(Schedule::build(s.project()).slots.len(), n as usize);

    while s.state()["canUndo"] == true {
        s.command(&json!({"op": "undo"}));
    }
    assert!(without_ids(&s) == before, "undo restores the project");
}

#[test]
fn frames_play_their_own_sounds() {
    use barkfart_core::audio::{encode_wav, tone, AudioPlan};
    let mut s = session();
    let without_ids = |s: &Session| codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
    let before = without_ids(&s);
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    let wav = encode_wav(&tone(440.0, 0.2, 8000, 0.5));
    let st = s.import("beep.wav", &wav);
    assert_eq!(st["soundList"][0]["name"], "beep.wav");
    let id = st["soundList"][0]["id"].as_str().unwrap().to_string();
    // importing adds no layer and no sound on any frame
    assert!(st["frameSound"].is_null());

    // frame 2 (index 1) plays it, quieter and to the left
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "frame_sound", "asset": id, "volume": 0.5, "pan": -0.5, "pitch": 2.0}));
    assert_eq!(st["frameSound"]["name"], "beep.wav");
    assert_eq!(st["frameSound"]["volume"], 0.5);
    assert_eq!(st["frameSound"]["pitch"], 2.0);
    let fps = s.project().settings.fps as f64;
    let sounds = |_: &str| Some(std::sync::Arc::new(tone(440.0, 0.2, 8000, 0.5)));
    let plan = AudioPlan::build(s.project(), s.project().root().unwrap(), &sounds);
    assert_eq!(plan.entries.len(), 1);
    assert!((plan.entries[0].start - 1.0 / fps).abs() < 1e-9);
    // the file keeps the sound's name
    let back = codec::decode(&codec::encode(s.project())).unwrap();
    assert_eq!(back.assets[&id].name.as_deref(), Some("beep.wav"));

    // a frame inserted before it moves it; deleting its frame removes it
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "frame", "i": 2}));
    assert_eq!(st["frameSound"]["name"], "beep.wav");
    // copy and paste it onto the last frame
    s.command(&json!({"op": "copy_sound"}));
    s.command(&json!({"op": "frame", "i": 3}));
    let st = s.command(&json!({"op": "paste_sound"}));
    assert_eq!(st["frameSound"]["volume"], 0.5);
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "delete_frame"}));
    let plan = AudioPlan::build(s.project(), s.project().root().unwrap(), &sounds);
    assert_eq!(plan.entries.len(), 1, "the deleted frame's sound went with it");
    assert!((plan.entries[0].start - 2.0 / fps).abs() < 1e-9);
    // clear
    s.command(&json!({"op": "frame", "i": 2}));
    let st = s.command(&json!({"op": "frame_sound", "asset": null}));
    assert!(st["frameSound"].is_null());

    while s.state()["canUndo"] == true {
        s.command(&json!({"op": "undo"}));
    }
    assert!(without_ids(&s) == before, "undo restores the project");
}

#[test]
fn creation_mode_draws_a_segment_from_a_joint() {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let before = codec::encode(s.project());
    s.command(&json!({"op": "build", "layer": fig}));
    let st = s.command(&json!({"op": "draw_tool", "shape": "circle"}));
    assert_eq!(st["drawTool"], "circle");
    // from the right hand to a point up and to the right of it
    let (x, y) = handle(&st, 6);
    let st = s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    assert_eq!(st["hit"], true);
    s.command(&json!({"op": "move", "x": x + 30.0, "y": y + 10.0}));
    let st = s.command(&json!({"op": "move", "x": x + 60.0, "y": y + 40.0}));
    assert!((st["drawLine"][2].as_f64().unwrap() - (x + 60.0) as f64).abs() < 0.01);
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], "Draw segment");
    assert!(st["drawLine"].is_null());
    assert_eq!(st["handles"].as_array().unwrap().len(), 12);
    assert_eq!(st["selected"]["shape"], "circle");
    // the new joint is where the finger let go, on both frames
    let (nx, ny) = handle(&st, 11);
    assert!((nx - (x + 60.0)).abs() < 0.05 && (ny - (y + 40.0)).abs() < 0.05, "{nx},{ny}");
    let st = s.command(&json!({"op": "frame", "i": 0}));
    let (nx, ny) = handle(&st, 11);
    assert!((nx - (x + 60.0)).abs() < 0.05 && (ny - (y + 40.0)).abs() < 0.05);
    // a tap on a joint (no drag) draws nothing
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["handles"].as_array().unwrap().len(), 12);
    s.command(&json!({"op": "build_done"}));
    s.command(&json!({"op": "undo"}));
    assert!(codec::encode(s.project()) == before, "undo takes the segment away");
}

#[test]
fn arrows_nudge_the_selected_figure_on_this_frame() {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let before: Vec<(f32, f32)> = (0..11).map(|i| handle(&st, i)).collect();
    s.command(&json!({"op": "select", "layer": fig}));
    let st = s.command(&json!({"op": "nudge", "dx": 10.0, "dy": -2.5}));
    assert_eq!(st["undo"], "Nudge figure");
    for i in 0..11 {
        let (x, y) = handle(&st, i);
        assert!((x - before[i].0 - 10.0).abs() < 0.02 && (y - before[i].1 + 2.5).abs() < 0.02, "joint {i}");
    }
    // the other frame keeps its pose
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(handle(&st, 0), before[0]);
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(handle(&st, 0), before[0]);
}

/// The IK arms: dragging a hand must not pull the arm in to one forearm's
/// length (the connector's stored 110 is not the 220 arm's reach).
#[test]
fn ik_arms_keep_their_length_when_dragged() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../app/assets/stock/figures/Stickfigure (IK).nodes");
    let Ok(bytes) = std::fs::read(path) else { return };
    let p = {
        let mut p = new_project();
        p.root_mut().unwrap().layers.clear();
        barkfart_core::import::import_file(&mut p, "ik.nodes", &bytes).unwrap();
        p
    };
    let f = p.root().unwrap().layers[0].as_figure().unwrap();
    let ci = barkfart_core::edit::canvas::connector_smart_stretch;
    // a bit shorter: the arm follows, the forearm keeps its share
    let (len, sets) = ci(f, f.index_of("n32").unwrap(), 200.0).expect("an IK arm");
    assert!((len - 200.0).abs() < 1e-3, "{len}");
    let get = |id: &str| sets.iter().find(|(k, _)| k.split('/').nth(1) == Some(id)).map(|x| x.1).unwrap();
    assert!((get("n16") - 100.0).abs() < 0.01);
    assert!((get("n28") - 100.0).abs() < 0.01);
    // at rest nothing changes; past the reach it stops at the full arm
    let (len, sets) = ci(f, f.index_of("n32").unwrap(), 220.0).unwrap();
    assert!((len - 220.0).abs() < 1e-3 && sets.iter().all(|(k, v)| !k.contains("/n28/") || (v - 110.0).abs() < 0.01));
    let (len, _) = ci(f, f.index_of("n32").unwrap(), 400.0).unwrap();
    assert!((len - 220.0).abs() < 1e-3);

    // through the session: a hand dragged a little stays where it was put
    let mut s = Session::new(Document::new(new_project()));
    let st = s.import("Stickfigure (IK).nodes", &bytes);
    let lid = st["selected"]["layer"].as_str().unwrap().to_string();
    let h = barkfart_core::edit::canvas::CanvasTools::default().handles(s.project(), 0.0).into_iter().find(|h| h.layer == lid && h.node == "n32").expect("hand handle");
    let (hx, hy) = (h.x, h.y);
    s.command(&json!({"op": "down", "x": hx, "y": hy, "r": 6}));
    for k in 1..=4 {
        s.command(&json!({"op": "move", "x": hx + 2.0 * k as f32, "y": hy}));
    }
    s.command(&json!({"op": "up"}));
    let f = s.project().root().unwrap().layer(&lid).unwrap().as_figure().unwrap();
    let mut r = barkfart_core::eval::Resolver::new(s.project());
    let n = f.node("n32").unwrap();
    let l = r.f(&n.length, 0.0);
    assert!(l > 180.0, "the arm was pulled in to {l}");
}

#[test]
fn segment_tools_set_gradient_curve_and_shape_options() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "up"}));
    let before = codec::encode(s.project());
    let st = s.command(&json!({"op": "segment", "gradient": true, "gradientColor": 0x0000ffffu32, "curve": 30.0, "segScale": 1.5, "angleLock": 2, "sides": 7}));
    let sel = &st["selected"];
    assert_eq!(sel["gradient"], true);
    assert_eq!(sel["gradientColor"], 0x0000ffffu32);
    assert_eq!(sel["curve"], 30.0);
    assert_eq!(sel["segScale"], 1.5);
    assert_eq!(sel["angleLock"], 2);
    assert_eq!(sel["sides"], 7);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
}

#[test]
fn silence_and_clear_frame() {
    use barkfart_core::audio::{encode_wav, tone, AudioPlan};
    use barkfart_core::timing::Schedule;
    let mut s = session();
    for _ in 0..3 {
        s.command(&json!({"op": "insert_frame"}));
    }
    let st = s.import("long.wav", &encode_wav(&tone(440.0, 2.0, 8000, 0.5)));
    let id = st["soundList"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "frame_sound", "asset": id}));
    // frame 3 silences it; frame 4 repeats frames 1-4 once
    s.command(&json!({"op": "frame", "i": 2}));
    let st = s.command(&json!({"op": "frame_silence", "on": true}));
    assert_eq!(st["frameSilence"], true);
    assert_eq!(st["frameMarks"][2][2], true);
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "frame_repeat", "back": 3, "times": 1}));
    let fps = s.project().settings.fps as f64;
    let sounds = |_: &str| Some(std::sync::Arc::new(tone(440.0, 2.0, 8000, 0.5)));
    let plan = AudioPlan::build(s.project(), s.project().root().unwrap(), &sounds);
    let out = Schedule::build(s.project()).retime(&plan);
    // played twice (frame 1 comes round again), each cut at the next silence
    assert_eq!(out.entries.len(), 2);
    assert!((out.entries[0].end - 2.0 / fps).abs() < 1e-9);
    assert!((out.entries[1].start - 4.0 / fps).abs() < 1e-9 && (out.entries[1].end - 6.0 / fps).abs() < 1e-9);

    // clear frame: nothing left on it, the other frames keep the figure; undoable
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "clear_frame"}));
    assert_eq!(st["handles"].as_array().unwrap().len(), 0);
    assert_eq!(st["nodeCount"], 0);
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(st["nodeCount"], 11);
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["handles"].as_array().unwrap().len(), 11);
}

#[test]
fn paste_properties_applies_only_the_ticked_parts() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let a = st["figures"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": a}));
    s.command(&json!({"op": "figure", "scale": 3.0, "angle": 30.0}));
    // bend the right arm of A
    let st = s.state();
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 40.0, "y": y + 40.0}));
    s.command(&json!({"op": "up"}));
    s.command(&json!({"op": "select", "layer": a}));
    let st = s.command(&json!({"op": "copy_props"}));
    assert!(st["propsFrom"].is_string());
    let st = s.command(&json!({"op": "add_figure"}));
    let b = st["selected"]["layer"].as_str().unwrap().to_string();
    // scale only
    let st = s.command(&json!({"op": "paste_props", "position": false, "scale": true, "rotation": false, "color": false, "filters": false, "layering": false, "nodes": false}));
    assert_eq!(st["selected"]["scale"], 3.0);
    assert_eq!(st["selected"]["figAngle"], 0.0);
    // then the joints too
    s.command(&json!({"op": "paste_props", "position": false, "scale": false, "rotation": true, "color": false, "filters": false, "layering": false, "nodes": true}));
    let p = s.project();
    let fa = p.root().unwrap().layer(&a).unwrap().as_figure().unwrap();
    let fb = p.root().unwrap().layer(&b).unwrap().as_figure().unwrap();
    let mut r = barkfart_core::eval::Resolver::new(p);
    for (na, nb) in fa.nodes().iter().zip(fb.nodes()) {
        assert!((r.f(&na.rotation, 0.0) - r.f(&nb.rotation, 0.0)).abs() < 1e-3, "{}", na.id);
    }
}

#[test]
fn figure_editor_reorders_and_pastes_branches() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let order = |s: &Session| -> Vec<String> {
        let f = s.project().root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
        f.draw_sequence().into_iter().map(|i| f.nodes()[i].id.clone()).collect()
    };
    s.command(&json!({"op": "build", "layer": fig}));
    s.command(&json!({"op": "select", "layer": fig, "node": "armR1"}));
    let before = order(&s);
    // the arm alone to the very back, then the whole arm branch to the front
    s.command(&json!({"op": "reorder", "branch": false, "to": -2}));
    assert_eq!(order(&s)[0], "armR1");
    s.command(&json!({"op": "reorder", "branch": true, "to": 2}));
    let o = order(&s);
    assert_eq!(&o[o.len() - 2..], ["armR1", "armR2"]);
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(order(&s), before);
    // copy the arm branch and paste it onto the head
    let st = s.command(&json!({"op": "copy_branch", "branch": true}));
    assert_eq!(st["branchCopied"], true);
    s.command(&json!({"op": "select", "layer": fig, "node": "head"}));
    let st = s.command(&json!({"op": "paste_branch"}));
    assert_eq!(st["handles"].as_array().unwrap().len(), 13);
    let f = s.project().root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
    let pasted = st["selected"]["node"].as_str().unwrap();
    assert_eq!(f.node(pasted).unwrap().parent.as_deref(), Some("head"));
}

#[test]
fn rotate_cameras_turns_every_frame() {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "rotate_cameras"}));
    assert_eq!(st["cam"]["rotation"], 90.0);
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(st["cam"]["rotation"], 90.0);
    let st = s.command(&json!({"op": "undo"}));
    assert!(st["cam"].is_null() || st["cam"]["rotation"] == 0.0);
}

#[test]
fn delete_one_segment_keeps_its_children_pointing_the_same_way() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let world = |s: &Session, id: &str| {
        let p = s.project();
        let f = p.root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
        let pose = barkfart_core::eval::FigurePose::compute(f, 0.0, &mut barkfart_core::eval::Resolver::new(p));
        pose.angle[f.index_of(id).unwrap()]
    };
    let before = world(&s, "armR2");
    s.command(&json!({"op": "build", "layer": fig}));
    s.command(&json!({"op": "select", "layer": fig, "node": "armR1"}));
    let st = s.command(&json!({"op": "delete_joint"}));
    assert_eq!(st["selected"]["node"], "torso");
    let f = s.project().root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
    assert!(f.node("armR1").is_none());
    assert_eq!(f.node("armR2").unwrap().parent.as_deref(), Some("torso"));
    assert!((world(&s, "armR2") - before).abs() < 1e-6);
    s.command(&json!({"op": "undo"}));
    assert!(s.project().root().unwrap().layer(&fig).unwrap().as_figure().unwrap().node("armR1").is_some());
}

/// View options "Outline selection": the selected figure's shapes, fattened
/// in the outline colour, then the figure again on top. Nothing for a layer
/// that is not there.
#[test]
fn selection_outline_follows_the_selected_figure() {
    let s = session();
    let scene = barkfart_core::eval::evaluate(s.project(), &s.project().root_symbol, 0.0);
    let own = barkfart_core::tess::tessellate(&scene, None).triangle_count();
    let out = barkfart_core::tess::selection_outline(&scene, "L2", 2.0, 0xff2d92);
    assert!(out.root().is_some_and(|r| !r.empty()), "layered, so the stage can put it over the scene");
    assert!(out.triangle_count() > 0);
    let magenta = out.batches.iter().flat_map(|b| b.rgba.iter()).filter(|c| **c >> 8 == 0xff2d92).count();
    assert!(magenta > 0, "the band is drawn in the outline colour");
    assert!(out.triangle_count() < own * 4, "only the selected figure, not the whole scene");
    assert_eq!(barkfart_core::tess::selection_outline(&scene, "nope", 2.0, 0xff2d92).triangle_count(), 0);
}

/// Animation Tools "Is looping animation": with tweening on, the last frame's
/// in-betweens tween back into frame 1; off (or tweening off), it holds.
#[test]
fn looping_tween_blends_the_last_frame_into_the_first() {
    let mut s = two_poses();
    let fps = s.project().settings.fps as f64;
    let x_at = |s: &Session, t: f64| {
        let p = s.project();
        let l = p.root().unwrap().layer("L2").unwrap();
        barkfart_core::eval::Resolver::new(p).f(&l.transform.x, t)
    };
    let (x0, x1) = (x_at(&s, 0.0), x_at(&s, 1.0 / fps));
    assert!((x1 - x0).abs() > 50.0, "the two frames differ: {x0} {x1}");
    let mid = 1.5 / fps;
    s.command(&json!({"op": "animation", "tweening": true}));
    assert_eq!(x_at(&s, mid), x1, "no looping: the last frame holds");
    let st = s.command(&json!({"op": "animation", "loop": true}));
    assert_eq!(st["loopTween"], true);
    assert!((x_at(&s, mid) - (x0 + x1) / 2.0).abs() < 0.5, "halfway back to frame 1: {}", x_at(&s, mid));
    assert_eq!(x_at(&s, 1.0 / fps), x1, "the last frame itself is unchanged");
    s.command(&json!({"op": "animation", "tweening": false}));
    assert_eq!(x_at(&s, mid), x1, "looping needs tweening on");
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(x_at(&s, mid), x1, "undone");
    assert!(barkfart_core::codec::encode(s.project()).contains("loopTween") == false);
}

/// Frame Tools "Slow Motion Tweening": this frame gets its own number of
/// in-between pictures at the usual picture rate, so it plays longer and
/// smoother; only with tweening on.
#[test]
fn slow_motion_tweening_stretches_one_frame() {
    use barkfart_core::timing::Schedule;
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true, "tweened": 1}));
    let (fps, rate) = (s.project().settings.fps as f64, s.project().settings.sample_rate());
    let before = Schedule::build(s.project()).total;
    assert!((before - 2.0 / fps).abs() < 1e-9);
    let st = s.command(&json!({"op": "frame_slow", "n": 11}));
    assert_eq!(st["undo"], "Slow motion");
    assert_eq!(st["frameSlow"], 11);
    assert_eq!(st["frameMarks"][0][3], true);
    let sc = Schedule::build(s.project());
    assert!((sc.total - (12.0 / rate + 1.0 / fps)).abs() < 1e-9, "frame 0 lasts 12 pictures: {}", sc.total);
    // twelve evenly spaced steps through frame 0, then frame 1
    let ts: Vec<f64> = (0..12).map(|k| sc.time_at((k as f64 + 0.5) / rate, rate)).collect();
    for (k, t) in ts.iter().enumerate() {
        assert!((t - k as f64 / 12.0 / fps).abs() < 1e-9, "picture {k}: {t}");
    }
    assert!((sc.time_at(12.5 / rate, rate) - 1.0 / fps).abs() < 1e-9);
    // only frame 0: frame 1 has no key of its own
    s.command(&json!({"op": "frame", "i": 1}));
    assert_eq!(s.state()["frameSlow"], 0);
    // tweening off: no slow motion
    s.command(&json!({"op": "animation", "tweening": false}));
    assert!((Schedule::build(s.project()).total - 2.0 / fps).abs() < 1e-9);
    s.command(&json!({"op": "undo"}));
    assert!(codec::encode(s.project()).contains("slowMotion"), "saved");
    s.command(&json!({"op": "undo"}));
    assert!((Schedule::build(s.project()).total - before).abs() < 1e-9, "one undo step");
}

/// Frame Tools "Expand gradient": the frame background's fade stretches
/// down past the stage, so its middle is much closer to the gradient colour.
#[test]
fn expand_gradient_stretches_the_background_fade() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "frame_bg", "color": 0xffffffffu32, "gradient": 0x000000ffu32, "useGradient": true}));
    let bg = |s: &Session| {
        let p = s.project();
        let scene = barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0);
        let barkfart_core::eval::Item::Group(g) = &scene.root.children[0] else { panic!("background first") };
        let barkfart_core::eval::Item::Shape(sh) = &g.children[0] else { panic!() };
        sh.gradient.clone().expect("a gradient")
    };
    let plain = format!("{:?}", bg(&s));
    let st = s.command(&json!({"op": "frame_bg", "expand": true}));
    assert_eq!(st["frameBg"]["expand"], true);
    assert_eq!(st["undo"], "Background colour");
    let expanded = format!("{:?}", bg(&s));
    assert_ne!(plain, expanded, "the fade's end moved");
    assert!(codec::encode(s.project()).contains("bgExpand"), "saved");
    s.command(&json!({"op": "insert_frame"}));
    let st = s.state();
    assert_eq!(st["frameBg"]["expand"], true, "a new frame copies this one's background, expansion included");
    assert_eq!(st["frameBg"]["useGradient"], true);
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(format!("{:?}", bg(&s)), plain, "one undo step");
}

/// Stickfigure Tools, long-press Paste: "Paste with layering" puts the new
/// figure where the original stood among the frame's figures; plain paste
/// puts it on top.
#[test]
fn paste_with_layering_keeps_the_stacking_place() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "add_figure"}));
    s.command(&json!({"op": "add_figure"}));
    let order = |s: &Session| -> Vec<String> { s.project().root().unwrap().layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect() };
    let figs = order(&s);
    assert!(figs.len() >= 3);
    let bottom = figs[0].clone();
    let root = s.project().root().unwrap().layer(&bottom).unwrap().as_figure().unwrap().nodes()[0].id.clone();
    s.command(&json!({"op": "select", "layer": bottom, "node": root}));
    s.command(&json!({"op": "copy_figure"}));
    s.command(&json!({"op": "paste_figure"}));
    let plain = order(&s);
    assert!(!figs.contains(plain.last().unwrap()), "a plain paste goes on top");
    s.command(&json!({"op": "undo"}));
    assert_eq!(order(&s), figs);
    s.command(&json!({"op": "paste_figure", "layering": true}));
    let layered = order(&s);
    assert_eq!(layered.len(), figs.len() + 1);
    assert!(!figs.contains(&layered[0]), "the copy of the bottom figure is at the bottom: {layered:?}");
    assert_eq!(&layered[1..], &figs[..]);
}

/// View options "Show filters": None drops every filter from the stage
/// picture, No blur only the blurs; the project keeps them all.
#[test]
fn show_filters_strips_effects_from_the_stage_only() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "filter", "blur": 8.0}));
    s.command(&json!({"op": "filter", "outline": 0xff0000ffu32, "outlineAmount": 1.0}));
    let p = s.project();
    let list = || barkfart_core::tess::layered(&barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0), None);
    let tys = |l: &barkfart_core::tess::DrawList| -> Vec<String> { l.isolates.iter().flat_map(|i| i.effects.iter().map(|e| e.ty.clone())).collect() };
    let all = tys(&list());
    assert!(all.iter().any(|t| t == barkfart_core::fx::BLUR) && all.iter().any(|t| t == barkfart_core::fx::OUTLINE), "{all:?}");
    let no_blur = tys(&list().without_effects(|ty| ty != barkfart_core::fx::BLUR && ty != barkfart_core::fx::MOTION));
    assert!(!no_blur.iter().any(|t| t == barkfart_core::fx::BLUR) && no_blur.iter().any(|t| t == barkfart_core::fx::OUTLINE));
    assert!(tys(&list().without_effects(|_| false)).is_empty());
    assert_eq!(tys(&list()), all, "the project still has them");
}

/// Library Tools: the project's figures with their frame counts; rename one
/// by id; delete one from every frame (one undo step brings it back).
#[test]
fn library_lists_renames_and_deletes_figures() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "add_figure"}));
    let lib = st["library"].as_array().unwrap().clone();
    assert_eq!(lib.len(), 2, "{lib:?}");
    assert_eq!(lib[0]["kind"], "figure");
    assert_eq!(lib[0]["frames"], 2, "the starter figure is on both frames");
    assert_eq!(lib[1]["frames"], 1, "the new one only on this frame");
    let id = lib[1]["layer"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "rename", "layer": id, "name": "Hero"}));
    assert_eq!(st["library"][1]["name"], "Hero");
    let before = codec::encode(s.project());
    let st = s.command(&json!({"op": "library_delete", "layer": id}));
    assert_eq!(st["undo"], "Delete from project");
    assert_eq!(st["library"].as_array().unwrap().len(), 1);
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before);
}

/// Segment Tools "Use segment color": a segment with its own colour keeps it
/// when the stickfigure colour changes; turning it off brings back the
/// figure colour. Saved and undoable.
#[test]
fn segments_with_their_own_colour_ignore_the_figure_colour() {
    let mut s = session();
    let col = |s: &Session, n: &str| {
        let p = s.project();
        let f = p.root().unwrap().layer("L2").unwrap().as_figure().unwrap();
        f.node(n).unwrap().color.value.color().to_rgba8888()
    };
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.command(&json!({"op": "segment", "color": 0xff0000ffu32}));
    assert_eq!(st["selected"]["ownColor"], true);
    s.command(&json!({"op": "figure", "color": 0x00ff00ffu32}));
    assert_eq!(col(&s, "armR1"), 0xff0000ff, "own colour kept");
    assert_eq!(col(&s, "armR2"), 0x00ff00ff, "the others follow the figure");
    assert_eq!(s.state()["selected"]["figColor"], 0x00ff00ffu32);
    let st = s.command(&json!({"op": "segment", "ownColor": false}));
    assert_eq!(st["selected"]["ownColor"], false);
    assert_eq!(col(&s, "armR1"), 0x00ff00ff, "back to the figure colour");
    s.command(&json!({"op": "undo"}));
    assert_eq!(col(&s, "armR1"), 0xff0000ff);
    let text = codec::encode(s.project());
    assert!(text.contains("ownColor") && text.contains("figureColor"));
    let back = codec::decode(&text).unwrap();
    assert_eq!(codec::encode(&back), text, "round trip");
}

/// Stickfigure Tools "Tween stickfigure colors" and "Keep stickfigure
/// during tween" (both on by default, as Barkfart always behaved).
#[test]
fn tween_colours_and_keep_during_tween() {
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true}));
    let fps = s.project().settings.fps as f64;
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.state();
    assert_eq!(st["selected"]["tweenColors"], true);
    assert_eq!(st["selected"]["keepInTween"], true);
    let st = s.command(&json!({"op": "figure", "tweenColors": false}));
    assert_eq!(st["selected"]["tweenColors"], false);
    assert!(codec::encode(s.project()).contains("colorsJump"));
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["selected"]["tweenColors"], true);

    // keep during tween: the figure is not on frame 2
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "delete_here"}));
    let drawn = |s: &Session, t: f64| {
        let p = s.project();
        barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None).triangle_count()
    };
    let with = drawn(&s, 0.5 / fps);
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.command(&json!({"op": "figure", "keepInTween": false}));
    assert_eq!(st["selected"]["keepInTween"], false);
    assert!(drawn(&s, 0.5 / fps) < with, "gone during the in-betweens");
    assert_eq!(drawn(&s, 0.0), with, "still on its own frame");
    assert!(codec::encode(s.project()).contains("vanishInTween"));
}

/// Camera Tools "Lock figure to camera": the locked figure keeps its place
/// and size in the shot when the camera zooms; without "Rotate and scale
/// with camera" the zoom still scales it. Unlock puts it back.
#[test]
fn lock_figure_to_camera() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "add_camera"}));
    let fig = s.project().root().unwrap().layers.iter().find(|l| l.as_figure().is_some()).unwrap().id.clone();
    let width = |s: &Session| {
        let p = s.project();
        let xy: Vec<f32> = barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0), None).batches.iter().flat_map(|b| b.xy.iter().step_by(2).copied().collect::<Vec<_>>()).collect();
        xy.iter().cloned().fold(f32::MIN, f32::max) - xy.iter().cloned().fold(f32::MAX, f32::min)
    };
    let w0 = width(&s);
    let st = s.command(&json!({"op": "camera_lock", "layer": fig, "on": true}));
    assert_eq!(st["undo"], "Lock to camera");
    assert_eq!(st["cameraLocks"][0]["layer"], fig.as_str());
    assert_eq!(st["cameraLocks"][0]["parallax"], 1.0);
    assert!((width(&s) - w0).abs() < 0.5, "kept in place");
    s.command(&json!({"op": "camera", "zoom": 2.0}));
    assert!((width(&s) - w0).abs() < 0.5, "a zoom does not change it in the shot: {} {w0}", width(&s));
    let st = s.command(&json!({"op": "camera_lock_set", "layer": fig, "rotate": false}));
    assert_eq!(st["cameraLocks"][0]["rotate"], false);
    assert!(width(&s) > w0 * 1.5, "now the zoom scales it: {}", width(&s));
    let st = s.command(&json!({"op": "camera_lock_set", "layer": fig, "parallax": 0.5}));
    assert_eq!(st["cameraLocks"][0]["parallax"], 0.5);
    let st = s.command(&json!({"op": "camera_lock", "layer": fig, "on": false}));
    assert_eq!(st["cameraLocks"].as_array().unwrap().len(), 0);
    assert!(codec::encode(s.project()).contains("\"parallax\"") == false, "unlocked: no parallax left");
}

/// Camera Tools "Widescreen effect" and "Wobble camera": per frame, only in
/// the played / exported picture (the editing view is untouched).
#[test]
fn widescreen_and_wobble_are_per_frame_playback_effects() {
    let mut s = two_poses();
    s.command(&json!({"op": "add_camera"}));
    let p0 = s.project().clone();
    let shot = |s: &Session, t: f64| {
        let p = s.project();
        barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None).batches.iter().flat_map(|b| b.xy.clone()).collect::<Vec<f32>>()
    };
    let stage = |s: &Session, t: f64| {
        let p = s.project();
        barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate_with(p, &p.root_symbol, t, false, false), None).batches.iter().flat_map(|b| b.xy.clone()).collect::<Vec<f32>>()
    };
    let fps = p0.settings.fps as f64;
    let (a0, e0) = (shot(&s, 0.0), stage(&s, 0.0));
    let st = s.command(&json!({"op": "camera_fx", "widescreen": true}));
    assert_eq!(st["undo"], "Camera effects");
    assert_eq!(st["camFx"]["widescreen"], true);
    assert!(shot(&s, 0.0).len() > a0.len(), "bars drawn");
    assert_eq!(stage(&s, 0.0), e0, "not in the editing view");
    assert_eq!(shot(&s, 1.0 / fps), {
        let mut s2 = Session::new(Document::new(p0.clone()));
        s2.command(&json!({"op": "frame", "i": 1}));
        shot(&s2, 1.0 / fps)
    }, "only this frame");
    s.command(&json!({"op": "camera_fx", "widescreen": false}));
    let calm = shot(&s, 0.3 / fps);
    s.command(&json!({"op": "camera_fx", "wobbleXY": 20.0, "wobbleRot": 2.0}));
    let st = s.state();
    assert_eq!(st["camFx"]["wobbleXY"], 20.0);
    assert_eq!(st["camFx"]["wobbleSpeed"], 3.0);
    assert_ne!(shot(&s, 0.3 / fps), calm, "it shakes");
    assert_eq!(stage(&s, 0.0), e0);
    let text = codec::encode(s.project());
    assert!(text.contains("wobbleXY") && text.contains("widescreen"));
}

/// Camera Tools "Auto-camera": the camera tweens from the start frame's to
/// the end frame's across the frames between, whatever they say, and the
/// span follows its frames when one is inserted inside it.
#[test]
fn auto_camera_tweens_across_frames() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "add_camera"}));
    for _ in 0..3 {
        s.command(&json!({"op": "insert_frame"}));
    }
    let fps = s.project().settings.fps as f64;
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "camera", "zoom": 2.0}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "camera", "zoom": 5.0}));
    let zoom = |s: &Session, f: f64| {
        let p = s.project();
        let c = p.root().unwrap().layers.iter().find_map(|l| if let barkfart_core::model::LayerKind::Camera(c) = &l.kind { Some(c) } else { None }).unwrap();
        barkfart_core::eval::camera_values(c, f / fps, &mut barkfart_core::eval::Resolver::new(p)).2
    };
    assert_eq!(zoom(&s, 1.0), 5.0);
    s.command(&json!({"op": "frame", "i": 0}));
    let st = s.command(&json!({"op": "auto_camera", "end": 3, "ease": 1}));
    assert_eq!(st["undo"], "Auto-camera");
    assert_eq!(st["autoCam"]["end"], 3);
    assert_eq!(st["frameMarks"][0][4], true);
    assert!((zoom(&s, 1.0) - (1.0 + 1.0 / 3.0)).abs() < 1e-4, "frame 2 follows the tween, not its own 5: {}", zoom(&s, 1.0));
    assert_eq!(zoom(&s, 3.0), 2.0);
    // a frame inserted inside the span: it still ends on the same (moved) frame
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(s.state()["autoCam"]["end"], 4);
    assert_eq!(zoom(&s, 4.0), 2.0);
    // delete it
    let st = s.command(&json!({"op": "auto_camera", "end": null}));
    assert!(st["autoCam"].is_null());
    assert_eq!(st["frameMarks"][0][4], false);
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["autoCam"]["end"], 4);
    assert!(codec::encode(s.project()).contains("autoCamEnd"));
}

/// Figure editor "Add polyfill...": taps on joints add polynodes and the
/// area fills; remove the last one, recolour, finish, delete; undoable.
#[test]
fn polyfill_tool_fills_between_joints() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "torso"}));
    s.command(&json!({"op": "build", "layer": "L2"}));
    let st = s.command(&json!({"op": "polyfill_new"}));
    assert_eq!(st["undo"], "Add polyfill");
    assert_eq!(st["polyfill"]["count"], 0);
    let tris = |s: &Session| {
        let p = s.project();
        barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0), None).triangle_count()
    };
    let before = tris(&s);
    let hs: Vec<_> = s.tools.handles(s.project(), 0.0).into_iter().filter(|h| h.layer == "L2").collect();
    for h in hs.iter().filter(|h| ["armR1", "armR2", "torso"].contains(&h.node.as_str())) {
        s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
        s.command(&json!({"op": "up"}));
    }
    let st = s.state();
    assert_eq!(st["polyfill"]["count"], 3, "three polynodes");
    assert_eq!(st["undo"], "Add polynode");
    assert!(tris(&s) > before, "the area is filled");
    let st = s.command(&json!({"op": "polyfill_undo"}));
    assert_eq!(st["polyfill"]["count"], 2);
    let st = s.command(&json!({"op": "polyfill_color", "color": 0xff0000ffu32}));
    assert_eq!(st["polyfill"]["color"], 0xff0000ffu32);
    let st = s.command(&json!({"op": "polyfill_edit", "id": null}));
    assert!(st["polyfill"].is_null());
    assert_eq!(st["polyfills"].as_array().unwrap().len(), 1);
    let id = st["polyfills"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "polyfill_edit", "id": id}));
    let st = s.command(&json!({"op": "polyfill_delete"}));
    assert_eq!(st["polyfills"].as_array().unwrap().len(), 0);
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["polyfills"].as_array().unwrap().len(), 1);
}

/// Figure editor "Add connector...": the next joint tapped is the endpoint;
/// the connector joint sits on the line between, halfway, and Connector
/// Tools moves it along that line.
#[test]
fn connector_sits_between_two_joints() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "torso"}));
    s.command(&json!({"op": "build", "layer": "L2"}));
    let st = s.command(&json!({"op": "connect_start"}));
    assert_eq!(st["connecting"], true);
    let at = |s: &Session, n: &str| {
        let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == "L2" && h.node == n).unwrap();
        (h.x, h.y)
    };
    let (a, b) = (at(&s, "torso"), at(&s, "armR2"));
    s.command(&json!({"op": "down", "x": b.0, "y": b.1, "r": 4}));
    s.command(&json!({"op": "up"}));
    let st = s.state();
    assert_eq!(st["undo"], "Add connector");
    assert_eq!(st["connecting"], false);
    assert_eq!(st["selected"]["connector"]["to"], "armR2");
    let id = st["selected"]["node"].as_str().unwrap().to_string();
    let c = at(&s, &id);
    assert!((c.0 - (a.0 + b.0) / 2.0).abs() < 1.0 && (c.1 - (a.1 + b.1) / 2.0).abs() < 1.0, "halfway: {c:?} between {a:?} and {b:?}");
    s.command(&json!({"op": "connector_set", "percent": 0.25}));
    let c = at(&s, &id);
    assert!((c.0 - (a.0 + (b.0 - a.0) * 0.25)).abs() < 1.0, "a quarter of the way: {c:?}");
    // it follows when the endpoint moves
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert!(s.state()["selected"]["connector"].is_null() || s.state()["selected"]["node"] != id.as_str());
}

/// Figure editor "Save stickfigure": the figure as a Stick Nodes .nodes file
/// that reads back with the same joints, in this frame's pose.
#[test]
fn save_stickfigure_writes_a_nodes_file() {
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let (name, bytes) = s.figure_file().expect("a file");
    assert!(!name.is_empty());
    let fig = barkfart_core::legacy::read_nodes(&bytes).expect("reads back");
    let joints = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().nodes().len();
    assert_eq!(fig.nodes.len(), joints);
    // importing it gives a figure again
    let n0 = s.state()["library"].as_array().unwrap().len();
    let mut p = s.project().clone();
    barkfart_core::import::import_file(&mut p, "Saved.nodes", &bytes).expect("imports");
    let mut s2 = Session::new(Document::new(p));
    assert_eq!(s2.command(&json!({"op": "state"}))["library"].as_array().unwrap().len(), n0 + 1);
}

/// Textfield Tools "Locked (to camera)": the text keeps its place in the shot.
#[test]
fn textfield_locks_to_the_camera() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "add_camera"}));
    let st = s.command(&json!({"op": "add_textfield"}));
    let id = st["textfield"]["layer"].as_str().unwrap().to_string();
    assert_eq!(st["textfield"]["cameraLocked"], false);
    let st = s.command(&json!({"op": "camera_lock", "layer": id, "on": true}));
    assert_eq!(st["textfield"]["cameraLocked"], true);
    let shot = |s: &Session| {
        let p = s.project();
        let scene = barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0);
        format!("{:?}", scene.root.children.iter().filter_map(|i| if let barkfart_core::eval::Item::Group(g) = i { g.source.ends_with(&id).then(|| g.clone()) } else { None }).collect::<Vec<_>>())
    };
    let before = shot(&s);
    assert!(before.contains("Text"), "the textfield is drawn: {before}");
    s.command(&json!({"op": "camera", "zoom": 1.7, "rotation": 20.0}));
    assert_eq!(shot(&s), before, "same place in the shot");
}

/// Figure editor "Trace frame / stickfigure": a faint copy to draw over
/// (one figure, or a whole frame); cleared on Done, never saved.
#[test]
fn trace_ghost_of_a_frame_or_a_figure() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "build", "layer": "L2"}));
    let st = s.command(&json!({"op": "trace", "frame": 1}));
    assert_eq!(st["trace"]["frame"], 1);
    let p = s.project();
    let scene = barkfart_core::eval::evaluate_with(p, &p.root_symbol, 1.0 / p.settings.fps as f64, false, false);
    let all = barkfart_core::tess::trace_ghost(&scene, None, 0x5a6270, 0.35);
    let one = barkfart_core::tess::trace_ghost(&scene, Some("L2"), 0x5a6270, 0.35);
    assert!(all.triangle_count() >= one.triangle_count() && one.triangle_count() > 0);
    assert!(all.batches.iter().flat_map(|b| b.rgba.iter()).all(|c| c >> 8 == 0x5a6270), "all grey");
    assert_eq!(barkfart_core::tess::trace_ghost(&scene, Some("nope"), 0, 0.3).triangle_count(), 0);
    let before = codec::encode(s.project());
    let st = s.command(&json!({"op": "build_done"}));
    assert!(st["trace"].is_null());
    assert_eq!(codec::encode(s.project()), before, "nothing saved");
}

/// Modify branch: replace a colour, increment thickness / length (ignoring
/// zeros), gradient colour, use segment colour; each one undo step.
#[test]
fn modify_branch_replace_and_increment() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "segment", "color": 0x112233ffu32}));
    let node = |s: &Session, n: &str| s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node(n).unwrap().clone();
    let other = node(&s, "armR2").color.value.color().to_rgba8888();
    let st = s.command(&json!({"op": "branch_edit", "replaceFrom": 0x112233ffu32, "replaceTo": 0xff0000ffu32}));
    assert_eq!(st["undo"], "Modify branch");
    assert_eq!(node(&s, "armR1").color.value.color().to_rgba8888(), 0xff0000ff);
    assert_eq!(node(&s, "armR2").color.value.color().to_rgba8888(), other, "only that colour");
    let (t1, l2) = (node(&s, "armR1").thickness.value.f(), node(&s, "armR2").length.value.f());
    s.command(&json!({"op": "branch_edit", "addThickness": 4.0, "addLength": 10.0}));
    assert_eq!(node(&s, "armR1").thickness.value.f(), t1 + 4.0);
    assert_eq!(node(&s, "armR2").length.value.f(), l2 + 10.0);
    s.command(&json!({"op": "undo"}));
    assert_eq!(node(&s, "armR1").thickness.value.f(), t1);
    s.command(&json!({"op": "branch_edit", "ownColor": false}));
    assert!(!node(&s, "armR1").own_color);
}

/// Segment Tools for trapezoids: end thickness and rounded ends.
#[test]
fn trapezoid_end_thickness_and_rounding() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "segment", "shape": "trapezoid"}));
    let tris = |s: &Session| {
        let p = s.project();
        barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, 0.0), None).triangle_count()
    };
    let flat = tris(&s);
    let st = s.command(&json!({"op": "segment", "trapRatio": 0.3}));
    assert_eq!(st["selected"]["trapRatio"], 0.3);
    let st = s.command(&json!({"op": "segment", "trapRoundStart": true, "trapRoundEnd": true}));
    assert_eq!(st["selected"]["trapRoundEnd"], true);
    assert!(tris(&s) > flat, "rounded ends add triangles");
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["selected"]["trapRoundEnd"], false);
}

/// Convert to movieclip: the figure becomes a movieclip playing its own
/// animation in its place (the picture does not change); Break apart turns
/// it back into figures posed as on this frame.
#[test]
fn convert_to_movieclip_and_break_apart() {
    let mut s = two_poses();
    let fps = s.project().settings.fps as f64;
    let (a0, a1) = (picture_at(&s, 0.0), picture_at(&s, 1.0 / fps));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.command(&json!({"op": "to_movieclip"}));
    assert_eq!(st["undo"], "Convert to movieclip");
    let lib = st["library"].as_array().unwrap().clone();
    let mc = lib.iter().find(|m| m["kind"] == "movieclip").expect("a movieclip");
    assert!(lib.iter().all(|m| m["layer"] != "L2"), "the figure moved into it");
    assert_eq!(picture_at(&s, 0.0).len(), a0.len());
    assert_eq!(picture_at(&s, 1.0 / fps).len(), a1.len(), "it still animates");
    let id = mc["layer"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "break_apart", "layer": id}));
    assert_eq!(st["undo"], "Break apart");
    assert!(st["library"].as_array().unwrap().iter().any(|m| m["kind"] == "figure"));
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert!(s.state()["library"].as_array().unwrap().iter().any(|m| m["layer"] == "L2"));
}

/// A movieclip (or sprite) has one main handle: dragging it moves it.
#[test]
fn movieclips_can_be_dragged() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "to_movieclip"}));
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node.is_empty()).expect("a movieclip handle");
    let before = picture_at(&s, 0.0);
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 6}));
    s.command(&json!({"op": "move", "x": h.x + 50.0, "y": h.y}));
    let st = s.command(&json!({"op": "up"}));
    assert!(st["undo"].as_str().is_some_and(|u| u.starts_with("Move")), "{}", st["undo"]);
    let after = picture_at(&s, 0.0);
    assert_eq!(before.len(), after.len());
    assert!((after[0] - before[0] - 50.0).abs() < 0.5, "moved right by 50");
    s.command(&json!({"op": "undo"}));
    assert_eq!(picture_at(&s, 0.0), before);
}

/// Movieclip Tools: a selected movieclip's loop mode, speed and offset.
#[test]
fn movieclip_tools_set_playback() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "to_movieclip"}));
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node.is_empty()).unwrap();
    s.command(&json!({"op": "select", "layer": h.layer, "node": ""}));
    let st = s.state();
    assert_eq!(st["movieclip"]["loop"], "loop");
    assert_eq!(st["movieclip"]["speed"], 1.0);
    let st = s.command(&json!({"op": "movieclip", "loop": "pingpong", "speed": 0.5, "offset": 0.1}));
    assert_eq!(st["undo"], "Movieclip");
    assert_eq!(st["movieclip"]["loop"], "pingpong");
    assert_eq!(st["movieclip"]["speed"], 0.5);
    assert_eq!(st["movieclip"]["offset"], 0.1);
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["movieclip"]["speed"], 1.0);
}

/// Movieclip editor: the movieclip's own frames with every tool; Finish
/// returns to the animation; a save in between still has the animation as
/// its root.
#[test]
fn movieclip_editor_edits_its_own_frames() {
    let mut s = two_poses();
    let main = s.project().root_symbol.clone();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "to_movieclip"}));
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node.is_empty()).unwrap();
    s.command(&json!({"op": "select", "layer": h.layer, "node": ""}));
    let st = s.command(&json!({"op": "edit_movieclip"}));
    assert_eq!(st["undo"], "Edit movieclip");
    assert!(st["editingClip"].is_string());
    let clip = s.project().root_symbol.clone();
    assert_ne!(clip, main);
    assert_eq!(st["frames"], 2, "the clip's own two frames");
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(s.state()["frames"], 3);
    // a save now still opens on the animation
    let bytes = s.save_copy().unwrap();
    let back = barkfart_core::codec::read_bark(&bytes).unwrap().project;
    assert_eq!(back.root_symbol, main);
    let st = s.command(&json!({"op": "movieclip_done"}));
    assert!(st["editingClip"].is_null());
    assert_eq!(s.project().root_symbol, main);
    assert!((s.project().symbol(&clip).unwrap().duration * s.project().settings.fps as f64 - 3.0).abs() < 1e-6, "the clip is three frames now");
}

/// Library Tools "Create new movieclip": an empty movieclip on this frame,
/// opened in the editor; figures added there belong to it.
#[test]
fn create_new_movieclip() {
    let mut s = two_poses();
    let st = s.command(&json!({"op": "new_movieclip"}));
    assert_eq!(st["undo"], "New movieclip");
    assert!(st["editingClip"].is_string());
    assert_eq!(st["frames"], 1);
    s.command(&json!({"op": "add_figure"}));
    assert_eq!(s.state()["library"].as_array().unwrap().len(), 1, "only the clip's own figure");
    let st = s.command(&json!({"op": "movieclip_done"}));
    let lib = st["library"].as_array().unwrap();
    assert!(lib.iter().any(|m| m["kind"] == "movieclip" && m["frames"] == 1), "{lib:?}");
    assert_eq!(st["frames"], 2, "back on the animation");
}

/// "Use frame tweening properties during repeat": while a repeat jumps back,
/// the repeating frame's in-betweens tween into the frame it jumps to; off,
/// the frame holds until the jump. Exports follow, slow motion included.
#[test]
fn repeat_tweens_into_the_frame_it_jumps_to() {
    use barkfart_core::timing::Schedule;
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true, "tweened": 3}));
    let fps = s.project().settings.fps as f64;
    let rate = s.project().settings.sample_rate();
    // frame 2 repeats back to frame 1 once
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "frame_repeat", "back": 1, "times": 1}));
    assert_eq!(st["frameRepeat"]["tween"], false);
    let x = |p: &barkfart_core::model::Project, t: f64, r: Option<barkfart_core::timing::Redirect>| {
        let l = p.root().unwrap().layer("L2").unwrap();
        barkfart_core::eval::Resolver::new(p).with_redirect(r).f(&l.transform.x, t)
    };
    let (x0, x1) = (x(s.project(), 0.0, None), x(s.project(), 1.0 / fps, None));
    let sch = Schedule::build(s.project());
    // the first pass of frame 2 is the second slot; in its middle the frame holds
    let mid = sch.slots[1].start + sch.slots[1].len / 2.0;
    assert_eq!(sch.time_at(mid, rate), 1.0 / fps, "holds until the jump");
    assert!(sch.redirect_at(mid).is_none());
    // on: it tweens into frame 1
    let st = s.command(&json!({"op": "frame_repeat", "back": 1, "times": 1, "tween": true}));
    assert_eq!(st["frameRepeat"]["tween"], true);
    let sch = Schedule::build(s.project());
    let t = sch.time_at(mid, rate);
    let r = sch.redirect_at(mid).expect("blends");
    let xm = x(s.project(), t, Some(r));
    assert!((xm - x1).abs() > 1.0 && (xm - x0).abs() < (x1 - x0).abs(), "between frame 2 and frame 1: {x0} {xm} {x1}");
    // the last pass of frame 2 (no jump after it) does not blend
    let last = sch.slots.last().unwrap();
    assert!(sch.redirect_at(last.start + last.len / 2.0).is_none());
    // exports: the in-betweens of the jumping pass carry the blend
    let pics = barkfart_core::export::pictures(&sch, 0, 2, rate, false);
    assert!(pics.iter().any(|p| p.2.is_some()));
    // slow motion gives its frame its own number of pictures in exports
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "frame_slow", "n": 9}));
    let sch = Schedule::build(s.project());
    let pics = barkfart_core::export::pictures(&sch, 0, 2, rate, false);
    let first = pics.iter().filter(|p| p.0 < 1.0 / fps - 1e-9).count();
    assert_eq!(first / 2, 10, "frame 1 plays twice, ten pictures each time: {}", first);
}

/// Save stickfigure with a Barkfart connector: it is saved where it sits
/// (every joint of the saved figure lands where it was, the connector too).
#[test]
fn saved_figure_keeps_connector_positions() {
    let mut s = session();
    s.command(&json!({"op": "select", "layer": "L2", "node": "torso"}));
    s.command(&json!({"op": "build", "layer": "L2"}));
    s.command(&json!({"op": "connect_start"}));
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.layer == "L2" && h.node == "armR2").unwrap();
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
    s.command(&json!({"op": "up"}));
    let points = |p: &barkfart_core::model::Project, layer: &str| {
        let f = p.root().unwrap().layer(layer).unwrap().as_figure().unwrap();
        let pose = barkfart_core::eval::pose::FigurePose::compute(f, 0.0, &mut barkfart_core::eval::Resolver::new(p));
        let mut v: Vec<(i64, i64)> = (0..pose.x.len()).map(|i| ((pose.x[i] - pose.x[0]).round() as i64, (pose.y[i] - pose.y[0]).round() as i64)).collect();
        v.sort();
        v
    };
    let want = points(s.project(), "L2");
    let (_, bytes) = s.figure_file().unwrap();
    let mut p = s.project().clone();
    barkfart_core::import::import_file(&mut p, "Saved.nodes", &bytes).unwrap();
    let new_layer = p.root().unwrap().layers.iter().rev().find(|l| l.as_figure().is_some()).unwrap().id.clone();
    let got = points(&p, &new_layer);
    let close = want.len() == got.len() && want.iter().zip(&got).all(|(a, b)| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1);
    assert!(close, "{got:?}\nvs\n{want:?}");
}

/// "Use gradient" on then off leaves the frame looking as before: no hidden
/// background key that the in-betweens fade through (user report 2026-11-22).
#[test]
fn use_gradient_on_and_off_leaves_no_trace() {
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true}));
    let fps = s.project().settings.fps as f64;
    let bg_at = |s: &Session, t: f64| {
        let p = s.project();
        let scene = barkfart_core::eval::evaluate(p, &p.root_symbol, t);
        scene.root.children.iter().find_map(|i| match i {
            barkfart_core::eval::Item::Group(g) if g.source == "background" => match &g.children[0] {
                barkfart_core::eval::Item::Shape(sh) => Some((sh.color.to_rgba8888(), sh.gradient.is_some())),
                _ => None,
            },
            _ => None,
        })
    };
    assert_eq!(bg_at(&s, 0.5 / fps), None, "no frame background to start with");
    s.command(&json!({"op": "frame_bg", "useGradient": true}));
    s.command(&json!({"op": "frame_bg", "useGradient": false}));
    let white = s.project().settings.background.to_rgba8888();
    for t in [0.0, 0.25 / fps, 0.5 / fps, 0.75 / fps, 1.0 / fps] {
        if let Some((c, _)) = bg_at(&s, t) {
            assert_eq!(c, white, "at {t}: the project background, not a fade to black");
        }
    }
    assert_eq!(s.state()["frameBg"]["useGradient"], false);
}

/// What differs between two panels (state()), without the undo labels.
fn diff(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                if path.is_empty() && ["undo", "redo", "canUndo", "canRedo", "dirty", "rev", "message"].contains(&k.as_str()) {
                    continue;
                }
                diff(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{path}/{k}"), out);
            }
        }
        _ if a != b => out.push(format!("{path}: {a} -> {b}")),
        _ => {}
    }
}

/// Runs a row's set-up: one command or a list. "@movieclip" turns the figure
/// into a movieclip and selects it; "@connector" makes a connector at the arm.
fn apply_setup(s: &mut Session, setup: &Option<Value>) {
    let list: Vec<Value> = match setup {
        Some(Value::Array(a)) => a.clone(),
        Some(c) => vec![c.clone()],
        None => vec![],
    };
    for c in &list {
        match c["op"].as_str() {
            Some("@movieclip") => {
                s.command(&json!({"op": "to_movieclip"}));
                let id = s.project().root().expect("root").layers.iter().find(|l| matches!(l.kind, barkfart_core::model::LayerKind::Symbol(_))).expect("a movieclip").id.clone();
                s.command(&json!({"op": "select", "layer": id, "node": ""}));
            }
            Some("@connector") => {
                s.command(&json!({"op": "build", "layer": "L2"}));
                s.command(&json!({"op": "connect_start"}));
                let (x, y) = joint_at(s, "armR2");
                s.command(&json!({"op": "down", "x": x, "y": y, "r": 4}));
                s.command(&json!({"op": "up"}));
            }
            _ => {
                s.command(c);
            }
        }
    }
}

/// The controls the on / off tests flip: (name, set-up before the first
/// picture (one command or a list), on, off).
fn toggle_rows() -> Vec<(&'static str, Option<Value>, Value, Value)> {
    vec![
        ("use gradient", None, json!({"op": "frame_bg", "useGradient": true}), json!({"op": "frame_bg", "useGradient": false})),
        ("expand gradient", Some(json!({"op": "frame_bg", "useGradient": true})), json!({"op": "frame_bg", "expand": true}), json!({"op": "frame_bg", "expand": false})),
        ("widescreen", None, json!({"op": "camera_fx", "widescreen": true}), json!({"op": "camera_fx", "widescreen": false})),
        ("wobble", None, json!({"op": "camera_fx", "wobbleXY": 8}), json!({"op": "camera_fx", "wobbleXY": 0})),
        ("wobble rotation", None, json!({"op": "camera_fx", "wobbleRot": 0.5}), json!({"op": "camera_fx", "wobbleRot": 0})),
        ("do not tween", None, json!({"op": "no_tween", "on": true}), json!({"op": "no_tween", "on": false})),
        ("pause", None, json!({"op": "frame_delay", "seconds": 0.5}), json!({"op": "frame_delay", "seconds": 0})),
        ("slow motion", None, json!({"op": "frame_slow", "n": 5}), json!({"op": "frame_slow", "n": 0})),
        ("tween colours", None, json!({"op": "figure", "tweenColors": false}), json!({"op": "figure", "tweenColors": true})),
        ("keep in tween", None, json!({"op": "figure", "keepInTween": false}), json!({"op": "figure", "keepInTween": true})),
        ("invert", None, json!({"op": "filter", "invert": true}), json!({"op": "filter", "invert": false})),
        ("motion blur", None, json!({"op": "filter", "motion": true}), json!({"op": "filter", "motion": false})),
        // the other stickfigure filters (each is one effect node keyed on the frame)
        ("blur", None, json!({"op": "filter", "blur": 6}), json!({"op": "filter", "blur": 0})),
        ("pixelate", None, json!({"op": "filter", "pixelate": 8}), json!({"op": "filter", "pixelate": 1})),
        ("hue shift", None, json!({"op": "filter", "hue": 90}), json!({"op": "filter", "hue": 0})),
        ("saturation", None, json!({"op": "filter", "saturation": 0.4}), json!({"op": "filter", "saturation": 1})),
        ("tint amount", Some(json!({"op": "filter", "tint": 0xff0000ffu32})), json!({"op": "filter", "tintAmount": 0.6}), json!({"op": "filter", "tintAmount": 0})),
        ("glow amount", None, json!({"op": "filter", "glowAmount": 0.7}), json!({"op": "filter", "glowAmount": 0})),
        ("shadow opacity", None, json!({"op": "filter", "shadowOpacity": 0.7}), json!({"op": "filter", "shadowOpacity": 0})),
        ("outline amount", Some(json!({"op": "filter", "outline": 0x00ff00ffu32, "outlineThickness": 6})), json!({"op": "filter", "outlineAmount": 1}), json!({"op": "filter", "outlineAmount": 0})),
        ("opacity", None, json!({"op": "filter", "opacity": 0.4}), json!({"op": "filter", "opacity": 1})),
        ("segment gradient", None, json!({"op": "segment", "gradient": true}), json!({"op": "segment", "gradient": false})),
        ("use segment colour", None, json!({"op": "segment", "ownColor": true}), json!({"op": "segment", "ownColor": false})),
        ("trapezoid rounded start", Some(json!({"op": "segment", "shape": "trapezoid"})), json!({"op": "segment", "trapRoundStart": true}), json!({"op": "segment", "trapRoundStart": false})),
        ("trapezoid rounded end", Some(json!({"op": "segment", "shape": "trapezoid"})), json!({"op": "segment", "trapRoundEnd": true}), json!({"op": "segment", "trapRoundEnd": false})),
        ("half arc", Some(json!({"op": "segment", "shape": "circle"})), json!({"op": "segment", "halfArc": 1}), json!({"op": "segment", "halfArc": 0})),
        ("textfield font", Some(json!({"op": "add_textfield"})), json!({"op": "textfield", "font": "Bangers"}), json!({"op": "textfield", "font": null})),
        ("lock to camera", None, json!({"op": "camera_lock", "layer": "L2", "on": true}), json!({"op": "camera_lock", "layer": "L2", "on": false})),
        ("auto-camera", None, json!({"op": "auto_camera", "end": 1, "ease": 1}), json!({"op": "auto_camera", "end": null})),
        // more of the app's switches
        ("lock joint", None, json!({"op": "pin", "v": true}), json!({"op": "pin", "v": false})),
        ("segment static", None, json!({"op": "segment", "static": true}), json!({"op": "segment", "static": false})),
        ("smart stretch", None, json!({"op": "segment", "smart": true}), json!({"op": "segment", "smart": false})),
        ("is stretchy", None, json!({"op": "node", "stretchy": true}), json!({"op": "node", "stretchy": false})),
        ("drag lock", None, json!({"op": "node", "dragLock": true}), json!({"op": "node", "dragLock": false})),
        ("angle lock", None, json!({"op": "segment", "angleLock": 1}), json!({"op": "segment", "angleLock": 0})),
        ("circle outline", Some(json!({"op": "segment", "shape": "circle"})), json!({"op": "segment", "outline": true}), json!({"op": "segment", "outline": false})),
        ("hollow circle", Some(json!({"op": "segment", "shape": "circle"})), json!({"op": "segment", "hollow": true}), json!({"op": "segment", "hollow": false})),
        ("right triangle", Some(json!({"op": "segment", "shape": "triangle"})), json!({"op": "segment", "rightTriangle": 1}), json!({"op": "segment", "rightTriangle": 0})),
        ("upside-down triangle", Some(json!({"op": "segment", "shape": "triangle"})), json!({"op": "segment", "upsideDown": true}), json!({"op": "segment", "upsideDown": false})),
        ("reverse gradient", Some(json!({"op": "segment", "gradient": true, "gradientColor": 0x00ff00ffu32})), json!({"op": "segment", "gradientReverse": true}), json!({"op": "segment", "gradientReverse": false})),
        ("sideways gradient", Some(json!({"op": "segment", "gradient": true, "gradientColor": 0x00ff00ffu32})), json!({"op": "segment", "gradientSideways": true}), json!({"op": "segment", "gradientSideways": false})),
        ("figure locked", None, json!({"op": "figure", "locked": true}), json!({"op": "figure", "locked": false})),
        ("visible in onion skin", None, json!({"op": "figure", "onion": false}), json!({"op": "figure", "onion": true})),
        ("flip figure", None, json!({"op": "flip_figure", "h": true}), json!({"op": "flip_figure", "h": true})),
        ("flip branch", None, json!({"op": "flip_branch", "h": true}), json!({"op": "flip_branch", "h": true})),
        ("layer order", Some(json!([{"op": "add_figure", "kind": "ball"}, {"op": "select", "layer": "L2", "node": "armR1"}])), json!({"op": "layer_order", "forward": true}), json!({"op": "layer_order", "forward": false})),
        ("nudge", None, json!({"op": "nudge", "dx": 5, "dy": -3}), json!({"op": "nudge", "dx": -5, "dy": 3})),
        ("turn figure", None, json!({"op": "figure", "turn": 45}), json!({"op": "figure", "turn": -45})),
        ("watermark", None, json!({"op": "watermark", "on": true, "text": "hi"}), json!({"op": "watermark", "on": false})),
        ("loop animation", None, json!({"op": "animation", "loop": true}), json!({"op": "animation", "loop": false})),
        ("silence frame", None, json!({"op": "frame_silence", "on": true}), json!({"op": "frame_silence", "on": false})),
        // from a value the control has to give back when it is ticked again
        ("segment colour kept while off", Some(json!({"op": "segment", "color": 0xff0000ffu32})), json!({"op": "segment", "ownColor": false}), json!({"op": "segment", "ownColor": true})),
        ("gradient colours kept while off", Some(json!({"op": "frame_bg", "color": 0x336699ffu32, "gradient": 0xff8800ffu32, "useGradient": true})), json!({"op": "frame_bg", "useGradient": false}), json!({"op": "frame_bg", "useGradient": true})),
        ("segment gradient colour kept while off", Some(json!({"op": "segment", "gradient": true, "gradientColor": 0x00ff00ffu32})), json!({"op": "segment", "gradient": false}), json!({"op": "segment", "gradient": true})),
        ("half arc kept while off", Some(json!([{"op": "segment", "shape": "circle"}, {"op": "segment", "halfArc": 1}])), json!({"op": "segment", "halfArc": 0}), json!({"op": "segment", "halfArc": 1})),
        ("textfield font kept while off", Some(json!([{"op": "add_textfield"}, {"op": "textfield", "font": "Bangers"}])), json!({"op": "textfield", "font": null}), json!({"op": "textfield", "font": "Bangers"})),
        ("pause kept while off", Some(json!({"op": "frame_delay", "seconds": 0.4})), json!({"op": "frame_delay", "seconds": 0}), json!({"op": "frame_delay", "seconds": 0.4})),
        ("motion blur distance kept while off", Some(json!([{"op": "filter", "motion": true}, {"op": "filter", "motionDistance": 60}])), json!({"op": "filter", "motion": false}), json!({"op": "filter", "motion": true})),
        // textfields, movieclips, polyfills and connectors
        ("textfield locked", Some(json!({"op": "add_textfield"})), json!({"op": "textfield", "locked": true}), json!({"op": "textfield", "locked": false})),
        ("textfield alignment", Some(json!({"op": "add_textfield"})), json!({"op": "textfield", "align": 2}), json!({"op": "textfield", "align": 1})),
        ("textfield text", Some(json!({"op": "add_textfield"})), json!({"op": "textfield", "text": "Hi there"}), json!({"op": "textfield", "text": "Hello"})),
        ("textfield scale", Some(json!({"op": "add_textfield"})), json!({"op": "textfield", "scale": 2.0}), json!({"op": "textfield", "scale": 1.0})),
        ("textfield colour", Some(json!([{"op": "add_textfield"}, {"op": "textfield", "color": 0xff0000ffu32}])), json!({"op": "textfield", "color": 0x0000ffffu32}), json!({"op": "textfield", "color": 0xff0000ffu32})),
        ("movieclip speed", Some(json!([{"op": "@movieclip"}])), json!({"op": "movieclip", "speed": 2.0}), json!({"op": "movieclip", "speed": 1.0})),
        ("movieclip offset", Some(json!([{"op": "@movieclip"}])), json!({"op": "movieclip", "offset": 0.5}), json!({"op": "movieclip", "offset": 0.0})),
        ("movieclip loop", Some(json!([{"op": "@movieclip"}, {"op": "movieclip", "loop": "loop"}])), json!({"op": "movieclip", "loop": "once"}), json!({"op": "movieclip", "loop": "loop"})),
        ("polyfill own colour", Some(json!([{"op": "build", "layer": "L2"}, {"op": "polyfill_new"}])), json!({"op": "polyfill_color", "own": false}), json!({"op": "polyfill_color", "own": true})),
        ("connector reversed", Some(json!([{"op": "@connector"}])), json!({"op": "connector_set", "reversed": true}), json!({"op": "connector_set", "reversed": false})),
        ("connector percent", Some(json!([{"op": "@connector"}])), json!({"op": "connector_set", "percent": 0.3}), json!({"op": "connector_set", "percent": 0.5})),
        ("camera lock kept while off", Some(json!([{"op": "camera", "zoom": 1.5, "rotation": 10}, {"op": "camera_lock", "layer": "L2", "on": true}])), json!({"op": "camera_lock", "layer": "L2", "on": false}), json!({"op": "camera_lock", "layer": "L2", "on": true})),
    ]
}

/// Switching a control on and back off must leave the animation looking as
/// before, in-betweens included (user report 2026-11-22: "Use gradient" left
/// a hidden key behind). Every on / off control of the frame, figure, segment,
/// filter and camera tools.
#[test]
fn toggling_a_control_on_and_off_changes_nothing() {
    let toggles = toggle_rows();
    let picture = |s: &Session, t: f64| -> Vec<u8> {
        let p = s.project();
        let list = barkfart_core::tess::layered(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
        let mut im = barkfart_core::fx::Image::new(160, 90, -640.0, 360.0, 0.125);
        for v in im.px.iter_mut() {
            *v = 1.0;
        }
        barkfart_core::fx::render(&list, &mut im);
        im.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
    };
    let mut failures = Vec::new();
    for (name, setup, on, off) in &toggles {
        let mut s = two_poses();
        s.command(&json!({"op": "animation", "tweening": true}));
        s.command(&json!({"op": "add_camera"}));
        s.command(&json!({"op": "frame", "i": 0}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        apply_setup(&mut s, setup);
        let fps = s.project().settings.fps as f64;
        let times = [0.0, 0.25 / fps, 0.5 / fps, 0.75 / fps, 1.0 / fps];
        let pictures = |s: &Session| -> Vec<Vec<u8>> { times.iter().map(|t| picture(s, *t)).collect() };
        // does `now` show `then`: the pictures within 2 / 255 and the panel exactly
        let same = |s: &Session, then: &(Vec<Vec<u8>>, serde_json::Value), what: &str, failures: &mut Vec<String>| {
            for (k, t) in times.iter().enumerate() {
                let bad = picture(s, *t).iter().zip(&then.0[k]).filter(|(a, b)| a.abs_diff(**b) > 2).count();
                if bad > 0 {
                    failures.push(format!("{name} ({what}): {bad} channel values differ at t = {t}"));
                }
            }
            let mut d = Vec::new();
            diff(&then.1, &s.state(), "", &mut d);
            if !d.is_empty() {
                failures.push(format!("{name} ({what}): the panel shows another value: {:?}", d.iter().take(4).collect::<Vec<_>>()));
            }
        };
        // saved and opened again, the project draws what it drew
        let reopened = |s: &Session, what: &str, failures: &mut Vec<String>| {
            let back = codec::decode(&codec::encode(s.project())).unwrap();
            for t in &times {
                let bad = picture_bytes(&back, *t).iter().zip(&picture_bytes(s.project(), *t)).filter(|(a, b)| a.abs_diff(**b) > 2).count();
                if bad > 0 {
                    failures.push(format!("{name} ({what}): after saving and opening again {bad} channel values differ at t = {t}"));
                }
            }
        };
        let base = (pictures(&s), s.state());
        reopened(&s, "before", &mut failures);
        s.command(on);
        reopened(&s, "on", &mut failures);
        if *name == "textfield font" {
            assert_eq!(s.state()["textfield"]["font"], "Bangers", "the font is on");
        }
        let switched = (pictures(&s), s.state());
        // a row that does nothing proves nothing: "on" must change a picture or the panel
        let mut shown = Vec::new();
        diff(&base.1, &switched.1, "", &mut shown);
        if switched.0 == base.0 && shown.is_empty() {
            failures.push(format!("{name}: turning it on changed neither a picture nor the panel (a wrong set-up or command?)"));
        }
        s.command(off);
        reopened(&s, "off", &mut failures);
        if *name == "textfield font" {
            assert!(s.state()["textfield"]["font"].is_null(), "the font is back to the default");
        }
        same(&s, &base, "on, then off", &mut failures);
        // the other way round: from the switched state, off and on again gives it back
        s.command(on);
        same(&s, &switched, "on again", &mut failures);
        s.command(off);
        same(&s, &base, "off again", &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Every editing command undoes exactly (the project is as it was) and
/// redoes exactly (as it was after the command).
#[test]
fn every_command_undoes_and_redoes_exactly() {
    let cmds: Vec<serde_json::Value> = vec![
        json!({"op": "insert_frame"}), json!({"op": "delete_frame"}), json!({"op": "animation", "tweening": true, "tweened": 3, "loop": true}),
        json!({"op": "tween_add", "at": [0.5]}), json!({"op": "no_tween", "on": true}), json!({"op": "easing", "v": "in"}),
        json!({"op": "delete_here"}), json!({"op": "fps", "v": 24}), json!({"op": "nudge", "dx": 5, "dy": -3}), json!({"op": "clear_frame"}),
        json!({"op": "add_figure"}), json!({"op": "add_textfield"}), json!({"op": "duplicate_figure"}), json!({"op": "flip_figure", "h": true}),
        json!({"op": "rotate_figure", "deg": 30}), json!({"op": "layer_order", "forward": true}), json!({"op": "add_segment"}),
        json!({"op": "delete_joint"}), json!({"op": "delete_segment"}), json!({"op": "split", "n": 2}), json!({"op": "duplicate_branch"}),
        json!({"op": "flip_branch", "h": true}), json!({"op": "pin", "v": true}), json!({"op": "segment", "thickness": 10, "color": 0xff00ffffu32}),
        json!({"op": "segment", "branch": true, "scaleLength": 1.3}), json!({"op": "branch_edit", "addThickness": 4, "replaceFrom": 0x000000ffu32, "replaceTo": 0xffffffffu32}),
        json!({"op": "branch_edit", "ownColor": false, "gradientColor": 0x00ff00ffu32, "addLength": 5}),
        json!({"op": "figure", "color": 0x00ff00ffu32, "scale": 1.4, "locked": true}), json!({"op": "node", "stretchy": true, "length": 60}),
        json!({"op": "figure", "tweenColors": false, "keepInTween": false}),
        json!({"op": "camera", "zoom": 1.5, "rotation": 15}), json!({"op": "unjoin"}), json!({"op": "camera_lock", "on": true}),
        json!({"op": "camera_lock_set", "layer": "L2", "parallax": 0.5, "rotate": true}),
        json!({"op": "to_movieclip"}), json!({"op": "new_movieclip"}), json!({"op": "library_delete", "layer": "L2"}), json!({"op": "rename", "name": "Bob"}),
        json!({"op": "add_camera"}), json!({"op": "watermark", "on": true, "text": "hi"}), json!({"op": "rotate_cameras"}), json!({"op": "reset_camera"}),
        json!({"op": "frame_delay", "seconds": 0.4}), json!({"op": "frame_repeat", "back": 1, "times": 2, "tween": true}), json!({"op": "frame_silence", "on": true}),
        json!({"op": "frame_slow", "n": 4}), json!({"op": "camera_fx", "widescreen": true, "wobbleXY": 9}), json!({"op": "auto_camera", "end": 1, "ease": 23}),
        json!({"op": "frame_bg", "color": 0x336699ffu32, "useGradient": true, "expand": true}), json!({"op": "frame_bg", "expand": true}), json!({"op": "filter", "blur": 6, "invert": true}),
        json!({"op": "clear_filters"}), json!({"op": "segment", "trapRatio": 0.4, "trapRoundEnd": true, "shape": "trapezoid"}), json!({"op": "segment", "trapRatio": 0.6, "trapRoundStart": true, "shape": "trapezoid"}), json!({"op": "segment", "ownColor": false}),
        json!({"op": "textfield", "font": "Bangers"}),
        json!({"op": "polyfill_new"}), json!({"op": "polyfill_color", "color": 0xff0000ffu32}), json!({"op": "polyfill_undo"}),
        json!({"op": "connector_set", "percent": 0.3, "reversed": true}),
        json!({"op": "movieclip", "loop": "once", "speed": 1.5, "offset": 0.2}),
        json!({"op": "edit_movieclip"}), json!({"op": "movieclip_done"}),
        // D7b, F1, F4
        json!({"op": "reset_segment"}), json!({"op": "break_off"}), json!({"op": "new_pose"}), json!({"op": "hold_longer"}),
        json!({"op": "block_length", "frames": 3}), json!({"op": "move_block", "to": 1}),
        json!({"op": "each", "label": "Flip figures", "cmd": {"op": "flip_figure", "h": true}}), json!({"op": "easing", "v": "hold", "block": true}),
        json!({"op": "move_fx", "kind": "wiggle", "on": true}), json!({"op": "stop_animating", "property": "L2/opacity"}),
        json!({"op": "add_time"}), json!({"op": "blend", "v": "MULTIPLY"}), json!({"op": "delete_pose"}), json!({"op": "let_go_here"}), json!({"op": "easing", "v": "bounce", "block": true}), json!({"op": "move_fx", "kind": "swing", "on": true, "amount": 20}),
    ];
    let text = |s: &Session| codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    for c in &cmds {
        let mut s = two_poses();
        let op = c["op"].as_str().unwrap_or("");
        // camera commands need a camera; tweened frames and auto-camera a next frame
        let early = ["camera", "reset_camera", "auto_camera", "tween_add"].contains(&op);
        if early {
            s.command(&json!({"op": "add_camera"}));
        }
        s.command(&json!({"op": "frame", "i": if early { 0 } else { 1 }}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        // the set-up a command needs: a textfield, the figure editor, a connector,
        // or a movieclip selected (its tools act on the selected layer)
        let movieclip = |s: &mut Session| {
            s.command(&json!({"op": "to_movieclip"}));
            let id = s.project().root().expect("root").layers.iter().find(|l| matches!(l.kind, barkfart_core::model::LayerKind::Symbol(_))).expect("a movieclip").id.clone();
            s.command(&json!({"op": "select", "layer": id, "node": ""}));
        };
        // "Use segment color" off has something to give up only after a colour of its own
        if op == "segment" && c["ownColor"] == json!(false) {
            s.command(&json!({"op": "segment", "color": 0xff0000ffu32}));
        }
        match op {
            // a figure in front to step past; a camera that is not where it starts
            "layer_order" => {
                s.command(&json!({"op": "add_figure", "kind": "ball"}));
                s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            }
            "reset_camera" => {
                s.command(&json!({"op": "camera", "zoom": 1.5, "rotation": 12}));
            }
            // a segment that differs from how it came
            "reset_segment" => {
                s.command(&json!({"op": "segment", "thickness": 77}));
            }
            // a frame without a pose of its own: the first pose held a frame longer
            "new_pose" => {
                s.command(&json!({"op": "frame", "i": 0}));
                s.command(&json!({"op": "hold_longer"}));
                s.command(&json!({"op": "frame", "i": 1}));
            }
            // a later pose to take away
            "delete_pose" => {
                s.command(&json!({"op": "frame", "i": 1}));
            }
            // a later pose with room before it
            "move_block" => {
                s.command(&json!({"op": "frame", "i": 0}));
                s.command(&json!({"op": "hold_longer"}));
                s.command(&json!({"op": "hold_longer"}));
                s.command(&json!({"op": "frame", "i": 3}));
            }
            "textfield" => {
                s.command(&json!({"op": "add_textfield"}));
            }
            "polyfill_new" => {
                s.command(&json!({"op": "build", "layer": "L2"}));
            }
            "polyfill_color" => {
                s.command(&json!({"op": "build", "layer": "L2"}));
                s.command(&json!({"op": "polyfill_new"}));
            }
            "polyfill_undo" => {
                // a polynode to take back off
                s.command(&json!({"op": "build", "layer": "L2"}));
                s.command(&json!({"op": "polyfill_new"}));
                let h = s.tools.handles(s.project(), s.time()).into_iter().find(|h| h.layer == "L2" && h.node == "armR2").expect("a joint");
                s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
                s.command(&json!({"op": "up"}));
            }
            "connector_set" => {
                // a connector to move between its two joints
                s.command(&json!({"op": "build", "layer": "L2"}));
                s.command(&json!({"op": "connect_start"}));
                let h = s.tools.handles(s.project(), s.time()).into_iter().find(|h| h.layer == "L2" && h.node == "armR2").expect("a joint");
                s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
                s.command(&json!({"op": "up"}));
            }
            "movieclip" | "edit_movieclip" => movieclip(&mut s),
            "movieclip_done" => {
                movieclip(&mut s);
                s.command(&json!({"op": "edit_movieclip"}));
            }
            _ => {}
        }
        let before = text(&s);
        let undo0 = s.state()["undo"].clone();
        let st = s.command(c);
        if st["undo"] == undo0 && text(&s) == before {
            skipped.push(c.to_string()); // not applicable here: nothing changed
            continue;
        }
        let after = text(&s);
        s.command(&json!({"op": "undo"}));
        if text(&s) != before {
            failures.push(format!("{}: undo differs", c["op"]));
            continue;
        }
        s.command(&json!({"op": "redo"}));
        if text(&s) != after {
            failures.push(format!("{}: redo differs", c["op"]));
        }
    }
    eprintln!("not applicable: {skipped:?}");
    assert!(skipped.len() <= 1, "commands that did nothing: {skipped:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Random editing (fixed seeds): no panics, every state saves and loads back
/// identically, every in-between evaluates, and undoing everything returns
/// to the start.
#[test]
fn random_editing_keeps_the_project_sound() {
    // all four seeds in release builds; two in debug, which is slower
    let seeds: &[u64] = if cfg!(debug_assertions) { &[1, 42] } else { &[1, 7, 42, 2026] };
    for &seed in seeds {
        let mut rng = seed;
        let mut next = move |n: usize| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n.max(1)
        };
        let mut s0 = two_poses();
        s0.command(&json!({"op": "add_camera"}));
        // a fresh history: undoing everything stops at this start
        let mut s = Session::new(Document::new(s0.project().clone()));
        let start = codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
        let mut steps = 0;
        // fewer edits than the undo history keeps (200)
        for _ in 0..150 {
            let frames = s.state()["frames"].as_i64().unwrap_or(1) as usize;
            let figs: Vec<String> = s.project().root().unwrap().layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect();
            if !figs.is_empty() && next(3) > 0 {
                let l = &figs[next(figs.len())];
                let nodes: Vec<String> = s.project().root().unwrap().layer(l).unwrap().as_figure().unwrap().nodes().iter().map(|n| n.id.clone()).collect();
                s.command(&json!({"op": "select", "layer": l, "node": nodes[next(nodes.len())]}));
            }
            let c = match next(24) {
                0 => json!({"op": "frame", "i": next(frames)}),
                1 => json!({"op": "insert_frame"}),
                2 => json!({"op": "delete_frame"}),
                3 => json!({"op": "add_figure"}),
                4 => {
                    let shape = ["line", "circle", "trapezoid", "triangle", "polygon"][next(5)];
                    json!({"op": "segment", "thickness": next(60), "shape": shape})
                }
                5 => json!({"op": "node", "length": 10 + next(150), "stretchy": next(2) == 0}),
                6 => json!({"op": "figure", "color": (next(0xffffff) as u32) << 8 | 0xff, "scale": 0.5 + next(20) as f64 / 10.0}),
                7 => json!({"op": "frame_bg", "useGradient": next(2) == 0, "color": (next(0xffffff) as u32) << 8 | 0xff}),
                8 => json!({"op": "frame_delay", "seconds": next(3) as f64 * 0.2}),
                9 => json!({"op": "frame_repeat", "back": next(3), "times": 1 + next(2), "tween": next(2) == 0}),
                10 => json!({"op": "animation", "tweening": next(2) == 0, "tweened": next(9), "loop": next(2) == 0}),
                11 => json!({"op": "frame_slow", "n": next(6)}),
                12 => json!({"op": "camera", "zoom": 0.5 + next(20) as f64 / 10.0, "rotation": next(90)}),
                13 => json!({"op": "camera_fx", "widescreen": next(2) == 0, "wobbleXY": next(20)}),
                14 => json!({"op": "auto_camera", "end": next(frames + 1), "ease": 1 + next(5)}),
                15 => json!({"op": "filter", "blur": next(10), "invert": next(2) == 0}),
                16 => json!({"op": "delete_here"}),
                17 => json!({"op": "copy_frame"}),
                18 => json!({"op": "paste_frame"}),
                19 => json!({"op": "add_segment"}),
                20 => json!({"op": "delete_joint"}),
                21 => json!({"op": "to_movieclip"}),
                22 => json!({"op": "camera_lock", "on": next(2) == 0}),
                _ => json!({"op": "nudge", "dx": next(20) as f64 - 10.0, "dy": next(20) as f64 - 10.0}),
            };
            let text_of = |s: &Session| codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
            let before = text_of(&s);
            let st = s.command(&c);
            if st["undo"].is_string() {
                steps += 1;
            }
            let after = text_of(&s);
            if after != before {
                // each edit undoes on its own, and redoes
                s.command(&json!({"op": "undo"}));
                let undone = text_of(&s);
                if undone != before {
                    let line = before.lines().zip(undone.lines()).position(|(a, b)| a != b).unwrap_or(0);
                    panic!("seed {seed}: undo of {c} differs at line {line}: {:?} vs {:?}", before.lines().nth(line), undone.lines().nth(line));
                }
                s.command(&json!({"op": "redo"}));
                assert!(text_of(&s) == after, "seed {seed}: redo of {c} differs");
            }
            let p = s.project();
            let text = codec::encode(p);
            let back = codec::decode(&text).unwrap_or_else(|e| panic!("seed {seed}, after {c}: does not load back: {e:?}"));
            assert_eq!(codec::encode(&back), text, "seed {seed}, after {c}: changes when loaded back");
            let fps = p.settings.fps as f64;
            let n = barkfart_core::timing::frame_count(p);
            // three pictures: the start, an in-between and a random frame
            for k in [0, 1, 2 * next(n as usize) as i32] {
                let _ = barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, k as f64 / (2.0 * fps)), None);
            }
            let _ = barkfart_core::timing::Schedule::build(p);
            // no two keys of a track a float hair apart (they fight over a frame)
            for (id, q) in p.property_index() {
                for w in q.track.keys().windows(2) {
                    assert!(w[1].time - w[0].time > 0.25 / fps, "seed {seed}, after {c}: {id} has keys {} and {} on one frame", w[0].time, w[1].time);
                }
            }
        }
        assert!(steps > 60, "seed {seed}: too few edits ({steps})");
        for _ in 0..1000 {
            let st = s.command(&json!({"op": "undo"}));
            if st["canUndo"] == false {
                break;
            }
        }
        let end = codec::encode(s.project()).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n");
        if end != start {
            let line = start.lines().zip(end.lines()).position(|(a, b)| a != b).unwrap_or(0);
            panic!("seed {seed} ({steps} edits): undoing everything does not return to the start, line {line}:\n{:?}\n{:?}", start.lines().nth(line), end.lines().nth(line));
        }
    }
}

/// Adding or pasting frames does not copy an auto-camera's start or end
/// marker (that would start a second span or cut the first short).
#[test]
fn new_and_pasted_frames_do_not_copy_auto_camera_markers() {
    let mut s = two_poses();
    s.command(&json!({"op": "add_camera"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "auto_camera", "end": 2, "ease": 1}));
    let marks = |s: &Session| -> Vec<bool> { s.state()["frameMarks"].as_array().unwrap().iter().map(|m| m[4] == true).collect() };
    assert_eq!(marks(&s), vec![true, false, false]);
    // a new frame after the start
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(marks(&s), vec![true, false, false, false], "the new frame does not start another auto-camera");
    s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(s.state()["autoCam"]["end"], 3, "the span still ends on its (moved) frame");
    // copy the start frame and paste it after the last
    s.command(&json!({"op": "copy_frame"}));
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "paste_frame"}));
    assert_eq!(marks(&s), vec![true, false, false, false, false]);
    s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(s.state()["autoCam"]["end"], 3);
}

/// A new frame copies the pose and the background, not the pause or the
/// repeat of the frame it was made from.
#[test]
fn new_frames_do_not_inherit_timing() {
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "frame_delay", "seconds": 0.5}));
    s.command(&json!({"op": "frame_repeat", "back": 1, "times": 2}));
    s.command(&json!({"op": "frame_bg", "color": 0x336699ffu32}));
    let st = s.command(&json!({"op": "insert_frame"}));
    assert_eq!(st["frame"], 2);
    assert_eq!(st["frameDelay"], 0.0, "no pause on the new frame");
    assert_eq!(st["frameRepeat"]["back"], 0, "no repeat on the new frame");
    assert_eq!(st["frameBg"]["color"], 0x336699ffu32, "the background is copied");
    s.command(&json!({"op": "frame", "i": 1}));
    assert_eq!(s.state()["frameDelay"], 0.5, "the original keeps its pause");
}

/// Deleting the frame an auto-camera ends on: it then ends on the frame
/// before (as the reference says, it adapts to deleted frames).
#[test]
fn deleting_an_auto_camera_end_frame_moves_the_end() {
    let mut s = two_poses();
    s.command(&json!({"op": "add_camera"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "auto_camera", "end": 3, "ease": 1}));
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "delete_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(s.state()["autoCam"]["end"], 2, "ends on the frame before the deleted one");
}

/// Textfield fonts: a bundled font by name, or a .ttf / .otf imported into
/// the project (kept with it, put on the selected textfield); saved.
#[test]
fn textfield_fonts_bundled_and_imported() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    let st = s.command(&json!({"op": "add_textfield"}));
    assert!(st["textfield"]["font"].is_null());
    let st = s.command(&json!({"op": "textfield", "font": "Bangers"}));
    assert_eq!(st["textfield"]["font"], "Bangers");
    let font = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/Lobster-Regular.ttf")).unwrap();
    let st = s.import("My Lobster.ttf", &font);
    assert_eq!(st["undo"], "Import font");
    let key = st["projectFonts"][0]["key"].as_str().unwrap().to_string();
    assert_eq!(st["projectFonts"][0]["name"], "My Lobster");
    assert_eq!(st["textfield"]["font"], key.as_str(), "put on the selected textfield");
    let bytes = s.save_copy().unwrap();
    let back = codec::read_bark(&bytes).unwrap();
    assert!(back.assets.keys().any(|k| k.ends_with(".ttf")), "the font travels with the project");
    let st = s.command(&json!({"op": "textfield", "font": null}));
    assert!(st["textfield"]["font"].is_null(), "back to the default font");
    // not a font
    let st = s.import("notes.ttf", b"hello this is not a font at all");
    assert!(st["message"].as_str().unwrap().contains("not a font"));
}

/// Textfield boxes are measured with the font's own widths when the
/// renderer has given them (wider font, wider box).
#[test]
fn textfield_box_follows_the_font() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    let mut narrow = barkfart_core::session::FontMetrics { line: 1.2, ..Default::default() };
    let mut wide = barkfart_core::session::FontMetrics { line: 1.0, ..Default::default() };
    for c in (32u8..127).map(char::from) {
        narrow.advance.insert(c, 0.5);
        wide.advance.insert(c, 1.0);
    }
    s.fonts.insert(String::new(), narrow);
    s.fonts.insert("Wide".into(), wide);
    s.command(&json!({"op": "add_textfield"}));
    let w = |st: &serde_json::Value| st["textfield"]["x1"].as_f64().unwrap() - st["textfield"]["x0"].as_f64().unwrap();
    let a = w(&s.command(&json!({"op": "textfield", "text": "Hello"})));
    let b = w(&s.command(&json!({"op": "textfield", "font": "Wide"})));
    assert!((b / a - 2.0).abs() < 0.01, "twice as wide: {a} {b}");
}

/// The picture at time t, rendered the way an export is (160 x 90, exact bytes).
fn picture_bytes(p: &barkfart_core::model::Project, t: f64) -> Vec<u8> {
    let list = barkfart_core::tess::layered(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
    let mut im = barkfart_core::fx::Image::new(160, 90, -640.0, 360.0, 0.125);
    for v in im.px.iter_mut() {
        *v = 1.0;
    }
    barkfart_core::fx::render(&list, &mut im);
    im.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// The project as saved, without the id counter.
fn saved_text(p: &barkfart_core::model::Project) -> String {
    codec::encode(p).lines().filter(|l| !l.contains("idCounter")).collect::<Vec<_>>().join("\n")
}

/// Goes to a frame and selects the man's upper right arm (the set-up most
/// commands act on).
fn pick_arm(s: &mut Session, frame: i64) {
    s.command(&json!({"op": "frame", "i": frame}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
}

fn joint_at(s: &Session, node: &str) -> (f32, f32) {
    let h = s.tools.handles(s.project(), s.time()).into_iter().find(|h| h.layer == "L2" && h.node == node).expect("a joint");
    (h.x, h.y)
}

/// The commands `every_command_undoes_and_redoes_exactly` leaves out: the
/// deletes, every paste, polyfill delete, reorder, tween, break apart, join,
/// frame sounds and the drags. Each runs after its own set-up and must
///  1. undo to exactly the project before and redo to exactly the one after,
///  2. undo to exactly the picture before and redo to the one after (frames
///     1 and 2 both), and
///  3. read back from the saved text as the very same project.
/// A case that changes nothing is a failure too: fix its set-up.
#[test]
fn deletes_pastes_and_drags_undo_redo_and_save_exactly() {
    use barkfart_core::audio::{encode_wav, tone};
    type Case = (&'static str, Box<dyn Fn(&mut Session) -> Vec<Value>>);
    let sound = |s: &mut Session| -> String {
        let st = s.import("beep.wav", &encode_wav(&tone(440.0, 0.2, 8000, 0.5)));
        st["soundList"][0]["id"].as_str().expect("the sound's id").to_string()
    };
    let cases: Vec<Case> = vec![
        ("delete figure", Box::new(|s| { pick_arm(s, 1); vec![json!({"op": "delete_figure"})] })),
        ("delete textfield", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "add_textfield"})); vec![json!({"op": "delete_textfield"})] })),
        ("paste figure (a new one, same frame)", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_figure"})); vec![json!({"op": "paste_figure"})] })),
        ("paste figure (onto another frame)", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "copy_figure"}));
            s.command(&json!({"op": "delete_here"}));
            vec![json!({"op": "paste_figure"})]
        })),
        ("paste figure with layering", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_figure"})); vec![json!({"op": "paste_figure", "layering": true})] })),
        ("paste frame", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_frame", "n": 0})); pick_arm(s, 0); vec![json!({"op": "paste_frame"})] })),
        ("paste two frames", Box::new(|s| { pick_arm(s, 0); s.command(&json!({"op": "copy_frame", "n": 1})); vec![json!({"op": "paste_frame"})] })),
        ("paste pose", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_pose"})); pick_arm(s, 0); vec![json!({"op": "paste_pose"})] })),
        ("paste properties", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_props"})); pick_arm(s, 0); vec![json!({"op": "paste_props"})] })),
        ("paste properties without the nodes", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "copy_props"}));
            pick_arm(s, 0);
            vec![json!({"op": "paste_props", "nodes": false, "color": false, "layering": false})]
        })),
        ("paste segment", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_branch", "branch": false})); vec![json!({"op": "paste_branch"})] })),
        ("paste branch", Box::new(|s| { pick_arm(s, 1); s.command(&json!({"op": "copy_branch", "branch": true})); vec![json!({"op": "paste_branch"})] })),
        ("paste filters", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "filter", "invert": true, "blur": 3}));
            s.command(&json!({"op": "copy_filters"}));
            s.command(&json!({"op": "clear_filters"}));
            vec![json!({"op": "paste_filters"})]
        })),
        ("paste textfield", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "add_textfield"}));
            s.command(&json!({"op": "copy_textfield"}));
            vec![json!({"op": "paste_textfield"})]
        })),
        ("delete polyfill", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "build", "layer": "L2"}));
            s.command(&json!({"op": "polyfill_new"}));
            vec![json!({"op": "polyfill_delete"})]
        })),
        ("send segment to the back", Box::new(|s| { pick_arm(s, 1); vec![json!({"op": "reorder", "to": -2})] })),
        ("bring branch to the front", Box::new(|s| { pick_arm(s, 1); vec![json!({"op": "reorder", "branch": true, "to": 2})] })),
        ("tween off on a frame", Box::new(|s| { pick_arm(s, 0); vec![json!({"op": "tween", "on": false})] })),
        ("break apart a movieclip", Box::new(|s| {
            pick_arm(s, 1);
            s.command(&json!({"op": "to_movieclip"}));
            let id = s.project().root().expect("root").layers.iter().find(|l| matches!(l.kind, barkfart_core::model::LayerKind::Symbol(_))).expect("a movieclip").id.clone();
            vec![json!({"op": "break_apart", "layer": id})]
        })),
        ("join a figure to a joint", Box::new(|s| {
            pick_arm(s, 0);
            s.command(&json!({"op": "add_figure", "kind": "ball"}));
            s.command(&json!({"op": "join"}));
            let (x, y) = joint_at(s, "armR2");
            vec![json!({"op": "down", "x": x, "y": y, "r": 20}), json!({"op": "up"})]
        })),
        ("set a frame's sound", Box::new(move |s| {
            let id = sound(s);
            pick_arm(s, 1);
            vec![json!({"op": "frame_sound", "asset": id, "volume": 0.5, "pan": -0.5, "pitch": 2.0})]
        })),
        ("clear a frame's sound", Box::new(move |s| {
            let id = sound(s);
            pick_arm(s, 1);
            s.command(&json!({"op": "frame_sound", "asset": id}));
            vec![json!({"op": "frame_sound", "asset": null})]
        })),
        ("paste sound", Box::new(move |s| {
            let id = sound(s);
            pick_arm(s, 0);
            s.command(&json!({"op": "frame_sound", "asset": id}));
            s.command(&json!({"op": "copy_sound"}));
            pick_arm(s, 1);
            vec![json!({"op": "paste_sound"})]
        })),
        ("drag a joint", Box::new(|s| {
            pick_arm(s, 1);
            let (x, y) = joint_at(s, "armR2");
            vec![json!({"op": "down", "x": x, "y": y, "r": 20}), json!({"op": "move", "x": x + 30.0, "y": y + 20.0}), json!({"op": "up"})]
        })),
        ("drag the main node", Box::new(|s| {
            pick_arm(s, 1);
            let h = s.tools.handles(s.project(), s.time()).into_iter().find(|h| h.layer == "L2" && h.root).expect("the main node");
            vec![json!({"op": "down", "x": h.x, "y": h.y, "r": 20}), json!({"op": "move", "x": h.x + 40.0, "y": h.y - 25.0}), json!({"op": "up"})]
        })),
    ];
    let times = |s: &Session| {
        let fps = s.project().settings.fps as f64;
        [0.0, 1.0 / fps]
    };
    let mut failures = Vec::new();
    for (name, run) in &cases {
        let mut s = two_poses();
        let cmds = run(&mut s);
        let (before, ts) = (saved_text(s.project()), times(&s));
        let pics_before: Vec<Vec<u8>> = ts.iter().map(|t| picture_bytes(s.project(), *t)).collect();
        for c in &cmds {
            s.command(c);
        }
        let after = saved_text(s.project());
        let pics_after: Vec<Vec<u8>> = ts.iter().map(|t| picture_bytes(s.project(), *t)).collect();
        if after == before {
            failures.push(format!("{name}: changed nothing"));
            continue;
        }
        // saved and read back: the same project
        let back = codec::decode(&codec::encode(s.project())).unwrap();
        if saved_text(&back) != after {
            failures.push(format!("{name}: the saved text reads back as another project"));
        }
        s.command(&json!({"op": "undo"}));
        if saved_text(s.project()) != before {
            failures.push(format!("{name}: undo differs"));
            continue;
        }
        if ts.iter().zip(&pics_before).any(|(t, p)| picture_bytes(s.project(), *t) != *p) {
            failures.push(format!("{name}: undo gives another picture"));
        }
        s.command(&json!({"op": "redo"}));
        if saved_text(s.project()) != after {
            failures.push(format!("{name}: redo differs"));
            continue;
        }
        if ts.iter().zip(&pics_after).any(|(t, p)| picture_bytes(s.project(), *t) != *p) {
            failures.push(format!("{name}: redo gives another picture"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Looking is not editing. Every command that only copies, selects, traces,
/// switches a tool or a view must leave the saved text, the undo and redo
/// steps and the "unsaved changes" marker exactly as they were (a Copy that
/// asks "save changes?" afterwards is the same kind of bug as a hidden key).
#[test]
fn copying_and_looking_never_touch_the_project() {
    use barkfart_core::audio::{encode_wav, tone};
    let looks: Vec<Value> = vec![
        json!({"op": "copy_figure"}), json!({"op": "copy_figure", "joined": true}), json!({"op": "copy_frame", "n": 1}),
        json!({"op": "copy_pose"}), json!({"op": "copy_props"}), json!({"op": "copy_branch", "branch": true}), json!({"op": "copy_branch", "branch": false}),
        json!({"op": "copy_filters"}), json!({"op": "copy_sound"}), json!({"op": "copy_textfield"}),
        json!({"op": "trace", "frame": 0}), json!({"op": "trace", "layer": "L2"}), json!({"op": "trace"}), json!({"op": "editor_bg", "color": 0x336699ffu32}), json!({"op": "editor_bg"}),
        json!({"op": "select", "layer": "L2"}), json!({"op": "select", "layer": "L2", "node": "armR2"}), json!({"op": "select"}),
        json!({"op": "cycle_figure", "dir": 1}), json!({"op": "cycle_figure", "dir": -1}), json!({"op": "cycle_joint", "dir": 1}), json!({"op": "cycle_joint", "dir": -1}),
        json!({"op": "mode", "v": "all"}), json!({"op": "mode", "v": "camera"}), json!({"op": "mode", "v": "figure"}),
        json!({"op": "join"}), json!({"op": "join_cancel"}), json!({"op": "build", "layer": "L2"}), json!({"op": "connect_start"}), json!({"op": "connect_cancel"}),
        json!({"op": "polyfill_edit", "id": "pf1"}), json!({"op": "polyfill_edit"}), json!({"op": "draw_tool", "shape": "circle"}), json!({"op": "draw_tool", "shape": "none"}),
        json!({"op": "build_done"}), json!({"op": "frame", "i": 0}), json!({"op": "frame", "i": 1}),
        json!({"op": "set", "k": "stretch", "v": true}), json!({"op": "set", "k": "scope", "v": "joint"}), json!({"op": "set", "k": "snap", "v": 15}),
        json!({"op": "set", "k": "onion", "v": false}), json!({"op": "set", "k": "axis", "v": true}), json!({"op": "set", "k": "static", "v": true}),
        json!({"op": "set", "k": "mainonly", "v": true}), json!({"op": "set", "k": "tween_new", "v": false}), json!({"op": "state"}),
    ];
    let mut failures = Vec::new();
    for c in &looks {
        let mut s = two_poses();
        // something to copy of every kind: a textfield, filters, a sound on this frame
        let st = s.import("beep.wav", &encode_wav(&tone(440.0, 0.2, 8000, 0.5)));
        let id = st["soundList"][0]["id"].as_str().expect("the sound's id").to_string();
        pick_arm(&mut s, 1);
        s.command(&json!({"op": "frame_sound", "asset": id}));
        s.command(&json!({"op": "filter", "invert": true}));
        s.command(&json!({"op": "add_textfield"}));
        pick_arm(&mut s, 1);
        s.doc.mark_saved();
        let key = |s: &Session| {
            let st = s.state();
            (saved_text(s.project()), st["undo"].clone(), st["redo"].clone(), st["canUndo"].clone(), st["canRedo"].clone(), st["dirty"].clone())
        };
        let before = key(&s);
        s.command(c);
        // a second one on top, as a button pressed twice
        s.command(c);
        let after = key(&s);
        if after != before {
            let what = if after.0 != before.0 { "the project" } else if after.5 != before.5 { "the unsaved marker" } else { "the undo steps" };
            failures.push(format!("{c}: changed {what}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Tween colours is a switch, not a rewrite: off, then on again, gives back
/// the very same project whatever easing the colour keys had (an imported
/// animation can have eased, held and linear colour keys), and while it is
/// off the colours jump at each frame instead of blending.
#[test]
fn tween_colours_off_and_on_keeps_every_colour_key_as_it_was() {
    use barkfart_core::model::{Color, Ease, Interp, Key, LayerKind, Value};
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true}));
    let fps = s.project().settings.fps as f64;
    let (red, blue) = (Value::C(Color::rgba8888(0xff0000ff)), Value::C(Color::rgba8888(0x0000ffff)));
    let kinds = [(Interp::Bezier, Ease::new(0.42, 0.0, 0.58, 1.0)), (Interp::Hold, Ease::LINEAR), (Interp::Linear, Ease::LINEAR)];
    // three segments with colour keys on frames 1 and 2: eased, held, linear
    let sym = s.project().root_symbol.clone();
    let ids: Vec<String> = {
        let LayerKind::Figure(f) = &s.project().root().unwrap().layer("L2").unwrap().kind else { panic!("a figure") };
        f.nodes().iter().filter(|n| n.parent.is_some()).map(|n| n.id.clone()).take(3).collect()
    };
    {
        let LayerKind::Figure(f) = &mut s.doc.project_mut().layer_mut(&sym, "L2").unwrap().kind else { panic!("a figure") };
        for (id, (interp, ease)) in ids.iter().zip(kinds) {
            let n = f.node_mut(id).unwrap();
            n.own_color = true;
            n.color.track.put(Key { time: 0.0, value: red, interp, ease });
            n.color.track.put(Key::new(1.0 / fps, blue, Interp::Linear));
        }
    }
    // the colours as drawn (the evaluator reads the layer's switch)
    fn collect(g: &barkfart_core::eval::Group, out: &mut Vec<(String, u32)>) {
        for it in &g.children {
            match it {
                barkfart_core::eval::Item::Shape(sh) => out.push((sh.source.clone(), sh.color.to_rgba8888())),
                barkfart_core::eval::Item::Group(g) => collect(g, out),
            }
        }
    }
    let colours = |s: &Session, t: f64| -> Vec<u32> {
        let p = s.project();
        let mut drawn = Vec::new();
        collect(&barkfart_core::eval::evaluate(p, &p.root_symbol, t).root, &mut drawn);
        ids.iter().map(|id| drawn.iter().find(|(src, _)| *src == format!("L2/{id}")).map(|x| x.1).expect("the segment is drawn")).collect()
    };
    let times = [0.0, 0.25 / fps, 0.5 / fps, 0.75 / fps, 1.0 / fps];
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let (text, blending): (String, Vec<Vec<u32>>) = (saved_text(s.project()), times.iter().map(|t| colours(&s, *t)).collect());
    assert_ne!(blending[2], blending[0], "with colours tweening the middle is a blend");

    s.command(&json!({"op": "figure", "tweenColors": false}));
    for (k, t) in times.iter().enumerate() {
        let want = if k == 4 { colours(&s, 1.0 / fps) } else { colours(&s, 0.0) };
        assert_eq!(colours(&s, *t), want, "colours off: a jump at time {t}, not a blend");
    }
    assert_eq!(colours(&s, 1.0 / fps), blending[4], "the next frame still has its own colours");
    s.command(&json!({"op": "figure", "tweenColors": true}));
    assert_eq!(saved_text(s.project()).replace(",\"colorsJump\":true", "").lines().count(), text.lines().count(), "only the flag differs");
    for (k, t) in times.iter().enumerate() {
        assert_eq!(colours(&s, *t), blending[k], "on again: the same blend at time {t}");
    }
    assert_eq!(saved_text(s.project()), text, "on again: the very same project");
}

/// "Use segment color" off keeps the segment's own colour aside (the
/// original greys the swatch out): ticking it again brings the colour back,
/// even after the stickfigure colour changed or the project was saved and
/// opened again, and a ticked, unchanged segment leaves nothing behind.
#[test]
fn a_segments_own_colour_comes_back_when_use_segment_colour_is_ticked_again() {
    let colour = |s: &Session| s.state()["selected"]["color"].as_u64().unwrap() as u32;
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "figure", "color": 0x00ff00ffu32}));
    let plain = saved_text(s.project());
    // own colour on, off, on with nothing changed in between: no trace
    s.command(&json!({"op": "segment", "ownColor": true}));
    s.command(&json!({"op": "segment", "ownColor": false}));
    assert_eq!(saved_text(s.project()), plain, "an own colour equal to the figure's keeps nothing aside");

    s.command(&json!({"op": "segment", "color": 0xff0000ffu32}));
    s.command(&json!({"op": "segment", "ownColor": false}));
    assert_eq!(colour(&s), 0x00ff00ff, "off: the figure colour");
    assert!(saved_text(s.project()).contains("B.ownColor"), "the own colour is kept in the project");
    s.command(&json!({"op": "figure", "color": 0x0000ffffu32}));
    assert_eq!(colour(&s), 0x0000ffff, "off: the segment follows the figure colour");

    // saved and opened again while it is off
    let mut again = Session::new(Document::new(codec::decode(&codec::encode(s.project())).unwrap()));
    again.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    again.command(&json!({"op": "segment", "ownColor": true}));
    assert_eq!(colour(&again), 0xff0000ff, "ticked again after a reload: its own colour");

    s.command(&json!({"op": "segment", "ownColor": true}));
    assert_eq!(colour(&s), 0xff0000ff, "ticked again: its own colour");
    assert!(!saved_text(s.project()).contains("B.ownColor"), "nothing kept aside any more");
    let round_one = saved_text(s.project());
    // a new own colour while off replaces the kept one
    s.command(&json!({"op": "segment", "ownColor": false}));
    s.command(&json!({"op": "segment", "color": 0xffff00ffu32}));
    s.command(&json!({"op": "segment", "ownColor": false}));
    s.command(&json!({"op": "segment", "ownColor": true}));
    assert_eq!(colour(&s), 0xffff00ff, "the new own colour comes back, not the one kept before");
    // undo goes back one step at a time: on, off, the new colour, off
    for _ in 0..4 {
        s.command(&json!({"op": "undo"}));
    }
    assert_eq!(colour(&s), 0xff0000ff, "undo: back to the first own colour");
    assert_eq!(saved_text(s.project()), round_one, "undo is exact");
}

/// Modify branch's "Use segment color" keeps own colours aside as well.
#[test]
fn modify_branch_use_segment_colour_off_and_on_gives_the_colours_back() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "segment", "branch": true, "color": 0xff0000ffu32}));
    let own = saved_text(s.project());
    let before: Vec<u8> = picture_bytes(s.project(), 0.0);
    s.command(&json!({"op": "branch_edit", "ownColor": false}));
    assert_ne!(picture_bytes(s.project(), 0.0), before, "off: the branch follows the figure colour");
    s.command(&json!({"op": "branch_edit", "ownColor": true}));
    assert_eq!(picture_bytes(s.project(), 0.0), before, "on again: the colours are back");
    assert_eq!(saved_text(s.project()), own, "and nothing is left behind");
}

/// A small picture (80 x 45) of the project at time t, for the matrix tests.
fn small_picture(p: &barkfart_core::model::Project, t: f64) -> Vec<u8> {
    let list = barkfart_core::tess::layered(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
    let mut im = barkfart_core::fx::Image::new(80, 45, -640.0, 360.0, 0.0625);
    for v in im.px.iter_mut() {
        *v = 1.0;
    }
    barkfart_core::fx::render(&list, &mut im);
    im.px.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// Three frames, a camera, tweening on, the middle frame and the man's arm
/// selected, plus a row's set-up.
fn three_frames(setup: &Option<Value>) -> Session {
    let mut s = two_poses();
    s.command(&json!({"op": "animation", "tweening": true}));
    s.command(&json!({"op": "add_camera"}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    apply_setup(&mut s, setup);
    s
}

/// What a viewer sees: every frame and every half frame.
fn all_pictures(s: &Session) -> Vec<Vec<u8>> {
    let fps = s.project().settings.fps as f64;
    let n = s.state()["frames"].as_i64().unwrap() as usize;
    (0..2 * n).map(|k| small_picture(s.project(), k as f64 / (2.0 * fps))).collect()
}

/// Keys of one property that sit a hair apart (float drift when a frame was
/// inserted, deleted or held: two keys where there should be one).
fn stray_keys(p: &barkfart_core::model::Project) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(sym) = p.root() {
        for l in &sym.layers {
            for q in l.properties() {
                let ks = q.track.keys();
                if ks.windows(2).any(|w| w[1].time - w[0].time < 1e-6) {
                    out.push(q.id.clone());
                }
            }
        }
    }
    out
}

/// The edits the matrix runs after a switch was flipped on and off.
fn follow_up_edits() -> Vec<(&'static str, Vec<Value>)> {
    vec![
        ("insert frame", vec![json!({"op": "insert_frame"})]),
        ("insert a blank frame", vec![json!({"op": "insert_frame", "copy": false})]),
        ("delete frame", vec![json!({"op": "delete_frame"})]),
        ("copy and paste frame", vec![json!({"op": "copy_frame", "n": 0}), json!({"op": "paste_frame"})]),
        ("frame rate", vec![json!({"op": "fps", "v": 24})]),
        ("tween frames", vec![json!({"op": "tween_add", "at": [0.5]})]),
        ("slow motion", vec![json!({"op": "frame_slow", "n": 3})]),
        ("clear frame", vec![json!({"op": "clear_frame"})]),
        ("figure colour and scale", vec![json!({"op": "figure", "color": 0x00ff00ffu32, "scale": 1.3})]),
        ("segment thickness", vec![json!({"op": "segment", "thickness": 12})]),
        ("rotate cameras", vec![json!({"op": "rotate_cameras"})]),
        ("camera zoom", vec![json!({"op": "camera", "zoom": 1.4, "rotation": 8})]),
        ("background colour", vec![json!({"op": "frame_bg", "color": 0x336699ffu32})]),
        ("break off", vec![json!({"op": "break_off"})]),
        ("hold longer", vec![json!({"op": "hold_longer"})]),
        ("new pose", vec![json!({"op": "frame", "i": 2}), json!({"op": "new_pose"})]),
    ]
}

/// A switch flipped on and off again must not change what the next edit
/// does: for every control T, "T on, T off, then F" gives the same pictures
/// (every frame and half frame, new frames too) and the same panel as "F"
/// alone. This is the test for hidden keys: a leftover that shows nothing
/// today but bites on the next edit. One test per edit F (they run in
/// parallel); `matrix_covers_every_edit` keeps the list and the tests in step.
fn next_edit_matrix(which: usize) {
    let (fname, edit) = follow_up_edits().swap_remove(which);
    let mut failures = Vec::new();
    for (name, setup, on, off) in &toggle_rows() {
        let (mut a, mut b) = (three_frames(setup), three_frames(setup));
        a.command(on);
        a.command(off);
        for s in [&mut a, &mut b] {
            s.command(&json!({"op": "frame", "i": 1}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            for c in &edit {
                s.command(c);
            }
        }
        for (what, s) in [("on and off, then", &a), ("alone", &b)] {
            let stray = stray_keys(s.project());
            if !stray.is_empty() {
                failures.push(format!("{name}, {what} {fname}: keys a hair apart on {:?}", stray.iter().take(4).collect::<Vec<_>>()));
            }
        }
        let (pa, pb) = (all_pictures(&a), all_pictures(&b));
        let bad: Vec<usize> = pa.iter().zip(&pb).enumerate().filter(|(_, (x, y))| x.len() != y.len() || x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 2)).map(|(k, _)| k).collect();
        let mut d = Vec::new();
        diff(&b.state(), &a.state(), "", &mut d);
        if pa.len() != pb.len() || !bad.is_empty() || !d.is_empty() {
            failures.push(format!("{name}, on and off, then {fname}: pictures differ at half frames {bad:?} ({} / {} of them); panel {:?}", pa.len(), pb.len(), d.iter().take(3).collect::<Vec<_>>()));
        }
    }
    assert!(failures.is_empty(), "{} switches differ:\n{}", failures.len(), failures.join("\n"));
}

macro_rules! next_edit_tests {
    ($($name:ident => $n:expr),* $(,)?) => {
        $( #[test] fn $name() { next_edit_matrix($n) } )*
        #[test]
        fn matrix_covers_every_edit() {
            assert_eq!(follow_up_edits().len(), [$($n),*].len(), "a test for every edit in follow_up_edits()");
        }
    };
}

next_edit_tests! {
    after_a_switch_insert_frame_is_unchanged => 0,
    after_a_switch_insert_blank_frame_is_unchanged => 1,
    after_a_switch_delete_frame_is_unchanged => 2,
    after_a_switch_copy_and_paste_frame_is_unchanged => 3,
    after_a_switch_frame_rate_is_unchanged => 4,
    after_a_switch_tween_frames_is_unchanged => 5,
    after_a_switch_slow_motion_is_unchanged => 6,
    after_a_switch_clear_frame_is_unchanged => 7,
    after_a_switch_figure_colour_and_scale_is_unchanged => 8,
    after_a_switch_segment_thickness_is_unchanged => 9,
    after_a_switch_rotate_cameras_is_unchanged => 10,
    after_a_switch_camera_zoom_is_unchanged => 11,
    after_a_switch_background_colour_is_unchanged => 12,
    after_a_switch_break_off_is_unchanged => 13,
    after_a_switch_hold_longer_is_unchanged => 14,
    after_a_switch_new_pose_is_unchanged => 15,
}

/// The toggle rows that are effects: keys since D5, not frame snapshots.
const EFFECT_ROWS: [&str; 12] = ["invert", "motion blur", "blur", "pixelate", "hue shift", "saturation", "tint amount", "glow amount", "shadow opacity", "outline amount", "opacity", "motion blur distance kept while off"];

/// Controls that belong to one frame (the middle one) leave the pictures at
/// the exact times of the first and last frame as they were; an effect (a
/// key) leaves the first frame as it was.
#[test]
fn a_per_frame_control_on_the_middle_frame_leaves_the_other_frames_alone() {
    let per_frame = [
        "use gradient", "expand gradient", "widescreen", "wobble", "wobble rotation", "do not tween", "pause", "slow motion", "invert", "motion blur",
        "nudge", "turn figure", "silence frame", "gradient colours kept while off", "pause kept while off", "motion blur distance kept while off",
        "blur", "pixelate", "hue shift", "saturation", "tint amount", "glow amount", "shadow opacity", "outline amount", "opacity",
    ];
    let mut failures = Vec::new();
    let mut checked = 0;
    for (name, setup, on, _) in &toggle_rows() {
        if !per_frame.contains(name) {
            continue;
        }
        checked += 1;
        let mut s = three_frames(setup);
        s.command(&json!({"op": "set", "k": "keying", "v": true})); // per-frame effects are keyed (D5)
        let fps = s.project().settings.fps as f64;
        // an effect is a key (D5): it lasts until the next key, so it may
        // reach the last frame, but never a frame before it; frame tools
        // belong to their frame alone
        let effect = EFFECT_ROWS.contains(name);
        let ends = |s: &Session| -> Vec<Vec<u8>> {
            let ts: &[f64] = if effect { &[0.0] } else { &[0.0, 2.0] };
            ts.iter().map(|t| small_picture(s.project(), *t / fps)).collect()
        };
        let before = ends(&s);
        s.command(on);
        if ends(&s) != before {
            failures.push(format!("{name}: changed {} while set on the middle one", if effect { "the first frame" } else { "the first or last frame" }));
        }
    }
    assert_eq!(checked, per_frame.len(), "every name in the list is a row");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Frames are snapshots (UI-SPEC v3.1): whatever a control did on the middle
/// frame, a frame inserted after it (a copy) or pasted after it draws exactly
/// what the middle frame draws, and the frames around it draw what they drew.
#[test]
fn a_new_frame_draws_what_the_frame_it_copies_draws() {
    let mut failures = Vec::new();
    for (name, setup, on, _) in &toggle_rows() {
        for how in ["insert", "paste"] {
            let mut s = three_frames(setup);
            s.command(on);
            s.command(&json!({"op": "frame", "i": 1}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            let fps = s.project().settings.fps as f64;
            let at = |s: &Session, i: usize| small_picture(s.project(), i as f64 / fps);
            let (first, middle, last) = (at(&s, 0), at(&s, 1), at(&s, 2));
            let panel = s.state();
            if how == "insert" {
                s.command(&json!({"op": "insert_frame"}));
            } else {
                s.command(&json!({"op": "copy_frame", "n": 0}));
                s.command(&json!({"op": "paste_frame"}));
            }
            let close = |a: &Vec<u8>, b: &Vec<u8>| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 2);
            let now: Vec<Vec<u8>> = (0..4).map(|i| at(&s, i)).collect();
            // a movieclip plays its own animation along the project's time: a frame that
            // moved in time shows another picture of it, so those rows are held to their settings
            let timeline = name.starts_with("movieclip");
            if !timeline && !close(&now[0], &first) {
                failures.push(format!("{name}, {how}: the first frame changed"));
            }
            if !timeline && !close(&now[1], &middle) {
                failures.push(format!("{name}, {how}: the copied frame changed"));
            }
            // the settings of the copied frame are on the new one (the panel apart from
            // the frame counters and the clipboard)
            let mut d = Vec::new();
            diff(&panel, &s.state(), "", &mut d);
            // where the playhead is, not what the frame shows (the pose's
            // first frame is a frame number too)
            d.retain(|x| !["/frame:", "/frames:", "/keyed:", "/library:", "/frameMarks:", "/noTweenFrames:", "/frameCopied:", "/frameClipCount:", "/selected/pose/start:", "/rows:"].iter().any(|p| x.starts_with(p)));
            // a frame inserted with Add frame does not copy the frame's timing (the pause,
            // slow motion and silence); a pasted frame does
            if how == "insert" {
                d.retain(|x| !["/frameDelay:", "/frameSlow:", "/frameSilence:"].iter().any(|p| x.starts_with(p)));
            }
            // a wobble is a shake that depends on the time: the same setting draws another
            // shake one frame later, so those rows are held to their settings, not their pixels
            if !name.starts_with("wobble") && !timeline && !close(&now[2], &middle) {
                failures.push(format!("{name}, {how}: the new frame does not draw what the copied frame draws (its panel differs: {:?})", d.iter().take(8).collect::<Vec<_>>()));
            }
            if !d.is_empty() {
                failures.push(format!("{name}, {how}: the new frame's settings differ from the copied frame's: {:?}", d.iter().take(8).collect::<Vec<_>>()));
            }
            if !timeline && !close(&now[3], &last) {
                failures.push(format!("{name}, {how}: the last frame changed"));
            }
        }
    }
    assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), failures.join("\n"));
}

/// Deleting the middle frame (with whatever a control did on it) leaves the
/// first and last frame drawing what they drew, and the saved project reads
/// back the same.
#[test]
fn deleting_a_frame_leaves_the_other_frames_as_they_were() {
    let mut failures = Vec::new();
    for (name, setup, on, _) in &toggle_rows() {
        let mut s = three_frames(setup);
        s.command(on);
        s.command(&json!({"op": "frame", "i": 1}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        let fps = s.project().settings.fps as f64;
        let at = |s: &Session, i: usize| small_picture(s.project(), i as f64 / fps);
        let (first, last) = (at(&s, 0), at(&s, 2));
        s.command(&json!({"op": "delete_frame"}));
        let close = |a: &Vec<u8>, b: &Vec<u8>| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= 2);
        if s.state()["frames"] != 2 {
            failures.push(format!("{name}: {} frames left", s.state()["frames"]));
            continue;
        }
        // (a movieclip plays along the project's time: the last frame moved in time)
        let timeline = name.starts_with("movieclip");
        if !timeline && !close(&at(&s, 0), &first) {
            failures.push(format!("{name}: the first frame changed"));
        }
        if !timeline && !close(&at(&s, 1), &last) {
            failures.push(format!("{name}: the last frame changed (or is not where it was)"));
        }
        s.command(&json!({"op": "undo"}));
        if s.state()["frames"] != 3 || !close(&at(&s, 2), &last) {
            failures.push(format!("{name}: undo of the delete is not the same project"));
        }
    }
    assert!(failures.is_empty(), "{} differences:\n{}", failures.len(), failures.join("\n"));
}

/// Layer up / Layer down step past the next layer that draws: with a camera
/// (or a sound layer) in between, one press still changes what is in front.
#[test]
fn layer_up_and_down_step_over_layers_that_do_not_draw() {
    use barkfart_core::model::LayerKind;
    let mut s = two_poses();
    s.command(&json!({"op": "add_camera"}));
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let at = |s: &Session, id: &str| s.project().root().unwrap().layers.iter().position(|l| l.id == id).unwrap();
    let cam = s.project().root().unwrap().layers.iter().find(|l| matches!(l.kind, LayerKind::Camera(_))).unwrap().id.clone();
    // the man is below the ball, with the camera between them or not
    assert!(at(&s, "L2") < at(&s, &ball));
    let st = s.command(&json!({"op": "layer_order", "forward": true}));
    assert!(at(&s, "L2") > at(&s, &ball), "one press: the man is in front of the ball");
    assert_eq!(st["undo"], "Bring forward");
    s.command(&json!({"op": "layer_order", "forward": false}));
    assert!(at(&s, "L2") < at(&s, &ball), "one press back: behind the ball again");
    // at the end of the stack nothing is left to pass
    let before = saved_text(s.project());
    s.command(&json!({"op": "layer_order", "forward": false}));
    assert_eq!(saved_text(s.project()), before, "already at the back");
    let _ = cam;
}

/// Shifting keys when a frame is inserted or deleted drifts a hair off the
/// frame times (0.2 + 0.1 is not 0.3). A figure that is on one frame only
/// must stay off the others, however many frames come and go around it.
#[test]
fn inserting_and_deleting_frames_keep_a_figure_on_its_own_frames() {
    let mut s = two_poses();
    // three frames; a ball on the middle one only
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    let fps = s.project().settings.fps as f64;
    let shown = |s: &Session| -> Vec<bool> {
        let p = s.project();
        let n = s.state()["frames"].as_i64().unwrap();
        let l = p.root().unwrap().layer(&ball).unwrap();
        let mut r = barkfart_core::eval::Resolver::new(p);
        (0..n).map(|i| r.b(&l.shown, i as f64 / fps)).collect()
    };
    assert_eq!(shown(&s), [false, true, false]);
    for (frame, want) in [(1, vec![false, true, true, false]), (2, vec![false, true, true, true, false]), (0, vec![false, false, true, true, true, false])] {
        s.command(&json!({"op": "frame", "i": frame}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        s.command(&json!({"op": "insert_frame"}));
        assert_eq!(shown(&s), want, "a frame inserted after frame {}", frame + 1);
        assert!(stray_keys(s.project()).is_empty(), "no keys a hair apart after inserting after frame {}", frame + 1);
    }
    // and taking them away again (the new frame 1 is a copy of the first)
    for _ in 0..3 {
        s.command(&json!({"op": "frame", "i": 0}));
        s.command(&json!({"op": "delete_frame"}));
        assert!(stray_keys(s.project()).is_empty(), "no keys a hair apart after a delete");
        let now = shown(&s);
        assert_eq!(*now.last().unwrap(), false, "the last frame still has no ball: {now:?}");
    }
}

/// A figure that is on one frame only stays on that frame through frame rate
/// changes, pastes and deletes at awkward positions (keys are rescaled and
/// shifted, and float error must not move or add one).
#[test]
fn a_one_frame_figure_stays_on_its_frame_through_rate_changes_pastes_and_deletes() {
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    // five frames; the ball on frame 3 (index 2) only
    s.command(&json!({"op": "frame", "i": 2}));
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let ball = st["selected"]["layer"].as_str().unwrap().to_string();
    let shown = |s: &Session| -> Vec<bool> {
        let p = s.project();
        let fps = p.settings.fps as f64;
        let n = s.state()["frames"].as_i64().unwrap();
        let l = p.root().unwrap().layer(&ball).unwrap();
        let mut r = barkfart_core::eval::Resolver::new(p);
        (0..n).map(|i| r.b(&l.shown, i as f64 / fps)).collect()
    };
    assert_eq!(shown(&s), [false, false, true, false, false]);
    for fps in [24, 7, 30, 12, 10, 3, 25, 10] {
        s.command(&json!({"op": "fps", "v": fps}));
        assert_eq!(shown(&s), [false, false, true, false, false], "after the frame rate became {fps}");
        assert!(stray_keys(s.project()).is_empty(), "no keys a hair apart at {fps} fps");
    }
    // copy frame 3 and paste it after frame 5, then after frame 1
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "copy_frame", "n": 0}));
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "paste_frame"}));
    assert_eq!(shown(&s), [false, false, true, false, false, true], "pasted after the last frame");
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "paste_frame"}));
    assert_eq!(shown(&s), [false, true, false, true, false, false, true], "pasted after the first frame");
    assert!(stray_keys(s.project()).is_empty(), "no keys a hair apart after the pastes");
    // delete each frame in turn from the front: the pattern loses its first entry each time
    let mut want = shown(&s);
    while want.len() > 1 {
        s.command(&json!({"op": "frame", "i": 0}));
        s.command(&json!({"op": "delete_frame"}));
        want.remove(0);
        assert_eq!(shown(&s), want, "after deleting the first frame");
        assert!(stray_keys(s.project()).is_empty(), "no keys a hair apart after a delete");
    }
}

/// Random frame editing against a model: which figure is on which frame.
/// Inserts, deletes, pastes, figures added or deleted on one frame, and frame
/// rate changes in any order; after every step the engine must agree with the
/// model and keep no keys a hair apart.
#[test]
fn random_frame_editing_keeps_every_figure_on_exactly_its_frames() {
    use std::collections::BTreeMap;
    let mut failures = Vec::new();
    for seed in 1..=40u64 {
        let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        let n0 = s.state()["frames"].as_i64().unwrap() as usize;
        let mut model: BTreeMap<String, Vec<bool>> = BTreeMap::new();
        model.insert("L2".into(), vec![true; n0]);
        let mut clip: Option<BTreeMap<String, bool>> = None;
        let mut fig_clip: Option<String> = None;
        let mut log: Vec<String> = Vec::new();
        for step in 0..30 {
            let n = model.values().next().map(|v| v.len()).unwrap_or(1).max(s.state()["frames"].as_i64().unwrap() as usize);
            let f = next(n);
            s.command(&json!({"op": "frame", "i": f as i64}));
            match next(11) {
                9 => {
                    // copy a figure that is on this frame
                    let ids: Vec<String> = model.iter().filter(|(k, v)| v[f] && s.project().root().and_then(|r| r.layer(k)).is_some_and(|l| l.as_figure().is_some())).map(|(k, _)| k.clone()).collect();
                    if !ids.is_empty() {
                        let id = ids[next(ids.len())].clone();
                        log.push(format!("copy {id} on {f}"));
                        s.command(&json!({"op": "select", "layer": id.clone()}));
                        s.command(&json!({"op": "copy_figure"}));
                        fig_clip = Some(id);
                    }
                }
                10 => {
                    // paste it here: the same figure where it is not, a new one where it is
                    if let Some(cid) = fig_clip.clone() {
                        log.push(format!("paste {cid} on {f}"));
                        let before: std::collections::HashSet<String> = s.project().root().unwrap().layers.iter().map(|l| l.id.clone()).collect();
                        s.command(&json!({"op": "paste_figure"}));
                        match model.get_mut(&cid) {
                            Some(v) if !v[f] => v[f] = true,
                            _ => {
                                let made: Vec<String> = s.project().root().unwrap().layers.iter().map(|l| l.id.clone()).filter(|id| !before.contains(id)).collect();
                                for id in made {
                                    let mut v = vec![false; n];
                                    v[f] = true;
                                    model.insert(id, v);
                                }
                            }
                        }
                    }
                }
                8 => {
                    // a textfield is on its frame only, like a figure
                    log.push(format!("add a textfield on {f}"));
                    let before: std::collections::HashSet<String> = s.project().root().unwrap().layers.iter().map(|l| l.id.clone()).collect();
                    s.command(&json!({"op": "add_textfield"}));
                    let made: Vec<String> = s.project().root().unwrap().layers.iter().map(|l| l.id.clone()).filter(|id| !before.contains(id)).collect();
                    for id in made {
                        let mut v = vec![false; n];
                        v[f] = true;
                        model.insert(id, v);
                    }
                }
                0 | 1 => {
                    log.push(format!("insert after {f}"));
                    s.command(&json!({"op": "insert_frame"}));
                    for v in model.values_mut() {
                        let c = v[f];
                        v.insert(f + 1, c);
                    }
                }
                2 => {
                    log.push(format!("delete frame {f}"));
                    if n > 1 {
                        s.command(&json!({"op": "delete_frame"}));
                        for v in model.values_mut() {
                            v.remove(f);
                        }
                    }
                }
                3 => {
                    log.push(format!("add a ball on {f}"));
                    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
                    if let Some(id) = st["selected"]["layer"].as_str() {
                        let mut v = vec![false; n];
                        v[f] = true;
                        model.insert(id.to_string(), v);
                    }
                }
                4 => {
                    // delete a figure from this frame
                    // (a textfield is selected by tapping it, not by id: figures only)
                    let ids: Vec<String> = model.iter().filter(|(k, v)| v[f] && s.project().root().and_then(|r| r.layer(k)).is_some_and(|l| l.as_figure().is_some())).map(|(k, _)| k.clone()).collect();
                    if !ids.is_empty() {
                        let id = ids[next(ids.len())].clone();
                        log.push(format!("delete {id} on {f}"));
                        s.command(&json!({"op": "select", "layer": id.clone()}));
                        s.command(&json!({"op": "delete_here"}));
                        model.get_mut(&id).unwrap()[f] = false;
                    }
                }
                5 => {
                    log.push(format!("copy frame {f}"));
                    s.command(&json!({"op": "copy_frame", "n": 0}));
                    clip = Some(model.iter().map(|(k, v)| (k.clone(), v[f])).collect());
                }
                6 => {
                    if let Some(c) = &clip {
                        log.push(format!("paste after {f}"));
                        s.command(&json!({"op": "paste_frame"}));
                        for (id, v) in model.iter_mut() {
                            let val = c.get(id).copied().unwrap_or(v[f]);
                            v.insert(f + 1, val);
                        }
                    }
                }
                _ => {
                    let fps = [24, 7, 30, 12, 10, 25, 5][next(7)];
                    log.push(format!("fps {fps}"));
                    s.command(&json!({"op": "fps", "v": fps}));
                }
            }
            // a figure on no frame at all is removed
            let p = s.project();
            let fps = p.settings.fps as f64;
            let frames = s.state()["frames"].as_i64().unwrap() as usize;
            model.retain(|id, v| v.iter().any(|x| *x) || p.root().unwrap().layer(id).is_some() && false);
            let mut bad = Vec::new();
            for (id, want) in &model {
                let Some(l) = p.root().and_then(|r| r.layer(id)) else {
                    bad.push(format!("{id} is gone"));
                    continue;
                };
                let mut r = barkfart_core::eval::Resolver::new(p);
                let got: Vec<bool> = (0..frames).map(|i| r.b(&l.shown, i as f64 / fps)).collect();
                if got != *want {
                    bad.push(format!("{id}: want {want:?} got {got:?}"));
                }
            }
            if frames != model.values().next().map(|v| v.len()).unwrap_or(frames) {
                bad.push(format!("{frames} frames, the model has {}", model.values().next().map(|v| v.len()).unwrap_or(0)));
            }
            if !stray_keys(p).is_empty() {
                bad.push(format!("keys a hair apart on {:?}", stray_keys(p).iter().take(3).collect::<Vec<_>>()));
            }
            if !bad.is_empty() {
                failures.push(format!("seed {seed}, step {step} ({}): {}\n    after: {}", log.last().cloned().unwrap_or_default(), bad.join("; "), log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
}

/// Random editing against a model of the per-frame settings: the background
/// colour and the pause travel with their frame through inserts (a new frame
/// copies the look, not the timing), deletes and pastes (which copy both).
#[test]
fn random_frame_editing_keeps_every_frame_setting_with_its_frame() {
    let mut failures = Vec::new();
    let colours = [0x336699ffu32, 0xcc3333ffu32, 0x33cc66ffu32];
    let delays = [0.1f64, 0.2, 0.5];
    for seed in 1..=24u64 {
        let mut rng = seed.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        s.command(&json!({"op": "add_camera"}));
        let n0 = s.state()["frames"].as_i64().unwrap() as usize;
        let read = |s: &mut Session, i: usize| -> (u32, f64) {
            s.command(&json!({"op": "frame", "i": i as i64}));
            let st = s.state();
            (st["frameBg"]["color"].as_u64().unwrap() as u32, st["frameDelay"].as_f64().unwrap())
        };
        let mut model: Vec<(u32, f64)> = (0..n0).map(|i| read(&mut s, i)).collect();
        let mut clip: Option<(u32, f64)> = None;
        let mut log: Vec<String> = Vec::new();
        for step in 0..30 {
            let n = model.len();
            let f = next(n);
            s.command(&json!({"op": "frame", "i": f as i64}));
            match next(8) {
                0 => {
                    let c = colours[next(3)];
                    log.push(format!("background {c:08x} on {f}"));
                    s.command(&json!({"op": "frame_bg", "color": c}));
                    model[f].0 = c;
                }
                1 => {
                    let d = delays[next(3)];
                    log.push(format!("pause {d} on {f}"));
                    s.command(&json!({"op": "frame_delay", "seconds": d}));
                    model[f].1 = d;
                }
                2 | 3 => {
                    log.push(format!("insert after {f}"));
                    s.command(&json!({"op": "insert_frame"}));
                    let look = model[f].0;
                    model.insert(f + 1, (look, 0.0));
                }
                4 => {
                    log.push(format!("delete {f}"));
                    if n > 1 {
                        s.command(&json!({"op": "delete_frame"}));
                        model.remove(f);
                    }
                }
                5 => {
                    log.push(format!("copy {f}"));
                    s.command(&json!({"op": "copy_frame", "n": 0}));
                    clip = Some(model[f]);
                }
                6 => {
                    if let Some(c) = clip {
                        log.push(format!("paste after {f}"));
                        s.command(&json!({"op": "paste_frame"}));
                        model.insert(f + 1, c);
                    }
                }
                _ => {
                    let fps = [24, 7, 30, 12, 10, 25, 5][next(7)];
                    log.push(format!("fps {fps}"));
                    s.command(&json!({"op": "fps", "v": fps}));
                }
            }
            let frames = s.state()["frames"].as_i64().unwrap() as usize;
            let mut bad = Vec::new();
            if frames != model.len() {
                bad.push(format!("{frames} frames, the model has {}", model.len()));
            } else {
                for (i, want) in model.clone().iter().enumerate() {
                    let got = read(&mut s, i);
                    if got.0 != want.0 || (got.1 - want.1).abs() > 1e-6 {
                        bad.push(format!("frame {i}: want ({:08x}, {}) got ({:08x}, {})", want.0, want.1, got.0, got.1));
                    }
                }
            }
            if !stray_keys(s.project()).is_empty() {
                bad.push(format!("keys a hair apart on {:?}", stray_keys(s.project()).iter().take(3).collect::<Vec<_>>()));
            }
            if !bad.is_empty() {
                failures.push(format!("seed {seed}, step {step} ({}): {}\n    after: {}", log.last().cloned().unwrap_or_default(), bad.join("; "), log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
}

/// Random editing against a model of a pose: the angle of the man's upper
/// right arm on each frame travels with its frame through inserts, deletes,
/// pastes and frame rate changes.
#[test]
fn random_frame_editing_keeps_every_pose_with_its_frame() {
    let mut failures = Vec::new();
    for seed in 1..=12u64 {
        let mut rng = seed.wrapping_mul(1103515245).wrapping_add(12345);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        let n0 = s.state()["frames"].as_i64().unwrap() as usize;
        let read = |s: &mut Session, i: usize| -> (f64, f64) {
            s.command(&json!({"op": "frame", "i": i as i64}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            let st = s.state();
            let at = st["figures"].as_array().unwrap().iter().find(|f| f["id"] == "L2").map(|f| f["at"][0].as_f64().unwrap()).unwrap_or(0.0);
            (st["selected"]["angle"].as_f64().unwrap(), at)
        };
        let mut model: Vec<(f64, f64)> = (0..n0).map(|i| read(&mut s, i)).collect();
        let mut clip: Option<(f64, f64)> = None;
        let mut log: Vec<String> = Vec::new();
        for step in 0..30 {
            let n = model.len();
            let f = next(n);
            s.command(&json!({"op": "frame", "i": f as i64}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            match next(9) {
                0 | 1 => {
                    let a = (next(12) as f64) * 15.0 - 90.0;
                    log.push(format!("angle {a} on {f}"));
                    s.command(&json!({"op": "node", "angle": a}));
                    model[f].0 = a;
                }
                8 => {
                    let dx = (next(5) as f64 + 1.0) * 10.0;
                    log.push(format!("nudge {dx} on {f}"));
                    s.command(&json!({"op": "nudge", "dx": dx, "dy": 0}));
                    model[f].1 += dx;
                }
                2 | 3 => {
                    log.push(format!("insert after {f}"));
                    s.command(&json!({"op": "insert_frame"}));
                    let v = model[f];
                    model.insert(f + 1, v);
                }
                4 => {
                    log.push(format!("delete {f}"));
                    if n > 1 {
                        s.command(&json!({"op": "delete_frame"}));
                        model.remove(f);
                    }
                }
                5 => {
                    log.push(format!("copy {f}"));
                    s.command(&json!({"op": "copy_frame", "n": 0}));
                    clip = Some(model[f]);
                }
                6 => {
                    if let Some(c) = clip {
                        log.push(format!("paste after {f}"));
                        s.command(&json!({"op": "paste_frame"}));
                        model.insert(f + 1, c);
                    }
                }
                _ => {
                    let fps = [24, 7, 30, 12, 10, 25, 5][next(7)];
                    log.push(format!("fps {fps}"));
                    s.command(&json!({"op": "fps", "v": fps}));
                }
            }
            let frames = s.state()["frames"].as_i64().unwrap() as usize;
            let mut bad = Vec::new();
            if frames != model.len() {
                bad.push(format!("{frames} frames, the model has {}", model.len()));
            } else {
                for (i, want) in model.clone().iter().enumerate() {
                    let got = read(&mut s, i);
                    if (got.0 - want.0).abs() > 0.02 || (got.1 - want.1).abs() > 0.05 {
                        bad.push(format!("frame {i}: want {want:?} got {got:?}"));
                    }
                }
            }
            if !stray_keys(s.project()).is_empty() {
                bad.push(format!("keys a hair apart on {:?}", stray_keys(s.project()).iter().take(3).collect::<Vec<_>>()));
            }
            if !bad.is_empty() {
                failures.push(format!("seed {seed}, step {step} ({}): {}\n    after: {}", log.last().cloned().unwrap_or_default(), bad.join("; "), log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
}

/// Random editing with undo and redo mixed in: the undo and redo stacks and
/// the project always walk the history of its saved texts exactly (undo gives
/// the text before the edit, redo the text after it, a new edit after an undo
/// drops the redo steps), and an edit that changes the project is one step.
#[test]
fn random_editing_with_undo_and_redo_walks_the_history_exactly() {
    let mut failures = Vec::new();
    for seed in 1..=24u64 {
        let mut rng = seed.wrapping_mul(2685821657736338717).wrapping_add(11);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        s.command(&json!({"op": "add_camera"}));
        let base = s.doc.undo_depth();
        let mut history = vec![saved_text(s.project())];
        let mut at = 0usize;
        let mut log: Vec<String> = Vec::new();
        for step in 0..40 {
            let frames = s.state()["frames"].as_i64().unwrap();
            let f = next(frames as usize) as i64;
            s.command(&json!({"op": "frame", "i": f}));
            s.command(&json!({"op": "select", "layer": "L2", "node": (["armR1", "armR2", "legL1", "head"][next(4)])}));
            let c: Value = match next(54) {
                45 => json!({"op": "add_segment", "len": 20 + next(60) as i64}),
                46 => json!({"op": "delete_segment"}),
                47 => json!({"op": "split", "n": 2 + next(3) as i64}),
                48 => json!({"op": "duplicate_branch"}),
                49 => json!({"op": "flip_branch", "h": next(2) == 0}),
                50 => json!({"op": "reorder", "branch": next(2) == 0, "to": ([-2, -1, 1, 2][next(4)])}),
                51 => json!({"op": "copy_branch", "branch": next(2) == 0}),
                52 => json!({"op": "paste_branch"}),
                53 => json!({"op": "delete_joint"}),
                41 | 42 => json!({"op": "@drag"}),
                43 => json!({"op": "@drag_cancelled"}),
                38 | 39 => json!({"op": "@join"}),
                40 => json!({"op": "unjoin"}),
                34 => json!({"op": "to_movieclip"}),
                35 | 36 => json!({"op": "@enter_movieclip"}),
                37 => json!({"op": "movieclip_done"}),
                0 => json!({"op": "insert_frame"}),
                1 => json!({"op": "delete_frame"}),
                2 => json!({"op": "fps", "v": ([24, 12, 10, 30][next(4)])}),
                3 => json!({"op": "figure", "scale": 1.0 + next(5) as f64 * 0.2}),
                4 => json!({"op": "figure", "color": ([0xff0000ffu32, 0x00ff00ffu32, 0x0000ffffu32][next(3)])}),
                5 => json!({"op": "figure", "turn": 15 * (next(5) as i64 + 1)}),
                6 => json!({"op": "segment", "thickness": 4 + next(30) as i64}),
                7 => json!({"op": "segment", "color": ([0xff00ffffu32, 0x00ffffffu32][next(2)])}),
                8 => json!({"op": "segment", "shape": (["circle", "trapezoid", "triangle", "line"][next(4)])}),
                9 => json!({"op": "segment", "gradient": next(2) == 0, "gradientColor": 0x00ff00ffu32}),
                10 => json!({"op": "segment", "ownColor": next(2) == 0}),
                11 => json!({"op": "node", "angle": next(24) as i64 * 15 - 180}),
                12 => json!({"op": "nudge", "dx": next(9) as i64 - 4, "dy": next(9) as i64 - 4}),
                13 => json!({"op": "flip_figure", "h": next(2) == 0}),
                14 => json!({"op": "layer_order", "forward": next(2) == 0}),
                15 => json!({"op": "add_figure", "kind": "ball"}),
                16 => json!({"op": "duplicate_figure"}),
                17 => json!({"op": "delete_here"}),
                18 => json!({"op": "filter", "invert": next(2) == 0, "blur": next(8) as i64}),
                19 => json!({"op": "clear_filters"}),
                20 => json!({"op": "frame_bg", "color": ([0x336699ffu32, 0xcc3333ffu32][next(2)]), "useGradient": next(2) == 0, "expand": next(2) == 0}),
                21 => json!({"op": "camera_fx", "widescreen": next(2) == 0, "wobbleXY": next(4) as i64}),
                22 => json!({"op": "camera", "zoom": 1.0 + next(4) as f64 * 0.25}),
                23 => json!({"op": "frame_delay", "seconds": next(4) as f64 * 0.1}),
                24 => json!({"op": "frame_slow", "n": next(4) as i64}),
                25 => json!({"op": "copy_frame", "n": next(2) as i64}),
                26 => json!({"op": "paste_frame"}),
                27 => json!({"op": "copy_figure"}),
                28 => json!({"op": "paste_figure"}),
                29 => json!({"op": "add_textfield"}),
                30 => json!({"op": "no_tween", "on": next(2) == 0}),
                31 => json!({"op": "tween_add", "at": [0.5]}),
                32 => json!({"op": "pin", "v": next(2) == 0}),
                _ => json!({"op": "easing", "v": (["hold", "smooth", "in", "out", "linear"][next(5)])}),
            };
            let what = next(10);
            let mut why = String::new();
            if what < 2 {
                if at == 0 {
                    continue; // not past where the test started
                }
                s.command(&json!({"op": "undo"}));
                log.push("undo".into());
                at -= 1;
            } else if what < 4 {
                if at + 1 >= history.len() {
                    if s.doc.redo_depth() != 0 {
                        why = format!("redo steps left at the end of the history ({})", s.doc.redo_depth());
                    }
                } else {
                    s.command(&json!({"op": "redo"}));
                    log.push("redo".into());
                    at += 1;
                }
            } else {
                let (before, depth) = (saved_text(s.project()), s.doc.undo_depth());
                if c["op"] == "@drag" || c["op"] == "@drag_cancelled" {
                    // a drag of a random joint: several moves, then up (or cancelled)
                    let t = s.time();
                    let hs = s.tools.handles(s.project(), t);
                    if !hs.is_empty() {
                        let h = &hs[next(hs.len())];
                        s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 20}));
                        for _ in 0..(1 + next(4)) {
                            s.command(&json!({"op": "move", "x": h.x + next(120) as f32 - 60.0, "y": h.y + next(120) as f32 - 60.0}));
                        }
                        s.command(&json!({"op": if c["op"] == "@drag" { "up" } else { "cancel" }}));
                    }
                } else if c["op"] == "@join" {
                    // join the selected figure to a joint of another one
                    s.command(&json!({"op": "join"}));
                    let t = s.time();
                    let other = s.tools.handles(s.project(), t).into_iter().find(|h| h.layer != "L2");
                    if let Some(h) = other {
                        s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 20}));
                        s.command(&json!({"op": "up"}));
                    }
                    s.command(&json!({"op": "join_cancel"}));
                } else if c["op"] == "@enter_movieclip" {
                    // select the first movieclip on the stage and edit it
                    let id = s.project().root().and_then(|r| r.layers.iter().find(|l| matches!(l.kind, barkfart_core::model::LayerKind::Symbol(_))).map(|l| l.id.clone()));
                    if let Some(id) = id {
                        s.command(&json!({"op": "select", "layer": id, "node": ""}));
                        s.command(&json!({"op": "edit_movieclip"}));
                    }
                } else {
                    s.command(&c);
                }
                log.push(c.to_string());
                let after = saved_text(s.project());
                let stepped = s.doc.undo_depth() > depth;
                if stepped && after == before {
                    why = format!("{c} left an undo step that changes nothing");
                } else if !stepped && after != before {
                    why = format!("{c} changed the project without an undo step");
                } else if stepped {
                    history.truncate(at + 1);
                    history.push(after);
                    at += 1;
                }
            }
            if why.is_empty() {
                let now = saved_text(s.project());
                if now != history[at] {
                    let first = now.lines().zip(history[at].lines()).position(|(a, b)| a != b).unwrap_or(0);
                    why = format!("the project is not the history's text {at} (first difference at line {first})");
                } else if s.doc.undo_depth() != base + at {
                    why = format!("{} steps to undo, the history says {}", s.doc.undo_depth() - base.min(s.doc.undo_depth()), at);
                } else if s.doc.redo_depth() != history.len() - 1 - at {
                    why = format!("{} steps to redo, the history says {}", s.doc.redo_depth(), history.len() - 1 - at);
                } else if let Some(msg) = {
                    // every figure is one tree of joints: one root, parents present, no cycles
                    s.project().root().unwrap().layers.iter().find_map(|l| {
                        let f = l.as_figure()?;
                        let nodes = f.nodes();
                        let roots = nodes.iter().filter(|n| n.parent.is_none()).count();
                        if roots != 1 {
                            return Some(format!("figure {} has {roots} main nodes", l.id));
                        }
                        for n in nodes {
                            let mut cur = n;
                            for _ in 0..=nodes.len() {
                                match &cur.parent {
                                    None => break,
                                    Some(pid) => match f.node(pid) {
                                        None => return Some(format!("figure {}: joint {} hangs off {pid}, which is gone", l.id, cur.id)),
                                        Some(par) => cur = par,
                                    },
                                }
                            }
                            if cur.parent.is_some() {
                                return Some(format!("figure {}: joint {} is its own ancestor", l.id, n.id));
                            }
                        }
                        None
                    })
                } {
                    why = msg;
                } else if let Some(msg) = {
                    // every layer's parent exists, and no layer is its own ancestor
                    let layers = &s.project().root().unwrap().layers;
                    layers.iter().find_map(|l| {
                        let mut cur = l;
                        for _ in 0..=layers.len() {
                            match &cur.parent {
                                None => return None,
                                Some(pid) => match layers.iter().find(|x| &x.id == pid) {
                                    None => return Some(format!("layer {} hangs off {pid}, which is gone", cur.id)),
                                    Some(par) => cur = par,
                                },
                            }
                        }
                        Some(format!("layer {} is its own ancestor", l.id))
                    })
                } {
                    why = msg;
                } else if let Some(msg) = {
                    // what is selected is still there (a panel on a deleted figure is a bug)
                    let st = s.state();
                    let sel = &st["selected"];
                    match (sel["layer"].as_str(), sel["node"].as_str()) {
                        (Some(l), Some(n)) => match s.project().root().and_then(|r| r.layer(l)) {
                            None => Some(format!("the selected figure {l} is gone")),
                            Some(layer) => match layer.as_figure() {
                                Some(f) if f.node(n).is_none() => Some(format!("the selected joint {n} is gone from {l}")),
                                _ => None,
                            },
                        },
                        _ => None,
                    }
                } {
                    why = msg;
                } else {
                    // the .bark file the app saves (the main animation, even inside a movieclip)
                    let bark = s.save_copy().map_err(|e| format!("save failed: {e}")).and_then(|bytes| codec::read_bark(&bytes).map_err(|e| format!("the saved file does not open: {e:?}")));
                    match bark {
                        Ok(b) if saved_text(&b.project) != saved_text(&s.saved_project()) => why = "the saved .bark file opens as another project".to_string(),
                        Err(e) => why = e,
                        Ok(_) => {}
                    }
                }
                if why.is_empty() {
                    // saved and read back: the very same project
                    match codec::decode(&codec::encode(s.project())) {
                        Ok(back) => {
                            if saved_text(&back) != now {
                                let first = saved_text(&back).lines().zip(now.lines()).position(|(a, b)| a != b).unwrap_or(0);
                                why = format!("the saved project reads back as another project (first difference at line {first})");
                            }
                        }
                        Err(e) => why = format!("the saved project does not read back: {e:?}"),
                    }
                }
            }
            if !why.is_empty() {
                failures.push(format!("seed {seed}, step {step}: {why}\n    after: {}", log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(4).cloned().collect::<Vec<_>>().join("\n"));
}

/// Random editing against a model of three more per-frame values: the camera
/// zoom, the invert filter and the blur filter of the man (keyed on every
/// frame, as in a Stick Nodes project). Inserted frames
/// copy them, pasted frames bring the copied frame's, deleted frames take
/// theirs along, and a frame rate change leaves them on their frames.
#[test]
fn random_frame_editing_keeps_camera_and_filter_values_with_their_frames() {
    let mut failures = Vec::new();
    let zooms = [1.0f64, 1.5, 2.0];
    for seed in 1..=20u64 {
        let mut rng = seed.wrapping_mul(3935559000370003845).wrapping_add(2691343689449507681);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        // effects keyed on every frame, as Stick Nodes files arrive: then a
        // change on a frame stays on that frame (with keying on, D5)
        s.command(&json!({"op": "set", "k": "keying", "v": true}));
        for i in 0..s.state()["frames"].as_i64().unwrap() {
            s.command(&json!({"op": "frame", "i": i}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            s.command(&json!({"op": "filter", "invert": false, "blur": 0}));
        }
        s.command(&json!({"op": "add_camera"}));
        let read = |s: &mut Session, i: usize| -> (f64, bool, f64) {
            s.command(&json!({"op": "frame", "i": i as i64}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            let st = s.state();
            (st["cam"]["zoom"].as_f64().unwrap(), st["selected"]["filters"]["invert"].as_bool().unwrap_or(false), st["selected"]["filters"]["blur"].as_f64().unwrap_or(0.0))
        };
        let n0 = s.state()["frames"].as_i64().unwrap() as usize;
        let mut model: Vec<(f64, bool, f64)> = (0..n0).map(|i| read(&mut s, i)).collect();
        let mut clip: Option<(f64, bool, f64)> = None;
        let mut log: Vec<String> = Vec::new();
        for step in 0..24 {
            let n = model.len();
            let f = next(n);
            s.command(&json!({"op": "frame", "i": f as i64}));
            s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
            match next(9) {
                0 => {
                    let z = zooms[next(3)];
                    log.push(format!("zoom {z} on {f}"));
                    s.command(&json!({"op": "camera", "zoom": z}));
                    model[f].0 = z;
                }
                1 => {
                    let on = next(2) == 0;
                    log.push(format!("invert {on} on {f}"));
                    s.command(&json!({"op": "filter", "invert": on}));
                    model[f].1 = on;
                }
                2 => {
                    let b = (next(4) * 2) as f64;
                    log.push(format!("blur {b} on {f}"));
                    s.command(&json!({"op": "filter", "blur": b}));
                    model[f].2 = b;
                }
                3 | 4 => {
                    log.push(format!("insert after {f}"));
                    s.command(&json!({"op": "insert_frame"}));
                    let v = model[f];
                    model.insert(f + 1, v);
                }
                5 => {
                    log.push(format!("delete {f}"));
                    if n > 1 {
                        s.command(&json!({"op": "delete_frame"}));
                        model.remove(f);
                    }
                }
                6 => {
                    log.push(format!("copy {f}"));
                    s.command(&json!({"op": "copy_frame", "n": 0}));
                    clip = Some(model[f]);
                }
                7 => {
                    if let Some(c) = clip {
                        log.push(format!("paste after {f}"));
                        s.command(&json!({"op": "paste_frame"}));
                        model.insert(f + 1, c);
                    }
                }
                _ => {
                    let fps = [24, 7, 30, 12, 10, 25, 5][next(7)];
                    log.push(format!("fps {fps}"));
                    s.command(&json!({"op": "fps", "v": fps}));
                }
            }
            let frames = s.state()["frames"].as_i64().unwrap() as usize;
            let mut bad = Vec::new();
            if frames != model.len() {
                bad.push(format!("{frames} frames, the model has {}", model.len()));
            } else {
                for (i, want) in model.clone().iter().enumerate() {
                    let got = read(&mut s, i);
                    if (got.0 - want.0).abs() > 0.02 || got.1 != want.1 || (got.2 - want.2).abs() > 0.02 {
                        bad.push(format!("frame {i}: want {want:?} got {got:?}"));
                    }
                }
            }
            if !stray_keys(s.project()).is_empty() {
                bad.push(format!("keys a hair apart on {:?}", stray_keys(s.project()).iter().take(3).collect::<Vec<_>>()));
            }
            if !bad.is_empty() {
                failures.push(format!("seed {seed}, step {step} ({}): {}\n    after: {}", log.last().cloned().unwrap_or_default(), bad.join("; "), log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
}

/// Stickfigure filters belong to a frame: clearing them, or pasting a copied
/// set, on one frame leaves the other frames' filters as they were.
#[test]
fn clearing_or_pasting_filters_on_one_frame_leaves_the_other_frames_alone() {
    let mut s = two_poses();
    s.command(&json!({"op": "set", "k": "keying", "v": true})); // per-frame effects are keyed (D5)
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    let on = |s: &mut Session, frame: i64| -> (bool, f64) {
        s.command(&json!({"op": "frame", "i": frame}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        let st = s.state();
        (st["selected"]["filters"]["invert"].as_bool().unwrap(), st["selected"]["filters"]["blur"].as_f64().unwrap())
    };
    for f in 0..3 {
        s.command(&json!({"op": "frame", "i": f}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        s.command(&json!({"op": "filter", "invert": true, "blur": 4.0 + f as f64}));
    }
    assert_eq!([on(&mut s, 0), on(&mut s, 1), on(&mut s, 2)], [(true, 4.0), (true, 5.0), (true, 6.0)]);
    // clear them on the middle frame only
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "clear_filters"}));
    assert_eq!([on(&mut s, 0), on(&mut s, 1), on(&mut s, 2)], [(true, 4.0), (false, 0.0), (true, 6.0)], "cleared on the middle frame only");
    // copy the last frame's filters and paste them on the first
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "copy_filters"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "clear_filters"}));
    s.command(&json!({"op": "paste_filters"}));
    assert_eq!([on(&mut s, 0), on(&mut s, 1), on(&mut s, 2)], [(true, 6.0), (false, 0.0), (true, 6.0)], "pasted on the first frame only");
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "paste_filters"}));
    assert_eq!([on(&mut s, 0), on(&mut s, 1), on(&mut s, 2)], [(true, 6.0), (true, 6.0), (true, 6.0)], "pasted on the middle frame only");
}

/// Random editing against a model of which frames play a sound: a frame's sound
/// stays on its frame through inserts (a new frame has none), deletes (the
/// sound goes with its frame) and frame rate changes.
#[test]
fn random_frame_editing_keeps_every_frame_sound_on_its_frame() {
    use barkfart_core::audio::{encode_wav, tone};
    let mut failures = Vec::new();
    for seed in 1..=20u64 {
        let mut rng = seed.wrapping_mul(9600629759793949339).wrapping_add(7);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        let st = s.import("beep.wav", &encode_wav(&tone(440.0, 0.2, 8000, 0.5)));
        let id = st["soundList"][0]["id"].as_str().unwrap().to_string();
        let n0 = s.state()["frames"].as_i64().unwrap() as usize;
        let mut model = vec![false; n0];
        let read = |s: &mut Session, i: usize| -> bool {
            s.command(&json!({"op": "frame", "i": i as i64}));
            !s.state()["frameSound"].is_null()
        };
        let mut log: Vec<String> = Vec::new();
        for step in 0..30 {
            let n = model.len();
            let f = next(n);
            s.command(&json!({"op": "frame", "i": f as i64}));
            match next(7) {
                0 | 1 => {
                    log.push(format!("sound on {f}"));
                    s.command(&json!({"op": "frame_sound", "asset": id}));
                    model[f] = true;
                }
                2 => {
                    log.push(format!("clear {f}"));
                    s.command(&json!({"op": "frame_sound", "asset": null}));
                    model[f] = false;
                }
                3 | 4 => {
                    log.push(format!("insert after {f}"));
                    s.command(&json!({"op": "insert_frame"}));
                    model.insert(f + 1, false);
                }
                5 => {
                    log.push(format!("delete {f}"));
                    if n > 1 {
                        s.command(&json!({"op": "delete_frame"}));
                        model.remove(f);
                    }
                }
                _ => {
                    let fps = [24, 7, 30, 12, 10, 25, 5][next(7)];
                    log.push(format!("fps {fps}"));
                    s.command(&json!({"op": "fps", "v": fps}));
                }
            }
            let frames = s.state()["frames"].as_i64().unwrap() as usize;
            let mut bad = Vec::new();
            if frames != model.len() {
                bad.push(format!("{frames} frames, the model has {}", model.len()));
            } else {
                let got: Vec<bool> = (0..frames).map(|i| read(&mut s, i)).collect();
                if got != model {
                    bad.push(format!("sounds on {got:?}, the model says {model:?}"));
                }
            }
            if !bad.is_empty() {
                failures.push(format!("seed {seed}, step {step} ({}): {}\n    after: {}", log.last().cloned().unwrap_or_default(), bad.join("; "), log.join(", ")));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(4).cloned().collect::<Vec<_>>().join("\n"));
}

/// Timing probe for big projects (100 figures, 30 frames). Run in release mode:
/// cargo test --release -p barkfart-core --test session -- --ignored --nocapture edit_time
/// 2026-11-22 (Intel i7-class dev machine): handles() 4.7 ms, state() 30 ms, an
/// edit 45 ms; before the Resolver was shared in handles(): 147, 490 and 545 ms
/// (a Resolver indexes every property of the project when it is made).
#[test]
#[ignore = "timing probe; run with --ignored --nocapture"]
fn edit_time_on_a_big_project() {
    let mut s = two_poses();
    for _ in 0..28 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    for _ in 0..99 {
        s.command(&json!({"op": "duplicate_figure"}));
    }
    let n = s.state()["figures"].as_array().unwrap().len();
    let time = |f: &mut dyn FnMut()| {
        let t = std::time::Instant::now();
        for _ in 0..10 {
            f();
        }
        t.elapsed().as_secs_f64() * 100.0
    };
    let edit = time(&mut || {
        s.command(&json!({"op": "segment", "thickness": 12}));
        s.command(&json!({"op": "segment", "thickness": 14}));
    }) / 2.0;
    let state = time(&mut || {
        let _ = s.state();
    });
    let handles = time(&mut || {
        let _ = s.tools.handles(s.project(), s.time());
    });
    s.command(&json!({"op": "frame", "i": 3}));
    let copy = time(&mut || {
        s.command(&json!({"op": "copy_frame", "n": 0}));
    });
    let paste = time(&mut || {
        s.command(&json!({"op": "paste_frame"}));
    });
    eprintln!("TIMING figures={n} frames={}: edit {edit:.1} ms, state() {state:.1} ms, handles() {handles:.1} ms, copy frame {copy:.1} ms, paste frame {paste:.1} ms", s.state()["frames"]);
}

/// A movieclip has its own frames: adding, deleting and pasting frames while
/// editing it leaves the main animation's frames and pictures alone, the
/// saved project is always the main animation, and leaving the clip puts the
/// main timeline back.
#[test]
fn editing_a_movieclips_frames_leaves_the_main_animation_alone() {
    let mut s = two_poses();
    s.command(&json!({"op": "to_movieclip"}));
    let id = s.project().root().unwrap().layers.iter().find(|l| matches!(l.kind, barkfart_core::model::LayerKind::Symbol(_))).unwrap().id.clone();
    let fps = s.project().settings.fps as f64;
    let main_frames = s.state()["frames"].as_i64().unwrap();
    let main_pics: Vec<Vec<u8>> = (0..main_frames).map(|i| small_picture(s.project(), i as f64 / fps)).collect();
    let main_saved = saved_text(&s.saved_project());
    s.command(&json!({"op": "select", "layer": id, "node": ""}));
    s.command(&json!({"op": "edit_movieclip"}));
    let inside = s.state()["frames"].as_i64().unwrap();
    assert!(inside >= 1);
    for k in 0..3 {
        s.command(&json!({"op": "frame", "i": (inside - 1 + k) as i64}));
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "copy_frame", "n": 0}));
    s.command(&json!({"op": "paste_frame"}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "delete_frame"}));
    let edited = s.state()["frames"].as_i64().unwrap();
    assert_ne!(edited, inside, "the clip's own frames changed");
    // the saved project is the main animation all the while, not the clip being edited
    assert_ne!(s.saved_project().root_symbol, s.project().root_symbol, "saving writes the main root, not the clip's");
    assert_eq!(s.saved_project().root().unwrap().duration, main_frames as f64 / fps, "and it keeps the main animation's length");
    s.command(&json!({"op": "movieclip_done"}));
    assert_eq!(s.state()["frames"].as_i64().unwrap(), main_frames, "back on the main timeline");
    let now: Vec<Vec<u8>> = (0..main_frames).map(|i| small_picture(s.project(), i as f64 / fps)).collect();
    assert_eq!(now, main_pics, "the main animation draws what it drew (the clip's first frames are the same)");
    // the main animation's own text differs only in the clip's symbol
    let after = saved_text(&s.saved_project());
    let (a, b): (Vec<&str>, Vec<&str>) = (main_saved.lines().collect(), after.lines().collect());
    assert!(a.len() != b.len() || a != b, "the clip's edits are in the project");
    assert_eq!(s.project().root_symbol, s.saved_project().root_symbol, "the main root is the project's again");
}

/// Every command and the keys its handler reads (generated from session.rs; refresh when ops change).
const FUZZ_OPS: &[(&str, &[&str])] = &[
    ("add_camera", &[]),
    ("add_figure", &["kind"]),
    ("add_segment", &["len"]),
    ("add_textfield", &[]),
    ("animation", &["loop", "tweened", "tweening"]),
    ("auto_camera", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("branch_edit", &["addLength", "addThickness", "gradientColor", "ignoreZero", "ownColor", "replaceFrom", "replaceTo"]),
    ("break_apart", &["layer"]),
    ("build", &["layer"]),
    ("build_done", &[]),
    ("camera", &["center", "rotation", "zoom"]),
    ("camera_fx", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("camera_lock", &["layer", "on"]),
    ("camera_lock_set", &["layer", "parallax", "rotate"]),
    ("clear_filters", &[]),
    ("clear_frame", &[]),
    ("connect_cancel", &[]),
    ("connect_start", &[]),
    ("connector_set", &["percent", "reversed"]),
    ("copy_branch", &["branch"]),
    ("copy_figure", &["joined"]),
    ("copy_filters", &[]),
    ("copy_frame", &["n"]),
    ("copy_pose", &[]),
    ("copy_props", &[]),
    ("copy_sound", &[]),
    ("copy_textfield", &[]),
    ("cycle_figure", &["dir"]),
    ("cycle_joint", &["dir"]),
    ("delete_everywhere", &[]),
    ("delete_figure", &[]),
    ("delete_frame", &[]),
    ("delete_here", &["joined"]),
    ("delete_joint", &[]),
    ("delete_segment", &[]),
    ("delete_textfield", &[]),
    ("draw_tool", &["shape"]),
    ("duplicate_branch", &[]),
    ("duplicate_figure", &[]),
    ("easing", &["v"]),
    ("edit_movieclip", &[]),
    ("editor_bg", &["color"]),
    ("figure", &["angle", "center", "color", "keepInTween", "locked", "onion", "scale", "turn", "tweenColors"]),
    ("filter", &["blur", "glow", "glowAmount", "glowIntensity", "hue", "invert", "motion", "motionAngle", "motionDistance", "opacity", "outline", "outlineAmount", "outlineThickness", "pixelate", "saturation", "shadow", "shadowAngle", "shadowBlur", "shadowDistance", "shadowOpacity", "tint", "tintAmount"]),
    ("flip_branch", &["h"]),
    ("flip_figure", &["h"]),
    ("fps", &["v"]),
    ("frame", &["i"]),
    ("frame_bg", &[]),
    ("frame_delay", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("frame_repeat", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("frame_silence", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("frame_slow", &["back", "ease", "end", "n", "on", "seconds", "times", "tween", "widescreen", "wobbleRot", "wobbleSpeed", "wobbleXY", "wobbleZoom"]),
    ("frame_sound", &["asset", "pan", "pitch", "volume"]),
    ("insert_frame", &["copy"]),
    ("join", &[]),
    ("join_cancel", &[]),
    ("layer_order", &["forward"]),
    ("library_delete", &["layer"]),
    ("mode", &["v"]),
    ("movieclip", &["loop", "offset", "speed"]),
    ("movieclip_done", &[]),
    ("new_movieclip", &[]),
    ("no_tween", &["on"]),
    ("node", &["angle", "dragLock", "length", "stretchy"]),
    ("nudge", &["dx", "dy"]),
    ("paste_branch", &[]),
    ("paste_figure", &["layering"]),
    ("paste_filters", &["color", "expand", "gradient", "useGradient"]),
    ("paste_frame", &[]),
    ("paste_pose", &[]),
    ("paste_props", &["color", "filters", "layering", "nodes", "position", "rotation", "scale"]),
    ("paste_sound", &[]),
    ("paste_textfield", &[]),
    ("pin", &["v"]),
    ("polyfill_color", &["color", "own"]),
    ("polyfill_delete", &["color", "own"]),
    ("polyfill_edit", &["id"]),
    ("polyfill_new", &[]),
    ("polyfill_undo", &["color", "own"]),
    ("rename", &["layer", "name"]),
    ("reorder", &["branch", "to"]),
    ("reset_camera", &[]),
    ("rotate_cameras", &[]),
    ("rotate_figure", &["deg"]),
    ("segment", &["angleLock", "branch", "color", "curve", "gradient", "gradientColor", "gradientReverse", "gradientSideways", "halfArc", "hollow", "outline", "outlineColor", "ownColor", "rightTriangle", "scaleLength", "segScale", "shape", "sides", "smart", "static", "stretchy", "thickness", "trapRatio", "trapRoundEnd", "trapRoundStart", "upsideDown"]),
    ("select", &["layer", "node"]),
    ("set", &["k", "v"]),
    ("split", &["n"]),
    ("textfield", &["align", "color", "font", "locked", "scale", "text"]),
    ("to_movieclip", &[]),
    ("trace", &["frame", "layer"]),
    ("tween", &["on"]),
    ("tween_add", &["at", "n"]),
    ("unjoin", &[]),
    ("watermark", &["color", "on", "text"]),
];

/// Odd inputs: every command with huge, negative, empty and wrongly typed
/// values on the keys it reads. Nothing may panic, and what is left must still
/// save and read back, and still draw (at the first frame and in between).
#[test]
fn odd_inputs_never_panic_and_leave_a_project_that_saves_and_draws() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let pool: Vec<Value> = vec![
        json!(0), json!(1), json!(-1), json!(2), json!(7), json!(100), json!(1000000), json!(-1000000), json!(0.5), json!(-0.25), json!(1e-9),
        json!(true), json!(false), Value::Null, json!("x"), json!("circle"), json!("Bangers"), json!([]), json!([0.5]), json!({}), json!(4294967295u64), json!(0xff0000ffu32),
    ];
    let mut failures: Vec<String> = Vec::new();
    for seed in 1..=30u64 {
        let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut next = move |n: usize| -> usize {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 33) as usize) % n
        };
        let mut s = two_poses();
        s.command(&json!({"op": "add_camera"}));
        s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
        let mut log: Vec<String> = Vec::new();
        for step in 0..50 {
            let (op, keys) = FUZZ_OPS[next(FUZZ_OPS.len())];
            let mut c = serde_json::Map::new();
            c.insert("op".into(), json!(op));
            for k in keys {
                if next(10) < 6 {
                    c.insert((*k).into(), pool[next(pool.len())].clone());
                }
            }
            let c = Value::Object(c);
            log.push(c.to_string());
            if catch_unwind(AssertUnwindSafe(|| {
                s.command(&c);
            }))
            .is_err()
            {
                failures.push(format!("seed {seed}, step {step}: {c} panicked\n    after: {}", log.iter().rev().take(6).rev().cloned().collect::<Vec<_>>().join(", ")));
                break;
            }
            let ok = catch_unwind(AssertUnwindSafe(|| {
                let p = s.project();
                let back = codec::decode(&codec::encode(p));
                let fps = (p.settings.fps as f64).max(1.0);
                for t in [0.0, 0.5 / fps, 1.0 / fps] {
                    let _ = barkfart_core::tess::layered(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
                }
                back.is_ok()
            }));
            match ok {
                Err(_) => {
                    failures.push(format!("seed {seed}, step {step}: after {c} the project can't be drawn or saved (panic)\n    after: {}", log.iter().rev().take(6).rev().cloned().collect::<Vec<_>>().join(", ")));
                    break;
                }
                Ok(false) => {
                    failures.push(format!("seed {seed}, step {step}: after {c} the saved project does not read back\n    after: {}", log.iter().rev().take(6).rev().cloned().collect::<Vec<_>>().join(", ")));
                    break;
                }
                Ok(true) => {}
            }
        }
    }
    assert!(failures.is_empty(), "{} runs went wrong:\n{}", failures.len(), failures.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

/// The history keeps the last 200 steps and they are exact: after 260 edits, 200
/// undos land on the state after the 60th edit, nothing more can be undone, and
/// 200 redos give back the last state.
#[test]
fn the_history_keeps_the_last_200_steps_exactly() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let mut texts = vec![saved_text(s.project())];
    for i in 0..260 {
        s.command(&json!({"op": "segment", "thickness": 5 + i}));
        texts.push(saved_text(s.project()));
    }
    for _ in 0..200 {
        assert!(s.doc.can_undo());
        s.command(&json!({"op": "undo"}));
    }
    assert!(!s.doc.can_undo(), "only the last 200 steps are kept");
    assert_eq!(saved_text(s.project()), texts[60], "200 undos: the state after the 60th edit");
    for _ in 0..200 {
        s.command(&json!({"op": "redo"}));
    }
    assert!(!s.doc.can_redo());
    assert_eq!(saved_text(s.project()), texts[260], "200 redos: the last state");
}

/// A segment's own colour kept aside while "Use segment color" is off lives in
/// the project only: a saved .nodes figure still opens (and imports back) and
/// the figure keeps the colours it had on the stage.
#[test]
fn a_kept_own_colour_does_not_disturb_a_saved_nodes_figure() {
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "segment", "color": 0xff0000ffu32}));
    s.command(&json!({"op": "segment", "ownColor": false}));
    assert!(saved_text(s.project()).contains("B.ownColor"), "set-up: a colour is kept aside");
    let colours = |s: &Session| -> Vec<u32> {
        let f = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().clone();
        let mut r = barkfart_core::eval::Resolver::new(s.project());
        f.nodes().iter().map(|n| r.color(&n.color, 0.0).to_rgba8888()).collect()
    };
    let before = colours(&s);
    let (name, bytes) = s.figure_file().expect("the figure saves");
    let mut other = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    let st = other.import(&format!("{name}.nodes"), &bytes);
    assert!(st["message"].as_str().map_or(true, |m| !m.to_lowercase().contains("not a") && !m.to_lowercase().contains("fail")), "the saved figure imports: {}", st["message"]);
    let imported: Vec<String> = other.project().root().unwrap().layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect();
    assert!(imported.len() >= 2, "the figure came in as a new one: {imported:?}");
    let new_id = imported.last().unwrap().clone();
    let f = other.project().root().unwrap().layer(&new_id).unwrap().as_figure().unwrap().clone();
    let mut r = barkfart_core::eval::Resolver::new(other.project());
    let got: Vec<u32> = f.nodes().iter().map(|n| r.color(&n.color, 0.0).to_rgba8888()).collect();
    assert_eq!(got.len(), before.len(), "the same joints");
    assert_eq!(got, before, "the same colours");
}

/// A project opened from a .bark file (the app's Open and New go through
/// Session::new(Document::with_assets(..))) also drops edits that change nothing.
#[test]
fn a_reopened_project_also_drops_edits_that_change_nothing() {
    let s = two_poses();
    let bytes = s.save_copy().unwrap();
    let file = codec::read_bark(&bytes).unwrap();
    let assets = file.assets.into_iter().map(|(k, v)| (k, std::sync::Arc::new(v))).collect();
    let mut again = Session::new(Document::with_assets(file.project, assets));
    again.command(&json!({"op": "fps", "v": 12}));
    again.command(&json!({"op": "fps", "v": 12}));
    again.command(&json!({"op": "fps", "v": 12}));
    assert_eq!(again.doc.undo_depth(), 1, "one step for the three identical edits");
    again.command(&json!({"op": "undo"}));
    assert!(again.state()["undo"].is_null(), "and one undo leaves nothing to undo");
    assert!(!again.doc.dirty(), "back to the saved state");
}

/// "Unsaved changes" follows the saved state, not a counter: undoing back to it
/// (or redoing to it) is clean, an edit is dirty, and a new edit made from before
/// the saved state makes it unreachable (dirty until the next save).
#[test]
fn the_unsaved_marker_follows_the_saved_state() {
    let mut s = two_poses();
    s.doc.mark_saved();
    assert!(!s.doc.dirty());
    s.command(&json!({"op": "fps", "v": 12}));
    assert!(s.doc.dirty(), "an edit after saving");
    s.command(&json!({"op": "undo"}));
    assert!(!s.doc.dirty(), "undone back to the saved state");
    s.command(&json!({"op": "redo"}));
    assert!(s.doc.dirty(), "redone away from it");
    s.command(&json!({"op": "undo"}));
    assert!(!s.doc.dirty());
    // two edits, saved after them, then undo and a different edit: the saved state is gone
    s.command(&json!({"op": "fps", "v": 15}));
    s.command(&json!({"op": "fps", "v": 20}));
    s.doc.mark_saved();
    s.command(&json!({"op": "undo"}));
    assert!(s.doc.dirty(), "one step before the saved state");
    s.command(&json!({"op": "fps", "v": 25}));
    assert!(s.doc.dirty());
    s.command(&json!({"op": "undo"}));
    assert!(s.doc.dirty(), "a different history: the saved state cannot be reached any more");
    s.doc.mark_saved();
    assert!(!s.doc.dirty());
    // a restored recovery copy is unsaved until saved
    s.doc.mark_unsaved();
    assert!(s.doc.dirty());
    s.command(&json!({"op": "fps", "v": 30}));
    s.command(&json!({"op": "undo"}));
    assert!(s.doc.dirty(), "a recovery copy never becomes clean by undoing");
    // a cancelled drag leaves the saved state clean
    s.doc.mark_saved();
    let h = s.tools.handles(s.project(), s.time()).into_iter().find(|h| h.layer == "L2" && h.node == "armR2").unwrap();
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 20}));
    s.command(&json!({"op": "move", "x": h.x + 40.0, "y": h.y}));
    s.command(&json!({"op": "cancel"}));
    assert!(!s.doc.dirty(), "a cancelled drag changes nothing");
}

#[test]
fn reset_segment_goes_back_to_how_the_figure_came() {
    // D7b: a figure added to the project remembers each segment's look
    let mut s = session();
    let st = s.command(&json!({"op": "add_figure", "kind": "stickman"}));
    let fig = st["figures"].as_array().unwrap().last().unwrap()["id"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "select", "layer": fig, "node": "armR1"}));
    let keys = ["thickness", "segScale", "color", "ownColor", "curve", "shape", "length", "stretchy", "gradient", "outline"];
    let pick = |st: &Value| keys.iter().map(|k| (k.to_string(), st["selected"][*k].clone())).collect::<Vec<_>>();
    let came = pick(&st);
    let d = &st["selected"]["defaults"];
    assert!(d.is_object(), "an added figure reports its defaults");
    assert_eq!(d["thickness"], st["selected"]["thickness"]);
    assert_eq!(d["shape"], st["selected"]["shape"]);
    s.command(&json!({"op": "segment", "thickness": 77.0}));
    s.command(&json!({"op": "segment", "color": 0x11aa33ffu32}));
    s.command(&json!({"op": "segment", "segScale": 2.5, "curve": 40.0, "shape": "circle", "gradient": true, "outline": true}));
    s.command(&json!({"op": "node", "length": 3.0, "stretchy": true}));
    let edited = pick(&s.command(&json!({"op": "state"})));
    assert_ne!(edited, came);
    let undo_before = s.command(&json!({"op": "state"}))["undo"].clone();
    let st = s.command(&json!({"op": "reset_segment"}));
    assert_eq!(pick(&st), came, "every value is back");
    assert_eq!(st["selected"]["defaults"], *d, "the base itself does not change");
    // one step
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["undo"], undo_before);
    assert_eq!(pick(&st), edited, "undo brings the edits back");
    let st = s.command(&json!({"op": "redo"}));
    assert_eq!(pick(&st), came);

    // edited again, saved and opened: the base comes along
    s.command(&json!({"op": "segment", "thickness": 5.0}));
    let text = codec::encode(s.project());
    let mut t = Session::new(Document::new(codec::decode(&text).unwrap()));
    let st = t.command(&json!({"op": "select", "layer": fig, "node": "armR1"}));
    assert_eq!(st["selected"]["defaults"], *d);
    assert_eq!(codec::encode(t.project()), text, "the base round-trips exactly");
    let st = t.command(&json!({"op": "reset_segment"}));
    assert_eq!(pick(&st), came);
}

#[test]
fn figures_from_before_d7b_have_no_defaults_and_reset_changes_nothing() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "select", "layer": fig, "node": "armR1"}));
    assert!(st["selected"]["defaults"].is_null());
    let undo = st["undo"].clone();
    let st = s.command(&json!({"op": "reset_segment"}));
    assert_eq!(st["undo"], undo, "nothing to reset to: no step");
}

#[test]
fn an_imported_stick_nodes_figure_remembers_how_it_came() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/legacy/synthetic/fig-v300b0.nodes");
    let mut s = session();
    let st = s.import("fig-v300b0.nodes", &std::fs::read(path).unwrap());
    let fig = st["selected"]["layer"].as_str().unwrap().to_string();
    let p = s.project();
    let f = p.root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
    let node = f.nodes()[1].id.clone();
    let st = s.command(&json!({"op": "select", "layer": fig, "node": node}));
    assert!(st["selected"]["defaults"].is_object());
    assert_eq!(st["selected"]["defaults"]["thickness"], st["selected"]["thickness"]);
}

/// Triangles with their colours, in any order (breaking a branch off changes
/// which layer draws it, not what is drawn).
fn triangles(s: &Session, frame: i32) -> Vec<Vec<i64>> {
    let p = s.project();
    let t = frame as f64 / p.settings.fps as f64;
    let list = barkfart_core::tess::tessellate(&barkfart_core::eval::evaluate(p, &p.root_symbol, t), None);
    let mut out: Vec<Vec<i64>> = Vec::new();
    for b in &list.batches {
        for (tri, col) in b.xy.chunks(6).zip(b.rgba.chunks(3)) {
            let mut v: Vec<i64> = tri.iter().map(|x| (x * 100.0).round() as i64).collect();
            v.extend(col.iter().map(|c| *c as i64));
            out.push(v);
        }
    }
    out.sort();
    out
}

#[test]
fn breaking_a_branch_off_changes_nothing_on_screen() {
    let mut s = animated();
    let before: Vec<_> = (0..4).map(|f| triangles(&s, f)).collect();
    let text = codec::encode(s.project());
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.command(&json!({"op": "break_off"}));
    let figs = st["figures"].as_array().unwrap();
    assert_eq!(figs.len(), 2, "the branch is a figure of its own");
    let new_id = st["selected"]["layer"].as_str().unwrap().to_string();
    assert_ne!(new_id, "L2");
    assert_eq!(st["selected"]["node"], "armR1");
    assert_eq!(st["selected"]["joinedTo"], figs[0]["name"]);
    for f in 0..4 {
        assert_eq!(triangles(&s, f), before[f as usize], "frame {f} looks the same");
    }
    // the old figure no longer has the arm
    let p = s.project();
    assert!(p.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").is_none());
    // one step, exactly undone and redone (undo keeps the id counter on
    // purpose, so ids are never reused: compare everything else)
    let after = codec::encode(s.project());
    s.command(&json!({"op": "undo"}));
    let no_counter = |x: &str| x.lines().filter(|l| !l.contains("\"idCounter\"")).collect::<Vec<_>>().join("\n");
    assert_eq!(no_counter(&codec::encode(s.project())), no_counter(&text));
    s.command(&json!({"op": "redo"}));
    assert_eq!(codec::encode(s.project()), after);
    // saved and opened, it still looks the same
    let t = Session::new(Document::new(codec::decode(&after).unwrap()));
    for f in 0..4 {
        assert_eq!(triangles(&t, f), before[f as usize], "reopened frame {f}");
    }
}

#[test]
fn the_main_joint_cannot_break_off() {
    let mut s = session();
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    let root = s.project().root().unwrap().layer(&fig).unwrap().as_figure().unwrap().nodes()[0].id.clone();
    s.command(&json!({"op": "select", "layer": fig, "node": root}));
    let undo = s.command(&json!({"op": "state"}))["undo"].clone();
    let st = s.command(&json!({"op": "break_off"}));
    assert_eq!(st["undo"], undo);
    assert_eq!(st["figures"].as_array().unwrap().len(), 1);
}

/// Where a figure's main joint is on the stage at time t.
fn figure_at(s: &Session, id: &str, t: f64) -> (f64, f64) {
    let p = s.project();
    let sym = p.root().unwrap();
    let a = barkfart_core::eval::layer_stage(p, sym, sym.layer(id).unwrap(), t);
    (a.tx, a.ty)
}

#[test]
fn each_figure_holds_or_tweens_on_its_own() {
    // F2: Hold or Tween belongs to one figure's key, not to the frame
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    s.command(&json!({"op": "add_figure", "kind": "stickman"}));
    s.command(&json!({"op": "insert_frame"}));
    let st = s.command(&json!({"op": "state"}));
    assert_eq!(st["frame"], 1);
    let figs: Vec<String> = st["figures"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(figs.len(), 2);
    // both figures move on the second frame
    for (i, id) in figs.iter().enumerate() {
        let at = &s.state()["figures"][i]["at"];
        let (x, y) = (at[0].as_f64().unwrap() as f32, at[1].as_f64().unwrap() as f32);
        s.command(&json!({"op": "down", "x": x, "y": y, "r": 10}));
        s.command(&json!({"op": "move", "x": x + 100.0, "y": y}));
        s.command(&json!({"op": "up"}));
        assert!(figure_at(&s, id, 0.1).0 - figure_at(&s, id, 0.0).0 > 50.0, "{id} moved on frame 2");
    }
    s.command(&json!({"op": "animation", "tweening": true, "tweened": 1}));
    // the first figure holds its first pose until the next one
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": figs[0]}));
    let st = s.command(&json!({"op": "easing", "v": "hold"}));
    assert_eq!(st["undo"], "Easing");
    let mid = 0.05;
    let (a0, a1, am) = (figure_at(&s, &figs[0], 0.0), figure_at(&s, &figs[0], 0.1), figure_at(&s, &figs[0], mid));
    assert!((am.0 - a0.0).abs() < 1e-3, "the held figure stays put between frames: {am:?} vs {a0:?} / {a1:?}");
    let (b0, b1, bm) = (figure_at(&s, &figs[1], 0.0), figure_at(&s, &figs[1], 0.1), figure_at(&s, &figs[1], mid));
    assert!((bm.0 - (b0.0 + b1.0) / 2.0).abs() < 1.0, "the other one tweens: {bm:?} between {b0:?} and {b1:?}");
    // the frame tells the timeline which it is: not every figure holds
    assert_eq!(st["noTweenFrames"][0], false);
}

/// The frames (at 30 fps here, 10 in editor projects) a property is keyed on.
fn keyed_frames_of(q: &barkfart_core::model::Property, fps: f64) -> Vec<i64> {
    q.track.keys().iter().map(|k| (k.time * fps).round() as i64).collect()
}

#[test]
fn a_figure_keyed_on_some_frames_only_stays_that_way() {
    // F3: the whole-figure key rule. Frames are no longer snapshots: a figure
    // can hold or tween across frames where it has no key of its own.
    let s = animated(); // 4 frames, figure L2 posed differently on frame 4
    let fps = s.project().settings.fps as f64;
    let mut p = s.project().clone();
    let sym = p.root_symbol.clone();
    {
        let l = p.layer_mut(&sym, "L2").unwrap();
        let tr = &mut l.transform;
        let mut ps: Vec<&mut barkfart_core::model::Property> = vec![&mut tr.x, &mut tr.y, &mut tr.rotation, &mut tr.scale_x, &mut tr.scale_y];
        let barkfart_core::model::LayerKind::Figure(f) = &mut l.kind else { panic!() };
        for n in f.nodes_mut() {
            ps.push(&mut n.rotation);
            ps.push(&mut n.length);
        }
        for q in ps {
            for fr in [1, 2] {
                if let Ok(i) = q.track.index_of(fr as f64 / fps) {
                    q.track.remove_at(i);
                }
            }
        }
    }
    let mut s = Session::new(Document::new(p));
    let rot = |s: &Session| {
        let p = s.project();
        let f = p.root().unwrap().layer("L2").unwrap().as_figure().unwrap();
        keyed_frames_of(&f.nodes()[3].rotation, fps)
    };
    assert_eq!(rot(&s), vec![0, 3]);
    // between its keys the figure tweens: frame 2 is a third of the way
    let (a, b, m) = (figure_at(&s, "L2", 0.0), figure_at(&s, "L2", 3.0 / fps), figure_at(&s, "L2", 1.0 / fps));
    assert!((m.0 - (a.0 + (b.0 - a.0) / 3.0)).abs() < 1e-3, "{m:?} between {a:?} and {b:?}");
    // an edit elsewhere leaves it alone
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "camera", "zoom": 1.5}));
    assert_eq!(rot(&s), vec![0, 3], "the camera edit did not key the figure");
    // posing it on frame 2 keys all of it there, and nothing on frame 3
    let st = s.command(&json!({"op": "state"}));
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x + 30.0, "y": y}));
    s.command(&json!({"op": "up"}));
    let p = s.project();
    let l = p.root().unwrap().layer("L2").unwrap();
    let f = l.as_figure().unwrap();
    for n in f.nodes() {
        assert_eq!(keyed_frames_of(&n.rotation, fps), vec![0, 1, 3], "{}: keyed with the whole figure", n.id);
        assert_eq!(keyed_frames_of(&n.length, fps), vec![0, 1, 3], "{}", n.id);
    }
    assert_eq!(keyed_frames_of(&l.transform.x, fps), vec![0, 1, 3]);
}

/// `animated()` with figure L2 keyed on frames 1 and 4 only (indexes 0, 3).
fn sparse() -> Session {
    let s = animated();
    let fps = s.project().settings.fps as f64;
    let mut p = s.project().clone();
    let sym = p.root_symbol.clone();
    let l = p.layer_mut(&sym, "L2").unwrap();
    for q in barkfart_core::edit::block_ops::pose_props(l) {
        for fr in [1, 2] {
            if let Ok(i) = q.track.index_of(fr as f64 / fps) {
                q.track.remove_at(i);
            }
        }
    }
    Session::new(Document::new(p))
}

fn l2_frames(s: &Session) -> Vec<i32> {
    let p = s.project();
    barkfart_core::edit::block_ops::pose_frames(p.root().unwrap().layer("L2").unwrap(), p.settings.fps as f64)
}

#[test]
fn blocks_lengthen_shorten_move_and_start() {
    // F4: one figure's row, one step each
    let mut s = sparse();
    assert_eq!(l2_frames(&s), vec![0, 3]);
    assert_eq!(s.state()["frames"], 4);
    let st = s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(st["selected"]["pose"]["start"], 0);
    assert_eq!(st["selected"]["pose"]["length"], 3);
    assert_eq!(st["selected"]["pose"]["here"], true);

    // Hold longer: the next pose starts a frame later; the animation grows
    let st = s.command(&json!({"op": "hold_longer"}));
    assert_eq!(st["undo"], "Hold longer");
    assert_eq!(l2_frames(&s), vec![0, 4]);
    assert_eq!(st["frames"], 5);
    assert_eq!(st["selected"]["pose"]["length"], 4);
    s.command(&json!({"op": "undo"}));
    assert_eq!(l2_frames(&s), vec![0, 3]);
    assert_eq!(s.state()["frames"], 4);

    // a set length, shorter
    let st = s.command(&json!({"op": "block_length", "frames": 2}));
    assert_eq!(st["undo"], "Pose length");
    assert_eq!(l2_frames(&s), vec![0, 2]);
    s.command(&json!({"op": "undo"}));

    // New pose on frame 2 (index 1): the picture does not change, the row gets a block
    let before: Vec<_> = (0..4).map(|f| triangles(&s, f)).collect();
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "new_pose"}));
    assert_eq!(st["undo"], "New pose");
    assert_eq!(l2_frames(&s), vec![0, 1, 3]);
    for f in 0..4 {
        assert_eq!(triangles(&s, f), before[f as usize], "frame {f}");
    }
    let st = s.command(&json!({"op": "new_pose"}));
    assert!(!st["message"].as_str().unwrap_or("").is_empty(), "not twice on one frame: it says why");
    assert_eq!(l2_frames(&s), vec![0, 1, 3]);

    // move that pose to frame 3 (index 2): it can't pass the poses beside it
    let st = s.command(&json!({"op": "move_block", "to": 2}));
    assert_eq!(st["undo"], "Move pose");
    assert_eq!(st["frame"], 2, "the playhead follows the pose");
    assert_eq!(l2_frames(&s), vec![0, 2, 3]);
    let st = s.command(&json!({"op": "move_block", "to": 3}));
    assert_eq!(st["undo"], "Move pose", "refused: no new step");
    assert_eq!(l2_frames(&s), vec![0, 2, 3]);
    // the first pose stays on the first frame
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "move_block", "to": 1}));
    assert_eq!(l2_frames(&s), vec![0, 2, 3]);
}

#[test]
fn add_frame_keys_every_figure_on_the_new_frame() {
    // F4's bridge: Add frame copies the whole moment, sparse rows included,
    // and pushes later keys right
    let mut s = sparse();
    let mid = triangles(&s, 1);
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(l2_frames(&s), vec![0, 2, 4], "keyed on the new frame; the frame-4 pose moved to 5");
    assert_eq!(triangles(&s, 2), mid, "the new frame shows the moment it copies");
}

#[test]
fn the_timeline_rows_show_each_figures_poses() {
    // G1: one row per layer, front first; blocks are [start, length, hold]
    let mut s = sparse();
    let st = s.command(&json!({"op": "state"}));
    let rows = st["rows"].as_array().unwrap();
    let l2 = rows.iter().find(|r| r["id"] == "L2").expect("a row for L2");
    assert_eq!(l2["kind"], "figure");
    assert_eq!(l2["blocks"], json!([[0, 3, false], [3, 1, false]]));
    assert_eq!(l2["on"].as_array().unwrap().len(), 4);
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "easing", "v": "hold"}));
    s.command(&json!({"op": "add_figure", "kind": "stickman"}));
    let st = s.command(&json!({"op": "state"}));
    let rows = st["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0]["id"], "L2", "the new figure is in front, so first");
    assert_eq!(rows[1]["blocks"][0], json!([0, 3, true]));
    // the new figure is on this frame only
    assert_eq!(rows[0]["on"], json!([true, false, false, false]));
}

#[test]
fn several_figures_change_together_in_one_step() {
    // G5: select_many and each
    let mut s = sparse();
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.command(&json!({"op": "duplicate_figure"}));
    let twin = st["selected"]["layer"].as_str().unwrap().to_string();
    assert_ne!(twin, "L2");
    let twin_frames = |s: &Session| {
        let p = s.project();
        barkfart_core::edit::block_ops::pose_frames(p.root().unwrap().layer(&twin).unwrap(), p.settings.fps as f64)
    };
    assert_eq!(twin_frames(&s), vec![0, 3], "the copy has the same poses");
    let ids = vec!["L2".to_string(), twin.clone()];
    let st = s.command(&json!({"op": "select_many", "layers": ids}));
    assert_eq!(st["multi"].as_array().unwrap().len(), 2);
    assert_eq!(st["selected"]["layer"], "L2");
    let undo = st["undo"].clone();
    // colour for both, one step
    let st = s.command(&json!({"op": "each", "label": "Colour figures", "cmd": {"op": "figure", "color": 0x00ff00ffu32}}));
    assert_eq!(st["undo"], "Colour figures");
    assert!(st["multi"].as_array().unwrap().iter().all(|m| m["color"] == 0x00ff00ffu32), "{}", st["multi"]);
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["undo"], undo);
    // New pose skips a figure that has one on this frame already
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "new_pose"}));
    s.command(&json!({"op": "select_many", "layers": ["L2", twin]}));
    let st = s.command(&json!({"op": "each", "label": "New pose", "cmd": {"op": "new_pose"}}));
    assert_eq!(st["undo"], "New pose");
    assert_eq!(l2_frames(&s), vec![0, 1, 3]);
    assert_eq!(twin_frames(&s), vec![0, 1, 3]);
    assert_eq!(st["multi"].as_array().unwrap().len(), 2, "still both");
    // picking one figure ends the group
    let st = s.command(&json!({"op": "select", "layer": "L2"}));
    assert!(st["multi"].is_null());
}

#[test]
fn wiggle_and_swing_move_a_figure_on_top_of_its_poses() {
    // H2 Movement: one step each; the eye hides without losing the settings
    let mut s = two_poses();
    let st = s.command(&json!({"op": "select", "layer": "L2"}));
    let fig = st["selected"]["layer"].as_str().unwrap().to_string();
    let still = (figure_at(&s, &fig, 0.03), figure_at(&s, &fig, 0.07));
    let st = s.command(&json!({"op": "move_fx", "kind": "wiggle", "on": true}));
    assert_eq!(st["undo"], "Add wiggle");
    assert_eq!(st["selected"]["filters"]["wiggle"]["amount"], 8.0);
    let moved = (figure_at(&s, &fig, 0.03), figure_at(&s, &fig, 0.07));
    assert_ne!(moved, still, "the figure wiggles");
    s.command(&json!({"op": "move_fx", "kind": "wiggle", "shown": false}));
    assert_eq!((figure_at(&s, &fig, 0.03), figure_at(&s, &fig, 0.07)), still, "hidden: as before");
    let st = s.command(&json!({"op": "move_fx", "kind": "wiggle", "on": false}));
    assert!(st["selected"]["filters"]["wiggle"].is_null());
    // the filter eye
    s.command(&json!({"op": "filter", "blur": 5}));
    let st = s.command(&json!({"op": "filter", "shown": {"blur": false}}));
    assert_eq!(st["selected"]["filters"]["hidden"], json!(["blur"]));
    assert_eq!(st["selected"]["filters"]["on"]["blur"], true, "still set up, only hidden");
}

#[test]
fn wiggle_moves_the_figure_not_its_joints() {
    let mut s = Session::new(Document::new(barkfart_core::session::new_editor_project()));
    let st = s.command(&json!({"op": "state"}));
    let fig = st["figures"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": fig}));
    let angles = |s: &Session| {
        let p = s.project();
        let f = p.root().unwrap().layer(&fig).unwrap().as_figure().unwrap();
        barkfart_core::eval::FigurePose::compute(f, 0.0, &mut barkfart_core::eval::Resolver::new(p)).angle
    };
    let before = angles(&s);
    s.command(&json!({"op": "move_fx", "kind": "wiggle", "on": true}));
    assert_eq!(angles(&s), before, "the joints keep their angles");
}

#[test]
fn with_keying_off_an_effect_is_the_same_on_every_frame() {
    // D5: effects are static unless the keying toggle is on
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "filter", "blur": 7}));
    let blur = |s: &mut Session, f: i64| {
        let st = s.command(&json!({"op": "frame", "i": f}));
        st["selected"]["filters"]["blur"].as_f64().unwrap()
    };
    assert_eq!(blur(&mut s, 0), 7.0, "frame 1 too");
    assert_eq!(blur(&mut s, 1), 7.0);
    // keying on: a key on this frame, lasting until the next key; never
    // reaching back
    s.command(&json!({"op": "insert_frame"}));
    assert_eq!(blur(&mut s, 2), 7.0);
    s.command(&json!({"op": "set", "k": "keying", "v": true}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "filter", "blur": 2}));
    assert_eq!(blur(&mut s, 1), 2.0);
    assert_eq!(blur(&mut s, 0), 7.0, "the frame before keeps its value");
    assert_eq!(blur(&mut s, 2), 2.0, "the frame after holds it: no key of its own");
    // keying off again: an animated value still keys (never flattened)
    s.command(&json!({"op": "set", "k": "keying", "v": false}));
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "filter", "blur": 5}));
    assert_eq!(blur(&mut s, 2), 5.0);
    assert_eq!(blur(&mut s, 0), 7.0, "the animation stays");
    assert_eq!(blur(&mut s, 1), 2.0);
}

#[test]
fn an_effect_switched_on_later_appears_on_its_frame() {
    // D5: the first key holds the old value from the start, so a glow keyed
    // on frame 3 appears there instead of fading in from frame 1
    let mut s = two_poses();
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "set", "k": "keying", "v": true}));
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "filter", "glowAmount": 0.8}));
    let glow = |s: &mut Session, f: i64| s.command(&json!({"op": "frame", "i": f}))["selected"]["filters"]["glowAmount"].as_f64().unwrap();
    assert_eq!(glow(&mut s, 2), 0.0);
    assert_eq!(glow(&mut s, 1), 0.0);
    assert_eq!(glow(&mut s, 3), 0.8);
}

#[test]
fn an_animated_effect_gets_a_row_and_can_stop_animating() {
    // H3: property rows with diamonds where the value changes; × makes it static
    let mut s = two_poses();
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "set", "k": "keying", "v": true}));
    s.command(&json!({"op": "filter", "glowAmount": 0.4}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "filter", "glowAmount": 0.9}));
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap().clone();
    let glow = row["props"].as_array().unwrap().iter().find(|p| p["name"] == "Glow amount").expect("a Glow amount row").clone();
    assert_eq!(glow["keys"], json!([0, 1]));
    let st = s.command(&json!({"op": "stop_animating", "property": glow["id"]}));
    assert_eq!(st["undo"], "Stop animating");
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == "L2").unwrap().clone();
    assert!(row["props"].as_array().unwrap().iter().all(|p| p["name"] != "Glow amount"));
    let st = s.command(&json!({"op": "frame", "i": 0}));
    assert_eq!(st["selected"]["filters"]["glowAmount"], 0.9, "the value it had there, everywhere");
}

#[test]
fn delete_a_pose_add_time_and_bounce() {
    // the Pose and Frame pages (round 10 boards)
    let mut s = sparse(); // L2 keyed on frames 1 and 4
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "frame", "i": 3}));
    let st = s.command(&json!({"op": "delete_pose"}));
    assert_eq!(st["undo"], "Delete pose");
    assert_eq!(l2_frames(&s), vec![0]);
    let st = s.command(&json!({"op": "delete_pose"}));
    assert!(!st["message"].as_str().unwrap_or("").is_empty(), "the first pose stays");
    s.command(&json!({"op": "undo"}));
    assert_eq!(l2_frames(&s), vec![0, 3]);
    // Add time here: one more frame, no new keys, later keys move back
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "add_time"}));
    assert_eq!(st["frames"], 5);
    assert_eq!(l2_frames(&s), vec![0, 4]);
    // the bounce ease, reported back
    s.command(&json!({"op": "frame", "i": 0}));
    let st = s.command(&json!({"op": "easing", "v": "bounce", "block": true}));
    assert_eq!(st["selected"]["pose"]["ease"], "bounce");
    let st = s.command(&json!({"op": "blend", "v": "SCREEN"}));
    assert_eq!(st["selected"]["blend"], "SCREEN");
}

#[test]
fn keying_an_effect_setting_later_animates_it() {
    let mut s = two_poses();
    let fig = s.state()["figures"][0]["id"].as_str().unwrap().to_string();
    s.command(&json!({"op": "select", "layer": fig}));
    s.command(&json!({"op": "set", "k": "keying", "v": true}));
    s.command(&json!({"op": "filter", "glowAmount": 0.6}));
    s.command(&json!({"op": "frame", "i": 1}));
    let st = s.command(&json!({"op": "filter", "glowIntensity": 2}));
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == fig.as_str()).unwrap().clone();
    let names: Vec<String> = row["props"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"Glow strength".to_string()), "{names:?}");
}

#[test]
fn a_picture_is_selected_and_changed_on_its_page() {
    // the Picture board: scale, flips, opacity, lock, one step each
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/legacy/synthetic/picture.png");
    let mut s = two_poses();
    let st = s.import("picture.png", &std::fs::read(path).unwrap());
    let pic = st["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "picture").expect("a picture row")["id"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "select", "layer": pic, "node": ""}));
    assert_eq!(st["picture"]["layer"], pic.as_str());
    assert_eq!(st["picture"]["scale"], 1.0);
    let st = s.command(&json!({"op": "picture", "scale": 1.5}));
    assert_eq!(st["undo"], "Picture");
    assert_eq!(st["picture"]["scale"], 1.5);
    let st = s.command(&json!({"op": "picture", "flipH": true}));
    assert_eq!(st["picture"]["flipH"], true);
    let st = s.command(&json!({"op": "picture", "opacity": 0.5, "locked": true}));
    assert_eq!(st["picture"]["opacity"], 0.5);
    assert_eq!(st["picture"]["locked"], true);
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["picture"]["opacity"], 1.0);
}

#[test]
fn joins_are_moments_and_nothing_jumps() {
    // D8 / I1: join from a frame on, let go later; the picture doesn't move
    // at the switch, and from then on the joined figure follows
    let mut s = animated(); // 4 frames, L2 posed differently on the last
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.command(&json!({"op": "duplicate_figure"}));
    let twin = st["selected"]["layer"].as_str().unwrap().to_string();
    let before: Vec<_> = (0..4).map(|f| triangles(&s, f)).collect();
    let fps = s.project().settings.fps as f64;
    let at = |s: &Session, f: i32| figure_at(s, &twin, f as f64 / fps);
    let twin_before: Vec<_> = (0..4).map(|f| at(&s, f)).collect();
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": twin}));
    let st = s.command(&json!({"op": "join_here", "layer": "L2"}));
    assert_eq!(st["undo"], "Join");
    for f in 0..4 {
        assert_eq!(triangles(&s, f), before[f as usize], "frame {f} unchanged by the join");
    }
    // moving L2 on frame 4 moves the twin there (joined), not on frame 1
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "nudge", "dx": 40, "dy": 0}));
    let (a0, a3) = (at(&s, 0), at(&s, 3));
    assert!((a0.0 - twin_before[0].0).abs() < 1e-3, "before the join the twin stays: {a0:?}");
    assert!((a3.0 - twin_before[3].0 - 40.0).abs() < 1.0, "joined, it follows: {a3:?} vs {:?}", twin_before[3]);
    // let go on frame 3: moving L2 there no longer moves it
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": twin}));
    let pic = (0..4).map(|f| triangles(&s, f)).collect::<Vec<_>>();
    let st = s.command(&json!({"op": "let_go_here"}));
    assert_eq!(st["undo"], "Let go");
    for f in 0..4 {
        assert_eq!(triangles(&s, f), pic[f as usize], "frame {f} unchanged by letting go");
    }
    // tweened: no jump at either switch (just made: a parent moved later on
    // the switch's frame moves only the joined side)
    for sw in [1.0, 2.0] {
        let (a, b) = (figure_at(&s, &twin, sw / fps - 1e-6), figure_at(&s, &twin, sw / fps + 1e-6));
        assert!((a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05, "switch at frame {}: {a:?} then {b:?}", sw + 1.0);
    }
    let t2 = at(&s, 2);
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "nudge", "dx": 0, "dy": 30}));
    assert!((at(&s, 2).1 - t2.1).abs() < 1e-3, "let go: it stays");
    // saved and opened, the same
    let text = codec::encode(s.project());
    let t = Session::new(Document::new(codec::decode(&text).unwrap()));
    assert_eq!(codec::encode(t.project()), text);
    for f in 0..4 {
        assert_eq!(triangles(&t, f), triangles(&s, f), "reopened frame {f}");
    }
}
