//! C interface to the engine for the Flutter app (dart:ffi). Every function
//! is safe to call with a null handle (it then does nothing) and never
//! unwinds across the boundary.

use barkfart_core::demo_figure;
use barkfart_render::{Renderer, View};
use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Engine {
    renderer: Renderer,
    label: Vec<u8>,
}

/// Creates the engine (GPU renderer). Returns null when no GPU device works.
#[no_mangle]
pub extern "C" fn bf_engine_new() -> *mut Engine {
    let made = catch_unwind(|| Renderer::headless_default().ok());
    match made {
        Ok(Some(renderer)) => {
            let a = &renderer.adapter;
            let mut label = format!("{} via {}", a.name, a.backend).into_bytes();
            label.push(0);
            Box::into_raw(Box::new(Engine { renderer, label }))
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

/// Renders the demo figure at time t into `out` (w * h * 4 bytes, straight
/// RGBA, rows top-down), fitting a 300 x 300 area. Returns 0 on success.
#[no_mangle]
pub extern "C" fn bf_render_demo(e: *const Engine, t: f32, w: u32, h: u32, out: *mut u8) -> i32 {
    if e.is_null() || out.is_null() || w == 0 || h == 0 || w > 8192 || h > 8192 {
        return 1;
    }
    let r = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: live handle
        let engine = unsafe { &*e };
        let aspect = w as f32 / h as f32;
        let (hw, hh) = if aspect >= 1.0 { (150.0 * aspect, 150.0) } else { (150.0, 150.0 / aspect) };
        let view = View { x0: -hw, y0: -hh, x1: hw, y1: hh, background: [1.0, 1.0, 1.0, 1.0] };
        engine.renderer.render(&demo_figure(t), w, h, view)
    }));
    match r {
        Ok(px) => {
            // SAFETY: caller provides w * h * 4 bytes
            unsafe { std::ptr::copy_nonoverlapping(px.as_ptr(), out, px.len()) };
            0
        }
        Err(_) => 2,
    }
}
