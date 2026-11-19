//! Structure edits with real logic (port of Java StructureEdits and
//! ProjectCodec.copyLayer). Plain settings (a flag, a name, a blend) are set
//! directly on the layer inside an undo step; these are the rest.

use super::{fail, EditResult, State};
use crate::model::*;
use std::sync::Arc;

/// A deep copy of a layer under new ids: `ids` maps old layer ids to new
/// ones, rewriting the layer's own id, its parent and property references
/// (links) that start with a mapped id. Names and text are never rewritten.
pub fn copy_layer(l: &Layer, ids: &std::collections::HashMap<String, String>) -> EditResult<Layer> {
    let mut p = Project { root_symbol: "S".into(), ..Default::default() };
    let mut s = Symbol::new("S", None, 1.0);
    s.layers.push(Arc::new(l.clone()));
    p.add_symbol(s);
    let json = crate::codec::encode(&p);
    let mut v: serde_json::Value = serde_json::from_str(&json).map_err(|e| super::EditError(e.to_string()))?;
    remap(&mut v, ids);
    let text = serde_json::to_string(&v).map_err(|e| super::EditError(e.to_string()))?;
    let q = crate::codec::decode(&text).map_err(|e| super::EditError(format!("cannot copy layer: {e}")))?;
    let layer = q.root().and_then(|s| s.layers.first()).map(|l| (**l).clone());
    match layer {
        Some(x) => Ok(x),
        None => fail("cannot copy layer"),
    }
}

fn remap(v: &mut serde_json::Value, ids: &std::collections::HashMap<String, String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m.iter_mut() {
                if k != "text" && k != "name" {
                    remap(x, ids);
                }
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(|x| remap(x, ids)),
        serde_json::Value::String(s) => {
            if let Some(n) = ids.get(s.as_str()) {
                *s = n.clone();
            } else if let Some(slash) = s.find('/') {
                if slash > 0 {
                    if let Some(n) = ids.get(&s[..slash]) {
                        *s = format!("{n}{}", &s[slash..]);
                    }
                }
            }
        }
        _ => {}
    }
}

fn layer<'a>(st: &'a mut State, symbol: &str, id: &str) -> EditResult<&'a mut Layer> {
    match st.project.layer_mut(symbol, id) {
        Some(l) => Ok(l),
        None => fail(format!("no layer {id}")),
    }
}

fn symbol<'a>(st: &'a mut State, id: &str) -> EditResult<&'a mut Symbol> {
    match st.project.symbol_mut(id) {
        Some(s) => Ok(s),
        None => fail(format!("no symbol {id}")),
    }
}

pub fn add_layer(st: &mut State, sym: &str, l: Layer, index: usize) -> EditResult<()> {
    let s = symbol(st, sym)?;
    let i = index.min(s.layers.len());
    s.layers.insert(i, Arc::new(l));
    Ok(())
}

pub fn remove_layer(st: &mut State, sym: &str, id: &str) -> EditResult<()> {
    let s = symbol(st, sym)?;
    let Some(i) = s.layer_index(id) else { return fail("layer not in symbol") };
    s.layers.remove(i);
    Ok(())
}

/// Moves a layer to an index (0 = bottom).
pub fn move_layer(st: &mut State, sym: &str, id: &str, to: usize) -> EditResult<()> {
    let s = symbol(st, sym)?;
    let Some(from) = s.layer_index(id) else { return fail("layer not in symbol") };
    let l = s.layers.remove(from);
    let target = to.min(s.layers.len());
    s.layers.insert(target, l);
    Ok(())
}

/// Sets the parent layer; refuses a parent that would make a cycle.
pub fn set_parent(st: &mut State, sym: &str, id: &str, parent: Option<&str>) -> EditResult<()> {
    {
        let s = st.project.symbol(sym).ok_or_else(|| super::EditError("no symbol".into()))?;
        let mut p = parent.map(str::to_string);
        let mut guard = s.layers.len() + 1;
        while let Some(pid) = p {
            if guard == 0 {
                break;
            }
            guard -= 1;
            if pid == id {
                return fail("parenting would create a cycle");
            }
            p = s.layer(&pid).and_then(|l| l.parent.clone());
        }
    }
    layer(st, sym, id)?.parent = parent.map(str::to_string);
    Ok(())
}

fn figure<'a>(st: &'a mut State, sym: &str, id: &str) -> EditResult<&'a mut Figure> {
    match &mut layer(st, sym, id)?.kind {
        LayerKind::Figure(f) => Ok(f),
        _ => fail("not a figure"),
    }
}

