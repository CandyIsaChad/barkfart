//! K2: Script layers (SCRIPTING.md "Script layers"). The plugin registry is
//! shared by the whole process, so each test uses plugin ids of its own.

use barkfart_core::codec;
use barkfart_core::edit::Document;
use barkfart_core::eval::Resolver;
use barkfart_core::model::{Interp, Key, LayerKind, Project, Value};
use barkfart_core::session::{new_editor_project, Session};
use serde_json::{json, Value as J};

/// A script plugin whose `step` body is `body` (it returns the table).
fn script(id: &str, body: &str) -> String {
    format!(
        r#"local bf = require("@barkfart")
return bf.script {{
	id = "{id}", name = "Sway", version = "1.0.0", author = "yeah",
	targets = {{ fig = bf.figure(), v = bf.value {{ label = "Strength" }} }},
	props = {{ amount = bf.number {{ default = 10, min = 0 }} }},
	step = function(s, p, t, scene)
		{body}
	end,
}}"#
    )
}

/// The stock figure, 16 frames at 8 fps, sliding right at 80 px a second.
fn sliding() -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    let p = s.doc.project_mut();
    p.settings.fps = 8.0;
    let sym = p.root_symbol.clone();
    p.root_mut().unwrap().duration = 2.0;
    let l = p.layer_mut(&sym, "L2").unwrap();
    let q = &mut l.transform.x;
    let x0 = q.value.f();
    q.track.clear();
    q.track.put(Key::new(0.0, Value::F(x0), Interp::Linear));
    q.track.put(Key::new(2.0, Value::F(x0 + 160.0), Interp::Linear));
    s.command(&json!({"op": "select", "layer": "L2"}));
    s
}

fn root_rotation_id(p: &Project) -> String {
    let LayerKind::Figure(f) = &p.root().unwrap().layer("L2").unwrap().kind else { panic!() };
    f.nodes()[0].rotation.id.clone()
}

/// A value at a frame: with scripts (`scripted`) or the keys alone.
fn value(p: &Project, id: &str, frame: f64, scripted: bool) -> f64 {
    let q = p.property(id).unwrap();
    let t = frame / p.settings.fps as f64;
    let mut r = if scripted { Resolver::new(p) } else { Resolver::plain(p) };
    r.resolve(q, t).f() as f64
}

fn add(s: &mut Session, id: &str, body: &str, from: i64, to: i64) -> J {
    let st = s.command(&json!({"op": "plugin_install", "name": "sway.luau", "source": script(id, body)}));
    assert_eq!(st["message"], "Sway 1.0.0 is ready.", "{}", st["message"]);
    s.command(&json!({"op": "script_add", "id": id, "from": from, "to": to}))
}

fn script_layer(st: &J) -> String {
    st["script"]["id"].as_str().expect("the Script page is open").to_string()
}

