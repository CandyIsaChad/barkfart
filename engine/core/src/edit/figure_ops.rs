//! Figure building (port of Java FigureOps): segments, split, duplicate,
//! flip, branch changes, layering, pins. Each is one undo step for the caller.

use super::{fail, EditError, EditResult, State};
use crate::eval::scene::CURVE_SAGITTA;
use crate::eval::{FigurePose, Resolver};
use crate::model::*;

fn copy_property(from: &Property, to: &mut Property) {
    to.value = from.value;
    to.track = from.track.clone();
}

/// Copies looks and options (not identity, parent, rotation or length).
fn copy_looks(from: &Node, to: &mut Node) {
    to.shape = from.shape;
    to.curve = from.curve;
    to.is_static = from.is_static;
    to.stretchy = from.stretchy;
    to.gradient = from.gradient;
    to.gradient_reverse = from.gradient_reverse;
    to.gradient_sideways = from.gradient_sideways;
    to.gradient_linear = from.gradient_linear;
    to.circle_hollow = from.circle_hollow;
    to.circle_outline = from.circle_outline;
    to.half_arc = from.half_arc;
    to.polygon_sides = from.polygon_sides;
    to.trapezoid_ratio = from.trapezoid_ratio;
    to.trapezoid_round_start = from.trapezoid_round_start;
    to.trapezoid_round_end = from.trapezoid_round_end;
    to.triangle_upside_down = from.triangle_upside_down;
    to.right_triangle = from.right_triangle;
    copy_property(&from.thickness, &mut to.thickness);
    copy_property(&from.color, &mut to.color);
    copy_property(&from.gradient_color, &mut to.gradient_color);
    copy_property(&from.outline_color, &mut to.outline_color);
    copy_property(&from.scale, &mut to.scale);
    copy_property(&from.shown, &mut to.shown);
}

fn map_floats(p: &mut Property, f: impl Fn(f32) -> f32) {
    p.value = Value::F(f(p.value.f()));
    let ks: Vec<Key> = p.track.keys().to_vec();
    p.track.clear();
    for k in ks {
        p.track.put(Key { value: Value::F(f(k.value.f())), ..k });
    }
}

fn figure<'a>(st: &'a mut State, sym: &str, layer: &str) -> EditResult<(&'a mut Figure, String)> {
    let Some(l) = st.project.layer_mut(sym, layer) else { return fail(format!("no layer {layer}")) };
    let id = l.id.clone();
    match &mut l.kind {
        LayerKind::Figure(f) => Ok((f, id)),
        _ => fail("not a figure"),
    }
}

/// A new segment from a node, inheriting its static / stretchy setting,
/// thickness and colour. Returns the new node's id.
pub fn add_segment(st: &mut State, sym: &str, layer: &str, parent: &str, rotation: f32, length: f32) -> EditResult<String> {
    let (f, lid) = figure(st, sym, layer)?;
    let Some(p) = f.node(parent).cloned() else { return fail(format!("no node {parent}")) };
    let shape = if p.parent.is_none() { Shape::Line } else { p.shape };
    let mut n = Node::new(&lid, &f.free_id("n"), Some(parent), shape);
    if p.parent.is_some() {
        n.is_static = p.is_static;
        n.stretchy = p.stretchy;
        n.thickness.value = p.thickness.value;
        n.color.value = p.color.value;
    }
    n.rotation.value = Value::F(rotation);
    n.length.value = Value::F(length);
    n.rest_length = length;
    let id = n.id.clone();
    f.add(n).map_err(EditError)?;
    Ok(id)
}

