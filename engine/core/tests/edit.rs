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
