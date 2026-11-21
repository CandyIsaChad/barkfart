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
