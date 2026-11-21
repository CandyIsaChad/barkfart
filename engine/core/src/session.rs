//! The editor session behind the app: the document with its history, the
//! canvas tools, the current frame, selection, mode and gestures. The app
//! talks to it with small JSON commands (see `command`) and draws what
//! `state` describes; every change is one undo step.
//!
//! Coordinates are stage units, y up (the camera is not applied while
//! editing; its frame is reported so the app can draw it).

use crate::edit::canvas::{CanvasTools, Handle, KeyScope};
use crate::edit::figure_ops::{self, BranchChange};
use crate::edit::{frame_ops, layer_ops, structure, Document, EditResult};
use crate::eval::{self, Affine, Resolver};
use crate::model::*;
use indexmap::IndexMap;
use serde_json::{json, Value as J};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Pose figures by their joints.
    Figure,
    /// Dragging anywhere moves everything.
    All,
    /// Dragging moves the camera.
    Camera,
}

#[derive(Clone, Debug)]
enum Gesture {
    None,
    /// A handle is held; the drag starts once the finger moves past the slop.
    Handle { hit: Handle, x: f32, y: f32, slop: f32, started: bool },
    /// A textfield is held; the drag starts past the slop and keys its position.
    Textfield { layer: String, x: f32, y: f32, tx: f32, ty: f32, slop: f32, started: bool },
    All { x: f32, y: f32 },
    Camera { xi: String, yi: String, x: f32, y: f32, cx: f32, cy: f32 },
}

pub struct Session {
    pub doc: Document,
    pub tools: CanvasTools,
    pub frame: i32,
    pub mode: Mode,
    /// Selected (layer, node).
    pub selected: Option<(String, String)>,
    /// The selected textfield layer (figures and textfields never both).
    pub selected_text: Option<String>,
    pub onion: bool,
    /// Every drag stretches (the Drag: Stretch switch); otherwise only stretchy segments do.
    pub stretch_all: bool,
    /// "Join to...": the figure waiting for a joint to be tapped.
    pub joining: Option<String>,
    /// The figure open in the figure editor (structure tools, its joints only).
    pub building: Option<String>,
    gesture: Gesture,
    /// The last tap (no drag): where, and which handle, for node digging.
    last_tap: Option<(f32, f32, String, String)>,
    pose_clip: Option<(IndexMap<String, f32>, IndexMap<String, f32>)>,
    /// Copied stickfigure (with joined figures) and copied figure properties.
    /// Copied figures and the time they were copied at (their pose there).
    figure_clip: Option<(Vec<Layer>, f64)>,
    props_clip: Option<layer_ops::Props>,
    textfield_clip: Option<Layer>,
    filter_clip: Option<crate::edit::filters::Snapshot>,
    /// Copied frame: its keys (times relative to the frame).
    frame_clip: Option<(crate::edit::editor::KeyClip, i32)>,
    /// Bumped on every change the app should redraw for (drags included).
    rev: u64,
    pub message: String,
}

const STARTER: &str = include_str!("starter.json");

/// A new project: one frame, one figure (the Java version's new project).
pub fn new_project() -> Project {
    crate::codec::decode(STARTER).expect("starter project")
}

/// A new project starting with a Stick Nodes figure file (the original's
/// default stickfigure), else the built-in one.
pub fn new_editor_project_with(name: &str, figure: &[u8]) -> Project {
    let mut p = new_editor_project();
    let backup = p.clone();
    if let Some(r) = p.root_mut() {
        r.layers.clear();
    }
    match crate::import::import_file(&mut p, name, figure) {
        Ok(_) => p,
        Err(_) => backup,
    }
}

/// A new project as the editor starts it: like the original app, 10 frames per second.
pub fn new_editor_project() -> Project {
    let mut p = new_project();
    p.settings.fps = 10.0;
    // the original's stage
    p.settings.width = 1920;
    p.settings.height = 1080;
    if let Some(r) = p.root_mut() {
        r.duration = 0.1;
    }
    p
}

impl Session {
    pub fn new(doc: Document) -> Session {
        let mut tools = CanvasTools::default();
        tools.key_scope = KeyScope::Figure;
        let frame = doc.project().view.frame.max(0);
        let mut doc = doc;
        // repair keys that drifted off their frames (older Barkfart builds)
        snap_keys(&mut doc.state);
        let mut s = Session { doc, tools, frame, mode: Mode::Figure, selected: None, selected_text: None, onion: true, stretch_all: false, joining: None, building: None, gesture: Gesture::None, last_tap: None, pose_clip: None, frame_clip: None, figure_clip: None, props_clip: None, textfield_clip: None, filter_clip: None, rev: 1, message: String::new() };
        s.validate();
        s
    }

