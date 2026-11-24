//! D34: Smooth tweens (Blender-like automatic easing) and handle types.

use barkfart_core::codec;
use barkfart_core::edit::Document;
use barkfart_core::model::*;
use barkfart_core::session::{new_editor_project, new_project, Session};
use serde_json::json;

fn track(keys: &[(f64, f32)], handle: HandleType) -> Track {
    let mut t = Track::new(ValueType::Float);
    for &(time, v) in keys {
        t.put(Key { handle, ..Key::new(time, Value::F(v), Interp::Smooth) });
    }
    t
}

fn at(t: &Track, time: f64) -> f64 {
    t.sample(time).f() as f64
}

/// value per second just left and just right of `time`
fn speeds(t: &Track, time: f64) -> (f64, f64) {
    let e = 1e-4;
    ((at(t, time) - at(t, time - e)) / e, (at(t, time + e) - at(t, time)) / e)
}

#[test]
fn motion_flows_through_a_middle_key() {
    let t = track(&[(0.0, 0.0), (1.0, 10.0), (3.0, 40.0)], HandleType::AutoClamped);
    let (l, r) = speeds(&t, 1.0);
    assert!((l - r).abs() < 0.05 * l.abs().max(1.0), "no jump in speed at the key: {l} vs {r}");
    assert!(l > 5.0, "still moving through it: {l}");
    // it passes through every key
    for (time, v) in [(0.0, 0.0), (1.0, 10.0), (3.0, 40.0)] {
        assert!((at(&t, time) - v).abs() < 1e-4);
    }
}

#[test]
fn it_settles_at_the_ends_and_on_turning_points() {
    let t = track(&[(0.0, 0.0), (1.0, 10.0), (2.0, 0.0)], HandleType::AutoClamped);
    let (_, start) = speeds(&t, 0.0);
    let (end, _) = speeds(&t, 2.0);
    let (l, r) = speeds(&t, 1.0);
    assert!(start.abs() < 0.05 && end.abs() < 0.05, "flat at the first and last key: {start}, {end}");
    assert!(l.abs() < 0.05 && r.abs() < 0.05, "flat on the turning point: {l}, {r}");
    // an ease in and out between two poses, not a straight line
    let lin = track(&[(0.0, 0.0), (1.0, 10.0)], HandleType::AutoClamped);
    assert!(at(&lin, 0.25) < 2.5 && at(&lin, 0.75) > 7.5 && (at(&lin, 0.5) - 5.0).abs() < 1e-4);
}

#[test]
fn clamped_handles_never_overshoot_and_plain_auto_may() {
    let keys = [(0.0, 0.0), (1.0, 10.0), (2.0, 10.0), (3.0, 0.0), (4.0, 2.0)];
    let clamped = track(&keys, HandleType::AutoClamped);
    let auto = track(&keys, HandleType::Auto);
    let mut most = (f64::MIN, f64::MIN);
    let mut least = f64::MAX;
    for i in 0..=400 {
        let time = i as f64 / 100.0;
        most.0 = most.0.max(at(&clamped, time));
        most.1 = most.1.max(at(&auto, time));
        least = least.min(at(&clamped, time));
    }
    assert!(most.0 <= 10.0 + 1e-6 && least >= -1e-6, "clamped stays between the keys: {} .. {least}", most.0);
    assert!(most.1 > 10.05, "plain auto swings past the held pose: {}", most.1);
}

#[test]
fn vector_handles_draw_straight_lines() {
    let t = track(&[(0.0, 0.0), (1.0, 10.0), (3.0, 20.0)], HandleType::Vector);
    for i in 0..=10 {
        let time = i as f64 / 10.0;
        assert!((at(&t, time) - 10.0 * time).abs() < 1e-3, "straight from the first key to the second at {time}");
    }
    assert!((at(&t, 2.0) - 15.0).abs() < 1e-3);
}

#[test]
fn colours_get_a_curve_per_channel() {
    let mut t = Track::new(ValueType::Color);
    t.put(Key::new(0.0, Value::C(Color::new(0.0, 1.0, 0.0, 1.0)), Interp::Smooth));
    t.put(Key::new(1.0, Value::C(Color::new(1.0, 0.0, 0.0, 1.0)), Interp::Smooth));
    let Value::C(c) = t.sample(0.25) else { panic!("a colour") };
    assert!(c.r > 0.0 && c.r < 0.25 && c.g > 0.75 && c.g < 1.0 && (c.a - 1.0).abs() < 1e-5, "{c:?}");
}