/// Splits a segment into equal pieces; a curved one along its arc, so the
/// drawn shape stays. Length keys are divided; rotation keys stay on the first piece.
pub fn split(st: &mut State, sym: &str, layer: &str, node: &str, pieces: usize) -> EditResult<()> {
    if !(2..=64).contains(&pieces) {
        return fail("pieces 2..64");
    }
    let (f, lid) = figure(st, sym, layer)?;
    let Some(n) = f.node(node).cloned() else { return fail(format!("cannot split {node}")) };
    let Some(parent) = n.parent.clone() else { return fail(format!("cannot split {node}")) };
    let len = n.length.value.f();
    let h = (n.curve * CURVE_SAGITTA) as f64;
    let chord = len as f64;
    let theta = if chord < 1e-9 { 0.0 } else { 4.0 * (2.0 * h / chord).atan() };
    let sub = theta / pieces as f64;
    let piece_chord = if theta.abs() < 1e-9 { chord / pieces as f64 } else { chord * (sub / 2.0).sin() / (theta / 2.0).sin() };
    let piece_curve = if theta.abs() < 1e-9 { 0.0 } else { (piece_chord / 2.0 * (sub / 4.0).tan() / CURVE_SAGITTA as f64) as f32 };
    let ratio = (piece_chord / chord.max(1e-9)) as f32;
    let first_turn = (theta / 2.0 - sub / 2.0).to_degrees() as f32;
    let step_turn = (-sub).to_degrees() as f32;
    let at = f.index_of(node).expect("found");
    let mut prev = parent;
    for i in 0..pieces - 1 {
        let mut piece = Node::new(&lid, &f.free_id("n"), Some(&prev), n.shape);
        copy_looks(&n, &mut piece);
        piece.curve = piece_curve;
        piece.draw_order = n.draw_order;
        if i == 0 {
            copy_property(&n.rotation, &mut piece.rotation);
            map_floats(&mut piece.rotation, |v| v + first_turn);
        } else {
            piece.rotation.value = Value::F(step_turn);
        }
        copy_property(&n.length, &mut piece.length);
        map_floats(&mut piece.length, |v| v * ratio);
        piece.rest_length = n.rest_length * ratio;
        prev = piece.id.clone();
        f.insert(at + i, piece).map_err(EditError)?;
    }
    f.reparent(node, &prev).map_err(EditError)?;
    let m = f.node_mut(node).expect("still there");
    m.rotation.value = Value::F(step_turn);
    m.rotation.track.clear();
    map_floats(&mut m.length, |v| v * ratio);
    m.rest_length *= ratio;
    m.curve = piece_curve;
    Ok(())
}

/// Copies a node and its branch next to the original, drawn above
/// everything; polyfills wholly inside the branch come along. Returns the copy's id.
pub fn duplicate_branch(st: &mut State, sym: &str, layer: &str, node: &str) -> EditResult<String> {
    let parent = {
        let (f, _) = figure(st, sym, layer)?;
        match f.node(node) {
            Some(n) if n.parent.is_some() => n.parent.clone().expect("has a parent"),
            _ => return fail(format!("cannot duplicate {node}")),
        }
    };
    let sub: Vec<Node> = {
        let (f, _) = figure(st, sym, layer)?;
        f.subtree(node).iter().map(|id| f.node(id).expect("in figure").clone()).collect()
    };
    let polys = figure(st, sym, layer)?.0.polyfills.clone();
    paste_nodes(st, sym, layer, &parent, &sub, &polys)
}

