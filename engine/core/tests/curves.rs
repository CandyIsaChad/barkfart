//! D34: the graph (Curves): channels, dragging keys and handles, adding values.

use barkfart_core::edit::block_ops::pose_frames;
use barkfart_core::edit::Document;
use barkfart_core::model::*;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::{json, Value as J};

/// Two poses four frames apart, the arm turned between them, Curves open.
fn posed() -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    for _ in 0..3 {
        s.command(&json!({"op": "hold_longer"}));
    }
    s.command(&json!({"op": "frame", "i": 4}));
    let t4 = 4.0 / s.project().settings.fps as f64;
    let h = s.tools.handles(s.project(), t4).into_iter().find(|h| h.layer == "L2" && h.node == "armR1").unwrap();
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
    s.command(&json!({"op": "move", "x": h.x + 60.0, "y": h.y + 90.0}));
    s.command(&json!({"op": "up"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "set", "k": "curves", "v": true}));
    s
}

fn channel<'a>(st: &'a J, id: &str) -> &'a J {
    st["curves"]["channels"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("no channel {id}: {}", st["curves"]["channels"]))
}

fn arm_id(s: &Session) -> String {
    s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation.id.clone()
}

fn arm(s: &Session) -> Property {
    s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation.clone()
}

#[test]
fn curves_list_the_values_that_change_only_while_open() {
    let mut s = posed();
    let id = arm_id(&s);
    let st = s.state();
    let c = channel(&st, &id);
    assert_eq!(c["focus"], true, "the selected joint's own");
    assert_eq!(c["unit"], "deg");
    assert_eq!(c["keys"].as_array().unwrap().len(), 2);
    assert_eq!(c["segs"][0][0], "c", "a Smooth stretch is a cubic: {}", c["segs"]);
    // the main joint hasn't moved: offered, not listed
    let x = s.project().root().unwrap().layer("L2").unwrap().transform.x.id.clone();
    assert!(st["curves"]["channels"].as_array().unwrap().iter().all(|c| c["id"] != x.as_str()));
    assert!(st["curves"]["addable"].as_array().unwrap().iter().any(|c| c["id"] == x.as_str()));
    let st = s.command(&json!({"op": "set", "k": "curves", "v": false}));
    assert!(st["curves"].is_null());
}

#[test]
fn dragging_a_key_changes_its_value_in_one_step_and_keeps_the_frames() {
    let mut s = posed();
    let id = arm_id(&s);
    let fps = s.project().settings.fps as f64;
    let frames = pose_frames(s.project().root().unwrap().layer("L2").unwrap(), fps);
    let before = arm(&s);
    let undo = s.state()["undo"].clone();
    s.command(&json!({"op": "curve_down", "property": id, "frame": 4, "part": "key"}));
    for dv in [5.0, 12.0, 20.0] {
        s.command(&json!({"op": "curve_move", "dv": dv}));
    }
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], "Move key");
    let after = arm(&s);
    assert!((after.track.keys()[1].value.f() - before.track.keys()[1].value.f() - 20.0).abs() < 1e-3, "from the start plus the whole move");
    assert_eq!(after.track.keys()[1].interp, before.track.keys()[1].interp);
    assert_eq!(pose_frames(s.project().root().unwrap().layer("L2").unwrap(), fps), frames, "no new frames, every pose value on the same frames");
    let st = s.command(&json!({"op": "undo"}));
    assert_eq!(st["undo"], undo);
    assert_eq!(arm(&s), before);
}

#[test]
fn dragging_an_automatic_handle_makes_it_aligned_and_free_moves_one_side() {
    let mut s = posed();
    // a middle key to shape: a pose at frame 2
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "new_pose"}));
    s.command(&json!({"op": "frame", "i": 0}));
    let id = arm_id(&s);
    s.command(&json!({"op": "curve_down", "property": id, "frame": 2, "part": "right"}));
    s.command(&json!({"op": "curve_move", "dt": 0.2, "dv": 15.0}));
    let st = s.command(&json!({"op": "up"}));
    assert_eq!(st["undo"], "Shape curve");
    let k = arm(&s).track.keys()[1];
    assert_eq!(k.handle, HandleType::Aligned);
    assert!((k.lh.1 / k.lh.0 - k.rh.1 / k.rh.0).abs() < 1e-6, "both sides in line: {:?} {:?}", k.lh, k.rh);
    // Free: the other side stays where it was
    s.command(&json!({"op": "handles", "v": "free", "frame": 2, "property": id}));
    let left = arm(&s).track.keys()[1].lh;
    s.command(&json!({"op": "curve_down", "property": id, "frame": 2, "part": "right"}));
    s.command(&json!({"op": "curve_move", "dv": -40.0}));
    s.command(&json!({"op": "up"}));
    let k = arm(&s).track.keys()[1];
    assert_eq!(k.handle, HandleType::Free);
    assert_eq!(k.lh, left);
    // the graph's handle buttons act on this one value only
    let thick = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().length.clone();
    assert!(thick.track.keys().iter().all(|k| k.handle == HandleType::AutoClamped));
}

