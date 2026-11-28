//! Smart-stretch tests: synthetic 2D affine smart-stretch coverage (normal pose
//! drag, stretch mode, length stepper, reset segment, locked/static/angle-locked
//! details, no-smart-stretch opt-out, studio workflow, undo/redo) plus a
//! local-only Iron Man rig test (skipped cleanly when the local `.nodes`
//! fixture is absent, per docs/barkfart/CLEANROOM.md).

use barkfart_core::codec;
use barkfart_core::edit::canvas::{CanvasTools, DRAG_HORIZONTAL};
use barkfart_core::edit::Document;
use barkfart_core::eval::{FigurePose, Resolver};
use barkfart_core::legacy;
use barkfart_core::model::*;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::json;

/// Builds a synthetic rig with a stretchy + smart-stretch limb `arm` (len 100,
/// angle 0 deg) carrying:
/// - `back`: static parallel return segment (len 100, 180 deg)
/// - `diag1` -> `diag2`: diagonal static armor chevron (each len 50*sqrt(2) at
///   135 deg / -135 deg relative to arm, returning to the shoulder origin (0,0)!)
/// - `pinned_detail`: static + pinned ("Lock node") diagonal detail
/// - `rel_lock`: static + LOCK_RELATIVE diagonal detail
/// - `abs_lock`: static + LOCK_ABSOLUTE detail
/// - `drag_lock_detail`: static + drag_lock detail
/// - `opt_out` -> `opt_out_child`: static with `no_smart_stretch: true`
/// - `forearm`: non-static child joint (must NOT be stretched)
fn build_rig_session() -> (Session, String) {
    let mut p = new_editor_project();
    p.root_mut().unwrap().layers.clear();
    let lid = p.new_id("L");
    let mut layer = Layer::figure(&lid, Some("Rig"));
    {
        let LayerKind::Figure(f) = &mut layer.kind else { unreachable!() };
        let add = |f: &mut Figure, id: &str, parent: Option<&str>, rot: f32, len: f32, is_static: bool| {
            let mut n = Node::new(&lid, id, parent, if parent.is_none() { Shape::None } else { Shape::Line });
            n.rotation.value = Value::F(rot);
            n.length.value = Value::F(len);
            n.rest_length = len;
            n.is_static = is_static;
            f.add(n).unwrap();
        };
        add(f, "root", None, 0.0, 0.0, false);
        add(f, "arm", Some("root"), 0.0, 100.0, false);
        f.node_mut("arm").unwrap().stretchy = true;
        f.node_mut("arm").unwrap().smart_stretch = true;

        // Parallel static armor back to shoulder
        add(f, "back", Some("arm"), 180.0, 100.0, true);

        // Diagonal static chevron: (100,0) -> (50,50) -> (0,0)
        let diag = (50.0f32).hypot(50.0);
        add(f, "diag1", Some("arm"), 135.0, diag, true);
        add(f, "diag2", Some("diag1"), 90.0, diag, true); // world 225 deg -> (-50,-50) back to (0,0)

        // Static + pinned ("Lock node") detail
        add(f, "pinned_detail", Some("arm"), 150.0, 40.0, true);
        f.node_mut("pinned_detail").unwrap().pinned = true;

        // Static + LOCK_RELATIVE detail (child of diag1, angle measured from arm)
        add(f, "rel_lock", Some("diag1"), -135.0, 30.0, true);
        f.node_mut("rel_lock").unwrap().angle_lock = LOCK_RELATIVE;

        // Static + LOCK_ABSOLUTE detail
        add(f, "abs_lock", Some("arm"), 45.0, 40.0, true);
        f.node_mut("abs_lock").unwrap().angle_lock = LOCK_ABSOLUTE;

        // Static + drag_lock detail
        add(f, "drag_lock_detail", Some("arm"), -150.0, 40.0, true);
        f.node_mut("drag_lock_detail").unwrap().drag_lock = DRAG_HORIZONTAL;

        // Opt-out branch (do not smart-stretch)
        add(f, "opt_out", Some("arm"), 180.0, 30.0, true);
        f.node_mut("opt_out").unwrap().no_smart_stretch = true;
        add(f, "opt_out_child", Some("opt_out"), 45.0, 25.0, true);

        // Non-static child joint (forearm)
        add(f, "forearm", Some("arm"), 30.0, 80.0, false);
        add(f, "forearm_detail", Some("forearm"), 180.0, 80.0, true);

        f.capture_base();
    }
    p.root_mut().unwrap().layers.push(std::sync::Arc::new(layer));
    let s = Session::new(Document::new(p));
    (s, lid)
}

