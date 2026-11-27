//! Fixes from the cloud reviews of Gemini's rounds (2026-11-28).

use barkfart_core::edit::Document;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::json;

/// Six frames, the stock figure (L2) and its twin; returns the twin's id.
fn two_figures() -> (Session, String) {
    let mut s = Session::new(Document::new(new_editor_project()));
    for _ in 0..5 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    let st = s.command(&json!({"op": "duplicate_figure"}));
    let twin = st["selected"]["layer"].as_str().unwrap().to_string();
    (s, twin)
}

fn joins(s: &Session, id: &str) -> Vec<(i32, Option<String>)> {
    let fps = s.project().settings.fps as f64;
    s.project().root().unwrap().layer(id).unwrap().joins.iter().map(|j| ((j.time * fps).round() as i32, j.parent.clone())).collect()
}

fn join_at(s: &mut Session, twin: &str, frame: i32) {
    s.command(&json!({"op": "frame", "i": frame}));
    s.command(&json!({"op": "select", "layer": twin}));
    let st = s.command(&json!({"op": "join_here", "layer": "L2"}));
    assert_eq!(st["undo"], "Join");
}

#[test]
fn deleting_the_frame_a_join_starts_on_keeps_the_join() {
    let (mut s, twin) = two_figures();
    join_at(&mut s, &twin, 3);
    assert_eq!(joins(&s, &twin), [(3, Some("L2".into()))]);
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "delete_frame"}));
    assert_eq!(joins(&s, &twin), [(3, Some("L2".into()))], "old frame 4, now 3, still joined");
    // with a let-go on the next frame, that one takes the time over
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "select", "layer": twin}));
    s.command(&json!({"op": "let_go_here"}));
    assert_eq!(joins(&s, &twin), [(3, Some("L2".into())), (4, None)]);
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "delete_frame"}));
    assert_eq!(joins(&s, &twin), [(3, None)]);
}

#[test]
fn pasting_frames_before_a_join_moves_the_join() {
    let (mut s, twin) = two_figures();
    join_at(&mut s, &twin, 3);
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "copy_frame", "n": 1}));
    s.command(&json!({"op": "paste_frame"}));
    assert_eq!(joins(&s, &twin), [(5, Some("L2".into()))], "two frames pasted after frame 2");
}

#[test]
fn adding_an_effect_to_several_figures_adds_one_to_each() {
    let (mut s, twin) = two_figures();
    s.command(&json!({"op": "select_many", "layers": ["L2", twin]}));
    // the app names the anchor layer; `each` must still give each figure its own
    s.command(&json!({"op": "each", "label": "Add effect", "cmd": {"op": "fx_add", "type": "vignette", "layer": "L2"}}));
    let count = |id: &str| s.project().root().unwrap().layer(id).unwrap().effects.nodes.keys().filter(|k| k.starts_with("FX")).count();
    assert_eq!((count("L2"), count(&twin)), (1, 1));
}

#[test]
fn copy_and_paste_effects_carry_the_effect_stack() {
    let (mut s, twin) = two_figures();
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "fx_add", "type": "vignette"}));
    s.command(&json!({"op": "fx_add", "type": "glow"}));
    s.command(&json!({"op": "copy_filters"}));
    s.command(&json!({"op": "select", "layer": twin}));
    s.command(&json!({"op": "paste_filters"}));
    let p = s.project();
    let tys = |id: &str| p.root().unwrap().layer(id).unwrap().effects.chain().into_iter().filter(|e| e.id.starts_with("FX")).map(|e| e.ty.clone()).collect::<Vec<_>>();
    assert_eq!(tys(&twin), tys("L2"));
    assert_eq!(tys(&twin).len(), 2);
    // its own ids and property ids
    let tl = p.root().unwrap().layer(&twin).unwrap();
    for e in tl.effects.nodes.values() {
        assert!(e.mix.id.starts_with(&format!("{twin}/")), "{}", e.mix.id);
        assert!(p.root().unwrap().layer("L2").unwrap().effects.nodes.get(&e.id).is_none() || e.id.starts_with("filter."), "own id: {}", e.id);
    }
}