/// Adds copies of `sub` (a branch in figure order, its first node the branch
/// root) under joint `parent`, drawn above everything; polyfills of `polys`
/// wholly inside the branch come along. Returns the new branch root's id.
pub fn paste_nodes(st: &mut State, sym: &str, layer: &str, parent: &str, sub: &[Node], polys: &[Polyfill]) -> EditResult<String> {
    let (f, lid) = figure(st, sym, layer)?;
    if f.node(parent).is_none() || sub.is_empty() {
        return fail(format!("cannot paste onto {parent}"));
    }
    let first = sub[0].id.clone();
    let max_order = f.nodes().iter().map(|x| x.draw_order).max().unwrap_or(i32::MIN);
    let base = max_order + 1;
    let min_sub = sub.iter().map(|x| x.draw_order).min().unwrap_or(i32::MAX);
    let mut ids: std::collections::HashMap<String, String> = Default::default();
    let mut made = String::new();
    for x in sub {
        let par = if x.id == first { Some(parent.to_string()) } else { x.parent.as_ref().and_then(|p| ids.get(p).cloned()) };
        let Some(par) = par else { continue };
        let mut c = Node::new(&lid, &f.free_id("n"), Some(&par), x.shape);
        copy_looks(x, &mut c);
        copy_property(&x.rotation, &mut c.rotation);
        copy_property(&x.length, &mut c.length);
        c.rest_length = x.rest_length;
        c.smart_stretch = x.smart_stretch;
        c.no_smart_stretch = x.no_smart_stretch;
        c.angle_lock = x.angle_lock;
        c.drag_lock = x.drag_lock;
        c.draw_order = base + (x.draw_order - min_sub);
        ids.insert(x.id.clone(), c.id.clone());
        if x.id == first {
            made = c.id.clone();
        }
        f.add(c).map_err(EditError)?;
    }
    let count = f.polyfills.len();
    let mut extra = Vec::new();
    for pf in polys {
        if !ids.contains_key(&pf.anchor) || !pf.nodes.iter().all(|x| ids.contains_key(x)) {
            continue;
        }
        let mut c = Polyfill::new(&lid, &format!("{}c{count}", pf.id));
        c.anchor = ids[&pf.anchor].clone();
        c.nodes = pf.nodes.iter().map(|x| ids[x].clone()).collect();
        c.use_own_color = pf.use_own_color;
        copy_property(&pf.color, &mut c.color);
        extra.push(c);
    }
    f.polyfills.extend(extra);
    Ok(made)
}

/// Draw-order moves of a segment (or its whole branch): one step back or
/// forward, or to the very back or front. `to`: -2 back end, -1 back one,
/// 1 forward one, 2 front end.
pub fn reorder(st: &mut State, sym: &str, layer: &str, node: &str, branch: bool, to: i32) -> EditResult<()> {
    let (f, _) = figure(st, sym, layer)?;
    if f.node(node).is_none() {
        return fail(format!("no node {node}"));
    }
    let moving: std::collections::HashSet<String> = if branch { f.subtree(node).into_iter().collect() } else { [node.to_string()].into_iter().collect() };
    let seq: Vec<String> = f.draw_sequence().into_iter().map(|i| f.nodes()[i].id.clone()).collect();
    let (group, rest): (Vec<String>, Vec<String>) = seq.iter().cloned().partition(|x| moving.contains(x));
    let before = seq.iter().take_while(|x| !moving.contains(*x)).count() as i64;
    let at = match to {
        -2 => 0,
        2 => rest.len() as i64,
        d => before + d.signum() as i64,
    }
    .clamp(0, rest.len() as i64) as usize;
    let mut out: Vec<String> = rest[..at].to_vec();
    out.extend(group);
    out.extend(rest[at..].iter().cloned());
    for (i, id) in out.iter().enumerate() {
        f.node_mut(id).expect("in figure").draw_order = i as i32;
    }
    Ok(())
}

/// Mirrors a branch about its parent's joint (horizontal: left-right).
/// The parent's angle is read again before each key is written, in the
/// same order as the Java version (it can depend on this joint through links).
pub fn flip_branch(st: &mut State, sym: &str, layer: &str, node: &str, horizontal: bool, t: f64) -> EditResult<()> {
    fn parent_angle(p: &Project, sym: &str, layer: &str, pi: usize, at: f64) -> f64 {
        let f = p.symbol(sym).and_then(|s| s.layer(layer)).and_then(|l| l.as_figure()).expect("figure");
        let mut r = Resolver::new(p);
        FigurePose::compute(f, at, &mut r).angle[pi]
    }
    let (pi, keys) = {
        let Some(f) = st.project.symbol(sym).and_then(|s| s.layer(layer)).and_then(|l| l.as_figure()) else { return fail("no figure") };
        let Some(top) = f.node(node) else { return fail(format!("cannot flip {node}")) };
        let Some(parent) = &top.parent else { return fail(format!("cannot flip {node}")) };
        (f.index_of(parent).expect("parent"), top.rotation.track.keys().to_vec())
    };
    let base = if horizontal { 180.0 } else { 0.0 };
    let pw_now = parent_angle(&st.project, sym, layer, pi, t);
    {
        let (f, _) = figure(st, sym, layer)?;
        let top = f.node_mut(node).expect("found");
        top.rotation.value = Value::F((base - 2.0 * pw_now - top.rotation.value.f() as f64) as f32);
        top.rotation.track.clear();
    }
    for k in keys {
        let pw = parent_angle(&st.project, sym, layer, pi, k.time);
        let (f, _) = figure(st, sym, layer)?;
        let top = f.node_mut(node).expect("found");
        top.rotation.track.put(Key { value: Value::F((base - 2.0 * pw - k.value.f() as f64) as f32), ..k });
    }
    let (f, _) = figure(st, sym, layer)?;
    for id in f.subtree(node) {
        let x = f.node_mut(&id).expect("in figure");
        if id != node {
            map_floats(&mut x.rotation, |v| -v);
        }
        x.curve = -x.curve;
        x.right_triangle = -x.right_triangle;
        x.half_arc = -x.half_arc;
    }
    Ok(())
}

