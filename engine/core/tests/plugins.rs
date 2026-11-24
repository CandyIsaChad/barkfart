//! K2: plugin modifiers in projects (SCRIPTING.md). The registry is shared by
//! the whole process, so each test uses plugin ids of its own.

use barkfart_core::codec;
use barkfart_core::edit::Document;
use barkfart_core::session::{new_editor_project, Session};
use serde_json::{json, Value as J};

fn stutter(id: &str, extra: &str) -> String {
    format!(
        r#"local bf = require("@barkfart")
return bf.modifier {{
	id = "{id}", name = "Stutter", version = "1.0.0", author = "yeah",
	props = {{ every = bf.number {{ default = 3, min = 1, step = 1 }}{extra} }},
	value = function(v, p, t) return v:at(math.floor(t.frame / p.every) * p.every) end,
}}"#
    )
}

/// The arm turning steadily over 8 frames (Linear, so frame values are easy).
fn moving() -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    let p = s.doc.project_mut();
    p.settings.fps = 8.0;
    let sym = p.root_symbol.clone();
    p.root_mut().unwrap().duration = 1.0;
    let l = p.layer_mut(&sym, "L2").unwrap();
    let barkfart_core::model::LayerKind::Figure(f) = &mut l.kind else { panic!() };
    let q = &mut f.node_mut("armR1").unwrap().rotation;
    q.track.clear();
    use barkfart_core::model::{Interp, Key, Value};
    q.track.put(Key::new(0.0, Value::F(0.0), Interp::Linear));
    q.track.put(Key::new(1.0, Value::F(80.0), Interp::Linear));
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    s.command(&json!({"op": "set", "k": "curves", "v": true}));
    s
}

const ARM: &str = "L2/armR1/rotation";

fn arm_at(s: &Session, frame: f64) -> f64 {
    let p = s.project();
    let q = p.property(ARM).unwrap();
    barkfart_core::eval::Resolver::new(p).resolve(q, frame / p.settings.fps as f64).f() as f64
}

fn channel(st: &J) -> J {
    st["curves"]["channels"].as_array().unwrap().iter().find(|c| c["id"] == ARM).cloned().expect("the arm's channel")
}

#[test]
fn a_plugin_modifier_drives_a_value_and_undoes_in_one_step() {
    let mut s = moving();
    let st = s.command(&json!({"op": "plugin_install", "name": "stutter.luau", "source": stutter("t1.stutter", "")}));
    assert_eq!(st["message"], "Stutter 1.0.0 is ready.");
    assert!(st["plugins"].as_array().unwrap().iter().any(|p| p["id"] == "t1.stutter" && p["kind"] == "modifier"));
    let before: Vec<f64> = (0..8).map(|f| arm_at(&s, f as f64)).collect();
    let undo = s.state()["undo"].clone();
    let st = s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "plugin", "id": "t1.stutter"}));
    assert_eq!(st["undo"], "Add modifier");
    let got: Vec<f64> = (0..8).map(|f| arm_at(&s, f as f64)).collect();
    assert_eq!(got, vec![before[0], before[0], before[0], before[3], before[3], before[3], before[6], before[6]]);
    let c = channel(&st);
    assert_eq!(c["mods"][0]["name"], "Stutter");
    assert_eq!(c["mods"][0]["settings"][0]["name"], "every");
    assert_eq!(c["mods"][0]["settings"][0]["value"], 3.0);
    assert!(!c["result"].as_array().unwrap().is_empty(), "the result is sampled for the graph");
    // settings, then off, then removed: one step each
    let st = s.command(&json!({"op": "modifier_set", "property": ARM, "index": 0, "props": {"every": 2}}));
    assert_eq!(st["undo"], "Modifier");
    assert_eq!(arm_at(&s, 3.0), before[2]);
    s.command(&json!({"op": "modifier_set", "property": ARM, "index": 0, "enabled": false}));
    assert_eq!(arm_at(&s, 3.0), before[3], "switched off, the keys show");
    let st = s.command(&json!({"op": "modifier_remove", "property": ARM, "index": 0}));
    assert_eq!(st["undo"], "Remove modifier");
    for _ in 0..4 {
        s.command(&json!({"op": "undo"}));
    }
    assert_eq!(s.state()["undo"], undo);
    assert!(s.project().property(ARM).unwrap().modifiers.is_empty());
}

