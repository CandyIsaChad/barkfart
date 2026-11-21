//! Whole-layer tools (port of Java LayerOps): duplicate (with joined
//! figures), push back / bring forward together with joined figures, copy /
//! paste all of a figure's properties, keep during tween.

use super::editor::{Editor, KeyRef};
use super::{fail, EditResult, State};
use crate::eval::Resolver;
use crate::model::*;
use std::sync::Arc;

/// The layer and every layer joined to it (directly or not), in stack order.
pub fn with_joined(s: &Symbol, id: &str) -> Vec<String> {
    let mut out = Vec::new();
    for x in &s.layers {
        let mut cur: Option<&Layer> = Some(x);
        let mut guard = s.layers.len() + 1;
        while let Some(c) = cur {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if c.id == id {
                out.push(x.id.clone());
                break;
            }
            cur = c.parent.as_deref().and_then(|p| s.layer(p));
        }
    }
    out
}

fn offset(q: &mut Property, d: f32) {
    if d == 0.0 {
        return;
    }
    q.value = Value::F(q.value.f() + d);
    let ks: Vec<Key> = q.track.keys().to_vec();
    q.track.clear();
    for k in ks {
        q.track.put(Key { value: Value::F(k.value.f() + d), ..k });
    }
}

/// Duplicates a layer (with `joined`, also the figures joined to it, still
/// joined to the copy), above the originals, moved by (dx, dy). Returns the
/// copies' ids, the copy of the layer itself first.
pub fn duplicate(st: &mut State, sym: &str, id: &str, joined: bool, dx: f32, dy: f32) -> EditResult<Vec<String>> {
    let block: Vec<String> = {
        let Some(s) = st.project.symbol(sym) else { return fail("no symbol") };
        if s.layer(id).is_none() {
            return fail("no layer");
        }
        if joined {
            with_joined(s, id)
        } else {
            vec![id.to_string()]
        }
    };
    let mut ids = std::collections::HashMap::new();
    for x in &block {
        let n = st.project.new_id("L");
        ids.insert(x.clone(), n);
    }
    let s = st.project.symbol(sym).expect("symbol");
    let at = block.iter().filter_map(|x| s.layer_index(x)).map(|i| i + 1).max().unwrap_or(0);
    let mut copies = Vec::new();
    let mut made = Vec::new();
    let orig_parent = s.layer(id).and_then(|l| l.parent.clone());
    for x in &block {
        let mut c = super::structure::copy_layer(s.layer(x).expect("layer"), &ids)?;
        if x == id {
            if !joined {
                c.parent = orig_parent.clone();
            }
            offset(&mut c.transform.x, dx);
            offset(&mut c.transform.y, dy);
        }
        made.push(c.id.clone());
        copies.push(Arc::new(c));
    }
    let s = st.project.symbol_mut(sym).expect("symbol");
    let at = at.min(s.layers.len());
    s.layers.splice(at..at, copies);
    Ok(made)
}

/// Push back / bring forward: the layer and its joined figures move
/// together past the next layer that is not part of them.
pub fn move_with_joined(st: &mut State, sym: &str, id: &str, forward: bool) -> EditResult<()> {
    let Some(s) = st.project.symbol_mut(sym) else { return fail("no symbol") };
    let block: std::collections::HashSet<String> = with_joined(s, id).into_iter().collect();
    let (inb, rest): (Vec<Arc<Layer>>, Vec<Arc<Layer>>) = s.layers.iter().cloned().partition(|l| block.contains(&l.id));
    let before = s.layers.iter().take_while(|l| !block.contains(&l.id)).count() as i64;
    let to = (before + if forward { 1 } else { -1 }).clamp(0, rest.len() as i64) as usize;
    let mut out: Vec<Arc<Layer>> = rest[..to].to_vec();
    out.extend(inb);
    out.extend(rest[to..].iter().cloned());
    s.layers = out;
    Ok(())
}

/// A figure's properties at one time ("Copy all figure properties").
#[derive(Clone, Debug)]
pub struct Props {
    pub transform: [f32; 5],
    pub opacity: f32,
    pub blend: Blend,
    pub effects: Vec<EffectNode>,
    pub source: String,
    pub stack_index: usize,
}

