//! The editor session behind the app: the document with its history, the
//! canvas tools, the current frame, selection, mode and gestures. The app
//! talks to it with small JSON commands (see `command`) and draws what
//! `state` describes; every change is one undo step.
//!
//! Coordinates are stage units, y up (the camera is not applied while
//! editing; its frame is reported so the app can draw it).

use std::collections::HashMap;
use crate::edit::canvas::{CanvasTools, Handle, KeyScope};
use crate::edit::figure_ops::{self, BranchChange};
use crate::edit::{frame_ops, layer_ops, structure, Document, EditResult};
use crate::eval::{self, Affine, Resolver};
use crate::model::*;
use indexmap::IndexMap;
use serde_json::{json, Value as J};

/// Where a segment keeps its own colour while "Use segment color" is off (a
/// node extra, so it is saved with the project and undone with the step).
const KEPT_OWN_COLOR: &str = "B.ownColor";

/// The frame background's gradient colour until one is chosen.
const DEFAULT_GRADIENT: u32 = 0x808080ff;

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
    /// Creation mode: a new segment is being drawn from a joint.
    Draw { layer: String, from: String, x0: f32, y0: f32, x: f32, y: f32, slop: f32 },
    /// The selected picture is held inside its box: it moves (Picture board).
    Picture { layer: String, x: f32, y: f32, tx: f32, ty: f32, slop: f32, started: bool },
    /// A corner of the selected picture's box is held: the picture scales
    /// about the opposite corner, which stays put. `o` is its origin, `c`
    /// the fixed corner, `d0` the finger's first distance from it.
    PictureScale { layer: String, o: (f32, f32), c: (f32, f32), d0: f32, tx: f32, ty: f32, sx: f32, sy: f32, started: bool },
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
    /// K: the selected group (its folder row was tapped; the Group page).
    pub selected_group: Option<String>,
    pub onion: bool,
    /// Every drag stretches (the Drag: Stretch switch); otherwise only stretchy segments do.
    pub stretch_all: bool,
    /// "Join to...": the figure waiting for a joint to be tapped.
    pub joining: Option<String>,
    /// The figure open in the figure editor (structure tools, its joints only).
    pub building: Option<String>,
    /// Figure editor: the polyfill being edited (taps on joints add polynodes).
    pub polyfill: Option<String>,
    /// Figure editor "Add connector...": the next joint tapped is the endpoint.
    pub connecting: bool,
    /// Figure editor "Trace": a frame (index) or a figure (layer id) shown
    /// faintly behind, to draw over. Editing view only, never saved.
    pub trace: Option<(Option<i32>, Option<String>)>,
    /// Figure editor "Background color": the stage colour while editing a
    /// figure (view only).
    pub editor_bg: Option<u32>,
    /// Figure editor size reference (FigureEditor board): the stock figure
    /// (the app's default stickfigure; None: the built-in one) and whether
    /// it shows.
    pub stock: Option<std::sync::Arc<Project>>,
    pub size_ref: bool,
    /// Figure editor "Mirror" (D13): a segment drawn from a joint is drawn
    /// across the figure's centre line too. A drawing toggle only.
    pub mirror: bool,
    /// The main animation's symbol. While a movieclip is edited the project's
    /// root symbol is the movieclip's, so every tool works on its frames;
    /// saving always writes the main one as the root.
    pub main_root: String,
    /// Font metrics for textfield boxes, by font key ("" the default).
    pub fonts: HashMap<String, FontMetrics>,
    /// Creation mode's draw tool (None: select and move).
    pub draw: Option<Shape>,
    gesture: Gesture,
    /// The last tap (no drag): where, and which handle, for node digging.
    last_tap: Option<(f32, f32, String, String)>,
    pose_clip: Option<(IndexMap<String, f32>, IndexMap<String, f32>)>,
    /// "Copy poses" with several figures selected: each figure's pose, by
    /// layer; "Paste poses" puts each back on its own figure.
    poses_clip: Vec<(String, IndexMap<String, f32>, IndexMap<String, f32>)>,
    /// G5: several figures selected at once (layer ids, the first also in
    /// `selected`); empty when one or none is.
    multi: Vec<String>,
    /// M5: each sound's length in seconds, by asset path (decoded once).
    sound_seconds: std::cell::RefCell<HashMap<String, f64>>,
    /// D12 / J1: editing makes a new figure. While a copy is edited, the
    /// document as it was (restored at the end, Cancel or Save), the
    /// original layer, and the frame.
    copy_edit: Option<(Box<Document>, String, i32)>,
    /// The motion path on the stage (View › Motion path).
    pub motion_path: bool,
    /// D5 / H3: the keying toggle. Off (the default), a change to an effect
    /// or the opacity is the same on every frame; on, it is a key here.
    pub keying: bool,
    /// Copied stickfigure (with joined figures) and copied figure properties.
    /// Copied figures and the time they were copied at (their pose there).
    figure_clip: Option<(Vec<Layer>, f64)>,
    props_clip: Option<layer_ops::Props>,
    /// With copied properties: the figure colour and every joint's
    /// (id, angle, length, colour, thickness) for "Node properties".
    props_nodes: Option<(u32, Vec<(String, f32, f32, u32, f32)>)>,
    textfield_clip: Option<Layer>,
    filter_clip: Option<crate::edit::filters::Snapshot>,
    sound_clip: Option<Clip>,
    /// Figure editor: a copied segment or branch (figure order) and its polyfills.
    branch_clip: Option<(Vec<Node>, Vec<Polyfill>)>,
    /// Copied frame: its keys (times relative to the frame).
    frame_clip: Option<(crate::edit::editor::KeyClip, i32)>,
    /// The background look of each copied frame (a frame with no key of its own has the default one)
    frame_looks: Vec<J>,
    /// And each figure's filters on each copied frame
    frame_filters: Vec<Vec<(String, crate::edit::filters::Snapshot)>>,
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
        Ok(_) => {
            // the starter figure can be reset to how it came (D7b), and it
            // tweens as a new animation does (D34)
            if let Some(r) = p.root_mut() {
                for l in r.layers.iter_mut() {
                    let l = std::sync::Arc::make_mut(l);
                    l.capture_base();
                    smooth_tweens(l);
                }
            }
            p
        }
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
    // D34: the app's new animations tween smoothly (Stick Nodes files and the
    // reference starter keep Linear)
    p.settings.tween_interp = Interp::Smooth;
    if let Some(r) = p.root_mut() {
        r.duration = 0.1;
        // the starter figure can be reset to how it came (D7b)
        for l in r.layers.iter_mut() {
            let l = std::sync::Arc::make_mut(l);
            l.capture_base();
            smooth_tweens(l);
        }
    }
    p
}

/// D34: a layer's tweening keys (Linear, on a number or colour) become
/// Smooth: what a new animation's figures start with.
pub fn smooth_tweens(l: &mut Layer) {
    for q in l.properties_mut() {
        if !q.ty.continuous() || !q.track.keys().iter().any(|k| k.interp == Interp::Linear) {
            continue;
        }
        let keys: Vec<Key> = q.track.keys().iter().map(|k| if k.interp == Interp::Linear { Key { interp: Interp::Smooth, ..*k } } else { *k }).collect();
        q.track.clear();
        for k in keys {
            q.track.put(k);
        }
    }
}

impl Session {
    pub fn new(mut doc: Document) -> Session {
        doc.exact = true; // an edit that changes nothing is not an undo step
        let mut tools = CanvasTools::default();
        tools.layer_handles = true;
        tools.key_scope = KeyScope::Figure;
        // D34: a key on an empty track tweens as the project's tweens do
        tools.editor.default_interp = doc.project().settings.tween_interp;
        let frame = doc.project().view.frame.max(0);
        let mut doc = doc;
        // repair keys that drifted off their frames (older Barkfart builds)
        snap_keys(&mut doc.state);
        let main_root = doc.project().root_symbol.clone();
        let mut s = Session { main_root, fonts: HashMap::new(), doc, tools, frame, mode: Mode::Figure, selected: None, selected_text: None, selected_group: None, onion: true, stretch_all: false, joining: None, building: None, polyfill: None, connecting: false, trace: None, editor_bg: None, stock: None, size_ref: true, mirror: false, draw: None, gesture: Gesture::None, last_tap: None, pose_clip: None, poses_clip: Vec::new(), multi: Vec::new(), copy_edit: None, motion_path: false, keying: false, sound_seconds: Default::default(), frame_clip: None, frame_looks: Vec::new(), frame_filters: Vec::new(), figure_clip: None, props_clip: None, props_nodes: None, textfield_clip: None, filter_clip: None, sound_clip: None, branch_clip: None, rev: 1, message: String::new() };
        s.validate();
        s
    }

    pub fn project(&self) -> &Project {
        self.doc.project()
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }

    /// M5: a sound file's length in seconds (0 when it can't be read).
    pub fn sound_length(&self, asset: &str) -> f64 {
        let p = self.project();
        let Some(path) = p.assets.get(asset).map(|a| a.path.clone()) else { return 0.0 };
        if let Some(v) = self.sound_seconds.borrow().get(&path) {
            return *v;
        }
        let v = self.doc.state.assets.get(&path).and_then(|b| crate::audio::decode_sound(b)).map(|pcm| pcm.seconds()).unwrap_or(0.0);
        self.sound_seconds.borrow_mut().insert(path, v);
        v
    }

    /// M5: a sound's loudness in `n` parts between `from` and `to` seconds of
    /// its file (0..1), for the waveform on its row. Empty when it can't be read.
    pub fn sound_peaks(&self, asset: &str, from: f64, to: f64, n: usize) -> Vec<f32> {
        let p = self.project();
        let Some(path) = p.assets.get(asset).map(|a| a.path.clone()) else { return Vec::new() };
        let Some(bytes) = self.doc.state.assets.get(&path) else { return Vec::new() };
        let Some(pcm) = crate::audio::decode_sound(bytes) else { return Vec::new() };
        let ch = pcm.channels.max(1) as usize;
        let rate = pcm.rate.max(1) as f64;
        let frames = pcm.data.len() / ch;
        let (a, b) = (((from * rate) as usize).min(frames), ((to.max(from) * rate) as usize).min(frames));
        if b <= a || n == 0 {
            return Vec::new();
        }
        (0..n)
            .map(|i| {
                let (s0, s1) = (a + (b - a) * i / n, a + (b - a) * (i + 1) / n);
                let peak = pcm.data[s0 * ch..s1.max(s0 + 1).min(frames) * ch].iter().map(|v| (*v as i32).unsigned_abs()).max().unwrap_or(0);
                peak as f32 / 32768.0
            })
            .collect()
    }

    /// M4: one layer alone (no parent, no effects), for the pictures inside
    /// its timeline blocks. None when the layer doesn't exist.
    pub fn layer_alone(&self, id: &str) -> Option<Project> {
        let mut p = self.project().clone();
        let root = p.root_mut()?;
        let keep = root.layers.iter().find(|x| x.id == id)?.clone();
        let mut layer = (*keep).clone();
        layer.parent = None;
        layer.parent_node = None;
        layer.effects = Default::default();
        // shown on every frame here: a block only asks for frames it is on
        layer.shown.track.clear();
        layer.shown.value = Value::B(true);
        root.layers = vec![std::sync::Arc::new(layer)];
        Some(p)
    }