pub fn add_node(st: &mut State, sym: &str, id: &str, n: Node) -> EditResult<()> {
    let f = figure(st, sym, id)?;
    if !n.parent.as_ref().is_some_and(|p| f.node(p).is_some()) {
        return fail(format!("unknown parent {:?}", n.parent));
    }
    f.add(n).map_err(super::EditError)
}

/// Removes a node, its descendants and the polyfills that use any of them.
pub fn remove_node(st: &mut State, sym: &str, id: &str, node: &str) -> EditResult<()> {
    let f = figure(st, sym, id)?;
    let sub: std::collections::HashSet<String> = f.subtree(node).into_iter().collect();
    f.remove_subtree(node).map_err(super::EditError)?;
    f.polyfills.retain(|pf| !sub.contains(&pf.anchor) && !pf.nodes.iter().any(|x| sub.contains(x)));
    Ok(())
}

/// Removes an effect from a linear chain, linking what followed it to what fed it.
pub fn remove_effect(st: &mut State, sym: &str, id: &str, effect: &str) -> EditResult<()> {
    let l = layer(st, sym, id)?;
    let Some(e) = l.effects.nodes.shift_remove(effect) else { return fail("no such effect") };
    for n in l.effects.nodes.values_mut() {
        if n.inputs.iter().any(|x| x == effect) {
            let mut ins = Vec::new();
            for x in &n.inputs {
                if x == effect {
                    ins.extend(e.inputs.iter().cloned());
                } else {
                    ins.push(x.clone());
                }
            }
            n.inputs = ins;
        }
    }
    if l.effects.output.as_deref() == Some(effect) {
        l.effects.output = e.inputs.first().cloned();
    }
    Ok(())
}

pub fn add_effect(st: &mut State, sym: &str, id: &str, e: EffectNode) -> EditResult<()> {
    layer(st, sym, id)?.effects.append(e).map_err(super::EditError)
}

pub fn clear_effects(st: &mut State, sym: &str, id: &str) -> EditResult<()> {
    let l = layer(st, sym, id)?;
    l.effects.nodes.clear();
    l.effects.output = None;
    Ok(())
}

/// Copy / paste a sound: a copy of a clip starting at `at` on the same layer,
/// with trims, fades, loop, pitch, and volume / pan keys moved along.
pub fn paste_clip(st: &mut State, sym: &str, id: &str, clip: &str, at: f64) -> EditResult<String> {
    let cid = st.project.new_id("C");
    let l = layer(st, sym, id)?;
    let lid = l.id.clone();
    let LayerKind::Audio(clips) = &mut l.kind else { return fail("not a sound layer") };
    let Some(src) = clips.iter().find(|c| c.id == clip).cloned() else { return fail("no such sound") };
    let mut c = Clip::new(&lid, &cid, src.asset.clone(), at, src.in_, src.out);
    c.loop_ = src.loop_;
    c.fade_in = src.fade_in;
    c.fade_out = src.fade_out;
    c.pitch = src.pitch;
    let dt = at - src.start;
    for (from, to) in [(&src.volume, &mut c.volume), (&src.pan, &mut c.pan)] {
        to.value = from.value;
        for k in from.track.keys() {
            to.track.put(Key { time: k.time + dt, ..*k });
        }
    }
    clips.push(c);
    Ok(cid)
}

/// Canvas presets: name, width, height.
pub const CANVAS_PRESETS: [(&str, i32, i32); 6] =
    [("16:9 HD", 1280, 720), ("16:9 Full HD", 1920, 1080), ("9:16 vertical", 720, 1280), ("9:16 vertical Full HD", 1080, 1920), ("1:1 square", 1080, 1080), ("4:3", 1024, 768)];

pub fn set_canvas_size(st: &mut State, w: i32, h: i32) -> EditResult<()> {
    if !(16..=8192).contains(&w) || !(16..=8192).contains(&h) {
        return fail("canvas 16..8192");
    }
    st.project.settings.width = w;
    st.project.settings.height = h;
    Ok(())
}

/// Parallax of a camera-locked layer, clamped to 0..1.
pub fn set_parallax(st: &mut State, sym: &str, id: &str, v: f32) -> EditResult<()> {
    layer(st, sym, id)?.parallax = v.clamp(0.0, 1.0);
    Ok(())
}

/// Adds an asset with its bytes (one undo step with whatever uses it).
pub fn add_asset(st: &mut State, a: Asset, bytes: Vec<u8>) {
    st.assets.insert(a.path.clone(), Arc::new(bytes));
    st.project.assets.insert(a.id.clone(), a);
}