#[test]
fn a_project_keeps_a_plugin_it_cannot_find() {
    let mut s = moving();
    s.command(&json!({"op": "plugin_install", "name": "s.luau", "source": stutter("t2.stutter", "")}));
    s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "plugin", "id": "t2.stutter"}));
    s.command(&json!({"op": "modifier_set", "property": ARM, "index": 0, "props": {"every": 4}}));
    let text = codec::encode(s.project());
    assert!(text.contains("\"type\": \"plugin\"") && text.contains("t2.stutter"), "saved");
    s.command(&json!({"op": "plugin_uninstall", "id": "t2.stutter"}));
    // missing: the value passes through; saving and opening keeps every setting
    let keyed = |f: f64| f * 10.0;
    for f in 0..8 {
        assert!((arm_at(&s, f as f64) - keyed(f as f64)).abs() < 1e-3, "frame {f} passes through");
    }
    let back = codec::decode(&text).unwrap();
    assert_eq!(codec::encode(&back), text, "byte for byte without the plugin");
    let st = s.state();
    let m = &channel(&st)["mods"][0];
    assert_eq!(m["missing"], true);
    assert_eq!(m["name"], "t2.stutter");
    // installed again (a newer version with another setting): the old setting
    // is kept, the new one starts from its default
    s.command(&json!({"op": "plugin_install", "name": "s.luau", "source": stutter("t2.stutter", ", wobble = bf.toggle { default = true }")}));
    let st = s.state();
    let m = &channel(&st)["mods"][0];
    assert_eq!(m["missing"], false);
    let every = m["settings"].as_array().unwrap().iter().find(|x| x["name"] == "every").unwrap();
    assert_eq!(every["value"], 4.0);
    let wobble = m["settings"].as_array().unwrap().iter().find(|x| x["name"] == "wobble").unwrap();
    assert_eq!(wobble["value"], true);
    assert_eq!(arm_at(&s, 5.0), keyed(4.0));
}

#[test]
fn a_plugin_that_misbehaves_is_switched_off() {
    let mut s = moving();
    let slow = r#"local bf = require("@barkfart")
return bf.modifier { id = "t3.slow", name = "Slow", version = "1", value = function(v, p, t) while true do end end }"#;
    s.command(&json!({"op": "plugin_install", "name": "slow.luau", "source": slow}));
    s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "plugin", "id": "t3.slow"}));
    let start = std::time::Instant::now();
    for f in 0..8 {
        assert!((arm_at(&s, f as f64) - f as f64 * 10.0).abs() < 1e-3, "passes through");
    }
    assert!(start.elapsed().as_millis() < 1000, "stopped once, then off: {:?}", start.elapsed());
    let st = s.state();
    let p = st["plugins"].as_array().unwrap().iter().find(|p| p["id"] == "t3.slow").unwrap().clone();
    assert_eq!(p["off"], "The plugin took too long and was stopped.");
    assert_eq!(channel(&st)["mods"][0]["off"], p["off"]);
    // a broken file never installs, and says why
    let st = s.command(&json!({"op": "plugin_install", "name": "broken.luau", "source": "return {"}));
    assert!(st["message"].as_str().unwrap().starts_with("The plugin couldn't be loaded"), "{}", st["message"]);
}

#[test]
fn built_in_modifiers_go_on_a_value_too() {
    let mut s = moving();
    let keyed: Vec<f64> = (0..8).map(|f| arm_at(&s, f as f64)).collect();
    let st = s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "swing"}));
    assert_eq!(st["undo"], "Add modifier");
    let c = channel(&st);
    assert_eq!(c["mods"][0]["kind"], "swing");
    assert_eq!(c["mods"][0]["amount"], 15.0);
    let swung: Vec<f64> = (0..8).map(|f| arm_at(&s, f as f64)).collect();
    assert_ne!(swung, keyed);
    s.command(&json!({"op": "modifier_set", "property": ARM, "index": 0, "amount": 0}));
    let flat: Vec<f64> = (0..8).map(|f| arm_at(&s, f as f64)).collect();
    for (a, b) in flat.iter().zip(&keyed) {
        assert!((a - b).abs() < 1e-3);
    }
    // a loop goes first, whatever order it was added in
    s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "wiggle"}));
    let st = s.command(&json!({"op": "modifier_add", "property": ARM, "kind": "loop"}));
    let kinds: Vec<String> = channel(&st)["mods"].as_array().unwrap().iter().map(|m| m["kind"].as_str().unwrap().to_string()).collect();
    assert_eq!(kinds, ["loop", "swing", "wiggle"]);
    // a colour can't take one
    let col = s.project().root().unwrap().layer("L2").unwrap().as_figure().unwrap().node("armR1").unwrap().color.id.clone();
    let st = s.command(&json!({"op": "modifier_add", "property": col, "kind": "wiggle"}));
    assert!(st["message"].as_str().unwrap().contains("numbers"));
}