    /// K: a group's row in the timeline is a preview strip of its members
    /// merged: the project with every other drawn layer hidden (the camera,
    /// joins and the group's own opacity and effects as they are). None when
    /// there is no such group.
    pub fn group_alone(&self, gid: &str) -> Option<Project> {
        let mut p = self.project().clone();
        let root = p.root_mut()?;
        if !root.groups.iter().any(|g| g.id == gid) {
            return None;
        }
        for l in root.layers.iter_mut() {
            let draws = !matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_));
            if draws && l.group.as_deref() != Some(gid) && l.visible {
                std::sync::Arc::make_mut(l).visible = false;
            }
        }
        Some(p)
    }

    /// H2: the selected figure alone, as on this frame, with a filter command
    /// applied to a copy (the effects browser's tile previews). None without
    /// a selected figure. The copy is never part of the project or its history.
    pub fn effect_preview(&self, filter: &J) -> Option<(Project, f64)> {
        let (l, _) = self.selected.clone()?;
        let t = self.time();
        let mut p = self.project().clone();
        let sym = p.root_symbol.clone();
        {
            let root = p.root_mut()?;
            let keep = root.layers.iter().find(|x| x.id == l)?.clone();
            let mut layer = (*keep).clone();
            // placed by its own transform (a joined figure is fitted anyway)
            layer.parent = None;
            layer.parent_node = None;
            root.layers = vec![std::sync::Arc::new(layer)];
        }
        let mut st = crate::edit::State { project: p, assets: Default::default() };
        if filter.is_object() {
            crate::edit::filters::apply(&mut st, &sym, &l, filter, t).ok()?;
        }
        Some((st.project, t))
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
            // a figure's joint, or (no joint) a movieclip or sprite
            let ok = p.root().and_then(|s| s.layer(l)).filter(|x| Resolver::new(p).b(&x.shown, t)).is_some_and(|x| match &x.kind {
                LayerKind::Figure(f) => f.node(node).is_some(),
                LayerKind::Symbol(_) | LayerKind::Sprite(_) => node.is_empty(),
                _ => false,
            });
            if !ok {
                self.selected = None;
            }
        }
        if let Some(g) = &self.selected_group {
            // anything else selected since (a new figure, a cycle, a paste) wins
            let other = self.selected.is_some() || self.selected_text.is_some() || !self.multi.is_empty();
            if other || !self.project().root().is_some_and(|r| r.groups.iter().any(|x| x.id == *g)) {
                self.selected_group = None;
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
            if text_hit(tl, l, &mut r, t, x, y, radius, &self.fonts) {
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
        r.b(&l.shown, t) && text_hit(tl, l, &mut r, t, x, y, radius, &self.fonts)
    }

    /// A move on the stage as a change of the layer's own x and y: through
    /// the inverse of its parent chain (a picture locked to a zoomed or
    /// turned camera moves with the finger, not at the camera's rate).
    fn to_parent(&self, layer: &str, dx: f32, dy: f32, t: f64) -> (f32, f32) {
        let p = self.project();
        let Some((s, l)) = p.root().and_then(|s| s.layer(layer).map(|l| (s, l))) else { return (dx, dy) };
        let parent = eval::layer_stage(p, s, l, t).mul(&eval::local_transform(p, l, t).inverse());
        let inv = parent.inverse();
        let (ox, oy) = (inv.map_x(0.0, 0.0), inv.map_y(0.0, 0.0));
        let (x, y) = (inv.map_x(dx as f64, dy as f64) - ox, inv.map_y(dx as f64, dy as f64) - oy);
        if x.is_finite() && y.is_finite() {
            (x as f32, y as f32)
        } else {
            (dx, dy)
        }
    }

    /// A press on the selected picture (Picture board): near a corner of its
    /// box, scaling about the opposite corner; inside the box, moving it.
    /// Locked pictures stay put.
    fn picture_press(&self, x: f32, y: f32, radius: f32, t: f64) -> Option<Gesture> {
        if self.building.is_some() {
            return None;
        }
        let (layer, node) = self.selected.as_ref()?;
        if !node.is_empty() {
            return None;
        }
        let p = self.project();
        let l = p.root()?.layer(layer)?;
        let mut r = Resolver::new(p);
        if l.locked || !l.visible || !r.b(&l.shown, t) || !matches!(l.kind, LayerKind::Sprite(_)) {
            return None;
        }
        let b = self.tools.sprite_box(p, layer, t)?;
        let (tx, ty) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
        let corner = (0..4).map(|i| (i, (b[i * 2] - x).hypot(b[i * 2 + 1] - y))).filter(|(_, d)| *d <= radius).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((i, _)) = corner {
            let j = (i + 2) % 4;
            let c = (b[j * 2], b[j * 2 + 1]);
            let d0 = (x - c.0).hypot(y - c.1);
            if d0 > 1.0 {
                // the point scaling turns about: the layer's anchor, found in the box
                let LayerKind::Sprite(sp) = &l.kind else { return None };
                let (u, v) = (r.f(&l.transform.anchor_x, t) / sp.width.max(1e-3), r.f(&l.transform.anchor_y, t) / sp.height.max(1e-3));
                let o = (b[0] + (b[2] - b[0]) * u + (b[6] - b[0]) * v, b[1] + (b[3] - b[1]) * u + (b[7] - b[1]) * v);
                let (sx, sy) = (r.f(&l.transform.scale_x, t), r.f(&l.transform.scale_y, t));
                return Some(Gesture::PictureScale { layer: layer.clone(), o, c, d0, tx, ty, sx, sy, started: false });
            }
        }
        // inside the box (a quad, maybe distorted): on the same side of every edge
        let side = |i: usize| {
            let (ax, ay, bx, by) = (b[i * 2], b[i * 2 + 1], b[(i + 1) % 4 * 2], b[(i + 1) % 4 * 2 + 1]);
            (bx - ax) * (y - ay) - (by - ay) * (x - ax)
        };
        let s: Vec<f32> = (0..4).map(side).collect();
        let inside = s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0);
        inside.then(|| Gesture::Picture { layer: layer.clone(), x, y, tx, ty, slop: radius * 0.25, started: false })
    }

    /// Runs one edit as an undo step and reports failures as the message.
    fn step<T>(&mut self, label: &str, f: impl FnOnce(&mut crate::edit::State) -> EditResult<T>) -> Option<T> {
        let r = self.doc.edit(label, |s| {
            anchor_poses(s);
            let v = f(s)?;
            // keys shifted by a frame operation sit a hair off their frames (0.2 + 0.1 is
            // not 0.3): put them on the frames before anchoring keys the gaps, or the gap
            // filler (a held value from the key before) wins over the shifted key
            snap_keys(s);
            anchor_poses(s);
            snap_keys(s);
            // K / D33: groups stay together, one level, never empty
            crate::edit::group_ops::tidy(s);
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
                // any press on the stage ends a group's selection
                if self.selected_group.take().is_some() {
                    self.rev += 1;
                }
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
                        // D8: joins are moments: from this frame on (on the first
                        // frame, for the whole animation, as before)
                        if t > 1e-9 {
                            self.step("Join", |s| crate::edit::join_ops::switch_at(s, &sym, &child, Some(&h.layer), Some(&h.node), true, t));
                        } else {
                            self.step("Join", |s| crate::edit::join_ops::join(s, &sym, &child, &h.layer, Some(&h.node), true, t));
                        }
                        self.joining = None;
                        self.selected = self.project().root().and_then(|s| s.layer(&child)).and_then(|l| l.as_figure()).and_then(|f| f.nodes().first().map(|n| (child.clone(), n.id.clone())));
                    } else {
                        self.message = "Tap a joint on another figure.".into();
                        self.rev += 1;
                    }
                    return true;
                }
                // Add connector: the tapped joint is the endpoint; a new joint on the
                // straight line from the selected one to it (halfway) is the connector
                if let (Some(b), true, Some((sl, sn))) = (self.building.clone(), self.connecting, self.selected.clone()) {
                    let near = self.tools.handles(self.project(), t).into_iter().filter(|h| h.layer == b && h.node != sn && (h.x - x).hypot(h.y - y) <= radius).min_by(|p, q| {
                        let d = |h: &Handle| (h.x - x).hypot(h.y - y);
                        d(p).partial_cmp(&d(q)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    if let (Some(h), true) = (near, sl == b) {
                        let sym = self.project().root_symbol.clone();
                        let made = self.step("Add connector", |s| {
                            let id = figure_ops::add_segment(s, &sym, &b, &sn, 30.0, 40.0)?;
                            let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &b).map(|l| &mut l.kind) else { return crate::edit::fail("no figure") };
                            let n = f.node_mut(&id).expect("new joint");
                            n.legacy_extras.insert(crate::eval::pose::CONNECT_TO.into(), format!("s:{}", h.node));
                            n.legacy_extras.insert("C.cMethod".into(), "f:1".into());
                            n.legacy_extras.insert("C.cPercent".into(), "f:0.5".into());
                            Ok(id)
                        });
                        self.connecting = false;
                        if let Some(id) = made {
                            self.selected = Some((b, id));
                        }
                    } else {
                        self.message = "Tap another joint of this figure.".into();
                        self.rev += 1;
                    }
                    return true;
                }
                // Polyfill Tools: a tap on a joint of this figure adds it as a polynode
                if let (Some(b), Some(pf)) = (self.building.clone(), self.polyfill.clone()) {
                    let near = self.tools.handles(self.project(), t).into_iter().filter(|h| h.layer == b && (h.x - x).hypot(h.y - y) <= radius).min_by(|p, q| {
                        let d = |h: &Handle| (h.x - x).hypot(h.y - y);
                        d(p).partial_cmp(&d(q)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    if let Some(h) = near {
                        let sym = self.project().root_symbol.clone();
                        self.step("Add polynode", |s| {
                            let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &b).map(|l| &mut l.kind) else { return crate::edit::fail("no figure") };
                            let Some(fill) = f.polyfills.iter_mut().find(|p| p.id == pf) else { return crate::edit::fail("no polyfill") };
                            fill.nodes.push(h.node.clone());
                            Ok(())
                        });
                    }
                    return true;
                }
                // creation mode with a draw tool: a drag from a joint draws a segment
                if let (Some(b), Some(_)) = (self.building.clone(), self.draw) {
                    // the nearest joint of the figure being edited (other figures' joints never take the press)
                    let near = self.tools.handles(self.project(), t).into_iter().filter(|h| h.layer == b && (h.x - x).hypot(h.y - y) <= radius).min_by(|p, q| {
                        let d = |h: &Handle| (h.x - x).hypot(h.y - y);
                        d(p).partial_cmp(&d(q)).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    if let Some(h) = near {
                        self.selected = Some((b.clone(), h.node.clone()));
                        self.gesture = Gesture::Draw { layer: b, from: h.node, x0: h.x, y0: h.y, x, y, slop: radius * 0.5 };
                        self.rev += 1;
                        return true;
                    }
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
                    // the selected picture (Picture board), under any joint: a
                    // corner scales it, inside its box moves it
                    None if self.picture_press(x, y, radius, t).is_some() => {
                        self.gesture = self.picture_press(x, y, radius, t).expect("just found");
                        self.last_tap = None;
                        true
                    }
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
            Gesture::Draw { layer, from, x0, y0, slop, .. } => {
                self.gesture = Gesture::Draw { layer, from, x0, y0, x, y, slop };
                self.rev += 1;
            }
            Gesture::Picture { layer, x: x0, y: y0, tx, ty, slop, started } => {
                if !started {
                    if (x - x0).hypot(y - y0) < slop {
                        return;
                    }
                    self.doc.begin("Move picture");
                    anchor_poses(&mut self.doc.state);
                    self.gesture = Gesture::Picture { layer: layer.clone(), x: x0, y: y0, tx, ty, slop, started: true };
                }
                let ed = self.tools.editor.clone();
                let (dx, dy) = self.to_parent(&layer, x - x0, y - y0, t);
                let ids = self.project().root().and_then(|s| s.layer(&layer)).map(|l| (l.transform.x.id.clone(), l.transform.y.id.clone()));
                if let Some((xi, yi)) = ids {
                    let _ = ed.change_in(&mut self.doc.state, &xi, Value::F(tx + dx), t);
                    let _ = ed.change_in(&mut self.doc.state, &yi, Value::F(ty + dy), t);
                }
                self.rev += 1;
            }
            Gesture::PictureScale { layer, o, c, d0, tx, ty, sx, sy, started } => {
                if !started {
                    self.doc.begin("Scale picture");
                    anchor_poses(&mut self.doc.state);
                    self.gesture = Gesture::PictureScale { layer: layer.clone(), o, c, d0, tx, ty, sx, sy, started: true };
                }
                // never smaller than a twentieth, never turned inside out
                let k = ((x - c.0).hypot(y - c.1) / d0.max(1e-3)).max(0.05);
                let ed = self.tools.editor.clone();
                let ids = self.project().root().and_then(|s| s.layer(&layer)).map(|l| [l.transform.x.id.clone(), l.transform.y.id.clone(), l.transform.scale_x.id.clone(), l.transform.scale_y.id.clone()]);
                if let Some([xi, yi, sxi, syi]) = ids {
                    // the origin moves so the held-still corner stays where it was
                    let (nx, ny) = (c.0 - (c.0 - o.0) * k, c.1 - (c.1 - o.1) * k);
                    let (dx, dy) = self.to_parent(&layer, nx - o.0, ny - o.1, t);
                    let _ = ed.change_in(&mut self.doc.state, &xi, Value::F(tx + dx), t);
                    let _ = ed.change_in(&mut self.doc.state, &yi, Value::F(ty + dy), t);
                    let _ = ed.change_in(&mut self.doc.state, &sxi, Value::F(sx * k), t);
                    let _ = ed.change_in(&mut self.doc.state, &syi, Value::F(sy * k), t);
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
            Gesture::Draw { layer, from, x0, y0, x, y, slop } => {
                if !cancel && (x - x0).hypot(y - y0) >= slop {
                    let shape = self.draw.unwrap_or(Shape::Line);
                    let sym = self.sym();
                    let t = self.time();
                    let twin = if self.mirror { mirrored(self.project(), &sym, &layer, &from, x, y, t) } else { None };
                    let made = self.step("Draw segment", |s| {
                        let id = draw_segment(s, &sym, &layer, &from, shape, x, y, t)?;
                        if let Some((from2, _, _, x2, y2)) = &twin {
                            draw_segment(s, &sym, &layer, from2, shape, *x2, *y2, t)?;
                        }
                        Ok(id)
                    });
                    if let Some(id) = made {
                        self.selected = Some((layer, id));
                    }
                }
                self.rev += 1;
            }
            Gesture::Picture { started, .. } | Gesture::PictureScale { started, .. } => {
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
                    snap_keys(s);
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
                let (tw, n, lp) = (c["tweening"].as_bool(), c["tweened"].as_i64(), c["loop"].as_bool());
                self.step("Animation settings", |s| {
                    if let Some(v) = tw {
                        s.project.settings.tweening = v;
                    }
                    if let Some(v) = lp {
                        s.project.settings.loop_tween = v;
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
                // a frame tool: every figure on the frame holds (or tweens), so
                // each is keyed there first (F3: figures are keyed on their own)
                let (fr, on, ed) = (self.frame, b("on"), self.tools.editor.clone());
                self.step(if on { "Do not tween" } else { "Tween" }, |s| {
                    key_whole_frame(s, fr);
                    frame_ops::set_tween(&ed, s, fr, !on)
                });
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
                self.frame_looks = (0..=n).map(|k| frame_look(self.project(), self.frame + k)).collect();
                let filters: Vec<Vec<(String, crate::edit::filters::Snapshot)>> = {
                    let (p, sym) = (self.project(), self.sym());
                    let figures: Vec<String> = p.root().map(|r| r.layers.iter().filter(|l| l.as_figure().is_some()).map(|l| l.id.clone()).collect()).unwrap_or_default();
                    (0..=n).map(|k| figures.iter().filter_map(|id| crate::edit::filters::snapshot(p, &sym, id, t + k as f64 / fps).map(|sn| (id.clone(), sn))).collect()).collect()
                };
                self.frame_filters = filters;
                self.message = if n == 0 { "Frame copied.".into() } else { format!("{} frames copied.", n + 1) };
                self.rev += 1;
            }
            "paste_frame" => {
                if let Some((clip, count)) = self.frame_clip.clone() {
                    let looks = self.frame_looks.clone();
                    let filters = self.frame_filters.clone();
                    let fr = self.frame;
                    let made = self.step("Paste frame", |s| {
                        for k in 0..count {
                            frame_ops::insert_after(s, fr + k, true)?;
                        }
                        let at = frame_ops::time(s, fr + 1);
                        let fps = st_fps(&s.project);
                        for (id, keys) in &clip.keys {
                            // an auto-camera's markers belong to its span, not to a copy
                            if id.ends_with("/cam/autoCam") || id.ends_with("/cam/autoCamEnd") {
                                continue;
                            }
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
                        // a copied frame with no background key had the default one: a pasted
                        // frame has the look of the frame it was copied from, not of the one
                        // it was inserted after
                        // and so are the figures' filters: a filter set up since the copy is not on it
                        for (k, list) in filters.iter().enumerate().take(count as usize) {
                            let tk = frame_ops::time(s, fr + 1 + k as i32);
                            for (lid, snap) in list {
                                if s.project.root().and_then(|r| r.layer(lid)).is_none() {
                                    continue;
                                }
                                if crate::edit::filters::snapshot(&s.project, &sym, lid, tk).as_ref() != Some(snap) {
                                    crate::edit::filters::clear(s, &sym, lid, tk)?;
                                    crate::edit::filters::restore(s, &sym, lid, snap, tk)?;
                                }
                            }
                        }
                        let has_camera = s.project.root().is_some_and(|r| r.layers.iter().any(|l| matches!(l.kind, LayerKind::Camera(_))));
                        for (k, look) in looks.iter().enumerate().take(count as usize) {
                            if has_camera && frame_look(&s.project, fr + 1 + k as i32) != *look {
                                set_frame_bg(s, &sym, frame_ops::time(s, fr + 1 + k as i32), look)?;
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
                    // D34: the curve through the poses (the app's default)
                    "auto" => (Interp::Smooth, Ease::LINEAR),
                    "smooth" => (Interp::Bezier, Ease::new(0.42, 0.0, 0.58, 1.0)),
                    "in" => (Interp::Bezier, Ease::new(0.42, 0.0, 1.0, 1.0)),
                    "out" => (Interp::Bezier, Ease::new(0.0, 0.0, 0.58, 1.0)),
                    // the Pose page's fourth ease (round 10 board)
                    "bounce" => (Interp::Bezier, Ease { kind: EaseKind::BounceOut, ..Ease::LINEAR }),
                    _ => (Interp::Linear, Ease::LINEAR),
                };
                // the Pose page names its pose's first frame; "block": the selected
                // figure's pose under the playhead (several figures: each its own)
                let fps_now = st_fps(self.project());
                let block_start = (c["block"].as_bool() == Some(true))
                    .then(|| self.sel_layer().and_then(|l| crate::edit::block_ops::block_at(self.project().root()?.layer(&l)?, fps_now, self.frame).map(|(st, _)| st)))
                    .flatten();
                let at = c["frame"].as_i64().map(|f| f as f64 / fps_now).or(block_start.map(|f| f as f64 / fps_now)).unwrap_or(t);
                let sel = self.keys_at(at, self.sel_layer().as_deref());
                if sel.is_empty() {
                    self.message = "Nothing is keyed on this frame.".into();
                } else {
                    let ed = self.tools.editor.clone();
                    self.step("Easing", |s| ed.set_interp_in(s, &sel, interp, ease));
                }
            }
            "handles" => {
                // D34: the handles of the pose's keys (like "easing": a frame,
                // the selected figure's pose under the playhead, or this frame)
                let Some(handle) = c["v"].as_str().and_then(HandleType::from_name) else {
                    self.message = "Unknown handle type.".into();
                    return self.state();
                };
                let fps_now = st_fps(self.project());
                let block_start = (c["block"].as_bool() == Some(true))
                    .then(|| self.sel_layer().and_then(|l| crate::edit::block_ops::block_at(self.project().root()?.layer(&l)?, fps_now, self.frame).map(|(st, _)| st)))
                    .flatten();
                let at = c["frame"].as_i64().map(|f| f as f64 / fps_now).or(block_start.map(|f| f as f64 / fps_now)).unwrap_or(t);
                let sel = self.keys_at(at, self.sel_layer().as_deref());
                if sel.is_empty() {
                    self.message = "Nothing is keyed on this frame.".into();
                } else {
                    let ed = self.tools.editor.clone();
                    self.step("Handles", |s| ed.set_handles_in(s, &sel, handle));
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
                    // the frames' own sounds follow their frames like the keys (other sound
                    // layers stay where they are on the timeline)
                    let sound_layers: Vec<String> = s.project.root().map(|r| r.layers.iter().filter(|l| matches!(l.kind, LayerKind::Audio(_)) && l.name.as_deref() == Some(frame_ops::FRAME_SOUNDS)).map(|l| l.id.clone()).collect()).unwrap_or_default();
                    for id in sound_layers {
                        if let Some(LayerKind::Audio(clips)) = s.project.layer_mut(&root, &id).map(|l| &mut l.kind) {
                            for c in clips.iter_mut() {
                                c.start *= k;
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
                    "tween_new" => self.tools.editor.default_interp = if b("v") { self.project().settings.tween_interp } else { Interp::Hold },
                    "keying" => self.keying = b("v"),
                    "path" => self.motion_path = b("v"),
                    "sizeref" => self.size_ref = b("v"),
                    "mirror" => self.mirror = b("v"),
                    _ => {}
                }
                self.rev += 1;
            }
            "select_many" => {
                self.selected_group = None;
                // G5: the selection box or lasso; figures only, in row order
                let p = self.project();
                let ids: Vec<String> = c["layers"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).filter(|id| p.root().and_then(|r| r.layer(id)).is_some_and(|l| l.as_figure().is_some())).map(str::to_string).collect()).unwrap_or_default();
                let mut seen = std::collections::HashSet::new();
                let ids: Vec<String> = ids.into_iter().filter(|x| seen.insert(x.clone())).collect();
                let first = ids.first().and_then(|l| p.root()?.layer(l)?.as_figure()?.nodes().first().map(|n| (l.clone(), n.id.clone())));
                self.selected_text = None;
                self.selected = first;
                self.multi = if ids.len() > 1 { ids } else { Vec::new() };
                self.rev += 1;
            }
            "each" => {
                // G5: one command for every selected figure, as one step. A
                // figure it cannot apply to is skipped (a New pose where it has
                // one already); an error anywhere undoes the lot.
                let inner = c["cmd"].clone();
                let iop = inner["op"].as_str().unwrap_or("").to_string();
                let label = c["label"].as_str().unwrap_or("Change figures").to_string();
                let list: Vec<String> = if self.multi.len() > 1 { self.multi.clone() } else { self.sel_layer().into_iter().collect() };
                let (keep, keep_multi) = (self.selected.clone(), self.multi.clone());
                let fps = st_fps(self.project());
                let fr = self.frame;
                self.doc.begin(&label);
                let mut said = String::new();
                for l in &list {
                    let Some(layer) = self.project().root().and_then(|r| r.layer(l)).cloned() else { continue };
                    if iop == "new_pose" && crate::edit::block_ops::pose_frames(&layer, fps).contains(&fr) {
                        continue;
                    }
                    let Some(root) = layer.as_figure().and_then(|f| f.nodes().first().map(|n| n.id.clone())) else { continue };
                    self.selected = Some((l.clone(), root));
                    self.command(&inner);
                    if said.is_empty() {
                        said = self.message.clone();
                    }
                    if !self.doc.in_transaction() {
                        break; // an error cancelled the group
                    }
                }
                if self.doc.in_transaction() {
                    self.doc.commit();
                }
                let alive = |s: &Self, id: &str| s.project().root().and_then(|r| r.layer(id)).is_some();
                self.multi = keep_multi.into_iter().filter(|id| alive(self, id)).collect();
                if self.multi.len() < 2 {
                    self.multi.clear();
                }
                self.selected = keep.filter(|(id, _)| alive(self, id)).or_else(|| self.multi.first().and_then(|l| self.project().root()?.layer(l)?.as_figure()?.nodes().first().map(|n| (l.clone(), n.id.clone()))));
                self.message = said;
                self.changed();
                self.validate();
            }
            "select" => {
                let l = c["layer"].as_str().map(str::to_string);
                self.multi.clear();
                self.selected_text = None;
                // K: a group's folder row selects the group (the Group page)
                self.selected_group = c["group"].as_str().filter(|g| self.project().root().is_some_and(|r| r.groups.iter().any(|x| x.id == *g))).map(str::to_string);
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
            "nudge" => {
                // Arrows window: the selected figure (or textfield) moves by (dx, dy)
                let (dx, dy) = (c["dx"].as_f64().unwrap_or(0.0) as f32, c["dy"].as_f64().unwrap_or(0.0) as f32);
                if let Some(id) = self.selected_text.clone() {
                    let ed = self.tools.editor.clone();
                    self.step("Nudge textfield", |s| {
                        anchor_poses(s);
                        let l = s.project.layer_mut(&sym, &id).ok_or_else(|| crate::edit::EditError("no textfield".into()))?;
                        let (xi, yi) = (l.transform.x.id.clone(), l.transform.y.id.clone());
                        let mut r = Resolver::new(&s.project);
                        let l = s.project.root().and_then(|x| x.layer(&id)).expect("textfield");
                        let (x, y) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
                        ed.change_in(s, &xi, Value::F(x + dx), t)?;
                        ed.change_in(s, &yi, Value::F(y + dy), t)?;
                        Ok(())
                    });
                } else if let Some(l) = self.sel_layer() {
                    let root = self.tools.handles(self.project(), t).into_iter().find(|h| h.layer == l && h.root);
                    if let (Some(h), false) = (root, self.dragging()) {
                        self.doc.begin("Nudge figure");
                        anchor_poses(&mut self.doc.state);
                        let stretch = self.tools.stretch;
                        self.tools.stretch = false;
                        self.tools.begin(&mut self.doc, &h, h.x, h.y, t);
                        let r = self.tools.move_to(&mut self.doc, h.x + dx, h.y + dy, t);
                        self.tools.end(&mut self.doc);
                        self.tools.stretch = stretch;
                        match r {
                            Ok(()) => {
                                anchor_poses(&mut self.doc.state);
                                snap_keys(&mut self.doc.state);
                                self.doc.commit();
                            }
                            Err(e) => {
                                self.doc.cancel();
                                self.message = e.0;
                            }
                        }
                        self.rev += 1;
                    }
                }
            }
            "clear_frame" => {
                // every figure and textfield leaves this frame (the frame stays)
                self.step("Clear frame", |s| {
                    anchor_poses(s);
                    let ids: Vec<String> = s.project.root().map(|r| r.layers.iter().filter(|l| matches!(l.kind, LayerKind::Figure(_) | LayerKind::Text(_) | LayerKind::Symbol(_))).map(|l| l.id.clone()).collect()).unwrap_or_default();
                    for id in ids {
                        if let Some(l) = s.project.layer_mut(&sym, &id) {
                            l.shown.track.put(Key::new(t, Value::B(false), Interp::Hold));
                        }
                    }
                    Ok(())
                });
                self.selected = None;
                self.selected_text = None;
                self.validate();
            }
            "draw_tool" => {
                // another tool ends polyfill editing and connector picking
                self.polyfill = None;
                self.connecting = false;
                self.draw = match c["shape"].as_str() {
                    None | Some("none") => None,
                    Some(x) => Some(shape_named(x)),
                };
                self.rev += 1;
            }
            "edit_copy" => {
                // D12 / J1: the figure editor works on a copy; the animated
                // figure never changes. Ends with edit_copy_end.
                if let (Some(l), None) = (self.sel_layer(), self.copy_edit.as_ref()) {
                    if self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).is_some() {
                        let snapshot = Box::new(self.doc.clone());
                        let fr = self.frame;
                        let made = self.step("Edit a copy", |s| {
                            let ids = layer_ops::duplicate(s, &sym, &l, false, 0.0, 0.0)?;
                            let c = ids.first().cloned().ok_or_else(|| crate::edit::EditError("no copy".into()))?;
                            let name = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.name.clone()).unwrap_or_else(|| "Figure".into());
                            if let Some(x) = s.project.layer_mut(&sym, &c) {
                                x.name = Some(format!("{name} (copy)"));
                            }
                            // the original stays as it is, out of the way while editing
                            if let Some(x) = s.project.layer_mut(&sym, &l) {
                                x.visible = false;
                            }
                            Ok(c)
                        });
                        if let Some(c) = made {
                            self.copy_edit = Some((snapshot, l, fr));
                            self.building = Some(c.clone());
                            let root = self.project().root().and_then(|s| s.layer(&c)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                            self.selected = root.map(|r| (c, r));
                            self.mode = Mode::Figure;
                            self.rev += 1;
                        }
                    }
                }
            }
            "edit_copy_end" => {
                // Cancel or Save as new figure (the app saves the copy's file
                // first): the animation and its history as they were
                if let Some((snapshot, l, fr)) = self.copy_edit.take() {
                    self.doc.restore(*snapshot);
                    self.draw = None;
                    self.building = None;
                    self.polyfill = None;
                    self.connecting = false;
                    self.trace = None;
                    self.frame = fr;
                    let root = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                    self.selected = root.map(|r| (l, r));
                    self.changed();
                    self.validate();
                }
            }
            "build_done" => {
                self.draw = None;
                self.building = None;
                self.polyfill = None;
                self.connecting = false;
                self.trace = None;
                self.rev += 1;
            }
            "polyfill_new" => {
                // Add polyfill...: drawn right before the selected joint (its anchor)
                if let (Some(b), Some((l, n))) = (self.building.clone(), self.selected.clone()) {
                    if l == b {
                        let made = self.step("Add polyfill", |s| {
                            let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &b).map(|x| &mut x.kind) else { return crate::edit::fail("no figure") };
                            let mut k = f.polyfills.len() + 1;
                            while f.polyfills.iter().any(|p| p.id == format!("pf{k}")) {
                                k += 1;
                            }
                            let id = format!("pf{k}");
                            let mut pf = Polyfill::new(&b, &id);
                            pf.anchor = n.clone();
                            pf.use_own_color = true;
                            pf.color.value = Value::C(Color::rgba8888(0x8fd3ffff));
                            f.polyfills.push(pf);
                            Ok(id)
                        });
                        if let Some(id) = made {
                            self.polyfill = Some(id);
                            self.draw = None;
                        }
                    }
                }
            }
            "branch_edit" => {
                // Modify branch, the rows beyond the Java reference's modify_branch:
                // replace one colour by another, use segment colour, gradient
                // colour, and increment thickness / length ("ignore 0s")
                if let Some((l, n)) = self.selected.clone() {
                    let c2 = c.clone();
                    self.step("Modify branch", |s| {
                        let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &l).map(|x| &mut x.kind) else { return crate::edit::fail("no figure") };
                        let fig_color = figure_ops::figure_color(f);
                        let ignore0 = c2["ignoreZero"].as_bool().unwrap_or(true);
                        for id in f.subtree(&n) {
                            let Some(x) = f.node_mut(&id).filter(|x| x.parent.is_some()) else { continue };
                            if let (Some(from), Some(to)) = (c2["replaceFrom"].as_u64(), c2["replaceTo"].as_u64()) {
                                let (from, to) = (Color::rgba8888(from as u32), Color::rgba8888(to as u32));
                                if x.color.track.is_empty() && x.color.value.color() == from {
                                    x.color.value = Value::C(to);
                                    x.own_color = true;
                                    x.legacy_extras.shift_remove(KEPT_OWN_COLOR);
                                }
                                if x.gradient_color.track.is_empty() && x.gradient_color.value.color() == from {
                                    x.gradient_color.value = Value::C(to);
                                }
                            }
                            if let Some(v) = c2["ownColor"].as_bool() {
                                use_own_color(x, v, fig_color);
                                if !v {
                                    x.color.track.clear();
                                    x.color.value = Value::C(fig_color);
                                }
                            }
                            if let Some(v) = c2["gradientColor"].as_u64() {
                                x.gradient_color.track.clear();
                                x.gradient_color.value = Value::C(Color::rgba8888(v as u32));
                            }
                            if let Some(d) = c2["addThickness"].as_f64() {
                                if !(ignore0 && x.thickness.value.f() == 0.0 && x.thickness.track.is_empty()) {
                                    let v = (x.thickness.value.f() + d as f32).clamp(0.0, 400.0);
                                    x.thickness.track.clear();
                                    x.thickness.value = Value::F(v);
                                }
                            }
                            if let Some(d) = c2["addLength"].as_f64() {
                                let d = d as f32;
                                let zero = x.length.track.is_empty() && x.length.value.f() == 0.0;
                                if !(ignore0 && zero) {
                                    x.length.value = Value::F((x.length.value.f() + d).max(0.0));
                                    for k in x.length.track.keys().to_vec() {
                                        let mut k = k;
                                        if let Value::F(v) = k.value {
                                            k.value = Value::F((v + d).max(0.0));
                                        }
                                        x.length.track.put(k);
                                    }
                                    if x.rest_length > 0.0 {
                                        x.rest_length = (x.rest_length + d).max(0.0);
                                    }
                                }
                            }
                        }
                        Ok(())
                    });
                }
            }
            "editor_bg" => {
                self.editor_bg = c["color"].as_u64().map(|v| v as u32);
                self.rev += 1;
            }
            "trace" => {
                // {"frame": i} or {"layer": id}; neither clears it
                let fr = c["frame"].as_i64().map(|v| v as i32);
                let ly = c["layer"].as_str().map(str::to_string);
                self.trace = if fr.is_some() || ly.is_some() { Some((fr, ly)) } else { None };
                self.rev += 1;
            }
            "connect_start" => {
                self.connecting = self.building.is_some() && self.selected.is_some();
                self.polyfill = None;
                self.rev += 1;
            }
            "connect_cancel" => {
                self.connecting = false;
                self.rev += 1;
            }
            "connector_set" => {
                // Connector Tools: where it sits between its two joints
                if let Some((l, n)) = self.selected.clone() {
                    let (pc, rv) = (c["percent"].as_f64(), c["reversed"].as_bool());
                    self.step("Connector", |s| {
                        let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &l).map(|x| &mut x.kind) else { return crate::edit::fail("no figure") };
                        let Some(x) = f.node_mut(&n) else { return crate::edit::fail("no joint") };
                        if !x.legacy_extras.contains_key(crate::eval::pose::CONNECT_TO) {
                            return crate::edit::fail("not a connector");
                        }
                        if let Some(v) = pc {
                            x.legacy_extras.insert("C.cPercent".into(), format!("f:{}", (v.clamp(0.0, 1.0) * 100.0).round() / 100.0));
                        }
                        if let Some(v) = rv {
                            x.legacy_extras.insert("C.cReversed".into(), format!("f:{}", if v { 1 } else { 0 }));
                        }
                        Ok(())
                    });
                }
            }
            "polyfill_edit" => {
                // an existing polyfill of the joint's figure, or none (Finish)
                self.polyfill = c["id"].as_str().map(str::to_string);
                self.rev += 1;
            }
            "polyfill_undo" | "polyfill_delete" | "polyfill_color" => {
                if let (Some(b), Some(pf)) = (self.building.clone(), self.polyfill.clone()) {
                    let c2 = c.clone();
                    let label = match op { "polyfill_undo" => "Remove polynode", "polyfill_delete" => "Delete polyfill", _ => "Polyfill colour" };
                    self.step(label, |s| {
                        let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &b).map(|x| &mut x.kind) else { return crate::edit::fail("no figure") };
                        let Some(i) = f.polyfills.iter().position(|p| p.id == pf) else { return crate::edit::fail("no polyfill") };
                        match c2["op"].as_str() {
                            Some("polyfill_undo") => {
                                f.polyfills[i].nodes.pop();
                            }
                            Some("polyfill_delete") => {
                                f.polyfills.remove(i);
                            }
                            _ => {
                                let fill = &mut f.polyfills[i];
                                if let Some(v) = c2["own"].as_bool() {
                                    fill.use_own_color = v;
                                }
                                if let Some(v) = c2["color"].as_u64() {
                                    fill.color.track.clear();
                                    fill.color.value = Value::C(Color::rgba8888(v as u32));
                                }
                            }
                        }
                        Ok(())
                    });
                    if op == "polyfill_delete" {
                        self.polyfill = None;
                    }
                }
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
                    l.capture_base();
                    if s.project.settings.tween_interp == Interp::Smooth {
                        smooth_tweens(&mut l);
                    }
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
                            if c2.get("font").is_some() {
                                // a bundled font's name, a font asset id, or null (default)
                                tl.font = c2["font"].as_str().filter(|f| !f.is_empty()).map(str::to_string);
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
                    self.step(if fwd { "Bring forward" } else { "Send back" }, |s| move_among_drawn(s, &sym, &l, fwd));
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
            "delete_joint" => {
                if let Some((l, n)) = self.selected.clone() {
                    let parent = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.node(&n)).and_then(|x| x.parent.clone());
                    if self.step("Delete segment", |s| figure_ops::delete_joint(s, &sym, &l, &n)).is_some() {
                        self.selected = parent.map(|p| (l, p));
                    }
                }
            }
            "reorder" => {
                if let Some((l, n)) = self.selected.clone() {
                    let (branch, to) = (b("branch"), c["to"].as_i64().unwrap_or(1) as i32);
                    self.step(if branch { "Reorder branch" } else { "Reorder segment" }, |s| figure_ops::reorder(s, &sym, &l, &n, branch, to));
                }
            }
            "reorder_at" => {
                // the Parts list's drag: this joint to place `index` (0: at the back)
                if let (Some(l), Some(n)) = (self.building.clone().or_else(|| self.sel_layer()), c["node"].as_str().map(str::to_string)) {
                    let index = c["index"].as_u64().unwrap_or(0) as usize;
                    self.step("Reorder segment", |s| figure_ops::reorder_at(s, &sym, &l, &n, index));
                }
            }
            "copy_branch" => {
                // the selected segment alone, or with everything hanging off it
                if let Some((l, n)) = self.selected.clone() {
                    let branch = b("branch");
                    if let Some(f) = self.project().root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()) {
                        let ids: Vec<String> = if branch { f.subtree(&n) } else { vec![n.clone()] };
                        let nodes: Vec<Node> = ids.iter().filter_map(|id| f.node(id)).filter(|x| x.parent.is_some()).cloned().collect();
                        if !nodes.is_empty() {
                            self.branch_clip = Some((nodes, f.polyfills.clone()));
                            self.message = if branch { "Branch copied.".into() } else { "Segment copied.".into() };
                        }
                        self.rev += 1;
                    }
                }
            }
            "paste_branch" => {
                if let (Some((l, n)), Some((nodes, polys))) = (self.selected.clone(), self.branch_clip.clone()) {
                    if let Some(id) = self.step("Paste segment", |s| figure_ops::paste_nodes(s, &sym, &l, &n, &nodes, &polys)) {
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
                    self.step("Split segment", |s| figure_ops::split(s, &sym, &l, &n, k.clamp(2, 21)));
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
            // F4: blocks. A block is one figure's pose key, lasting until its
            // next one; these change the selected figure's row only.
            "delete_pose" => {
                // the Pose page's pinned button: the pose under the playhead goes,
                // the one before it lasts until the next (one step)
                if let Some(l) = self.sel_layer() {
                    let fr = self.frame;
                    self.step("Delete pose", |s| crate::edit::block_ops::delete_pose(s, &sym, &l, fr));
                }
            }
            "add_time" => {
                // the Frame page's "Add time here": a frame after this one with no
                // keys of its own; every layer holds or tweens across it
                let (fr, n) = (self.frame, c["n"].as_i64().unwrap_or(1).clamp(1, 100) as i32);
                self.step("Add time", |s| frame_ops::insert_tweened(s, fr, n));
            }
            "blend" => {
                if let Some(l) = self.sel_layer().or_else(|| self.selected_text.clone()) {
                    let v = c["v"].as_str().and_then(Blend::from_name).unwrap_or(Blend::Normal);
                    self.step("Blend", |s| {
                        let Some(x) = s.project.layer_mut(&sym, &l) else { return crate::edit::fail("no layer") };
                        x.blend = v;
                        Ok(())
                    });
                }
            }
            "new_pose" | "hold_longer" | "block_length" | "move_block" => {
                let layer = c["layer"].as_str().map(str::to_string).or_else(|| self.sel_layer());
                if let Some(l) = layer {
                    let fr = self.frame;
                    let op = c["op"].as_str().unwrap_or("").to_string();
                    let (frames, to) = (c["frames"].as_i64().unwrap_or(1) as i32, c["to"].as_i64().unwrap_or(fr as i64) as i32);
                    let label = match op.as_str() {
                        "new_pose" => "New pose",
                        "hold_longer" => "Hold longer",
                        "block_length" => "Pose length",
                        _ => "Move pose",
                    };
                    let done = self.step(label, |s| match op.as_str() {
                        "new_pose" => crate::edit::block_ops::new_pose(s, &sym, &l, fr),
                        "hold_longer" => crate::edit::block_ops::hold_longer(s, &sym, &l, fr),
                        "block_length" => crate::edit::block_ops::set_block_length(s, &sym, &l, fr, frames),
                        _ => crate::edit::block_ops::move_block(s, &sym, &l, fr, to),
                    });
                    if done.is_some() && op == "move_block" {
                        self.frame = to;
                        self.changed();
                    }
                } else {
                    self.message = "Select a figure first.".into();
                }
            }
            "break_off" => {
                // D3 / F1: the selected joint's branch becomes its own figure,
                // joined where it was (one step); the new figure is selected
                if let Some((l, n)) = self.selected.clone() {
                    if let Some(id) = self.step("Break off", |s| crate::edit::join_ops::break_off(s, &sym, &l, &n)) {
                        self.selected = Some((id, n));
                        self.rev += 1;
                    }
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
                            } else if let Some(v) = c2["stretchy"].as_bool() {
                                BranchChange::Stretchy(v)
                            } else if let Some(v) = c2["scaleLength"].as_f64() {
                                BranchChange::ScaleLength(v as f32)
                            } else {
                                return Ok(());
                            };
                            let colour = matches!(ch, BranchChange::Color(_));
                            figure_ops::modify_branch(s, &sym, &l, &n, &ch)?;
                            if colour {
                                // a colour of their own (the Java reference has no such flag)
                                if let Some(LayerKind::Figure(f)) = s.project.layer_mut(&sym, &l).map(|x| &mut x.kind) {
                                    for id in f.subtree(&n) {
                                        if let Some(x) = f.node_mut(&id).filter(|x| x.parent.is_some()) {
                                            x.own_color = true;
                                            x.legacy_extras.shift_remove(KEPT_OWN_COLOR);
                                        }
                                    }
                                }
                            }
                            return Ok(());
                        }
                        let Some(layer) = s.project.layer_mut(&sym, &l) else { return crate::edit::fail("no figure") };
                        let LayerKind::Figure(fig) = &mut layer.kind else { return crate::edit::fail("not a figure") };
                        let fig_color = figure_ops::figure_color(fig);
                        let Some(node) = fig.node_mut(&n) else { return crate::edit::fail("no segment") };
                        if let Some(v) = c2["thickness"].as_f64() {
                            set_static(&mut node.thickness, Value::F(v.clamp(0.0, 400.0) as f32));
                        }
                        if let Some(v) = c2["color"].as_u64() {
                            // a colour of its own ("Use segment color" on)
                            set_static(&mut node.color, Value::C(Color::rgba8888(v as u32)));
                            node.own_color = true;
                            node.legacy_extras.shift_remove(KEPT_OWN_COLOR);
                        }
                        if let Some(v) = c2["ownColor"].as_bool() {
                            use_own_color(node, v, fig_color);
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
                        // the rest of the original's Segment Tools
                        if let Some(v) = c2["gradient"].as_bool() {
                            node.gradient = v;
                        }
                        if let Some(v) = c2["gradientColor"].as_u64() {
                            set_static(&mut node.gradient_color, Value::C(Color::rgba8888(v as u32)));
                        }
                        if let Some(v) = c2["gradientReverse"].as_bool() {
                            node.gradient_reverse = v;
                        }
                        if let Some(v) = c2["gradientSideways"].as_bool() {
                            node.gradient_sideways = v;
                        }
                        if let Some(v) = c2["segScale"].as_f64() {
                            set_static(&mut node.scale, Value::F(v.clamp(0.01, 100.0) as f32));
                        }
                        if let Some(v) = c2["curve"].as_f64() {
                            node.curve = v.clamp(-1000.0, 1000.0) as f32;
                        }
                        if let Some(v) = c2["outline"].as_bool() {
                            node.circle_outline = v;
                        }
                        if let Some(v) = c2["outlineColor"].as_u64() {
                            set_static(&mut node.outline_color, Value::C(Color::rgba8888(v as u32)));
                        }
                        if let Some(v) = c2["hollow"].as_bool() {
                            node.circle_hollow = v;
                        }
                        if let Some(v) = c2["halfArc"].as_i64() {
                            node.half_arc = v.clamp(-1, 1) as i32;
                        }
                        if let Some(v) = c2["sides"].as_i64() {
                            node.polygon_sides = v.clamp(3, 64) as i32;
                        }
                        // trapezoid: end width relative to the start, rounded ends
                        if let Some(v) = c2["trapRatio"].as_f64() {
                            node.trapezoid_ratio = ((v.clamp(0.0, 10.0) * 100.0).round() / 100.0) as f32;
                        }
                        if let Some(v) = c2["trapRoundStart"].as_bool() {
                            node.trapezoid_round_start = v;
                        }
                        if let Some(v) = c2["trapRoundEnd"].as_bool() {
                            node.trapezoid_round_end = v;
                        }
                        if let Some(v) = c2["rightTriangle"].as_i64() {
                            node.right_triangle = v.clamp(-1, 1) as i32;
                        }
                        if let Some(v) = c2["upsideDown"].as_bool() {
                            node.triangle_upside_down = v;
                        }
                        if let Some(v) = c2["angleLock"].as_i64() {
                            node.angle_lock = v.clamp(0, 2) as i32;
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
                        if let Some(v) = c2["keepInTween"].as_bool() {
                            layer.vanish_in_tween = !v;
                        }
                        if let Some(v) = c2["tweenColors"].as_bool() {
                            // colours blend between frames, or jump at each frame: a
                            // switch the evaluator reads (eval::figure_color), so the keys
                            // keep their easing and off, then on, changes nothing
                            layer.colors_jump = !v;
                        }
                        if let Some(v) = c2["color"].as_u64() {
                            // the stickfigure colour: segments without their own colour follow it
                            if let LayerKind::Figure(f) = &mut layer.kind {
                                figure_ops::set_figure_color(f, Color::rgba8888(v as u32));
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
            "reset_segment" => {
                // "Reset this segment" (D7b): every look value of the selected
                // segment back to how its figure came into the project, and its
                // length on this frame back to the made-with length. One step.
                if let Some((l, n)) = self.selected.clone() {
                    let ed = self.tools.editor.clone();
                    self.step("Reset segment", move |s| {
                        let (base, len_id) = {
                            let Some(f) = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()) else { return crate::edit::fail("no figure") };
                            let Some(node) = f.node(&n) else { return crate::edit::fail("no segment") };
                            let Some(b) = f.base_of(&n) else { return crate::edit::fail("This figure came from an older project, so there is nothing to reset it to.") };
                            (b.clone(), node.length.id.clone())
                        };
                        ed.change_in(s, &len_id, Value::F(base.length), t)?;
                        let Some(layer) = s.project.layer_mut(&sym, &l) else { return crate::edit::fail("no figure") };
                        let LayerKind::Figure(fig) = &mut layer.kind else { return crate::edit::fail("not a figure") };
                        let fig_color = figure_ops::figure_color(fig);
                        let Some(node) = fig.node_mut(&n) else { return crate::edit::fail("no segment") };
                        node.shape = base.shape;
                        set_static(&mut node.thickness, Value::F(base.thickness));
                        set_static(&mut node.scale, Value::F(base.scale));
                        if base.own_color {
                            set_static(&mut node.color, Value::C(base.color));
                            node.own_color = true;
                        } else {
                            use_own_color(node, false, fig_color);
                        }
                        node.legacy_extras.shift_remove(KEPT_OWN_COLOR);
                        node.curve = base.curve;
                        node.is_static = base.is_static;
                        node.stretchy = base.stretchy;
                        node.smart_stretch = base.smart_stretch;
                        node.gradient = base.gradient;
                        set_static(&mut node.gradient_color, Value::C(base.gradient_color));
                        node.gradient_reverse = base.gradient_reverse;
                        node.gradient_sideways = base.gradient_sideways;
                        node.circle_outline = base.circle_outline;
                        set_static(&mut node.outline_color, Value::C(base.outline_color));
                        node.circle_hollow = base.circle_hollow;
                        node.half_arc = base.half_arc;
                        node.polygon_sides = base.polygon_sides;
                        node.trapezoid_ratio = base.trapezoid_ratio;
                        node.trapezoid_round_start = base.trapezoid_round_start;
                        node.trapezoid_round_end = base.trapezoid_round_end;
                        node.right_triangle = base.right_triangle;
                        node.triangle_upside_down = base.triangle_upside_down;
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
                    // "Paste with layering": the original's place among the figures shown
                    // on the frame it was copied from
                    let rank = c["layering"].as_bool().unwrap_or(false).then(|| {
                        let p = self.project();
                        let root = p.root()?;
                        let at = root.layer_index(&clip.first()?.id)?;
                        let mut r = Resolver::new(p);
                        Some(root.layers[..at].iter().filter(|x| r.b(&x.shown, t_copy)).count())
                    }).flatten();
                    let made = self.step("Paste stickfigure", |s| {
                        let sym2 = s.project.root_symbol.clone();
                        let tw = s.project.settings.tween_interp;
                        let here = |s: &crate::edit::State, id: &str| s.project.root().and_then(|r| r.layer(id)).map(|x| Resolver::new(&s.project).b(&x.shown, t));
                        let same = clip.iter().all(|l| here(s, &l.id) == Some(false));
                        if same {
                            for l in &clip {
                                let vals: Vec<(String, Value)> = l.properties().iter().filter(|q| !q.track.is_empty()).map(|q| (q.id.clone(), q.key_value(t_copy))).collect();
                                for (pid, v) in vals {
                                    if let Some(q) = s.project.property_mut(&pid) {
                                        let interp = if matches!(v, Value::B(_)) { Interp::Hold } else { crate::edit::editor::interp_at(q, t, tw) };
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
                        let mut last_at: Option<usize> = None;
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
                            let at = s.project.root().map(|r| {
                                let top = r.layers.len();
                                // joined figures keep their order: each right above the last
                                if let Some(prev) = first.as_ref().and_then(|_| last_at) {
                                    return prev + 1;
                                }
                                let Some(k) = rank else { return top };
                                // before the k-th figure shown on this frame (k shown below it)
                                let mut res = Resolver::new(&s.project);
                                r.layers.iter().enumerate().filter(|(_, x)| res.b(&x.shown, t)).nth(k).map_or(top, |(i, _)| i)
                            }).unwrap_or(0);
                            structure::add_layer(s, &sym2, c, at)?;
                            last_at = Some(at);
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
            "join_to" => {
                // the join gesture as one command: the selected figure joins
                // `layer` at joint `node` (or the figure itself)
                if let (Some(child), Some(target)) = (self.sel_layer(), c["layer"].as_str().map(str::to_string)) {
                    let node = c["node"].as_str().map(str::to_string);
                    self.step("Join", |s| crate::edit::join_ops::join(s, &sym, &child, &target, node.as_deref(), true, t));
                    self.joining = None;
                }
            }
            "join_here" | "let_go_here" => {
                // D8 / I1: joins are moments. From this frame on the selected
                // figure is joined to `layer` (its `node`), or lets go
                if let Some(child) = self.sel_layer() {
                    let let_go = c["op"] == "let_go_here";
                    let target = c["layer"].as_str().map(str::to_string);
                    let node = c["node"].as_str().map(str::to_string);
                    if !let_go && target.is_none() {
                        self.message = "Pick the figure to join first.".into();
                    } else {
                        self.step(if let_go { "Let go" } else { "Join" }, |s| crate::edit::join_ops::switch_at(s, &sym, &child, if let_go { None } else { target.as_deref() }, node.as_deref(), true, t));
                    }
                }
            }
            "join_cancel" => {
                self.joining = None;
                self.rev += 1;
            }
            "unjoin" => {
                // D8: letting go is a moment too (from this frame on; on the
                // first frame, for the whole animation)
                if let Some(l) = self.sel_layer() {
                    if t > 1e-9 {
                        self.step("Let go", |s| crate::edit::join_ops::switch_at(s, &sym, &l, None, None, true, t));
                    } else {
                        self.step("Unjoin", |s| crate::edit::join_ops::unjoin(s, &sym, &l, t));
                    }
                }
            }
            "copy_props" => {
                if let Some(l) = self.sel_layer() {
                    match layer_ops::copy_props(self.project(), &sym, &l, t) {
                        Ok(p) => {
                            self.props_clip = Some(p);
                            let pr = self.project();
                            self.props_nodes = pr.root().and_then(|s| s.layer(&l)).and_then(|x| x.as_figure()).map(|f| {
                                let mut r = Resolver::new(pr);
                                let nodes: Vec<(String, f32, f32, u32, f32)> = f.nodes().iter().map(|n| (n.id.clone(), r.f(&n.rotation, t), r.f(&n.length, t), color_of(&mut r, &n.color, t), r.f(&n.thickness, t))).collect();
                                (nodes.get(1).map(|n| n.3).unwrap_or(0x000000ff), nodes)
                            });
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
                    let on = |k: &str| c.get(k).and_then(|v| v.as_bool()).unwrap_or(true);
                    let parts = layer_ops::Parts { position: on("position"), scale: on("scale"), rotation: on("rotation"), filters: on("filters"), layering: on("layering") };
                    let (color, nodes) = (on("color"), on("nodes"));
                    let extra = self.props_nodes.clone();
                    self.step("Paste properties", |s| {
                        if nodes {
                            anchor_poses(s);
                        }
                        layer_ops::paste_parts(&ed, s, &sym, &l, &pc, parts, t)?;
                        let Some((fig_color, list)) = extra else { return Ok(()) };
                        if color && !nodes {
                            let root = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.nodes().first().map(|n| n.id.clone()));
                            if let Some(root) = root {
                                figure_ops::modify_branch(s, &sym, &l, &root, &BranchChange::Color(Color::rgba8888(fig_color)))?;
                            }
                        }
                        if nodes {
                            // only between figures of the same make (the same joints)
                            let same = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()).is_some_and(|f| f.nodes().len() == list.len() && f.nodes().iter().zip(&list).all(|(n, c)| n.id == c.0));
                            if !same {
                                return crate::edit::fail("Node properties paste only onto the same kind of figure.");
                            }
                            for (id, rot, len, col, thick) in &list {
                                let ids = s.project.root().and_then(|r| r.layer(&l)).and_then(|x| x.as_figure()).and_then(|f| f.node(id)).filter(|n| n.parent.is_some()).map(|n| (n.rotation.id.clone(), n.length.id.clone()));
                                if let Some((ri, li)) = ids {
                                    ed.change_in(s, &ri, Value::F(*rot), t)?;
                                    ed.change_in(s, &li, Value::F(*len), t)?;
                                }
                                let lay = s.project.layer_mut(&sym, &l).expect("figure");
                                if let LayerKind::Figure(f) = &mut lay.kind {
                                    if let Some(n) = f.node_mut(id) {
                                        set_static(&mut n.color, Value::C(Color::rgba8888(*col)));
                                        set_static(&mut n.thickness, Value::F(*thick));
                                    }
                                }
                            }
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
            "group" => {
                // K: the layers named, else the several selected, else the selected one
                let list: Vec<String> = match c["layers"].as_array() {
                    Some(a) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                    None if self.multi.len() > 1 => self.multi.clone(),
                    None => self.sel_layer().into_iter().collect(),
                };
                let name = c["name"].as_str().map(str::to_string);
                if let Some(gid) = self.step("Group", |s| crate::edit::group_ops::group(s, &sym, &list, name.as_deref())) {
                    self.message = match list.len() {
                        0 | 1 => String::new(),
                        _ => "Grouped. They now sit together in the layer list.".into(),
                    };
                    // the new group is selected: its page opens
                    self.selected = None;
                    self.selected_text = None;
                    self.multi.clear();
                    self.selected_group = Some(gid);
                }
            }
            "group_order" => {
                if let Some(gid) = c["group"].as_str().map(str::to_string).or_else(|| self.selected_group.clone()) {
                    let fwd = b("forward");
                    self.step(if fwd { "Bring group forward" } else { "Send group back" }, |s| crate::edit::group_ops::move_group(s, &sym, &gid, fwd));
                }
            }
            "ungroup" => {
                if let Some(gid) = c["group"].as_str().map(str::to_string).or_else(|| self.selected_group.clone()) {
                    self.step("Ungroup", |s| crate::edit::group_ops::ungroup(s, &sym, &gid));
                }
            }
            "delete_group" => {
                if let Some(gid) = c["group"].as_str().map(str::to_string).or_else(|| self.selected_group.clone()) {
                    if self.step("Delete group", |s| crate::edit::group_ops::delete(s, &sym, &gid)).is_some() {
                        self.validate();
                    }
                }
            }
            "group_set" => {
                if let Some(gid) = c["group"].as_str().map(str::to_string).or_else(|| self.selected_group.clone()) {
                    let (name, opacity, open) = (c["name"].as_str().map(str::to_string), c["opacity"].as_f64().map(|v| v as f32), c["open"].as_bool());
                    // opening and closing the folder row is not an edit to undo
                    if name.is_none() && opacity.is_none() && open.is_some() {
                        let r = self.doc.project_mut().symbol_mut(&sym).and_then(|s| s.groups.iter_mut().find(|g| g.id == gid)).map(|g| g.open = open.unwrap_or(true));
                        if r.is_some() {
                            self.rev += 1;
                        }
                    } else {
                        let label = if name.is_some() { "Rename group" } else { "Group opacity" };
                        self.step(label, |s| crate::edit::group_ops::set(s, &sym, &gid, name.as_deref(), opacity, open));
                    }
                }
            }
            "copy_poses" => {
                // several figures (the multi-select board): each one's pose here
                let list: Vec<String> = if self.multi.len() > 1 { self.multi.clone() } else { self.sel_layer().into_iter().collect() };
                let p = self.project();
                let mut r = Resolver::new(p);
                self.poses_clip = list
                    .iter()
                    .filter_map(|l| {
                        let fig = p.root()?.layer(l)?.as_figure()?;
                        let rot = fig.nodes().iter().filter(|n| n.parent.is_some()).map(|n| (n.id.clone(), r.f(&n.rotation, t))).collect();
                        let len = fig.nodes().iter().filter(|n| n.parent.is_some()).map(|n| (n.id.clone(), r.f(&n.length, t))).collect();
                        Some((l.clone(), rot, len))
                    })
                    .collect();
                self.message = match self.poses_clip.len() {
                    0 => String::new(),
                    1 => "Pose copied.".into(),
                    n => format!("{n} poses copied."),
                };
                self.rev += 1;
            }
            "paste_poses" => {
                // each copied pose back on its own figure, here, as one step
                let clip = self.poses_clip.clone();
                if !clip.is_empty() {
                    let ed = self.tools.editor.clone();
                    self.doc.begin("Paste poses");
                    anchor_poses(&mut self.doc.state);
                    for (l, rot, len) in &clip {
                        if self.project().root().and_then(|r| r.layer(l)).is_none() {
                            continue;
                        }
                        if let Err(e) = ed.pose(&mut self.doc, &sym, l, rot, len, t) {
                            self.message = e.0;
                        }
                    }
                    self.doc.commit();
                    self.changed();
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
            "camera_lock" => {
                // Lock Figure To Camera: the figure moves with the camera (kept where
                // it is now); joined figures have to be unjoined first
                let id = c["layer"].as_str().map(str::to_string).or_else(|| self.sel_layer());
                let on = c["on"].as_bool().unwrap_or(true);
                if let Some(id) = id {
                    let joined = self.project().root().and_then(|r| r.layer(&id)).is_some_and(|l| l.parent.as_ref().is_some_and(|p| !self.project().root().and_then(|r| r.layer(p)).is_some_and(|x| matches!(x.kind, LayerKind::Camera(_)))));
                    if on && joined {
                        self.message = "You can't lock a joined figure to the camera. Unjoin it first.".into();
                    } else {
                        self.step(if on { "Lock to camera" } else { "Unlock from camera" }, |s| {
                            let cam = if on { Some(ensure_camera(s, &sym)?) } else { None };
                            crate::edit::join_ops::reparent(s, &sym, &id, cam.as_deref(), t)?;
                            if let Some(l) = s.project.layer_mut(&sym, &id) {
                                if !on {
                                    l.parallax = 0.0;
                                    l.lock_move_only = false;
                                }
                            }
                            Ok(())
                        });
                    }
                }
            }
            "camera_lock_set" => {
                // parallax in the original's units (1 = exactly with the camera) and
                // "Rotate and scale with camera?"
                if let Some(id) = c["layer"].as_str().map(str::to_string) {
                    let (par, rot) = (c["parallax"].as_f64(), c["rotate"].as_bool());
                    self.step("Camera lock", |s| {
                        let Some(l) = s.project.layer_mut(&sym, &id) else { return crate::edit::fail("no layer") };
                        if let Some(p) = par {
                            l.parallax = (1.0 - p.clamp(-1.0, 1.0)) as f32;
                        }
                        if let Some(r) = rot {
                            l.lock_move_only = !r;
                        }
                        Ok(())
                    });
                }
            }
            "to_movieclip" => {
                // the selected figure and the figures joined to it become a movieclip
                // that plays their animation in a loop, in their place
                // a group (its page): every member, pictures and text too
                let from_group: Option<(Vec<String>, String)> = c["group"].as_str().and_then(|gid| {
                    let r = self.project().root()?;
                    let g = r.groups.iter().find(|g| g.id == gid)?;
                    Some((r.layers.iter().filter(|l| l.group.as_deref() == Some(gid)).map(|l| l.id.clone()).collect(), g.name.clone()))
                });
                let start = match from_group {
                    Some((ids, name)) => ids.first().cloned().map(|first| (first, ids, Some(name))),
                    // several selected (the multi-select board): all of them
                    None => self.sel_layer().map(|l| (l.clone(), if self.multi.len() > 1 { self.multi.clone() } else { vec![l] }, None)),
                };
                if let Some((l, mut set, group_name)) = start {
                    let p = self.project();
                    let mut i = 0;
                    while i < set.len() {
                        let cur = set[i].clone();
                        if let Some(r) = p.root() {
                            for x in &r.layers {
                                if x.parent.as_deref() == Some(cur.as_str()) && !set.contains(&x.id) {
                                    set.push(x.id.clone());
                                }
                            }
                        }
                        i += 1;
                    }
                    let name = group_name.or_else(|| p.root().and_then(|r| r.layer(&l)).and_then(|x| x.name.clone())).unwrap_or_else(|| "Movieclip".into());
                    let order: Vec<String> = p.root().map(|r| r.layers.iter().filter(|x| set.contains(&x.id)).map(|x| x.id.clone()).collect()).unwrap_or_default();
                    if self.step("Convert to movieclip", |s| crate::edit::symbol_ops::convert_to_symbol(s, &sym, &order, &format!("{name} clip")).map(|_| ())).is_some() {
                        self.selected = None;
                        self.selected_group = None;
                        self.multi.clear();
                        self.validate();
                    } else if self.message.is_empty() {
                        self.message = "This figure is joined to one outside it; unjoin it first.".into();
                    }
                }
            }
            "edit_movieclip" => {
                // the movieclip editor: its own frames, with every tool
                let target = self.sel_layer().and_then(|l| self.project().root()?.layer(&l).and_then(|x| if let LayerKind::Symbol(sl) = &x.kind { Some(sl.symbol.clone()) } else { None }));
                if let Some(sid) = target {
                    if self.step("Edit movieclip", |s| {
                        s.project.root_symbol = sid.clone();
                        Ok(())
                    }).is_some() {
                        self.selected = None;
                        self.frame = 0;
                        self.validate();
                    }
                }
            }
            "new_movieclip" => {
                // Create new movieclip: an empty one-frame movieclip on this frame,
                // opened in the movieclip editor
                if self.project().root_symbol == self.main_root {
                    let fr_now = self.frame;
                    let made = self.step("New movieclip", |s| {
                        let fps = st_fps(&s.project);
                        let n = s.project.library.values().filter(|x| x.id != s.project.root_symbol).count() + 1;
                        let sid = s.project.new_id("S");
                        let name = format!("Movieclip {n}");
                        s.project.add_symbol(Symbol::new(&sid, Some(&name), 1.0 / fps));
                        let lid = s.project.new_id("L");
                        let mut inst = Layer::symbol(&lid, Some(&name), &sid);
                        if let LayerKind::Symbol(x) = &mut inst.kind {
                            x.loop_mode = SymbolLoop::Loop;
                        }
                        let at = s.project.root().map(|r| r.layers.len()).unwrap_or(0);
                        structure::add_layer(s, &sym, inst, at)?;
                        only_on_frame(s, &sym, &lid, fr_now);
                        s.project.root_symbol = sid;
                        Ok(())
                    });
                    if made.is_some() {
                        self.selected = None;
                        self.frame = 0;
                        self.validate();
                    }
                }
            }
            "movieclip_done" => {
                if self.project().root_symbol != self.main_root {
                    let main = self.main_root.clone();
                    self.step("Finish movieclip", |s| {
                        s.project.root_symbol = main;
                        Ok(())
                    });
                    self.selected = None;
                    self.frame = 0;
                    self.validate();
                }
            }
            "movieclip" => {
                // Movieclip Tools: loop mode, speed, start offset (seconds)
                if let Some(l) = self.sel_layer() {
                    let c2 = c.clone();
                    self.step("Movieclip", |s| {
                        let Some(x) = s.project.layer_mut(&sym, &l) else { return crate::edit::fail("no layer") };
                        let LayerKind::Symbol(sl) = &mut x.kind else { return crate::edit::fail("not a movieclip") };
                        if let Some(m) = c2["loop"].as_str() {
                            sl.loop_mode = match m { "once" => SymbolLoop::Once, "pingpong" => SymbolLoop::PingPong, _ => SymbolLoop::Loop };
                        }
                        if let Some(v) = c2["speed"].as_f64() {
                            sl.speed.track.clear();
                            sl.speed.value = Value::D((v.clamp(0.0, 10.0) * 100.0).round() / 100.0);
                        }
                        if let Some(v) = c2["offset"].as_f64() {
                            sl.time_offset.track.clear();
                            sl.time_offset.value = Value::D((v.clamp(-600.0, 600.0) * 100.0).round() / 100.0);
                        }
                        Ok(())
                    });
                }
            }
            "break_apart" => {
                // a movieclip back into figures, posed as it shows on this frame
                if let Some(id) = c["layer"].as_str().map(str::to_string) {
                    self.step("Break apart", |s| crate::edit::symbol_ops::break_apart(s, &sym, &id, t).map(|_| ()));
                    self.validate();
                }
            }
            "library_delete" => {
                // Library Tools: a figure, movieclip or sprite off every frame (the
                // ones joined to it stay where they are, unjoined)
                if let Some(id) = c["layer"].as_str().map(str::to_string) {
                    self.step("Delete from project", |s| {
                        let kids: Vec<String> = s.project.root().map(|r| r.layers.iter().filter(|x| x.parent.as_deref() == Some(id.as_str())).map(|x| x.id.clone()).collect()).unwrap_or_default();
                        for k in kids {
                            crate::edit::join_ops::unjoin(s, &sym, &k, t)?;
                        }
                        structure::remove_layer(s, &sym, &id)
                    });
                    if self.selected.as_ref().is_some_and(|(l, _)| l == c["layer"].as_str().unwrap_or("")) {
                        self.selected = None;
                    }
                    self.validate();
                }
            }
            "rename" => {
                if let Some(l) = c["layer"].as_str().map(str::to_string).or_else(|| self.sel_layer()) {
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
            "watermark" => {
                // Project Tools: on / off, text, colour (exports only)
                let c2 = c.clone();
                self.step("Watermark", move |s| {
                    let cur = s.project.settings.watermark.clone();
                    let on = c2["on"].as_bool().unwrap_or(cur.is_some());
                    if !on {
                        s.project.settings.watermark = None;
                        return Ok(());
                    }
                    let (mut text, mut color) = cur.unwrap_or((":)".into(), Color::rgba8888(0xff3b30ff)));
                    if let Some(v) = c2["text"].as_str() {
                        text = v.chars().take(200).collect();
                    }
                    if let Some(v) = c2["color"].as_u64() {
                        color = Color::rgba8888(v as u32);
                    }
                    s.project.settings.watermark = Some((text, color));
                    Ok(())
                });
            }
            "rotate_cameras" => {
                // Make vertical: every frame's camera turns 90 degrees (one step)
                self.step("Rotate cameras", |s| {
                    let cam = ensure_camera(s, &sym)?;
                    anchor_poses(s);
                    let l = s.project.layer_mut(&sym, &cam).expect("camera");
                    let LayerKind::Camera(cm) = &mut l.kind else { return crate::edit::fail("not a camera") };
                    let keys: Vec<Key> = cm.rotation.track.keys().iter().map(|k| Key { value: Value::F(k.value.f() + 90.0), ..k.clone() }).collect();
                    cm.rotation.track.clear();
                    for k in keys {
                        cm.rotation.track.put(k);
                    }
                    cm.rotation.value = Value::F(cm.rotation.value.f() + 90.0);
                    Ok(())
                });
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
            "frame_delay" | "frame_repeat" | "frame_silence" | "frame_slow" | "camera_fx" | "auto_camera" => {
                let c2 = c.clone();
                let delay = op == "frame_delay";
                let label = match op { "frame_delay" => "Frame delay", "frame_repeat" => "Repeat frames", "frame_slow" => "Slow motion", "camera_fx" => "Camera effects", "auto_camera" => "Auto-camera", _ => "Silence sounds" };
                self.step(label, move |s| {
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
                    if let Some(v) = c2["on"].as_bool().filter(|_| c2["op"] == "frame_silence") {
                        set(&mut cm.silence, if v { 1.0 } else { 0.0 });
                        return Ok(());
                    }
                    if c2["op"] == "auto_camera" {
                        // from this frame to a later one; "end": null deletes it
                        let here = (t * fps).round() as i64;
                        if let Some((_, b, _)) = crate::eval::auto_camera_at(cm, t).filter(|(a, _, _)| (a - t).abs() < 0.25 / fps) {
                            // replacing (or deleting) the one that starts here: its end goes
                            let bi = (b * fps).round() as i32;
                            set(&mut cm.auto_cam_end, 0.0);
                            let tb = bi as f64 / fps;
                            cm.auto_cam_end.track.put(Key::new(tb, Value::F(0.0), Interp::Hold));
                        }
                        match c2["end"].as_i64() {
                            Some(e) if e > here && e < n as i64 => {
                                let code = c2["ease"].as_i64().unwrap_or(1).clamp(1, 35) as f32;
                                set(&mut cm.auto_cam, code);
                                let te = e as f64 / fps;
                                for i in 0..n {
                                    let ti = i as f64 / fps;
                                    if cm.auto_cam_end.track.index_of(ti).is_err() {
                                        cm.auto_cam_end.track.put(Key::new(ti, Value::F(0.0), Interp::Hold));
                                    }
                                }
                                cm.auto_cam_end.track.put(Key::new(te, Value::F(1.0), Interp::Hold));
                            }
                            Some(_) => return crate::edit::fail("the auto-camera ends on a later frame"),
                            None => set(&mut cm.auto_cam, 0.0),
                        }
                        return Ok(());
                    }
                    if c2["op"] == "camera_fx" {
                        // Camera Tools: widescreen and wobble of this frame
                        if let Some(v) = c2["widescreen"].as_bool() {
                            set(&mut cm.widescreen, if v { 1.0 } else { 0.0 });
                        }
                        if let Some(v) = c2["wobbleXY"].as_f64() {
                            set(&mut cm.wobble_xy, v.clamp(0.0, 500.0) as f32);
                        }
                        if let Some(v) = c2["wobbleRot"].as_f64() {
                            set(&mut cm.wobble_rot, ((v.clamp(0.0, 90.0) * 100.0).round() / 100.0) as f32);
                        }
                        if let Some(v) = c2["wobbleSpeed"].as_f64() {
                            set(&mut cm.wobble_speed, ((v.clamp(0.1, 60.0) * 100.0).round() / 100.0) as f32);
                        }
                        if let Some(v) = c2["wobbleZoom"].as_bool() {
                            set(&mut cm.wobble_no_zoom, if v { 0.0 } else { 1.0 });
                        }
                        return Ok(());
                    }
                    if c2["op"] == "frame_slow" {
                        // in-between pictures for this frame (0 = off)
                        set(&mut cm.slow_motion, c2["n"].as_i64().unwrap_or(0).clamp(0, crate::timing::MAX_SLOW as i64) as f32);
                        return Ok(());
                    }
                    if delay {
                        let v = c2["seconds"].as_f64().unwrap_or(0.0).clamp(0.0, crate::timing::MAX_DELAY as f64);
                        set(&mut cm.delay, ((v * 100.0).round() / 100.0) as f32);
                    } else {
                        // back at most to the first frame (frame 0 cannot repeat anything)
                        let back = c2["back"].as_i64().unwrap_or(0).clamp(0, ((t * fps).round() as i64).max(0)) as f32;
                        let times = c2["times"].as_i64().unwrap_or(1).clamp(1, 1000) as f32;
                        set(&mut cm.repeat_back, back);
                        set(&mut cm.repeat_times, if back > 0.0 { times } else { 0.0 });
                        if let Some(v) = c2["tween"].as_bool() {
                            set(&mut cm.repeat_tween, if v && back > 0.0 { 1.0 } else { 0.0 });
                        }
                    }
                    Ok(())
                });
            }
            "frame_sound" => {
                // this frame's sound: a clip on the "Frame sounds" layer starting here
                let c2 = c.clone();
                let label = if c2.get("asset").is_some_and(|a| a.is_null()) { "Clear sound" } else { "Frame sound" };
                self.step(label, move |s| {
                    let lid = frame_sounds_layer(s, &sym)?;
                    let fps = st_fps(&s.project);
                    let new_cid = s.project.new_id("C");
                    let clear = c2.get("asset").is_some_and(J::is_null);
                    let pick = match c2["asset"].as_str() {
                        Some(id) if s.project.assets.get(id).is_some_and(|x| x.kind == AssetKind::Audio) => Some(id.to_string()),
                        Some(_) => return crate::edit::fail("no such sound"),
                        None => None,
                    };
                    let l = s.project.layer_mut(&sym, &lid).expect("frame sounds");
                    let LayerKind::Audio(clips) = &mut l.kind else { return crate::edit::fail("not a sound layer") };
                    let at = clips.iter().position(|x| (x.start - t).abs() < 0.25 / fps);
                    if clear {
                        if let Some(i) = at {
                            clips.remove(i);
                        }
                        return Ok(());
                    }
                    let i = match (at, pick) {
                        (Some(i), Some(a)) => {
                            clips[i].asset = Some(a);
                            i
                        }
                        (Some(i), None) => i,
                        (None, Some(a)) => {
                            clips.push(Clip::new(&lid, &new_cid, Some(a), t, 0.0, 0.0));
                            clips.len() - 1
                        }
                        (None, None) => return crate::edit::fail("pick a sound for this frame first"),
                    };
                    let cl = &mut clips[i];
                    if let Some(v) = c2["volume"].as_f64() {
                        cl.volume.value = Value::F(v.clamp(0.0, 1.0) as f32);
                    }
                    if let Some(v) = c2["pan"].as_f64() {
                        cl.pan.value = Value::F(v.clamp(-1.0, 1.0) as f32);
                    }
                    if let Some(v) = c2["pitch"].as_f64() {
                        cl.pitch = v.clamp(0.25, 4.0);
                    }
                    // the Sound board: fades and looping
                    if let Some(v) = c2["fadeIn"].as_f64() {
                        cl.fade_in = v.clamp(0.0, 60.0);
                    }
                    if let Some(v) = c2["fadeOut"].as_f64() {
                        cl.fade_out = v.clamp(0.0, 60.0);
                    }
                    if let Some(v) = c2["loop"].as_bool() {
                        cl.loop_ = v;
                    }
                    Ok(())
                });
            }
            "copy_sound" => {
                if let Some(cl) = frame_sound(self.project(), t) {
                    self.sound_clip = Some(cl.clone());
                    self.message = "Sound copied.".into();
                    self.rev += 1;
                }
            }
            "paste_sound" => {
                if let Some(src) = self.sound_clip.clone() {
                    self.step("Paste sound", move |s| {
                        let lid = frame_sounds_layer(s, &sym)?;
                        let fps = st_fps(&s.project);
                        let cid = s.project.new_id("C");
                        let l = s.project.layer_mut(&sym, &lid).expect("frame sounds");
                        let LayerKind::Audio(clips) = &mut l.kind else { return crate::edit::fail("not a sound layer") };
                        clips.retain(|x| (x.start - t).abs() >= 0.25 / fps);
                        let mut c = Clip::new(&lid, &cid, src.asset.clone(), t, src.in_, src.out);
                        c.pitch = src.pitch;
                        c.volume.value = src.volume.value;
                        c.pan.value = src.pan.value;
                        clips.push(c);
                        Ok(())
                    });
                }
            }
            "frame_bg" => {
                let c2 = c.clone();
                self.step("Background colour", move |s| set_frame_bg(s, &sym, t, &c2));
            }
            "filter" if self.selected_group.is_some() && self.sel_layer().is_none() => {
                // K / D33: the selected group's effects (static values)
                let gid = self.selected_group.clone().expect("a group");
                let c2 = c.clone();
                self.step("Group effects", move |s| crate::edit::filters::apply_group(s, &sym, &gid, &c2));
            }
            "filter" => {
                if let Some(l) = self.sel_layer() {
                    let c2 = c.clone();
                    let keyed = self.keying;
                    self.step("Stickfigure filters", move |s| crate::edit::filters::apply_keyed(s, &sym, &l, &c2, t, keyed));
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
            "stop_animating" => {
                // H3: the × on a property row: the value it has here, on every frame
                if let Some(id) = c["property"].as_str().map(str::to_string) {
                    self.step("Stop animating", |s| {
                        let Some(q) = s.project.property_mut(&id) else { return crate::edit::fail("no such property") };
                        let v = q.key_value(t);
                        q.track.clear();
                        q.value = v;
                        Ok(())
                    });
                }
            }
            "picture" => {
                // the Picture page: scale, flips, opacity, lock (pictures are
                // selected as their layer with no joint)
                if let Some(l) = self.sel_layer() {
                    let c2 = c.clone();
                    let (ed, keyed) = (self.tools.editor.clone(), self.keying);
                    self.step("Picture", move |s| {
                        let (sx, sy, op_id) = {
                            let Some(x) = s.project.root().and_then(|r| r.layer(&l)) else { return crate::edit::fail("no picture") };
                            if !matches!(x.kind, LayerKind::Sprite(_)) {
                                return crate::edit::fail("not a picture");
                            }
                            let mut r = Resolver::new(&s.project);
                            ((r.f(&x.transform.scale_x, t), x.transform.scale_x.id.clone()), (r.f(&x.transform.scale_y, t), x.transform.scale_y.id.clone()), x.opacity.id.clone())
                        };
                        let (mut nx, mut ny) = (sx.0, sy.0);
                        if let Some(v) = c2["scale"].as_f64() {
                            let v = v.clamp(0.01, 100.0) as f32;
                            nx = v * nx.signum();
                            ny = v * ny.signum();
                        }
                        if c2["flipH"].as_bool() == Some(true) {
                            nx = -nx;
                        }
                        if c2["flipV"].as_bool() == Some(true) {
                            ny = -ny;
                        }
                        if nx != sx.0 {
                            ed.change_in(s, &sx.1, Value::F(nx), t)?;
                        }
                        if ny != sy.0 {
                            ed.change_in(s, &sy.1, Value::F(ny), t)?;
                        }
                        if let Some(v) = c2["opacity"].as_f64() {
                            let v = Value::F(v.clamp(0.0, 1.0) as f32);
                            let Some(q) = s.project.property_mut(&op_id) else { return crate::edit::fail("no opacity") };
                            // D5: static unless keying (or already animated)
                            let animated = q.track.keys().windows(2).any(|w| w[0].value != w[1].value);
                            if keyed || animated {
                                q.track.put(Key::new(t, v, Interp::Linear));
                            } else {
                                set_static(q, v);
                            }
                        }
                        if let Some(v) = c2["locked"].as_bool() {
                            if let Some(x) = s.project.layer_mut(&sym, &l) {
                                x.locked = v;
                            }
                        }
                        Ok(())
                    });
                }
            }
            "move_fx" => {
                if let Some(l) = self.sel_layer() {
                    let label = match (c["kind"].as_str(), c["on"].as_bool()) {
                        (Some("wiggle"), Some(true)) => "Add wiggle",
                        (Some("swing"), Some(true)) => "Add swing",
                        (_, Some(false)) => "Remove effect",
                        _ => "Change effect",
                    };
                    self.step(label, |s| crate::edit::filters::movement(s, &sym, &l, c));
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
        // G5: the group lasts while its first figure stays selected
        if !self.multi.is_empty() && self.selected.as_ref().map(|s| &s.0) != self.multi.first() {
            self.multi.clear();
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
        if crate::files::kind_of(file_name) == crate::files::Kind::Sound {
            return self.import_sound(file_name, bytes);
        }
        let lower = file_name.to_lowercase();
        if lower.ends_with(".ttf") || lower.ends_with(".otf") {
            return self.import_font(file_name, bytes);
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
                // an imported figure remembers how it came (D7b)
                if let Some(l) = s.project.layer_mut(&sym, id) {
                    l.capture_base();
                }
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

    /// A sound joins the project's sound list (Frame Tools > Sounds); frames
    /// then pick it.
    /// A font file: kept in the project (so the project carries it) and put
    /// on the selected textfield.
    fn import_font(&mut self, file_name: &str, bytes: &[u8]) -> J {
        // a TrueType / OpenType file starts with one of these
        let ok = bytes.len() > 12 && (bytes.starts_with(&[0, 1, 0, 0]) || bytes.starts_with(b"OTTO") || bytes.starts_with(b"true"));
        if !ok {
            self.message = format!("{file_name} is not a font Barkfart can read (use .ttf or .otf).");
            self.rev += 1;
            return self.state();
        }
        let lower = file_name.to_lowercase();
        let ext = lower.rfind('.').map(|i| lower[i..].to_string()).unwrap_or_default();
        let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
        let name = base.rsplit_once('.').map(|(n, _)| n).unwrap_or(base).to_string();
        let data = std::sync::Arc::new(bytes.to_vec());
        let sym = self.project().root_symbol.clone();
        let text = self.selected_text.clone();
        let made = self.step("Import font", move |s| {
            let aid = s.project.new_id("A");
            let path = format!("assets/{aid}{ext}");
            s.project.assets.insert(aid.clone(), Asset { id: aid.clone(), kind: AssetKind::Font, path: path.clone(), name: Some(name.clone()) });
            s.assets.insert(path, data);
            if let Some(id) = &text {
                if let Some(LayerKind::Text(tl)) = s.project.layer_mut(&sym, id).map(|l| &mut l.kind) {
                    tl.font = Some(aid.clone());
                }
            }
            Ok(name)
        });
        if let Some(name) = made {
            self.message = format!("Imported font {name}.");
        }
        self.state()
    }

    fn import_sound(&mut self, file_name: &str, bytes: &[u8]) -> J {
        let Some(pcm) = crate::audio::decode_sound(bytes) else {
            self.message = format!("Can't play {file_name}. Use mp3, m4a, ogg, flac or wav.");
            self.rev += 1;
            return self.state();
        };
        let lower = file_name.to_lowercase();
        let ext = lower.rfind('.').map(|i| lower[i..].to_string()).unwrap_or_default();
        let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name).to_string();
        let data = std::sync::Arc::new(bytes.to_vec());
        let made = self.step("Import sound", move |s| {
            let aid = s.project.new_id("A");
            let path = format!("assets/{aid}{ext}");
            s.project.assets.insert(aid.clone(), Asset { id: aid.clone(), kind: AssetKind::Audio, path: path.clone(), name: Some(name.clone()) });
            s.assets.insert(path, data);
            Ok(name)
        });
        if let Some(name) = made {
            self.message = format!("Imported sound {name} ({:.1} s).", pcm.seconds());
        }
        self.state()
    }

    /// The stored bytes of an asset file (sounds are decoded from these).
    pub fn asset_bytes(&self, path: &str) -> Option<std::sync::Arc<Vec<u8>>> {
        self.doc.state.assets.get(path).cloned()
    }

    /// The figure editor's size reference (FigureEditor board): the stock
    /// figure at its own size, standing on the same ground just right of the
    /// figure being built, as a project of its own to draw faintly behind;
    /// and where its "Stock size" label goes (handle space, y up).
    pub fn size_reference(&self) -> Option<(Project, [f32; 2])> {
        if !self.size_ref {
            return None;
        }
        let b = self.building.as_ref()?;
        let p = self.project();
        let mut all = CanvasTools::default();
        all.show_static = true;
        let extent = |hs: &mut dyn Iterator<Item = crate::edit::canvas::Handle>| {
            hs.fold(None, |e: Option<[f32; 4]>, h| Some(e.map_or([h.x, h.y, h.x, h.y], |[x0, y0, x1, y1]| [x0.min(h.x), y0.min(h.y), x1.max(h.x), y1.max(h.y)])))
        };
        let [_, by0, bx1, _] = extent(&mut all.handles(p, self.time()).into_iter().filter(|h| h.layer == *b))?;
        let mut stock = self.stock.as_deref().cloned().unwrap_or_else(new_editor_project);
        let [sx0, sy0, sx1, _] = extent(&mut all.handles(&stock, 0.0).into_iter())?;
        // its joints clear the built figure's by a body's width; feet level
        let (dx, dy) = (bx1 + 140.0 - sx0, by0 - sy0);
        let r = stock.root_mut()?;
        for l in r.layers.iter_mut() {
            let l = std::sync::Arc::make_mut(l);
            for (q, d) in [(&mut l.transform.x, dx), (&mut l.transform.y, dy)] {
                q.track.clear();
                q.value = Value::F(q.value.f() + d);
            }
        }
        Some((stock, [(sx0 + sx1) / 2.0 + dx, by0 - 34.0]))
    }

    /// Everything the app draws besides the picture.
    pub fn state(&self) -> J {
        let p = self.project();
        let t = self.time();
        let mut handles = Vec::new();
        // every joint's place, worked out once (the handle list and the figures' main joints both need it)
        let all_handles = self.tools.handles(p, t);
        if self.mode == Mode::Figure {
            // joints connected to the selected one (its parent and children)
            let near: std::collections::HashSet<String> = match &self.selected {
                Some((l, n)) => self.tools.neighbours(p, l, n, false, t).into_iter().map(|h| h.node).collect(),
                None => Default::default(),
            };
            for h in all_handles.iter() {
                if self.building.as_ref().is_some_and(|b| *b != h.layer) {
                    continue;
                }
                let sel = self.selected.as_ref().is_some_and(|(l, n)| *l == h.layer && *n == h.node);
                let fig = self.selected.as_ref().is_some_and(|(l, _)| *l == h.layer);
                let near = fig && near.contains(&h.node);
                let flags = h.root as u32 | (sel as u32) << 1 | (fig as u32) << 2 | (near as u32) << 3;
                if self.building.is_some() {
                    // figure editor View options: node highlight and draw order
                    // (16 polynode, 32 connector, 64 drag / angle lock, 128 polyfill
                    // anchor; then the index)
                    let f = p.root().and_then(|r| r.layer(&h.layer)).and_then(|l| l.as_figure());
                    let (extra, idx) = f.and_then(|f| {
                        let i = f.index_of(&h.node)?;
                        let n = &f.nodes()[i];
                        let poly = f.polyfills.iter().any(|pf| pf.nodes.iter().any(|x| *x == h.node));
                        let conn = n.legacy_extras.contains_key(crate::eval::pose::CONNECT_TO) || n.legacy_extras.get("connector").is_some_and(|v| v == "b:true");
                        let lock = n.drag_lock != 0 || n.angle_lock != 0;
                        // 128: a polyfill hangs off this joint (its anchor; a triangle handle)
                        let anchor = f.polyfills.iter().any(|pf| pf.anchor == h.node);
                        Some(((poly as u32) << 4 | (conn as u32) << 5 | (lock as u32) << 6 | (anchor as u32) << 7, i))
                    }).unwrap_or((0, 0));
                    handles.push(json!([round2(h.x), round2(h.y), flags | extra, idx]));
                    continue;
                }
                handles.push(json!([round2(h.x), round2(h.y), flags]));
            }
        }
        let frames = self.frame_count();
        let keyed = self.keyed_frames(frames);
        let figures: Vec<J> = p
            .root()
            .map(|s| {
                // where each figure's main joint is (View options > figure IDs)
                let roots: std::collections::HashMap<String, (f32, f32)> = all_handles.iter().filter(|h| h.root).map(|h| (h.layer.clone(), (h.x, h.y))).collect();
                s.layers.iter().filter(|l| l.as_figure().is_some()).map(|l| json!({"id": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone()), "at": roots.get(&l.id).map(|(x, y)| json!([round2(*x), round2(*y)]))})).collect()
            })
            .unwrap_or_default();
        let sel = self.selected.as_ref().and_then(|(l, n)| {
            let layer = p.root()?.layer(l)?;
            let f = layer.as_figure()?;
            let node = f.node(n)?;
            // F4: the pose (block) under the playhead
            let pose = {
                let fps = st_fps(p);
                let fr = (t * fps).round() as i32;
                let n = (p.root().map(|r| r.duration).unwrap_or(0.0) * fps).round().max(1.0) as i32;
                crate::edit::block_ops::block_at(layer, fps, fr).map(|(start, next)| {
                    let key = node.rotation.track.keys().iter().find(|k| ((k.time * fps).round() as i32) == start).copied();
                    json!({
                        "start": start, "length": next.unwrap_or(n) - start, "last": next.is_none(),
                        "hold": key.is_some_and(|k| k.interp == Interp::Hold),
                        // the "easing" command's names
                        "ease": key.map(|k| match k.interp {
                            Interp::Hold => "hold",
                            Interp::Linear => "linear",
                            Interp::Smooth => "auto",
                            _ if k.ease == Ease::new(0.42, 0.0, 1.0, 1.0) => "in",
                            _ if k.ease == Ease::new(0.0, 0.0, 0.58, 1.0) => "out",
                            _ if k.ease.kind == EaseKind::BounceOut => "bounce",
                            _ => "custom",
                        }),
                        "here": start == fr,
                        // D34: the pose's handles (the Smooth curve's shape here)
                        "handles": key.map(|k| k.handle.name()),
                    })
                })
            };
            let mut r = Resolver::new(p);
            Some(json!({
                "layer": l, "node": n, "name": layer.name.clone().unwrap_or_else(|| l.clone()),
                "root": node.parent.is_none(), "static": node.is_static, "pinned": node.pinned,
                "smart": node.smart_stretch, "shape": shape_name(node.shape),
                "thickness": r.f(&node.thickness, t), "color": color_of(&mut r, &node.color, t),
                "angle": round2(r.f(&node.rotation, t)), "length": round2(r.f(&node.length, t)), "restLength": round2(node.rest_length),
                "gradient": node.gradient, "gradientColor": color_of(&mut r, &node.gradient_color, t), "gradientReverse": node.gradient_reverse, "gradientSideways": node.gradient_sideways,
                "segScale": round2(r.f(&node.scale, t)), "curve": round2(node.curve), "outline": node.circle_outline, "outlineColor": color_of(&mut r, &node.outline_color, t),
                "hollow": node.circle_hollow, "halfArc": node.half_arc, "sides": node.polygon_sides, "trapRatio": round2(node.trapezoid_ratio), "trapRoundStart": node.trapezoid_round_start, "trapRoundEnd": node.trapezoid_round_end, "rightTriangle": node.right_triangle, "upsideDown": node.triangle_upside_down, "angleLock": node.angle_lock,
                "stretchy": node.stretchy, "ownColor": node.own_color,
                // how the segment came (D7b), for changed-dots and resets; absent
                // for figures from before D7b
                "defaults": f.base_of(n).map(|b| json!({
                    "shape": shape_name(b.shape), "length": round2(b.length), "thickness": b.thickness, "segScale": round2(b.scale), "color": b.color.to_rgba8888(),
                    "ownColor": b.own_color, "curve": round2(b.curve), "static": b.is_static, "stretchy": b.stretchy, "smart": b.smart_stretch,
                    "gradient": b.gradient, "gradientColor": b.gradient_color.to_rgba8888(), "gradientReverse": b.gradient_reverse, "gradientSideways": b.gradient_sideways,
                    "outline": b.circle_outline, "outlineColor": b.outline_color.to_rgba8888(), "hollow": b.circle_hollow, "halfArc": b.half_arc, "sides": b.polygon_sides,
                    "trapRatio": round2(b.trapezoid_ratio), "trapRoundStart": b.trapezoid_round_start, "trapRoundEnd": b.trapezoid_round_end, "rightTriangle": b.right_triangle, "upsideDown": b.triangle_upside_down,
                })),
                "connector": node.legacy_extras.get(crate::eval::pose::CONNECT_TO).map(|to| {
                    let num = |k: &str, d: f64| node.legacy_extras.get(k).and_then(|v| v.split_once(':')).and_then(|(_, x)| x.parse::<f64>().ok()).unwrap_or(d);
                    json!({"to": to.trim_start_matches("s:"), "percent": num("C.cPercent", 0.5), "reversed": num("C.cReversed", 0.0) != 0.0})
                }), "dragLock": node.drag_lock != 0,
                "scale": round2(r.f(&layer.transform.scale_y, t).abs()), "figAngle": round2(r.f(&layer.transform.rotation, t)),
                "locked": layer.locked, "onion": layer.onion,
                "joinedTo": layer.parent_at(t).0.and_then(|pid| p.root()?.layer(pid)).map(|x| x.name.clone().unwrap_or_else(|| x.id.clone())),
                "blend": layer.blend.name(),
                "keepInTween": !layer.vanish_in_tween,
                // F4: the pose (block) under the playhead
                "pose": pose,
                "tweenColors": !layer.colors_jump,
                "figColor": f.color.map(|c| c.to_rgba8888()).unwrap_or_else(|| f.nodes().get(1).map(|n| color_of(&mut r, &n.color, t)).unwrap_or(0x000000ff)),
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
            let (w, h) = text_size(tl, &self.fonts);
            let (x0, y0, x1, y1) = text_rect(tl, cx, cy, w * sx, h * sy);
            Some(json!({
                "layer": id, "name": layer.name.clone().unwrap_or_else(|| id.clone()),
                "cameraLocked": layer.parent.as_ref().is_some_and(|pid| p.root().and_then(|r| r.layer(pid)).is_some_and(|x| matches!(x.kind, LayerKind::Camera(_)))),
                "text": tl.text.clone().unwrap_or_default(),
                "font": tl.font,
                "color": color_of(&mut r, &tl.color, t),
                "scale": round2(sy),
                "align": align_name(tl.align, tl.corner),
                "locked": layer.locked,
                "x": round2(cx), "y": round2(cy),
                "x0": round2(x0), "y0": round2(y0), "x1": round2(x1), "y1": round2(y1),
            }))
        });
        // Auto-camera starting on this frame: its end frame and ease
        let auto_cam = p.root().and_then(|r| r.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).and_then(|c| {
            let fps = st_fps(p);
            let t0 = self.frame as f64 / fps;
            crate::eval::auto_camera_at(c, t0).filter(|(a, _, _)| (a - t0).abs() < 0.25 / fps).map(|(_, b, code)| json!({"end": (b * fps).round() as i64, "ease": code}))
        });
        // Camera Tools: this frame's widescreen and wobble
        let cam_fx = p.root().and_then(|r| r.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).map(|c| {
            let fps = st_fps(p);
            let g = |q: &Property| crate::timing::setting(q, fps, self.frame);
            let speed = g(&c.wobble_speed);
            json!({"widescreen": g(&c.widescreen) > 0.5, "wobbleXY": g(&c.wobble_xy), "wobbleRot": g(&c.wobble_rot), "wobbleSpeed": if speed > 0.0 { speed } else { 3.0 }, "wobbleZoom": g(&c.wobble_no_zoom) < 0.5})
        });
        // Movieclip Tools: the selected movieclip's playback
        let movieclip = self.selected.as_ref().and_then(|(l, _)| {
            let layer = p.root()?.layer(l)?;
            let LayerKind::Symbol(sl) = &layer.kind else { return None };
            let mut r = Resolver::new(p);
            let mode = match sl.loop_mode { SymbolLoop::Once => "once", SymbolLoop::Loop => "loop", SymbolLoop::PingPong => "pingpong" };
            Some(json!({"layer": l, "name": layer.name.clone().unwrap_or_else(|| l.clone()), "loop": mode, "speed": round2(r.d(&sl.speed, t) as f32), "offset": round2(r.d(&sl.time_offset, t) as f32), "frames": p.symbol(&sl.symbol).map(|s| (s.duration * st_fps(p)).round() as i64).unwrap_or(0)}))
        });
        // the motion path: the selected joint's place on every frame, and the
        // frames its figure has poses on (numbered on the stage)
        let motion_path = self.selected.as_ref().filter(|_| self.motion_path && self.mode == Mode::Figure).and_then(|(l, n)| {
            let layer = p.root()?.layer(l)?;
            let f = layer.as_figure()?;
            f.node(n)?.parent.as_ref()?;
            let fps = st_fps(p);
            let pts = self.tools.joint_path(p, l, n, frames as i32, fps);
            let keys = crate::edit::block_ops::pose_frames(layer, fps);
            Some(json!({"points": pts.iter().map(|(x, y)| json!([round2(*x), round2(*y)])).collect::<Vec<_>>(), "keys": keys}))
        });
        // the Picture page: the selected picture
        let picture = self.selected.as_ref().and_then(|(l, n)| {
            if !n.is_empty() {
                return None;
            }
            let layer = p.root()?.layer(l)?;
            let LayerKind::Sprite(sp) = &layer.kind else { return None };
            let mut r = Resolver::new(p);
            let (sx, sy) = (r.f(&layer.transform.scale_x, t), r.f(&layer.transform.scale_y, t));
            // the file's own name when it was kept, else the picture's name
            let file = sp.asset.as_ref().and_then(|a| p.assets.get(a)).and_then(|a| a.name.clone());
            Some(json!({
                "layer": l, "name": layer.name.clone().unwrap_or_else(|| l.clone()), "file": file,
                "scale": round2(sy.abs()), "flipH": sx < 0.0, "flipV": sy < 0.0, "opacity": round2(r.f(&layer.opacity, t)),
                "blend": layer.blend.name(), "locked": layer.locked,
                "box": self.tools.sprite_box(p, l, t).map(|b| b.iter().map(|v| round2(*v)).collect::<Vec<_>>()),
                "cameraLocked": layer.parent.as_ref().and_then(|pid| p.root()?.layer(pid)).is_some_and(|x| matches!(x.kind, LayerKind::Camera(_))),
            }))
        });
        // Lock Figure To Camera: the locked ones (parallax in the original's units)
        let camera_locks: Vec<J> = p.root().map(|r| {
            r.layers.iter().filter(|l| l.parent.as_ref().is_some_and(|pid| r.layer(pid).is_some_and(|x| matches!(x.kind, LayerKind::Camera(_))))).map(|l| {
                json!({"layer": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone()), "parallax": round2(1.0 - l.parallax), "rotate": !l.lock_move_only})
            }).collect()
        }).unwrap_or_default();
        // Library Tools: every figure, movieclip and sprite with the frames it is on
        let library: Vec<J> = p.root().map(|r| {
            let mut res = Resolver::new(p);
            let fps = st_fps(p);
            let n = ((r.duration * fps).round() as i32).max(1);
            r.layers.iter().filter_map(|l| {
                let (kind, nodes) = match &l.kind {
                    LayerKind::Figure(f) => ("figure", f.nodes().len()),
                    LayerKind::Symbol(_) => ("movieclip", 0),
                    LayerKind::Sprite(_) => ("sprite", 0),
                    _ => return None,
                };
                let on = (0..n).filter(|i| res.b(&l.shown, *i as f64 / fps)).count();
                Some(json!({"layer": l.id, "kind": kind, "name": l.name.clone().unwrap_or_else(|| l.id.clone()), "nodes": nodes, "frames": on, "joined": r.layers.iter().filter(|x| x.parent.as_deref() == Some(l.id.as_str())).count()}))
            }).collect()
        }).unwrap_or_default();
        let bg = {
            let (c, g, u) = frame_bg(p, t);
            let fps = st_fps(p);
            let expand = p.root().and_then(|r| r.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).is_some_and(|c| crate::timing::setting(&c.bg_expand, fps, self.frame) > 0.5);
            json!({"color": c, "gradient": g, "useGradient": u, "expand": expand})
        };
        let sound_list: Vec<J> = p.assets.values().filter(|a| a.kind == AssetKind::Audio).map(|a| json!({"id": a.id, "name": sound_name(p, &a.id)})).collect();
        let frame_snd = frame_sound(p, t).map(|c| {
            let a = c.asset.clone().unwrap_or_default();
            let vol = if let Value::F(v) = c.volume.value { v } else { 1.0 };
            let pan = if let Value::F(v) = c.pan.value { v } else { 0.0 };
            json!({"asset": a, "name": sound_name(p, &a), "volume": round2(vol), "pan": round2(pan), "pitch": (c.pitch * 100.0).round() / 100.0,
                "fadeIn": (c.fade_in * 10.0).round() / 10.0, "fadeOut": (c.fade_out * 10.0).round() / 10.0, "loop": c.loop_})
        });
        // the segment being drawn (J5): its ends, its length and angle in the
        // figure's own space, how thick it will be on the stage, the shape;
        // and its mirror image when Mirror is on
        let draw_line = match &self.gesture {
            Gesture::Draw { layer, from, x0, y0, x, y, .. } => {
                let sym = self.sym();
                let shape = self.draw.unwrap_or(Shape::Line);
                let (len, deg, width) = draw_measure(p, &sym, layer, from, *x, *y, t).unwrap_or((0.0, 0.0, 0.0));
                let twin = if self.mirror { mirrored(p, &sym, layer, from, *x, *y, t) } else { None };
                json!({"line": [x0, y0, x, y], "length": len.round(), "angle": deg.round(), "width": round2(width as f32), "shape": shape_name(shape),
                    "mirror": twin.map(|(_, a, b, c, d)| json!([round2(a), round2(b), round2(c), round2(d)]))})
            }
            _ => J::Null,
        };
        let fset = crate::timing::frame_settings(p, self.frame);
        // per thumbnail: [delay, repeat] flags
        let rep = json!({"back": fset.1, "times": fset.2, "tween": crate::timing::frame_repeat_tween(p, self.frame)});
        // joints of everything shown on this frame (the original's node counter)
        let node_count: usize = {
            let mut r = Resolver::new(p);
            p.root().map(|s| s.layers.iter().filter(|l| r.b(&l.shown, t)).filter_map(|l| l.as_figure()).map(|f| f.nodes().len()).sum()).unwrap_or(0)
        };
        let marks: Vec<J> = (0..frames).map(|f| { let (d, _, n) = crate::timing::frame_settings(p, f as i32); json!([d > 0.0, n > 0, crate::timing::frame_silences(p, f as i32), crate::timing::frame_slow(p, f as i32) > 0, p.root().and_then(|r| r.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).is_some_and(|c| crate::timing::setting(&c.auto_cam, st_fps(p), f as i32) > 0.5)]) }).collect();
        let cam = self.camera_frame();
        let cam_vals = p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(c) } else { None })).map(|c| {
            let mut r = Resolver::new(p);
            json!({"zoom": round2(r.f(&c.zoom, t)), "rotation": round2(r.f(&c.rotation, t))})
        });
        // G5: the figures selected together, with what the multi-select page
        // shows mixed: colour, locked, onion skin, the pose under the playhead
        let multi: Option<Vec<J>> = (self.multi.len() > 1).then(|| {
            let fps = st_fps(p);
            let fr = self.frame;
            self.multi
                .iter()
                .filter_map(|id| {
                    let l = p.root()?.layer(id)?;
                    let f = l.as_figure()?;
                    let block = crate::edit::block_ops::block_at(l, fps, fr);
                    let hold = block.and_then(|(st, _)| crate::edit::block_ops::pose_props_of(l).iter().find_map(|q| q.track.keys().iter().find(|k| ((k.time * fps).round() as i32) == st).copied())).is_some_and(|k| k.interp == Interp::Hold);
                    Some(json!({
                        "id": id, "name": l.name.clone().unwrap_or_else(|| id.clone()),
                        "color": f.color.map(|c| c.to_rgba8888()), "locked": l.locked, "onion": l.onion,
                        "hold": hold, "here": block.is_some_and(|(st, _)| st == fr),
                        // the multi-select page's "Hero has Glow" line and opacity
                        "effects": crate::edit::filters::state(p, &p.root_symbol, id, t)["on"].as_object().map(|m| m.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect::<Vec<_>>()).unwrap_or_default(),
                        "opacity": round2(Resolver::new(p).f(&l.opacity, t)),
                    }))
                })
                .collect()
        });
        // G1: the expanded timeline's rows, front first (row order is draw
        // order). Each: kind, the eye, the frames it is on, its blocks
        // [start, length, hold] (a block is a pose, lasting to the next).
        let rows: Vec<J> = p
            .root()
            .map(|s| {
                let fps = st_fps(p);
                // the camera row leads (round 10 board), wherever the camera sits
                // in the layer list (it draws nothing, so its place there is moot)
                let mut order: Vec<&std::sync::Arc<Layer>> = s.layers.iter().rev().collect();
                order.sort_by_key(|l| !matches!(l.kind, LayerKind::Camera(_)));
                let n = (frames as i32).max(1);
                let mut r = Resolver::new(p);
                order
                    .into_iter()
                    .map(|l| {
                        // M5: a sound row is its clips: [first frame, frames, asset, in, out]
                        if let LayerKind::Audio(clips) = &l.kind {
                            let list: Vec<J> = clips
                                .iter()
                                .map(|c| {
                                    // a clip without an end plays to the end of its file
                                    let out = if c.out > c.in_ { c.out } else { c.asset.as_deref().map(|a| self.sound_length(a)).unwrap_or(0.0) };
                                    let secs = ((out - c.in_) / c.pitch.max(0.01)).max(0.0);
                                    json!([round2((c.start * fps) as f32), round2((secs * fps) as f32), c.asset, c.in_, out])
                                })
                                .collect();
                            return json!({"id": l.id, "name": l.name.clone().unwrap_or_else(|| "Sound".into()), "kind": "sound", "visible": l.visible, "clips": list, "blocks": [], "on": [], "props": []});
                        }
                        let mut starts = crate::edit::block_ops::pose_frames(l, fps);
                        // M5: the camera is keyed on every frame; its row shows where
                        // the view changes, so equal neighbours read as one block
                        if matches!(l.kind, LayerKind::Camera(_)) {
                            let props = crate::edit::block_ops::pose_props_of(l);
                            let at = |f: i32| props.iter().map(|q| q.key_value(f as f64 / fps)).collect::<Vec<_>>();
                            let mut kept: Vec<i32> = Vec::new();
                            for f in starts {
                                if kept.last().is_none_or(|&prev| at(prev) != at(f)) {
                                    kept.push(f);
                                }
                            }
                            starts = kept;
                        }
                        let props = crate::edit::block_ops::pose_props_of(l);
                        let blocks: Vec<J> = starts
                            .iter()
                            .enumerate()
                            .filter(|(_, st)| **st < n)
                            .map(|(i, st)| {
                                let next = starts.get(i + 1).copied().unwrap_or(n).min(n);
                                let hold = props.iter().find_map(|q| q.track.keys().iter().find(|k| ((k.time * fps).round() as i32) == *st)).is_some_and(|k| k.interp == Interp::Hold);
                                json!([st, next - st, hold])
                            })
                            .collect();
                        let on: Vec<bool> = (0..n.min(2000)).map(|f| r.b(&l.shown, f as f64 / fps)).collect();
                        let kind = match &l.kind {
                            LayerKind::Figure(_) => "figure",
                            LayerKind::Sprite(_) => "picture",
                            LayerKind::Text(_) => "text",
                            LayerKind::Symbol(_) => "movieclip",
                            LayerKind::Camera(_) => "camera",
                            _ => "other",
                        };
                        // H3: animated values other than the pose (effects, opacity,
                        // colours): a thin row each, a diamond where the value changes
                        let pose_ids: std::collections::HashSet<&str> = props.iter().map(|q| q.id.as_str()).collect();
                        let animated: Vec<J> = l
                            .properties()
                            .into_iter()
                            .filter(|q| q.id != l.shown.id && !pose_ids.contains(q.id.as_str()))
                            .filter_map(|q| {
                                let keys = q.track.keys();
                                let mut marks: Vec<i32> = Vec::new();
                                for (i, k) in keys.iter().enumerate() {
                                    if i == 0 || keys[i - 1].value != k.value {
                                        marks.push((k.time * fps).round() as i32);
                                    }
                                }
                                (marks.len() > 1 || (keys.len() > 1 && keys.iter().any(|k| k.value != keys[0].value))).then(|| json!({"id": q.id, "name": prop_label(l, &q.id), "keys": marks}))
                            })
                            .collect();
                        json!({"id": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone()), "kind": kind, "visible": l.visible, "on": on, "blocks": blocks, "joined": l.parent.is_some(),
                            // M5: what it is joined to (the chain on its row)
                            "joinedTo": l.parent.as_ref().and_then(|pid| s.layer(pid)).filter(|x| !matches!(x.kind, LayerKind::Camera(_))).map(|x| x.name.clone().unwrap_or_else(|| x.id.clone())),
                            // I2: joins over time, [frame, joined to (null: lets go), what it let go of]
                            "joins": l.joins.iter().enumerate().map(|(i, j)| {
                                let name = |id: Option<&str>| id.and_then(|pid| s.layer(pid)).map(|x| x.name.clone().unwrap_or_else(|| x.id.clone()));
                                let prev = if i == 0 { l.parent.as_deref() } else { l.joins[i - 1].parent.as_deref() };
                                json!([((j.time * fps).round() as i32), name(j.parent.as_deref()), name(prev)])
                            }).collect::<Vec<_>>(),
                            "props": animated, "group": l.group})
                    })
                    .collect()
            })
            .unwrap_or_default();
        // K / D33: a group's folder row comes right before its members (which
        // sit together), with a tick wherever a member has a pose
        let rows: Vec<J> = {
            let groups = p.root().map(|r| r.groups.clone()).unwrap_or_default();
            let mut out: Vec<J> = Vec::with_capacity(rows.len() + groups.len());
            let mut seen: Vec<String> = Vec::new();
            for (i, r) in rows.iter().enumerate() {
                if let Some(gid) = r["group"].as_str() {
                    if !seen.iter().any(|x| x == gid) {
                        seen.push(gid.to_string());
                        let members: Vec<&J> = rows[i..].iter().filter(|x| x["group"].as_str() == Some(gid)).collect();
                        let mut ticks: Vec<i64> = members.iter().flat_map(|m| m["blocks"].as_array().map(|b| b.iter().filter_map(|x| x[0].as_i64()).collect::<Vec<_>>()).unwrap_or_default()).collect();
                        ticks.sort_unstable();
                        ticks.dedup();
                        let g = groups.iter().find(|x| x.id == gid);
                        out.push(json!({"id": gid, "kind": "group", "name": g.map(|x| x.name.clone()).unwrap_or_default(), "open": g.is_none_or(|x| x.open),
                            "visible": members.iter().any(|m| m["visible"] != false), "count": members.len(), "ticks": ticks, "blocks": [], "on": [], "props": []}));
                    }
                }
                out.push(r.clone());
            }
            out
        };
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
            "handles": handles, "selected": sel, "figures": figures, "rows": rows, "multi": multi,
            // H1: the FX toggle shows only when something has effects
            "hasEffects": p.root().is_some_and(|r| r.layers.iter().any(|l| crate::edit::filters::any_active(l)) || r.groups.iter().any(|g| crate::edit::filters::stack_active(&g.effects))), "camera": cam, "movieclip": movieclip, "picture": picture, "motionPath": motion_path,
            // J3: the Parts list while editing a figure: front first, each
            // segment (shape, colour, its number) and each polyfill just behind
            // its anchor; the main joint is the figure itself
            "parts": self.building.as_ref().and_then(|b| p.root()?.layer(b)).and_then(|l| l.as_figure()).map(|f| {
                let mut r = Resolver::new(p);
                let seq = f.draw_sequence();
                let mut out: Vec<J> = Vec::new();
                for &i in seq.iter().rev() {
                    let n = &f.nodes()[i];
                    if n.parent.is_none() {
                        continue;
                    }
                    let connector = n.legacy_extras.contains_key(crate::eval::pose::CONNECT_TO) || n.legacy_extras.get("connector").is_some_and(|v| v == "b:true");
                    out.push(json!({"id": n.id, "kind": if connector { "connector" } else { "segment" }, "shape": shape_name(n.shape), "color": color_of(&mut r, &n.color, t), "n": i + 1}));
                    for (k, pf) in f.polyfills.iter().enumerate().filter(|(_, pf)| pf.anchor == n.id) {
                        out.push(json!({"id": pf.id, "kind": "polyfill", "color": color_of(&mut r, &pf.color, t), "n": f.nodes().len() + k + 1}));
                    }
                }
                json!({"list": out, "nodes": f.nodes().len()})
            }),
            // J1: the copy being edited, and the figure it came from
            "copyEdit": self.copy_edit.as_ref().and_then(|(_, l, _)| {
                let copy = self.building.as_ref().and_then(|b| p.root()?.layer(b)).and_then(|x| x.name.clone());
                let of = self.doc.state.project.root().and_then(|r| r.layer(l)).and_then(|x| x.name.clone());
                Some(json!({"name": copy?, "of": of.unwrap_or_else(|| "The figure".into())}))
            }), "editingClip": (p.root_symbol != self.main_root).then(|| p.root().and_then(|r| r.name.clone()).unwrap_or_else(|| "Movieclip".into())),
            "branchCopied": self.branch_clip.is_some(),
            "watermark": p.settings.watermark.as_ref().map(|(t, c)| json!({"text": t, "color": c.to_rgba8888()})),
            "propsFrom": self.props_clip.as_ref().and_then(|c| p.root()?.layer(&c.source).map(|l| l.name.clone().unwrap_or_else(|| l.id.clone()))),
            "textfield": textfield, "textfieldCopied": self.textfield_clip.is_some(),
            "projectFonts": p.assets.values().filter(|a| a.kind == AssetKind::Font).map(|a| json!({"key": a.id, "name": a.name.clone().unwrap_or_else(|| a.id.clone())})).collect::<Vec<_>>(),
            "filtersCopied": self.filter_clip.is_some(),
            "frameBg": bg, "camFx": cam_fx, "autoCam": auto_cam, "library": library, "cameraLocks": camera_locks,
            "drawTool": self.draw.map(shape_name).unwrap_or("none"), "drawLine": draw_line,
            "soundList": sound_list, "frameSound": frame_snd, "soundCopied": self.sound_clip.is_some(),
            "frameDelay": fset.0, "frameRepeat": rep,
            "frameMarks": marks, "frameSilence": crate::timing::frame_silences(p, self.frame), "frameSlow": crate::timing::frame_slow(p, self.frame), "nodeCount": node_count,
            "hasCamera": p.root().is_some_and(|s| s.layers.iter().any(|l| matches!(l.kind, LayerKind::Camera(_)))),
            "opts": {
                "stretch": self.stretch_all, "scope": if self.tools.key_scope == KeyScope::Joint { "joint" } else { "figure" },
                "snap": self.tools.snap_degrees, "onion": self.onion, "keying": self.keying, "path": self.motion_path, "axis": self.tools.axis_lock, "static": self.tools.show_static,
                "tweenNew": self.tools.editor.default_interp != Interp::Hold, "mainOnly": self.tools.main_nodes_only, "sizeRef": self.size_ref, "mirror": self.mirror,
                "onionBefore": p.view.onion_before, "onionAfter": p.view.onion_after, "onionFront": p.view.onion_in_front,
            },
            "pose": self.pose_clip.is_some(), "posesCopied": self.poses_clip.len(),
            "group": self.selected_group.as_ref().filter(|_| self.selected.is_none() && self.selected_text.is_none() && self.multi.is_empty()).and_then(|gid| {
                let r = p.root()?;
                let g = r.groups.iter().find(|x| x.id == *gid)?;
                let members: Vec<&std::sync::Arc<Layer>> = r.layers.iter().rev().filter(|l| l.group.as_deref() == Some(gid.as_str())).collect();
                Some(json!({"id": g.id, "name": g.name, "opacity": round2(g.opacity), "open": g.open,
                    // the effects that are on (a switched-off one keeps its settings, silent)
                    "effects": group_on(p, gid),
                    "filters": crate::edit::filters::group_state(p, &p.root_symbol, gid),
                    "members": members.iter().map(|l| json!({"id": l.id, "name": l.name.clone().unwrap_or_else(|| l.id.clone())})).collect::<Vec<_>>()}))
            }),
            "groups": p.root().map(|r| r.groups.iter().map(|g| json!({
                "id": g.id, "name": g.name, "opacity": round2(g.opacity), "open": g.open, "effects": group_on(p, &g.id).len(),
                "members": r.layers.iter().rev().filter(|l| l.group.as_deref() == Some(g.id.as_str())).map(|l| l.id.clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()).unwrap_or_default(),
            "joining": self.joining.is_some(),
            "frameCopied": self.frame_clip.is_some(),
            "figureCopied": self.figure_clip.is_some(), "propsCopied": self.props_clip.is_some(),
            "cam": cam_vals,
            "tweening": p.settings.tweening, "loopTween": p.settings.loop_tween, "tweenedFrames": p.settings.tweened_frames, "rate": p.settings.sample_rate(),
            "frameNoTween": no_tween,
            "noTweenFrames": self.no_tween_frames(frames),
            "frameClipCount": self.frame_clip.as_ref().map(|c| c.1).unwrap_or(0),
            "connecting": self.connecting,
            "editorBg": self.editor_bg,
            "sizeRef": self.size_reference().map(|(_, at)| json!([round2(at[0]), round2(at[1])])),
            "trace": self.trace.as_ref().map(|(f, l)| json!({"frame": f, "layer": l})),
            "polyfill": self.polyfill.as_ref().and_then(|id| {
                let f = p.root()?.layer(self.building.as_ref()?)?.as_figure()?;
                let pf = f.polyfills.iter().find(|x| &x.id == id)?;
                Some(json!({"id": pf.id, "anchor": pf.anchor, "count": pf.nodes.len(), "own": pf.use_own_color, "color": pf.color.value.color().to_rgba8888()}))
            }),
            "polyfills": self.building.as_ref().and_then(|b| p.root()?.layer(b)?.as_figure()).map(|f| f.polyfills.iter().map(|x| json!({"id": x.id, "anchor": x.anchor, "count": x.nodes.len()})).collect::<Vec<_>>()),
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

    /// Save stickfigure: the figure being edited (or the selected one) as a
    /// Stick Nodes .nodes file, in its pose on this frame, with its name.
    pub fn figure_file(&self) -> Result<(String, Vec<u8>), String> {
        let id = self.building.clone().or_else(|| self.sel_layer()).ok_or("no stickfigure selected")?;
        let p = self.project();
        let l = p.root().and_then(|s| s.layer(&id)).ok_or("no stickfigure")?;
        if l.as_figure().is_none() {
            return Err("not a stickfigure".into());
        }
        let t = self.time();
        let mut copy = l.clone();
        let mut r = Resolver::new(p);
        for q in copy.properties_mut() {
            if !q.track.is_empty() {
                q.value = r.resolve(q, t);
                q.track.clear();
            }
        }
        // connectors made in Barkfart: the original has no record of their
        // endpoint, so they are saved where they sit (their angle and length)
        if let LayerKind::Figure(f) = &mut copy.kind {
            let pose = crate::eval::pose::FigurePose::compute(f, 0.0, &mut Resolver::new(p));
            let ids: Vec<usize> = f.nodes().iter().enumerate().filter(|(_, n)| n.legacy_extras.contains_key(crate::eval::pose::CONNECT_TO) && n.parent.is_some()).map(|(i, _)| i).collect();
            let base: Vec<f64> = ids.iter().map(|&i| pose.base_angle(f.nodes(), i)).collect();
            for (k, &i) in ids.iter().enumerate() {
                let pi = pose.parent[i];
                if pi < 0 {
                    continue;
                }
                let pi = pi as usize;
                let (dx, dy) = (pose.x[i] - pose.x[pi], pose.y[i] - pose.y[pi]);
                let n = &mut f.nodes_mut()[i];
                let scale = n.scale.value.f().max(1e-6) as f64;
                n.length.value = Value::F((dx.hypot(dy) / scale) as f32);
                n.rotation.value = Value::F((dy.atan2(dx).to_degrees() - base[k]) as f32);
                n.legacy_extras.shift_remove(crate::eval::pose::CONNECT_TO);
            }
        }
        let fig = crate::legacy::to_legacy(&copy).map_err(|e| e.0)?;
        Ok((l.name.clone().unwrap_or_else(|| "Stickfigure".into()), crate::legacy::write_nodes(&fig)))
    }

    /// The project as a .bark file without marking it saved.
    pub fn save_copy(&self) -> Result<Vec<u8>, String> {
        let assets: IndexMap<String, Vec<u8>> = self.doc.state.assets.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
        crate::codec::write_bark(&self.saved_project(), &assets).map_err(|e| e.0)
    }

    /// The project as it is saved: the main animation as the root, even
    /// while a movieclip is being edited.
    pub fn saved_project(&self) -> std::borrow::Cow<'_, Project> {
        if self.project().root_symbol == self.main_root {
            std::borrow::Cow::Borrowed(self.project())
        } else {
            let mut p = self.project().clone();
            p.root_symbol = self.main_root.clone();
            std::borrow::Cow::Owned(p)
        }
    }

    /// The project as a .bark file; marks it saved.
    pub fn save(&mut self) -> Result<Vec<u8>, String> {
        if self.project().root_symbol == self.main_root {
            self.doc.project_mut().view.frame = self.frame;
        }
        let assets: IndexMap<String, Vec<u8>> = self.doc.state.assets.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect();
        let bytes = crate::codec::write_bark(&self.saved_project(), &assets).map_err(|e| e.0)?;
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
/// The whole-figure key rule (REDESIGN D2, F3), run around every edit: a key
/// on a figure keys all of it (every joint's angle and length, and its
/// transform) at the same moment, and the camera likewise; nothing else is
/// keyed, so between its own keys a figure holds or tweens. Being shown stays
/// keyed on every frame (a held value: it changes nothing on screen, and
/// "delete on this frame only" relies on it).
pub fn anchor_poses(st: &mut crate::edit::State) {
    let n = frame_ops::frame_count(st);
    let fps = st_fps(&st.project);
    // D34: a pose's first keys move as the project's tweens do
    let tw = st.project.settings.tween_interp;
    let sym = st.project.root_symbol.clone();
    let ids: Vec<String> = st.project.root().map(|s| s.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
    for id in ids {
        let Some(l) = st.project.layer_mut(&sym, &id) else { continue };
        key_every_frame(&mut l.shown, n, fps);
        let camera = matches!(l.kind, LayerKind::Camera(_) | LayerKind::Text(_));
        let ps = crate::edit::block_ops::pose_props(l);
        if camera {
            // the camera and textfields stay frame-by-frame things, as in
            // Stick Nodes (a textfield sits on one frame)
            for q in ps {
                key_every_frame(q, n, fps);
            }
            continue;
        }
        // every moment any of them is keyed (on the frame grid), and the first
        // frame: a pose is never left unkeyed, so frame commands (which copy
        // and move keys) always have keys to work with
        let mut frames: Vec<i64> = ps.iter().flat_map(|q| q.track.keys().iter().map(|k| (k.time * fps).round() as i64)).collect();
        frames.push(0);
        frames.sort_unstable();
        frames.dedup();
        for q in ps {
            key_at(q, &frames, fps, tw);
        }
    }
}

/// A readable name for a layer's property (H3 property rows): "Glow amount",
/// "Blur radius", "Opacity", "armR1 colour".
fn prop_label(l: &Layer, id: &str) -> String {
    let rest = id.strip_prefix(&format!("{}/", l.id)).unwrap_or(id);
    let word = |p: &str| match p {
        "mix" => "amount".to_string(),
        "color" | "tint" => "colour".to_string(),
        "gradientColor" => "gradient colour".to_string(),
        "outlineColor" => "outline colour".to_string(),
        other => other.to_string(),
    };
    if rest == "opacity" {
        return "Opacity".into();
    }
    if let Some((node, param)) = rest.split_once('/') {
        if let Some(kind) = node.strip_prefix("filter.") {
            let name = match kind {
                "invert" => "Invert",
                "adjust" => "Colour",
                "pixelate" => "Pixelate",
                "tint" => "Tint",
                "outline" => "Outline",
                "blur" => "Blur",
                "motion" => "Motion blur",
                "glow" => "Glow",
                "shadow" => "Shadow",
                k => k,
            };
            return if kind == "invert" && param == "mix" { name.into() } else { format!("{name} {}", word(param)) };
        }
        return format!("{node} {}", word(param));
    }
    rest.to_string()
}

/// Every layer's pose keyed on one frame (frame tools that act on the whole
/// frame, like "Do not tween").
pub fn key_whole_frame(st: &mut crate::edit::State, frame: i32) {
    let fps = st_fps(&st.project);
    let tw = st.project.settings.tween_interp;
    let sym = st.project.root_symbol.clone();
    let ids: Vec<String> = st.project.root().map(|s| s.layers.iter().map(|l| l.id.clone()).collect()).unwrap_or_default();
    for id in ids {
        let Some(l) = st.project.layer_mut(&sym, &id) else { continue };
        for q in crate::edit::block_ops::pose_props(l) {
            key_at(q, &[frame as i64], fps, tw);
        }
    }
}

/// Keys a property on the given frames where it has no key (its value
/// there); a track's first key tweens as `tween` (the project's, D34).
fn key_at(q: &mut Property, frames: &[i64], fps: f64, tween: Interp) {
    for &i in frames {
        let t = i as f64 / fps;
        if q.track.index_of(t).is_err() {
            let v = q.key_value(t);
            let interp = crate::edit::editor::interp_at(q, t, tween);
            q.track.put(Key::new(t, v, interp));
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
/// Character widths and line height of a font, per unit of text size
/// (filled by the renderer, which has the fonts).
#[derive(Clone, Debug, Default)]
pub struct FontMetrics {
    pub line: f32,
    pub advance: HashMap<char, f32>,
}

fn text_size(tl: &TextLayer, fonts: &HashMap<String, FontMetrics>) -> (f32, f32) {
    let size = tl.size.max(0.0);
    let text = tl.text.as_deref().unwrap_or("");
    let lines: Vec<&str> = text.split('\n').collect();
    // the textfield's font, else the default (key ""), else an estimate
    let m = tl.font.as_ref().and_then(|f| fonts.get(f)).or_else(|| fonts.get(""));
    let def = fonts.get("");
    let adv = |c: char| m.and_then(|m| m.advance.get(&c)).or_else(|| def.and_then(|d| d.advance.get(&c))).copied().unwrap_or(0.55);
    let mut w = 0f32;
    for l in &lines {
        w = w.max(l.chars().map(adv).sum::<f32>() * size);
    }
    let line = m.map_or(1.2, |m| m.line);
    (w.max(size * 0.55), lines.len() as f32 * size * line)
}

fn text_hit(tl: &TextLayer, l: &Layer, r: &mut Resolver, t: f64, x: f32, y: f32, radius: f32, fonts: &HashMap<String, FontMetrics>) -> bool {
    let (sx, sy) = (r.f(&l.transform.scale_x, t).abs(), r.f(&l.transform.scale_y, t).abs());
    let (cx, cy) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
    let (w, h) = text_size(tl, fonts);
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

/// Creation mode: a new segment from joint `from` of the figure, ending at
/// stage point (x, y), with the draw tool's shape. Returns the new joint.
#[allow(clippy::too_many_arguments)]
fn draw_segment(s: &mut crate::edit::State, sym: &str, layer: &str, from: &str, shape: Shape, x: f32, y: f32, t: f64) -> EditResult<String> {
    let id = figure_ops::add_segment(s, sym, layer, from, 0.0, 1.0)?;
    let p = &s.project;
    let sy = p.symbol(sym).ok_or_else(|| crate::edit::EditError("no symbol".into()))?;
    let l = sy.layer(layer).ok_or_else(|| crate::edit::EditError("no figure".into()))?;
    let f = l.as_figure().ok_or_else(|| crate::edit::EditError("not a figure".into()))?;
    let mut r = Resolver::new(p);
    let pose = crate::eval::FigurePose::compute(f, t, &mut r);
    let (i, pi) = (f.index_of(&id).expect("new joint"), f.index_of(from).expect("start joint"));
    // the drop point in the figure's own space
    let inv = eval::layer_stage(p, sy, l, t).inverse();
    let (lx, ly) = (inv.map_x(x as f64, y as f64), inv.map_y(x as f64, y as f64));
    let (dx, dy) = (lx - pose.x[pi], ly - pose.y[pi]);
    let scale = (r.f(&f.nodes()[i].scale, t) as f64).abs().max(1e-6);
    let len = (dx.hypot(dy) / scale) as f32;
    let rel = (dy.atan2(dx).to_degrees() - pose.base_angle(f.nodes(), i)) as f32;
    let fresh = matches!(f.node(from).map(|n| n.parent.is_none()), Some(true));
    let l = s.project.layer_mut(sym, layer).ok_or_else(|| crate::edit::EditError("no figure".into()))?;
    let LayerKind::Figure(f) = &mut l.kind else { return crate::edit::fail("not a figure") };
    let n = f.node_mut(&id).ok_or_else(|| crate::edit::EditError("no joint".into()))?;
    n.rotation.value = Value::F(rel);
    n.length.value = Value::F(len);
    n.rest_length = len;
    n.shape = shape;
    if fresh {
        // drawn from the main node: the original's defaults
        n.thickness.value = Value::F(32.0);
        n.color.value = Value::C(Color::rgba8888(0x1a1a1aff));
    }
    Ok(id)
}

/// A group's effects that are on, by kind (K / D33).
fn group_on(p: &Project, gid: &str) -> Vec<String> {
    crate::edit::filters::group_state(p, &p.root_symbol, gid)["on"].as_object().map(|m| m.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect()).unwrap_or_default()
}

/// While a segment is drawn from `from` to the stage point (x, y): its length
/// and angle (degrees, anticlockwise from the figure's x axis) in the
/// figure's own space, and how thick it will be on the stage.
fn draw_measure(p: &Project, sym: &str, layer: &str, from: &str, x: f32, y: f32, t: f64) -> Option<(f64, f64, f64)> {
    let sy = p.symbol(sym)?;
    let l = sy.layer(layer)?;
    let f = l.as_figure()?;
    let mut r = Resolver::new(p);
    let pose = crate::eval::FigurePose::compute(f, t, &mut r);
    let fi = f.index_of(from)?;
    let st = eval::layer_stage(p, sy, l, t);
    let inv = st.inverse();
    let (dx, dy) = (inv.map_x(x as f64, y as f64) - pose.x[fi], inv.map_y(x as f64, y as f64) - pose.y[fi]);
    let n = &f.nodes()[fi];
    // a new segment takes its parent's thickness (the original's defaults off the main node)
    let thick = if n.parent.is_none() { 32.0 } else { r.f(&n.thickness, t) as f64 };
    let k = (st.map_x(1.0, 0.0) - st.map_x(0.0, 0.0)).hypot(st.map_y(1.0, 0.0) - st.map_y(0.0, 0.0));
    Some((dx.hypot(dy), dy.atan2(dx).to_degrees(), thick * k))
}

/// Mirror drawing (D13): the joint across the figure's centre line (the
/// vertical through its main node) from `from`, or `from` itself when it
/// sits on the line, with the drop point mirrored. Returns that joint, its
/// stage position and the mirrored drop point; None when there is no such
/// joint or the mirror image would be the same segment.
fn mirrored(p: &Project, sym: &str, layer: &str, from: &str, x: f32, y: f32, t: f64) -> Option<(String, f32, f32, f32, f32)> {
    let sy = p.symbol(sym)?;
    let l = sy.layer(layer)?;
    let f = l.as_figure()?;
    let mut r = Resolver::new(p);
    let pose = crate::eval::FigurePose::compute(f, t, &mut r);
    let st = eval::layer_stage(p, sy, l, t);
    let inv = st.inverse();
    let (lx, ly) = (inv.map_x(x as f64, y as f64), inv.map_y(x as f64, y as f64));
    let ax = pose.x[f.nodes().iter().position(|n| n.parent.is_none())?];
    let fi = f.index_of(from)?;
    let (wx, wy) = (2.0 * ax - pose.x[fi], pose.y[fi]);
    let j = (0..f.nodes().len())
        .map(|j| (j, (pose.x[j] - wx).hypot(pose.y[j] - wy)))
        .filter(|(_, d)| *d <= 1.5)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?
        .0;
    let (mx, my) = (2.0 * ax - lx, ly);
    if j == fi && (mx - lx).abs() < 2.0 {
        return None;
    }
    let s = |x: f64, y: f64| (st.map_x(x, y) as f32, st.map_y(x, y) as f32);
    let (jx, jy) = s(pose.x[j], pose.y[j]);
    let (dx, dy) = s(mx, my);
    Some((f.nodes()[j].id.clone(), jx, jy, dx, dy))
}

/// The root's "Frame sounds" audio layer, added (at the bottom) when missing.
fn frame_sounds_layer(s: &mut crate::edit::State, sym: &str) -> EditResult<String> {
    let name = frame_ops::FRAME_SOUNDS;
    if let Some(id) = s.project.root().and_then(|r| r.layers.iter().find(|l| matches!(l.kind, LayerKind::Audio(_)) && l.name.as_deref() == Some(name)).map(|l| l.id.clone())) {
        return Ok(id);
    }
    let id = s.project.new_id("L");
    structure::add_layer(s, sym, Layer::audio(&id, Some(name)), 0)?;
    Ok(id)
}

/// The sound starting on the frame at t.
fn frame_sound(p: &Project, t: f64) -> Option<&Clip> {
    let fps = st_fps(p);
    p.root()?.layers.iter().filter(|l| l.name.as_deref() == Some(frame_ops::FRAME_SOUNDS)).find_map(|l| match &l.kind {
        LayerKind::Audio(clips) => clips.iter().find(|c| (c.start - t).abs() < 0.25 / fps),
        _ => None,
    })
}

fn sound_name(p: &Project, id: &str) -> String {
    p.assets.get(id).map(|a| a.name.clone().unwrap_or_else(|| a.path.rsplit('/').next().unwrap_or(&a.path).to_string())).unwrap_or_else(|| id.to_string())
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
    let Some(c) = cam else { return (0xffffffff, DEFAULT_GRADIENT, false) };
    let mut r = Resolver::new(p);
    let (top, bottom) = (color_of(&mut r, &c.background, t), color_of(&mut r, &c.background2, t));
    if bottom & 0xff != 0 {
        (bottom, top, true)
    } else {
        let color = if top & 0xff == 0 { p.settings.background.to_rgba8888() | 0xff } else { top };
        let gradient = if bottom == 0 { DEFAULT_GRADIENT } else { bottom | 0xff };
        (color, gradient, false)
    }
}

fn color_of(r: &mut Resolver, p: &Property, t: f64) -> u32 {
    match r.resolve(p, t) {
        Value::C(c) => c.to_rgba8888(),
        _ => 0x000000ff,
    }
}

/// Layer up / down: the figure (with what is joined to it) moves past the next
/// layer that draws, not past a camera or a sound layer, which show nothing
/// (one press must change what is in front). The Java reference's
/// `layer_ops::move_with_joined` steps one layer whatever it is.
fn move_among_drawn(st: &mut crate::edit::State, sym: &str, id: &str, forward: bool) -> EditResult<()> {
    let Some(s) = st.project.symbol_mut(sym) else { return crate::edit::fail("no symbol") };
    let block: std::collections::HashSet<String> = layer_ops::with_joined(s, id).into_iter().collect();
    let (inb, rest): (Vec<_>, Vec<_>) = s.layers.iter().cloned().partition(|l| block.contains(&l.id));
    let before = s.layers.iter().take_while(|l| !block.contains(&l.id)).count().min(rest.len());
    let draws = |l: &std::sync::Arc<Layer>| !matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_));
    let to = if forward { rest[before..].iter().position(draws).map(|k| before + k + 1) } else { rest[..before].iter().rposition(draws) };
    let Some(to) = to else { return Ok(()) };
    let mut out: Vec<_> = rest[..to].to_vec();
    out.extend(inb);
    out.extend(rest[to..].iter().cloned());
    s.layers = out;
    Ok(())
}

/// The frame background at time t (the "frame_bg" command's work, also used by
/// a paste): colour, gradient, use gradient and expand gradient.
fn set_frame_bg(s: &mut crate::edit::State, sym: &str, t: f64, c2: &J) -> EditResult<()> {
    let cam = ensure_camera(s, sym)?;
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
    let expand = c2["expand"].as_bool();
    // no gradient: the fill is `background` and `background2` keeps the
    // gradient colour with zero alpha; with one, top is the gradient colour
    let (top, bottom) = if use_gradient { (gradient | 0xff, color | 0xff) } else { (color, gradient & !0xff) };
    let (n, fps) = (frame_ops::frame_count(s), st_fps(&s.project));
    let project_bg = s.project.settings.background.to_rgba8888() | 0xff;
    let l = s.project.layer_mut(sym, &cam).expect("camera");
    let LayerKind::Camera(cm) = &mut l.kind else { return crate::edit::fail("not a camera") };
    for (first, q, v) in [(true, &mut cm.background, top), (false, &mut cm.background2, bottom)] {
        // frames with no gradient of their own have the default gradient
        // colour (hidden: alpha 0); key them as that, or a frame added later
        // between them and this one blends its colour towards black
        if !first && q.track.is_empty() && matches!(q.value, Value::C(c) if c.a <= 0.0) {
            q.value = Value::C(Color::rgba8888(DEFAULT_GRADIENT & !0xff));
        }
        key_every_frame(q, n, fps);
        // frames with no background of their own show the project's:
        // say so, or the in-betweens fade through transparent black
        if first {
            for k in q.track.keys().to_vec() {
                if matches!(k.value, Value::C(c) if c.a <= 0.0) {
                    q.track.put(Key::new(k.time, Value::C(Color::rgba8888(project_bg)), k.interp));
                }
            }
        }
        let interp = q.track.keys().iter().find(|k| k.time == t).map(|k| k.interp).unwrap_or(Interp::Linear);
        q.track.put(Key::new(t, Value::C(Color::rgba8888(v)), interp));
    }
    if let Some(on) = expand {
        // a frame setting: exact HOLD keys, every frame its own
        let q = &mut cm.bg_expand;
        for i in 0..n {
            let ti = i as f64 / fps;
            if q.track.index_of(ti).is_err() {
                let old = crate::timing::setting(q, fps, i);
                q.track.put(Key::new(ti, Value::F(old), Interp::Hold));
            }
        }
        q.track.put(Key::new(t, Value::F(if on { 1.0 } else { 0.0 }), Interp::Hold));
    }
    Ok(())
}

/// What a frame's background is, as the command takes it (to copy and paste a frame's look).
fn frame_look(p: &Project, i: i32) -> J {
    let fps = st_fps(p);
    let (color, gradient, use_gradient) = frame_bg(p, i as f64 / fps);
    let expand = p.root().and_then(|s| s.layers.iter().find_map(|l| if let LayerKind::Camera(c) = &l.kind { Some(crate::timing::setting(&c.bg_expand, fps, i) > 0.5) } else { None })).unwrap_or(false);
    json!({"color": color, "gradient": gradient, "useGradient": use_gradient, "expand": expand})
}

/// "Use segment color" on or off for one segment. Off: it follows the
/// stickfigure colour and its own colour is kept aside (the original greys
/// the swatch out); on: the kept colour comes back.
fn use_own_color(node: &mut Node, on: bool, fig_color: Color) {
    if !on {
        let own = node.color.value.color();
        if node.own_color && node.color.track.is_empty() && own != fig_color {
            node.legacy_extras.insert(KEPT_OWN_COLOR.into(), format!("s:{:08x}", own.to_rgba8888()));
        }
        set_static(&mut node.color, Value::C(fig_color));
    } else if !node.own_color {
        let kept = node.legacy_extras.shift_remove(KEPT_OWN_COLOR).and_then(|t| u32::from_str_radix(t.trim_start_matches("s:"), 16).ok());
        if let Some(c) = kept {
            set_static(&mut node.color, Value::C(Color::rgba8888(c)));
        }
    }
    node.own_color = on;
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
