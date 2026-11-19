//! Editing operations against the Java version (engine/testdata/edit, made
//! by EditDumpTest): each scenario runs the same operation with the same
//! arguments; the project text after it, and after undo, must match.

use barkfart_core::codec::{decode, encode};
use barkfart_core::edit::editor::{Editor, KeyRef};
use barkfart_core::edit::Document;
use barkfart_core::model::*;
use indexmap::IndexMap;
use std::path::PathBuf;

fn dir(group: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/edit").join(group)
}

fn keys(p: &Project, id: &str) -> Vec<KeyRef> {
    p.property(id).unwrap().track.keys().iter().map(|k| KeyRef { property: id.into(), time: k.time }).collect()
}

fn check(group: &str, name: &str, op: impl FnOnce(&mut Document, &mut Editor)) {
    let d = dir(group);
    let before = std::fs::read_to_string(d.join(format!("{name}.before.json"))).unwrap();
    let mut doc = Document::new(decode(&before).unwrap());
    assert_eq!(encode(doc.project()), before, "{name}: loads");
    let mut e = Editor::default();
    op(&mut doc, &mut e);
    let want = std::fs::read_to_string(d.join(format!("{name}.after.json"))).unwrap();
    let got = encode(doc.project());
    if want != got {
        let line = want.lines().zip(got.lines()).position(|(a, b)| a != b).unwrap_or(0);
        panic!("{name}: differs at line {}\n  java: {:?}\n  rust: {:?}", line + 1, want.lines().nth(line).unwrap_or(""), got.lines().nth(line).unwrap_or(""));
    }
    doc.undo();
    let want = std::fs::read_to_string(d.join(format!("{name}.undo.json"))).unwrap();
    assert_eq!(encode(doc.project()), want, "{name}: after undo");
}

#[test]
fn editor_operations_match_java() {
    check("editor", "change-autokey", |d, e| {
        e.change(d, "L2/armR1/rotation", Value::F(33.0), 0.5).unwrap();
    });
    check("editor", "change-static", |d, e| {
        e.auto_key = false;
        e.change(d, "L2/head/thickness", Value::F(14.0), 0.5).unwrap();
    });
    check("editor", "change-refused", |d, e| {
        e.auto_key = false;
        e.change(d, "L2/armR1/rotation", Value::F(1.0), 0.5).unwrap();
    });
    check("editor", "change-interp-follows", |d, e| {
        e.default_interp = Interp::Hold;
        e.change(d, "L2/legL1/rotation", Value::F(-100.0), 0.25).unwrap();
        e.change(d, "L2/legL1/rotation", Value::F(-90.0), 0.75).unwrap();
    });
    check("editor", "change-replaces-key", |d, e| {
        e.default_interp = Interp::Linear;
        let t = keys(d.project(), "L2/armR1/rotation")[1].time;
        e.change(d, "L2/armR1/rotation", Value::F(10.0), t).unwrap();
    });
    check("editor", "pose", |d, e| {
        let mut rot = IndexMap::new();
        rot.insert("armL1".to_string(), -120.0f32);
        let mut len = IndexMap::new();
        len.insert("head".to_string(), 44.0f32);
        let (s, l) = (d.project().root_symbol.clone(), d.project().root().unwrap().layers[0].id.clone());
        e.pose(d, &s, &l, &rot, &len, 0.61).unwrap();
    });
    check("editor", "retime", |d, e| {
        let k = keys(d.project(), "L2/armR1/rotation");
        e.retime(d, &k[1..2], 0.2).unwrap();
    });
    check("editor", "retime-onto-key", |d, e| {
        let k = keys(d.project(), "L2/armR1/rotation");
        e.retime(d, &k[0..1], k[1].time).unwrap();
    });
    check("editor", "scale-time", |d, e| {
        let mut sel = keys(d.project(), "L2/armR1/rotation");
        sel.extend(keys(d.project(), "L2/x"));
        e.scale_time(d, &sel, 0.5, 2.0).unwrap();
    });
    check("editor", "stagger", |d, e| {
        let p = d.project();
        let chain = vec![keys(p, "L2/x"), keys(p, "L2/armR1/rotation"), Vec::new(), keys(p, "L2/head/color")];
        e.stagger(d, &chain, 2.0).unwrap();
    });
    check("editor", "delete", |d, e| {
        let mut sel = keys(d.project(), "L2/armR1/rotation")[0..2].to_vec();
        sel.extend(keys(d.project(), "L2/head/color"));
        e.delete(d, &sel).unwrap();
    });
    check("editor", "copy-paste", |d, e| {
        let c = e.copy(d.project(), &keys(d.project(), "L2/armR1/rotation"));
        let mut map = IndexMap::new();
        map.insert("L2/armR1/rotation".to_string(), "L2/armL1/rotation".to_string());
        e.paste(d, &c, 1.25, &map).unwrap();
    });
    check("editor", "paste-type-mismatch", |d, e| {
        let c = e.copy(d.project(), &keys(d.project(), "L2/armR1/rotation"));
        let mut map = IndexMap::new();
        map.insert("L2/armR1/rotation".to_string(), "L2/head/color".to_string());
        e.paste(d, &c, 0.0, &map).unwrap();
    });
    check("editor", "easing", |d, e| {
        let mut sel = keys(d.project(), "L2/armR1/rotation");
        sel.extend(keys(d.project(), "L2/shown"));
        e.set_interp(d, &sel, Interp::Bezier, Ease::named(EaseKind::BounceOut)).unwrap();
    });
}

