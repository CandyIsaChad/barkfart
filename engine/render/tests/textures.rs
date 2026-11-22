use barkfart_core::model::{Color, Layer, LayerKind, Project, Symbol};
use barkfart_core::{eval, tess};
use barkfart_render::glyphs::{GlyphAtlas, PAGE};
use barkfart_render::{Renderer, View};

/// The GPU tests take turns (two devices made at once can crash the Linux
/// driver; see spike.rs).
static GPU: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn gpu() -> std::sync::MutexGuard<'static, ()> {
    GPU.lock().unwrap_or_else(|e| e.into_inner())
}


fn project(l: Layer) -> Project {
    let mut p = Project { root_symbol: "S1".into(), ..Default::default() };
    let mut s = Symbol::new("S1", Some("root"), 1.0);
    s.layers.push(std::sync::Arc::new(l));
    p.add_symbol(s);
    p
}

fn px(img: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * w + x) * 4) as usize;
    [img[o], img[o + 1], img[o + 2], img[o + 3]]
}

#[test]
fn sprites_sample_their_image_and_missing_images_draw_plain() {
    let _gpu = gpu();
    let Ok(r) = Renderer::headless_default() else { return };
    let mut l = Layer::sprite("L1", Some("s"));
    if let LayerKind::Sprite(s) = &mut l.kind {
        s.asset = Some("img".into());
        s.width = 40.0;
        s.height = 20.0;
    }
    let p = project(l);
    let view = View { x0: -10.0, y0: -10.0, x1: 50.0, y1: 30.0, background: [0.0, 0.0, 0.0, 1.0] };
    let d = tess::layered(&eval::evaluate(&p, "S1", 0.0), None);
    // missing: plain white (the tint)
    let a = r.render(&d, 60, 40, view);
    assert_eq!(px(&a, 60, 20, 20), [255, 255, 255, 255]);
    // a 2x1 image: red left, blue right
    r.set_texture("img", 2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
    let b = r.render(&d, 60, 40, view);
    let (left, right) = (px(&b, 60, 15, 20), px(&b, 60, 45, 20));
    assert!(left[0] > 200 && left[2] < 60, "left is red: {left:?}");
    assert!(right[2] > 200 && right[0] < 60, "right is blue: {right:?}");
    assert_eq!(px(&b, 60, 5, 5), [0, 0, 0, 255], "outside: background");
    r.remove_texture("img");
    assert!(!r.has_texture("img"));
}

#[test]
fn text_draws_through_the_glyph_atlas() {
    let _gpu = gpu();
    let Ok(r) = Renderer::headless_default() else { return };
    let atlas = GlyphAtlas::new();
    r.set_texture(PAGE, atlas.width, atlas.height, &atlas.rgba);
    let mut l = Layer::text("L1", Some("t"));
    if let LayerKind::Text(t) = &mut l.kind {
        t.text = Some("HI".into());
        t.size = 64.0;
        t.color.value = barkfart_core::model::Value::C(Color::rgba8888(0xff0000ff));
    }
    let p = project(l);
    let d = tess::layered(&eval::evaluate(&p, "S1", 0.0), Some(&atlas));
    assert!(d.batches.iter().any(|b| b.texture.as_deref() == Some(PAGE)), "glyph quads");
    let view = View { x0: -100.0, y0: -50.0, x1: 100.0, y1: 50.0, background: [1.0, 1.0, 1.0, 1.0] };
    let img = r.render(&d, 200, 100, view);
    let red = img.chunks_exact(4).filter(|c| c[0] > 200 && c[1] < 80 && c[2] < 80).count();
    assert!(red > 300, "text pixels: {red}");
    // nothing far from the text
    assert_eq!(px(&img, 200, 2, 2), [255, 255, 255, 255]);
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("text.png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&out).unwrap()), 200, 100);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
}

/// Text in a bundled or imported font uses that font's glyph page; the
/// default font stays on page 0, and characters a font lacks come from it.
#[test]
fn text_uses_the_textfields_font() {
    use barkfart_core::GlyphSource;
    let mut atlas = GlyphAtlas::new();
    assert!(atlas.add_bundled("Bangers").is_some());
    assert!(atlas.add_bundled("No Such Font").is_none());
    let lobster = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/Lobster-Regular.ttf")).unwrap();
    let page = atlas.add_font("A9", &lobster).expect("a usable font");
    assert_eq!(page.id, "glyphs:A9");
    assert!(atlas.add_font("A10", b"not a font").is_none());
    assert_eq!(atlas.glyph_in(None, 'A').unwrap().texture, "glyphs:0");
    assert_eq!(atlas.glyph_in(Some("Bangers"), 'A').unwrap().texture, "glyphs:Bangers");
    assert_eq!(atlas.glyph_in(Some("A9"), 'A').unwrap().texture, "glyphs:A9");
    assert_eq!(atlas.glyph_in(Some("unknown"), 'A').unwrap().texture, "glyphs:0", "unknown fonts fall back");
    assert_ne!(atlas.glyph_in(Some("Bangers"), 'W').unwrap().advance, atlas.glyph_in(None, 'W').unwrap().advance);
}