#[test]
fn smooth_keys_and_handles_survive_a_save() {
    let mut s = Session::new(Document::new(new_editor_project()));
    let p = s.doc.project_mut();
    let sym = p.root_symbol.clone();
    let l = p.layer_mut(&sym, "L2").unwrap();
    let LayerKind::Figure(f) = &mut l.kind else { panic!() };
    let n = f.node_mut("armR1").unwrap();
    n.rotation.track.clear();
    n.rotation.track.put(Key { handle: HandleType::Free, lh: (-0.2, -3.0), rh: (0.1, 4.0), ..Key::new(0.0, Value::F(10.0), Interp::Smooth) });
    n.rotation.track.put(Key { handle: HandleType::Vector, ..Key::new(0.5, Value::F(40.0), Interp::Smooth) });
    let text = codec::encode(s.project());
    assert!(text.contains("\"tweenInterp\": \"smooth\""));
    let back = codec::decode(&text).unwrap();
    assert_eq!(codec::encode(&back), text);
    let r = back.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation.track.keys().to_vec();
    assert_eq!(r[0].handle, HandleType::Free);
    assert_eq!((r[0].lh, r[0].rh), ((-0.2, -3.0), (0.1, 4.0)));
    assert_eq!(r[1].handle, HandleType::Vector);
    assert_eq!(r[1].interp, Interp::Smooth);
}

#[test]
fn new_animations_tween_smoothly_and_stick_nodes_ones_stay_linear() {
    assert_eq!(new_project().settings.tween_interp, Interp::Linear, "the reference starter is unchanged");
    let text = codec::encode(&new_project());
    assert!(!text.contains("tweenInterp") && !text.contains("\"S\""));
    let mut s = Session::new(Document::new(new_editor_project()));
    assert_eq!(s.project().settings.tween_interp, Interp::Smooth);
    // two poses four frames apart: the arm eases out of the first and into the second
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    for _ in 0..3 {
        s.command(&json!({"op": "hold_longer"}));
    }
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let rot = |s: &Session, f: i32| -> f64 {
        let p = s.project();
        let q = &p.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation;
        q.key_value(f as f64 / p.settings.fps as f64).f() as f64
    };
    // drag the elbow round the shoulder
    let t4 = 4.0 / s.project().settings.fps as f64;
    let h = s.tools.handles(s.project(), t4).into_iter().find(|h| h.layer == "L2" && h.node == "armR1").expect("the elbow");
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
    s.command(&json!({"op": "move", "x": h.x + 60.0, "y": h.y + 90.0}));
    s.command(&json!({"op": "up"}));
    let (a, b) = (rot(&s, 0), rot(&s, 4));
    assert!((b - a).abs() > 40.0, "the arm moved: {a} -> {b}");
    let quarter = (rot(&s, 1) - a) / (b - a);
    assert!(quarter < 0.25 - 0.03, "eases out of the first pose: {quarter}");
    let st = s.command(&json!({"op": "frame", "i": 0}));
    let _ = st;
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    let st = s.state();
    assert_eq!(st["selected"]["pose"]["ease"], "auto");
    assert_eq!(st["selected"]["pose"]["handles"], "auto_clamped");
    // Even makes it straight again; Auto brings the curve back; both one step
    let st = s.command(&json!({"op": "easing", "v": "linear", "block": true}));
    assert_eq!(st["undo"], "Easing");
    assert!(((rot(&s, 1) - a) / (b - a) - 0.25).abs() < 1e-3, "even");
    s.command(&json!({"op": "easing", "v": "auto", "block": true}));
    assert!((rot(&s, 1) - a) / (b - a) < 0.22);
    // handles: straight towards the neighbours makes the start sharp
    let st = s.command(&json!({"op": "handles", "v": "vector", "block": true}));
    assert_eq!(st["undo"], "Handles");
    assert_eq!(st["selected"]["pose"]["handles"], "vector");
    // freezing keeps the shape exactly
    s.command(&json!({"op": "handles", "v": "auto_clamped", "block": true}));
    let shape: Vec<f64> = (0..=4).map(|f| rot(&s, f)).collect();
    s.command(&json!({"op": "handles", "v": "free", "block": true}));
    let frozen: Vec<f64> = (0..=4).map(|f| rot(&s, f)).collect();
    for (x, y) in shape.iter().zip(&frozen) {
        assert!((x - y).abs() < 1e-3, "free keeps the curve: {shape:?} vs {frozen:?}");
    }
}