use barkfart_core::edit::figure_ops::{self as fo, BranchChange};
use barkfart_core::edit::structure as se;

/// Runs one structural edit as an undo step.
fn step(d: &mut Document, f: impl FnOnce(&mut barkfart_core::edit::State) -> barkfart_core::edit::EditResult<()>) {
    d.edit("op", f).unwrap();
}

#[test]
fn figure_and_structure_operations_match_java() {
    let g = "figure";
    check(g, "add-segment", |d, _| step(d, |s| fo::add_segment(s, "S1", "L2", "armR2", 30.0, 20.0).map(|_| ())));
    check(g, "add-segment-root", |d, _| step(d, |s| fo::add_segment(s, "S1", "L2", "hip", -45.0, 33.0).map(|_| ())));
    check(g, "split-3", |d, _| step(d, |s| fo::split(s, "S1", "L2", "armR1", 3)));
    check(g, "split-curved", |d, _| {
        step(d, |s| {
            let LayerKind::Figure(f) = &mut s.project.layer_mut("S1", "L2").unwrap().kind else { unreachable!() };
            f.node_mut("legL1").unwrap().curve = 12.0;
            Ok(())
        });
        step(d, |s| fo::split(s, "S1", "L2", "legL1", 4));
    });
    check(g, "duplicate-branch", |d, _| step(d, |s| fo::duplicate_branch(s, "S1", "L2", "armL1").map(|_| ())));
    check(g, "duplicate-branch-fill", |d, _| step(d, |s| fo::duplicate_branch(s, "S1", "L2", "torso").map(|_| ())));
    check(g, "flip-branch-h", |d, _| step(d, |s| fo::flip_branch(s, "S1", "L2", "armR1", true, 0.6)));
    check(g, "flip-branch-v", |d, _| step(d, |s| fo::flip_branch(s, "S1", "L2", "legL1", false, 0.0)));
    check(g, "flip-figure", |d, _| step(d, |s| fo::flip_figure(s, "S1", "L2", true)));
    check(g, "modify-thickness", |d, _| step(d, |s| fo::modify_branch(s, "S1", "L2", "torso", &BranchChange::Thickness(7.0))));
    check(g, "modify-color", |d, _| step(d, |s| fo::modify_branch(s, "S1", "L2", "armL1", &BranchChange::Color(Color::rgba8888(0x336699ff)))));
    check(g, "modify-shape", |d, _| step(d, |s| fo::modify_branch(s, "S1", "L2", "legR1", &BranchChange::Shape(Shape::Triangle))));
    check(g, "modify-scale", |d, _| step(d, |s| fo::modify_branch(s, "S1", "L2", "hip", &BranchChange::ScaleLength(1.5))));
    check(g, "modify-static", |d, _| step(d, |s| fo::modify_branch(s, "S1", "L2", "armR1", &BranchChange::Static(true))));
    check(g, "push-back", |d, _| step(d, |s| fo::push_branch(s, "S1", "L2", "armL1", false)));
    check(g, "push-forward", |d, _| step(d, |s| fo::push_branch(s, "S1", "L2", "legL1", true)));
    check(g, "pin", |d, _| step(d, |s| fo::set_pinned(s, "S1", "L2", "legR2", true)));
    check(g, "remove-node", |d, _| step(d, |s| se::remove_node(s, "S1", "L2", "armL1")));
    check(g, "remove-layer", |d, _| {
        let id = d.project().root().unwrap().layers[2].id.clone();
        step(d, |s| se::remove_layer(s, "S1", &id));
    });
    check(g, "move-layer", |d, _| step(d, |s| se::move_layer(s, "S1", "L2", 4)));
    check(g, "set-parent", |d, _| {
        let id = d.project().root().unwrap().layers[3].id.clone();
        step(d, |s| se::set_parent(s, "S1", &id, Some("L2")));
    });
    check(g, "remove-effect", |d, _| step(d, |s| se::remove_effect(s, "S1", "L2", "fx1")));
    check(g, "clear-effects", |d, _| step(d, |s| se::clear_effects(s, "S1", "L2")));
    check(g, "paste-clip", |d, _| step(d, |s| se::paste_clip(s, "S1", "Lb", "c1", 1.3).map(|_| ())));
    check(g, "canvas-size", |d, _| step(d, |s| se::set_canvas_size(s, 720, 1280)));
    check(g, "parallax", |d, _| {
        let id = d.project().root().unwrap().layers[3].id.clone();
        step(d, |s| se::set_parallax(s, "S1", &id, 7.0));
    });
    check(g, "copy-layer", |d, _| {
        step(d, |s| {
            let nid = s.project.new_id("L");
            let mut ids = std::collections::HashMap::new();
            ids.insert("L2".to_string(), nid);
            let c = se::copy_layer(s.project.root().unwrap().layer("L2").unwrap(), &ids)?;
            se::add_layer(s, "S1", c, 1)
        });
    });
}