#[test]
fn a_script_moves_a_figure_on_top_of_its_keys_and_undoes_in_one_step() {
    let mut s = sliding();
    let st = add(&mut s, "s1.sway", "return { fig = { rotate = { root = p.amount }, move = { y = t.elapsed * 8 } } }", 2, 10);
    assert_eq!(st["undo"], "Add script");
    let lid = script_layer(&st);
    // its row: a script, on Hero (the stock figure), with one run of 8 frames
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["id"] == lid.as_str()).cloned().expect("a script row");
    assert_eq!(row["kind"], "script");
    assert_eq!(row["runs"][0][0], 2);
    assert_eq!(row["runs"][0][1], 8);
    assert_eq!(row["runs"][0][3].as_array().unwrap().len(), 8, "a trace of the run");
    // its page: what it moves, its settings, its run
    let page = &st["script"];
    assert_eq!(page["targets"][0]["name"], "fig");
    assert_eq!(page["targets"][0]["to"], "L2");
    assert_eq!(page["targets"][1]["label"], "Strength");
    assert_eq!(page["targets"][1]["to"], J::Null);
    assert_eq!(page["settings"][0]["name"], "amount");
    assert_eq!(page["settings"][0]["value"], 10.0);
    assert_eq!(page["run"], page["runs"][0]["id"]);

    let p = s.project();
    let rot = root_rotation_id(p);
    for f in [2.0, 4.0, 9.0, 9.5] {
        assert!((value(p, &rot, f, true) - value(p, &rot, f, false) - 10.0).abs() < 1e-4, "frame {f}: on top of the keys");
    }
    for f in [0.0, 1.0, 10.0, 12.0] {
        assert!((value(p, &rot, f, true) - value(p, &rot, f, false)).abs() < 1e-6, "frame {f}: outside the run, the keys");
    }
    // t.elapsed: 0 on the block's first frame, blended between frames
    assert!((value(p, "L2/y", 6.0, true) - value(p, "L2/y", 6.0, false) - 4.0).abs() < 1e-4);
    assert!((value(p, "L2/y", 6.5, true) - value(p, "L2/y", 6.5, false) - 4.5).abs() < 1e-4);
    // the stage draws it: the figure's picture changes
    let a = barkfart_core::eval::evaluate(p, &p.root_symbol, 4.0 / 8.0);
    s.command(&json!({"op": "script_set", "visible": false}));
    let p = s.project();
    let b = barkfart_core::eval::evaluate(p, &p.root_symbol, 4.0 / 8.0);
    assert_ne!(format!("{:?}", a.root), format!("{:?}", b.root), "a hidden script does nothing");
    s.command(&json!({"op": "undo"}));
    // its settings change it
    s.command(&json!({"op": "script_set", "props": {"amount": 25}}));
    let p = s.project();
    assert!((value(p, &rot, 4.0, true) - value(p, &rot, 4.0, false) - 25.0).abs() < 1e-4);
    // undo, undo: no script
    s.command(&json!({"op": "undo"}));
    let st = s.command(&json!({"op": "undo"}));
    assert!(st["rows"].as_array().unwrap().iter().all(|r| r["kind"] != "script"));
    let p = s.project();
    assert!((value(p, &rot, 4.0, true) - value(p, &rot, 4.0, false)).abs() < 1e-6);
}

fn run_values(p: &Project, id: &str, frames: impl Iterator<Item = i64>) -> Vec<f64> {
    frames.map(|f| value(p, id, f as f64, true) - value(p, id, f as f64, false)).collect()
}

#[test]
fn runs_are_repeatable_whatever_order_frames_are_drawn_in_and_each_has_its_own_seed() {
    let mut s = sliding();
    let st = add(&mut s, "s2.noise", "return { fig = { rotate = { root = t.random(0, 100) } } }", 0, 6);
    let lid = script_layer(&st);
    let st = s.command(&json!({"op": "script_run_add", "layer": lid, "frame": 8, "len": 6}));
    assert_eq!(st["script"]["runs"].as_array().unwrap().len(), 2);
    assert_eq!(st["script"]["run"], st["script"]["runs"][1]["id"], "the new run is picked");
    let p = s.project().clone();
    let rot = root_rotation_id(&p);
    let first = run_values(&p, &rot, 0..6);
    let second = run_values(&p, &rot, 8..14);
    assert!(first.iter().all(|v| (0.0..100.0).contains(v)));
    assert_ne!(first, second, "each run has its own random numbers");
    // drawn backwards on another thread (nothing cached there): the same
    let (p2, rot2) = (p.clone(), rot.clone());
    let back = std::thread::spawn(move || {
        let mut v = run_values(&p2, &rot2, (0..6).rev());
        v.reverse();
        v
    })
    .join()
    .unwrap();
    assert_eq!(first, back);
    // a new seed: different numbers, one undo step
    let r0 = st["script"]["runs"][0]["id"].as_str().unwrap().to_string();
    let st = s.command(&json!({"op": "script_run", "run": r0, "newSeed": true}));
    assert_eq!(st["undo"], "Script run");
    assert_ne!(run_values(s.project(), &rot, 0..6), first);
}

