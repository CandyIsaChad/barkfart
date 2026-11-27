//! A drag on a frame inside a pose changes that pose; "New pose" is how to
//! start another (the user, 2026-11-24).

use barkfart_core::edit::block_ops::pose_frames;
use barkfart_core::edit::Document;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::json;

/// Two poses, on frames 0 and 4; the first tweens into the second.
fn two_poses() -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    for _ in 0..3 {
        s.command(&json!({"op": "hold_longer"}));
    }
    s
}

/// The same, the first pose held until the second.
fn held_poses() -> Session {
    let mut s = two_poses();
    s.command(&json!({"op": "easing", "v": "hold", "block": true}));
    assert_eq!(s.state()["undo"], "Easing");
    s
}

fn frames(s: &Session) -> Vec<i32> {
    let p = s.project();
    pose_frames(p.root().unwrap().layer("L2").unwrap(), p.settings.fps as f64)
}

fn arm(s: &Session, frame: i32) -> f32 {
    let p = s.project();
    let q = &p.root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().rotation;
    q.key_value(frame as f64 / p.settings.fps as f64).f()
}

/// Drags the elbow by (dx, dy) on the playhead's frame.
fn drag_elbow(s: &mut Session, dx: f32, dy: f32) {
    let t = s.time();
    let h = s.tools.handles(s.project(), t).into_iter().find(|h| h.layer == "L2" && h.node == "armR1").expect("the elbow");
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 4}));
    s.command(&json!({"op": "move", "x": h.x + dx, "y": h.y + dy}));
    s.command(&json!({"op": "up"}));
}

#[test]
fn a_drag_inside_a_held_pose_splits_and_creates_new_pose_at_playhead() {
    let mut s = held_poses();
    assert_eq!(frames(&s), [0, 4]);
    s.command(&json!({"op": "frame", "i": 2}));
    let before0 = arm(&s, 0);
    let before_leg = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("legR1").unwrap().rotation.key_value(0.2).f();
    drag_elbow(&mut s, 60.0, 90.0);
    assert_eq!(frames(&s), [0, 2, 4], "auto-created new pose at playhead frame 2");
    assert_eq!(s.frame, 2, "playhead stays on frame 2");
    assert!((arm(&s, 0) - before0).abs() < 1e-4, "frame 0 pose untouched");
    assert!((arm(&s, 2) - before0).abs() > 20.0, "frame 2 pose changed");
    let after_leg = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("legR1").unwrap().rotation.key_value(0.2).f();
    assert!((after_leg - before_leg).abs() < 1e-4, "unedited joints do not jump");
    assert_eq!(s.state()["undo"], "Pose node");
    // 1 single Undo removes both the split pose and the edit!
    s.command(&json!({"op": "undo"}));
    assert_eq!(frames(&s), [0, 4]);
    assert!((arm(&s, 2) - before0).abs() < 1e-4);
    s.command(&json!({"op": "redo"}));
    assert_eq!(frames(&s), [0, 2, 4]);
    // on a pose's own first frame it edits in place without adding a duplicate key
    s.command(&json!({"op": "frame", "i": 4}));
    drag_elbow(&mut s, -40.0, 30.0);
    assert_eq!(frames(&s), [0, 2, 4]);
}

#[test]
fn a_drag_inside_a_tween_bakes_interpolated_pose_at_playhead() {
    let mut s = two_poses();
    // Pose frame 4 differently on TWO joints (armR1 and legR1) so frame 2 is truly mid-tween
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "node", "angle": 90.0}));
    s.command(&json!({"op": "select", "layer": "L2", "node": "legR1"}));
    s.command(&json!({"op": "node", "angle": 60.0}));

    s.command(&json!({"op": "frame", "i": 2}));
    let arm0 = arm(&s, 0);
    let arm4 = arm(&s, 4);
    let arm2_mid = arm(&s, 2);
    let leg2_mid = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("legR1").unwrap().rotation.key_value(0.2).f();
    assert!((arm2_mid - arm0).abs() > 10.0 && (arm4 - arm2_mid).abs() > 10.0, "frame 2 is mid-tween");

    drag_elbow(&mut s, -50.0, 20.0);
    assert_eq!(frames(&s), [0, 2, 4], "split block at playhead frame 2");
    assert_eq!(s.frame, 2, "playhead stayed on frame 2");
    assert!((arm(&s, 0) - arm0).abs() < 1e-4, "frame 0 untouched");
    assert!((arm(&s, 4) - arm4).abs() < 1e-4, "frame 4 untouched");
    assert!((arm(&s, 2) - arm2_mid).abs() > 5.0, "arm changed at frame 2");
    let leg2_after = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("legR1").unwrap().rotation.key_value(0.2).f();
    assert!((leg2_after - leg2_mid).abs() < 1e-3, "unedited legR1 preserved exact mid-tween angle ({leg2_mid} vs {leg2_after})");

    // 1 single undo returns to [0, 4] cleanly
    s.command(&json!({"op": "undo"}));
    assert_eq!(frames(&s), [0, 4]);
    s.command(&json!({"op": "redo"}));
    assert_eq!(frames(&s), [0, 2, 4]);
}

#[test]
fn lengthening_then_shortening_a_pose_is_a_round_trip() {
    // the bughunt (2026-11-24): grow then shrink left the animation a frame
    // longer, the last pose held for two frames
    let mut s = Session::new(Document::new(new_editor_project()));
    for _ in 0..4 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s.command(&json!({"op": "select", "layer": "L2"}));
    assert_eq!(frames(&s), [0, 1, 2, 3, 4]);
    assert_eq!(s.frame_count(), 5);
    s.command(&json!({"op": "frame", "i": 2}));
    s.command(&json!({"op": "block_length", "frames": 2}));
    assert_eq!(frames(&s), [0, 1, 2, 4, 5]);
    assert_eq!(s.frame_count(), 6);
    s.command(&json!({"op": "block_length", "frames": 1}));
    assert_eq!(frames(&s), [0, 1, 2, 3, 4]);
    assert_eq!(s.frame_count(), 5, "the frame is given back");
    // a last pose that lasts several frames keeps its length both ways
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "block_length", "frames": 3}));
    assert_eq!(s.frame_count(), 7);
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "block_length", "frames": 3}));
    assert_eq!(frames(&s), [0, 1, 4, 5, 6]);
    assert_eq!(s.frame_count(), 9, "the last pose still lasts 3 frames");
    s.command(&json!({"op": "block_length", "frames": 1}));
    assert_eq!(frames(&s), [0, 1, 2, 3, 4]);
    assert_eq!(s.frame_count(), 7);
    // shortening the last pose itself shrinks the animation back cleanly
    s.command(&json!({"op": "frame", "i": 4}));
    s.command(&json!({"op": "block_length", "frames": 1}));
    assert_eq!(s.frame_count(), 5);
    assert_eq!(frames(&s), [0, 1, 2, 3, 4]);
}
