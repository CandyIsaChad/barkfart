//! C interface to the engine for the Flutter app (dart:ffi). Every function
//! is safe to call with a null handle (it then does nothing) and never
//! unwinds across the boundary.

use barkfart_core::edit::Document;
use barkfart_core::model::Project;
use barkfart_core::session::Session;
use barkfart_core::{codec, eval, tess};
use barkfart_render::glyphs::{GlyphAtlas, PAGE};
use barkfart_render::{Renderer, View};
use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub struct Engine {
    renderer: Renderer,
    /// Decoded sounds of the current project (for the soundtrack export).
    plan: Option<barkfart_core::audio::AudioPlan>,
    /// Frame delays and repeats in wall time (rebuilt with the plan).
    schedule: Option<barkfart_core::timing::Schedule>,
    player: barkfart_audio::Player,
    audio_label: Vec<u8>,
    glyphs: GlyphAtlas,
    /// Image ids uploaded for the current project.
    images: Vec<String>,
    label: Vec<u8>,
    /// The open project with its history, selection and gestures.
    session: Option<Session>,
    /// Decoded sounds by asset id (the plan is rebuilt from them after edits).
    sounds: std::collections::HashMap<String, std::sync::Arc<barkfart_core::audio::Pcm>>,
    plan_rev: u64,
    /// The last command's answer (NUL-terminated JSON) and the last saved file.
    answer: Vec<u8>,
    saved: Vec<u8>,
    error: Vec<u8>,
}

impl Engine {
    fn project(&self) -> Option<&Project> {
        self.session.as_ref().map(|s| s.project())
    }

    /// Opens a project: textures, sounds, a fresh session.
    fn open(&mut self, p: Project, entries: indexmap::IndexMap<String, Vec<u8>>) {
        for id in self.images.drain(..) {
            self.renderer.remove_texture(&id);
        }
        // sprite images (anything missing or unreadable draws in plain colour)
        for id in p.assets.keys() {
            if let Some(img) = barkfart_core::image::asset_bytes(&p, &entries, id).and_then(barkfart_core::image::decode) {
                self.renderer.set_texture(id, img.w, img.h, &img.px);
                self.images.push(id.clone());
            }
        }
        // sounds: decoded once
        self.sounds.clear();
        for (id, a) in &p.assets {
            if a.kind != barkfart_core::model::AssetKind::Audio {
                continue;
            }
            if let Some(pcm) = barkfart_core::image::asset_bytes(&p, &entries, id).and_then(barkfart_core::audio::decode_sound) {
                self.sounds.insert(id.clone(), std::sync::Arc::new(pcm));
            }
        }
        let assets = entries.into_iter().map(|(k, v)| (k, std::sync::Arc::new(v))).collect();
        self.session = Some(Session::new(Document::with_assets(p, assets)));
        self.player.stop();
        self.plan_rev = 0;
        self.refresh_plan();
        self.set_error("");
    }

