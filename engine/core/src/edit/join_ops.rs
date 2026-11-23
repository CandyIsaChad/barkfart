//! Joining (port of Java JoinOps): attach a layer to a figure's joint (or the
//! figure, or the camera) and detach it, keeping it where it is: its
//! transform, static value and every key, is rewritten in the new frame.

use super::{fail, EditResult, State};
use crate::eval::{self, Affine};
use crate::model::*;

/// Attaches `child` to `target`'s node (None: to the figure itself).
pub fn join(st: &mut State, sym: &str, child: &str, target: &str, node: Option<&str>, rotate: bool, t: f64) -> EditResult<()> {
    {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        if target == child {
            return fail("a figure cannot join itself");
        }
        let mut cur = s.layer(target);
        let mut guard = s.layers.len() + 1;
        while let Some(c) = cur {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if c.id == child {
                return fail("that would join a figure to its own joined figure");
            }
            cur = c.parent.as_deref().and_then(|p| s.layer(p));
        }
        let Some(f) = s.layer(target).and_then(|l| l.as_figure()) else { return fail("can only join to a figure") };
        if let Some(n) = node {
            if f.node(n).is_none() {
                return fail(format!("no node {n}"));
            }
        }
    }
    change(st, sym, child, Some(target), node, rotate, t)
}

/// Detaches a layer from whatever it is joined to.
pub fn unjoin(st: &mut State, sym: &str, child: &str, t: f64) -> EditResult<()> {
    change(st, sym, child, None, None, true, t)
}

/// Gives a layer any parent (or none), keeping it in place (camera locks).
pub fn reparent(st: &mut State, sym: &str, child: &str, parent: Option<&str>, t: f64) -> EditResult<()> {
    change(st, sym, child, parent, None, true, t)
}

fn props(l: &Layer) -> [&Property; 5] {
    let tr = &l.transform;
    [&tr.x, &tr.y, &tr.rotation, &tr.scale_x, &tr.scale_y]
}

fn props_mut(l: &mut Layer) -> [&mut Property; 5] {
    let tr = &mut l.transform;
    [&mut tr.x, &mut tr.y, &mut tr.rotation, &mut tr.scale_x, &mut tr.scale_y]
}

/// One of x, y, rotation, scaleX, scaleY from a local matrix (no skew), with
/// the anchor accounted for; rotation stays continuous with `old`.
fn part(i: usize, ax: f64, ay: f64, old: f64, m: &Affine) -> f32 {
    let sx = m.a.hypot(m.b);
    let mut rot = m.b.atan2(m.a).to_degrees();
    let sy = if sx < 1e-12 { 0.0 } else { (m.a * m.d - m.b * m.c) / sx };
    let x = m.tx + m.a * ax + m.c * ay;
    let y = m.ty + m.b * ax + m.d * ay;
    rot += 360.0 * java_round_f64((old - rot) / 360.0);
    (match i {
        0 => x,
        1 => y,
        2 => rot,
        3 => sx,
        _ => sy,
    }) as f32
}

