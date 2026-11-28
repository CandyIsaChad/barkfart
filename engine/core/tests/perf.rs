//! Performance benchmarks and regressions for posing/dragging dense 250+ node rigs
//! (e.g. Iron Man) and Figure Studio node-visibility filtering.

use barkfart_core::edit::Document;
use barkfart_core::eval;
use barkfart_core::model::*;
use barkfart_core::session::Session;
use barkfart_core::tess;
use serde_json::json;
use std::path::Path;
use std::time::Instant;

/// Builds a synthetic 254-node rig (16 movable skeleton joints + 238 static armor
/// details) matching dense Stick Nodes characters like Iron Man.
fn dense_rig_session() -> Session {
    let mut p = barkfart_core::session::new_editor_project();
    let sym = p.root_symbol.clone();
    p.symbol_mut(&sym).unwrap().layers.clear();
    let mut layer = Layer::figure("L1", Some("Armored Hero"));
    if let LayerKind::Figure(f) = &mut layer.kind {
        let root = Node::new("L1", "n0", None, Shape::None);
        f.add(root).unwrap();
        // 15 movable joints across torso, head, arms, legs
        let mut movable = vec!["n0".to_string()];
        let mut idx = 1usize;
        for chain in 0..5 {
            let mut parent = "n0".to_string();
            for step in 0..3 {
                let id = format!("n{idx}");
                idx += 1;
                let mut n = Node::new("L1", &id, Some(&parent), Shape::Line);
                n.rotation.value = Value::F((chain as f32 * 55.0) - 90.0 + step as f32 * 15.0);
                n.length.value = Value::F(60.0);
                n.rest_length = 60.0;
                n.thickness.value = Value::F(18.0);
                n.stretchy = step == 1;
                n.smart_stretch = step == 1;
                f.add(n).unwrap();
                movable.push(id.clone());
                parent = id;
            }
        }
        // 238 static detail nodes attached across the movable bones
        for d in 0..238 {
            let parent = &movable[1 + (d % (movable.len() - 1))];
            let id = format!("n{idx}");
            idx += 1;
            let mut n = Node::new(
                "L1",
                &id,
                Some(parent),
                if d % 3 == 0 {
                    Shape::Trapezoid
                } else if d % 3 == 1 {
                    Shape::Circle
                } else {
                    Shape::Line
                },
            );
            n.is_static = true;
            n.rotation.value = Value::F(((d * 37) % 360) as f32 - 180.0);
            n.length.value = Value::F(14.0 + (d % 12) as f32);
            n.rest_length = n.length.value.f();
            n.thickness.value = Value::F(6.0 + (d % 8) as f32);
            n.gradient = d % 4 == 0;
            f.add(n).unwrap();
        }
        f.capture_base();
    }
    p.symbol_mut(&sym).unwrap().layers.push(std::sync::Arc::new(layer));
    let mut s = Session::new(Document::new(p));
    // Add 10 frames so timeline rows/marks/keyed frames reflect a real animation
    for _ in 0..9 {
        s.command(&json!({"op": "insert_frame"}));
    }
    s
}

fn iron_man_session() -> Option<Session> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata/legacy/local/_Iron Man.nodes");
    let bytes = std::fs::read(&path).ok()?;
    let p = barkfart_core::session::new_editor_project_with("_Iron Man.nodes", &bytes);
    let mut s = Session::new(Document::new(p));
    for _ in 0..9 {
        s.command(&json!({"op": "insert_frame"}));
    }
    Some(s)
}

fn bench_drag(label: &str, s: &mut Session, fast: bool) -> (f64, usize, f64) {
    let hs = s.tools.handles(s.project(), s.time());
    let target = hs.iter().find(|h| !h.root).expect("non-root handle").clone();
    let down = s.command(&json!({"op": "down", "x": target.x, "y": target.y, "r": 24.0}));
    assert_eq!(down["hit"], true, "{label}: hit target handle");
    // Cross drag slop first
    let _ = s.command(&json!({"op": "move", "x": target.x + 25.0, "y": target.y + 10.0, "fast": fast}));

    const ITERS: usize = 60;
    let mut last_bytes = 0usize;
    let t0 = Instant::now();
    for i in 0..ITERS {
        let a = i as f32 * 0.1;
        let out = s.command(&json!({
            "op": "move",
            "x": target.x + 30.0 + a.cos() * 20.0,
            "y": target.y + 15.0 + a.sin() * 20.0,
            "fast": fast,
        }));
        if fast {
            assert_eq!(out["fast"], true, "fast move must return fast patch");
            assert!(out.get("rows").is_none(), "fast move must omit rows");
            assert!(out.get("modifierTargets").is_none(), "fast move must omit modifierTargets");
            assert!(out["handles"].is_array(), "fast move must include handles");
        } else {
            assert!(out["rows"].is_array(), "default move keeps full state for backward compatibility");
        }
        last_bytes = serde_json::to_vec(&out).unwrap().len();
    }
    let move_us = t0.elapsed().as_secs_f64() * 1e6 / ITERS as f64;

    let t1 = Instant::now();
    let mut tris = 0usize;
    for _ in 0..ITERS {
        let scene = eval::evaluate_with(s.project(), &s.project().root_symbol, s.time(), false, false);
        let list = tess::layered(&scene, None);
        tris = list.triangle_count();
    }
    let eval_tess_us = t1.elapsed().as_secs_f64() * 1e6 / ITERS as f64;

    s.command(&json!({"op": "up"}));
    eprintln!(
        "[perf] {label} (fast={fast}): move+json={move_us:.1} us ({:.2} ms), payload={last_bytes} bytes, eval+tess={eval_tess_us:.1} us ({:.2} ms, {tris} tris)",
        move_us / 1000.0,
        eval_tess_us / 1000.0
    );
    (move_us, last_bytes, eval_tess_us)
}