fn node_pose(p: &Project, layer: &str, t: f64) -> (Figure, FigurePose) {
    let f = p.root().unwrap().layer(layer).unwrap().as_figure().unwrap().clone();
    let mut r = Resolver::new(p);
    let pose = FigurePose::compute(&f, t, &mut r);
    (f, pose)
}

fn node_len(p: &Project, layer: &str, node: &str, t: f64) -> f32 {
    let f = p.root().unwrap().layer(layer).unwrap().as_figure().unwrap();
    let mut r = Resolver::new(p);
    r.f(&f.node(node).unwrap().length, t)
}

fn node_rot(p: &Project, layer: &str, node: &str, t: f64) -> f32 {
    let f = p.root().unwrap().layer(layer).unwrap().as_figure().unwrap();
    let mut r = Resolver::new(p);
    r.f(&f.node(node).unwrap().rotation, t)
}

#[test]
fn affine_smart_stretch_scales_lengths_and_angles_with_zero_drift() {
    let (mut s, lid) = build_rig_session();
    let before_json = codec::encode(s.project());

    // Drag `arm` in normal pose mode (stretch_all == false, arm.stretchy == true)
    // from (100, 0) to (220, 0) -> k = 2.2
    let h = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "arm").unwrap();
    s.command(&json!({"op": "down", "x": h.x, "y": h.y, "r": 10}));
    s.command(&json!({"op": "move", "x": h.x + 60.0, "y": h.y}));
    s.command(&json!({"op": "move", "x": h.x + 120.0, "y": h.y}));
    s.command(&json!({"op": "up"}));

    let p = s.project();
    let arm_len = node_len(p, &lid, "arm", 0.0);
    assert!((arm_len - 220.0).abs() < 0.2, "arm stretched to ~220: {arm_len}");
    let k = (arm_len / 100.0) as f64;

    let (f, pose) = node_pose(p, &lid, 0.0);
    let at = |id: &str| {
        let i = f.index_of(id).unwrap();
        (pose.x[i], pose.y[i])
    };

    // Parallel back segment reaches shoulder (0, 0)
    let (bx, by) = at("back");
    assert!(bx.hypot(by) < 1e-3, "back reaches shoulder: ({bx}, {by})");

    // Diagonal armor chevron: diag1 apex at (50*k, 50) (perpendicular height 50 preserved!),
    // diag2 tip returns EXACTLY to shoulder (0, 0)!
    let (d1x, d1y) = at("diag1");
    assert!((d1x - 50.0 * k).abs() < 1e-2 && (d1y - 50.0).abs() < 1e-2, "diag1 apex affine: ({d1x}, {d1y}) vs ({}, 50)", 50.0 * k);
    let (d2x, d2y) = at("diag2");
    assert!(d2x.hypot(d2y) < 1e-2, "diag2 returns to shoulder: ({d2x}, {d2y})");

    // Static + pinned ("Lock node") detail stretches cleanly instead of being yanked by hold_pins
    let b_pin = 150.0f64.to_radians();
    let want_pin_len = 40.0 * ((k * b_pin.cos()).powi(2) + b_pin.sin().powi(2)).sqrt();
    let got_pin_len = node_len(p, &lid, "pinned_detail", 0.0) as f64;
    assert!((got_pin_len - want_pin_len).abs() < 0.05, "pinned static detail stretched: {got_pin_len} vs {want_pin_len}");

    // Static + LOCK_RELATIVE, LOCK_ABSOLUTE, and drag_lock details all stretched cleanly
    for (id, rest, deg) in [("rel_lock", 30.0f64, -135.0f64), ("abs_lock", 40.0, 45.0), ("drag_lock_detail", 40.0, -150.0)] {
        let br = deg.to_radians();
        let want = rest * ((k * br.cos()).powi(2) + br.sin().powi(2)).sqrt();
        let got = node_len(p, &lid, id, 0.0) as f64;
        assert!((got - want).abs() < 0.05, "{id}: got {got}, want {want}");
    }

    // Opt-out (`no_smart_stretch`) branch and non-static child limb stay untouched
    assert!((node_len(p, &lid, "opt_out", 0.0) - 30.0).abs() < 1e-4);
    assert!((node_len(p, &lid, "opt_out_child", 0.0) - 25.0).abs() < 1e-4);
    assert!((node_len(p, &lid, "forearm", 0.0) - 80.0).abs() < 1e-4);
    assert!((node_len(p, &lid, "forearm_detail", 0.0) - 80.0).abs() < 1e-4);

    // Drag a SECOND time: squash to k = 0.35, then drag back to rest k = 1.0 -> zero compounding drift!
    let h2 = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "arm").unwrap();
    s.command(&json!({"op": "down", "x": h2.x, "y": h2.y, "r": 10}));
    s.command(&json!({"op": "move", "x": 35.0, "y": h2.y}));
    s.command(&json!({"op": "up"}));
    let (f_sq, pose_sq) = node_pose(s.project(), &lid, 0.0);
    let d2_sq = f_sq.index_of("diag2").unwrap();
    assert!(pose_sq.x[d2_sq].hypot(pose_sq.y[d2_sq]) < 1e-2, "squashed diag2 still anchored at shoulder: ({}, {})", pose_sq.x[d2_sq], pose_sq.y[d2_sq]);

    let h3 = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "arm").unwrap();
    s.command(&json!({"op": "down", "x": h3.x, "y": h3.y, "r": 10}));
    s.command(&json!({"op": "move", "x": 100.0, "y": h3.y}));
    s.command(&json!({"op": "up"}));
    assert!((node_len(s.project(), &lid, "diag1", 0.0) - (50.0f32).hypot(50.0)).abs() < 0.05);
    assert!((node_rot(s.project(), &lid, "diag1", 0.0) - 135.0).abs() < 0.05);
    assert!((node_rot(s.project(), &lid, "diag2", 0.0) - 90.0).abs() < 0.05);

    // 3 drags -> 3 undos restore the exact original project byte-for-byte
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before_json, "undo restores exact project");
    s.command(&json!({"op": "redo"}));
    assert!((node_len(s.project(), &lid, "arm", 0.0) - 220.0).abs() < 0.2, "redo restores stretched pose");
}