fn change(st: &mut State, sym: &str, child: &str, parent: Option<&str>, node: Option<&str>, rotate: bool, t: f64) -> EditResult<()> {
    // where the layer is at every time that matters, before the change
    let (times, world, template, keyed) = {
        let p = &st.project;
        let Some(s) = p.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(child) else { return fail(format!("no layer {child}")) };
        let mut times: Vec<f64> = vec![t];
        for q in props(l) {
            for k in q.track.keys() {
                times.push(k.time);
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        times.dedup();
        let world: Vec<Affine> = times.iter().map(|&tt| eval::parent_frame(p, s, l, tt).mul(&eval::local_transform(p, l, tt))).collect();
        let keyed = times.len() > 1 || props(l).iter().any(|q| !q.track.is_empty());
        // the first key at each time (any property) gives the interpolation
        let mut template: Vec<Key> = Vec::new();
        for q in props(l) {
            for k in q.track.keys() {
                if !template.iter().any(|x| x.time == k.time) {
                    template.push(*k);
                }
            }
        }
        (times, world, template, keyed)
    };
    {
        let Some(l) = st.project.layer_mut(sym, child) else { return fail("no layer") };
        l.parent = parent.map(str::to_string);
        l.parent_node = node.map(str::to_string);
        l.parent_node_rotate = rotate;
        // a join for the whole timeline replaces any joins over time
        l.joins.clear();
    }
    // the same placement in the new frame
    let (local, ax, ay, old_rot) = {
        let p = &st.project;
        let s = p.symbol(sym).expect("symbol");
        let l = s.layer(child).expect("layer");
        let local: Vec<Affine> = times.iter().zip(&world).map(|(&tt, w)| eval::parent_frame(p, s, l, tt).inverse().mul(w)).collect();
        (local, l.transform.anchor_x.value.f() as f64, l.transform.anchor_y.value.f() as f64, l.transform.rotation.value.f() as f64)
    };
    let at = |time: f64| times.iter().position(|x| *x == time).expect("time listed");
    let l = st.project.layer_mut(sym, child).expect("layer");
    for (i, q) in props_mut(l).into_iter().enumerate() {
        q.track.clear();
        if keyed {
            for k in &template {
                q.track.put(Key { value: Value::F(part(i, ax, ay, old_rot, &local[at(k.time)])), ..*k });
            }
        }
        q.value = Value::F(part(i, ax, ay, old_rot, &local[at(t)]));
    }
    Ok(())
}

/// Join-anchor flip: mirrors every layer joined at the anchor (a joint, or
/// None for the figure itself) across its axis: y, rotation and y scale change sign.
pub fn flip_joined(st: &mut State, sym: &str, anchor_layer: &str, node: Option<&str>) -> EditResult<()> {
    let ids: Vec<String> = {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        s.layers.iter().filter(|l| l.parent.as_deref() == Some(anchor_layer) && l.parent_node.as_deref() == node).map(|l| l.id.clone()).collect()
    };
    if ids.is_empty() {
        return fail("nothing is joined there");
    }
    for id in ids {
        let l = st.project.layer_mut(sym, &id).expect("layer");
        for q in [&mut l.transform.y, &mut l.transform.rotation, &mut l.transform.scale_y] {
            q.value = Value::F(-q.value.f());
            let ks: Vec<Key> = q.track.keys().to_vec();
            q.track.clear();
            for k in ks {
                q.track.put(Key { value: Value::F(-k.value.f()), ..k });
            }
        }
    }
    Ok(())
}

/// Break off (REDESIGN D3, F1): the branch from `node` down becomes its own
/// figure, joined to this one at the joint it hung from, so nothing moves on
/// any frame. The new figure is drawn just above this one. Figures joined to
/// a joint of the branch follow it to the new figure. Returns the new
/// figure's layer id.
pub fn break_off(st: &mut State, sym: &str, layer: &str, node: &str) -> EditResult<String> {
    let (src, sub, parent, absolute, index) = {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(layer) else { return fail(format!("no layer {layer}")) };
        let Some(f) = l.as_figure() else { return fail("only a figure's branch can break off") };
        let Some(n) = f.node(node) else { return fail(format!("no node {node}")) };
        let Some(parent) = n.parent.clone() else { return fail("The main joint is the whole figure; pick a joint further out.") };
        let sub = f.subtree(node);
        let inside = |id: &str| sub.iter().any(|x| x == id);
        // a connector across the break would lose its other end
        for (ci, (ei, _)) in crate::eval::pose::connector_links(f) {
            if inside(&f.nodes()[ci].id) != inside(&f.nodes()[ei].id) {
                return fail("A connector ties this branch to the rest of the figure, so it can't break off.");
            }
        }
        // so would a polyfill spanning it
        for pf in &f.polyfills {
            let ids: Vec<&str> = std::iter::once(pf.anchor.as_str()).chain(pf.nodes.iter().map(String::as_str)).collect();
            let k = ids.iter().filter(|x| inside(x)).count();
            if k > 0 && k < ids.len() {
                return fail("A polyfill spans this joint, so the branch can't break off.");
            }
        }
        (l.clone(), sub, parent, n.angle_lock == LOCK_ABSOLUTE, s.layer_index(layer).unwrap_or(0))
    };
    let new_id = st.project.new_id("L");
    let mut ids = std::collections::HashMap::new();
    ids.insert(layer.to_string(), new_id.clone());
    let mut l2 = super::structure::copy_layer(&src, &ids)?;
    let name = src.name.clone().unwrap_or_else(|| src.id.clone());
    l2.name = Some(format!("{name} part"));
    // the new figure: a main joint where the branch hung, then the branch
    {
        let LayerKind::Figure(f2) = &mut l2.kind else { return fail("not a figure") };
        let root_id = f2.root().map(|r| r.id.clone()).unwrap_or_else(|| "root".into());
        let mut list = vec![Node::new(&new_id, &root_id, None, Shape::Line)];
        for id in &sub {
            let mut n = f2.node(id).expect("copied").clone();
            if id == node {
                n.parent = Some(root_id.clone());
            }
            list.push(n);
        }
        f2.set_nodes(list).map_err(super::EditError)?;
        let keep: Vec<Polyfill> = f2.polyfills.iter().filter(|pf| sub.iter().any(|x| *x == pf.anchor)).cloned().collect();
        f2.polyfills = keep;
        if let Some(b) = &f2.base {
            let b: indexmap::IndexMap<String, NodeBase> = b.iter().filter(|(k, _)| sub.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
            f2.base = Some(std::sync::Arc::new(b));
        }
    }
    // placed by the joint alone (the joint's frame already carries this
    // figure's position, turn and scale)
    {
        let tr = &mut l2.transform;
        for (q, v) in [(&mut tr.x, 0.0), (&mut tr.y, 0.0), (&mut tr.rotation, 0.0), (&mut tr.scale_x, 1.0), (&mut tr.scale_y, 1.0), (&mut tr.anchor_x, 0.0), (&mut tr.anchor_y, 0.0)] {
            q.track.clear();
            q.value = Value::F(v);
        }
        l2.parent = Some(layer.to_string());
        l2.parent_node = Some(parent);
        // a branch locked to the stage's angle keeps it
        l2.parent_node_rotate = !absolute;
    }
    // the branch leaves this figure
    {
        let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
        let LayerKind::Figure(f) = &mut l.kind else { return fail("not a figure") };
        f.remove_subtree(node).map_err(super::EditError)?;
        f.polyfills.retain(|pf| !sub.iter().any(|x| *x == pf.anchor));
    }
    // figures joined to a joint of the branch go with it
    let followers: Vec<String> = st
        .project
        .symbol(sym)
        .map(|s| s.layers.iter().filter(|x| x.parent.as_deref() == Some(layer) && x.parent_node.as_ref().is_some_and(|n| sub.contains(n))).map(|x| x.id.clone()).collect())
        .unwrap_or_default();
    for id in followers {
        if let Some(x) = st.project.layer_mut(sym, &id) {
            x.parent = Some(new_id.clone());
        }
    }
    super::structure::add_layer(st, sym, l2, index + 1)?;
    Ok(new_id)
}


/// Joins over time (D8, I1): from `t` on, `child` is joined to `parent`'s
/// `node` (or the figure, or to nothing), until its next switch. Nothing
/// moves: the transform at the switch is kept as the end of the interval
/// before it, and the keys from `t` to the next switch are rewritten in the
/// new frame, with one on `t`.
pub fn switch_at(st: &mut State, sym: &str, child: &str, parent: Option<&str>, node: Option<&str>, rotate: bool, t: f64) -> EditResult<()> {
    if let Some(target) = parent {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        if target == child {
            return fail("a figure cannot join itself");
        }
        // no loops through the target's own joins at this moment
        let mut cur = s.layer(target);
        let mut guard = s.layers.len() + 1;
        while let Some(c) = cur {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if c.id == child {
                return fail("that would join a figure to its own joined figure");
            }
            cur = c.parent_at(t).0.and_then(|p| s.layer(p));
        }
        let Some(f) = s.layer(target).and_then(|l| l.as_figure()) else { return fail("can only join to a figure") };
        if let Some(n) = node {
            if f.node(n).is_none() {
                return fail(format!("no node {n}"));
            }
        }
    }
    let eps = 1e-9;
    // the interval the switch opens, and where the layer is over it (world)
    let (times, world, before, template, ax, ay, old_rot) = {
        let p = &st.project;
        let Some(s) = p.symbol(sym) else { return fail("no symbol") };
        let Some(l) = s.layer(child) else { return fail(format!("no layer {child}")) };
        let end = l.joins.iter().find(|j| j.time > t + eps).map(|j| j.time);
        let mut times: Vec<f64> = vec![t];
        for q in props(l) {
            for k in q.track.keys() {
                if k.time > t + eps && end.is_none_or(|e| k.time < e - eps) {
                    times.push(k.time);
                }
            }
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        times.dedup();
        let world: Vec<Affine> = times.iter().map(|&tt| eval::parent_frame(p, s, l, tt).mul(&eval::local_transform(p, l, tt))).collect();
        // the values at t in the frame before the switch
        let here = eval::local_transform(p, l, t);
        let (ax, ay, old_rot) = (l.transform.anchor_x.value.f() as f64, l.transform.anchor_y.value.f() as f64, l.transform.rotation.value.f() as f64);
        let before: [f32; 5] = std::array::from_fn(|i| part(i, ax, ay, old_rot, &here));
        let mut template: Vec<Key> = Vec::new();
        for q in props(l) {
            for k in q.track.keys() {
                if times.iter().any(|x| (x - k.time).abs() < eps) && !template.iter().any(|x| (x.time - k.time).abs() < eps) {
                    template.push(*k);
                }
            }
        }
        (times, world, before, template, ax, ay, old_rot)
    };
    {
        let Some(l) = st.project.layer_mut(sym, child) else { return fail("no layer") };
        l.joins.retain(|j| (j.time - t).abs() > eps);
        let at = l.joins.iter().position(|j| j.time > t).unwrap_or(l.joins.len());
        l.joins.insert(at, JoinSwitch { time: t, parent: parent.map(str::to_string), node: node.map(str::to_string), rotate, before });
    }
    // the same places in the new frame
    let local: Vec<Affine> = {
        let p = &st.project;
        let s = p.symbol(sym).expect("symbol");
        let l = s.layer(child).expect("layer");
        times.iter().zip(&world).map(|(&tt, w)| eval::parent_frame(p, s, l, tt).inverse().mul(w)).collect()
    };
    let l = st.project.layer_mut(sym, child).expect("layer");
    for (i, q) in props_mut(l).into_iter().enumerate() {
        for (j, &tt) in times.iter().enumerate() {
            let interp = template.iter().find(|k| (k.time - tt).abs() < eps).map_or(Interp::Linear, |k| k.interp);
            let k = q.track.keys().iter().find(|k| (k.time - tt).abs() < eps).copied().unwrap_or(Key::new(tt, Value::F(0.0), interp));
            q.track.put(Key { time: tt, value: Value::F(part(i, ax, ay, old_rot, &local[j])), ..k });
        }
    }
    Ok(())
}