#[test]
fn a_handle_on_a_linear_stretch_makes_it_smooth() {
    let mut s = posed();
    let id = arm_id(&s);
    s.command(&json!({"op": "easing", "v": "linear", "frame": 0}));
    assert_eq!(arm(&s).track.keys()[0].interp, Interp::Linear);
    s.command(&json!({"op": "curve_down", "property": id, "frame": 0, "part": "right"}));
    s.command(&json!({"op": "curve_move", "dv": 30.0}));
    s.command(&json!({"op": "up"}));
    assert_eq!(arm(&s).track.keys()[0].interp, Interp::Smooth, "the handle takes effect");
}

#[test]
fn adding_a_value_keys_it_once_or_only_shows_it() {
    let mut s = posed();
    let p = s.project().root().unwrap().layer("L2").unwrap();
    let (x, opacity) = (p.transform.x.id.clone(), p.opacity.id.clone());
    let undo = s.state()["undo"].clone();
    // a pose value that hasn't moved: shown, nothing to undo
    let st = s.command(&json!({"op": "curve_add", "property": x}));
    assert_eq!(st["undo"], undo);
    let c = channel(&st, &x);
    assert_eq!(c["unit"], "size");
    // a value with no keys: keyed on this frame, one step
    let st = s.command(&json!({"op": "curve_add", "property": opacity}));
    assert_eq!(st["undo"], "Add a value");
    let c = channel(&st, &opacity);
    assert_eq!(c["keys"].as_array().unwrap().len(), 1);
    // and it can be hidden again
    let st = s.command(&json!({"op": "curve_show", "property": x, "v": false}));
    assert!(st["curves"]["channels"].as_array().unwrap().iter().all(|c| c["id"] != x.as_str()));
}

#[test]
fn the_main_joint_moves_on_its_x_and_y() {
    // the user: "make sure it works for the root part too, for X, Y movement"
    let mut s = posed();
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.state();
    let x = s.project().root().unwrap().layer("L2").unwrap().transform.x.id.clone();
    let offered = st["curves"]["addable"].as_array().unwrap().iter().find(|c| c["id"] == x.as_str()).expect("the main joint's x is offered");
    assert_eq!(offered["group"], "Main joint");
    s.command(&json!({"op": "curve_add", "property": x}));
    s.command(&json!({"op": "curve_down", "property": x, "frame": 4, "part": "key"}));
    s.command(&json!({"op": "curve_move", "dv": 150.0}));
    s.command(&json!({"op": "up"}));
    let st = s.state();
    let c = channel(&st, &x);
    assert_eq!(c["focus"], true, "the main joint selected: its x and y come first");
    let fps = s.project().settings.fps as f64;
    let q = s.project().root().unwrap().layer("L2").unwrap().transform.x.clone();
    let (a, b, mid) = (q.key_value(0.0).f(), q.key_value(4.0 / fps).f(), q.key_value(2.0 / fps).f());
    assert!((b - a - 150.0).abs() < 1e-3);
    assert!(mid > a && mid < b, "moves between the poses");
}

#[test]
fn parts_can_be_named_and_the_graph_uses_the_names() {
    // D35 (the user: so they don't show up as "Node 32" in the graph)
    let mut s = posed();
    let id = arm_id(&s);
    let st = s.state();
    let before = channel(&st, &id)["name"].as_str().unwrap().to_string();
    assert!(before.ends_with(" angle") && !before.contains("Right"), "{before}");
    let st = s.command(&json!({"op": "rename_part", "node": "armR1", "name": "  Right forearm "}));
    assert_eq!(st["undo"], "Rename part");
    assert_eq!(channel(&st, &id)["name"], "Right forearm angle");
    assert_eq!(st["selected"]["partName"], "Right forearm");
    s.command(&json!({"op": "build", "layer": "L2"}));
    let st = s.state();
    let part = st["parts"]["list"].as_array().unwrap().iter().find(|p| p["id"] == "armR1").unwrap().clone();
    assert_eq!(part["name"], "Right forearm");
    // saved and read back
    let text = barkfart_core::codec::encode(s.project());
    assert!(text.contains("\"name\": \"Right forearm\""));
    let back = barkfart_core::codec::decode(&text).unwrap();
    assert_eq!(back.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().name.as_deref(), Some("Right forearm"));
    // an empty name: back to its shape and place
    s.command(&json!({"op": "build_done"}));
    let st = s.command(&json!({"op": "rename_part", "node": "armR1", "name": " "}));
    assert_eq!(channel(&st, &id)["name"].as_str().unwrap(), before);
    s.command(&json!({"op": "undo"}));
    assert_eq!(channel(&s.state(), &id)["name"], "Right forearm angle");
}
