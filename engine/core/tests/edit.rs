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
