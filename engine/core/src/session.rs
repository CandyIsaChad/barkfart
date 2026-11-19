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
    pub onion: bool,
    gesture: Gesture,
    /// The last tap (no drag): where, and which handle, for node digging.
    last_tap: Option<(f32, f32, String, String)>,
    pose_clip: Option<(IndexMap<String, f32>, IndexMap<String, f32>)>,
    /// Bumped on every change the app should redraw for (drags included).
    rev: u64,
    pub message: String,
}

const STARTER: &str = include_str!("starter.json");

/// A new project: one frame, one figure (the Java version's new project).
pub fn new_project() -> Project {
    crate::codec::decode(STARTER).expect("starter project")
}

impl Session {
    pub fn new(doc: Document) -> Session {
        let mut tools = CanvasTools::default();
        tools.key_scope = KeyScope::Figure;
        let frame = doc.project().view.frame.max(0);
        let mut s = Session { doc, tools, frame, mode: Mode::Figure, selected: None, onion: true, gesture: Gesture::None, last_tap: None, pose_clip: None, rev: 1, message: String::new() };
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
            let ok = self.project().root().and_then(|s| s.layer(l)).and_then(|l| l.as_figure()).is_some_and(|f| f.node(node).is_some());
            if !ok {
                self.selected = None;
            }
        }
    }

    fn changed(&mut self) {
        self.rev += 1;
        self.validate();
    }

    fn figure_ids(&self) -> Vec<String> {
        self.project().root().map(|s| s.layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect()).unwrap_or_default()
    }

    fn sel_layer(&self) -> Option<String> {
        self.selected.as_ref().map(|s| s.0.clone())
    }

    /// Runs one edit as an undo step and reports failures as the message.
    fn step<T>(&mut self, label: &str, f: impl FnOnce(&mut crate::edit::State) -> EditResult<T>) -> Option<T> {
        let r = self.doc.edit(label, f);
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
                let sel = self.selected.clone();
                let hit = self.tools.pick_cycling(self.project(), x, y, radius, t, sel.as_ref().map(|(l, n)| (l.as_str(), n.as_str())), prev.as_ref().map(|(l, n)| (l.as_str(), n.as_str())));
                match hit {
                    Some(h) => {
                        self.selected = Some((h.layer.clone(), h.node.clone()));
                        self.gesture = Gesture::Handle { hit: h, x, y, slop: radius * 0.25, started: false };
                        self.rev += 1;
                        true
                    }
                    None => {
                        if self.selected.take().is_some() {
                            self.rev += 1;
                        }
                        self.last_tap = None;
                        false
                    }
                }
            }
            Mode::All => {
                self.doc.begin("Move everything");
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
                    self.tools.begin(&mut self.doc, &hit, x0, y0, t);
                    self.gesture = Gesture::Handle { hit, x: x0, y: y0, slop, started: true };
                }
                if let Err(e) = self.tools.move_to(&mut self.doc, x, y, t) {
                    self.message = e.0;
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
                    } else {
                        self.tools.end(&mut self.doc);
                    }
                    self.last_tap = None;
                } else {
                    self.last_tap = Some((x, y, hit.layer, hit.node));
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
                if self.step("Insert frame", |s| frame_ops::insert_after(s, fr, copy)).is_some() {
                    self.frame += 1;
                    self.changed();
                }
            }
            "delete_frame" => {
                let fr = self.frame;
                self.step("Delete frame", |s| frame_ops::delete(s, fr));
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
                    "stretch" => self.tools.stretch = b("v"),
                    "scope" => self.tools.key_scope = if c["v"].as_str() == Some("joint") { KeyScope::Joint } else { KeyScope::Figure },
                    "snap" => self.tools.snap_degrees = f("v").max(0.0),
                    "onion" => self.onion = b("v"),
                    "axis" => self.tools.axis_lock = b("v"),
                    "static" => self.tools.show_static = b("v"),
                    "tween_new" => self.tools.editor.default_interp = if b("v") { Interp::Linear } else { Interp::Hold },
                    _ => {}
                }
                self.rev += 1;
            }
            "select" => {
                let l = c["layer"].as_str().map(str::to_string);
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
            "add_figure" => {
                let n = self.figure_ids().len() + 1;
                let template = new_project();
                let src = template.root().and_then(|s| s.layers.first()).map(|l| (**l).clone()).expect("starter figure");
                let made = self.step("Add figure", |s| {
                    let id = s.project.new_id("L");
                    let mut ids = std::collections::HashMap::new();
                    ids.insert(src.id.clone(), id.clone());
                    let mut l = structure::copy_layer(&src, &ids)?;
                    l.name = Some(format!("Figure {n}"));
                    // offset so it does not sit exactly on the last one
                    l.transform.x.value = Value::F(40.0 * ((n as f32 - 1.0) % 6.0) - 100.0 * ((n > 1) as i32 as f32));
                    let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                    let sym = s.project.root_symbol.clone();
                    structure::add_layer(s, &sym, l, at)?;
                    Ok(id)
                });
                if let Some(id) = made {
                    self.selected = Some((id, "hip".into()));
                }
            }
            "delete_figure" => {
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
                    let r = self.tools.rotate_figure(&mut self.doc, &l, f("deg") as f64, t);
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
                    if let Err(e) = ed.pose(&mut self.doc, &sym, &l, &rot, &len, t) {
                        self.message = e.0;
                    }
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
            _ => self.message = format!("unknown command {op}"),
        }
        self.state()
    }

    /// Everything the app draws besides the picture.
    pub fn state(&self) -> J {
        let p = self.project();
        let t = self.time();
        let mut handles = Vec::new();
        if self.mode == Mode::Figure {
            for h in self.tools.handles(p, t) {
                let sel = self.selected.as_ref().is_some_and(|(l, n)| *l == h.layer && *n == h.node);
                let fig = self.selected.as_ref().is_some_and(|(l, _)| *l == h.layer);
                let flags = h.root as u32 | (sel as u32) << 1 | (fig as u32) << 2;
                handles.push(json!([round2(h.x), round2(h.y), flags]));
            }
        }
        let frames = self.frame_count();
        let keyed: Vec<bool> = (0..frames.min(4000)).map(|i| frame_ops::has_keys(&self.doc.state, i)).collect();
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
            }))
        });
        let cam = self.camera_frame();
        json!({
            "rev": self.rev, "frame": self.frame, "frames": frames, "fps": p.settings.fps,
            "w": p.settings.width, "h": p.settings.height, "keyed": keyed,
            "canUndo": self.doc.can_undo(), "canRedo": self.doc.can_redo(),
            "undo": self.doc.undo_label(), "redo": self.doc.redo_label(), "dirty": self.doc.dirty(),
            "mode": match self.mode { Mode::Figure => "figure", Mode::All => "all", Mode::Camera => "camera" },
            "handles": handles, "selected": sel, "figures": figures, "camera": cam,
            "hasCamera": p.root().is_some_and(|s| s.layers.iter().any(|l| matches!(l.kind, LayerKind::Camera(_)))),
            "opts": {
                "stretch": self.tools.stretch, "scope": if self.tools.key_scope == KeyScope::Joint { "joint" } else { "figure" },
                "snap": self.tools.snap_degrees, "onion": self.onion, "axis": self.tools.axis_lock, "static": self.tools.show_static,
                "tweenNew": self.tools.editor.default_interp != Interp::Hold,
            },
            "pose": self.pose_clip.is_some(),
            "message": self.message,
        })
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

fn round2(v: f32) -> f64 {
    (v as f64 * 100.0).round() / 100.0
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
