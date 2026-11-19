//! Camera tools (port of Java CameraOps): follow a layer, lock a layer to
//! the camera, lock text to a corner of the shot.

use super::editor::Editor;
use super::{fail, EditResult, State};
use crate::eval;
use crate::model::*;

fn origin(p: &Project, sym: &str, id: &str, t: f64) -> Option<(f64, f64)> {
    let s = p.symbol(sym)?;
    let a = eval::layer_stage(p, s, s.layer(id)?, t);
    Some((a.map_x(0.0, 0.0), a.map_y(0.0, 0.0)))
}

/// Keys the camera's x / y on every frame f0..f1 to follow the layer's
/// origin, keeping the offset it has at f0, then thins the keys.
pub fn follow(st: &mut State, sym: &str, cam: &str, target: &str, f0: i32, f1: i32) -> EditResult<()> {
    let fps = if st.project.settings.fps > 0.0 { st.project.settings.fps as f64 } else { 30.0 };
    let (a, b) = (f0.min(f1), f0.max(f1));
    let (ta, tb) = (a as f64 / fps, b as f64 / fps);
    let Some(start) = origin(&st.project, sym, target, ta) else { return fail("no target") };
    let origins: Vec<(f64, f64)> = (a..=b).map(|f| origin(&st.project, sym, target, f as f64 / fps).expect("target")).collect();
    let Some(l) = st.project.layer_mut(sym, cam) else { return fail("no camera") };
    let LayerKind::Camera(c) = &mut l.kind else { return fail("not a camera") };
    let ox = c.x.key_value(ta).f() - start.0 as f32;
    let oy = c.y.key_value(ta).f() - start.1 as f32;
    for q in [&mut c.x, &mut c.y] {
        let keep: Vec<Key> = q.track.keys().iter().filter(|k| k.time < ta - 1e-9 || k.time > tb + 1e-9).copied().collect();
        q.track.clear();
        for k in keep {
            q.track.put(k);
        }
    }
    for (i, f) in (a..=b).enumerate() {
        let t = f as f64 / fps;
        let o = origins[i];
        c.x.track.put(Key::new(t, Value::F(o.0 as f32 + ox), Interp::Linear));
        c.y.track.put(Key::new(t, Value::F(o.1 as f32 + oy), Interp::Linear));
    }
    for q in [&mut c.x, &mut c.y] {
        let eps = key_reducer::default_epsilon(q.ty);
        key_reducer::reduce(&mut q.track, eps);
    }
    Ok(())
}

/// Locks a layer to the camera (it stays put in the shot) or unlocks it; no jump.
pub fn lock(st: &mut State, sym: &str, layer: &str, cam: &str, on: bool, t: f64) -> EditResult<()> {
    super::join_ops::reparent(st, sym, layer, if on { Some(cam) } else { None }, t)
}

/// Locks text to a corner of the shot: locked to the camera, aligned to the
/// corner, placed `margin` in from the edges, unturned.
pub fn pin_to_corner(ed: &Editor, st: &mut State, sym: &str, text: &str, cam: &str, corner: i32, margin: f32, t: f64) -> EditResult<()> {
    if !(1..=4).contains(&corner) {
        return fail("corner 1..4");
    }
    let locked = st.project.symbol(sym).and_then(|s| s.layer(text)).map(|l| l.parent.as_deref() == Some(cam)).unwrap_or(false);
    if !locked {
        lock(st, sym, text, cam, true, t)?;
    }
    let (w, h) = (st.project.settings.width as f32 / 2.0 - margin, st.project.settings.height as f32 / 2.0 - margin);
    let ids = {
        let Some(l) = st.project.layer_mut(sym, text) else { return fail("no layer") };
        let LayerKind::Text(tl) = &mut l.kind else { return fail("not text") };
        tl.corner = corner;
        l.parallax = 0.0;
        [l.transform.x.id.clone(), l.transform.y.id.clone(), l.transform.rotation.id.clone()]
    };
    let left = corner == 1 || corner == 3;
    let top = corner == 1 || corner == 2;
    ed.change_in(st, &ids[0], Value::F(if left { -w } else { w }), t)?;
    ed.change_in(st, &ids[1], Value::F(if top { h } else { -h }), t)?;
    ed.change_in(st, &ids[2], Value::F(0.0), t)?;
    Ok(())
}

/// Making a polyfill: pick the anchor (the fill draws just before it), then
/// the nodes around the shape in order, then finish (one undo step).
#[derive(Clone, Debug)]
pub struct PolyfillBuilder {
    pub anchor: String,
    pub nodes: Vec<String>,
}

impl PolyfillBuilder {
    pub fn new(f: &Figure, anchor: &str) -> EditResult<PolyfillBuilder> {
        if f.node(anchor).is_none() {
            return fail(format!("no node {anchor}"));
        }
        Ok(PolyfillBuilder { anchor: anchor.into(), nodes: Vec::new() })
    }

    /// Adds a node; false when unknown, the same as the last pick, or the anchor first.
    pub fn add(&mut self, f: &Figure, id: &str) -> bool {
        if f.node(id).is_none() || self.nodes.last().is_some_and(|l| l == id) || (self.nodes.is_empty() && id == self.anchor) {
            return false;
        }
        self.nodes.push(id.into());
        true
    }

    pub fn undo_last(&mut self) -> bool {
        self.nodes.pop().is_some()
    }

    pub fn can_finish(&self) -> bool {
        self.nodes.len() >= 2
    }

    /// Adds the finished fill; colour None uses the anchor segment's colour.
    pub fn finish(&self, st: &mut State, sym: &str, layer: &str, id: &str, color: Option<Color>) -> EditResult<()> {
        if !self.can_finish() {
            return fail("pick at least two nodes after the anchor");
        }
        let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
        let lid = l.id.clone();
        let LayerKind::Figure(f) = &mut l.kind else { return fail("not a figure") };
        let mut pf = Polyfill::new(&lid, id);
        pf.anchor = self.anchor.clone();
        pf.nodes = self.nodes.clone();
        if let Some(c) = color {
            pf.use_own_color = true;
            pf.color.value = Value::C(c);
        }
        f.polyfills.push(pf);
        Ok(())
    }

    /// A polyfill id not used in the figure.
    pub fn free_id(f: &Figure) -> String {
        let mut i = f.polyfills.len();
        loop {
            let id = format!("pf{i}");
            if !f.polyfills.iter().any(|p| p.id == id) {
                return id;
            }
            i += 1;
        }
    }
}