    /// Rebuilds what the audio thread plays when the project changed.
    fn refresh_plan(&mut self) {
        let Some(s) = &self.session else { return };
        if s.rev() == self.plan_rev && self.plan.is_some() {
            return;
        }
        let p = s.project();
        // sounds imported since the project was opened
        for (id, a) in &p.assets {
            if a.kind == barkfart_core::model::AssetKind::Audio && !self.sounds.contains_key(id) {
                if let Some(pcm) = s.asset_bytes(&a.path).and_then(|b| barkfart_core::audio::decode_sound(&b)) {
                    self.sounds.insert(id.clone(), std::sync::Arc::new(pcm));
                }
            }
        }
        if let Some(root) = p.root() {
            let sounds = &self.sounds;
            let plan = barkfart_core::audio::AudioPlan::build(p, root, &|id: &str| sounds.get(id).cloned());
            // the player runs in wall time: delays and repeats move the sounds
            let schedule = barkfart_core::timing::Schedule::build(p);
            self.player.set_plan(schedule.retime(&plan));
            self.schedule = Some(schedule);
            self.plan = Some(plan);
        }
        self.plan_rev = s.rev();
    }
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
            let player = barkfart_audio::Player::new();
            let mut audio_label = if player.available() { format!("{} at {} Hz", player.device, player.rate) } else { player.device.clone() }.into_bytes();
            audio_label.push(0);
            Box::into_raw(Box::new(Engine {
                renderer,
                plan: None,
                schedule: None,
                player,
                audio_label,
                glyphs,
                images: Vec::new(),
                label,
                session: None,
                sounds: Default::default(),
                plan_rev: 0,
                answer: vec![0],
                saved: Vec::new(),
                error: vec![0],
            }))
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
            engine.open(p, entries);
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
    let Some(p) = engine.project() else { return 2 };
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
    let Some(p) = engine.project() else { return 2 };
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

/// "device at rate Hz", or why there is no sound output.
#[no_mangle]
pub extern "C" fn bf_audio_device(e: *const Engine) -> *const c_char {
    if e.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    unsafe { (*e).audio_label.as_ptr() as *const c_char }
}

/// Plays the project's sound from time t, looping [loop_start, loop_end) when loop_end > loop_start.
#[no_mangle]
pub extern "C" fn bf_play(e: *const Engine, t: f64, loop_start: f64, loop_end: f64) {
    if e.is_null() {
        return;
    }
    // SAFETY: live handle; the app calls in from one thread
    let engine = unsafe { &mut *(e as *mut Engine) };
    engine.refresh_plan();
    engine.player.play(t, loop_start, loop_end);
}

#[no_mangle]
pub extern "C" fn bf_stop(e: *const Engine) {
    if e.is_null() {
        return;
    }
    // SAFETY: live handle
    unsafe { (*e).player.stop() };
}

/// Wall seconds of one pass of the animation (frame delays and repeats included).
#[no_mangle]
pub extern "C" fn bf_wall_total(e: *const Engine) -> f64 {
    with_schedule(e, |s| s.total)
}

/// The project time shown at wall time w, one picture per 1/rate seconds.
#[no_mangle]
pub extern "C" fn bf_wall_time(e: *const Engine, w: f64, rate: f64) -> f64 {
    with_schedule(e, |s| s.time_at(w, rate))
}

/// The wall time a frame first plays at.
#[no_mangle]
pub extern "C" fn bf_frame_wall(e: *const Engine, frame: i32) -> f64 {
    with_schedule(e, |s| s.wall_of(frame))
}

fn with_schedule(e: *const Engine, f: impl FnOnce(&barkfart_core::timing::Schedule) -> f64) -> f64 {
    if e.is_null() {
        return 0.0;
    }
    // SAFETY: live handle; the app calls in from one thread
    let engine = unsafe { &mut *(e as *mut Engine) };
    engine.refresh_plan();
    engine.schedule.as_ref().map(f).unwrap_or(0.0)
}

/// The time the sound is playing (the picture follows it), or -1 when
/// stopped or when there is no sound output (the caller keeps its own clock).
#[no_mangle]
pub extern "C" fn bf_playback_time(e: *const Engine) -> f64 {
    if e.is_null() {
        return -1.0;
    }
    // SAFETY: live handle
    let engine = unsafe { &*e };
    if !engine.player.available() {
        return -1.0;
    }
    engine.player.position().unwrap_or(-1.0)
}

// ---------------------------------------------------------------- export

enum Writer {
    Gif(barkfart_core::export::Gif<std::io::BufWriter<std::fs::File>>),
    Png(barkfart_core::export::PngZip<std::io::BufWriter<std::fs::File>>),
}

/// An export in progress: bf_export_step renders and writes one frame.
pub struct Export {
    writer: Option<Writer>,
    path: std::path::PathBuf,
    w: u32,
    h: u32,
    start: u32,
    count: u32,
    next: u32,
    transparent: bool,
    error: Vec<u8>,
    /// (time, extra seconds) per picture when frames pause or repeat.
    pics: Option<Vec<(f64, f64)>>,
}

fn cstr(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the caller passes a NUL-terminated string
    unsafe { std::ffi::CStr::from_ptr(p) }.to_str().ok().map(str::to_string)
}

/// Starts exporting the loaded project to `path`: format 0 = GIF, 1 = PNG
/// frames in a ZIP. Frames [start, start + count) at the project fps
/// (count 0 = to the end). Returns null on failure (see bf_engine_error).
#[no_mangle]
pub extern "C" fn bf_export_begin(e: *mut Engine, format: i32, path: *const c_char, w: u32, h: u32, start: u32, count: u32, transparent: i32) -> *mut Export {
    if e.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    let r = (|| -> Result<Export, String> {
        let p = engine.project().ok_or("no project")?;
        let path = std::path::PathBuf::from(cstr(path).ok_or("no file name")?);
        if w == 0 || h == 0 || w > 4096 || h > 4096 {
            return Err(format!("size {w}x{h} is not allowed"));
        }
        // pictures per second: the frame rate, or more with tweening on;
        // start and count are in frames, converted to pictures
        let fps = p.settings.fps.max(1.0) as f64;
        let rate = p.settings.sample_rate();
        let per = rate / fps;
        let total = ((p.root().map(|s| s.duration).unwrap_or(0.0) * rate).round() as u32).max(1);
        // frame delays and repeats: the pictures follow the playing schedule
        let sch = barkfart_core::timing::Schedule::build(p);
        let pics = if sch.identity {
            None
        } else {
            let frames = barkfart_core::timing::frame_count(p);
            let first = (start as i32).clamp(0, frames - 1);
            let n = if count == 0 { frames - first } else { (count as i32).clamp(1, frames - first) };
            Some(barkfart_core::export::pictures(&sch, first, n, rate, format == 1))
        };
        let start = ((start as f64 * per).round() as u32).min(total - 1);
        let count = if count == 0 { total - start } else { ((count as f64 * per).round() as u32).max(1).min(total - start) };
        let count = pics.as_ref().map(|v| v.len() as u32).unwrap_or(count);
        let fps = rate;
        let file = std::io::BufWriter::new(std::fs::File::create(&path).map_err(|e| e.to_string())?);
        let writer = match format {
            0 => Writer::Gif(barkfart_core::export::Gif::new(file, w as u16, h as u16, fps, true).map_err(|e| e.to_string())?),
            1 => Writer::Png(barkfart_core::export::PngZip::new(file, w, h, transparent != 0)),
            _ => return Err(format!("unknown export format {format}")),
        };
        Ok(Export { writer: Some(writer), path, w, h, start, count, next: 0, transparent: transparent != 0, error: vec![0], pics })
    })();
    match r {
        Ok(x) => Box::into_raw(Box::new(x)),
        Err(err) => {
            engine.set_error(err);
            std::ptr::null_mut()
        }
    }
}

/// Renders and writes the next frame. Returns frames done so far, or -1
/// when finished (file complete), -2 on failure (see bf_export_error).
#[no_mangle]
pub extern "C" fn bf_export_step(e: *const Engine, x: *mut Export) -> i32 {
    if e.is_null() || x.is_null() {
        return -2;
    }
    // SAFETY: live handles
    let (engine, job) = unsafe { (&*e, &mut *x) };
    let Some(p) = engine.project() else { return -2 };
    let r = catch_unwind(AssertUnwindSafe(|| -> Result<bool, String> {
        if job.next >= job.count {
            match job.writer.take() {
                Some(Writer::Gif(g)) => {
                    use std::io::Write;
                    g.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())?;
                }
                Some(Writer::Png(z)) => {
                    use std::io::Write;
                    z.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())?;
                }
                None => {}
            }
            return Ok(true);
        }
        let (t, extra) = match &job.pics {
            Some(v) => v[job.next as usize],
            None => (barkfart_core::export::frame_time(job.start, job.next, p.settings.sample_rate()), 0.0),
        };
        let mut scene = eval::evaluate(p, &p.root_symbol, t);
        // Project Tools > watermark: small, at the top right of every exported picture
        if let Some((text, color)) = p.settings.watermark.as_ref().filter(|w| !w.0.trim().is_empty()) {
            let (hw, hh) = (p.settings.width as f64 / 2.0, p.settings.height as f64 / 2.0);
            let size = (p.settings.height as f32 * 0.045).max(8.0);
            let margin = size as f64 * 0.5;
            let shape = eval::Shape::new(eval::ShapeKind::Text { text: text.clone(), font: None, size, xf: eval::Affine::translate(hw - margin, hh - margin), corner: 2, align: 2 }, *color, "watermark");
            scene.root.children.push(eval::Item::Shape(shape));
        }
        let list = tess::layered(&scene, Some(&engine.glyphs));
        let (hw, hh) = (p.settings.width as f32 / 2.0, p.settings.height as f32 / 2.0);
        let bg = p.settings.background;
        let view = View { x0: -hw, y0: -hh, x1: hw, y1: hh, background: [bg.r, bg.g, bg.b, bg.a] };
        let transparent = job.transparent && matches!(job.writer, Some(Writer::Png(_)));
        let px = engine.renderer.render_export(&list, job.w, job.h, view, 2, transparent);
        match job.writer.as_mut() {
            Some(Writer::Gif(g)) => g.frame_held(&px, extra).map_err(|e| e.to_string())?,
            Some(Writer::Png(z)) => z.frame(&px).map_err(|e| e.to_string())?,
            None => return Err("export already finished".into()),
        }
        job.next += 1;
        Ok(false)
    }));
    match r {
        Ok(Ok(true)) => -1,
        Ok(Ok(false)) => job.next as i32,
        Ok(Err(err)) => {
            job.error = format!("{err}\0").into_bytes();
            job.writer = None;
            let _ = std::fs::remove_file(&job.path);
            -2
        }
        Err(_) => {
            job.error = b"export failed (internal error)\0".to_vec();
            job.writer = None;
            let _ = std::fs::remove_file(&job.path);
            -2
        }
    }
}

