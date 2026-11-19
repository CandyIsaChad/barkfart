//! Checks against reference data exported from the Java version
//! (engine/testdata/ref, made by core/src/test/java/app/barkfart/RefDumpTest.java).

use std::path::PathBuf;

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/ref")
}

fn cases() -> Vec<String> {
    let idx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(ref_dir().join("index.json")).unwrap()).unwrap();
    idx.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn every_reference_project_loads() {
    let names = cases();
    assert!(names.len() >= 80);
    for n in &names {
        let json = std::fs::read_to_string(ref_dir().join(n).join("project.json")).unwrap();
        let p = barkfart_core::codec::decode(&json).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert!(p.root().is_some(), "{n}");
    }
}

use barkfart_core::eval::{self, Item, ParamValue, Scene, ShapeKind};
use barkfart_core::model::Color;
use serde_json::{json, Value as J};

fn color_json(c: &Color) -> J {
    json!({"kind": "Color", "r": c.r, "g": c.g, "b": c.b, "a": c.a})
}

fn floats(v: &[f32]) -> J {
    J::Array(v.iter().map(|x| json!(x)).collect())
}

/// The scene in the same shape as the Java reference dump.
fn scene_json(s: &Scene) -> J {
    json!({
        "primitives": s.primitives,
        "diagnostics": s.diagnostics.iter().map(|d| format!("{} {}", d.kind.name(), d.subject)).collect::<Vec<_>>(),
        "root": group_json(&s.root),
    })
}

fn group_json(g: &eval::Group) -> J {
    let effects: Vec<J> = g
        .effects
        .iter()
        .map(|e| {
            let mut params = serde_json::Map::new();
            for (k, v) in &e.params {
                params.insert(
                    k.clone(),
                    match v {
                        ParamValue::F(x) => json!(x),
                        ParamValue::D(x) => json!(x),
                        ParamValue::C(c) => color_json(c),
                        ParamValue::B(b) => json!(b),
                    },
                );
            }
            json!({"kind": "ResolvedEffect", "type": e.ty, "mix": e.mix, "params": params})
        })
        .collect();
    let children: Vec<J> = g
        .children
        .iter()
        .map(|c| match c {
            Item::Group(x) => group_json(x),
            Item::Shape(s) => {
                let grad = s.gradient.map(|g| {
                    json!({"kind": "Gradient", "radial": g.radial, "x0": g.x0, "y0": g.y0, "x1": g.x1, "y1": g.y1, "r2": g.r2, "end": color_json(&g.end)})
                });
                let mut m = serde_json::Map::new();
                let kind = match &s.kind {
                    ShapeKind::Line { .. } => "Line",
                    ShapeKind::Circle { .. } => "Circle",
                    ShapeKind::Ring { .. } => "Ring",
                    ShapeKind::Polygon { .. } => "Polygon",
                    ShapeKind::Quad { .. } => "Quad",
                    ShapeKind::Text { .. } => "Text",
                };
                m.insert("kind".into(), json!(kind));
                m.insert("color".into(), color_json(&s.color));
                m.insert("source".into(), json!(s.source));
                m.insert("gradient".into(), grad.unwrap_or(J::Null));
                let extra = match &s.kind {
                    ShapeKind::Line { x1, y1, x2, y2, width, bend } => json!({"x1": x1, "y1": y1, "x2": x2, "y2": y2, "width": width, "bend": bend}),
                    ShapeKind::Circle { cx, cy, r, start, span } => json!({"cx": cx, "cy": cy, "r": r, "start": start, "span": span}),
                    ShapeKind::Ring { cx, cy, r, width, start, span } => {
                        json!({"cx": cx, "cy": cy, "r": r, "width": width, "start": start, "span": span})
                    }
                    ShapeKind::Polygon { points, convex } => json!({"points": floats(points), "convex": convex}),
                    ShapeKind::Quad { asset, corners } => json!({"asset": asset, "corners": floats(corners)}),
                    ShapeKind::Text { text, font, size, xf, corner } => json!({"text": text, "font": font, "size": size,
                        "xf": {"kind": "Affine", "a": xf.a, "b": xf.b, "c": xf.c, "d": xf.d, "tx": xf.tx, "ty": xf.ty}, "corner": corner}),
                };
                for (k, v) in extra.as_object().unwrap() {
                    m.insert(k.clone(), v.clone());
                }
                J::Object(m)
            }
        })
        .collect();
    json!({"kind": "Group", "source": g.source, "opacity": g.opacity, "blend": g.blend.name(),
        "adjustment": g.adjustment, "effects": effects, "children": children})
}

/// Compares JSON trees: same keys and strings, numbers within tolerance.
fn diff(path: &str, want: &J, got: &J, tol: f64, out: &mut Vec<String>) {
    if out.len() > 8 {
        return;
    }
    match (want, got) {
        (J::Number(a), J::Number(b)) => {
            let (a, b) = (a.to_string().parse::<f64>().unwrap(), b.to_string().parse::<f64>().unwrap());
            let scale = 1f64.max(a.abs()).max(b.abs());
            if !((a - b).abs() <= tol * scale || (a.is_nan() && b.is_nan())) {
                out.push(format!("{path}: want {a} got {b}"));
            }
        }
        (J::Object(a), J::Object(b)) => {
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => diff(&format!("{path}.{k}"), va, vb, tol, out),
                    None => out.push(format!("{path}.{k}: missing")),
                }
            }
            for k in b.keys() {
                if !a.contains_key(k) {
                    out.push(format!("{path}.{k}: extra"));
                }
            }
        }
        (J::Array(a), J::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{path}: length want {} got {}", a.len(), b.len()));
                return;
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, tol, out);
            }
        }
        _ => {
            if want != got {
                out.push(format!("{path}: want {want} got {got}"));
            }
        }
    }
}

fn read_gz(path: &std::path::Path) -> J {
    use std::io::Read;
    let mut s = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut s).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[test]
fn scenes_match_the_java_evaluator() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        for tag in ["0.0", "0.37", "1.1"] {
            let r = read_gz(&dir.join(format!("t{tag}.json.gz")));
            let t: f64 = tag.parse().unwrap();
            for (key, camera) in [("scene", true), ("stageScene", false)] {
                let got = scene_json(&eval::evaluate_with(&p, &p.root_symbol, t, false, camera));
                let mut out = Vec::new();
                diff("", &r[key], &got, 1e-4, &mut out);
                checked += 1;
                if !out.is_empty() {
                    failures.push(format!("{n} t={tag} {key}:\n  {}", out.join("\n  ")));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{} of {checked} scenes differ:\n{}", failures.len(), failures.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}