#[test]
fn editing_what_a_script_reads_changes_its_result() {
    let mut s = sliding();
    add(&mut s, "s3.drag", "return { fig = { rotate = { root = scene.fig.velocity.x * 0.1 } } }", 2, 8);
    let rot = root_rotation_id(s.project());
    // 80 px a second → 8 degrees
    assert!((run_values(s.project(), &rot, 4..5)[0] - 8.0).abs() < 1e-3);
    // twice as fast
    {
        let p = s.doc.project_mut();
        let sym = p.root_symbol.clone();
        let q = &mut p.layer_mut(&sym, "L2").unwrap().transform.x;
        let x0 = q.track.keys()[0].value.f();
        q.track.put(Key::new(2.0, Value::F(x0 + 320.0), Interp::Linear));
    }
    assert!((run_values(s.project(), &rot, 4..5)[0] - 16.0).abs() < 1e-3, "worked out again");
}

#[test]
fn replace_keep_and_mix_each_do_their_part() {
    let mut s = sliding();
    // give the main joint a keyed turn, so "replace" can be told from "add"
    {
        let p = s.doc.project_mut();
        let rot = root_rotation_id(p);
        p.property_mut(&rot).unwrap().track.put(Key::new(0.0, Value::F(30.0), Interp::Linear));
    }
    let st = add(&mut s, "s4.hold", "return { fig = { rotate = { root = 50 } } }", 2, 6);
    let run = st["script"]["run"].as_str().unwrap().to_string();
    let rot = root_rotation_id(s.project());
    let keyed = value(s.project(), &rot, 3.0, false);
    assert!((value(s.project(), &rot, 3.0, true) - (keyed + 50.0)).abs() < 1e-4, "added on top");
    s.command(&json!({"op": "script_run", "run": run, "replace": true}));
    assert!((value(s.project(), &rot, 3.0, true) - 50.0).abs() < 1e-4, "replaces the keys");
    s.command(&json!({"op": "script_run", "run": run, "mix": 0.5}));
    assert!((value(s.project(), &rot, 3.0, true) - (keyed + 0.5 * (50.0 - keyed))).abs() < 1e-3, "half mixed in");
    // after the block: the keys, unless it keeps its last frame
    let after = value(s.project(), &rot, 9.0, false);
    assert!((value(s.project(), &rot, 9.0, true) - after).abs() < 1e-6);
    let st = s.command(&json!({"op": "script_run", "run": run, "keep": true}));
    assert_eq!(st["script"]["runs"][0]["keep"], true);
    assert!((value(s.project(), &rot, 9.0, true) - (after + 0.5 * (50.0 - after))).abs() < 1e-3, "holds after the block");
    // moving the block keeps its length; trimming can't go under a frame
    let st = s.command(&json!({"op": "script_run", "run": run, "from": 5, "move": true}));
    assert_eq!((st["script"]["runs"][0]["from"].as_i64(), st["script"]["runs"][0]["frames"].as_i64()), (Some(5), Some(4)));
    let st = s.command(&json!({"op": "script_run", "run": run, "to": 2}));
    assert_eq!(st["script"]["runs"][0]["frames"], 1);
}

#[test]
fn a_script_drives_effect_settings_plugin_effects_included() {
    let mut s = sliding();
    // a blur on the figure, and a plugin effect with a setting of its own
    s.command(&json!({"op": "filter", "blur": 4}));
    let fx = r#"local bf = require("@barkfart")
return bf.effect { id = "s5.ripple", name = "Ripple", version = "1.0.0",
	props = { amount = bf.number { default = 2 } },
	shader = bf.wgsl("fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> { return layer(uv); }") }"#;
    s.command(&json!({"op": "plugin_install", "name": "ripple.luau", "source": fx}));
    s.command(&json!({"op": "plugin_effect", "id": "s5.ripple"}));
    let st = add(&mut s, "s5.pulse", "return { v = p.amount + scene.v }", 0, 16);
    let values = st["script"]["values"].as_array().unwrap();
    let pick = |needle: &str| values.iter().find(|v| v["id"].as_str().unwrap().contains(needle)).map(|v| v["id"].as_str().unwrap().to_string()).unwrap_or_else(|| panic!("{needle} in {values:?}"));
    let blur = pick("radius");
    let ripple = pick("plugin.s5.ripple/amount");
    for target in [blur, ripple] {
        let st = s.command(&json!({"op": "script_set", "target": {"name": "v", "to": target}}));
        assert_eq!(st["undo"], "Script target");
        assert!(st["script"]["targets"][1]["toName"].as_str().unwrap().contains('›'), "{}", st["script"]["targets"][1]["toName"]);
        let p = s.project();
        let keyed = value(p, &target, 3.0, false);
        assert!(keyed > 0.0);
        assert!((value(p, &target, 3.0, true) - (keyed + 10.0 + keyed)).abs() < 1e-3, "{target}");
    }
    // something that isn't a number can't be picked
    let st = s.command(&json!({"op": "script_set", "target": {"name": "v", "to": "L2/shown"}}));
    assert_eq!(st["message"], "A script changes numbers; pick a value that is one.");
}