/// Frames the export will write.
#[no_mangle]
pub extern "C" fn bf_export_count(x: *const Export) -> i32 {
    if x.is_null() {
        return 0;
    }
    // SAFETY: live handle
    unsafe { (*x).count as i32 }
}

#[no_mangle]
pub extern "C" fn bf_export_error(x: *const Export) -> *const c_char {
    if x.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    unsafe { (*x).error.as_ptr() as *const c_char }
}

/// Frees the job; an unfinished export is cancelled and its file removed.
#[no_mangle]
pub extern "C" fn bf_export_free(x: *mut Export) {
    if x.is_null() {
        return;
    }
    // SAFETY: made by bf_export_begin, freed once
    let job = unsafe { Box::from_raw(x) };
    if job.writer.is_some() {
        drop(job.writer);
        let _ = std::fs::remove_file(&job.path);
    }
}

/// Writes the project's soundtrack for frames [start, start + count) as a
/// stereo 44.1 kHz WAV. Returns 0 on success, 1 when there is no sound.
#[no_mangle]
pub extern "C" fn bf_export_soundtrack(e: *mut Engine, path: *const c_char, start: u32, count: u32) -> i32 {
    if e.is_null() {
        return 2;
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    engine.refresh_plan();
    let (Some(p), Some(plan)) = (engine.project(), &engine.plan) else { return 2 };
    let fps = p.settings.fps.max(1.0) as f64;
    let total = ((p.root().map(|s| s.duration).unwrap_or(0.0) * fps).round() as u32).max(1);
    let count = if count == 0 { total.saturating_sub(start) } else { count };
    // frame delays and repeats: the soundtrack follows the exported pictures
    let sch = barkfart_core::timing::Schedule::build(p);
    let wav = if sch.identity {
        barkfart_core::audio::bounce(plan, start as f64 / fps, count as f64 / fps)
    } else {
        let part = sch.subset(start as i32, count.max(1) as i32);
        barkfart_core::audio::bounce(&part.retime(plan), 0.0, part.total)
    };
    let Some(wav) = wav else { return 1 };
    let Some(path) = cstr(path) else { return 2 };
    match std::fs::write(&path, wav) {
        Ok(()) => 0,
        Err(err) => {
            engine.set_error(err);
            2
        }
    }
}

// ---------------------------------------------------------------- editing

/// Opens a new project (one frame, one figure). Returns 0.
#[no_mangle]
pub extern "C" fn bf_doc_new(e: *mut Engine) -> i32 {
    if e.is_null() {
        return 1;
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    engine.open(barkfart_core::session::new_editor_project(), Default::default());
    0
}

/// Runs one editing command (JSON, see barkfart_core::session) and returns
/// the editor state as JSON, valid until the next call. Null without a project.
#[no_mangle]
pub extern "C" fn bf_cmd(e: *mut Engine, json: *const c_char) -> *const c_char {
    if e.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    let Some(text) = cstr(json) else { return std::ptr::null() };
    let Some(s) = engine.session.as_mut() else { return std::ptr::null() };
    let out = catch_unwind(AssertUnwindSafe(|| {
        let c: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        s.command(&c)
    }));
    let v = match out {
        Ok(v) => v,
        Err(_) => serde_json::json!({"error": "internal error"}),
    };
    engine.answer = serde_json::to_vec(&v).unwrap_or_default();
    engine.answer.push(0);
    engine.answer.as_ptr() as *const c_char
}

/// Renders the open project at time t into `out` (w * h * 4 bytes, straight
/// RGBA, rows top-down) showing the stage rectangle [x0, x1] x [y0, y1]
/// (y up). flags: 1 = through the camera (as played), 2 = onion skin.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn bf_render_view(e: *const Engine, t: f64, w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, flags: u32, out: *mut u8, micros: *mut u32) -> i32 {
    if e.is_null() || out.is_null() || w == 0 || h == 0 || w > 8192 || h > 8192 || !(x1 > x0) || !(y1 > y0) {
        return 1;
    }
    // SAFETY: live handle
    let engine = unsafe { &*e };
    let Some(p) = engine.project() else { return 2 };
    let r = catch_unwind(AssertUnwindSafe(|| {
        let start = std::time::Instant::now();
        let camera = flags & 1 != 0;
        let scene = eval::evaluate_with(p, &p.root_symbol, t, false, camera);
        let mut list = tess::layered(&scene, Some(&engine.glyphs));
        if flags & 2 != 0 {
            let v = &p.view;
            // ghosts only of frames that exist (none before the first, none after the last)
            let fps = p.settings.fps.max(1.0) as f64;
            let frames = ((p.root().map(|r| r.duration).unwrap_or(0.0) * fps).round() as i32).max(1);
            let here = ((t * fps).round() as i32).clamp(0, frames - 1);
            let onion = tess::OnionSkin { apply_camera: camera, before: v.onion_before.clamp(0, 3).min(here), after: v.onion_after.clamp(0, 3).min(frames - 1 - here), ..Default::default() };
            let ghosts = onion.build_layered(p, &p.root_symbol, t);
            list = if v.onion_in_front { list.with_overlay(ghosts) } else { list.with_underlay(ghosts) };
        }
        // View options "Outline selection": a thin magenta band around the selected figure
        if flags & 4 != 0 {
            if let Some((layer, _)) = engine.session.as_ref().and_then(|s| s.selected.as_ref()) {
                let pad = 2.0 * (x1 - x0) / w as f32;
                list = list.with_overlay(tess::selection_outline(&scene, layer, pad, 0xff2d92));
            }
        }
        let cpu = start.elapsed();
        let bg = p.settings.background;
        let view = View { x0, y0, x1, y1, background: [bg.r, bg.g, bg.b, bg.a] };
        let px = engine.renderer.render(&list, w, h, view);
        if std::env::var_os("BARKFART_TRACE").is_some() {
            eprintln!("render_view {w}x{h}: evaluate+tessellate {:.1} ms, gpu+readback {:.1} ms, {} triangles", cpu.as_secs_f64() * 1e3, (start.elapsed() - cpu).as_secs_f64() * 1e3, list.triangle_count());
        }
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

/// The open project as a .bark file; *len gets its size. The bytes stay
/// valid until the next save. Null on failure (see bf_engine_error).
#[no_mangle]
pub extern "C" fn bf_save(e: *mut Engine, len: *mut usize) -> *const u8 {
    if e.is_null() || len.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    let Some(s) = engine.session.as_mut() else { return std::ptr::null() };
    match s.save() {
        Ok(b) => {
            engine.saved = b;
            // SAFETY: the caller passes a usize
            unsafe { *len = engine.saved.len() };
            engine.saved.as_ptr()
        }
        Err(err) => {
            engine.set_error(err);
            std::ptr::null()
        }
    }
}

/// Like bf_save, but the project stays marked unsaved (recovery copies).
#[no_mangle]
pub extern "C" fn bf_save_copy(e: *mut Engine, len: *mut usize) -> *const u8 {
    if e.is_null() || len.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle
    let engine = unsafe { &mut *e };
    let Some(s) = engine.session.as_ref() else { return std::ptr::null() };
    match s.save_copy() {
        Ok(b) => {
            engine.saved = b;
            // SAFETY: the caller passes a usize
            unsafe { *len = engine.saved.len() };
            engine.saved.as_ptr()
        }
        Err(err) => {
            engine.set_error(err);
            std::ptr::null()
        }
    }
}

/// Draws a project's first frame fitted into w x h, transparent around it.
fn render_fitted(engine: &Engine, p: &Project, w: u32, h: u32) -> Option<Vec<u8>> {
    let scene = eval::evaluate_with(p, &p.root_symbol, 0.0, false, false);
    let list = tess::layered(&scene, Some(&engine.glyphs));
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for b in &list.batches {
        for v in b.xy.chunks(2) {
            x0 = x0.min(v[0]);
            x1 = x1.max(v[0]);
            y0 = y0.min(v[1]);
            y1 = y1.max(v[1]);
        }
    }
    if x0 > x1 {
        return None;
    }
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let k = ((x1 - x0) / w as f32).max((y1 - y0) / h as f32).max(1e-3) * 1.15;
    let (hw, hh) = (w as f32 * k / 2.0, h as f32 * k / 2.0);
    let view = View { x0: cx - hw, y0: cy - hh, x1: cx + hw, y1: cy + hh, background: [0.0, 0.0, 0.0, 0.0] };
    Some(engine.renderer.render(&list, w, h, view))
}

/// Draws one built-in figure (barkfart_core::figures) fitted into w x h,
/// transparent around it, for the figure picker. Returns 0 on success.
#[no_mangle]
pub extern "C" fn bf_render_figure(e: *const Engine, kind: *const c_char, w: u32, h: u32, out: *mut u8) -> i32 {
    if e.is_null() || out.is_null() || w == 0 || h == 0 || w > 2048 || h > 2048 {
        return 1;
    }
    // SAFETY: live handle
    let engine = unsafe { &*e };
    let Some(kind) = cstr(kind) else { return 1 };
    let r = catch_unwind(AssertUnwindSafe(|| barkfart_core::figures::preview_project(&kind).and_then(|p| render_fitted(engine, &p, w, h))));
    match r {
        Ok(Some(px)) => {
            // SAFETY: caller provides w * h * 4 bytes
            unsafe { std::ptr::copy_nonoverlapping(px.as_ptr(), out, px.len()) };
            0
        }
        _ => 2,
    }
}

/// Draws what a Stick Nodes file (.nodes, .nodemc, .stknds) holds, for the
/// figure picker. Returns 0 on success, 2 when it cannot be read.
#[no_mangle]
pub extern "C" fn bf_render_file(e: *const Engine, name: *const c_char, data: *const u8, len: usize, w: u32, h: u32, out: *mut u8) -> i32 {
    if e.is_null() || out.is_null() || data.is_null() || w == 0 || h == 0 || w > 2048 || h > 2048 {
        return 1;
    }
    // SAFETY: live handle; the caller passes len readable bytes
    let (engine, bytes) = unsafe { (&*e, std::slice::from_raw_parts(data, len)) };
    let Some(name) = cstr(name) else { return 1 };
    let r = catch_unwind(AssertUnwindSafe(|| barkfart_core::figures::preview_file(&name, bytes).and_then(|p| render_fitted(engine, &p, w, h))));
    match r {
        Ok(Some(px)) => {
            // SAFETY: caller provides w * h * 4 bytes
            unsafe { std::ptr::copy_nonoverlapping(px.as_ptr(), out, px.len()) };
            0
        }
        _ => 2,
    }
}

/// Imports a file into the open project (one undo step); returns the editor
/// state as JSON like bf_cmd (its "message" says why when it failed).
#[no_mangle]
pub extern "C" fn bf_import(e: *mut Engine, name: *const c_char, data: *const u8, len: usize) -> *const c_char {
    if e.is_null() || data.is_null() {
        return std::ptr::null();
    }
    // SAFETY: live handle; the caller passes len readable bytes
    let (engine, bytes) = unsafe { (&mut *e, std::slice::from_raw_parts(data, len)) };
    let Some(name) = cstr(name) else { return std::ptr::null() };
    let Some(s) = engine.session.as_mut() else { return std::ptr::null() };
    let v = catch_unwind(AssertUnwindSafe(|| s.import(&name, bytes))).unwrap_or_else(|_| serde_json::json!({"error": "internal error"}));
    engine.answer = serde_json::to_vec(&v).unwrap_or_default();
    engine.answer.push(0);
    engine.answer.as_ptr() as *const c_char
}

/// Opens a new project whose first figure comes from a Stick Nodes figure file.
#[no_mangle]
pub extern "C" fn bf_doc_new_with(e: *mut Engine, name: *const c_char, data: *const u8, len: usize) -> i32 {
    if e.is_null() || data.is_null() {
        return 1;
    }
    // SAFETY: live handle; the caller passes len readable bytes
    let (engine, bytes) = unsafe { (&mut *e, std::slice::from_raw_parts(data, len)) };
    let name = cstr(name).unwrap_or_else(|| "Stickfigure.nodes".into());
    let p = barkfart_core::session::new_editor_project_with(&name, bytes);
    engine.open(p, Default::default());
    0
}
