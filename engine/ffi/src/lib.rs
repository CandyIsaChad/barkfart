//! C interface to the engine for the Flutter app (dart:ffi). Every function
//! is safe to call with a null handle (it then does nothing) and never
//! unwinds across the boundary.

use barkfart_core::model::Project;
use barkfart_core::{codec, eval, tess};
use barkfart_render::glyphs::{GlyphAtlas, PAGE};
use barkfart_render::{Renderer, View};
use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Engine {
    renderer: Renderer,
    glyphs: GlyphAtlas,
    /// Image ids uploaded for the current project.
    images: Vec<String>,
    label: Vec<u8>,
    project: Option<Project>,
    error: Vec<u8>,
}

impl Engine {
    fn set_error(&mut self, e: impl std::fmt::Display) {
        self.error = format!("{e}").into_bytes();
        self.error.push(0);
    }
}

/// Creates the engine (GPU renderer). Returns null when no GPU device works.
#[no_mangle]
pub extern "C" fn bf_engine_new() -> *mut Engine {
    match catch_unwind(|| Renderer::headless_default().ok()) {
        Ok(Some(renderer)) => {
            let a = &renderer.adapter;
            let mut label = format!("{} via {}", a.name, a.backend).into_bytes();
            label.push(0);
            let glyphs = GlyphAtlas::new();
            renderer.set_texture(PAGE, glyphs.width, glyphs.height, &glyphs.rgba);
            Box::into_raw(Box::new(Engine { renderer, glyphs, images: Vec::new(), label, project: None, error: vec![0] }))
        }
        _ => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn bf_engine_free(e: *mut Engine) {
    if !e.is_null() {
        // SAFETY: made by bf_engine_new and freed once by the caller
        drop(unsafe { Box::from_raw(e) });
    }
}

/// "GPU name via backend", NUL-terminated, valid until the engine is freed.
#[no_mangle]
pub extern "C" fn bf_engine_adapter(e: *const Engine) -> *const c_char {
    if e.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    unsafe { (*e).label.as_ptr() as *const c_char }
}

/// The last error message ("" when none), valid until the next call.
#[no_mangle]
pub extern "C" fn bf_engine_error(e: *const Engine) -> *const c_char {
    if e.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    unsafe { (*e).error.as_ptr() as *const c_char }
}

/// Loads a project from a .bark file (or plain project.json) in memory.
/// Returns 0 on success; the error is in bf_engine_error.
#[no_mangle]
pub extern "C" fn bf_project_load(e: *mut Engine, data: *const u8, len: usize) -> i32 {
    if e.is_null() || data.is_null() {
        return 1;
    }
    // SAFETY: live handle; the caller passes len readable bytes
    let (engine, bytes) = unsafe { (&mut *e, std::slice::from_raw_parts(data, len)) };
    let r = catch_unwind(AssertUnwindSafe(|| {
        if bytes.starts_with(b"PK") {
            codec::read_bark(bytes).map(|b| (b.project, b.assets))
        } else {
            codec::decode(&String::from_utf8_lossy(bytes)).map(|p| (p, Default::default()))
        }
    }));
    match r {
        Ok(Ok((p, entries))) => {
            for id in engine.images.drain(..) {
                engine.renderer.remove_texture(&id);
            }
            // sprite images (anything missing or unreadable draws in plain colour)
            for id in p.assets.keys() {
                if let Some(img) = barkfart_core::image::asset_bytes(&p, &entries, id).and_then(barkfart_core::image::decode) {
                    engine.renderer.set_texture(id, img.w, img.h, &img.px);
                    engine.images.push(id.clone());
                }
            }
            engine.project = Some(p);
            engine.set_error("");
            0
        }
        Ok(Err(err)) => {
            engine.set_error(err);
            2
        }
        Err(_) => {
            engine.set_error("the project could not be read (internal error)");
            3
        }
    }
}

/// Project facts into out[0..4]: duration (s), fps, width, height. 0 = ok.
#[no_mangle]
pub extern "C" fn bf_project_info(e: *const Engine, out: *mut f64) -> i32 {
    if e.is_null() || out.is_null() {
        return 1;
    }
    // SAFETY: live handle
    let engine = unsafe { &*e };
    let Some(p) = &engine.project else { return 2 };
    let duration = p.root().map(|s| s.duration).unwrap_or(0.0);
    let v = [duration, p.settings.fps as f64, p.settings.width as f64, p.settings.height as f64];
    // SAFETY: the caller provides 4 doubles
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), out, 4) };
    0
}

/// Renders the loaded project's shot at time t into `out` (w * h * 4 bytes,
/// straight RGBA, rows top-down), the stage fitted inside. Writes the
/// evaluate + tessellate + render time in microseconds to `micros` when not
/// null. Returns 0 on success.
#[no_mangle]
pub extern "C" fn bf_render_frame(e: *const Engine, t: f64, w: u32, h: u32, out: *mut u8, micros: *mut u32) -> i32 {
    if e.is_null() || out.is_null() || w == 0 || h == 0 || w > 8192 || h > 8192 {
        return 1;
    }
    // SAFETY: live handle
    let engine = unsafe { &*e };
    let Some(p) = &engine.project else { return 2 };
    let r = catch_unwind(AssertUnwindSafe(|| {
        let start = std::time::Instant::now();
        let scene = eval::evaluate(p, &p.root_symbol, t);
        // isolates: effects, adjustment layers, translucent groups drawn as one piece
        let list = tess::layered(&scene, Some(&engine.glyphs));
        let (sw, sh) = (p.settings.width.max(1) as f32, p.settings.height.max(1) as f32);
        let aspect = w as f32 / h as f32;
        let (hw, hh) = if aspect >= sw / sh { (sh * aspect / 2.0, sh / 2.0) } else { (sw / 2.0, sw / aspect / 2.0) };
        let bg = p.settings.background;
        let view = View { x0: -hw, y0: -hh, x1: hw, y1: hh, background: [bg.r, bg.g, bg.b, bg.a] };
        let px = engine.renderer.render(&list, w, h, view);
        (px, start.elapsed().as_micros().min(u32::MAX as u128) as u32)
    }));
    match r {
        Ok((px, us)) => {
            // SAFETY: caller provides w * h * 4 bytes (and a u32 when micros is not null)
            unsafe {
                std::ptr::copy_nonoverlapping(px.as_ptr(), out, px.len());
                if !micros.is_null() {
                    *micros = us;
                }
            }
            0
        }
        Err(_) => 3,
    }
}