#[test]
fn smart_stretch_via_length_stepper_reset_and_canvas_direct() {
    let (mut s, lid) = build_rig_session();
    let before_json = codec::encode(s.project());

    // Select `arm` and stretch via Segment Tools length stepper (`op: "node", "length": 180`)
    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "node", "length": 180.0}));

    let (f, pose) = node_pose(s.project(), &lid, 0.0);
    let d1 = f.index_of("diag1").unwrap();
    let d2 = f.index_of("diag2").unwrap();
    assert!((pose.x[d1] - 90.0).abs() < 1e-2 && (pose.y[d1] - 50.0).abs() < 1e-2, "stepper diag1: ({}, {})", pose.x[d1], pose.y[d1]);
    assert!(pose.x[d2].hypot(pose.y[d2]) < 1e-2, "stepper diag2 anchored at origin: ({}, {})", pose.x[d2], pose.y[d2]);
    assert!((node_len(s.project(), &lid, "back", 0.0) - 180.0).abs() < 1e-2);
    assert!((node_len(s.project(), &lid, "opt_out", 0.0) - 30.0).abs() < 1e-4);

    // Reset segment restores BOTH `arm` AND all smart-stretched static details back to rest!
    s.command(&json!({"op": "reset_segment"}));
    assert!((node_len(s.project(), &lid, "arm", 0.0) - 100.0).abs() < 1e-3);
    assert!((node_len(s.project(), &lid, "back", 0.0) - 100.0).abs() < 1e-3);
    assert!((node_len(s.project(), &lid, "diag1", 0.0) - (50.0f32).hypot(50.0)).abs() < 1e-2);
    assert!((node_rot(s.project(), &lid, "diag1", 0.0) - 135.0).abs() < 1e-2);

    // Two undos (reset_segment + node length) restore byte-identical state
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), before_json);

    // Direct CanvasTools drag on a stretchy node works even with tools.stretch == false
    let mut doc = Document::new(s.project().clone());
    let mut tools = CanvasTools::default();
    tools.stretch = false;
    tools.smart_stretch_angles = true;
    let h = tools.handles(doc.project(), 0.0).into_iter().find(|h| h.node == "arm").unwrap();
    tools.begin(&mut doc, &h, h.x, h.y, 0.0);
    tools.move_to(&mut doc, h.x + 50.0, h.y, 0.0).unwrap();
    tools.end(&mut doc);
    assert!((node_len(doc.project(), &lid, "arm", 0.0) - 150.0).abs() < 0.2);
    assert!((node_len(doc.project(), &lid, "back", 0.0) - 150.0).abs() < 0.2);
    let (f2, pose2) = node_pose(doc.project(), &lid, 0.0);
    let d2 = f2.index_of("diag2").unwrap();
    assert!(pose2.x[d2].hypot(pose2.y[d2]) < 1e-2);
}

