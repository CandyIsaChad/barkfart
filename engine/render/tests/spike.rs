use barkfart_core::{codec, eval, tess};
use barkfart_render::{Renderer, View};

fn kitchen_sink() -> barkfart_core::model::Project {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../testdata/ref/kitchen-sink/project.json");
    codec::decode(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The GPU tests take turns: creating GL and Vulkan devices on two threads
/// at once crashes the Linux driver now and then (SIGSEGV in about one run
/// in four). The app only ever makes one renderer.
static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gpu() -> std::sync::MutexGuard<'static, ()> {
    GPU.lock().unwrap_or_else(|e| e.into_inner())
}

fn renderer(order: &[wgpu::Backends]) -> Option<Renderer> {
    match Renderer::headless(order) {
        Ok(r) => {
            eprintln!("adapter: {} via {} ({})", r.adapter.name, r.adapter.backend, r.adapter.driver);
            Some(r)
        }
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn shot(r: &Renderer, p: &barkfart_core::model::Project, t: f64, w: u32, h: u32) -> Vec<u8> {
    let s = eval::evaluate(p, &p.root_symbol, t);
    let d = if std::env::var("BARKFART_FLAT").is_ok() { tess::tessellate(&s, None) } else { tess::layered(&s, None) };
    let (hw, hh) = (p.settings.width as f32 / 2.0, p.settings.height as f32 / 2.0);
    let bg = p.settings.background;
    r.render(&d, w, h, View { x0: -hw, y0: -hh, x1: hw, y1: hh, background: [bg.r, bg.g, bg.b, bg.a] })
}

#[test]
fn renders_the_kitchen_sink_project() {
    let _gpu = gpu();
    let Some(r) = renderer(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL]) else { return };
    let p = kitchen_sink();
    let (w, h) = (640u32, 360u32);
    let px = shot(&r, &p, 0.6, w, h);
    assert_eq!(px.len(), (w * h * 4) as usize);
    // kitchen-sink has a full-frame 0.75-opacity invert adjustment layer over #101820 -> #b7b3af
    let corner = [183u8, 179, 175, 255];
    assert_eq!(&px[0..4], &corner, "corner shows background through kitchen-sink's 0.75 invert adjustment");
    let drawn = px.chunks_exact(4).filter(|c| *c != corner).count();
    assert!(drawn > 2000, "something is drawn: {drawn} pixels");
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("kitchen-sink.png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out).unwrap()), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px).unwrap();
    eprintln!("wrote {}", out.display());
}

#[test]
fn the_gl_backend_renders_the_same() {
    let _gpu = gpu();
    let (Some(gl), Some(vk)) = (renderer(&[wgpu::Backends::GL]), renderer(&[wgpu::Backends::PRIMARY])) else { return };
    let p = kitchen_sink();
    let (a, b) = (shot(&gl, &p, 0.6, 320, 180), shot(&vk, &p, 0.6, 320, 180));
    let differ = a.chunks_exact(4).zip(b.chunks_exact(4)).filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8)).count();
    assert!(differ < 320 * 180 / 200, "{differ} pixels differ between GL and Vulkan");
}

#[test]
#[ignore] // timing: cargo test --release -p barkfart-render -- --ignored --nocapture
fn timing() {
    let _gpu = gpu();
    let r = Renderer::headless_default().unwrap();
    let p = kitchen_sink();
    for (w, h) in [(1280u32, 720u32), (2048, 1536)] {
        shot(&r, &p, 0.0, w, h);
        let t = std::time::Instant::now();
        for i in 0..20 {
            shot(&r, &p, i as f64 / 20.0, w, h);
        }
        eprintln!("{w}x{h}: {:.1} ms per frame (evaluate + tessellate + render + readback)", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
}

#[test]
fn matches_the_locked_v1_reference_render() {
    // FormatLockTest: Raster.fill(tessellate(evaluate(p, root, 0.6)), 320, 240, white)
    let Some(r) = renderer(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL]) else { return };
    let p = kitchen_sink();
    let mut d = tess::tessellate(&eval::evaluate(&p, &p.root_symbol, 0.6), None);
    // that image was drawn with plain source-over for every batch (blend modes ignored)
    for b in &mut d.batches {
        b.blend = Some(barkfart_core::model::Blend::Normal);
    }
    let px = r.render(&d, 320, 240, View { x0: -160.0, y0: -120.0, x1: 160.0, y1: 120.0, background: [1.0, 1.0, 1.0, 1.0] });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../core/src/test/resources/formats/v1-kitchen-sink.png");
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = dec.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let ch = info.color_type.samples();
    assert_eq!((info.width, info.height), (320, 240));
    let mut bad = 0;
    for i in 0..(320 * 240) {
        let want = &buf[i * ch..i * ch + 3];
        let got = &px[i * 4..i * 4 + 3];
        if want.iter().zip(got).any(|(a, b)| a.abs_diff(*b) > 96) {
            bad += 1;
        }
    }
    // edges differ (the reference raster has no anti-aliasing); shapes must not
    assert!(bad < 320 * 240 / 50, "{bad} pixels differ from the v1 reference");
    eprintln!("{bad} of {} pixels differ (edges)", 320 * 240);
}

#[test]
#[ignore] // timing with gaps between frames, like 24 fps playback
fn timing_spaced() {
    let _gpu = gpu();
    let r = Renderer::headless_default().unwrap();
    let p = kitchen_sink();
    shot(&r, &p, 0.0, 1280, 720);
    for gap in [0u64, 10, 40] {
        let mut total = 0.0;
        for i in 0..15 {
            std::thread::sleep(std::time::Duration::from_millis(gap));
            let t = std::time::Instant::now();
            shot(&r, &p, i as f64 / 20.0, 1280, 720);
            total += t.elapsed().as_secs_f64();
        }
        eprintln!("gap {gap} ms: {:.1} ms per frame", total * 1000.0 / 15.0);
    }
}

#[test]
fn export_frames_are_supersampled_and_composited() {
    let _gpu = gpu();
    let Some(r) = renderer(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL]) else { return };
    let p = kitchen_sink();
    let d = tess::layered(&eval::evaluate(&p, &p.root_symbol, 0.6), None);
    let (hw, hh) = (p.settings.width as f32 / 2.0, p.settings.height as f32 / 2.0);
    let bg = p.settings.background;
    let view = View { x0: -hw, y0: -hh, x1: hw, y1: hh, background: [bg.r, bg.g, bg.b, bg.a] };
    let solid = r.render_export(&d, 320, 180, view, 2, false);
    assert!(solid.chunks_exact(4).all(|p| p[3] == 255), "opaque over the background");
    assert_eq!(&solid[0..3], &[183, 179, 175]);
    let clear = r.render_export(&d, 320, 180, view, 2, true);
    assert_eq!(clear[3], 0, "transparent corner");
    // smooth edges: some partly covered pixels
    let partial = clear.chunks_exact(4).filter(|p| p[3] > 10 && p[3] < 245).count();
    assert!(partial > 50, "anti-aliased edges: {partial}");
}