#[test]
fn a_moved_filter_stays_where_it_was_put() {
    let (mut s, _) = two_figures();
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "fx_add", "type": "vignette"}));
    s.command(&json!({"op": "filter", "blur": 6}));
    let order = |s: &Session| s.project().root().unwrap().layer("L2").unwrap().effects.chain().into_iter().filter(|e| e.id.starts_with("FX") || e.id == "filter.blur").map(|e| e.id.clone()).collect::<Vec<_>>();
    let before = order(&s);
    assert_eq!(before.len(), 2);
    assert!(before[0].starts_with("FX"), "a filter turned on joins the end: {before:?}");
    s.command(&json!({"op": "fx_reorder", "id": "filter.blur", "dir": -1}));
    assert_eq!(order(&s)[0], "filter.blur");
    // another filter control used: the order the user set stays
    s.command(&json!({"op": "filter", "blur": 9}));
    s.command(&json!({"op": "filter", "glow": 0.5}));
    assert_eq!(order(&s)[0], "filter.blur", "{:?}", order(&s));
}

fn shown(s: &Session, id: &str) -> Vec<i32> {
    let p = s.project();
    let fps = p.settings.fps as f64;
    let l = p.root().unwrap().layer(id).unwrap();
    (0..s.frame_count()).filter(|&f| barkfart_core::edit::block_ops::shown_at(l, f, fps)).collect()
}

/// A figure added on `frame` (shown there only) of an `n`-frame animation.
fn added_on(n: i32, frame: i32) -> (Session, String) {
    let mut s = Session::new(Document::new(new_editor_project()));
    for _ in 1..n {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "frame", "i": frame}));
    let st = s.command(&json!({"op": "add_figure"}));
    let id = st["selected"]["layer"].as_str().unwrap().to_string();
    assert_eq!(shown(&s, &id), [frame]);
    (s, id)
}

#[test]
fn a_moved_block_is_shown_only_where_it_went() {
    let (mut s, x) = added_on(8, 3);
    // Add time on the last frame: no keys on the new frames, x's last
    // hidden key holds across them
    s.command(&json!({"op": "frame", "i": 7}));
    s.command(&json!({"op": "add_time", "n": 12}));
    assert_eq!(s.frame_count(), 20);
    assert_eq!(shown(&s, &x), [3]);
    s.command(&json!({"op": "frame", "i": 3}));
    s.command(&json!({"op": "select", "layer": x}));
    s.command(&json!({"op": "move_block", "layer": x, "to": 12}));
    assert_eq!(shown(&s, &x), [12], "not on every frame after it");
}

#[test]
fn the_left_grip_resizes_a_block_on_its_own() {
    let (mut s, x) = added_on(10, 5);
    s.command(&json!({"op": "extend_to", "layer": x, "frame": 9}));
    assert_eq!(shown(&s, &x), (5..10).collect::<Vec<_>>());
    s.command(&json!({"op": "timeline_drag_start", "kind": "block_start", "layer": x, "frame": 5}));
    s.command(&json!({"op": "timeline_drag_move", "to": 2}));
    s.command(&json!({"op": "up"}));
    assert_eq!(shown(&s, &x), (2..10).collect::<Vec<_>>(), "its end stays on frame 10");
    assert_eq!(s.frame_count(), 10);
    s.command(&json!({"op": "timeline_drag_start", "kind": "block_start", "layer": x, "frame": 2}));
    s.command(&json!({"op": "timeline_drag_move", "to": 7}));
    s.command(&json!({"op": "up"}));
    assert_eq!(shown(&s, &x), (7..10).collect::<Vec<_>>());
    assert_eq!(s.frame_count(), 10, "the animation keeps its length");
}

#[test]
fn shortening_a_pose_gives_frames_back_with_another_figure_there() {
    let mut s = Session::new(Document::new(new_editor_project()));
    for _ in 0..4 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "select", "layer": "L2"}));
    s.command(&json!({"op": "duplicate_figure"}));
    s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(s.frame_count(), 5);
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "block_length", "frames": 3}));
    assert_eq!(s.frame_count(), 7);
    s.command(&json!({"op": "block_length", "frames": 1}));
    assert_eq!(s.frame_count(), 5, "the two frames are given back");
}
