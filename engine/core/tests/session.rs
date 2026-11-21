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
#[ignore = "smart-stretch impulse (reset / multiplier) not modelled yet: waiting for ground truth from the original (a .nodemc posed at several stretches)"]
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
    // copy and paste onto frame 5 only
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": ball}));
    s.command(&json!({"op": "copy_figure"}));
    s.command(&json!({"op": "frame", "i": 4}));
    let st = s.command(&json!({"op": "paste_figure"}));
    let copy = st["selected"]["layer"].as_str().unwrap().to_string();
    let fps = s.project().settings.fps as f64;
    let shown: Vec<bool> = (0..5).map(|f| s.tools.handles(s.project(), f as f64 / fps).iter().any(|h| h.layer == copy)).collect();
    assert_eq!(shown, vec![false, false, false, false, true]);
    // deleting it on its only frame removes it
    s.command(&json!({"op": "select", "layer": copy}));
    let st = s.command(&json!({"op": "delete_here"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
}