#[test]
fn smart_stretch_on_static_child_and_reset_impulse_multiplier() {
    let (mut s, lid) = build_rig_session();
    // Turn smart_stretch OFF on `arm` and put it ON `diag1` (direct static child),
    // and set stretchReset x 0.5 on `diag2`.
    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "segment", "smart": false}));
    s.command(&json!({"op": "select", "layer": lid, "node": "diag1"}));
    s.command(&json!({"op": "segment", "smart": true}));
    s.command(&json!({"op": "select", "layer": lid, "node": "diag2"}));
    s.command(&json!({"op": "segment", "stretchReset": true, "stretchMultiplier": 0.5}));

    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "node", "length": 200.0})); // k = 2.0

    // `back` does NOT have smart_stretch (and arm doesn't), so `back` stays 100
    assert!((node_len(s.project(), &lid, "back", 0.0) - 100.0).abs() < 1e-3);
    // `diag1` has smart_stretch -> stretches with k = 2.0
    let diag = (50.0f64).hypot(50.0);
    let want_d1 = diag * ((2.0 * 135.0f64.to_radians().cos()).powi(2) + 135.0f64.to_radians().sin().powi(2)).sqrt();
    assert!((node_len(s.project(), &lid, "diag1", 0.0) as f64 - want_d1).abs() < 0.05);
    // `diag2` has stretchReset x 0.5 -> k_eff = 1 + 0.5*(2 - 1) = 1.5
    let want_d2 = diag * ((1.5 * 225.0f64.to_radians().cos()).powi(2) + 225.0f64.to_radians().sin().powi(2)).sqrt();
    assert!((node_len(s.project(), &lid, "diag2", 0.0) as f64 - want_d2).abs() < 0.05);
}

#[test]
fn studio_smart_stretch_apply_and_legacy_nodes_export_preserves_flag() {
    let (mut s, lid) = build_rig_session();
    // Simulate a legacy v171 imported figure (like Iron Man) where smart_stretch starts false
    {
        let sym = s.project().root_symbol.clone();
        let l = s.doc.project_mut().layer_mut(&sym, &lid).unwrap();
        let LayerKind::Figure(f) = &mut l.kind else { unreachable!() };
        f.node_mut("root").unwrap().legacy_extras.insert("fig.version".into(), "i:171".into());
        f.node_mut("arm").unwrap().stretchy = false;
        f.node_mut("arm").unwrap().smart_stretch = false;
    }

    // Add a second frame and enter Figure Studio from frame 1 (verifies `build` resets to frame 0 cleanly)
    s.command(&json!({"op": "insert_frame"}));
    s.command(&json!({"op": "frame", "i": 1}));
    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "build"}));
    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "node", "stretchy": true}));
    s.command(&json!({"op": "segment", "smart": true}));
    s.command(&json!({"op": "build_apply"}));

    // Export to .nodes and re-read: smartStretch must survive even though source was v171!
    let layer = s.project().root().unwrap().layer(&lid).unwrap();
    let lf = legacy::to_legacy(layer).unwrap();
    assert!(lf.version >= 252, "legacy version upgraded to carry smartStretch: v{}", lf.version);
    let bytes = legacy::write_nodes(&lf);
    let reread = legacy::read_nodes(&bytes).unwrap();
    let arm_ln = reread.nodes.iter().find(|n| n.i("smartStretch") != 0);
    assert!(arm_ln.is_some(), "smartStretch flag preserved in exported .nodes");

    // Back on frame 1, stretching `arm` smart-stretches its static details
    s.command(&json!({"op": "select", "layer": lid, "node": "arm"}));
    s.command(&json!({"op": "node", "length": 160.0}));
    let (f, pose) = node_pose(s.project(), &lid, s.time());
    let d2 = f.index_of("diag2").unwrap();
    assert!(pose.x[d2].hypot(pose.y[d2]) < 1e-2, "studio-enabled smart stretch works on frame 1: ({}, {})", pose.x[d2], pose.y[d2]);
}

