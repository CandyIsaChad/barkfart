//! GPU renderer against the CPU reference (barkfart_core::fx), which itself
//! matches the Java compositor. Same tolerance as the Java GL check that
//! passed on the iPad: mean channel difference under 2 and under 1% of
//! pixels off by more than 48 (premultiplied).

use barkfart_core::{codec, eval, fx, tess};
use barkfart_render::{Options, Renderer, View};
use std::path::PathBuf;

fn ref_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../testdata/ref")
}

fn cases() -> Vec<String> {
    let idx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(ref_dir().join("index.json")).unwrap()).unwrap();
    idx.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap().to_string()).collect()
}

/// Over the pixels either image covers: mean channel difference, and the
/// share of pixels off by more than 12 (edges differ by rasterization ties;
/// intermediate passes are 8-bit).
fn stats(gpu: &[u8], cpu: &[u8], n: usize) -> (f64, f64, usize) {
    let (mut sum, mut bad, mut covered) = (0u64, 0usize, 0usize);
    for i in 0..n {
        if gpu[i * 4 + 3] == 0 && cpu[i * 4 + 3] == 0 {
            continue;
        }
        covered += 1;
        let mut worst = 0u8;
        for c in 0..4 {
            let d = gpu[i * 4 + c].abs_diff(cpu[i * 4 + c]);
            sum += d as u64;
            worst = worst.max(d);
        }
        if worst > 12 {
            bad += 1;
        }
    }
    if covered == 0 {
        return (0.0, 0.0, 0);
    }
    (sum as f64 / (4.0 * covered as f64), bad as f64 / covered as f64, covered)
}

fn compare(gpu: &[u8], cpu: &[u8], n: usize) -> Option<String> {
    let (mean, bad, covered) = stats(gpu, cpu, n);
    if std::env::var("BARKFART_FXSTATS").is_ok() {
        eprintln!("  mean {mean:.3} bad {:.3}% covered {covered}", bad * 100.0);
    }
    if mean < MEAN && bad < BAD {
        None
    } else {
        Some(format!("mean {mean:.2} over {covered} covered pixels, {:.2}% off by more than 12", bad * 100.0))
    }
}

fn sampled_like_reference(x0: f32, y0: f32, w: usize, h: usize, scale: f32) -> View {
    View { x0, y0: y0 - h as f32 / scale, x1: x0 + w as f32 / scale, y1: y0, background: [0.0; 4] }
}

/// GPUs snap vertices to 1/256 px; snap the reference's input the same way
/// so an edge a hair from a pixel centre falls on the same side in both.
/// (The reference's tiny sample offset then reproduces the GPU's top-left rule.)
fn snapped(d: &barkfart_core::DrawList, x0: f32, y0: f32, scale: f32) -> barkfart_core::DrawList {
    let mut d = d.clone();
    for b in &mut d.batches {
        for i in 0..b.vertex_count() {
            let px = ((b.xy[i * 2] - x0) * scale * 256.0).round() / 256.0;
            let py = ((y0 - b.xy[i * 2 + 1]) * scale * 256.0).round() / 256.0;
            b.xy[i * 2] = x0 + px / scale;
            b.xy[i * 2 + 1] = y0 - py / scale;
        }
    }
    d
}

const MEAN: f64 = 0.6;
const BAD: f64 = 0.005;

fn check(backends: wgpu::Backends, label: &str) {
    let r = match Renderer::headless_with(&[backends], Options { samples: 1 }) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping {label}: {e}");
            return;
        }
    };
    eprintln!("{label}: {} via {}", r.adapter.name, r.adapter.backend);
    let mut failures = Vec::new();
    let mut checked = 0;
    for n in cases() {
        let dir = ref_dir().join(&n);
        let p = codec::decode(&std::fs::read_to_string(dir.join("project.json")).unwrap()).unwrap();
        let fx_case = n.starts_with("fx");
        let scale = if fx_case { 1.0 } else { 200.0 / p.settings.width as f32 };
        let (w, h) = if fx_case { (160usize, 180usize) } else { (200, ((p.settings.height as f32 * scale).round() as usize).max(1)) };
        let (x0, y0) = (-(w as f32) / 2.0 / scale, h as f32 / 2.0 / scale);
        for t in [0.0, 0.37, 1.1] {
            let s = eval::evaluate_with(&p, &p.root_symbol, t, false, true);
            let d = tess::layered(&s, None);
            let mut im = fx::Image::new(w, h, x0, y0, scale);
            fx::render(&snapped(&d, x0, y0, scale), &mut im);
            let cpu = im.to_premultiplied_u8();
            let gpu = r.render_raw(&d, w as u32, h as u32, sampled_like_reference(x0, y0, w, h, scale));
            checked += 1;
            if let Some(why) = compare(&gpu, &cpu, w * h) {
                failures.push(format!("{n} t={t}: {why}"));
            }
        }
    }
    eprintln!("{label}: {checked} frames compared");
    assert!(failures.is_empty(), "{label}: {} of {checked} frames differ:\n{}", failures.len(), failures.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn vulkan_or_metal_matches_the_reference() {
    check(wgpu::Backends::PRIMARY, "primary");
}

#[test]
fn gl_matches_the_reference() {
    check(wgpu::Backends::GL, "gl");
}

#[test]
#[ignore] // writes GPU and CPU images of one case: BARKFART_CASE=name
fn dump_case() {
    let n = std::env::var("BARKFART_CASE").unwrap();
    let r = Renderer::headless_with(&[wgpu::Backends::PRIMARY], Options { samples: 1 }).unwrap();
    let p = codec::decode(&std::fs::read_to_string(ref_dir().join(&n).join("project.json")).unwrap()).unwrap();
    let scale = if n.starts_with("fx") { 1.0 } else { 200.0 / p.settings.width as f32 };
    let (w, h) = if n.starts_with("fx") { (160usize, 180usize) } else { (200, ((p.settings.height as f32 * scale).round() as usize).max(1)) };
    let (x0, y0) = (-(w as f32) / 2.0 / scale, h as f32 / 2.0 / scale);
    let s = eval::evaluate_with(&p, &p.root_symbol, 0.0, false, true);
    let d = tess::layered(&s, None);
    let mut im = fx::Image::new(w, h, x0, y0, scale);
    fx::render(&d, &mut im);
    let view = sampled_like_reference(x0, y0, w, h, scale);
    for (name, px) in [("cpu", im.to_premultiplied_u8()), ("gpu", r.render_raw(&d, w as u32, h as u32, view))] {
        let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{n}-{name}.png"));
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out).unwrap()), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&px).unwrap();
        eprintln!("wrote {}", out.display());
    }
    for b in &d.batches {
        eprintln!("batch texture {:?} verts {} first rgba {:08x?}", b.texture, b.vertex_count(), b.rgba.first());
        for i in 0..b.vertex_count().min(6) {
            eprintln!("  px ({:.4}, {:.4})", (b.xy[i * 2] - x0) * scale, (y0 - b.xy[i * 2 + 1]) * scale);
        }
    }
}
