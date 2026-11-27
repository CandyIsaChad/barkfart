//! Movieclips (port of Java SymbolOps): make layers into a symbol with one
//! looping instance in their place, and break an instance apart into plain
//! layers posed as it shows them now.

use super::{fail, EditResult, State};
use crate::eval::{self, Affine, Resolver};
use crate::model::*;
use std::sync::Arc;

/// Moves the layers (in stack order) into a new symbol, with one looping
/// instance where the first was. Joins across the selection's edge are
/// refused. Returns (symbol id, instance id).
pub fn convert_to_symbol(st: &mut State, sym: &str, layers: &[String], name: &str) -> EditResult<(String, String)> {
    if layers.is_empty() {
        return fail("nothing selected");
    }
    {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        for id in layers {
            if s.layer(id).is_none() {
                return fail(format!("{id} is not in this symbol"));
            }
        }
        for l in &s.layers {
            let inside = layers.contains(&l.id);
            if let Some(p) = &l.parent {
                if inside != layers.contains(p) {
                    return fail(format!("{} is joined across the selection; include both or unjoin first", l.name.clone().unwrap_or_default()));
                }
            }
        }
    }
    let sid = st.project.new_id("S");
    let duration = st.project.symbol(sym).expect("symbol").duration;
    let mut new_sym = Symbol::new(&sid, Some(name), duration);
    let s = st.project.symbol_mut(sym).expect("symbol");
    let at = s.layers.iter().position(|l| layers.contains(&l.id)).unwrap_or(s.layers.len());
    let (moved, rest): (Vec<Arc<Layer>>, Vec<Arc<Layer>>) = s.layers.drain(..).partition(|l| layers.contains(&l.id));
    s.layers = rest;
    new_sym.layers = moved;
    st.project.add_symbol(new_sym);
    let lid = st.project.new_id("L");
    let mut inst = Layer::symbol(&lid, Some(name), &sid);
    if let LayerKind::Symbol(x) = &mut inst.kind {
        x.loop_mode = SymbolLoop::Loop;
    }
    let s = st.project.symbol_mut(sym).expect("symbol");
    let at = at.min(s.layers.len());
    s.layers.insert(at, Arc::new(inst));
    Ok((sid, lid))
}

fn set_transform(c: &mut Layer, m: &Affine) {
    let sx = m.a.hypot(m.b);
    let rot = m.b.atan2(m.a).to_degrees();
    let sy = if sx < 1e-12 { 0.0 } else { (m.a * m.d - m.b * m.c) / sx };
    let tr = &mut c.transform;
    tr.anchor_x.value = Value::F(0.0);
    tr.anchor_y.value = Value::F(0.0);
    tr.x.value = Value::F(m.tx as f32);
    tr.y.value = Value::F(m.ty as f32);
    tr.rotation.value = Value::F(rot as f32);
    tr.scale_x.value = Value::F(sx as f32);
    tr.scale_y.value = Value::F(sy as f32);
}