/// Local-only test against the real Iron Man `.nodes` rig (skipped cleanly when
/// the local reference fixture is absent, per docs/barkfart/CLEANROOM.md).
#[test]
fn iron_man_rig_smart_stretch_stretches_all_locked_limb_details_and_undoes_cleanly() {
    let candidates = [
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../main/Payload/IOSLauncher.app/stickfigures/_Iron Man.nodes"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/legacy/local/_Iron Man.nodes"),
    ];
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else {
        return; // local fixture absent (e.g. CI): skip cleanly
    };

    let mut p = new_editor_project();
    p.root_mut().unwrap().layers.clear();
    let mut s = Session::new(Document::new(p));
    let st = s.import("_Iron Man.nodes", &bytes);
    let lid = st["selected"]["layer"].as_str().expect("imported layer").to_string();

    // Open Iron Man in Figure Studio, make upper arm (n27) and shin (n49) stretchy + smart stretch, apply.
    s.command(&json!({"op": "build", "layer": lid}));
    for limb in ["n27", "n49"] {
        s.command(&json!({"op": "select", "layer": lid, "node": limb}));
        s.command(&json!({"op": "node", "stretchy": true}));
        s.command(&json!({"op": "segment", "smart": true}));
    }
    s.command(&json!({"op": "build_apply"}));

    // Capture rest pose & serialized snapshot before stretching
    let rest_json = codec::encode(s.project());
    let (f_rest, pose_rest) = node_pose(s.project(), &lid, 0.0);

    // Helper: collect all static descendants of `limb_id` (stopping at non-static children)
    let static_desc = |f: &Figure, limb_id: &str| -> Vec<usize> {
        let si = f.index_of(limb_id).unwrap();
        let mut out = Vec::new();
        let mut stack = vec![si];
        while let Some(u) = stack.pop() {
            let uid = &f.nodes()[u].id;
            for (j, n) in f.nodes().iter().enumerate() {
                if n.parent.as_deref() == Some(uid.as_str()) && n.is_static && !n.no_smart_stretch {
                    out.push(j);
                    stack.push(j);
                }
            }
        }
        out
    };

    // 1) Drag-stretch Iron Man's upper arm `n27` (16 static armor details) in normal pose mode!
    let n27_desc = static_desc(&f_rest, "n27");
    assert_eq!(n27_desc.len(), 16, "Iron Man upper arm n27 has 16 static detail nodes");
    let n27_i = f_rest.index_of("n27").unwrap();
    let n27_pi = pose_rest.parent[n27_i] as usize;
    let h_arm = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "n27").unwrap();
    let h_sh = s.tools.handles(s.project(), 0.0).into_iter().find(|h| h.node == "n113").unwrap();
    // Pull elbow along the upper-arm direction to ~2.2x length
    let (vx, vy) = (h_arm.x - h_sh.x, h_arm.y - h_sh.y);
    s.command(&json!({"op": "down", "x": h_arm.x, "y": h_arm.y, "r": 12}));
    s.command(&json!({"op": "move", "x": h_sh.x + vx * 2.2, "y": h_sh.y + vy * 2.2}));
    s.command(&json!({"op": "up"}));

    let arm_len = node_len(s.project(), &lid, "n27", 0.0);
    let k = (arm_len / f_rest.nodes()[n27_i].rest_length) as f64;
    assert!((k - 2.2).abs() < 0.05, "n27 stretched to ~2.2x (k={k})");

    // Verify EVERY SINGLE ONE of the 16 static detail nodes matches exact 2D affine scaling (< 0.05px error)
    let (_f_st, pose_st) = node_pose(s.project(), &lid, 0.0);
    let (ox, oy) = (pose_rest.x[n27_pi], pose_rest.y[n27_pi]);
    let u_rest = pose_rest.angle[n27_i].to_radians();
    let u_new = pose_st.angle[n27_i].to_radians();
    let (ux0, uy0) = (u_rest.cos(), u_rest.sin());
    let (nx0, ny0) = (-uy0, ux0);
    let (ux1, uy1) = (u_new.cos(), u_new.sin());
    let (nx1, ny1) = (-uy1, ux1);
    let mut stretched_count = 0usize;
    for &j in &n27_desc {
        let dx0 = pose_rest.x[j] - ox;
        let dy0 = pose_rest.y[j] - oy;
        let along = (dx0 * ux0 + dy0 * uy0) * k;
        let perp = dx0 * nx0 + dy0 * ny0;
        let want_x = ox + along * ux1 + perp * nx1;
        let want_y = oy + along * uy1 + perp * ny1;
        let err = (pose_st.x[j] - want_x).hypot(pose_st.y[j] - want_y);
        assert!(err < 0.05, "Iron Man n27 detail {} drifted by {err:.4}px", f_rest.nodes()[j].id);
        let l0 = f_rest.nodes()[j].rest_length;
        let l1 = node_len(s.project(), &lid, &f_rest.nodes()[j].id, 0.0);
        if (l1 - l0).abs() > 1.0 {
            stretched_count += 1;
        }
    }
    assert_eq!(stretched_count, n27_desc.len(), "every static detail on n27 stretched proportionally");
    // Forearm `n4` (non-static child of n27) must keep its original length 110.0
    assert!((node_len(s.project(), &lid, "n4", 0.0) - 110.0).abs() < 1e-3, "forearm n4 untouched");

    // Undo drag -> byte-identical to rest_json!
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), rest_json, "Iron Man drag undo is byte-identical");

    // 2) Stretch Iron Man's shin `n49` (6 static diagonal armor details n47, n50->n51->n52->n64)
    // via Segment Tools length stepper (160 -> 400, k = 2.5): every static armor node must match
    // exact 2D affine scaling (< 0.05px error), keeping `n64` anchored to knee `n63` (0.82px
    // scaled from its 0.40px rest gap, instead of detaching by 25.8px without angle updates!).
    let n49_desc = static_desc(&f_rest, "n49");
    assert_eq!(n49_desc.len(), 6, "Iron Man shin n49 has 6 static detail nodes");
    let n49_i = f_rest.index_of("n49").unwrap();
    let n49_pi = pose_rest.parent[n49_i] as usize;
    s.command(&json!({"op": "select", "layer": lid, "node": "n49"}));
    s.command(&json!({"op": "node", "length": 400.0}));
    let (f_shin, pose_shin) = node_pose(s.project(), &lid, 0.0);
    let k_shin = (400.0 / f_rest.nodes()[n49_i].rest_length) as f64;
    let (sox, soy) = (pose_rest.x[n49_pi], pose_rest.y[n49_pi]);
    let su = pose_rest.angle[n49_i].to_radians();
    let (sux, suy) = (su.cos(), su.sin());
    let (snx, sny) = (-suy, sux);
    for &j in &n49_desc {
        let dx0 = pose_rest.x[j] - sox;
        let dy0 = pose_rest.y[j] - soy;
        let along = (dx0 * sux + dy0 * suy) * k_shin;
        let perp = dx0 * snx + dy0 * sny;
        let want_x = sox + along * sux + perp * snx;
        let want_y = soy + along * suy + perp * sny;
        let err = (pose_shin.x[j] - want_x).hypot(pose_shin.y[j] - want_y);
        assert!(err < 0.05, "Iron Man n49 detail {} drifted by {err:.4}px", f_rest.nodes()[j].id);
    }
    let knee_i = f_shin.index_of("n63").unwrap();
    let n64_i = f_shin.index_of("n64").unwrap();
    let stretched_gap = (pose_shin.x[n64_i] - pose_shin.x[knee_i]).hypot(pose_shin.y[n64_i] - pose_shin.y[knee_i]);
    assert!(stretched_gap < 1.2, "Iron Man shin armor n64 stayed anchored to knee (gap {stretched_gap:.2}px)");

    // Reset segment restores shin AND all 6 static armor details back to rest
    s.command(&json!({"op": "reset_segment"}));
    for &j in &n49_desc {
        let id = &f_rest.nodes()[j].id;
        assert!((node_len(s.project(), &lid, id, 0.0) - f_rest.nodes()[j].rest_length).abs() < 0.05, "reset restored {id} length");
    }

    // Undo reset + undo stepper -> byte-identical to rest_json
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "undo"}));
    assert_eq!(codec::encode(s.project()), rest_json, "Iron Man stepper+reset undo is byte-identical");
}
