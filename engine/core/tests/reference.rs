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
                    ShapeKind::Text { text, font, size, xf, corner, align } => {
                        let mut o = json!({"text": text, "font": font, "size": size,
                            "xf": {"kind": "Affine", "a": xf.a, "b": xf.b, "c": xf.c, "d": xf.d, "tx": xf.tx, "ty": xf.ty}, "corner": corner});
                        // newer textfields carry their own line alignment; older text layers follow corner
                        if *align >= 0 {
                            o.as_object_mut().unwrap().insert("align".into(), json!(align));
                        }
                        o
                    }
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

/// The same stand-in glyphs as RefDumpTest.GLYPHS.
struct RefGlyphs;

impl barkfart_core::GlyphSource for RefGlyphs {
    fn base_px(&self) -> f32 {
        20.0
    }

    fn line_height(&self) -> f32 {
        24.0
    }

    fn glyph(&self, c: char) -> Option<barkfart_core::Glyph> {
        let g = |u0, v0, u1, v1, x, y, w, h, advance| barkfart_core::Glyph { texture: "glyphs:0".into(), u0, v0, u1, v1, x, y, w, h, advance };
        if c == ' ' {
            return Some(g(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 8.0));
        }
        let code = c as u32;
        if code > 126 {
            return None;
        }
        let (u, v) = ((code % 16) as f32 / 16.0, (code / 16) as f32 / 8.0);
        Some(g(u, v + 0.125, u + 0.0625, v, 1.0, -4.0, 10.0, 16.0, 12.0))
    }
}

