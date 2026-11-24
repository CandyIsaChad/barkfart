//! K2: plugin effects on the GPU: a plugin's WGSL runs over the layer with
//! its settings; a shader that doesn't compile switches the plugin off (the
//! layer draws as it is), as does a missing plugin.

use barkfart_core::edit::Document;
use barkfart_core::session::{new_editor_project, Session};
use barkfart_core::{eval, tess};
use barkfart_render::{Options, Renderer, View};
use serde_json::json;

static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn effect(id: &str, body: &str) -> String {
    format!(
        r#"local bf = require("@barkfart")
return bf.effect {{
	id = "{id}", name = "Red", version = "1.0.0", author = "yeah",
	props = {{ amount = bf.number {{ default = 1, min = 0, max = 1 }}, tint = bf.colour {{ default = bf.rgb(1, 0, 0) }} }},
	shader = bf.wgsl([[
{body}
]]),
}}"#
    )
}

const RED: &str = "fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> {
    let c = layer(uv);
    return mix(c, vec4<f32>(u.tint.rgb * c.a, c.a), u.amount);
}";

/// (red, green, blue) summed over the covered pixels.
fn picture(r: &Renderer, s: &Session) -> (f64, f64, f64) {
    let p = s.project();
    let scene = eval::evaluate_with(p, &p.root_symbol, 0.0, false, false);
    let list = tess::layered(&scene, None);
    let px = r.render(&list, 96, 96, View { x0: -480.0, y0: -480.0, x1: 480.0, y1: 480.0, background: [0.0; 4] });
    let mut sum = (0.0, 0.0, 0.0);
    for q in px.chunks(4) {
        if q[3] > 0 {
            sum.0 += q[0] as f64;
            sum.1 += q[1] as f64;
            sum.2 += q[2] as f64;
        }
    }
    sum
}

fn session_with(id: &str, body: &str) -> Session {
    let mut s = Session::new(Document::new(new_editor_project()));
    let st = s.command(&json!({"op": "plugin_install", "name": "red.luau", "source": effect(id, body)}));
    assert!(st["message"].as_str().unwrap().ends_with("is ready."), "{}", st["message"]);
    s.command(&json!({"op": "select", "layer": "L2", "node": "armR1"}));
    // a green figure (the starter is black), so red shows the shader ran
    s.command(&json!({"op": "figure", "color": 0x20c040ffu32}));
    s
}

#[test]
fn a_plugin_effect_runs_its_shader_over_the_layer() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(r) = Renderer::headless_with(&[wgpu::Backends::PRIMARY], Options { samples: 1 }) else { return };
    let mut s = session_with("rt.red", RED);
    let plain = picture(&r, &s);
    assert!(plain.1 > 0.0, "the figure is green: {plain:?}");
    let st = s.command(&json!({"op": "plugin_effect", "id": "rt.red"}));
    assert_eq!(st["undo"], "Effects");
    assert_eq!(st["selected"]["pluginEffects"][0]["name"], "Red");
    let red = picture(&r, &s);
    assert!(red.0 > 0.0 && red.1 == 0.0 && red.2 == 0.0, "all red: {red:?}");
    // its settings reach the shader: half way back to the figure's colours
    let st = s.command(&json!({"op": "plugin_effect", "id": "rt.red", "props": {"amount": 0.5}}));
    assert_eq!(st["selected"]["pluginEffects"][0]["settings"][0]["value"], 0.5);
    let half = picture(&r, &s);
    assert!(half.1 > 0.0 && half.1 < plain.1, "half: {half:?} vs {plain:?}");
    // off, then removed: the figure as it was
    s.command(&json!({"op": "plugin_effect", "id": "rt.red", "enabled": false}));
    assert_eq!(picture(&r, &s), plain);
    let st = s.command(&json!({"op": "plugin_effect", "id": "rt.red", "remove": true}));
    assert_eq!(st["undo"], "Remove effect");
    assert_eq!(st["selected"]["pluginEffects"], json!([]));
    // missing: the layer passes through, the project keeps it
    s.command(&json!({"op": "undo"}));
    s.command(&json!({"op": "plugin_effect", "id": "rt.red", "enabled": true}));
    s.command(&json!({"op": "plugin_uninstall", "id": "rt.red"}));
    assert_eq!(picture(&r, &s), plain);
    let st = s.state();
    assert_eq!(st["selected"]["pluginEffects"][0]["missing"], true);
}

#[test]
fn a_shader_that_does_not_compile_switches_the_plugin_off() {
    let _gpu = GPU.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(r) = Renderer::headless_with(&[wgpu::Backends::PRIMARY], Options { samples: 1 }) else { return };
    let mut s = session_with("rt.broken", "fn effect(uv: vec2<f32>, u: Props) -> vec4<f32> { return nonsense(uv); }");
    let plain = picture(&r, &s);
    s.command(&json!({"op": "plugin_effect", "id": "rt.broken"}));
    assert_eq!(picture(&r, &s), plain, "the figure draws as it is");
    let st = s.state();
    let p = st["plugins"].as_array().unwrap().iter().find(|p| p["id"] == "rt.broken").unwrap().clone();
    let off = p["off"].as_str().expect("switched off");
    assert!(off.starts_with("Its shader didn't compile"), "{off}");
    assert_eq!(st["selected"]["pluginEffects"][0]["off"], p["off"]);
}
