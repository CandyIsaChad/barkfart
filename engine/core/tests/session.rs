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

/// Four frames: hip moves right and the right arm swings, keyed on 0 and 3 (linear in between).
fn animated() -> Session {
    let mut s = session();
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "inbetween", "n": 2}));
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
fn in_betweens_tween_and_keys_show_on_the_strip() {
    let s = animated();
    let st = s.state();
    assert_eq!(st["frames"], 4);
    assert_eq!(st["keyed"], json!([true, false, false, true]));
    // the in-betweens are between the two poses
    let (a, m, b) = (picture(&s, 0), picture(&s, 1), picture(&s, 3));
    assert!(!same(&a, &m) && !same(&m, &b));
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
        let figs = st["figures"].as_array().unwrap().len();
        assert_eq!(figs, if k == 0 || k == 3 { 1 } else { 2 }, "frame {k}: a copy only when there is something on both sides");
        s.command(&json!({"op": "undo"}));
        let mut back = codec::decode(&codec::encode(s.project())).unwrap();
        back.id_counter = codec::decode(&text).unwrap().id_counter;
        assert_eq!(codec::encode(&back), text, "frame {k}: undo");
    }
}

#[test]
fn a_cut_makes_two_figures_that_look_the_same() {
    let mut s = animated();
    let before: Vec<Vec<f32>> = (0..4).map(|f| picture(&s, f)).collect();
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.command(&json!({"op": "cut"}));
    assert_eq!(st["figures"].as_array().unwrap().len(), 2);
    assert_ne!(st["selected"]["layer"], "L2", "the part after the cut is selected");
    for f in 0..4 {
        assert!(same(&picture(&s, f), &before[f as usize]), "frame {f} changed");
    }
    // the two parts are independent: posing the second does not touch the first
    let st = s.command(&json!({"op": "frame", "i": 3}));
    let (x, y) = handle(&st, 6);
    s.command(&json!({"op": "down", "x": x, "y": y, "r": 20}));
    s.command(&json!({"op": "move", "x": x - 50.0, "y": y}));
    s.command(&json!({"op": "up"}));
    assert!(same(&picture(&s, 0), &before[0]) && same(&picture(&s, 1), &before[1]));
}

#[test]
fn easing_a_frame() {
    let mut s = animated();
    let lin = picture(&s, 1);
    s.command(&json!({"op": "frame", "i": 0}));
    let st = s.command(&json!({"op": "easing", "v": "smooth"}));
    assert_eq!(st["undo"], "Easing");
    assert!(!same(&picture(&s, 1), &lin), "the in-between moved");
    assert!(same(&picture(&s, 0), &picture(&{ let mut t = animated(); t.command(&json!({"op": "frame", "i": 0})); t }, 0)));
}

#[test]
fn a_cut_inside_an_eased_tween() {
    let mut s = animated();
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "inbetween", "n": 4}));
    s.command(&json!({"op": "easing", "v": "smooth"}));
    let n = s.state()["frames"].as_i64().unwrap() as i32;
    let before: Vec<Vec<f32>> = (0..n).map(|f| picture(&s, f)).collect();
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "cut"}));
    let mut worst = 0f32;
    for f in 0..n {
        let now = picture(&s, f);
        assert_eq!(now.len(), before[f as usize].len());
        for (a, b) in now.iter().zip(&before[f as usize]) {
            worst = worst.max((a - b).abs());
        }
    }
    eprintln!("eased cut: largest vertex change {worst}");
    assert!(worst < 1e-3, "frames after a cut in an eased tween moved by {worst}");
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