fn draw_json(d: &barkfart_core::DrawList) -> J {
    let batches: Vec<J> = d
        .batches
        .iter()
        .map(|b| {
            json!({
                "blend": b.blend().name(),
                "effectGroup": b.effect_group,
                "texture": b.texture,
                "xy": floats(&b.xy),
                "uv": floats(&b.uv),
                "rgba": b.rgba.iter().map(|c| format!("{c:08x}")).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "batches": batches })
}

#[test]
fn triangles_match_the_java_tessellator() {
    let mut failures = Vec::new();
    let mut checked = 0;
    let mut vertices = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        for tag in ["0.0", "0.37", "1.1"] {
            let r = read_gz(&dir.join(format!("t{tag}.json.gz")));
            let t: f64 = tag.parse().unwrap();
            let s = eval::evaluate_with(&p, &p.root_symbol, t, false, true);
            let d = barkfart_core::tess::tessellate(&s, Some(&RefGlyphs));
            vertices += d.batches.iter().map(|b| b.vertex_count()).sum::<usize>();
            let got = draw_json(&d);
            let mut out = Vec::new();
            // coordinates within 1e-3 px relative; colours compare as text (exact)
            diff("", &r["draw"], &got, 1e-4, &mut out);
            checked += 1;
            if !out.is_empty() {
                failures.push(format!("{n} t={tag}:\n  {}", out.join("\n  ")));
            }
        }
    }
    eprintln!("{checked} draw lists, {vertices} vertices compared");
    assert!(failures.is_empty(), "{} of {checked} draw lists differ:\n{}", failures.len(), failures.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn the_locked_v1_fixture_loads_from_its_bark_file() {
    let path = ref_dir().join("../../../core/src/test/resources/formats/v1-kitchen-sink.bark");
    let b = barkfart_core::codec::read_bark(&std::fs::read(path).unwrap()).unwrap();
    let p = &b.project;
    assert_eq!(p.library.len(), 2);
    assert_eq!(p.root().unwrap().layers.len(), 7);
    let man = p.root().unwrap().layers[0].as_figure().unwrap();
    assert_eq!(man.node("armR1").unwrap().rotation.track.len(), 3);
    assert_eq!(man.polyfills.len(), 1);
    // same scene as the exported JSON copy
    let json = barkfart_core::codec::decode(&std::fs::read_to_string(ref_dir().join("kitchen-sink/project.json")).unwrap()).unwrap();
    let (a, c) = (eval::evaluate(p, &p.root_symbol, 0.6), eval::evaluate(&json, &json.root_symbol, 0.6));
    assert_eq!(a.root, c.root);
}

fn isolate_json(d: &barkfart_core::DrawList, i: usize) -> J {
    use barkfart_core::tess::IsoItem;
    let iso = &d.isolates[i];
    let effects: Vec<J> = group_json(&eval::Group { effects: iso.effects.clone(), ..eval::Group::new("") })["effects"].as_array().unwrap().clone();
    let items: Vec<J> = iso
        .items
        .iter()
        .map(|it| match it {
            IsoItem::Batch(b) => json!(b),
            IsoItem::Isolate(k) => isolate_json(d, *k),
        })
        .collect();
    json!({
        "source": iso.source, "opacity": iso.opacity, "blend": iso.blend.name(), "adjustment": iso.adjustment,
        "effects": effects,
        "bounds": if iso.empty() { J::Null } else { json!([iso.min_x, iso.min_y, iso.max_x, iso.max_y]) },
        "items": items,
    })
}

#[test]
fn isolate_trees_match_the_java_layered_tessellator() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        for tag in ["0.0", "0.37", "1.1"] {
            let r = read_gz(&dir.join(format!("t{tag}.json.gz")));
            let s = eval::evaluate_with(&p, &p.root_symbol, tag.parse().unwrap(), false, true);
            let d = barkfart_core::tess::layered(&s, Some(&RefGlyphs));
            let mut got = draw_json(&d);
            got["root"] = isolate_json(&d, 0);
            let mut out = Vec::new();
            diff("", &r["layered"], &got, 1e-4, &mut out);
            checked += 1;
            if !out.is_empty() {
                failures.push(format!("{n} t={tag}:\n  {}", out.join("\n  ")));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {checked} isolate trees differ:\n{}", failures.len(), failures.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

fn read_gz_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::io::Read;
    let mut v = Vec::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_end(&mut v).unwrap();
    v
}

#[test]
fn cpu_renders_match_the_java_compositor() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        for tag in ["0.0", "0.37", "1.1"] {
            let r = read_gz(&dir.join(format!("t{tag}.json.gz")));
            let cpu = &r["cpu"];
            let num = |k: &str| cpu[k].to_string().parse::<f64>().unwrap();
            let (w, h) = (num("w") as usize, num("h") as usize);
            let want = read_gz_bytes(&dir.join(format!("t{tag}.cpu.rgba.gz")));
            let s = eval::evaluate_with(&p, &p.root_symbol, tag.parse().unwrap(), false, true);
            let d = barkfart_core::tess::layered(&s, Some(&RefGlyphs));
            let mut im = barkfart_core::fx::Image::new(w, h, num("x0") as f32, num("y0") as f32, num("scale") as f32);
            barkfart_core::fx::render(&d, &mut im);
            let got = im.to_premultiplied_u8();
            let (mut worst, mut off) = (0u8, 0usize);
            for (a, b) in want.iter().zip(&got) {
                let dd = a.abs_diff(*b);
                worst = worst.max(dd);
                if dd > 1 {
                    off += 1;
                }
            }
            checked += 1;
            if off > 0 {
                failures.push(format!("{n} t={tag}: {off} channels off by more than 1 (worst {worst})"));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {checked} renders differ:\n{}", failures.len(), failures.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn onion_skins_match_the_java_ghosts() {
    let mut failures = Vec::new();
    let mut checked = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        for tag in ["0.0", "0.37", "1.1"] {
            let r = read_gz(&dir.join(format!("t{tag}.json.gz")));
            let d = barkfart_core::tess::OnionSkin::default().build_layered(&p, &p.root_symbol, tag.parse().unwrap());
            let mut got = draw_json(&d);
            got["root"] = isolate_json(&d, 0);
            let mut out = Vec::new();
            diff("", &r["onion"], &got, 1e-4, &mut out);
            checked += 1;
            if !out.is_empty() {
                failures.push(format!("{n} t={tag}:\n  {}", out.join("\n  ")));
            }
        }
    }
    assert!(failures.is_empty(), "{} of {checked} onion skins differ:\n{}", failures.len(), failures.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn bounced_soundtracks_match_the_java_mixer() {
    use barkfart_core::audio::{self, AudioPlan};
    use std::sync::Arc;
    let mono = Arc::new(audio::tone(440.0, 1.0, 22050, 0.5));
    let high = Arc::new(audio::tone(660.0, 0.6, 44100, 0.4));
    let sounds = |id: &str| match id {
        "tone440" => Some(mono.clone()),
        "tone660" => Some(high.clone()),
        _ => None,
    };
    let dir = ref_dir().join("audio");
    for i in 0..6 {
        let p = barkfart_core::codec::decode(&std::fs::read_to_string(dir.join(format!("audio-{i}.json"))).unwrap()).unwrap();
        let plan = AudioPlan::build(&p, p.root().unwrap(), &sounds);
        let want = read_gz_bytes(&dir.join(format!("audio-{i}.wav.gz")));
        let got = audio::bounce(&plan, 0.1 * i as f64, 2.5).unwrap_or_default();
        assert_eq!(want.len(), got.len(), "audio-{i}: length");
        if want.is_empty() {
            continue; // silent plan on both sides
        }
        let (mut worst, mut off) = (0i32, 0usize);
        for k in (44..want.len()).step_by(2) {
            let a = i16::from_le_bytes([want[k], want[k + 1]]) as i32;
            let b = i16::from_le_bytes([got[k], got[k + 1]]) as i32;
            worst = worst.max((a - b).abs());
            if (a - b).abs() > 1 {
                off += 1;
            }
        }
        assert!(off == 0, "audio-{i}: {off} samples off by more than 1 (worst {worst})");
        assert_eq!(&want[..44], &got[..44], "audio-{i}: header");
    }
}

#[test]
fn gif_bytes_match_the_java_writer() {
    let (gw, gh) = (37usize, 23usize);
    let mut g = barkfart_core::export::Gif::new(Vec::new(), gw as u16, gh as u16, 24.0, true).unwrap();
    for f in 0..7usize {
        let mut px = vec![0u8; gw * gh * 4];
        for y in 0..gh {
            for x in 0..gw {
                let o = (y * gw + x) * 4;
                px[o] = (x * 7 + f * 20) as u8;
                px[o + 1] = (y * 11) as u8;
                px[o + 2] = ((x * y + f * 31) & 0xff) as u8;
                px[o + 3] = if (x + y + f) % 5 == 0 { 60 } else { 255 };
            }
        }
        g.frame(&px).unwrap();
    }
    let got = g.finish().unwrap();
    let want = std::fs::read(ref_dir().join("export.gif")).unwrap();
    assert_eq!(got.len(), want.len());
    assert!(got == want, "GIF bytes differ from the Java writer");
}

#[test]
fn png_zip_frames_decode_to_the_pixels() {
    let px: Vec<u8> = (0..(5 * 3 * 4)).map(|i| (i * 13 % 256) as u8).collect();
    let mut z = barkfart_core::export::PngZip::new(std::io::Cursor::new(Vec::new()), 5, 3, true);
    z.frame(&px).unwrap();
    z.frame(&px).unwrap();
    let bytes = z.finish().unwrap().into_inner();
    let mut a = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(a.len(), 2);
    let mut f = a.by_name("frame_0002.png").unwrap();
    let mut b = Vec::new();
    std::io::Read::read_to_end(&mut f, &mut b).unwrap();
    let img = barkfart_core::image::decode(&b).unwrap();
    assert_eq!((img.w, img.h), (5, 3));
    assert_eq!(img.px, px);
}

#[test]
fn compressed_sounds_decode_to_the_tone() {
    // 0.5 s stereo 440 Hz sine made with ffmpeg (testdata/sounds)
    for ext in ["mp3", "m4a", "ogg", "flac"] {
        let b = std::fs::read(ref_dir().join(format!("../sounds/tone440.{ext}"))).unwrap();
        let p = barkfart_core::audio::decode_sound(&b).unwrap_or_else(|| panic!("{ext} did not decode"));
        assert_eq!(p.channels, 2, "{ext}");
        assert!((p.seconds() - 0.5).abs() < 0.06, "{ext}: {} s", p.seconds());
        // count rising zero crossings of the left channel over the middle 0.3 s
        let (a, b2) = ((0.1 * p.rate as f64) as usize, (0.4 * p.rate as f64) as usize);
        let mut crossings = 0;
        for i in a..b2 {
            if p.data[i * 2] < 0 && p.data[(i + 1) * 2] >= 0 {
                crossings += 1;
            }
        }
        assert!((crossings as i32 - 132).abs() <= 3, "{ext}: {crossings} cycles in 0.3 s (440 Hz is 132)");
    }
}

#[test]
fn saving_writes_the_same_text_as_the_java_version() {
    let mut differ = Vec::new();
    let mut n = 0;
    let mut files: Vec<std::path::PathBuf> = cases().iter().map(|c| ref_dir().join(c).join("project.json")).collect();
    for i in 0..6 {
        files.push(ref_dir().join(format!("audio/audio-{i}.json")));
    }
    for f in files {
        let json = std::fs::read_to_string(&f).unwrap();
        let p = barkfart_core::codec::decode(&json).unwrap();
        let back = barkfart_core::codec::encode(&p);
        n += 1;
        if back != json {
            let line = json.lines().zip(back.lines()).position(|(a, b)| a != b).unwrap_or(0);
            differ.push(format!(
                "{}: line {}\n  java: {:?}\n  rust: {:?}",
                f.display(),
                line + 1,
                json.lines().nth(line).unwrap_or(""),
                back.lines().nth(line).unwrap_or("")
            ));
        }
    }
    assert!(differ.is_empty(), "{} of {n} files differ:\n{}", differ.len(), differ.iter().take(6).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn bark_files_round_trip_with_their_assets() {
    let bytes = std::fs::read(ref_dir().join("sound-demo.bark")).unwrap();
    let b = barkfart_core::codec::read_bark(&bytes).unwrap();
    let out = barkfart_core::codec::write_bark(&b.project, &b.assets).unwrap();
    let again = barkfart_core::codec::read_bark(&out).unwrap();
    assert_eq!(barkfart_core::codec::encode(&again.project), barkfart_core::codec::encode(&b.project));
    assert_eq!(again.assets, b.assets);
}

#[test]
fn gif_decoding_matches_an_independent_decoder() {
    // our reference GIF (made by the Java writer) and the gif crate as a second opinion
    let bytes = std::fs::read(ref_dir().join("export.gif")).unwrap();
    let ours = barkfart_core::image::decode_gif(&bytes).unwrap();
    assert_eq!((ours.w, ours.h, ours.frames.len()), (37, 23, 7));
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut d = opts.read_info(std::io::Cursor::new(&bytes)).unwrap();
    let mut screen = vec![0u8; 37 * 23 * 4];
    let mut k = 0;
    while let Some(f) = d.read_next_frame().unwrap() {
        // full-canvas frames: copy with transparency like a browser
        for (i, px) in f.buffer.chunks_exact(4).enumerate() {
            let (x, y) = (f.left as usize + i % f.width as usize, f.top as usize + i / f.width as usize);
            if px[3] > 0 {
                screen[(y * 37 + x) * 4..(y * 37 + x) * 4 + 4].copy_from_slice(px);
            }
        }
        let a = &ours.frames[k].rgba;
        let same = a.chunks_exact(4).zip(screen.chunks_exact(4)).all(|(p, q)| p[3] == 0 || p == q);
        assert!(same, "frame {k} differs");
        assert!((ours.frames[k].delay - f.delay as f64 / 100.0).abs() < 1e-9 || f.delay <= 1);
        if f.dispose == gif::DisposalMethod::Background {
            screen.fill(0);
        }
        k += 1;
    }
    assert_eq!(k, 7);
    assert!(barkfart_core::image::decode_gif(b"GIF89a broken").is_none());
}