/// Replaces an instance by static copies of its symbol's figure and sprite
/// layers, posed as shown at t, with the instance's placement and opacity
/// baked in (other layer types are skipped). Returns the new layer ids.
pub fn break_apart(st: &mut State, sym: &str, inst: &str, t: f64) -> EditResult<Vec<String>> {
    let p = st.project.clone();
    let Some(s) = p.symbol(sym) else { return fail("no symbol") };
    let Some(il) = s.layer(inst) else { return fail("not in this symbol") };
    let LayerKind::Symbol(sl) = &il.kind else { return fail("not a movieclip") };
    let Some(target) = p.symbol(&sl.symbol) else { return fail("the movieclip's symbol is missing") };
    let mut r = Resolver::new(&p);
    let inner = eval::instance_time(sl, target, t, &mut r);
    let place = eval::layer_stage(&p, s, il, t);
    let opacity = r.f(&il.opacity, t);
    let at = s.layer_index(inst).expect("found");
    let mut out = Vec::new();
    let mut ids = Vec::new();
    for l in &target.layers {
        // ids only for the layers that are copied (like the Java version)
        if !matches!(l.kind, LayerKind::Figure(_) | LayerKind::Sprite(_)) {
            continue;
        }
        let lid = st.project.new_id("L");
        let mut c = match &l.kind {
            LayerKind::Figure(f) => {
                let mut c = Layer::figure(&lid, l.name.as_deref());
                let LayerKind::Figure(cf) = &mut c.kind else { unreachable!() };
                for n in f.nodes() {
                    let mut m = Node::new(&lid, &n.id, n.parent.as_deref(), n.shape);
                    m.draw_order = n.draw_order;
                    m.curve = n.curve;
                    m.triangle_upside_down = n.triangle_upside_down;
                    m.right_triangle = n.right_triangle;
                    m.gradient = n.gradient;
                    (m.gradient_reverse, m.gradient_sideways, m.gradient_linear) = n.effective_gradient();
                    m.is_static = n.is_static;
                    m.stretchy = n.stretchy;
                    m.smart_stretch = n.smart_stretch;
                    m.no_smart_stretch = n.no_smart_stretch;
                    m.rest_length = n.rest_length;
                    m.circle_hollow = n.circle_hollow;
                    m.circle_outline = n.circle_outline;
                    m.half_arc = n.half_arc;
                    m.polygon_sides = n.polygon_sides;
                    m.trapezoid_ratio = n.trapezoid_ratio;
                    m.trapezoid_round_start = n.trapezoid_round_start;
                    m.trapezoid_round_end = n.trapezoid_round_end;
                    m.angle_lock = n.angle_lock;
                    m.drag_lock = n.drag_lock;
                    m.pinned = n.pinned;
                    m.rotation.value = r.resolve(&n.rotation, inner);
                    m.length.value = r.resolve(&n.length, inner);
                    m.thickness.value = r.resolve(&n.thickness, inner);
                    m.scale.value = r.resolve(&n.scale, inner);
                    m.color.value = r.resolve(&n.color, inner);
                    m.shown.value = r.resolve(&n.shown, inner);
                    m.gradient_color.value = r.resolve(&n.gradient_color, inner);
                    m.outline_color.value = r.resolve(&n.outline_color, inner);
                    let _ = cf.add(m);
                }
                for pf in &f.polyfills {
                    let mut q = Polyfill::new(&lid, &pf.id);
                    q.anchor = pf.anchor.clone();
                    q.nodes = pf.nodes.clone();
                    q.use_own_color = pf.use_own_color;
                    q.color.value = r.resolve(&pf.color, inner);
                    cf.polyfills.push(q);
                }
                c
            }
            LayerKind::Sprite(sp) => {
                let mut c = Layer::sprite(&lid, l.name.as_deref());
                if let LayerKind::Sprite(cs) = &mut c.kind {
                    cs.asset = sp.asset.clone();
                    cs.width = sp.width;
                    cs.height = sp.height;
                    cs.tint.value = r.resolve(&sp.tint, inner);
                }
                c
            }
            _ => continue,
        };
        let m = place.mul(&eval::layer_stage(&p, target, l, inner));
        set_transform(&mut c, &m);
        c.opacity.value = Value::F((opacity * r.f(&l.opacity, inner)).clamp(0.0, 1.0));
        c.shown.value = Value::B(r.b(&l.shown, inner));
        c.blend = l.blend;
        c.visible = l.visible;
        for e in l.effects.chain() {
            let mut n = EffectNode::new(&lid, &e.id, &e.ty);
            n.enabled = e.enabled;
            n.mix.value = r.resolve(&e.mix, inner);
            for (k, q) in &e.params {
                let v = r.resolve(q, inner);
                n.param(&lid, k, q.ty, v);
            }
            let _ = c.effects.append(n);
        }
        ids.push(lid);
        out.push(Arc::new(c));
    }
    let s = st.project.symbol_mut(sym).expect("symbol");
    s.layers.remove(at);
    let at = at.min(s.layers.len());
    s.layers.splice(at..at, out);
    Ok(ids)
}