pub fn copy_props(p: &Project, sym: &str, id: &str, t: f64) -> EditResult<Props> {
    let Some(s) = p.symbol(sym) else { return fail("no symbol") };
    let Some(l) = s.layer(id) else { return fail("no layer") };
    let mut r = Resolver::new(p);
    let tr = &l.transform;
    let transform = [r.f(&tr.x, t), r.f(&tr.y, t), r.f(&tr.rotation, t), r.f(&tr.scale_x, t), r.f(&tr.scale_y, t)];
    let opacity = r.f(&l.opacity, t);
    let mut effects = Vec::new();
    for e in l.effects.chain() {
        let mut n = EffectNode::new("clip", &e.id, &e.ty);
        n.enabled = e.enabled;
        n.mix.value = r.resolve(&e.mix, t);
        for (k, q) in &e.params {
            let v = r.resolve(q, t);
            n.param("clip", k, q.ty, v);
        }
        effects.push(n);
    }
    Ok(Props { transform, opacity, blend: l.blend, effects, source: l.id.clone(), stack_index: s.layer_index(id).expect("found") })
}

/// Which copied properties a paste applies (Paste Stickfigure Properties).
#[derive(Clone, Copy, Debug)]
pub struct Parts {
    pub position: bool,
    pub scale: bool,
    pub rotation: bool,
    /// Opacity, blend and effects.
    pub filters: bool,
    pub layering: bool,
}

/// Pastes properties at time t (keyed like a pose edit): position (optional),
/// rotation, scale, opacity, blend and effects; with `layer`, its stack place.
#[allow(clippy::too_many_arguments)]
pub fn paste_props(ed: &Editor, st: &mut State, sym: &str, id: &str, c: &Props, position: bool, layer: bool, t: f64) -> EditResult<()> {
    paste_parts(ed, st, sym, id, c, Parts { position, scale: true, rotation: true, filters: true, layering: layer }, t)
}

/// Pastes the chosen parts of copied properties at time t.
pub fn paste_parts(ed: &Editor, st: &mut State, sym: &str, id: &str, c: &Props, parts: Parts, t: f64) -> EditResult<()> {
    let ids: Vec<String> = {
        let Some(l) = st.project.symbol(sym).and_then(|s| s.layer(id)) else { return fail("no layer") };
        let tr = &l.transform;
        [&tr.x, &tr.y, &tr.rotation, &tr.scale_x, &tr.scale_y, &l.opacity].iter().map(|q| q.id.clone()).collect()
    };
    for i in 0..5 {
        let on = match i {
            0 | 1 => parts.position,
            2 => parts.rotation,
            _ => parts.scale,
        };
        if on {
            ed.change_in(st, &ids[i], Value::F(c.transform[i]), t)?;
        }
    }
    if !parts.filters {
        return layering(st, sym, id, c, parts.layering);
    }
    ed.change_in(st, &ids[5], Value::F(c.opacity), t)?;
    let chain: Vec<String> = st.project.symbol(sym).and_then(|s| s.layer(id)).map(|l| l.effects.chain().iter().map(|e| e.id.clone()).collect()).unwrap_or_default();
    {
        let l = st.project.layer_mut(sym, id).expect("layer");
        if l.blend != c.blend {
            l.blend = c.blend;
        }
    }
    for e in chain {
        super::structure::remove_effect(st, sym, id, &e)?;
    }
    for e in &c.effects {
        let fx = st.project.new_id("FX");
        let mut n = EffectNode::new(id, &fx, &e.ty);
        n.enabled = e.enabled;
        n.mix.value = e.mix.value;
        for (k, q) in &e.params {
            n.param(id, k, q.ty, q.value);
        }
        super::structure::add_effect(st, sym, id, n)?;
    }
    layering(st, sym, id, c, parts.layering)
}

fn layering(st: &mut State, sym: &str, id: &str, c: &Props, on: bool) -> EditResult<()> {
    if on {
        let len = st.project.symbol(sym).map(|s| s.layers.len()).unwrap_or(1);
        let to = c.stack_index.min(len.saturating_sub(1));
        if st.project.symbol(sym).and_then(|s| s.layer_index(id)) != Some(to) {
            super::structure::move_layer(st, sym, id, to)?;
        }
    }
    Ok(())
}

/// "Keep during tween" off: every key of the layer holds until the next
/// (the figure jumps between poses); on: they tween again.
pub fn set_tweening(ed: &Editor, st: &mut State, sym: &str, id: &str, tween: bool) -> EditResult<()> {
    let sel: Vec<KeyRef> = {
        let Some(l) = st.project.symbol(sym).and_then(|s| s.layer(id)) else { return fail("no layer") };
        l.properties().iter().flat_map(|q| q.track.keys().iter().map(|k| KeyRef { property: q.id.clone(), time: k.time })).collect()
    };
    if sel.is_empty() {
        return Ok(());
    }
    ed.set_interp_in(st, &sel, if tween { Interp::Linear } else { Interp::Hold }, Ease::LINEAR)
}