use barkfart_core::edit::{camera_ops as co, join_ops as jo, layer_ops as lo, symbol_ops as so};

#[test]
fn join_layer_movieclip_and_camera_operations_match_java() {
    let g = "ops";
    check(g, "join-joint", |d, _| step(d, |s| jo::join(s, "S1", "L7", "L2", Some("armR2"), true, 0.6)));
    check(g, "join-no-rotate", |d, _| step(d, |s| jo::join(s, "S1", "L8", "L2", Some("head"), false, 0.6)));
    check(g, "join-figure", |d, _| step(d, |s| jo::join(s, "S1", "L7", "L2", None, true, 0.0)));
    check(g, "unjoin", |d, _| step(d, |s| jo::unjoin(s, "S1", "L5", 0.3)));
    check(g, "flip-joined", |d, _| {
        step(d, |s| jo::join(s, "S1", "L7", "L2", Some("armR2"), true, 0.6));
        step(d, |s| jo::flip_joined(s, "S1", "L2", Some("armR2")));
    });
    check(g, "lock-camera", |d, _| step(d, |s| co::lock(s, "S1", "L7", "Lc", true, 0.2)));
    check(g, "duplicate-joined", |d, _| step(d, |s| lo::duplicate(s, "S1", "L2", true, 30.0, -5.0).map(|_| ())));
    check(g, "duplicate-alone", |d, _| step(d, |s| lo::duplicate(s, "S1", "L5", false, 0.0, 12.0).map(|_| ())));
    check(g, "move-joined-forward", |d, _| step(d, |s| lo::move_with_joined(s, "S1", "L2", true)));
    check(g, "move-joined-back", |d, _| step(d, |s| lo::move_with_joined(s, "S1", "L8", false)));
    check(g, "paste-props", |d, e| {
        let c = lo::copy_props(d.project(), "S1", "L2", 0.6).unwrap();
        let ed = e.clone();
        step(d, |s| lo::paste_props(&ed, s, "S1", "L7", &c, true, true, 0.3));
    });
    check(g, "paste-props-pose-only", |d, e| {
        let c = lo::copy_props(d.project(), "S1", "L2", 0.2).unwrap();
        let ed = e.clone();
        step(d, |s| lo::paste_props(&ed, s, "S1", "L8", &c, false, false, 0.9));
    });
    check(g, "tweening-off", |d, e| {
        let ed = e.clone();
        step(d, |s| lo::set_tweening(&ed, s, "S1", "L2", false));
    });
    check(g, "convert-symbol", |d, _| step(d, |s| so::convert_to_symbol(s, "S1", &["L7".to_string(), "L8".to_string()], "clip").map(|_| ())));
    check(g, "break-apart", |d, _| step(d, |s| so::break_apart(s, "S1", "L5", 0.4).map(|_| ())));
    check(g, "camera-follow", |d, _| step(d, |s| co::follow(s, "S1", "Lc", "L2", 0, 12)));
    check(g, "pin-corner", |d, e| {
        let ed = e.clone();
        step(d, |s| co::pin_to_corner(&ed, s, "S1", "L8", "Lc", 2, 10.0, 0.0));
    });
    check(g, "polyfill", |d, _| {
        step(d, |s| {
            let f = s.project.root().unwrap().layer("L2").unwrap().as_figure().unwrap().clone();
            let mut b = co::PolyfillBuilder::new(&f, "torso")?;
            b.add(&f, "armR1");
            b.add(&f, "armR1");
            b.add(&f, "armR2");
            b.add(&f, "head");
            let id = co::PolyfillBuilder::free_id(&f);
            b.finish(s, "S1", "L2", &id, Some(Color::rgba8888(0xff000080)))
        });
    });
}

