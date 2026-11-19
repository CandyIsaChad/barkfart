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