    pub fn project(&self) -> &Project {
        self.doc.project()
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    fn sym(&self) -> String {
        self.project().root_symbol.clone()
    }

    pub fn time(&self) -> f64 {
        frame_ops::time(&self.doc.state, self.frame)
    }

    pub fn frame_count(&self) -> i32 {
        frame_ops::frame_count(&self.doc.state)
    }

    fn validate(&mut self) {
        let n = self.frame_count();
        self.frame = self.frame.clamp(0, n - 1);
        if let Some((l, node)) = &self.selected {
            let t = self.time();
            let p = self.project();
            let ok = p.root().and_then(|s| s.layer(l)).filter(|x| Resolver::new(p).b(&x.shown, t)).and_then(|l| l.as_figure()).is_some_and(|f| f.node(node).is_some());
            if !ok {
                self.selected = None;
            }
        }
        if let Some(id) = &self.selected_text {
            let t = self.time();
            let p = self.project();
            let ok = p.root().and_then(|s| s.layer(id)).filter(|x| Resolver::new(p).b(&x.shown, t)).is_some_and(|l| matches!(l.kind, LayerKind::Text(_)));
            if !ok {
                self.selected_text = None;
            }
        }
    }

    fn sync_tools(&mut self) {
        self.tools.active_figure = self.selected.as_ref().map(|s| s.0.clone());
        if let Some(b) = &self.building {
            if self.project().root().and_then(|s| s.layer(b)).is_none() {
                self.building = None;
            }
        }
    }

    fn changed(&mut self) {
        self.rev += 1;
        self.validate();
        self.sync_tools();
    }

    fn figure_ids(&self) -> Vec<String> {
        self.project().root().map(|s| s.layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect()).unwrap_or_default()
    }

    fn sel_layer(&self) -> Option<String> {
        self.selected.as_ref().map(|s| s.0.clone())
    }

    /// The topmost visible textfield whose box (plus the tap radius) is hit.
    fn textfield_at(&self, x: f32, y: f32, radius: f32, t: f64) -> Option<String> {
        let p = self.project();
        let mut r = Resolver::new(p);
        for l in p.root()?.layers.iter().rev() {
            let LayerKind::Text(tl) = &l.kind else { continue };
            if !r.b(&l.shown, t) {
                continue;
            }
            if text_hit(tl, l, &mut r, t, x, y, radius) {
                return Some(l.id.clone());
            }
        }
        None
    }

    fn textfield_hit(&self, id: &str, x: f32, y: f32, radius: f32, t: f64) -> bool {
        let p = self.project();
        let Some(l) = p.root().and_then(|s| s.layer(id)) else { return false };
        let LayerKind::Text(tl) = &l.kind else { return false };
        let mut r = Resolver::new(p);
        r.b(&l.shown, t) && text_hit(tl, l, &mut r, t, x, y, radius)
    }

    /// Runs one edit as an undo step and reports failures as the message.
    fn step<T>(&mut self, label: &str, f: impl FnOnce(&mut crate::edit::State) -> EditResult<T>) -> Option<T> {
        let r = self.doc.edit(label, |s| {
            anchor_poses(s);
            let v = f(s)?;
            anchor_poses(s);
            snap_keys(s);
            Ok(v)
        });
        self.changed();
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.message = e.0;
                None
            }
        }
    }

    // ---- gestures (stage units) ----

    pub fn touch_down(&mut self, x: f32, y: f32, radius: f32) -> bool {
        self.end_gesture(false);
        let t = self.time();
        match self.mode {
            Mode::Figure => {
                let prev = self.last_tap.as_ref().filter(|(tx, ty, _, _)| (tx - x).hypot(ty - y) <= radius).map(|(_, _, l, n)| (l.clone(), n.clone()));
                // nearest joint wins; tapping the same spot again steps to the next one under the finger
                self.sync_tools();
                if let Some(child) = self.joining.clone() {
                    // pick a joint of another figure (the figure itself and what hangs off it excluded)
                    let own = layer_ops::with_joined(self.project().root().expect("root"), &child);
                    let near = self.tools.handles(self.project(), t).into_iter().filter(|h| !own.contains(&h.layer)).min_by(|a, b| {
                        let d = |h: &Handle| (h.x - x).hypot(h.y - y);
                        d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    if let Some(h) = near.filter(|h| (h.x - x).hypot(h.y - y) <= radius * 1.5) {
                        let sym = self.sym();
                        self.step("Join", |s| crate::edit::join_ops::join(s, &sym, &child, &h.layer, Some(&h.node), true, t));
                        self.joining = None;
                        self.selected = self.project().root().and_then(|s| s.layer(&child)).and_then(|l| l.as_figure()).and_then(|f| f.nodes().first().map(|n| (child.clone(), n.id.clone())));
                    } else {
                        self.message = "Tap a joint on another figure.".into();
                        self.rev += 1;
                    }
                    return true;
                }
                // textfields are drawn on top, so tapping one selects it; a
                // selected textfield is dragged even when a figure is under it
                if self.building.is_none() {
                    let picked = match self.selected_text.clone() {
                        Some(id) if self.textfield_hit(&id, x, y, radius, t) => Some(id),
                        _ => self.textfield_at(x, y, radius, t),
                    };
                    if let Some(id) = picked {
                        let p = self.project();
                        let l = p.root().and_then(|s| s.layer(&id)).expect("textfield");
                        let mut r = Resolver::new(p);
                        let (tx, ty) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
                        self.selected = None;
                        self.selected_text = Some(id.clone());
                        self.last_tap = None;
                        self.gesture = Gesture::Textfield { layer: id, x, y, tx, ty, slop: radius * 0.25, started: false };
                        self.rev += 1;
                        return true;
                    }
                }
                let mut hit = self.tools.pick_cycling(self.project(), x, y, radius, t, None, prev.as_ref().map(|(l, n)| (l.as_str(), n.as_str())));
                if let (Some(b), Some(h)) = (&self.building, &hit) {
                    if h.layer != *b {
                        hit = None;
                    }
                }
                match hit {
                    Some(h) => {
                        self.selected = Some((h.layer.clone(), h.node.clone()));
                        self.selected_text = None;
                        self.gesture = Gesture::Handle { hit: h, x, y, slop: radius * 0.25, started: false };
                        self.rev += 1;
                        true
                    }
                    None if self.building.is_some() => false,
                    None => {
                        let changed = self.selected.take().is_some() | self.selected_text.take().is_some();
                        if changed {
                            self.rev += 1;
                        }
                        self.last_tap = None;
                        false
                    }
                }
            }
            Mode::All => {
                self.doc.begin("Move everything");
                anchor_poses(&mut self.doc.state);
                self.gesture = Gesture::All { x, y };
                true
            }
            Mode::Camera => {
                let p = self.project();
                let Some(s) = p.root() else { return false };
                let mut r = Resolver::new(p);
                let Some(cam) = eval::active_camera(s, t, &mut r) else {
                    self.message = "There is no camera. Add one first.".into();
                    self.rev += 1;
                    return false;
                };
                let (cx, cy) = (r.f(&cam.x, t), r.f(&cam.y, t));
                let (xi, yi) = (cam.x.id.clone(), cam.y.id.clone());
                self.doc.begin("Move camera");
                anchor_poses(&mut self.doc.state);
                self.gesture = Gesture::Camera { xi, yi, x, y, cx, cy };
                true
            }
        }
    }

    pub fn touch_move(&mut self, x: f32, y: f32) {
        let t = self.time();
        match self.gesture.clone() {
            Gesture::None => {}
            Gesture::Handle { hit, x: x0, y: y0, slop, started } => {
                if !started {
                    if (x - x0).hypot(y - y0) < slop {
                        return;
                    }
                    let stretchy = self.project().root().and_then(|s| s.layer(&hit.layer)).and_then(|l| l.as_figure()).and_then(|f| f.node(&hit.node)).is_some_and(|n| n.stretchy);
                    self.tools.stretch = self.stretch_all || stretchy;
                    self.doc.begin(if hit.root { "Move figure" } else { "Pose node" });
                    anchor_poses(&mut self.doc.state);
                    self.tools.begin(&mut self.doc, &hit, x0, y0, t);
                    self.gesture = Gesture::Handle { hit, x: x0, y: y0, slop, started: true };
                }
                if let Err(e) = self.tools.move_to(&mut self.doc, x, y, t) {
                    self.message = e.0;
                }
                self.rev += 1;
            }
            Gesture::Textfield { layer, x: x0, y: y0, tx, ty, slop, started } => {
                if !started {
                    if (x - x0).hypot(y - y0) < slop {
                        return;
                    }
                    // a locked textfield does not move (the drag deselects it)
                    if self.project().root().and_then(|s| s.layer(&layer)).is_some_and(|l| l.locked) {
                        self.selected_text = None;
                        self.gesture = Gesture::None;
                        self.rev += 1;
                        return;
                    }
                    self.doc.begin("Move textfield");
                    anchor_poses(&mut self.doc.state);
                    self.gesture = Gesture::Textfield { layer: layer.clone(), x: x0, y: y0, tx, ty, slop, started: true };
                }
                let ed = self.tools.editor.clone();
                let ids = self.project().root().and_then(|s| s.layer(&layer)).map(|l| (l.transform.x.id.clone(), l.transform.y.id.clone()));
                if let Some((xi, yi)) = ids {
                    let _ = ed.change_in(&mut self.doc.state, &xi, Value::F(tx + (x - x0)), t);
                    let _ = ed.change_in(&mut self.doc.state, &yi, Value::F(ty + (y - y0)), t);
                }
                self.rev += 1;
            }
            Gesture::All { x: x0, y: y0 } => {
                let _ = self.tools.move_all(&mut self.doc, x - x0, y - y0, t);
                self.gesture = Gesture::All { x, y };
                self.rev += 1;
            }
            Gesture::Camera { xi, yi, x: x0, y: y0, cx, cy } => {
                let ed = self.tools.editor.clone();
                let _ = ed.change_in(&mut self.doc.state, &xi, Value::F(cx + (x - x0)), t);
                let _ = ed.change_in(&mut self.doc.state, &yi, Value::F(cy + (y - y0)), t);
                self.rev += 1;
            }
        }
    }

    pub fn touch_up(&mut self) {
        self.end_gesture(false);
    }

    pub fn touch_cancel(&mut self) {
        self.end_gesture(true);
    }

    fn end_gesture(&mut self, cancel: bool) {
        match std::mem::replace(&mut self.gesture, Gesture::None) {
            Gesture::None => return,
            Gesture::Handle { hit, x, y, started, .. } => {
                if started {
                    if cancel {
                        self.tools.cancel(&mut self.doc);
                        self.doc.cancel();
                    } else {
                        self.tools.end(&mut self.doc);
                        anchor_poses(&mut self.doc.state);
                        snap_keys(&mut self.doc.state);
                        self.doc.commit();
                    }
                    self.last_tap = None;
                } else {
                    self.last_tap = Some((x, y, hit.layer, hit.node));
                }
            }
            Gesture::Textfield { started, .. } => {
                if started {
                    if cancel {
                        self.doc.cancel();
                    } else {
                        anchor_poses(&mut self.doc.state);
                        snap_keys(&mut self.doc.state);
                        self.doc.commit();
                    }
                }
            }
            Gesture::All { .. } | Gesture::Camera { .. } => {
                if cancel {
                    self.doc.cancel();
                } else {
                    self.doc.commit();
                }
            }
        }
        self.changed();
    }

    pub fn dragging(&self) -> bool {
        !matches!(self.gesture, Gesture::None)
    }

    // ---- commands ----

    /// Runs one JSON command ({"op": ..., ...}); returns the new state (or an error).
    pub fn command(&mut self, c: &J) -> J {
        self.message.clear();
        let op = c["op"].as_str().unwrap_or("");
        let f = |k: &str| c[k].as_f64().unwrap_or(0.0) as f32;
        let b = |k: &str| c[k].as_bool().unwrap_or(false);
        if self.dragging() && !matches!(op, "move" | "up" | "cancel" | "state") {
            self.end_gesture(false);
        }
        let sym = self.sym();
        let t = self.time();
        match op {
            "state" => {}
            "mark_unsaved" => {
                self.doc.mark_unsaved();
                self.rev += 1;
            }
            "down" => {
                let hit = self.touch_down(f("x"), f("y"), f("r"));
                let mut s = self.state();
                s["hit"] = json!(hit);
                return s;
            }
            "move" => self.touch_move(f("x"), f("y")),
            "up" => self.touch_up(),
            "cancel" => self.touch_cancel(),
            "undo" => {
                self.doc.undo();
                self.changed();
            }
            "redo" => {
                self.doc.redo();
                self.changed();
            }
            "frame" => {
                self.frame = c["i"].as_i64().unwrap_or(0) as i32;
                self.last_tap = None;
                self.changed();
            }
            "insert_frame" => {
                let fr = self.frame;
                let copy = c.get("copy").and_then(|v| v.as_bool()).unwrap_or(true);
                let made = self.step("Insert frame", |s| {
                    frame_ops::insert_after(s, fr, copy)?;
                    if copy {
                        // a new frame is a whole pose (a snapshot), like the original's frames
                        anchor_poses(s);
                        frame_ops::key_pose(s, None, frame_ops::time(s, fr + 1))?;
                    }
                    Ok(())
                });
                if made.is_some() {
                    self.frame += 1;
                    self.changed();
                }
            }
            "delete_frame" => {
                let fr = self.frame;
                self.step("Delete frame", |s| frame_ops::delete(s, fr));
            }
            "animation" => {
                // Animation Tools: tweening on/off and tweened frames (playback and export only)
                let (tw, n) = (c["tweening"].as_bool(), c["tweened"].as_i64());
                self.step("Animation settings", |s| {
                    if let Some(v) = tw {
                        s.project.settings.tweening = v;
                    }
                    if let Some(v) = n {
                        s.project.settings.tweened_frames = (v as i32).clamp(0, 8);
                    }
                    Ok(())
                });
            }
            "tween_add" => {
                // "Add tweened frame": new real frames between this one and the next,
                // each the pose a fraction of the way there
                let fracs: Vec<f64> = match c["n"].as_i64() {
                    Some(n) => {
                        let n = n.clamp(1, 240);
                        (1..=n).map(|k| k as f64 / (n + 1) as f64).collect()
                    }
                    None => c["at"].as_array().map(|a| a.iter().filter_map(|v| v.as_f64()).collect()).unwrap_or_else(|| vec![0.5]),
                };
                let (fr, n) = (self.frame, self.frame_count());
                if fr + 1 >= n {
                    self.message = "Tweened frames go between two frames: add the next frame first.".into();
                } else if let Some(k) = self.step("Add tweened frame", |s| add_tweened(s, fr, &fracs)) {
                    self.frame = fr + k;
                    self.changed();
                }
            }
            "no_tween" => {
                let (fr, on, ed) = (self.frame, b("on"), self.tools.editor.clone());
                self.step(if on { "Do not tween" } else { "Tween" }, |s| frame_ops::set_tween(&ed, s, fr, !on));
            }
            "copy_frame" => {
                // this frame plus n after it
                let n = (c["n"].as_i64().unwrap_or(0) as i32).clamp(0, self.frame_count() - 1 - self.frame);
                let fps = st_fps(self.project());
                let mut sel = Vec::new();
                for k in 0..=n {
                    sel.extend(self.keys_at(t + k as f64 / fps, None));
                }
                self.frame_clip = Some((self.tools.editor.copy(self.project(), &sel), n + 1));
                self.message = if n == 0 { "Frame copied.".into() } else { format!("{} frames copied.", n + 1) };
                self.rev += 1;
            }
            "paste_frame" => {
                if let Some((clip, count)) = self.frame_clip.clone() {
                    let fr = self.frame;
                    let made = self.step("Paste frame", |s| {
                        for k in 0..count {
                            frame_ops::insert_after(s, fr + k, true)?;
                        }
                        let at = frame_ops::time(s, fr + 1);
                        let fps = st_fps(&s.project);
                        for (id, keys) in &clip.keys {
                            let Some(p) = s.project.property_mut(id) else { continue };
                            if keys.is_empty() || !p.ty.checked(&keys[0].value) {
                                continue;
                            }
                            for k in keys {
                                // onto the frame grid (clip times are relative to the first copied frame)
                                let tt = at + ((k.time * fps).round() / fps);
                                p.track.put(Key { time: tt, ..*k });
                            }
                        }
                        Ok(())
                    });
                    if made.is_some() {
                        self.frame = fr + count;
                        self.changed();
                    }
                }
            }
            "easing" => {
                let v = c["v"].as_str().unwrap_or("linear").to_string();
                let (interp, ease) = match v.as_str() {
                    "hold" => (Interp::Hold, Ease::LINEAR),
                    "smooth" => (Interp::Bezier, Ease::new(0.42, 0.0, 0.58, 1.0)),
                    "in" => (Interp::Bezier, Ease::new(0.42, 0.0, 1.0, 1.0)),
                    "out" => (Interp::Bezier, Ease::new(0.0, 0.0, 0.58, 1.0)),
                    _ => (Interp::Linear, Ease::LINEAR),
                };
                let sel = self.keys_at(t, self.sel_layer().as_deref());
                if sel.is_empty() {
                    self.message = "Nothing is keyed on this frame.".into();
                } else {
                    let ed = self.tools.editor.clone();
                    self.step("Easing", |s| ed.set_interp_in(s, &sel, interp, ease));
                }
            }
            "delete_here" => {
                // gone from this frame only (hold: with the figures joined to it)
                if let Some(l) = self.sel_layer() {
                    let fr = self.frame;
                    let joined = b("joined");
                    self.step("Delete stickfigure", |s| {
                        let block = if joined { layer_ops::with_joined(s.project.root().expect("root"), &l) } else { vec![l.clone()] };
                        for id in &block {
                            if let Some(x) = s.project.layer_mut(&sym, id) {
                                x.shown.track.put(Key::new(t, Value::B(false), Interp::Hold));
                            }
                        }
                        // a figure on no frame at all is removed
                        let n = frame_ops::frame_count(s);
                        let fps = st_fps(&s.project);
                        for id in &block {
                            let gone = s.project.root().and_then(|r| r.layer(id)).is_some_and(|x| (0..n).all(|i| !Resolver::new(&s.project).b(&x.shown, i as f64 / fps)));
                            if gone {
                                structure::remove_layer(s, &sym, id)?;
                            }
                        }
                        let _ = fr;
                        Ok(())
                    });
                    self.selected = None;
                }
            }
            "tween" => {
                let (fr, on, ed) = (self.frame, b("on"), self.tools.editor.clone());
                self.step(if on { "Tween on" } else { "Tween off" }, |s| frame_ops::set_tween(&ed, s, fr, on));
            }
            "fps" => {
                let v = f("v").clamp(1.0, 120.0);
                self.step("Frame rate", |s| {
                    // keys stay on their frames: scale every time by old/new
                    let old = s.project.settings.fps as f64;
                    let k = old / v as f64;
                    let root = s.project.root_symbol.clone();
                    let ids: Vec<String> = s.project.root().map(|r| r.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
                    for id in ids {
                        if let Some(l) = s.project.layer_mut(&root, &id) {
                            for p in l.properties_mut() {
                                let keys: Vec<Key> = p.track.keys().iter().map(|q| Key { time: q.time * k, ..q.clone() }).collect();
                                p.track.clear();
                                for q in keys {
                                    p.track.put(q);
                                }
                            }
                        }
                    }
                    if let Some(r) = s.project.root_mut() {
                        r.duration *= k;
                    }
                    s.project.settings.fps = v;
                    Ok(())
                });
            }
            "canvas" => {
                let (w, h) = (c["w"].as_i64().unwrap_or(1280) as i32, c["h"].as_i64().unwrap_or(720) as i32);
                self.step("Canvas size", |s| structure::set_canvas_size(s, w.clamp(16, 4096), h.clamp(16, 4096)));
            }
            "mode" => {
                self.mode = match c["v"].as_str().unwrap_or("") {
                    "all" => Mode::All,
                    "camera" => Mode::Camera,
                    _ => Mode::Figure,
                };
                self.rev += 1;
            }
            "set" => {
                match c["k"].as_str().unwrap_or("") {
                    "stretch" => self.stretch_all = b("v"),
                    "scope" => self.tools.key_scope = if c["v"].as_str() == Some("joint") { KeyScope::Joint } else { KeyScope::Figure },
                    "snap" => self.tools.snap_degrees = f("v").max(0.0),
                    "onion" => self.onion = b("v"),
                    "axis" => self.tools.axis_lock = b("v"),
                    "static" => self.tools.show_static = b("v"),
                    "mainonly" => self.tools.main_nodes_only = b("v"),
                    // onion skin view settings (saved with the project, not undo steps)
                    "onion_before" => self.doc.project_mut().view.onion_before = (c["v"].as_i64().unwrap_or(1) as i32).clamp(0, 3),
                    "onion_after" => self.doc.project_mut().view.onion_after = (c["v"].as_i64().unwrap_or(1) as i32).clamp(0, 3),
                    "onion_front" => self.doc.project_mut().view.onion_in_front = b("v"),
                    "tween_new" => self.tools.editor.default_interp = if b("v") { Interp::Linear } else { Interp::Hold },
                    _ => {}
                }
                self.rev += 1;
            }
            "select" => {
                let l = c["layer"].as_str().map(str::to_string);
                self.selected_text = None;
                self.selected = match l {
                    Some(l) => {
                        let root = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                        let node = c["node"].as_str().map(str::to_string).or(root);
                        node.map(|n| (l, n))
                    }
                    None => None,
                };
                self.rev += 1;
            }
            "cycle_figure" => {
                let ids = self.figure_ids();
                if !ids.is_empty() {
                    let dir = c["dir"].as_i64().unwrap_or(1) as i32;
                    let at = self.sel_layer().and_then(|l| ids.iter().position(|x| *x == l)).map(|i| i as i32).unwrap_or(-1);
                    let n = ids.len() as i32;
                    let i = (((at + dir) % n) + n) % n;
                    let l = ids[i as usize].clone();
                    let root = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                    self.selected = root.map(|r| (l, r));
                    self.selected_text = None;
                    self.rev += 1;
                }
            }
            "cycle_joint" => {
                if let Some((l, n)) = self.selected.clone() {
                    let ids: Vec<String> = self.tools.handles(self.project(), t).into_iter().filter(|h| h.layer == l).map(|h| h.node).collect();
                    if let Some(at) = ids.iter().position(|x| *x == n) {
                        let dir = c["dir"].as_i64().unwrap_or(1) as i32;
                        let k = ids.len() as i32;
                        let i = (((at as i32 + dir) % k) + k) % k;
                        self.selected = Some((l, ids[i as usize].clone()));
                        self.rev += 1;
                    }
                }
            }
            "build" => {
                let l = c["layer"].as_str().map(str::to_string).or(self.sel_layer());
                if let Some(l) = l {
                    if self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).is_some() {
                        self.building = Some(l.clone());
                        if self.sel_layer().as_deref() != Some(&l) {
                            let root = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                            self.selected = root.map(|r| (l, r));
                        }
                        self.mode = Mode::Figure;
                        self.rev += 1;
                    }
                }
            }
            "build_done" => {
                self.building = None;
                self.rev += 1;
            }
            "add_figure" => {
                let n = self.figure_ids().len() + 1;
                let kind = c["kind"].as_str().unwrap_or("stickman").to_string();
                let name = crate::figures::ALL.iter().find(|(id, _)| *id == kind).map(|(_, nm)| *nm).unwrap_or("Figure");
                let template = new_project();
                let src = template.root().and_then(|s| s.layers.first()).map(|l| (**l).clone()).expect("starter figure");
                let fr_now = self.frame;
                let made = self.step("Add figure", |s| {
                    let id = s.project.new_id("L");
                    let mut l = if kind == "stickman" {
                        let mut ids = std::collections::HashMap::new();
                        ids.insert(src.id.clone(), id.clone());
                        structure::copy_layer(&src, &ids)?
                    } else {
                        match crate::figures::build(&kind, &id, name) {
                            Some(l) => l,
                            None => return crate::edit::fail(format!("no built-in figure {kind}")),
                        }
                    };
                    l.name = Some(if kind == "stickman" { format!("Figure {n}") } else { format!("{name} {n}") });
                    // offset so it does not sit exactly on the last one
                    // side by side: left, right, further left, further right ...
                    let k = (n as i32) / 2;
                    let side = if n % 2 == 0 { -1.0 } else { 1.0 };
                    l.transform.x.value = Value::F(side * 230.0 * (((k - 1) % 3) + 1) as f32 * ((n > 1) as i32 as f32));
                    let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                    let sym = s.project.root_symbol.clone();
                    structure::add_layer(s, &sym, l, at)?;
                    // frames are snapshots: the new figure is on this frame only
                    only_on_frame(s, &sym, &id, fr_now);
                    Ok(id)
                });
                if let Some(id) = made {
                    let root = self.project().root().and_then(|s| s.layer(&id)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                    self.selected = root.map(|r| (id, r));
                    self.selected_text = None;
                }
            }
            "add_textfield" => {
                let n = self
                    .project()
                    .root()
                    .map(|s| s.layers.iter().filter(|l| matches!(l.kind, LayerKind::Text(_))).count())
                    .unwrap_or(0)
                    + 1;
                let fr_now = self.frame;
                let made = self.step("Add textfield", |s| {
                    let id = s.project.new_id("L");
                    let mut l = Layer::text(&id, Some(&format!("Textfield {n}")));
                    let LayerKind::Text(t) = &mut l.kind else { unreachable!() };
                    t.text = Some("Hello".into());
                    t.size = 64.0;
                    t.color.value = Value::C(Color::rgba8888(0x1a1a1aff));
                    t.align = 1;
                    let sym = s.project.root_symbol.clone();
                    let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                    structure::add_layer(s, &sym, l, at)?;
                    only_on_frame(s, &sym, &id, fr_now);
                    Ok(id)
                });
                if let Some(id) = made {
                    self.selected = None;
                    self.selected_text = Some(id);
                }
            }
            "textfield" => {
                if let Some(id) = self.selected_text.clone() {
                    let c2 = c.clone();
                    let ed = self.tools.editor.clone();
                    self.step("Textfield", move |s| {
                        let scale = {
                            let Some(layer) = s.project.layer_mut(&sym, &id) else { return crate::edit::fail("no textfield") };
                            let LayerKind::Text(tl) = &mut layer.kind else { return crate::edit::fail("not a textfield") };
                            if let Some(v) = c2["text"].as_str() {
                                tl.text = Some(v.to_string());
                            }
                            if let Some(v) = c2["color"].as_u64() {
                                tl.color.track.put(Key::new(t, Value::C(Color::rgba8888(v as u32)), Interp::Linear));
                            }
                            if let Some(v) = c2["align"].as_i64() {
                                tl.align = (v as i32).clamp(0, 2);
                            }
                            if let Some(v) = c2["locked"].as_bool() {
                                layer.locked = v;
                            }
                            c2["scale"].as_f64().map(|v| (layer.transform.scale_x.id.clone(), layer.transform.scale_y.id.clone(), (v.clamp(0.01, 100.0)) as f32))
                        };
                        if let Some((xi, yi, v)) = scale {
                            ed.change_in(s, &xi, Value::F(v), t)?;
                            ed.change_in(s, &yi, Value::F(v), t)?;
                        }
                        Ok(())
                    });
                    self.validate();
                }
            }
            "delete_textfield" => {
                if let Some(id) = self.selected_text.clone() {
                    self.step("Delete textfield", |s| {
                        let n = frame_ops::frame_count(s);
                        let fps = st_fps(&s.project);
                        if let Some(l) = s.project.layer_mut(&sym, &id) {
                            l.shown.track.put(Key::new(t, Value::B(false), Interp::Hold));
                        }
                        let gone = s.project.root().and_then(|r| r.layer(&id)).is_some_and(|x| (0..n).all(|i| !Resolver::new(&s.project).b(&x.shown, i as f64 / fps)));
                        if gone {
                            structure::remove_layer(s, &sym, &id)?;
                        }
                        Ok(())
                    });
                    self.selected_text = None;
                }
            }
            "copy_textfield" => {
                if let Some(id) = self.selected_text.clone() {
                    if let Some(l) = self.project().root().and_then(|s| s.layer(&id)) {
                        self.textfield_clip = Some(l.clone());
                        self.message = "Textfield copied.".into();
                        self.rev += 1;
                    }
                }
            }
            "paste_textfield" => {
                if let Some(src) = self.textfield_clip.clone() {
                    let fr_now = self.frame;
                    let made = self.step("Paste textfield", |s| {
                        let id = s.project.new_id("L");
                        let mut ids = std::collections::HashMap::new();
                        ids.insert(src.id.clone(), id.clone());
                        let mut c = structure::copy_layer(&src, &ids)?;
                        c.name = Some("Textfield copy".into());
                        let sym = s.project.root_symbol.clone();
                        let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                        structure::add_layer(s, &sym, c, at)?;
                        only_on_frame(s, &sym, &id, fr_now);
                        Ok(id)
                    });
                    if let Some(id) = made {
                        self.selected = None;
                        self.selected_text = Some(id);
                    }
                }
            }
            "delete_everywhere" | "delete_figure" => {
                if let Some(l) = self.sel_layer() {
                    self.step("Delete figure", |s| structure::remove_layer(s, &sym, &l));
                }
            }
            "duplicate_figure" => {
                if let Some(l) = self.sel_layer() {
                    if let Some(ids) = self.step("Duplicate figure", |s| layer_ops::duplicate(s, &sym, &l, true, 30.0, -10.0)) {
                        let root = self.selected.as_ref().map(|x| x.1.clone()).unwrap_or_default();
                        self.selected = ids.first().map(|i| (i.clone(), root));
                        self.validate();
                    }
                }
            }
            "flip_figure" => {
                if let Some(l) = self.sel_layer() {
                    let h = c.get("h").and_then(|v| v.as_bool()).unwrap_or(true);
                    self.step("Flip figure", |s| figure_ops::flip_figure(s, &sym, &l, h));
                }
            }
            "rotate_figure" => {
                if let Some(l) = self.sel_layer() {
                    self.doc.begin("Rotate figure");
                    anchor_poses(&mut self.doc.state);
                    let r = self.tools.rotate_figure(&mut self.doc, &l, f("deg") as f64, t);
                    self.doc.commit();
                    if let Err(e) = r {
                        self.message = e.0;
                    }
                    self.changed();
                }
            }
            "layer_order" => {
                if let Some(l) = self.sel_layer() {
                    let fwd = b("forward");
                    self.step(if fwd { "Bring forward" } else { "Send back" }, |s| layer_ops::move_with_joined(s, &sym, &l, fwd));
                }
            }
            "add_segment" => {
                if let Some((l, n)) = self.selected.clone() {
                    let len = c.get("len").and_then(|v| v.as_f64()).unwrap_or(40.0) as f32;
                    if let Some(id) = self.step("Add segment", |s| figure_ops::add_segment(s, &sym, &l, &n, 30.0, len)) {
                        self.selected = Some((l, id));
                    }
                }
            }
            "delete_segment" => {
                if let Some((l, n)) = self.selected.clone() {
                    let is_root = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.node(&n)).is_some_and(|x| x.parent.is_none());
                    if is_root {
                        self.message = "The main node cannot be deleted. Delete the figure instead.".into();
                    } else {
                        self.step("Delete segment", |s| structure::remove_node(s, &sym, &l, &n));
                    }
                }
            }
            "split" => {
                if let Some((l, n)) = self.selected.clone() {
                    let k = c["n"].as_u64().unwrap_or(2) as usize;
                    self.step("Split segment", |s| figure_ops::split(s, &sym, &l, &n, k.clamp(2, 10)));
                }
            }
            "duplicate_branch" => {
                if let Some((l, n)) = self.selected.clone() {
                    if let Some(id) = self.step("Duplicate branch", |s| figure_ops::duplicate_branch(s, &sym, &l, &n)) {
                        self.selected = Some((l, id));
                    }
                }
            }
            "flip_branch" => {
                if let Some((l, n)) = self.selected.clone() {
                    let h = c.get("h").and_then(|v| v.as_bool()).unwrap_or(true);
                    self.step("Flip branch", |s| figure_ops::flip_branch(s, &sym, &l, &n, h, t));
                }
            }
            "pin" => {
                if let Some((l, n)) = self.selected.clone() {
                    let on = b("v");
                    self.step(if on { "Lock joint" } else { "Unlock joint" }, |s| figure_ops::set_pinned(s, &sym, &l, &n, on));
                }
            }
            "segment" => {
                // shape, thickness, colour, static, smart-stretch of the selected segment (not keyed)
                if let Some((l, n)) = self.selected.clone() {
                    let c2 = c.clone();
                    let branch = b("branch");
                    self.step("Segment", move |s| {
                        if branch {
                            let ch = if let Some(v) = c2["thickness"].as_f64() {
                                BranchChange::Thickness(v as f32)
                            } else if let Some(v) = c2["color"].as_u64() {
                                BranchChange::Color(Color::rgba8888(v as u32))
                            } else if let Some(v) = c2["shape"].as_str() {
                                BranchChange::Shape(shape_named(v))
                            } else if let Some(v) = c2["static"].as_bool() {
                                BranchChange::Static(v)
                            } else {
                                return Ok(());
                            };
                            return figure_ops::modify_branch(s, &sym, &l, &n, &ch);
                        }
                        let Some(layer) = s.project.layer_mut(&sym, &l) else { return crate::edit::fail("no figure") };
                        let LayerKind::Figure(fig) = &mut layer.kind else { return crate::edit::fail("not a figure") };
                        let Some(node) = fig.node_mut(&n) else { return crate::edit::fail("no segment") };
                        if let Some(v) = c2["thickness"].as_f64() {
                            set_static(&mut node.thickness, Value::F(v.clamp(0.0, 400.0) as f32));
                        }
                        if let Some(v) = c2["color"].as_u64() {
                            set_static(&mut node.color, Value::C(Color::rgba8888(v as u32)));
                        }
                        if let Some(v) = c2["shape"].as_str() {
                            if node.parent.is_some() {
                                node.shape = shape_named(v);
                            }
                        }
                        if let Some(v) = c2["static"].as_bool() {
                            node.is_static = v;
                        }
                        if let Some(v) = c2["smart"].as_bool() {
                            node.smart_stretch = v;
                        }
                        Ok(())
                    });
                }
            }
            "figure" => {
                // Stickfigure Tools values: keyed on this frame like a pose
                if let Some(l) = self.sel_layer() {
                    let c2 = c.clone();
                    let ed = self.tools.editor.clone();
                    let mut r = Resolver::new(self.project());
                    let lay = self.project().root().and_then(|s| s.layer(&l)).cloned();
                    let Some(lay) = lay else { return self.state() };
                    let (x, y, rot, sx, sy) = (r.f(&lay.transform.x, t), r.f(&lay.transform.y, t), r.f(&lay.transform.rotation, t), r.f(&lay.transform.scale_x, t), r.f(&lay.transform.scale_y, t));
                    let ids = (lay.transform.x.id.clone(), lay.transform.y.id.clone(), lay.transform.rotation.id.clone(), lay.transform.scale_x.id.clone(), lay.transform.scale_y.id.clone());
                    self.step("Stickfigure", move |s| {
                        if let Some(v) = c2["scale"].as_f64() {
                            let v = v.clamp(0.01, 100.0) as f32;
                            // keep the sign (a flipped figure stays flipped)
                            ed.change_in(s, &ids.3, Value::F(v * sx.signum() * if sx == 0.0 { 1.0 } else { 1.0 }), t)?;
                            ed.change_in(s, &ids.4, Value::F(v * if sy < 0.0 { -1.0 } else { 1.0 }), t)?;
                        }
                        if let Some(v) = c2["angle"].as_f64() {
                            ed.change_in(s, &ids.2, Value::F(v as f32), t)?;
                        }
                        if let Some(v) = c2["turn"].as_f64() {
                            ed.change_in(s, &ids.2, Value::F(rot + v as f32), t)?;
                        }
                        if c2["center"].as_bool() == Some(true) && (x != 0.0 || y != 0.0) {
                            ed.change_in(s, &ids.0, Value::F(0.0), t)?;
                            ed.change_in(s, &ids.1, Value::F(0.0), t)?;
                        }
                        let layer = s.project.layer_mut(&sym, &l).expect("layer");
                        if let Some(v) = c2["locked"].as_bool() {
                            layer.locked = v;
                        }
                        if let Some(v) = c2["onion"].as_bool() {
                            layer.onion = v;
                        }
                        if let Some(v) = c2["color"].as_u64() {
                            let root = layer.as_figure().and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                            if let Some(root) = root {
                                figure_ops::modify_branch(s, &sym, &l, &root, &BranchChange::Color(Color::rgba8888(v as u32)))?;
                            }
                        }
                        Ok(())
                    });
                    self.validate();
                }
            }
            "node" => {
                // Segment Tools for the segment ending at the selected node
                if let Some((l, n)) = self.selected.clone() {
                    let c2 = c.clone();
                    let ed = self.tools.editor.clone();
                    self.step("Segment", move |s| {
                        if let Some(v) = c2["length"].as_f64() {
                            let id = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.node(&n)).map(|x| x.length.id.clone());
                            if let Some(id) = id {
                                ed.change_in(s, &id, Value::F(v.max(0.0) as f32), t)?;
                            }
                        }
                        if let Some(v) = c2["angle"].as_f64() {
                            let id = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.node(&n)).map(|x| x.rotation.id.clone());
                            if let Some(id) = id {
                                ed.change_in(s, &id, Value::F(v as f32), t)?;
                            }
                        }
                        let Some(layer) = s.project.layer_mut(&sym, &l) else { return Ok(()) };
                        let LayerKind::Figure(fig) = &mut layer.kind else { return Ok(()) };
                        let Some(node) = fig.node_mut(&n) else { return Ok(()) };
                        if let Some(v) = c2["stretchy"].as_bool() {
                            node.stretchy = v;
                        }
                        if let Some(v) = c2["dragLock"].as_bool() {
                            node.drag_lock = if v { crate::edit::canvas::DRAG_HORIZONTAL | crate::edit::canvas::DRAG_VERTICAL } else { 0 };
                        }
                        Ok(())
                    });
                }
            }
            "camera" => {
                let cam = self.project().root().and_then(|s| s.layers.iter().find(|l| matches!(l.kind, LayerKind::Camera(_))).map(|l| l.id.clone()));
                if let Some(id) = cam {
                    let c2 = c.clone();
                    let ed = self.tools.editor.clone();
                    self.step("Camera", move |s| {
                        let ids = {
                            let l = s.project.root().and_then(|r| r.layer(&id)).expect("camera");
                            let LayerKind::Camera(cm) = &l.kind else { return Ok(()) };
                            (cm.x.id.clone(), cm.y.id.clone(), cm.zoom.id.clone(), cm.rotation.id.clone())
                        };
                        if let Some(v) = c2["zoom"].as_f64() {
                            ed.change_in(s, &ids.2, Value::F(v.clamp(0.01, 100.0) as f32), t)?;
                        }
                        if let Some(v) = c2["rotation"].as_f64() {
                            ed.change_in(s, &ids.3, Value::F(v as f32), t)?;
                        }
                        if c2["center"].as_bool() == Some(true) {
                            ed.change_in(s, &ids.0, Value::F(0.0), t)?;
                            ed.change_in(s, &ids.1, Value::F(0.0), t)?;
                        }
                        Ok(())
                    });
                }
            }
            "copy_figure" => {
                if let Some(l) = self.sel_layer() {
                    let s0 = self.project().root().expect("root");
                    let ids = if b("joined") { layer_ops::with_joined(s0, &l) } else { vec![l.clone()] };
                    self.figure_clip = Some((ids.iter().filter_map(|i| s0.layer(i).map(|x| x.clone())).collect(), t));
                    self.message = "Stickfigure copied.".into();
                    self.rev += 1;
                }
            }
            "paste_figure" => {
                // Like the original: figures are matched across frames by id (that is
                // what tweens). Pasting onto a frame that does not have the copied
                // figure brings the same figure back there (it tweens with its
                // neighbours); where it is already present, the paste is a new figure.
                if let Some((clip, t_copy)) = self.figure_clip.clone() {
                    let fr_now = self.frame;
                    let made = self.step("Paste stickfigure", |s| {
                        let sym2 = s.project.root_symbol.clone();
                        let here = |s: &crate::edit::State, id: &str| s.project.root().and_then(|r| r.layer(id)).map(|x| Resolver::new(&s.project).b(&x.shown, t));
                        let same = clip.iter().all(|l| here(s, &l.id) == Some(false));
                        if same {
                            for l in &clip {
                                let vals: Vec<(String, Value)> = l.properties().iter().filter(|q| !q.track.is_empty()).map(|q| (q.id.clone(), q.key_value(t_copy))).collect();
                                for (pid, v) in vals {
                                    if let Some(q) = s.project.property_mut(&pid) {
                                        let interp = if matches!(v, Value::B(_)) { Interp::Hold } else { crate::edit::editor::interp_at(q, t, Interp::Linear) };
                                        q.track.put(Key::new(t, v, interp));
                                    }
                                }
                                if let Some(x) = s.project.layer_mut(&sym2, &l.id) {
                                    x.shown.track.put(Key::new(t, Value::B(true), Interp::Hold));
                                }
                            }
                            return Ok(clip.first().map(|l| l.id.clone()));
                        }
                        let mut ids = std::collections::HashMap::new();
                        for l in &clip {
                            let n = s.project.new_id("L");
                            ids.insert(l.id.clone(), n);
                        }
                        let mut first = None;
                        for l in &clip {
                            let mut c = structure::copy_layer(l, &ids)?;
                            // the new figure holds the copied pose
                            for q in c.properties_mut() {
                                if !q.track.is_empty() {
                                    let v = q.key_value(t_copy);
                                    q.track.clear();
                                    q.value = v;
                                }
                            }
                            let id = c.id.clone();
                            first.get_or_insert(id.clone());
                            let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                            structure::add_layer(s, &sym2, c, at)?;
                            only_on_frame(s, &sym2, &id, fr_now);
                        }
                        Ok(first)
                    });
                    if let Some(Some(id)) = made {
                        let root = self.project().root().and_then(|s| s.layer(&id)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                        self.selected = root.map(|r| (id, r));
                        self.validate();
                    }
                }
            }
            "join" => {
                self.joining = self.sel_layer();
                self.rev += 1;
            }
            "join_cancel" => {
                self.joining = None;
                self.rev += 1;
            }
            "unjoin" => {
                if let Some(l) = self.sel_layer() {
                    self.step("Unjoin", |s| crate::edit::join_ops::unjoin(s, &sym, &l, t));
                }
            }
            "copy_props" => {
                if let Some(l) = self.sel_layer() {
                    match layer_ops::copy_props(self.project(), &sym, &l, t) {
                        Ok(p) => {
                            self.props_clip = Some(p);
                            self.message = "Properties copied.".into();
                        }
                        Err(e) => self.message = e.0,
                    }
                    self.rev += 1;
                }
            }
            "paste_props" => {
                if let (Some(l), Some(pc)) = (self.sel_layer(), self.props_clip.clone()) {
                    let ed = self.tools.editor.clone();
                    let (pos, lay) = (c.get("position").and_then(|v| v.as_bool()).unwrap_or(true), c.get("layering").and_then(|v| v.as_bool()).unwrap_or(true));
                    self.step("Paste properties", |s| layer_ops::paste_props(&ed, s, &sym, &l, &pc, pos, lay, t));
                }
            }
            "copy_pose" => {
                if let Some(l) = self.sel_layer() {
                    let p = self.project();
                    if let Some(fig) = p.root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()) {
                        let mut r = Resolver::new(p);
                        let rot: IndexMap<String, f32> = fig.nodes().iter().filter(|n| n.parent.is_some()).map(|n| (n.id.clone(), r.f(&n.rotation, t))).collect();
                        let len: IndexMap<String, f32> = fig.nodes().iter().filter(|n| n.parent.is_some()).map(|n| (n.id.clone(), r.f(&n.length, t))).collect();
                        self.pose_clip = Some((rot, len));
                        self.message = "Pose copied.".into();
                    }
                    self.rev += 1;
                }
            }
            "paste_pose" => {
                if let (Some(l), Some((rot, len))) = (self.sel_layer(), self.pose_clip.clone()) {
                    let ed = self.tools.editor.clone();
                    self.doc.begin("Paste pose");
                    anchor_poses(&mut self.doc.state);
                    if let Err(e) = ed.pose(&mut self.doc, &sym, &l, &rot, &len, t) {
                        self.message = e.0;
                    }
                    self.doc.commit();
                    self.changed();
                }
            }
            "rename" => {
                if let Some(l) = self.sel_layer() {
                    let name = c["name"].as_str().unwrap_or("").trim().to_string();
                    self.step("Rename", |s| {
                        if let Some(x) = s.project.layer_mut(&sym, &l) {
                            x.name = if name.is_empty() { None } else { Some(name) };
                        }
                        Ok(())
                    });
                }
            }
            "add_camera" => {
                let has = self.project().root().is_some_and(|s| s.layers.iter().any(|l| matches!(l.kind, LayerKind::Camera(_))));
                if has {
                    self.message = "This animation already has a camera.".into();
                } else {
                    self.step("Add camera", |s| {
                        let id = s.project.new_id("L");
                        let l = Layer::camera(&id, Some("Camera"));
                        let sym = s.project.root_symbol.clone();
                        let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                        structure::add_layer(s, &sym, l, at)
                    });
                    self.mode = Mode::Camera;
                }
            }
            "reset_camera" => {
                let cam = self.project().root().and_then(|s| s.layers.iter().find(|l| matches!(l.kind, LayerKind::Camera(_))).map(|l| l.id.clone()));
                if let Some(id) = cam {
                    self.step("Reset camera", |s| {
                        let l = s.project.layer_mut(&sym, &id).expect("camera");
                        let LayerKind::Camera(c) = &mut l.kind else { return Ok(()) };
                        for (p, v) in [(&mut c.x, 0.0), (&mut c.y, 0.0), (&mut c.rotation, 0.0), (&mut c.zoom, 1.0)] {
                            p.track.clear();
                            p.value = Value::F(v);
                        }
                        Ok(())
                    });
                }
            }
            "frame_delay" | "frame_repeat" => {
                let c2 = c.clone();
                let delay = op == "frame_delay";
                self.step(if delay { "Frame delay" } else { "Repeat frames" }, move |s| {
                    let cam = ensure_camera(s, &sym)?;
                    let (n, fps) = (frame_ops::frame_count(s), st_fps(&s.project));
                    let l = s.project.layer_mut(&sym, &cam).expect("camera");
                    let LayerKind::Camera(cm) = &mut l.kind else { return crate::edit::fail("not a camera") };
                    let set = |q: &mut Property, v: f32| {
                        // every frame keeps its own setting (exact keys, HOLD)
                        for i in 0..n {
                            let ti = i as f64 / fps;
                            if q.track.index_of(ti).is_err() {
                                let old = crate::timing::setting(q, fps, i);
                                q.track.put(Key::new(ti, Value::F(old), Interp::Hold));
                            }
                        }
                        q.track.put(Key::new(t, Value::F(v), Interp::Hold));
                    };
                    if delay {
                        let v = c2["seconds"].as_f64().unwrap_or(0.0).clamp(0.0, crate::timing::MAX_DELAY as f64);
                        set(&mut cm.delay, ((v * 100.0).round() / 100.0) as f32);
                    } else {
                        let back = c2["back"].as_i64().unwrap_or(0).clamp(0, 100_000) as f32;
                        let times = c2["times"].as_i64().unwrap_or(1).clamp(1, 1000) as f32;
                        set(&mut cm.repeat_back, back);
                        set(&mut cm.repeat_times, if back > 0.0 { times } else { 0.0 });
                    }
                    Ok(())
                });
            }
            "frame_bg" => {
                let c2 = c.clone();
                self.step("Background colour", move |s| {
                    let cam = ensure_camera(s, &sym)?;
                    let (mut color, mut gradient, mut use_gradient) = frame_bg(&s.project, t);
                    if let Some(v) = c2["color"].as_u64() {
                        color = v as u32;
                    }
                    if let Some(v) = c2["gradient"].as_u64() {
                        gradient = v as u32;
                    }
                    if let Some(v) = c2["useGradient"].as_bool() {
                        use_gradient = v;
                    }
                    // no gradient: the fill is `background` and `background2` keeps the
                    // gradient colour with zero alpha; with one, top is the gradient colour
                    let (top, bottom) = if use_gradient { (gradient | 0xff, color | 0xff) } else { (color, gradient & !0xff) };
                    let (n, fps) = (frame_ops::frame_count(s), st_fps(&s.project));
                    let l = s.project.layer_mut(&sym, &cam).expect("camera");
                    let LayerKind::Camera(cm) = &mut l.kind else { return crate::edit::fail("not a camera") };
                    for (q, v) in [(&mut cm.background, top), (&mut cm.background2, bottom)] {
                        key_every_frame(q, n, fps);
                        let interp = q.track.keys().iter().find(|k| k.time == t).map(|k| k.interp).unwrap_or(Interp::Linear);
                        q.track.put(Key::new(t, Value::C(Color::rgba8888(v)), interp));
                    }
                    Ok(())
                });
            }
            "filter" => {
                if let Some(l) = self.sel_layer() {
                    let c2 = c.clone();
                    self.step("Stickfigure filters", move |s| crate::edit::filters::apply(s, &sym, &l, &c2, t));
                }
            }
            "clear_filters" => {
                if let Some(l) = self.sel_layer() {
                    self.step("Clear filters", |s| crate::edit::filters::clear(s, &sym, &l, t));
                }
            }
            "copy_filters" => {
                if let Some(l) = self.sel_layer() {
                    if let Some(snap) = crate::edit::filters::snapshot(self.project(), &sym, &l, t) {
                        self.filter_clip = Some(snap);
                        self.message = "Filters copied.".into();
                        self.rev += 1;
                    }
                }
            }
            "paste_filters" => {
                if let (Some(l), Some(snap)) = (self.sel_layer(), self.filter_clip.clone()) {
                    self.step("Paste filters", |s| {
                        crate::edit::filters::clear(s, &sym, &l, t)?;
                        crate::edit::filters::restore(s, &sym, &l, &snap, t)
                    });
                }
            }
            _ => self.message = format!("unknown command {op}"),
        }
        self.state()
    }

    /// Imports a Stick Nodes file (.nodes, .nodemc, .stknds, sounds, images)
    /// as one undo step; like an added figure it shows from this frame on.
    pub fn import(&mut self, file_name: &str, bytes: &[u8]) -> J {
        self.message.clear();
        if self.dragging() {
            self.end_gesture(false);
        }
        let fr_now = self.frame;
        let made = self.step("Import", |s| {
            let before: std::collections::HashSet<String> = s.project.root().map(|r| r.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
            let got = crate::import::import_file(&mut s.project, file_name, bytes).map_err(|e| crate::edit::EditError(e.to_string()))?;
            for (path, b) in got.assets {
                s.assets.insert(path, std::sync::Arc::new(b));
            }
            let sym = s.project.root_symbol.clone();
            let added: Vec<String> = s.project.root().map(|r| r.layers.iter().filter(|l| !before.contains(&l.id)).map(|l| l.id.clone()).collect()).unwrap_or_default();
            for id in &added {
                only_on_frame(s, &sym, id, fr_now);
            }
            Ok((got.layer, got.message))
        });
        if let Some((layer, msg)) = made {
            let root = self.project().root().and_then(|s| s.layer(&layer)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
            self.selected = root.map(|r| (layer, r));
            self.validate();
            if !msg.is_empty() {
                self.message = msg;
            }
        }
        self.state()
    }

    /// Everything the app draws besides the picture.
    pub fn state(&self) -> J {
        let p = self.project();
        let t = self.time();
        let mut handles = Vec::new();
        if self.mode == Mode::Figure {
            // joints connected to the selected one (its parent and children)
            let near: std::collections::HashSet<String> = match &self.selected {
                Some((l, n)) => self.tools.neighbours(p, l, n, false, t).into_iter().map(|h| h.node).collect(),
                None => Default::default(),
            };
            for h in self.tools.handles(p, t) {
                if self.building.as_ref().is_some_and(|b| *b != h.layer) {
                    continue;
                }
                let sel = self.selected.as_ref().is_some_and(|(l, n)| *l == h.layer && *n == h.node);
                let fig = self.selected.as_ref().is_some_and(|(l, _)| *l == h.layer);
                let near = fig && near.contains(&h.node);
                let flags = h.root as u32 | (sel as u32) << 1 | (fig as u32) << 2 | (near as u32) << 3;
                handles.push(json!([round2(h.x), round2(h.y), flags]));
            }
        }
        let frames = self.frame_count();
        let keyed = self.keyed_frames(frames);
        let figures: Vec<J> = p
            .root()
            .map(|s| s.layers.iter().filter(|l| l.as_figure().is_some()).map(|l| json!({"id": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone())})).collect())
            .unwrap_or_default();
        let sel = self.selected.as_ref().and_then(|(l, n)| {
            let layer = p.root()?.layer(l)?;
            let f = layer.as_figure()?;
            let node = f.node(n)?;
            let mut r = Resolver::new(p);
            Some(json!({
                "layer": l, "node": n, "name": layer.name.clone().unwrap_or_else(|| l.clone()),
                "root": node.parent.is_none(), "static": node.is_static, "pinned": node.pinned,
                "smart": node.smart_stretch, "shape": shape_name(node.shape),
                "thickness": r.f(&node.thickness, t), "color": color_of(&mut r, &node.color, t),
                "angle": round2(r.f(&node.rotation, t)), "length": round2(r.f(&node.length, t)),
                "stretchy": node.stretchy, "dragLock": node.drag_lock != 0,
                "scale": round2(r.f(&layer.transform.scale_y, t).abs()), "figAngle": round2(r.f(&layer.transform.rotation, t)),
                "locked": layer.locked, "onion": layer.onion,
                "joinedTo": layer.parent.as_ref().and_then(|pid| p.root()?.layer(pid)).map(|x| x.name.clone().unwrap_or_else(|| x.id.clone())),
                "figColor": f.nodes().get(1).map(|n| color_of(&mut r, &n.color, t)).unwrap_or(0x000000ff),
                "layerPos": p.root()?.layers.iter().filter(|x| x.as_figure().is_some()).position(|x| x.id == *l).map(|i| i + 1),
                "figureCount": p.root()?.layers.iter().filter(|x| x.as_figure().is_some()).count(),
                "filters": crate::edit::filters::state(p, &p.root_symbol, l, t),
            }))
        });
        let textfield = self.selected_text.as_ref().and_then(|id| {
            let layer = p.root()?.layer(id)?;
            let LayerKind::Text(tl) = &layer.kind else { return None };
            let mut r = Resolver::new(p);
            let (sx, sy) = (r.f(&layer.transform.scale_x, t).abs(), r.f(&layer.transform.scale_y, t).abs());
            let (cx, cy) = (r.f(&layer.transform.x, t), r.f(&layer.transform.y, t));
            let (w, h) = text_size(tl);
            let (x0, y0, x1, y1) = text_rect(tl, cx, cy, w * sx, h * sy);
            Some(json!({
                "layer": id, "name": layer.name.clone().unwrap_or_else(|| id.clone()),
                "text": tl.text.clone().unwrap_or_default(),
                "color": color_of(&mut r, &tl.color, t),
                "scale": round2(sy),
                "align": align_name(tl.align, tl.corner),
                "locked": layer.locked,
                "x": round2(cx), "y": round2(cy),
                "x0": round2(x0), "y0": round2(y0), "x1": round2(x1), "y1": round2(y1),
            }))
        });
        let bg = {
            let (c, g, u) = frame_bg(p, t);
            json!({"color": c, "gradient": g, "useGradient": u})
        };
        let fset = crate::timing::frame_settings(p, self.frame);
        // per thumbnail: [delay, repeat] flags
        let rep = json!({"back": fset.1, "times": fset.2});
        let marks: Vec<J> = (0..frames).map(|f| { let (d, _, n) = crate::timing::frame_settings(p, f as i32); json!([d > 0.0, n > 0]) }).collect();
        let cam = self.camera_frame();
        let cam_vals = p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).map(|c| {
            let mut r = Resolver::new(p);
            json!({"zoom": round2(r.f(&c.zoom, t)), "rotation": round2(r.f(&c.rotation, t))})
        });
        let no_tween = {
            let k = self.keys_at(t, None);
            !k.is_empty() && k.iter().all(|r| p.property(&r.property).and_then(|q| q.track.keys().iter().find(|x| x.time == r.time)).is_some_and(|x| x.interp == Interp::Hold))
        };
        json!({
            "rev": self.rev, "frame": self.frame, "frames": frames, "fps": p.settings.fps,
            "w": p.settings.width, "h": p.settings.height, "keyed": keyed,
            "canUndo": self.doc.can_undo(), "canRedo": self.doc.can_redo(),
            "undo": self.doc.undo_label(), "redo": self.doc.redo_label(), "dirty": self.doc.dirty(),
            "mode": match self.mode { Mode::Figure => "figure", Mode::All => "all", Mode::Camera => "camera" },
            "handles": handles, "selected": sel, "figures": figures, "camera": cam,
            "textfield": textfield, "textfieldCopied": self.textfield_clip.is_some(),
            "filtersCopied": self.filter_clip.is_some(),
            "frameBg": bg,
            "frameDelay": fset.0, "frameRepeat": rep,
            "frameMarks": marks,
            "hasCamera": p.root().is_some_and(|s| s.layers.iter().any(|l| matches!(l.kind, LayerKind::Camera(_)))),
            "opts": {
                "stretch": self.stretch_all, "scope": if self.tools.key_scope == KeyScope::Joint { "joint" } else { "figure" },
                "snap": self.tools.snap_degrees, "onion": self.onion, "axis": self.tools.axis_lock, "static": self.tools.show_static,
                "tweenNew": self.tools.editor.default_interp != Interp::Hold, "mainOnly": self.tools.main_nodes_only,
                "onionBefore": p.view.onion_before, "onionAfter": p.view.onion_after, "onionFront": p.view.onion_in_front,
            },
            "pose": self.pose_clip.is_some(),
            "joining": self.joining.is_some(),
            "frameCopied": self.frame_clip.is_some(),
            "figureCopied": self.figure_clip.is_some(), "propsCopied": self.props_clip.is_some(),
            "cam": cam_vals,
            "tweening": p.settings.tweening, "tweenedFrames": p.settings.tweened_frames, "rate": p.settings.sample_rate(),
            "frameNoTween": no_tween,
            "noTweenFrames": self.no_tween_frames(frames),
            "frameClipCount": self.frame_clip.as_ref().map(|c| c.1).unwrap_or(0),
            "building": self.building.as_ref().and_then(|b| p.root()?.layer(b)).map(|l| json!({"layer": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone())})),
            "builtins": crate::figures::ALL.iter().map(|(id, name)| json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "message": self.message,
        })
    }

    /// Which frames have keys, in one pass over every key (hidden figures' keys don't count).
    fn keyed_frames(&self, frames: i32) -> Vec<bool> {
        let p = self.project();
        let n = frames.clamp(0, 100_000) as usize;
        let mut out = vec![false; n];
        let fps = if p.settings.fps > 0.0 { p.settings.fps as f64 } else { 30.0 };
        let Some(s) = p.root() else { return out };
        let mut r = Resolver::new(p);
        for l in &s.layers {
            for q in l.properties() {
                for k in q.track.keys() {
                    let i = (k.time * fps).round();
                    if i < 0.0 || i as usize >= n || out[i as usize] {
                        continue;
                    }
                    if r.b(&l.shown, k.time) {
                        out[i as usize] = true;
                    }
                }
            }
        }
        out
    }

    /// Frames whose keys all hold (Do not tween this frame), one pass over every key.
    fn no_tween_frames(&self, frames: i32) -> Vec<bool> {
        let p = self.project();
        let n = frames.clamp(0, 100_000) as usize;
        let fps = st_fps(p);
        let (mut any, mut hold) = (vec![false; n], vec![true; n]);
        if let Some(s) = p.root() {
            for l in &s.layers {
                for q in l.properties() {
                    if !q.ty.continuous() {
                        continue;
                    }
                    for k in q.track.keys() {
                        let i = (k.time * fps).round();
                        if i >= 0.0 && (i as usize) < n {
                            any[i as usize] = true;
                            hold[i as usize] &= k.interp == Interp::Hold;
                        }
                    }
                }
            }
        }
        (0..n).map(|i| any[i] && hold[i]).collect()
    }

    /// Every key at time t (of one layer, or all), for easing.
    fn keys_at(&self, t: f64, only: Option<&str>) -> Vec<crate::edit::editor::KeyRef> {
        let p = self.project();
        let eps = 0.25 / p.settings.fps.max(1.0) as f64;
        let mut out = Vec::new();
        let Some(s) = p.root() else { return out };
        for l in &s.layers {
            if only.is_some_and(|o| o != l.id) {
                continue;
            }
            for q in l.properties() {
                for k in q.track.keys() {
                    if (k.time - t).abs() < eps {
                        out.push(crate::edit::editor::KeyRef { property: q.id.clone(), time: k.time });
                    }
                }
            }
        }
        out
    }

    /// The export frame on the stage: four corners (x, y) through the active camera.
    pub fn camera_frame(&self) -> Vec<f32> {
        let p = self.project();
        let (hw, hh) = (p.settings.width as f64 / 2.0, p.settings.height as f64 / 2.0);
        let view = p.root().map(|s| eval::camera_view(s, self.time(), &mut Resolver::new(p))).unwrap_or(Affine::IDENTITY);
        let inv = view.inverse();
        let mut out = Vec::new();
        for (x, y) in [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)] {
            out.push(inv.map_x(x, y) as f32);
            out.push(inv.map_y(x, y) as f32);
        }
        out
    }

    /// The project as a .bark file without marking it saved.
    pub fn save_copy(&self) -> Result<Vec<u8>, String> {
        let assets: IndexMap<String, Vec<u8>> = self.doc.state.assets.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
        crate::codec::write_bark(self.project(), &assets).map_err(|e| e.0)
    }

    /// The project as a .bark file; marks it saved.
    pub fn save(&mut self) -> Result<Vec<u8>, String> {
        self.doc.project_mut().view.frame = self.frame;
        let assets: IndexMap<String, Vec<u8>> = self.doc.state.assets.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
        let bytes = crate::codec::write_bark(self.project(), &assets).map_err(|e| e.0)?;
        self.doc.mark_saved();
        self.rev += 1;
        Ok(bytes)
    }
}