/// Mirrors a whole figure: its layer's x or y scale changes sign, keys included.
pub fn flip_figure(st: &mut State, sym: &str, layer: &str, horizontal: bool) -> EditResult<()> {
    let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
    let s = if horizontal { &mut l.transform.scale_x } else { &mut l.transform.scale_y };
    map_floats(s, |v| -v);
    Ok(())
}

/// A change applied to every node of a branch except the root ("Modify Branch").
#[derive(Clone, Debug)]
pub enum BranchChange {
    Static(bool),
    Thickness(f32),
    Color(Color),
    Shape(Shape),
    ScaleLength(f32),
}

pub fn modify_branch(st: &mut State, sym: &str, layer: &str, node: &str, change: &BranchChange) -> EditResult<()> {
    let (f, _) = figure(st, sym, layer)?;
    if f.node(node).is_none() {
        return fail(format!("no node {node}"));
    }
    for id in f.subtree(node) {
        let x = f.node_mut(&id).expect("in figure");
        if x.parent.is_none() {
            continue;
        }
        match change {
            BranchChange::Static(on) => x.is_static = *on,
            BranchChange::Thickness(w) => {
                x.thickness.track.clear();
                x.thickness.value = Value::F(*w);
            }
            BranchChange::Color(c) => {
                x.color.track.clear();
                x.color.value = Value::C(*c);
            }
            BranchChange::Shape(s) => x.shape = *s,
            BranchChange::ScaleLength(k) => {
                let k = *k;
                map_floats(&mut x.length, |v| v * k);
                x.rest_length *= k;
            }
        }
    }
    Ok(())
}

/// Moves a whole branch one step back or forward in the figure's layering,
/// keeping its own order.
pub fn push_branch(st: &mut State, sym: &str, layer: &str, node: &str, forward: bool) -> EditResult<()> {
    let (f, _) = figure(st, sym, layer)?;
    if f.node(node).is_none() {
        return fail(format!("no node {node}"));
    }
    let branch: std::collections::HashSet<String> = f.subtree(node).into_iter().collect();
    let seq: Vec<String> = f.draw_sequence().into_iter().map(|i| f.nodes()[i].id.clone()).collect();
    let (in_branch, rest): (Vec<String>, Vec<String>) = seq.iter().cloned().partition(|x| branch.contains(x));
    let before = seq.iter().take_while(|x| !branch.contains(*x)).count() as i64;
    let to = (before + if forward { 1 } else { -1 }).clamp(0, rest.len() as i64) as usize;
    let mut out: Vec<String> = rest[..to].to_vec();
    out.extend(in_branch);
    out.extend(rest[to..].iter().cloned());
    for (i, id) in out.iter().enumerate() {
        f.node_mut(id).expect("in figure").draw_order = i as i32;
    }
    Ok(())
}

pub fn set_pinned(st: &mut State, sym: &str, layer: &str, node: &str, on: bool) -> EditResult<()> {
    let (f, _) = figure(st, sym, layer)?;
    if let Some(n) = f.node_mut(node) {
        n.pinned = on;
    }
    Ok(())
}
