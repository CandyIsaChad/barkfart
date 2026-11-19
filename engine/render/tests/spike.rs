use barkfart_core::demo_figure;
use barkfart_render::{Renderer, View};

#[test]
fn renders_the_demo_figure_on_this_gpu() {
    let r = match Renderer::headless_default() {
        Ok(r) => r,
        Err(e) => {
            // CI machines without a GPU: say so instead of failing silently
            eprintln!("skipping: {e}");
            return;
        }
    };
    eprintln!("adapter: {} via {} ({})", r.adapter.name, r.adapter.backend, r.adapter.driver);
    let (w, h) = (300u32, 300u32);
    let view = View { x0: -150.0, y0: -150.0, x1: 150.0, y1: 150.0, background: [1.0, 1.0, 1.0, 1.0] };
    let px = r.render(&demo_figure(0.25), w, h, view);
    assert_eq!(px.len(), (w * h * 4) as usize);
    let at = |x: u32, y: u32| {
        let o = ((y * w + x) * 4) as usize;
        [px[o], px[o + 1], px[o + 2], px[o + 3]]
    };
    assert_eq!(at(2, 2), [255, 255, 255, 255], "background");
    // head centre at (0, 75): pixel (150, 75) from the top
    assert_eq!(at(150, 75), [0, 0, 0, 255], "head");
    assert_eq!(at(150, 130), [0, 0, 0, 255], "body");
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("spike.png");
    let f = std::fs::File::create(&out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&px).unwrap();
    eprintln!("wrote {}", out.display());
}

#[test]
fn the_gl_backend_renders_too() {
    // cheap Android phones may only have GLES; make sure that path draws the same
    let r = match Renderer::headless(&[wgpu::Backends::GL]) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping GL: {e}");
            return;
        }
    };
    eprintln!("GL adapter: {} ({})", r.adapter.name, r.adapter.driver);
    let view = View { x0: -150.0, y0: -150.0, x1: 150.0, y1: 150.0, background: [1.0, 1.0, 1.0, 1.0] };
    let px = r.render(&demo_figure(0.25), 300, 300, view);
    let o = ((75 * 300 + 150) * 4) as usize;
    assert_eq!(&px[o..o + 4], &[0, 0, 0, 255]);
}

#[test]
#[ignore] // timing only: cargo test --release -p barkfart-render -- --ignored --nocapture
fn timing() {
    let r = Renderer::headless_default().unwrap();
    let view = View { x0: -150.0, y0: -150.0, x1: 150.0, y1: 150.0, background: [1.0, 1.0, 1.0, 1.0] };
    for (w, h) in [(1280u32, 685u32), (2048, 1536)] {
        r.render(&demo_figure(0.0), w, h, view);
        let t = std::time::Instant::now();
        for i in 0..20 {
            r.render(&demo_figure(i as f32 / 20.0), w, h, view);
        }
        eprintln!("{w}x{h}: {:.1} ms per frame", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
}
