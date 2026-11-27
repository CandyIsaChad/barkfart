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
fn a_drag_inside_a_held_pose_changes_that_pose() {
    let mut s = held_poses();
    assert_eq!(frames(&s), [0, 4]);
    s.command(&json!({"op": "frame", "i": 2}));
    let before = arm(&s, 0);
    drag_elbow(&mut s, 60.0, 90.0);
    assert_eq!(frames(&s), [0, 4], "no new pose on frame 3");
    assert_eq!(s.frame, 2, "the playhead stays: the pose looks the same here");
    assert!((arm(&s, 0) - before).abs() > 20.0, "the pose itself changed");
    assert_eq!(arm(&s, 2), arm(&s, 0));
    assert_eq!(s.state()["undo"], "Pose node");
    // on a pose's own first frame it is as before
    s.command(&json!({"op": "frame", "i": 4}));
    drag_elbow(&mut s, -40.0, 30.0);
    assert_eq!(frames(&s), [0, 4]);
}

#[test]
fn a_drag_inside_a_tween_goes_to_its_pose_first() {
    let mut s = two_poses();
    s.command(&json!({"op": "frame", "i": 2}));
    let first = arm(&s, 0);
    drag_elbow(&mut s, -50.0, 20.0);
    assert_eq!(frames(&s), [0, 4], "no new pose on frame 3");
    assert_eq!(s.frame, 0, "the playhead went to the pose being changed");
    assert!((arm(&s, 0) - first).abs() > 10.0);
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