use barkfart_core::edit::frame_ops as fr;

#[test]
fn frame_operations_match_java() {
    let g = "frames";
    check(g, "insert-copy", |d, _| step(d, |s| fr::insert_after(s, 5, true)));
    check(g, "insert-blank", |d, _| step(d, |s| fr::insert_after(s, 5, false)));
    check(g, "delete-frame", |d, _| step(d, |s| fr::delete(s, 18).map(|_| ())));
    check(g, "delete-first", |d, _| step(d, |s| fr::delete(s, 0).map(|_| ())));
    check(g, "tween-off", |d, e| {
        let ed = e.clone();
        step(d, |s| fr::set_tween(&ed, s, 18, false));
    });
    check(g, "key-pose", |d, _| step(d, |s| fr::key_pose(s, None, 0.37).map(|_| ())));
    check(g, "key-pose-between", |d, _| step(d, |s| fr::key_pose_between(s, Some("L2"), 0.0, 0.75, 0.3).map(|_| ())));
    check(g, "hold", |d, _| step(d, |s| fr::hold(s, 18, 4, false)));
    check(g, "hold-camera-moves", |d, _| step(d, |s| fr::hold(s, 6, 3, true)));
    check(g, "insert-tweened", |d, _| step(d, |s| fr::insert_tweened(s, 10, 3)));
    check(g, "slow-motion", |d, _| step(d, |s| fr::slow_motion(s, 0, 2).map(|_| ())));
}

use barkfart_core::edit::canvas::{CanvasTools, Handle, KeyScope};

fn tools(e: &Editor) -> CanvasTools {
    CanvasTools::new(e.clone())
}

fn handle(c: &CanvasTools, d: &Document, node: &str, t: f64) -> Handle {
    c.handles(d.project(), t).into_iter().find(|h| h.node == node).expect("handle")
}

#[allow(clippy::too_many_arguments)]
fn drag(c: &mut CanvasTools, d: &mut Document, node: &str, t: f64, dx1: f32, dy1: f32, dx2: f32, dy2: f32) {
    let h = handle(c, d, node, t);
    c.begin(d, &h, h.x + 2.0, h.y - 1.0, t);
    c.move_to(d, h.x + 2.0 + dx1, h.y - 1.0 + dy1, t).unwrap();
    c.move_to(d, h.x + 2.0 + dx2, h.y - 1.0 + dy2, t).unwrap();
    c.end(d);
}

