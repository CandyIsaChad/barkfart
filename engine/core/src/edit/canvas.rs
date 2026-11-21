//! Canvas tools (port of Java CanvasTools): joint handles, picking (with
//! node digging), dragging joints and figures (pose, stretch, axis lock,
//! angle snap, smart-stretch, pinned joints), turning a figure, panning
//! everything. A drag is one undo step: begin, move.., end (or cancel).

use super::editor::{interp_at, Editor};
use super::{Document, EditResult};
use crate::eval::{self, Affine, FigurePose, Resolver};
use crate::model::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Handle {
    pub layer: String,
    pub node: String,
    pub x: f32,
    pub y: f32,
    pub root: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyScope {
    /// The dragged joint's angle and length.
    Joint,
    /// Every joint's angle and length plus the figure's position.
    Figure,
}

pub const DRAG_HORIZONTAL: i32 = 1;
pub const DRAG_VERTICAL: i32 = 2;

#[derive(Clone, Debug)]
struct Drag {
    hit: Handle,
    view: Affine,
    grab_dx: f32,
    grab_dy: f32,
    start_x: f32,
    start_y: f32,
    start_length: HashMap<String, f32>,
    /// Pinned joints and their stage positions at drag start.
    pins: Vec<(String, (f64, f64))>,
}

#[derive(Clone, Debug)]
pub struct CanvasTools {
    pub editor: Editor,
    /// Dragging a joint changes length as well as angle.
    pub stretch: bool,
    pub key_scope: KeyScope,
    /// Show and pick static (detail) nodes too.
    pub show_static: bool,
    /// Edit through the camera (the editor edits in stage space).
    pub through_camera: bool,
    pub main_nodes_only: bool,
    pub hide_locked: bool,
    pub active_figure: Option<String>,
    /// Figure drags keep to the axis moved along most.
    pub axis_lock: bool,
    /// Joint angles snap to multiples of this (0: off).
    pub snap_degrees: f32,
    drag: Option<Drag>,
}

impl Default for CanvasTools {
    fn default() -> Self {
        CanvasTools {
            editor: Editor::default(),
            stretch: false,
            key_scope: KeyScope::Joint,
            show_static: false,
            through_camera: false,
            main_nodes_only: false,
            hide_locked: false,
            active_figure: None,
            axis_lock: false,
            snap_degrees: 0.0,
            drag: None,
        }
    }
}

/// Stick Nodes IK rigs (e.g. "Stickfigure (IK)"): a stretchy segment S with a
/// static smart-stretch connector X under it. Measured on the original
/// (movieclip frames it stored, 2026-11-21): S stops at the connector's reach; with
/// k = S's length / X's rest length, every static segment below X (down to the
/// next connector) gets rest x (1 + m (k - 1)), m being the multiplier of a
/// "reset stretch impulse" segment and 1 otherwise; X follows S's length.
/// Returns S's length and the (length property, value) changes, or None when
/// S is not such a segment.
pub fn connector_smart_stretch(f: &Figure, si: usize, len: f32) -> Option<(f32, Vec<(String, f32)>)> {
    let nodes = f.nodes();
    let s = &nodes[si];
    if !s.stretchy || s.is_static {
        return None;
    }
    let links = crate::eval::pose::connector_links(f);
    let x = nodes.iter().enumerate().find(|(j, n)| n.parent.as_deref() == Some(s.id.as_str()) && n.is_static && n.smart_stretch && links.contains_key(j))?;
    let (xi, xn) = x;
    // the leg stops at the connector's stored value (its full reach), else at S's rest length
    let reach = xn.legacy_extras.get("C.cValue").and_then(|v| v.split_once(':')).and_then(|(_, n)| n.parse::<f32>().ok()).filter(|v| *v > 0.0);
    let rest_s = if s.rest_length > 0.0 { s.rest_length } else { len };
    let clamped = len.min(reach.unwrap_or(rest_s));
    let rest_x = if xn.rest_length > 0.0 { xn.rest_length } else { rest_s };
    if rest_x <= 1e-6 {
        return None;
    }
    let k = clamped / rest_x;
    let mut out = vec![(xn.length.id.clone(), clamped)];
    // static descendants of X, stopping at connectors, non-static segments and do-not-smart-stretch ones
    let mut inside = vec![false; nodes.len()];
    inside[xi] = true;
    for j in xi + 1..nodes.len() {
        let n = &nodes[j];
        let Some(pj) = n.parent.as_ref().and_then(|p| f.index_of(p)) else { continue };
        if !inside[pj] || !n.is_static || n.no_smart_stretch || links.contains_key(&j) {
            continue;
        }
        inside[j] = true;
        let m = crate::eval::pose::stretch_reset(n).unwrap_or(1.0);
        let rest = if n.rest_length > 0.0 { n.rest_length } else { continue };
        out.push((n.length.id.clone(), rest * (1.0 + m * (k - 1.0))));
    }
    Some((clamped, out))
}

/// Snaps an angle to the nearest multiple of step (step <= 0: unchanged).
pub fn snap(deg: f64, step: f64) -> f64 {
    if step > 0.0 {
        java_round_f64(deg / step) * step
    } else {
        deg
    }
}

fn root_symbol(p: &Project) -> &Symbol {
    p.root().expect("root symbol")
}

/// Joints of a figure that other layers are joined to.
pub fn join_anchors(s: &Symbol, figure: &str) -> std::collections::HashSet<String> {
    s.layers.iter().filter(|l| l.parent.as_deref() == Some(figure)).filter_map(|l| l.parent_node.clone()).collect()
}

impl CanvasTools {
    pub fn new(editor: Editor) -> Self {
        CanvasTools { editor, ..CanvasTools::default() }
    }

    pub fn filtering(&self) -> bool {
        self.main_nodes_only || self.hide_locked
    }

    fn view(&self, p: &Project, l: &Layer, t: f64) -> Affine {
        let s = root_symbol(p);
        if self.through_camera {
            let mut r = Resolver::new(p);
            eval::camera_view(s, t, &mut r).mul(&eval::layer_stage(p, s, l, t))
        } else {
            eval::layer_stage(p, s, l, t)
        }
    }

    /// Handles for every visible, unlocked figure layer of the root symbol.
    pub fn handles(&self, p: &Project, t: f64) -> Vec<Handle> {
        let s = root_symbol(p);
        let mut r = Resolver::new(p);
        let mut out = Vec::new();
        for l in &s.layers {
            let LayerKind::Figure(f) = &l.kind else { continue };
            if !l.visible || l.locked || !r.b(&l.shown, t) {
                continue;
            }
            let v = self.view(p, l, t);
            let pose = FigurePose::compute(f, t, &mut r);
            let anchors = if self.main_nodes_only && self.active_figure.as_deref() != Some(&l.id) { Some(join_anchors(s, &l.id)) } else { None };
            for (i, n) in f.nodes().iter().enumerate() {
                if n.is_static && !self.show_static {
                    continue;
                }
                if let Some(a) = &anchors {
                    if n.parent.is_some() && !a.contains(&n.id) {
                        continue;
                    }
                }
                if self.hide_locked && n.pinned {
                    continue;
                }
                out.push(Handle { layer: l.id.clone(), node: n.id.clone(), x: v.map_x(pose.x[i], pose.y[i]) as f32, y: v.map_y(pose.x[i], pose.y[i]) as f32, root: n.parent.is_none() });
            }
        }
        out
    }

    /// Nearest handle within radius; ties go to the topmost layer, then joints over main nodes.
    pub fn pick(&self, p: &Project, x: f32, y: f32, radius: f32, t: f64) -> Option<Handle> {
        let mut best: Option<Handle> = None;
        let mut best_d = (radius * radius) as f64;
        for h in self.handles(p, t) {
            let d = ((h.x - x) * (h.x - x) + (h.y - y) * (h.y - y)) as f64;
            if d <= best_d || (best.as_ref().is_some_and(|b| b.root && !h.root) && d <= best_d + 1e-9) {
                best_d = d;
                best = Some(h);
            }
        }
        best
    }

    /// Node digging: handles within reach nearest first; tapping again on the
    /// same cluster gives the next; a selected joint within reach wins first.
    pub fn pick_cycling(&self, p: &Project, x: f32, y: f32, radius: f32, t: f64, selected: Option<(&str, &str)>, previous: Option<(&str, &str)>) -> Option<Handle> {
        let d2 = |h: &Handle| ((h.x - x) * (h.x - x) + (h.y - y) * (h.y - y)) as f64;
        let mut near: Vec<Handle> = self.handles(p, t).into_iter().filter(|h| d2(h) <= (radius * radius) as f64).collect();
        if near.is_empty() {
            return None;
        }
        near.sort_by(|a, b| {
            let (da, db) = (d2(a), d2(b));
            if (da - db).abs() > 1e-6 {
                return da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal);
            }
            a.root.cmp(&b.root)
        });
        let is = |h: &Handle, k: Option<(&str, &str)>| k.is_some_and(|(l, n)| h.layer == l && h.node == n);
        if let Some(at) = near.iter().rposition(|h| is(h, previous)) {
            return Some(near[(at + 1) % near.len()].clone());
        }
        let mut h = near[0].clone();
        for c in &near {
            if is(c, selected) {
                h = c.clone();
            }
        }
        Some(h)
    }

    /// Joints directly connected to a joint (and, across joins, figures joined at it).
    pub fn neighbours(&self, p: &Project, layer: &str, node: &str, across_joins: bool, t: f64) -> Vec<Handle> {
        let s = root_symbol(p);
        let Some(fl) = s.layer(layer) else { return Vec::new() };
        let Some(n) = fl.as_figure().and_then(|f| f.node(node)) else { return Vec::new() };
        let mut out = Vec::new();
        for h in self.handles(p, t) {
            if h.layer == layer {
                let hn = fl.as_figure().and_then(|f| f.node(&h.node));
                if n.parent.as_deref() == Some(h.node.as_str()) || hn.is_some_and(|x| x.parent.as_deref() == Some(node)) {
                    out.push(h);
                }
                continue;
            }
            if !across_joins {
                continue;
            }
            let hl = s.layer(&h.layer).expect("layer");
            if h.root && hl.parent.as_deref() == Some(layer) && hl.parent_node.as_deref() == Some(node) {
                out.push(h);
            } else if n.parent.is_none() && fl.parent.as_deref() == Some(h.layer.as_str()) && fl.parent_node.as_deref() == Some(h.node.as_str()) {
                out.push(h);
            }
        }
        out
    }

    /// Turns a figure about its main node by delta degrees at t (one undo step).
    pub fn rotate_figure(&self, d: &mut Document, layer: &str, delta: f64, t: f64) -> EditResult<()> {
        let p = d.project();
        let s = root_symbol(p);
        let Some(l) = s.layer(layer) else { return super::fail("no layer") };
        let Some(f) = l.as_figure() else { return super::fail("not a figure") };
        let mut r = Resolver::new(p);
        let stage = eval::layer_stage(p, s, l, t);
        let pose = FigurePose::compute(f, t, &mut r);
        let (px, py) = (stage.map_x(pose.x[0], pose.y[0]), stage.map_y(pose.x[0], pose.y[0]));
        let rot = r.f(&l.transform.rotation, t) as f64 + delta;
        let (x0, y0) = (r.f(&l.transform.x, t), r.f(&l.transform.y, t));
        let ids = [l.transform.rotation.id.clone(), l.transform.x.id.clone(), l.transform.y.id.clone()];
        let ed = self.editor.clone();
        d.edit("Rotate figure", |st| {
            ed.change_in(st, &ids[0], Value::F(rot as f32), t)?;
            let p = &st.project;
            let s = root_symbol(p);
            let after = eval::layer_stage(p, s, s.layer(layer).expect("layer"), t);
            let (qx, qy) = (after.map_x(pose.x[0], pose.y[0]), after.map_y(pose.x[0], pose.y[0]));
            ed.change_in(st, &ids[1], Value::F((x0 as f64 + (px - qx)) as f32), t)?;
            ed.change_in(st, &ids[2], Value::F((y0 as f64 + (py - qy)) as f32), t)?;
            Ok(())
        })
    }

    /// Panning mode: moves every top-level figure, sprite, text and movieclip by (dx, dy).
    pub fn move_all(&self, d: &mut Document, dx: f32, dy: f32, t: f64) -> EditResult<()> {
        let p = d.project();
        let mut r = Resolver::new(p);
        let moves: Vec<(String, f32, String, f32)> = root_symbol(p)
            .layers
            .iter()
            .filter(|l| l.parent.is_none() && !l.locked && !matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_)))
            .map(|l| (l.transform.x.id.clone(), r.f(&l.transform.x, t) + dx, l.transform.y.id.clone(), r.f(&l.transform.y, t) + dy))
            .collect();
        let ed = self.editor.clone();
        d.edit("Move everything", |st| {
            for (xi, x, yi, y) in &moves {
                ed.change_in(st, xi, Value::F(*x), t)?;
                ed.change_in(st, yi, Value::F(*y), t)?;
            }
            Ok(())
        })
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// Starts a drag on a handle (view coordinates of the finger); opens one undo step.
    pub fn begin(&mut self, d: &mut Document, hit: &Handle, x: f32, y: f32, t: f64) {
        let p = d.project();
        let s = root_symbol(p);
        let Some(l) = s.layer(&hit.layer) else { return };
        let Some(f) = l.as_figure() else { return };
        let view = self.view(p, l, t);
        let start_x = l.transform.x.key_value(t).f();
        let start_y = l.transform.y.key_value(t).f();
        let mut r0 = Resolver::new(p);
        let start_length: HashMap<String, f32> = f.nodes().iter().map(|n| (n.id.clone(), r0.f(&n.length, t))).collect();
        let p0 = FigurePose::compute(f, t, &mut r0);
        let stage = eval::layer_stage(p, s, l, t);
        let pins: Vec<(String, (f64, f64))> = f
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, n)| n.pinned && n.parent.is_some() && n.id != hit.node)
            .map(|(i, n)| (n.id.clone(), (stage.map_x(p0.x[i], p0.y[i]), stage.map_y(p0.x[i], p0.y[i]))))
            .collect();
        // which properties get keyed at the playhead (with their current values) before anything changes
        let mut keyed: Vec<String> = Vec::new();
        if self.key_scope == KeyScope::Figure {
            for n in f.nodes() {
                if n.parent.is_some() {
                    keyed.push(n.rotation.id.clone());
                    keyed.push(n.length.id.clone());
                }
            }
            keyed.push(l.transform.x.id.clone());
            keyed.push(l.transform.y.id.clone());
        } else if !hit.root {
            let n = f.node(&hit.node).expect("node");
            keyed.push(n.rotation.id.clone());
            keyed.push(n.length.id.clone());
        } else {
            keyed.push(l.transform.x.id.clone());
            keyed.push(l.transform.y.id.clone());
        }
        let default = self.editor.default_interp;
        d.begin(if hit.root { "Move figure" } else { "Pose node" });
        for id in keyed {
            if let Some(q) = d.state.project.property_mut(&id) {
                if q.track.index_of(t).is_ok() {
                    continue;
                }
                let interp = interp_at(q, t, default);
                let v = q.key_value(t);
                q.track.put(Key::new(t, v, interp));
            }
        }
        self.drag = Some(Drag { hit: hit.clone(), view, grab_dx: hit.x - x, grab_dy: hit.y - y, start_x, start_y, start_length, pins });
    }

    /// Moves the dragged joint (or figure) toward the finger (view coordinates).
    pub fn move_to(&mut self, d: &mut Document, x: f32, y: f32, t: f64) -> EditResult<()> {
        let Some(drag) = self.drag.clone() else { return Ok(()) };
        let (mut tx, mut ty) = (x + drag.grab_dx, y + drag.grab_dy);
        let p = d.project();
        let s = root_symbol(p);
        let Some(l) = s.layer(&drag.hit.layer) else { return Ok(()) };
        let Some(f) = l.as_figure() else { return Ok(()) };
        let Some(n) = f.node(&drag.hit.node) else { return Ok(()) };
        if n.drag_lock == DRAG_HORIZONTAL {
            ty = drag.hit.y;
        } else if n.drag_lock == DRAG_VERTICAL {
            tx = drag.hit.x;
        }
        let ed = self.editor.clone();
        if n.parent.is_none() {
            if self.axis_lock {
                if (tx - drag.hit.x).abs() >= (ty - drag.hit.y).abs() {
                    ty = drag.hit.y;
                } else {
                    tx = drag.hit.x;
                }
            }
            let inv = drag.view.inverse();
            let (ox, oy) = (inv.map_x(drag.hit.x as f64, drag.hit.y as f64), inv.map_y(drag.hit.x as f64, drag.hit.y as f64));
            let (nx, ny) = (inv.map_x(tx as f64, ty as f64), inv.map_y(tx as f64, ty as f64));
            let local = eval::layer_stage(p, s, l, t);
            let dxp = local.a * (nx - ox) + local.c * (ny - oy);
            let dyp = local.b * (nx - ox) + local.d * (ny - oy);
            let (xi, yi) = (l.transform.x.id.clone(), l.transform.y.id.clone());
            ed.change_in(&mut d.state, &xi, Value::F((drag.start_x as f64 + dxp) as f32), t)?;
            ed.change_in(&mut d.state, &yi, Value::F((drag.start_y as f64 + dyp) as f32), t)?;
            self.hold_pins(d, &drag, t)?;
            return Ok(());
        }
        let mut r = Resolver::new(p);
        let pose = FigurePose::compute(f, t, &mut r);
        let i = f.index_of(&n.id).expect("node");
        let pi = pose.parent[i] as usize;
        let inv = drag.view.inverse();
        let (lx, ly) = (inv.map_x(tx as f64, ty as f64), inv.map_y(tx as f64, ty as f64));
        let (dx, dy) = (lx - pose.x[pi], ly - pose.y[pi]);
        if dx * dx + dy * dy < 1e-9 {
            return Ok(());
        }
        let world = dy.atan2(dx).to_degrees();
        let current = r.f(&n.rotation, t) as f64;
        let mut rel = world - pose.base_angle(f.nodes(), i);
        rel = snap(rel, self.snap_degrees as f64);
        rel += 360.0 * java_round_f64((current - rel) / 360.0);
        let (rot_id, len_id) = (n.rotation.id.clone(), n.length.id.clone());
        let scale = r.f(&n.scale, t);
        let smart = n.smart_stretch;
        // an IK rig: the stretchy segment has a smart-stretch connector under it
        let ik = if self.stretch && scale > 1e-6 { connector_smart_stretch(f, i, ((dx * dx + dy * dy).sqrt() / scale as f64) as f32) } else { None };
        ed.change_in(&mut d.state, &rot_id, Value::F(rel as f32), t)?;
        if self.stretch && scale > 1e-6 {
            let len = ((dx * dx + dy * dy).sqrt() / scale as f64) as f32;
            if let Some((clamped, sets)) = ik {
                ed.change_in(&mut d.state, &len_id, Value::F(clamped), t)?;
                for (id, v) in sets {
                    ed.change_in(&mut d.state, &id, Value::F(v), t)?;
                }
                return self.hold_pins(d, &drag, t);
            }
            ed.change_in(&mut d.state, &len_id, Value::F(len), t)?;
            if smart {
                self.smart_stretch_in(d, &drag.hit.layer, &pose, i, len, t, &drag.start_length)?;
            }
        }
        self.hold_pins(d, &drag, t)
    }

    /// Smart-stretch (fitted, NOTES.md): with k = length / rest of the
    /// stretched node, each static descendant keeps its angle and gets
    /// rest x sqrt((k cos b)^2 + sin^2 b), b its angle to the stretched node.
    #[allow(clippy::too_many_arguments)]
    fn smart_stretch_in(&self, d: &mut Document, layer: &str, pose: &FigurePose, si: usize, len: f32, t: f64, start: &HashMap<String, f32>) -> EditResult<()> {
        let changes: Vec<(String, f32)> = {
            let p = d.project();
            let Some(f) = root_symbol(p).layer(layer).and_then(|l| l.as_figure()) else { return Ok(()) };
            let nodes = f.nodes();
            let s = &nodes[si];
            let rest = if s.rest_length > 0.0 { s.rest_length } else { start.get(&s.id).copied().unwrap_or(0.0) };
            if rest <= 1e-6 {
                return Ok(());
            }
            let k = (len / rest) as f64;
            let mut inside = vec![false; nodes.len()];
            inside[si] = true;
            let mut out = Vec::new();
            for j in si + 1..nodes.len() {
                let pj = pose.parent[j];
                let nj = &nodes[j];
                if pj < 0 || !inside[pj as usize] || !nj.is_static || nj.no_smart_stretch {
                    continue;
                }
                inside[j] = true;
                let rj = if nj.rest_length > 0.0 { nj.rest_length } else { start.get(&nj.id).copied().unwrap_or(0.0) };
                let b = (pose.angle[j] - pose.angle[si]).to_radians();
                let fct = (k * k * b.cos() * b.cos() + b.sin() * b.sin()).sqrt();
                out.push((nj.length.id.clone(), (rj as f64 * fct) as f32));
            }
            out
        };
        for (id, v) in changes {
            self.editor.change_in(&mut d.state, &id, Value::F(v), t)?;
        }
        Ok(())
    }

    /// Smart-stretch as its own step, for a pose given by the caller (nodes
    /// without a rest length use the lengths from the last drag's start).
    pub fn smart_stretch(&self, d: &mut Document, layer: &str, pose: &FigurePose, si: usize, len: f32, t: f64) -> EditResult<()> {
        let start = self.drag.as_ref().map(|g| g.start_length.clone()).unwrap_or_default();
        d.begin("Smart stretch");
        let r = self.smart_stretch_in(d, layer, pose, si, len, t, &start);
        d.commit();
        r
    }

    /// Turns each pinned joint and its parent (two-bone, same bend side) so
    /// the joint stays where it was on the stage.
    fn hold_pins(&self, d: &mut Document, drag: &Drag, t: f64) -> EditResult<()> {
        for (pin, target) in &drag.pins {
            let (sets, ok) = {
                let p = d.project();
                let s = root_symbol(p);
                let l = s.layer(&drag.hit.layer).expect("layer");
                let f = l.as_figure().expect("figure");
                let mut r = Resolver::new(p);
                let pose = FigurePose::compute(f, t, &mut r);
                let inv = eval::layer_stage(p, s, l, t).inverse();
                let (tx, ty) = (inv.map_x(target.0, target.1), inv.map_y(target.0, target.1));
                let Some(j) = f.index_of(pin) else { continue };
                let pj = pose.parent[j];
                if pj < 0 {
                    continue;
                }
                let pj = pj as usize;
                let pn = &f.nodes()[pj];
                let a = pose.parent[pj];
                let two_bone = a >= 0 && pn.id != drag.hit.node && !pn.pinned;
                let lj = (pose.x[j] - pose.x[pj]).hypot(pose.y[j] - pose.y[pj]);
                let mut sets: Vec<(usize, f64)> = Vec::new();
                if two_bone {
                    let a = a as usize;
                    let lp = (pose.x[pj] - pose.x[a]).hypot(pose.y[pj] - pose.y[a]);
                    let (ax, ay) = (pose.x[a], pose.y[a]);
                    let dd = (tx - ax).hypot(ty - ay);
                    if lp > 1e-9 && dd > 1e-9 {
                        let dc = ((lp - lj).abs() + 1e-9).max((lp + lj - 1e-9).min(dd));
                        let cos_a = (lp * lp + dc * dc - lj * lj) / (2.0 * lp * dc);
                        let alpha = cos_a.clamp(-1.0, 1.0).acos();
                        let phi = (ty - ay).atan2(tx - ax);
                        let side = (tx - ax) * (pose.y[pj] - ay) - (ty - ay) * (pose.x[pj] - ax);
                        let elbow = phi + if side >= 0.0 { alpha } else { -alpha };
                        sets.push((pj, elbow.to_degrees()));
                    }
                }
                (sets, (j, pj, tx, ty, lj))
            };
            // the elbow first, then the pinned joint from the new elbow position
            for (i, world) in sets {
                self.set_world_angle(d, &drag.hit.layer, i, world, t)?;
            }
            let (j, pj, tx, ty, lj) = ok;
            if lj > 1e-9 {
                let world = {
                    let p = d.project();
                    let f = root_symbol(p).layer(&drag.hit.layer).and_then(|l| l.as_figure()).expect("figure");
                    let mut r = Resolver::new(p);
                    let pose = FigurePose::compute(f, t, &mut r);
                    (ty - pose.y[pj]).atan2(tx - pose.x[pj]).to_degrees()
                };
                self.set_world_angle(d, &drag.hit.layer, j, world, t)?;
            }
        }
        Ok(())
    }

    fn set_world_angle(&self, d: &mut Document, layer: &str, i: usize, world: f64, t: f64) -> EditResult<()> {
        let (id, rel) = {
            let p = d.project();
            let f = root_symbol(p).layer(layer).and_then(|l| l.as_figure()).expect("figure");
            let mut r = Resolver::new(p);
            let pose = FigurePose::compute(f, t, &mut r);
            let n = &f.nodes()[i];
            let mut rel = world - pose.base_angle(f.nodes(), i);
            let current = r.f(&n.rotation, t) as f64;
            rel += 360.0 * java_round_f64((current - rel) / 360.0);
            (n.rotation.id.clone(), rel)
        };
        self.editor.change_in(&mut d.state, &id, Value::F(rel as f32), t)?;
        Ok(())
    }

    /// Ends the drag: one undo step.
    pub fn end(&mut self, d: &mut Document) {
        if self.drag.take().is_some() {
            d.commit();
        }
    }

    /// Abandons the drag and reverts it.
    pub fn cancel(&mut self, d: &mut Document) {
        if self.drag.take().is_some() {
            d.cancel();
        }
    }
}
