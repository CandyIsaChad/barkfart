//! The editor through the C interface, with the app's screen <-> stage
//! mapping: a handle drawn at a pixel is picked by a touch at that pixel,
//! and the picture shows the figure there.

use barkfart_ffi::*;
use std::ffi::{CStr, CString};

fn cmd(e: *mut Engine, json: &str) -> serde_json::Value {
    let c = CString::new(json).unwrap();
    let p = bf_cmd(e, c.as_ptr());
    assert!(!p.is_null());
    serde_json::from_str(unsafe { CStr::from_ptr(p) }.to_str().unwrap()).unwrap()
}

/// The app's view: centre (cx, cy) in stage units, `s` pixels per unit, y up.
struct View {
    cx: f32,
    cy: f32,
    s: f32,
    w: u32,
    h: u32,
}

impl View {
    fn rect(&self) -> (f32, f32, f32, f32) {
        let (hw, hh) = (self.w as f32 / 2.0 / self.s, self.h as f32 / 2.0 / self.s);
        (self.cx - hw, self.cy - hh, self.cx + hw, self.cy + hh)
    }
    fn to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        let (x0, _, _, y1) = self.rect();
        ((x - x0) * self.s, (y1 - y) * self.s)
    }
    fn to_stage(&self, px: f32, py: f32) -> (f32, f32) {
        let (x0, _, _, y1) = self.rect();
        (x0 + px / self.s, y1 - py / self.s)
    }
}

#[test]
fn handles_touches_and_picture_agree() {
    let e = bf_engine_new();
    if e.is_null() {
        eprintln!("skipping: no GPU");
        return;
    }
    assert_eq!(bf_doc_new(e), 0);
    let v = View { cx: 30.0, cy: 20.0, s: 0.45, w: 500, h: 400 };
    let st = cmd(e, r#"{"op":"state"}"#);
    let handles = st["handles"].as_array().unwrap().clone();
    assert_eq!(handles.len(), 11);
    let (x0, y0, x1, y1) = v.rect();
    let mut px = vec![0u8; (v.w * v.h * 4) as usize];
    assert_eq!(bf_render_view(e, 0.0, v.w, v.h, x0, y0, x1, y1, 2, px.as_mut_ptr(), std::ptr::null_mut()), 0);
    let names = ["hip", "torso", "head", "armL1", "armL2", "armR1", "armR2", "legL1", "legL2", "legR1", "legR2"];
    for (i, h) in handles.iter().enumerate() {
        let (sx, sy) = v.to_screen(h[0].as_f64().unwrap() as f32, h[1].as_f64().unwrap() as f32);
        // a touch at the drawn handle (a few pixels off) picks that joint
        let (tx, ty) = v.to_stage(sx + 2.0, sy - 1.0);
        let r = 6.0 / v.s;
        let st = cmd(e, &format!(r#"{{"op":"down","x":{tx},"y":{ty},"r":{r}}}"#));
        cmd(e, r#"{"op":"up"}"#);
        assert_eq!(st["selected"]["node"], names[i], "touch at the {} handle", names[i]);
        // the picture has the black figure under every joint but the hip and head centre
        if names[i] != "hip" && names[i] != "head" {
            let (ix, iy) = (sx.round() as usize, sy.round() as usize);
            let k = (iy * v.w as usize + ix) * 4;
            assert!(px[k] < 100 && px[k + 1] < 100 && px[k + 2] < 100, "{} at ({ix}, {iy}) is not drawn: {:?}", names[i], &px[k..k + 4]);
        }
    }
    // drag the right hand through the interface: one step, undoable
    let h = &handles[6];
    let (sx, sy) = v.to_screen(h[0].as_f64().unwrap() as f32, h[1].as_f64().unwrap() as f32);
    let (tx, ty) = v.to_stage(sx, sy);
    cmd(e, &format!(r#"{{"op":"down","x":{tx},"y":{ty},"r":10}}"#));
    let (mx, my) = v.to_stage(sx + 30.0, sy - 20.0);
    cmd(e, &format!(r#"{{"op":"move","x":{mx},"y":{my}}}"#));
    let st = cmd(e, r#"{"op":"up"}"#);
    assert_eq!(st["undo"], "Pose node");
    let mut len = 0usize;
    let bytes = bf_save(e, &mut len);
    assert!(!bytes.is_null() && len > 100);
    let st = cmd(e, r#"{"op":"undo"}"#);
    assert_eq!(st["canUndo"], false);
    bf_engine_free(e);
}

#[test]
fn export_follows_tweening() {
    let e = bf_engine_new();
    if e.is_null() {
        eprintln!("skipping: no GPU");
        return;
    }
    bf_doc_new(e);
    cmd(e, r#"{"op":"insert_frame"}"#);
    let path = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("tween.gif");
    let c = CString::new(path.to_str().unwrap()).unwrap();
    let count = |e| {
        let job = bf_export_begin(e, 0, c.as_ptr(), 64, 36, 0, 0, 0);
        assert!(!job.is_null());
        let n = bf_export_count(job);
        bf_export_free(job);
        n
    };
    assert_eq!(count(e), 2, "2 frames at 10 fps, no tweening");
    cmd(e, r#"{"op":"animation","tweening":true,"tweened":5}"#);
    assert_eq!(count(e), 12, "0.2 s at 60 pictures a second");
    bf_engine_free(e);
}