#[test]
fn canvas_tools_match_java() {
    use barkfart_core::edit::figure_ops::set_pinned;
    let g = "canvas";
    check(g, "drag-joint", |d, e| drag(&mut tools(e), d, "armR2", 0.5, 15.0, -20.0, 25.0, -10.0));
    check(g, "drag-unkeyed-time", |d, e| drag(&mut tools(e), d, "armL2", 0.33, -12.0, 30.0, -18.0, 22.0));
    check(g, "drag-stretch", |d, e| {
        let mut c = tools(e);
        c.stretch = true;
        drag(&mut c, d, "legR2", 0.4, 10.0, -10.0, 40.0, -35.0);
    });
    check(g, "drag-snap", |d, e| {
        let mut c = tools(e);
        c.snap_degrees = 45.0;
        drag(&mut c, d, "armL1", 0.2, 5.0, 5.0, 31.0, -17.0);
    });
    check(g, "drag-figure-scope", |d, e| {
        e.default_interp = Interp::Hold;
        let mut c = tools(e);
        c.key_scope = KeyScope::Figure;
        drag(&mut c, d, "head", 0.66, 8.0, 3.0, 14.0, 9.0);
    });
    check(g, "drag-root", |d, e| drag(&mut tools(e), d, "hip", 0.5, 30.0, 8.0, 44.0, -13.0));
    check(g, "drag-root-axis", |d, e| {
        let mut c = tools(e);
        c.axis_lock = true;
        drag(&mut c, d, "hip", 0.5, 30.0, 8.0, 5.0, -40.0);
    });
    check(g, "drag-root-axis-x", |d, e| {
        let mut c = tools(e);
        c.axis_lock = true;
        drag(&mut c, d, "hip", 0.5, 4.0, -30.0, 41.0, 17.0);
    });
    check(g, "drag-root-rotated", |d, e| {
        let mut c = tools(e);
        c.rotate_figure(d, "L2", 35.0, 0.5).unwrap();
        drag(&mut c, d, "hip", 0.5, 30.0, 8.0, 44.0, -13.0);
    });
    check(g, "drag-smart-stretch", |d, e| {
        // set outside the undo history, like the Java scenario
        let l = d.state.project.layer_mut("S1", "L2").unwrap();
        let LayerKind::Figure(f) = &mut l.kind else { panic!() };
        f.node_mut("legR1").unwrap().smart_stretch = true;
        step(d, |s| barkfart_core::edit::figure_ops::modify_branch(s, "S1", "L2", "legR2", &barkfart_core::edit::figure_ops::BranchChange::Static(true)));
        let mut c = tools(e);
        c.stretch = true;
        drag(&mut c, d, "legR1", 0.4, 10.0, -10.0, 30.0, -25.0);
    });
    check(g, "drag-pinned", |d, e| {
        step(d, |s| set_pinned(s, "S1", "L2", "legR2", true));
        step(d, |s| set_pinned(s, "S1", "L2", "armL2", true));
        drag(&mut tools(e), d, "hip", 0.5, 20.0, 25.0, 26.0, 31.0);
    });
    check(g, "drag-pinned-joint", |d, e| {
        step(d, |s| set_pinned(s, "S1", "L2", "armR2", true));
        drag(&mut tools(e), d, "torso", 0.5, -15.0, -6.0, -22.0, -9.0);
    });
    check(g, "drag-cancel", |d, e| {
        let mut c = tools(e);
        e.change(d, "L2/x", Value::F(3.0), 0.5).unwrap();
        let h = handle(&c, d, "armR1", 0.5);
        c.begin(d, &h, h.x, h.y, 0.5);
        c.move_to(d, h.x + 20.0, h.y + 20.0, 0.5).unwrap();
        c.cancel(d);
    });
    check(g, "rotate-figure", |d, e| tools(e).rotate_figure(d, "L2", 30.0, 0.5).unwrap());
    check(g, "move-all", |d, e| tools(e).move_all(d, 12.0, -7.0, 0.4).unwrap());
}

#[test]
fn canvas_picking_matches_java() {
    let want = std::fs::read_to_string(dir("canvas").join("picking.txt")).unwrap();
    let before = std::fs::read_to_string(dir("canvas").join("drag-joint.before.json")).unwrap();
    let d = Document::new(decode(&before).unwrap());
    let p = d.project();
    let mut c = CanvasTools::default();
    let mut got = Vec::new();
    for pass in 0..3 {
        c.show_static = pass == 1;
        c.main_nodes_only = pass == 2;
        got.push(format!("pass {pass}"));
        for h in c.handles(p, 0.5) {
            got.push(format!("{} {} {} {} {}", h.layer, h.node, h.x, h.y, h.root));
        }
    }
    c.show_static = false;
    c.main_nodes_only = false;
    let hip = handle(&c, &d, "hip", 0.5);
    let mut prev: Option<String> = None;
    for _ in 0..5 {
        let h = c.pick_cycling(p, hip.x + 3.0, hip.y + 2.0, 70.0, 0.5, None, prev.as_deref().map(|n| ("L2", n))).unwrap();
        got.push(format!("dig {}", h.node));
        prev = Some(h.node);
    }
    let h = c.pick_cycling(p, hip.x + 3.0, hip.y + 2.0, 70.0, 0.5, Some(("L2", "legL1")), None).unwrap();
    got.push(format!("selected {}", h.node));
    got.push(format!("pick {}", c.pick(p, hip.x + 3.0, hip.y + 2.0, 70.0, 0.5).unwrap().node));
    for n in c.neighbours(p, "L2", "torso", true, 0.5) {
        got.push(format!("neighbour {}", n.node));
    }
    // positions: equal to a few float ulps (Java prints the shortest float spelling)
    let want: Vec<&str> = want.lines().collect();
    assert_eq!(want.len(), got.len(), "line count");
    for (w, g) in want.iter().zip(&got) {
        let (ws, gs): (Vec<&str>, Vec<&str>) = (w.split(' ').collect(), g.split(' ').collect());
        assert_eq!(ws.len(), gs.len(), "{w} / {g}");
        for (a, b) in ws.iter().zip(&gs) {
            match (a.parse::<f32>(), b.parse::<f32>()) {
                (Ok(x), Ok(y)) => assert!((x - y).abs() <= 1e-4 * (1.0 + x.abs()), "{w} / {g}"),
                _ => assert_eq!(a, b, "{w} / {g}"),
            }
        }
    }
}