#[test]
fn bake_turns_a_run_into_keys() {
    let mut s = sliding();
    let st = add(&mut s, "s6.jitter", "return { fig = { rotate = { root = t.random(-30, 30) } } }", 2, 7);
    let run = st["script"]["run"].as_str().unwrap().to_string();
    let rot = root_rotation_id(s.project());
    let scripted: Vec<f64> = (2..7).map(|f| value(s.project(), &rot, f as f64, true)).collect();
    let st = s.command(&json!({"op": "script_bake", "run": run}));
    assert_eq!(st["undo"], "Bake to keys");
    assert!(st["script"]["runs"].as_array().unwrap().is_empty(), "the run is gone");
    let p = s.project();
    let baked: Vec<f64> = (2..7).map(|f| value(p, &rot, f as f64, true)).collect();
    for (a, b) in scripted.iter().zip(&baked) {
        assert!((a - b).abs() < 1e-3, "{scripted:?} vs {baked:?}");
    }
    assert!((2..7).all(|f| p.property(&rot).unwrap().track.keys().iter().any(|k| (k.time * 8.0 - f as f64).abs() < 1e-6)), "a key on each frame");
    s.command(&json!({"op": "undo"}));
    assert_eq!(s.state()["script"]["runs"].as_array().unwrap().len(), 1);
}

#[test]
fn a_script_layer_saves_and_loads() {
    let mut s = sliding();
    let st = add(&mut s, "s7.save", "return { fig = { rotate = { root = 1 } } }", 1, 5);
    let run = st["script"]["run"].as_str().unwrap().to_string();
    s.command(&json!({"op": "script_run", "run": run, "replace": true, "keep": true}));
    s.command(&json!({"op": "script_set", "props": {"amount": 3.5}}));
    let text = codec::encode(s.project());
    assert!(text.contains("\"SCRIPT\""));
    let back = codec::decode(&text).unwrap();
    assert_eq!(codec::encode(&back), text);
    let a = s.project().root().unwrap().layers.iter().find(|l| matches!(l.kind, LayerKind::Script(_))).unwrap().clone();
    let b = back.root().unwrap().layers.iter().find(|l| matches!(l.kind, LayerKind::Script(_))).unwrap().clone();
    assert_eq!(a, b);
}

#[test]
fn a_failing_script_is_switched_off_and_lets_values_through() {
    let mut s = sliding();
    add(&mut s, "s8.stuck", "while true do end", 0, 4);
    let p = s.project();
    let rot = root_rotation_id(p);
    assert!((value(p, &rot, 1.0, true) - value(p, &rot, 1.0, false)).abs() < 1e-9);
    let st = s.state();
    assert_eq!(st["script"]["error"], "The plugin took too long and was stopped.");
    assert!(st["plugins"].as_array().unwrap().iter().any(|x| x["id"] == "s8.stuck" && !x["off"].is_null()));
    // a project whose script plugin isn't installed keeps it, doing nothing
    let mut p = s.project().clone();
    if let Some(LayerKind::Script(sc)) = p.root_mut().unwrap().layers.iter_mut().find(|l| matches!(l.kind, LayerKind::Script(_))).map(|l| &mut std::sync::Arc::make_mut(l).kind) {
        sc.plugin = "s8.nowhere".into();
    }
    let mut s2 = Session::new(Document::new(p));
    let lid = s2.project().root().unwrap().layers.iter().find(|l| matches!(l.kind, LayerKind::Script(_))).unwrap().id.clone();
    let st = s2.command(&json!({"op": "script_pick", "layer": lid}));
    assert_eq!(st["script"]["missing"], true);
    let row = st["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "script").cloned().unwrap();
    assert_eq!(row["missing"], true);
}