#[test]
fn dense_rig_drag_benchmark_probe() {
    let mut syn = dense_rig_session();
    let (_, full_bytes, _) = bench_drag("synthetic-254-nodes", &mut syn, false);
    let (_, fast_bytes, _) = bench_drag("synthetic-254-nodes", &mut syn, true);
    assert!(
        fast_bytes * 20 < full_bytes,
        "fast drag payload ({fast_bytes} B) should be >20x smaller than full state ({full_bytes} B)"
    );
    if let Some(mut im) = iron_man_session() {
        let (_, im_full, _) = bench_drag("iron-man-254-nodes", &mut im, false);
        let (_, im_fast, _) = bench_drag("iron-man-254-nodes", &mut im, true);
        assert!(
            im_fast * 20 < im_full,
            "Iron Man fast drag payload ({im_fast} B) should be >20x smaller than full state ({im_full} B)"
        );
    }
}

#[test]
fn figure_studio_unlocked_only_hides_and_skips_hit_testing_static_nodes() {
    let mut s = dense_rig_session();
    // Outside Figure Studio, show_static defaults to false (16 unlocked handles)
    assert_eq!(s.state()["opts"]["static"], false);
    assert_eq!(s.state()["handles"].as_array().unwrap().len(), 16);

    // Entering Figure Studio defaults to "All nodes" (static = true, all 254 handles visible)
    let st_enter = s.command(&json!({"op": "build", "layer": "L1"}));
    assert_eq!(st_enter["opts"]["static"], true);
    assert_eq!(st_enter["handles"].as_array().unwrap().len(), 254);

    // Find a static detail node that sits far enough from any unlocked joint so we can test hit-testing cleanly
    let all_hs = s.tools.handles(s.project(), s.time());
    let unlocked_pts: Vec<(f32, f32)> = {
        let f = s.project().root().unwrap().layer("L1").unwrap().as_figure().unwrap();
        all_hs
            .iter()
            .filter(|h| !f.node(&h.node).unwrap().is_static)
            .map(|h| (h.x, h.y))
            .collect()
    };
    let static_target = {
        let f = s.project().root().unwrap().layer("L1").unwrap().as_figure().unwrap();
        all_hs
            .iter()
            .filter(|h| f.node(&h.node).unwrap().is_static)
            .max_by(|a, b| {
                let da = unlocked_pts.iter().map(|(x, y)| (a.x - x).hypot(a.y - y)).fold(f32::INFINITY, f32::min);
                let db = unlocked_pts.iter().map(|(x, y)| (b.x - x).hypot(b.y - y)).fold(f32::INFINITY, f32::min);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("static detail node")
            .clone()
    };
    let min_dist_to_unlocked = unlocked_pts
        .iter()
        .map(|(x, y)| (static_target.x - x).hypot(static_target.y - y))
        .fold(f32::INFINITY, f32::min);
    assert!(min_dist_to_unlocked > 4.0, "static target should be distinct from unlocked joints");

    // In "All nodes" mode, tapping that static detail node selects it
    let tap_all = s.command(&json!({"op": "down", "x": static_target.x, "y": static_target.y, "r": 3.0}));
    s.command(&json!({"op": "up"}));
    assert_eq!(tap_all["hit"], true);
    assert_eq!(tap_all["selected"]["node"], static_target.node);

    // Switch to "Unlocked only" while that static node is selected:
    // root + 15 unlocked joints + currently selected static node = 17 visible handles
    let st_unlocked_keep = s.command(&json!({"op": "set", "k": "static", "v": false}));
    assert_eq!(st_unlocked_keep["opts"]["static"], false);
    assert_eq!(
        st_unlocked_keep["handles"].as_array().unwrap().len(),
        17,
        "Unlocked only keeps root + unlocked joints + currently selected static node"
    );

    // Select an unlocked joint (root "n0"): now the previously selected static detail node hides!
    let st_unlocked = s.command(&json!({"op": "select", "layer": "L1", "node": "n0"}));
    assert_eq!(
        st_unlocked["handles"].as_array().unwrap().len(),
        16,
        "Unlocked only shows exactly 16 unlocked handles once an unlocked node is selected"
    );

    // Hit-testing in "Unlocked only" MUST skip hidden static detail nodes (they cannot steal taps!)
    let tap_hidden = s.command(&json!({"op": "down", "x": static_target.x, "y": static_target.y, "r": 3.0}));
    s.command(&json!({"op": "up"}));
    assert_eq!(tap_hidden["hit"], false, "hidden static node must not be hit-testable in Unlocked only mode");

    // Switching back to "All nodes" restores all 254 handles immediately and makes the static node tappable again
    let st_back = s.command(&json!({"op": "set", "k": "static", "v": true}));
    assert_eq!(st_back["handles"].as_array().unwrap().len(), 254);
    let tap_again = s.command(&json!({"op": "down", "x": static_target.x, "y": static_target.y, "r": 3.0}));
    s.command(&json!({"op": "up"}));
    assert_eq!(tap_again["hit"], true);
    assert_eq!(tap_again["selected"]["node"], static_target.node);

    // Exiting Figure Studio cleanly restores the pre-studio show_static state (false)
    let st_exit = s.command(&json!({"op": "build_cancel"}));
    assert_eq!(st_exit["opts"]["static"], false);
    assert_eq!(st_exit["handles"].as_array().unwrap().len(), 16);
}