/// Keys a hair away from a frame (float error from shifting keys when
/// frames are inserted or deleted) are put exactly on the frame. When two
/// land on the same frame, the one that was keyed exactly there wins: the
/// other is a stale shifted copy (it made tweens snap back, "rubber-band").
/// Keys deliberately between frames are left alone.
pub fn snap_keys(st: &mut crate::edit::State) -> usize {
    let fps = st_fps(&st.project);
    let ids: Vec<String> = st.project.root().map(|s| s.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
    let sym = st.project.root_symbol.clone();
    let mut fixed = 0;
    for id in ids {
        let needs = {
            let Some(l) = st.project.root().and_then(|s| s.layer(&id)) else { continue };
            l.properties().iter().any(|q| q.track.keys().iter().any(|k| {
                let g = (k.time * fps).round() / fps;
                g != k.time && (g - k.time).abs() < 1e-4
            }))
        };
        if !needs {
            continue;
        }
        let Some(l) = st.project.layer_mut(&sym, &id) else { continue };
        for q in l.properties_mut() {
            let mut out: Vec<(f64, Key)> = Vec::new();
            for k in q.track.keys() {
                let g = (k.time * fps).round() / fps;
                let (t, off) = if (g - k.time).abs() < 1e-4 { (g, (g - k.time).abs()) } else { (k.time, 0.0) };
                match out.iter_mut().find(|(_, o)| o.time == t) {
                    Some((best, o)) => {
                        if off < *best {
                            *best = off;
                            *o = Key { time: t, ..*k };
                        }
                        fixed += 1;
                    }
                    None => {
                        if t != k.time {
                            fixed += 1;
                        }
                        out.push((off, Key { time: t, ..*k }));
                    }
                }
            }
            q.track.clear();
            for (_, k) in out {
                q.track.put(k);
            }
        }
    }
    fixed
}

/// Inserts one real frame per fraction after `frame`, each keyed with the
/// pose that fraction of the way to the next frame. Returns frames added.
pub fn add_tweened(st: &mut crate::edit::State, frame: i32, fracs: &[f64]) -> EditResult<i32> {
    let k = fracs.len() as i32;
    if k == 0 {
        return Ok(0);
    }
    anchor_poses(st);
    let (t0, t1) = (frame_ops::time(st, frame), frame_ops::time(st, frame + 1));
    // the poses, sampled before anything moves
    let mut poses: Vec<(String, Vec<Value>, Interp)> = Vec::new();
    if let Some(s) = st.project.root() {
        for l in &s.layers {
            for q in l.properties() {
                // frame delay / repeat stay with their own frame
                if q.track.is_empty() || frame_ops::is_frame_setting(&q.id) {
                    continue;
                }
                let vals = fracs.iter().map(|f| q.key_value(t0 + (t1 - t0) * f)).collect();
                poses.push((q.id.clone(), vals, crate::edit::editor::interp_at(q, t0, Interp::Linear)));
            }
        }
    }
    frame_ops::insert_tweened(st, frame, k)?;
    let fps = st_fps(&st.project);
    for (id, vals, interp) in poses {
        let Some(q) = st.project.property_mut(&id) else { continue };
        for (j, v) in vals.into_iter().enumerate() {
            let at = frame_ops::time_of(fps, frame + 1 + j as i32);
            q.track.put(Key::new(at, v, interp));
        }
    }
    Ok(k)
}

fn st_fps(p: &Project) -> f64 {
    if p.settings.fps > 0.0 { p.settings.fps as f64 } else { 30.0 }
}

/// Frames are snapshots (like the original): every figure's pose and its
/// visibility are keyed on every frame, so changing one frame can never
/// change another (tweening only blends neighbouring frames).
pub fn anchor_poses(st: &mut crate::edit::State) {
    let n = frame_ops::frame_count(st);
    let fps = st_fps(&st.project);
    let sym = st.project.root_symbol.clone();
    let ids: Vec<String> = st.project.root().map(|s| s.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
    for id in ids {
        let Some(l) = st.project.layer_mut(&sym, &id) else { continue };
        key_every_frame(&mut l.shown, n, fps);
        let tr = &mut l.transform;
        let mut ps: Vec<&mut Property> = vec![&mut tr.x, &mut tr.y, &mut tr.rotation, &mut tr.scale_x, &mut tr.scale_y];
        match &mut l.kind {
            LayerKind::Figure(f) => {
                for nd in f.nodes_mut() {
                    ps.push(&mut nd.rotation);
                    ps.push(&mut nd.length);
                }
            }
            LayerKind::Camera(c) => {
                ps.push(&mut c.x);
                ps.push(&mut c.y);
                ps.push(&mut c.zoom);
                ps.push(&mut c.rotation);
            }
            _ => {}
        }
        for q in ps {
            key_every_frame(q, n, fps);
        }
    }
}

fn key_every_frame(q: &mut Property, n: i32, fps: f64) {
    let hold = matches!(q.value, Value::B(_));
    for i in 0..n {
        let t = i as f64 / fps;
        if q.track.index_of(t).is_err() {
            let v = q.key_value(t);
            let interp = if hold { Interp::Hold } else { crate::edit::editor::interp_at(q, t, Interp::Linear) };
            q.track.put(Key::new(t, v, interp));
        }
    }
}

/// A figure (with what is joined to it) shown on one frame only.
fn only_on_frame(st: &mut crate::edit::State, sym: &str, id: &str, frame: i32) {
    let n = frame_ops::frame_count(st);
    let fps = st_fps(&st.project);
    let Some(l) = st.project.layer_mut(sym, id) else { return };
    l.shown.track.clear();
    for i in 0..n {
        l.shown.track.put(Key::new(i as f64 / fps, Value::B(i == frame), Interp::Hold));
    }
}


/// An estimate of a textfield's block size in stage units at scale 1 (the
/// engine's glyph atlas is not available to the session).
fn text_size(tl: &TextLayer) -> (f32, f32) {
    let size = tl.size.max(0.0);
    let text = tl.text.as_deref().unwrap_or("");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut w = 0f32;
    for l in &lines {
        w = w.max(l.chars().count() as f32 * size * 0.55);
    }
    (w.max(size * 0.55), lines.len() as f32 * size * 1.2)
}

fn text_hit(tl: &TextLayer, l: &Layer, r: &mut Resolver, t: f64, x: f32, y: f32, radius: f32) -> bool {
    let (sx, sy) = (r.f(&l.transform.scale_x, t).abs(), r.f(&l.transform.scale_y, t).abs());
    let (cx, cy) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
    let (w, h) = text_size(tl);
    let (x0, y0, x1, y1) = text_rect(tl, cx, cy, w * sx, h * sy);
    (x - x.clamp(x0, x1)).hypot(y - y.clamp(y0, y1)) <= radius
}

/// The box the text occupies, anchored by `corner` at (x, y).
fn text_rect(tl: &TextLayer, x: f32, y: f32, w: f32, h: f32) -> (f32, f32, f32, f32) {
    let left = tl.corner == 1 || tl.corner == 3;
    let right = tl.corner == 2 || tl.corner == 4;
    let top = if tl.corner == 1 || tl.corner == 2 {
        0.0
    } else if tl.corner == 3 || tl.corner == 4 {
        h
    } else {
        h / 2.0
    };
    let x0 = if left {
        x
    } else if right {
        x - w
    } else {
        x - w / 2.0
    };
    let y1 = y + top;
    (x0, y1 - h, x0 + w, y1)
}

fn align_name(align: i32, corner: i32) -> &'static str {
    match if align < 0 {
        match corner {
            1 | 3 => 0,
            2 | 4 => 2,
            _ => 1,
        }
    } else {
        align
    } {
        0 => "left",
        2 => "right",
        _ => "center",
    }
}

fn round2(v: f32) -> f64 {
    (v as f64 * 100.0).round() / 100.0
}

/// The root's camera, added (at the stage, changing nothing) when there is
/// none: it holds the per-frame settings (background, delay, repeat).
fn ensure_camera(s: &mut crate::edit::State, sym: &str) -> EditResult<String> {
    if let Some(id) = s.project.root().and_then(|r| r.layers.iter().find(|l| matches!(l.kind, LayerKind::Camera(_))).map(|l| l.id.clone())) {
        return Ok(id);
    }
    let id = s.project.new_id("L");
    let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
    structure::add_layer(s, sym, Layer::camera(&id, Some("Camera")), at)?;
    Ok(id)
}

/// The frame's background (colour, gradient colour, gradient on) at t, read
/// from the camera's two background colours (see the "frame_bg" command).
fn frame_bg(p: &Project, t: f64) -> (u32, u32, bool) {
    let cam = p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None }));
    let Some(c) = cam else { return (0xffffffff, 0x808080ff, false) };
    let mut r = Resolver::new(p);
    let (top, bottom) = (color_of(&mut r, &c.background, t), color_of(&mut r, &c.background2, t));
    if bottom & 0xff != 0 {
        (bottom, top, true)
    } else {
        let color = if top & 0xff == 0 { 0xffffffff } else { top };
        let gradient = if bottom == 0 { 0x808080ff } else { bottom | 0xff };
        (color, gradient, false)
    }
}

fn color_of(r: &mut Resolver, p: &Property, t: f64) -> u32 {
    match r.resolve(p, t) {
        Value::C(c) => c.to_rgba8888(),
        _ => 0x000000ff,
    }
}

fn set_static(p: &mut Property, v: Value) {
    if p.track.is_empty() {
        p.value = v;
    } else {
        // keyed: the change applies to every key (a segment property, not a pose)
        let keys: Vec<Key> = p.track.keys().iter().map(|k| Key { value: v, ..k.clone() }).collect();
        p.track.clear();
        for k in keys {
            p.track.put(k);
        }
        p.value = v;
    }
}

pub fn shape_name(s: Shape) -> &'static str {
    match s {
        Shape::None => "none",
        Shape::Line => "line",
        Shape::Circle => "circle",
        Shape::SquareLine => "square",
        Shape::Ellipse => "ellipse",
        Shape::Triangle => "triangle",
        Shape::Trapezoid => "trapezoid",
        Shape::Polygon => "polygon",
    }
}

pub fn shape_named(s: &str) -> Shape {
    Shape::ALL.into_iter().find(|x| shape_name(*x) == s).unwrap_or(Shape::Line)
}
