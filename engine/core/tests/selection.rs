//! Regression tests for strict mutual-exclusion selection across Camera,
//! Script, Adjustment (Effect), Group, Figure, Textfield, Picture, and Movieclip.

use barkfart_core::edit::Document;
use barkfart_core::model::LayerKind;
use barkfart_core::plugins;
use barkfart_core::session::{new_project, Mode, Session};
use serde_json::{json, Value};

const DUMMY_SCRIPT: &str = r#"local bf = require("@barkfart")
return bf.script {
    id = "test.selection_script",
    name = "Selection Test Script",
    version = "1.0.0",
    targets = { fig = bf.figure() },
    step = function(s, p, t, scene)
    end,
}
"#;

fn assert_only_selected(st: &Value, expected: &str, expected_id: Option<&str>) {
    let mode = st["mode"].as_str().unwrap_or("");
    let active: Vec<(&str, Option<&str>)> = [
        ("camera", (mode == "camera").then_some(None)),
        ("figure", st["selected"].as_object().map(|m| m["layer"].as_str())),
        ("script", st["script"].as_object().map(|m| m["id"].as_str())),
        ("group", st["group"].as_object().map(|m| m["id"].as_str())),
        ("textfield", st["textfield"].as_object().map(|m| m["layer"].as_str())),
        ("picture", st["picture"].as_object().map(|m| m["layer"].as_str())),
        ("movieclip", st["movieclip"].as_object().map(|m| m["layer"].as_str())),
        ("adjustment", st["adjustment"].as_object().map(|m| m["layer"].as_str())),
    ]
    .into_iter()
    .filter_map(|(k, v)| v.map(|id| (k, id)))
    .collect();

    if expected == "none" {
        assert!(active.is_empty(), "expected no selection, got {active:?} (mode={mode})");
        assert!(st["multi"].is_null(), "expected multi to be null");
        return;
    }

    assert_eq!(
        active.len(),
        1,
        "expected exactly one active selection ({expected}), got {active:?} (mode={mode})"
    );
    assert_eq!(active[0].0, expected, "wrong active selection kind");
    if let Some(eid) = expected_id {
        assert_eq!(active[0].1, Some(eid), "wrong active selection id for {expected}");
    }
    if expected == "camera" {
        assert_eq!(mode, "camera");
    } else {
        assert_eq!(mode, "figure", "non-camera selection ({expected}) must exit Camera mode");
    }
    assert!(st["multi"].is_null(), "single selection must clear multi");
}

#[test]
fn camera_script_adjustment_group_figure_text_picture_selection_is_mutually_exclusive() {
    plugins::install(DUMMY_SCRIPT, "selection_script.luau").expect("install test script");
    let mut s = Session::new(Document::new(new_project()));

    // Build a project with: Figure (L2), second Figure, Textfield, Adjustment, Picture, Group, Script, Camera
    let fig1 = s.project().root().unwrap().layers.iter().find(|l| l.as_figure().is_some()).unwrap().id.clone();
    let st = s.command(&json!({"op": "add_figure", "kind": "ball"}));
    let fig2 = st["selected"]["layer"].as_str().unwrap().to_string();
    assert_only_selected(&st, "figure", Some(&fig2));

    let st = s.command(&json!({"op": "add_textfield"}));
    let text_id = st["textfield"]["layer"].as_str().unwrap().to_string();
    assert_only_selected(&st, "textfield", Some(&text_id));

    let st = s.command(&json!({"op": "add_adjustment", "name": "Grade"}));
    let adj_id = st["adjustment"]["layer"].as_str().unwrap().to_string();
    assert_only_selected(&st, "adjustment", Some(&adj_id));

    let pic_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/legacy/synthetic/picture.png");
    let st = s.import("picture.png", &std::fs::read(pic_path).unwrap());
    let pic_id = st["rows"].as_array().unwrap().iter().find(|r| r["kind"] == "picture").unwrap()["id"].as_str().unwrap().to_string();

    let st = s.command(&json!({"op": "group", "layers": [fig1.clone(), fig2.clone()], "name": "Heroes"}));
    let group_id = st["group"]["id"].as_str().unwrap().to_string();
    assert_only_selected(&st, "group", Some(&group_id));

    let st = s.command(&json!({"op": "script_add", "id": "test.selection_script"}));
    let script_id = st["script"]["id"].as_str().unwrap().to_string();
    assert_only_selected(&st, "script", Some(&script_id));

    // Adding camera clears script selection and enters Camera mode exclusively
    let st = s.command(&json!({"op": "add_camera"}));
    let cam_id = s.project().root().unwrap().layers.iter().find(|l| matches!(l.kind, LayerKind::Camera(_))).unwrap().id.clone();
    assert_only_selected(&st, "camera", None);
    assert_eq!(s.mode, Mode::Camera);
    assert!(s.selected_script.is_none());
    assert!(s.selected.is_none());
    assert!(s.selected_text.is_none());
    assert!(s.selected_group.is_none());

    // Exact reported user sequence: Camera -> Script (via script_pick)
    let st = s.command(&json!({"op": "script_pick", "layer": script_id}));
    assert_only_selected(&st, "script", Some(&script_id));
    assert_eq!(s.mode, Mode::Figure);

    // Script -> Camera (via mode: "camera")
    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    assert!(s.selected_script.is_none());

    // Camera -> Adjustment (via select)
    let st = s.command(&json!({"op": "select", "layer": adj_id}));
    assert_only_selected(&st, "adjustment", Some(&adj_id));

    // Adjustment -> Camera (via select on camera layer)
    let st = s.command(&json!({"op": "select", "layer": cam_id}));
    assert_only_selected(&st, "camera", None);

    // Camera -> Group (via select group)
    let st = s.command(&json!({"op": "select", "group": group_id}));
    assert_only_selected(&st, "group", Some(&group_id));

    // Group -> Camera -> Figure
    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "select", "layer": fig1}));
    assert_only_selected(&st, "figure", Some(&fig1));

    // Figure -> Script (via script_pick) -> Stage tap clears script cleanly
    let st = s.command(&json!({"op": "script_pick", "layer": script_id}));
    assert_only_selected(&st, "script", Some(&script_id));
    let st = s.command(&json!({"op": "down", "x": 5000.0, "y": 5000.0, "r": 10.0}));
    assert_only_selected(&st, "none", None);
    s.command(&json!({"op": "up"}));

    // Camera -> Script via generic select op
    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "select", "layer": script_id}));
    assert_only_selected(&st, "script", Some(&script_id));

    // Camera -> Textfield -> Camera -> Picture -> Camera -> cycle_figure -> Camera -> select_many
    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "select", "layer": text_id}));
    assert_only_selected(&st, "textfield", Some(&text_id));

    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "select", "layer": pic_id}));
    assert_only_selected(&st, "picture", Some(&pic_id));

    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "cycle_figure", "dir": 1}));
    assert_only_selected(&st, "figure", None);

    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
    let st = s.command(&json!({"op": "select_many", "layers": [fig1, fig2]}));
    assert_eq!(st["mode"], "figure");
    assert!(st["script"].is_null() && st["group"].is_null() && st["textfield"].is_null());
    assert_eq!(st["multi"].as_array().unwrap().len(), 2);

    // Multi -> Camera clears multi cleanly
    let st = s.command(&json!({"op": "mode", "v": "camera"}));
    assert_only_selected(&st, "camera", None);
}
