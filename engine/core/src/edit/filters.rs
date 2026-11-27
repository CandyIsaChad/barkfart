//! Stickfigure Filters (reference 9.4): every panel control is one effect
//! node on the selected figure's layer, keyed on the frame it was changed on.
//! The nodes are chained in the panel's order so the picture matches it.

use super::{fail, EditResult, State};
use crate::eval::Resolver;
use crate::model::*;
use serde_json::Value as J;

pub const INVERT_ID: &str = "filter.invert";
pub const ADJUST_ID: &str = "filter.adjust";
pub const PIXELATE_ID: &str = "filter.pixelate";
pub const TINT_ID: &str = "filter.tint";
pub const OUTLINE_ID: &str = "filter.outline";
pub const BLUR_ID: &str = "filter.blur";
pub const MOTION_ID: &str = "filter.motion";
pub const GLOW_ID: &str = "filter.glow";
pub const SHADOW_ID: &str = "filter.shadow";

/// The Effects list's names for the filter nodes.
pub const KINDS: [(&str, &str); 9] = [("invert", INVERT_ID), ("adjust", ADJUST_ID), ("pixelate", PIXELATE_ID), ("tint", TINT_ID), ("outline", OUTLINE_ID), ("blur", BLUR_ID), ("motion", MOTION_ID), ("glow", GLOW_ID), ("shadow", SHADOW_ID)];

fn kind_id(kind: &str) -> Option<&'static str> {
    KINDS.iter().find(|(k, _)| *k == kind).map(|(_, id)| *id)
}

/// The panel's order (top to bottom).
const ORDER: [&str; 9] = [INVERT_ID, ADJUST_ID, PIXELATE_ID, TINT_ID, OUTLINE_ID, BLUR_ID, MOTION_ID, GLOW_ID, SHADOW_ID];

fn fps_of(st: &State) -> f64 {
    if st.project.settings.fps > 0.0 {
        st.project.settings.fps as f64
    } else {
        30.0
    }
}

thread_local! {
    /// D5 / H3: while `apply` runs with keying off, a change sets the value
    /// on every frame (the property stays static) instead of keying t.
    static STATIC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// D34: how a new effect key tweens (the project's tweens, set by `apply`).
    static TWEEN: std::cell::Cell<Interp> = const { std::cell::Cell::new(Interp::Linear) };
}

fn key_at(p: &mut Property, t: f64, v: Value) {
    // a value that already changes over time is animated: a change keys it
    // here whatever the toggle says (keying off never flattens an animation)
    let animated = p.track.keys().windows(2).any(|w| w[0].value != w[1].value);
    if STATIC.with(|s| s.get()) && !animated {
        p.value = v;
        let keys: Vec<Key> = p.track.keys().iter().map(|k| Key { value: v, ..*k }).collect();
        p.track.clear();
        for k in keys {
            p.track.put(k);
        }
        return;
    }
    // a new key tweens as the project's tweens do (D34), or as the key
    // before it here does
    let interp = if p.ty.continuous() { crate::edit::editor::interp_at(p, t, TWEEN.with(|x| x.get())) } else { Interp::Hold };
    // a first key keeps what the frames before showed: the old value from the
    // start, held (an effect switched on here appears here, it doesn't fade in
    // from the first frame; later changes tween as keys do)
    if p.track.is_empty() && t > 0.0 && p.value != v {
        p.track.put(Key::new(0.0, p.value, Interp::Hold));
    }
    p.track.put(Key::new(t, v, interp));
}

/// A property that has never been keyed gets its current value on every
/// frame, so a change on one frame leaves the others alone.
fn key_every_frame(p: &mut Property, n: i32, fps: f64) {
    if !p.track.is_empty() {
        return;
    }
    let base = p.value.clone();
    for i in 0..n {
        p.track.put(Key::new(i as f64 / fps, base.clone(), Interp::Linear));
    }
}

/// Makes sure the filter's effect node exists, silent (mix 0).
fn ensure(l: &mut Layer, n: i32, fps: f64, id: &str, ty: &str) {
    if l.effects.nodes.contains_key(id) {
        return;
    }
    let mut e = EffectNode::new(&l.id, id, ty);
    // the panel's starting values, written out so the picture and the panel agree
    let owner = l.id.clone();
    match ty {
        crate::fx::OUTLINE => {
            e.param(&owner, "width", ValueType::Float, Value::F(4.0));
        }
        crate::fx::MOTION => {
            e.param(&owner, "distance", ValueType::Float, Value::F(30.0));
        }
        crate::fx::SHADOW => {
            e.param(&owner, "dx", ValueType::Float, Value::F(-7.07));
            e.param(&owner, "dy", ValueType::Float, Value::F(7.07));
            e.param(&owner, "radius", ValueType::Float, Value::F(6.0));
        }
        _ => {}
    }
    // silent until a control turns it on: a value, not a key on every frame
    // (D5: keys are where something changes; H3)
    let _ = (n, fps);
    e.mix.value = Value::F(0.0);
    if l.effects.append(e).is_ok() {
        reorder(l);
    }
}

/// Filters run in the panel's order; anything else (stacked FX* effects, plugin
/// effects, effects from files) stays in front of them, preserving its own chain order.
fn reorder(l: &mut Layer) {
    let ours: std::collections::HashSet<&str> = ORDER.into_iter().collect();
    let mut chain: Vec<String> = l.effects.chain().into_iter().filter(|e| !ours.contains(e.id.as_str())).map(|e| e.id.clone()).collect();
    for k in l.effects.nodes.keys() {
        if !ours.contains(k.as_str()) && !chain.contains(k) {
            chain.push(k.clone());
        }
    }
    chain.extend(ORDER.iter().filter(|id| l.effects.nodes.contains_key(**id)).map(|id| id.to_string()));
    rewire_chain(&mut l.effects, &chain);
}

fn rewire_chain(stack: &mut EffectStack, chain: &[String]) {
    let mut prev: Option<String> = None;
    for id in chain {
        if let Some(e) = stack.nodes.get_mut(id) {
            e.inputs = prev.iter().cloned().collect();
            prev = Some(id.clone());
        }
    }
    stack.output = prev;
}

fn set_mix(l: &mut Layer, id: &str, t: f64, v: f32) {
    if let Some(e) = l.effects.nodes.get_mut(id) {
        key_at(&mut e.mix, t, Value::F(v.clamp(0.0, 1.0)));
    }
}

fn set_param(l: &mut Layer, id: &str, name: &str, ty: ValueType, t: f64, v: Value) {
    let owner = l.id.clone();
    let Some(e) = l.effects.nodes.get_mut(id) else { return };
    // a setting changed for the first time starts from its default, so a key
    // keeps what the other frames showed (it doesn't reach back)
    let p = match e.params.get_mut(name) {
        Some(p) => p,
        None => e.param(&owner, name, ty, default_param(id, name).unwrap_or(v)),
    };
    key_at(p, t, v);
}

/// Applies the controls present in the command ({"op":"filter", ...}).
/// Applies the controls with the keying toggle (D5): keyed, a change is a
/// key on t; not keyed, it is the value on every frame.
pub fn apply_keyed(st: &mut State, sym: &str, id: &str, c: &J, t: f64, keyed: bool) -> EditResult<()> {
    STATIC.with(|s| s.set(!keyed));
    let r = apply(st, sym, id, c, t);
    STATIC.with(|s| s.set(false));
    r
}

pub fn apply(st: &mut State, sym: &str, id: &str, c: &J, t: f64) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    TWEEN.with(|x| x.set(st.project.settings.tween_interp));
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no layer") };
    if !matches!(l.kind, LayerKind::Figure(_) | LayerKind::Adjustment | LayerKind::Sprite(_) | LayerKind::Symbol(_) | LayerKind::Text(_)) {
        return fail("not a drawable layer");
    }
    apply_to(l, n, fps, c, t)
}

/// K / D33: the same controls on a group's effect stack. Group effects hold
/// static values (keying is off whatever the toggle says); "opacity" is the
/// group's own and goes through `group_ops::set`.
pub fn apply_group(st: &mut State, sym: &str, gid: &str, c: &J) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    let Some(s) = st.project.symbol_mut(sym) else { return fail("no symbol") };
    let Some(g) = s.groups.iter_mut().find(|g| g.id == gid) else { return fail("There is no such group.") };
    let mut scratch = Layer::adjustment(gid, None);
    scratch.effects = std::mem::take(&mut g.effects);
    let mut c = c.clone();
    if let Some(m) = c.as_object_mut() {
        m.remove("opacity");
    }
    STATIC.with(|x| x.set(true));
    let r = apply_to(&mut scratch, n, fps, &c, 0.0);
    STATIC.with(|x| x.set(false));
    g.effects = scratch.effects;
    r
}

/// K / D33: a group's effects as the Effects list reads them.
pub fn group_state(p: &Project, sym: &str, gid: &str) -> J {
    let Some(g) = p.symbol(sym).and_then(|s| s.groups.iter().find(|g| g.id == gid)) else {
        return serde_json::json!({});
    };
    let mut scratch = Layer::adjustment(gid, None);
    scratch.effects = g.effects.clone();
    state_of(p, &scratch, 0.0)
}

fn apply_to(l: &mut Layer, n: i32, fps: f64, c: &J, t: f64) -> EditResult<()> {
    // H2: the Effects list's eye: hidden keeps the settings and keys
    if let Some(m) = c["shown"].as_object() {
        for (kind, v) in m {
            if let (Some(id), Some(on)) = (kind_id(kind), v.as_bool()) {
                if let Some(e) = l.effects.nodes.get_mut(id) {
                    e.enabled = on;
                }
            }
        }
    }
    if let Some(v) = c["opacity"].as_f64() {
        key_at(&mut l.opacity, t, Value::F(v.clamp(0.0, 1.0) as f32));
    }
    if let Some(v) = c["invert"].as_bool() {
        ensure(l, n, fps, INVERT_ID, crate::fx::INVERT);
        set_mix(l, INVERT_ID, t, if v { 1.0 } else { 0.0 });
    }
    if let Some(v) = c["hue"].as_f64() {
        ensure(l, n, fps, ADJUST_ID, crate::fx::ADJUST);
        set_param(l, ADJUST_ID, "hue", ValueType::Float, t, Value::F(v as f32));
        set_mix(l, ADJUST_ID, t, 1.0);
    }
    if let Some(v) = c["saturation"].as_f64() {
        ensure(l, n, fps, ADJUST_ID, crate::fx::ADJUST);
        set_param(l, ADJUST_ID, "saturation", ValueType::Float, t, Value::F(v.clamp(0.0, 4.0) as f32));
        set_mix(l, ADJUST_ID, t, 1.0);
    }
    if let Some(v) = c["pixelate"].as_f64() {
        ensure(l, n, fps, PIXELATE_ID, crate::fx::PIXELATE);
        set_param(l, PIXELATE_ID, "size", ValueType::Float, t, Value::F(v.clamp(1.0, crate::fx::MAX_LENGTH as f64) as f32));
        set_mix(l, PIXELATE_ID, t, 1.0);
    }
    if let Some(v) = c["tint"].as_u64() {
        ensure(l, n, fps, TINT_ID, crate::fx::TINT);
        set_param(l, TINT_ID, "color", ValueType::Color, t, Value::C(Color::rgba8888(v as u32)));
    }
    if let Some(v) = c["tintAmount"].as_f64() {
        ensure(l, n, fps, TINT_ID, crate::fx::TINT);
        set_param(l, TINT_ID, "amount", ValueType::Float, t, Value::F(v.clamp(0.0, 1.0) as f32));
        set_mix(l, TINT_ID, t, 1.0);
    }
    if let Some(v) = c["outline"].as_u64() {
        ensure(l, n, fps, OUTLINE_ID, crate::fx::OUTLINE);
        set_param(l, OUTLINE_ID, "color", ValueType::Color, t, Value::C(Color::rgba8888(v as u32)));
    }
    if let Some(v) = c["outlineAmount"].as_f64() {
        ensure(l, n, fps, OUTLINE_ID, crate::fx::OUTLINE);
        set_mix(l, OUTLINE_ID, t, v as f32);
    }
    if let Some(v) = c["outlineThickness"].as_f64() {
        ensure(l, n, fps, OUTLINE_ID, crate::fx::OUTLINE);
        set_param(l, OUTLINE_ID, "width", ValueType::Float, t, Value::F(v.clamp(0.0, 64.0) as f32));
    }
    if let Some(v) = c["blur"].as_f64() {
        ensure(l, n, fps, BLUR_ID, crate::fx::BLUR);
        set_param(l, BLUR_ID, "radius", ValueType::Float, t, Value::F(v.clamp(0.0, crate::fx::MAX_LENGTH as f64) as f32));
        set_mix(l, BLUR_ID, t, 1.0);
    }
    if let Some(v) = c["motion"].as_bool() {
        ensure(l, n, fps, MOTION_ID, crate::fx::MOTION);
        set_mix(l, MOTION_ID, t, if v { 1.0 } else { 0.0 });
    }
    if let Some(v) = c["motionDistance"].as_f64() {
        ensure(l, n, fps, MOTION_ID, crate::fx::MOTION);
        set_param(l, MOTION_ID, "distance", ValueType::Float, t, Value::F(v.clamp(0.0, crate::fx::MAX_LENGTH as f64) as f32));
    }
    if let Some(v) = c["motionAngle"].as_f64() {
        ensure(l, n, fps, MOTION_ID, crate::fx::MOTION);
        set_param(l, MOTION_ID, "angle", ValueType::Float, t, Value::F(v as f32));
    }
    if let Some(v) = c["glow"].as_u64() {
        ensure(l, n, fps, GLOW_ID, crate::fx::GLOW);
        set_param(l, GLOW_ID, "color", ValueType::Color, t, Value::C(Color::rgba8888(v as u32)));
    }
    if let Some(v) = c["glowAmount"].as_f64() {
        ensure(l, n, fps, GLOW_ID, crate::fx::GLOW);
        set_mix(l, GLOW_ID, t, v as f32);
    }
    if let Some(v) = c["glowIntensity"].as_f64() {
        ensure(l, n, fps, GLOW_ID, crate::fx::GLOW);
        set_param(l, GLOW_ID, "strength", ValueType::Float, t, Value::F(v.clamp(0.0, 4.0) as f32));
    }
    if let Some(v) = c["shadow"].as_u64() {
        ensure(l, n, fps, SHADOW_ID, crate::fx::SHADOW);
        set_param(l, SHADOW_ID, "color", ValueType::Color, t, Value::C(Color::rgba8888(v as u32)));
    }
    if let Some(v) = c["shadowOpacity"].as_f64() {
        ensure(l, n, fps, SHADOW_ID, crate::fx::SHADOW);
        set_mix(l, SHADOW_ID, t, v as f32);
    }
    if c.get("shadowDistance").is_some() || c.get("shadowAngle").is_some() {
        let distance = c["shadowDistance"].as_f64().unwrap_or(10.0);
        let angle = c["shadowAngle"].as_f64().unwrap_or(135.0).to_radians();
        ensure(l, n, fps, SHADOW_ID, crate::fx::SHADOW);
        set_param(l, SHADOW_ID, "dx", ValueType::Float, t, Value::F((angle.cos() * distance) as f32));
        set_param(l, SHADOW_ID, "dy", ValueType::Float, t, Value::F((angle.sin() * distance) as f32));
    }
    if let Some(v) = c["shadowBlur"].as_f64() {
        ensure(l, n, fps, SHADOW_ID, crate::fx::SHADOW);
        set_param(l, SHADOW_ID, "radius", ValueType::Float, t, Value::F(v.clamp(0.0, crate::fx::MAX_LENGTH as f64) as f32));
    }
    reorder(l);
    Ok(())
}

/// The panel's starting values of a filter's settings.
fn default_param(node: &str, name: &str) -> Option<Value> {
    let f = |v: f32| Some(Value::F(v));
    let c = |v: u32| Some(Value::C(Color::rgba8888(v)));
    match (node, name) {
        (ADJUST_ID, "hue") => f(0.0),
        (ADJUST_ID, "saturation") => f(1.0),
        (PIXELATE_ID, "size") => f(1.0),
        (TINT_ID, "color") => c(0xffffffff),
        (TINT_ID, "amount") => f(0.0),
        (OUTLINE_ID, "color") => c(0xff00ffff),
        (OUTLINE_ID, "width") => f(4.0),
        (BLUR_ID, "radius") => f(0.0),
        (MOTION_ID, "distance") => f(30.0),
        (MOTION_ID, "angle") => f(0.0),
        (GLOW_ID, "color") => c(0x0a84ffff),
        (GLOW_ID, "strength") => f(1.0),
        (SHADOW_ID, "color") => c(0x000000ff),
        (SHADOW_ID, "dx") => f(-7.07),
        (SHADOW_ID, "dy") => f(7.07),
        (SHADOW_ID, "radius") => f(6.0),
        _ => None,
    }
}

/// Clear all filters of the figure on this frame (filters belong to a frame, like
/// every other setting of a snapshot): every filter goes silent here with its
/// settings back to the defaults, and the opacity is 1. Other frames keep theirs.
/// A filter that ends up silent on every frame goes altogether.
pub fn clear(st: &mut State, sym: &str, id: &str, t: f64) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    {
        let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
        if l.as_figure().is_none() {
            return fail("not a figure");
        }
        let present: Vec<(String, Vec<String>)> = ORDER.iter().filter_map(|x| l.effects.nodes.get(*x).map(|e| ((*x).to_string(), e.params.keys().cloned().collect()))).collect();
        for (x, params) in present {
            set_mix(l, &x, t, 0.0);
            for name in params {
                if let Some(v) = default_param(&x, &name) {
                    let ty = if matches!(v, Value::C(_)) { ValueType::Color } else { ValueType::Float };
                    set_param(l, &x, &name, ty, t, v);
                }
            }
        }
    }
    let gone: Vec<String> = {
        let Some(l) = st.project.root().and_then(|r| r.layer(id)) else { return fail("no figure") };
        ORDER.iter().filter(|x| l.effects.nodes.get(**x).is_some_and(|e| e.mix.track.keys().iter().all(|k| matches!(k.value, Value::F(v) if v <= 0.0)))).map(|x| (*x).to_string()).collect()
    };
    for x in gone {
        super::structure::remove_effect(st, sym, id, &x)?;
    }
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
    key_every_frame(&mut l.opacity, n, fps);
    key_at(&mut l.opacity, t, Value::F(1.0));
    Ok(())
}

/// A copied filter set: the layer's opacity and its filter nodes at time t.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    opacity: f32,
    nodes: Vec<SnapNode>,
}

#[derive(Clone, Debug, PartialEq)]
struct SnapNode {
    id: String,
    ty: String,
    mix: f32,
    params: Vec<(String, ValueType, Value)>,
}

pub fn snapshot(p: &Project, sym: &str, id: &str, t: f64) -> Option<Snapshot> {
    let l = p.symbol(sym)?.layer(id)?;
    // a figure with no filters and a plain opacity needs no evaluation (and a Resolver
    // indexes the whole project when it is made: this runs for every figure on a copy)
    if !ORDER.iter().any(|x| l.effects.nodes.contains_key(*x)) && l.opacity.track.is_empty() && l.opacity.modifiers.is_empty() {
        return Some(Snapshot { opacity: l.opacity.value.f(), nodes: Vec::new() });
    }
    let mut r = Resolver::new(p);
    let mut nodes = Vec::new();
    for e in l.effects.nodes.values() {
        if !ORDER.contains(&e.id.as_str()) {
            continue;
        }
        let mut params = Vec::new();
        for (k, q) in &e.params {
            params.push((k.clone(), q.ty, r.resolve(q, t)));
        }
        nodes.push(SnapNode { id: e.id.clone(), ty: e.ty.clone(), mix: r.f(&e.mix, t), params });
    }
    Some(Snapshot { opacity: r.f(&l.opacity, t), nodes })
}

pub fn restore(st: &mut State, sym: &str, id: &str, snap: &Snapshot, t: f64) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    {
        let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
        if l.as_figure().is_none() {
            return fail("not a figure");
        }
        key_every_frame(&mut l.opacity, n, fps);
        key_at(&mut l.opacity, t, Value::F(snap.opacity));
    }
    for s in &snap.nodes {
        let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
        ensure(l, n, fps, &s.id, &s.ty);
        set_mix(l, &s.id, t, s.mix);
        for (k, ty, v) in &s.params {
            set_param(l, &s.id, k, *ty, t, v.clone());
        }
    }
    if let Some(l) = st.project.layer_mut(sym, id) {
        reorder(l);
    }
    Ok(())
}

/// The panel's values at t (defaults where no filter node exists).
pub fn state(p: &Project, sym: &str, id: &str, t: f64) -> J {
    let Some(l) = p.symbol(sym).and_then(|s| s.layer(id)) else {
        return serde_json::json!({});
    };
    state_of(p, l, t)
}

fn state_of(p: &Project, l: &Layer, t: f64) -> J {
    let mut r = Resolver::new(p);
    let num = |r: &mut Resolver, e: Option<&EffectNode>, name: &str, d: f32| e.and_then(|e| e.params.get(name)).map(|q| r.f(q, t)).unwrap_or(d);
    let col = |r: &mut Resolver, e: Option<&EffectNode>, name: &str, d: u32| {
        e.and_then(|e| e.params.get(name))
            .map(|q| match r.resolve(q, t) {
                Value::C(c) => c.to_rgba8888(),
                _ => d,
            })
            .unwrap_or(d)
    };
    let mix = |r: &mut Resolver, e: Option<&EffectNode>, d: f32| e.map(|e| r.f(&e.mix, t)).unwrap_or(d);
    // controls whose value is their own on / off (blur 0, pixelate 1, hue 0, saturation 1,
    // tint amount 0): on a frame where the filter is silent the panel shows the off value,
    // not what another frame set
    let live = |r: &mut Resolver, e: Option<&EffectNode>, name: &str, on_default: f32, off: f32| {
        let silent = e.is_some_and(|e| r.f(&e.mix, t) <= 0.0);
        if silent { off } else { num(r, e, name, on_default) }
    };
    let n = |x: &str| l.effects.nodes.get(x);
    let (invert, adjust, pixel, tint, outline, blur, motion, glow, shadow) =
        (n(INVERT_ID), n(ADJUST_ID), n(PIXELATE_ID), n(TINT_ID), n(OUTLINE_ID), n(BLUR_ID), n(MOTION_ID), n(GLOW_ID), n(SHADOW_ID));
    let (dx, dy) = (num(&mut r, shadow, "dx", -7.07), num(&mut r, shadow, "dy", 7.07));
    let distance = (dx * dx + dy * dy).sqrt();
    // H2: which effects are on here (the Effects list on the Stickfigure page)
    let audible = |r: &mut Resolver, e: Option<&EffectNode>| e.is_some_and(|e| r.f(&e.mix, t) > 0.0);
    let on = serde_json::json!({
        "invert": audible(&mut r, invert),
        "adjust": audible(&mut r, adjust) && (live(&mut r, adjust, "hue", 0.0, 0.0).abs() > 1e-3 || (live(&mut r, adjust, "saturation", 1.0, 1.0) - 1.0).abs() > 1e-3),
        "pixelate": audible(&mut r, pixel) && live(&mut r, pixel, "size", 1.0, 1.0) > 1.0,
        "tint": audible(&mut r, tint) && live(&mut r, tint, "amount", 0.0, 0.0) > 0.0,
        "outline": audible(&mut r, outline),
        "blur": audible(&mut r, blur) && live(&mut r, blur, "radius", 0.0, 0.0) > 0.0,
        "motion": audible(&mut r, motion),
        "glow": audible(&mut r, glow),
        "shadow": audible(&mut r, shadow),
    });
    let hidden: Vec<&str> = KINDS.iter().filter(|(_, id)| l.effects.nodes.get(*id).is_some_and(|e| !e.enabled)).map(|(k, _)| *k).collect();
    // Wiggle and Swing (H2 Movement): modifiers on the figure's position / angle
    let tr = &l.transform;
    let wiggle = tr.x.modifiers.iter().find_map(|m| if let ModifierKind::Wiggle { frequency, amplitude, .. } = m.kind { Some(serde_json::json!({"amount": round2(amplitude), "speed": round2(frequency), "shown": m.enabled})) } else { None });
    let swing = tr.rotation.modifiers.iter().find_map(|m| if let ModifierKind::Sine { frequency, amplitude, .. } = m.kind { Some(serde_json::json!({"amount": round2(amplitude), "speed": round2(frequency), "shown": m.enabled})) } else { None });
    serde_json::json!({
        "on": on,
        "hidden": hidden,
        "wiggle": wiggle,
        "swing": swing,
        "opacity": round2(r.f(&l.opacity, t)),
        "invert": mix(&mut r, invert, 0.0) > 0.5,
        "hue": round2(live(&mut r, adjust, "hue", 0.0, 0.0)),
        "saturation": round2(live(&mut r, adjust, "saturation", 1.0, 1.0)),
        "pixelate": round2(live(&mut r, pixel, "size", 1.0, 1.0)),
        "tint": col(&mut r, tint, "color", 0xffffffff),
        "tintAmount": round2(live(&mut r, tint, "amount", 0.0, 0.0)),
        "outline": col(&mut r, outline, "color", 0xff00ffff),
        "outlineAmount": round2(mix(&mut r, outline, 1.0)),
        "outlineThickness": round2(num(&mut r, outline, "width", 4.0)),
        "blur": round2(live(&mut r, blur, "radius", 0.0, 0.0)),
        "motion": mix(&mut r, motion, 0.0) > 0.5,
        "motionDistance": round2(num(&mut r, motion, "distance", 30.0)),
        "motionAngle": round2(num(&mut r, motion, "angle", 0.0)),
        "glow": col(&mut r, glow, "color", 0x0a84ffff),
        "glowAmount": round2(mix(&mut r, glow, 0.0)),
        "glowIntensity": round2(num(&mut r, glow, "strength", 1.0)),
        "shadow": col(&mut r, shadow, "color", 0x000000ff),
        "shadowOpacity": round2(mix(&mut r, shadow, 0.0)),
        "shadowDistance": round2(distance),
        "shadowAngle": round2(dy.atan2(dx).to_degrees()),
        "shadowBlur": round2(num(&mut r, shadow, "radius", 6.0)),
    })
}

fn round2(v: f32) -> f64 {
    (v as f64 * 100.0).round() / 100.0
}

/// Whether any effect on the layer changes the picture on some frame (H1:
/// the FX toggle shows only then). A filter turned on and off again leaves
/// its node silent, which does not count. Read from the keys, not frame by
/// frame, so it stays cheap.
pub fn any_active(l: &Layer) -> bool {
    stack_active(&l.effects)
}

/// Whether a single effect node produces a visible change on any frame.
pub fn node_active(e: &EffectNode) -> bool {
    let values = |q: &Property| -> Vec<f32> {
        let mut v: Vec<f32> = q.track.keys().iter().map(|k| k.value.f()).collect();
        if v.is_empty() {
            v.push(q.value.f());
        }
        v
    };
    if !values(&e.mix).iter().any(|m| *m > 0.0) {
        return false;
    }
    let param_off = |name: &str, off: f32| e.params.get(name).is_none_or(|q| values(q).iter().all(|v| (v - off).abs() < 1e-4));
    match e.id.as_str() {
        BLUR_ID => !param_off("radius", 0.0),
        PIXELATE_ID => !param_off("size", 1.0),
        ADJUST_ID => !(param_off("hue", 0.0) && param_off("saturation", 1.0)),
        TINT_ID => !param_off("amount", 0.0),
        _ => true,
    }
}

/// The same for any effect stack (a group's, D33).
pub fn stack_active(effects: &EffectStack) -> bool {
    effects.nodes.values().any(node_active)
}


/// Wiggle and Swing (H2, the effects browser's Movement group) on a figure:
/// wiggle is noise on its position, swing a sine on its angle, both on top
/// of its poses. {"kind", "on"?, "amount"?, "speed"?, "shown"?}; one step.
pub fn movement(st: &mut State, sym: &str, id: &str, c: &J) -> EditResult<()> {
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
    if l.as_figure().is_none() {
        return fail("not a figure");
    }
    let wiggle = match c["kind"].as_str() {
        Some("wiggle") => true,
        Some("swing") => false,
        _ => return fail("wiggle or swing"),
    };
    let seed = id.bytes().fold(7i32, |a, b| a.wrapping_mul(31).wrapping_add(b as i32)) & 0xffff;
    let tr = &mut l.transform;
    let props: Vec<&mut Property> = if wiggle { vec![&mut tr.x, &mut tr.y] } else { vec![&mut tr.rotation] };
    let mine = |m: &Modifier| matches!((wiggle, &m.kind), (true, ModifierKind::Wiggle { .. }) | (false, ModifierKind::Sine { .. }));
    for (i, q) in props.into_iter().enumerate() {
        match c["on"].as_bool() {
            Some(false) => q.modifiers.retain(|m| !mine(m)),
            Some(true) if !q.modifiers.iter().any(mine) => q.modifiers.push(Modifier::new(if wiggle {
                ModifierKind::Wiggle { frequency: 2.0, amplitude: 8.0, seed: seed + i as i32, octaves: 2 }
            } else {
                ModifierKind::Sine { frequency: 0.5, amplitude: 10.0, phase: 0.0 }
            })),
            _ => {}
        }
        for m in q.modifiers.iter_mut().filter(|m| mine(m)) {
            if let Some(v) = c["shown"].as_bool() {
                m.enabled = v;
            }
            match &mut m.kind {
                ModifierKind::Wiggle { frequency, amplitude, .. } | ModifierKind::Sine { frequency, amplitude, .. } => {
                    if let Some(v) = c["amount"].as_f64() {
                        *amplitude = v.clamp(0.0, 1000.0) as f32;
                    }
                    if let Some(v) = c["speed"].as_f64() {
                        *frequency = v.clamp(0.05, 30.0) as f32;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}


/// K2: a plugin effect on a layer: added with its defaults (effect node
/// `plugin.<id>`, type `plugin:<id>`; a point is two values, a choice its
/// index), changed like the built-in filters (keyed or static, D5), or
/// removed. `c`: {"id", "props"?: {name: value}, "enabled"?, "remove"?}.
pub fn apply_plugin(st: &mut State, sym: &str, layer: &str, c: &J, t: f64, keyed: bool) -> EditResult<()> {
    use barkfart_script::PropKind as K;
    let Some(pid) = c["id"].as_str() else { return fail("which plugin?") };
    let node = format!("plugin.{pid}");
    let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
    if c["remove"].as_bool() == Some(true) {
        let ids: Vec<String> = l.effects.nodes.keys().filter(|k| **k == node).cloned().collect();
        for k in ids {
            l.effects.nodes.shift_remove(&k);
        }
        // the chain runs through what's left
        reorder(l);
        return Ok(());
    }
    if !l.effects.nodes.contains_key(&node) {
        let Some(def) = crate::plugins::definition(pid) else { return fail("That plugin isn't installed.") };
        if def.kind != barkfart_script::Kind::Effect {
            return fail(format!("{} isn't an effect.", def.name));
        }
        let owner = l.id.clone();
        let mut e = EffectNode::new(&owner, &node, &format!("plugin:{pid}"));
        for p in &def.props {
            match &p.kind {
                K::Number { default, .. } | K::Angle { default } => {
                    e.param(&owner, &p.name, ValueType::Float, Value::F(*default as f32));
                }
                K::Toggle { default } => {
                    e.param(&owner, &p.name, ValueType::Bool, Value::B(*default));
                }
                K::Colour { default } => {
                    e.param(&owner, &p.name, ValueType::Color, Value::C(Color::new(default[0], default[1], default[2], default[3])));
                }
                K::Choice { default, .. } => {
                    e.param(&owner, &p.name, ValueType::Float, Value::F(*default as f32));
                }
                K::Point { default } => {
                    e.param(&owner, &format!("{}.x", p.name), ValueType::Float, Value::F(default.0 as f32));
                    e.param(&owner, &format!("{}.y", p.name), ValueType::Float, Value::F(default.1 as f32));
                }
                K::Text { .. } => {}
            }
        }
        l.effects.append(e).map_err(super::EditError)?;
        reorder(l);
    }
    let Some(e) = l.effects.nodes.get_mut(&node) else { return fail("no such effect") };
    if let Some(on) = c["enabled"].as_bool() {
        e.enabled = on;
    }
    if let Some(J::Object(set)) = c.get("props") {
        STATIC.with(|x| x.set(!keyed));
        for (k, v) in set {
            let Some(q) = e.params.get_mut(k.as_str()) else { continue };
            let value = match (q.ty, v) {
                (ValueType::Bool, J::Bool(b)) => Value::B(*b),
                (ValueType::Color, J::Number(n)) => Value::C(Color::rgba8888(n.as_u64().unwrap_or(0xffffffff) as u32)),
                (_, J::Number(n)) => Value::F(n.as_f64().unwrap_or(0.0) as f32),
                _ => continue,
            };
            key_at(q, t, value);
        }
        STATIC.with(|x| x.set(false));
    }
    Ok(())
}

/// K2: a layer's plugin effects for the app: each with its settings (from
/// the plugin's definition; missing or switched off says so).
pub fn plugin_state(p: &Project, l: &Layer, t: f64) -> J {
    use barkfart_script::PropKind as K;
    let mut r = Resolver::new(p);
    let list: Vec<J> = l
        .effects
        .nodes
        .values()
        .filter_map(|e| {
            let pid = e.ty.strip_prefix("plugin:")?;
            let def = crate::plugins::definition(pid);
            let off = crate::plugins::list().into_iter().find(|x| x.def.id == pid).and_then(|x| x.off);
            let get = |r: &mut Resolver, n: &str| e.params.get(n).map(|q| r.resolve(q, t));
            let settings: Vec<J> = def
                .as_ref()
                .map(|d| {
                    d.props
                        .iter()
                        .filter_map(|pd| {
                            let (kind, value, extra) = match &pd.kind {
                                K::Number { min, max, step, .. } => ("number", serde_json::json!(get(&mut r, &pd.name)?.f()), serde_json::json!({"min": min, "max": max, "step": step})),
                                K::Angle { .. } => ("angle", serde_json::json!(get(&mut r, &pd.name)?.f()), serde_json::json!({})),
                                K::Toggle { .. } => ("toggle", serde_json::json!(matches!(get(&mut r, &pd.name)?, Value::B(true))), serde_json::json!({})),
                                K::Colour { .. } => ("colour", serde_json::json!(get(&mut r, &pd.name).map(|v| match v { Value::C(c) => c.to_rgba8888(), _ => 0xffffffff })?), serde_json::json!({})),
                                K::Choice { options, .. } => {
                                    let i = get(&mut r, &pd.name)?.f().round().max(0.0) as usize;
                                    ("choice", serde_json::json!(options.get(i).cloned().unwrap_or_default()), serde_json::json!({"options": options}))
                                }
                                K::Point { .. } => ("point", serde_json::json!([get(&mut r, &format!("{}.x", pd.name))?.f(), get(&mut r, &format!("{}.y", pd.name))?.f()]), serde_json::json!({})),
                                K::Text { .. } => return None,
                            };
                            Some(serde_json::json!({"name": pd.name, "label": pd.label.clone().unwrap_or_else(|| pd.name.clone()), "kind": kind, "value": value, "extra": extra}))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(serde_json::json!({"id": pid, "name": def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| pid.to_string()), "version": def.as_ref().map(|d| d.version.clone()),
                "author": def.as_ref().and_then(|d| d.author.clone()), "missing": def.is_none(), "off": off, "enabled": e.enabled, "settings": settings}))
        })
        .collect();
    serde_json::json!(list)
}

// ---------------------------------------------------------------- Round 3: stacked & keyable effects

/// Where a stacked effect operation targets: a layer or a layer group.
#[derive(Clone, Copy, Debug)]
pub enum StackTarget<'a> {
    Layer(&'a str),
    Group(&'a str),
}

fn param_kind_name(k: crate::fx::ParamKind) -> &'static str {
    match k {
        crate::fx::ParamKind::Float => "float",
        crate::fx::ParamKind::Angle => "angle",
        crate::fx::ParamKind::Color => "color",
    }
}

/// Full built-in effect catalog from `barkfart_core::fx::catalog()`, serialized for the UI.
pub fn catalog_json() -> J {
    let list: Vec<J> = crate::fx::catalog()
        .iter()
        .map(|m| {
            let params: Vec<J> = m
                .params
                .iter()
                .map(|pm| {
                    let def = match pm.kind {
                        crate::fx::ParamKind::Color => serde_json::json!(pm.default_c.to_rgba8888()),
                        _ => serde_json::json!(round2(pm.default_f)),
                    };
                    serde_json::json!({
                        "name": pm.name,
                        "label": pm.label,
                        "kind": param_kind_name(pm.kind),
                        "default": def,
                        "min": pm.min,
                        "max": pm.max,
                        "step": pm.step,
                    })
                })
                .collect();
            serde_json::json!({
                "type": m.ty,
                "label": m.label,
                "category": m.category,
                "params": params,
            })
        })
        .collect();
    serde_json::json!(list)
}

fn prop_key_info(r: &mut Resolver, q: &Property, t: f64, fps: f64, frame: i32) -> (J, bool, bool, Vec<i32>) {
    let val = match r.resolve(q, t) {
        Value::F(x) => serde_json::json!(round2(x)),
        Value::D(x) => serde_json::json!(round2(x as f32)),
        Value::C(c) => serde_json::json!(c.to_rgba8888()),
        Value::B(b) => serde_json::json!(b),
    };
    let keys: Vec<i32> = q.track.keys().iter().map(|k| (k.time * fps).round() as i32).collect();
    let keyed = keys.contains(&frame);
    let animated = !keys.is_empty();
    (val, keyed, animated, keys)
}

/// Serializes an `EffectStack` for the Round 3 stacked effects UI (`effectStack`).
/// Preserves exact chain execution order; hides silent legacy `filter.*` placeholders
/// while showing all explicitly added `FX*` / plugin / active effects.
pub fn stack_state(p: &Project, effects: &EffectStack, t: f64, fps: f64, frame: i32) -> J {
    let mut r = Resolver::new(p);
    let mut out: Vec<J> = Vec::new();
    for e in effects.chain() {
        if e.id.starts_with("filter.") && !node_active(e) {
            continue;
        }
        let (mix_val, mix_keyed, mix_anim, mix_keys) = prop_key_info(&mut r, &e.mix, t, fps, frame);
        let mix_obj = serde_json::json!({
            "propId": e.mix.id,
            "value": mix_val,
            "default": 1.0,
            "min": 0.0,
            "max": 1.0,
            "step": 0.05,
            "keyed": mix_keyed,
            "animated": mix_anim,
            "keys": mix_keys,
        });
        if let Some(meta) = crate::fx::meta(&e.ty) {
            let mut params: Vec<J> = Vec::with_capacity(meta.params.len());
            for pm in meta.params {
                let def_json = match pm.kind {
                    crate::fx::ParamKind::Color => serde_json::json!(pm.default_c.to_rgba8888()),
                    _ => serde_json::json!(round2(pm.default_f)),
                };
                if let Some(q) = e.params.get(pm.name) {
                    let (val, keyed, animated, keys) = prop_key_info(&mut r, q, t, fps, frame);
                    params.push(serde_json::json!({
                        "name": pm.name,
                        "label": pm.label,
                        "kind": param_kind_name(pm.kind),
                        "propId": q.id,
                        "value": val,
                        "default": def_json,
                        "min": pm.min,
                        "max": pm.max,
                        "step": pm.step,
                        "keyed": keyed,
                        "animated": animated,
                        "keys": keys,
                    }));
                } else {
                    // Legacy node that omitted an optional param (e.g. brightness on color.adjust)
                    params.push(serde_json::json!({
                        "name": pm.name,
                        "label": pm.label,
                        "kind": param_kind_name(pm.kind),
                        "propId": format!("{}/{}/{}", e.mix.id.split('/').next().unwrap_or(""), e.id, pm.name),
                        "value": def_json,
                        "default": def_json,
                        "min": pm.min,
                        "max": pm.max,
                        "step": pm.step,
                        "keyed": false,
                        "animated": false,
                        "keys": [],
                    }));
                }
            }
            out.push(serde_json::json!({
                "id": e.id,
                "type": e.ty,
                "label": meta.label,
                "category": meta.category,
                "enabled": e.enabled,
                "mix": mix_obj,
                "params": params,
            }));
        } else if let Some(pid) = e.ty.strip_prefix("plugin:") {
            use barkfart_script::PropKind as K;
            let def = crate::plugins::definition(pid);
            let label = def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| pid.to_string());
            let mut params: Vec<J> = Vec::new();
            if let Some(d) = &def {
                for pd in &d.props {
                    let p_label = pd.label.clone().unwrap_or_else(|| pd.name.clone());
                    match &pd.kind {
                        K::Number { default, min, max, step } => {
                            if let Some(q) = e.params.get(&pd.name) {
                                let (val, keyed, animated, keys) = prop_key_info(&mut r, q, t, fps, frame);
                                params.push(serde_json::json!({"name": pd.name, "label": p_label, "kind": "float", "propId": q.id, "value": val, "default": default, "min": min, "max": max, "step": step, "keyed": keyed, "animated": animated, "keys": keys}));
                            }
                        }
                        K::Angle { default } => {
                            if let Some(q) = e.params.get(&pd.name) {
                                let (val, keyed, animated, keys) = prop_key_info(&mut r, q, t, fps, frame);
                                params.push(serde_json::json!({"name": pd.name, "label": p_label, "kind": "angle", "propId": q.id, "value": val, "default": default, "min": -3600.0, "max": 3600.0, "step": 5.0, "keyed": keyed, "animated": animated, "keys": keys}));
                            }
                        }
                        K::Toggle { default } => {
                            if let Some(q) = e.params.get(&pd.name) {
                                let (val, keyed, animated, keys) = prop_key_info(&mut r, q, t, fps, frame);
                                params.push(serde_json::json!({"name": pd.name, "label": p_label, "kind": "bool", "propId": q.id, "value": val, "default": default, "min": 0.0, "max": 1.0, "step": 1.0, "keyed": keyed, "animated": animated, "keys": keys}));
                            }
                        }
                        K::Colour { default } => {
                            if let Some(q) = e.params.get(&pd.name) {
                                let (val, keyed, animated, keys) = prop_key_info(&mut r, q, t, fps, frame);
                                let dc = Color::new(default[0], default[1], default[2], default[3]).to_rgba8888();
                                params.push(serde_json::json!({"name": pd.name, "label": p_label, "kind": "color", "propId": q.id, "value": val, "default": dc, "min": 0.0, "max": 1.0, "step": 0.0, "keyed": keyed, "animated": animated, "keys": keys}));
                            }
                        }
                        _ => {}
                    }
                }
            }
            out.push(serde_json::json!({
                "id": e.id,
                "type": e.ty,
                "label": label,
                "category": "Plugins",
                "enabled": e.enabled,
                "missing": def.is_none(),
                "mix": mix_obj,
                "params": params,
            }));
        }
    }
    serde_json::json!(out)
}

fn with_stack_mut<T>(st: &mut State, sym: &str, target: StackTarget<'_>, f: impl FnOnce(&mut EffectStack, &str, bool) -> EditResult<T>) -> EditResult<T> {
    match target {
        StackTarget::Layer(id) => {
            let Some(l) = st.project.layer_mut(sym, id) else { return fail("no layer") };
            let owner = l.id.clone();
            f(&mut l.effects, &owner, false)
        }
        StackTarget::Group(gid) => {
            let Some(s) = st.project.symbol_mut(sym) else { return fail("no symbol") };
            let Some(g) = s.groups.iter_mut().find(|g| g.id == gid) else { return fail("no group") };
            let owner = g.id.clone();
            f(&mut g.effects, &owner, true)
        }
    }
}

fn build_effect_node(owner: &str, fx_id: &str, ty: &str) -> EditResult<EffectNode> {
    let mut e = EffectNode::new(owner, fx_id, ty);
    if let Some(meta) = crate::fx::meta(ty) {
        for pm in meta.params {
            match pm.kind {
                crate::fx::ParamKind::Color => {
                    e.param(owner, pm.name, ValueType::Color, Value::C(pm.default_c));
                }
                crate::fx::ParamKind::Float | crate::fx::ParamKind::Angle => {
                    e.param(owner, pm.name, ValueType::Float, Value::F(pm.default_f));
                }
            }
        }
        return Ok(e);
    }
    if let Some(pid) = ty.strip_prefix("plugin:") {
        use barkfart_script::PropKind as K;
        let Some(def) = crate::plugins::definition(pid) else { return fail("That plugin isn't installed.") };
        if def.kind != barkfart_script::Kind::Effect {
            return fail(format!("{} isn't an effect.", def.name));
        }
        for p in &def.props {
            match &p.kind {
                K::Number { default, .. } | K::Angle { default } => {
                    e.param(owner, &p.name, ValueType::Float, Value::F(*default as f32));
                }
                K::Toggle { default } => {
                    e.param(owner, &p.name, ValueType::Bool, Value::B(*default));
                }
                K::Colour { default } => {
                    e.param(owner, &p.name, ValueType::Color, Value::C(Color::new(default[0], default[1], default[2], default[3])));
                }
                K::Choice { default, .. } => {
                    e.param(owner, &p.name, ValueType::Float, Value::F(*default as f32));
                }
                K::Point { default } => {
                    e.param(owner, &format!("{}.x", p.name), ValueType::Float, Value::F(default.0 as f32));
                    e.param(owner, &format!("{}.y", p.name), ValueType::Float, Value::F(default.1 as f32));
                }
                K::Text { .. } => {}
            }
        }
        return Ok(e);
    }
    fail(format!("unknown effect type {ty}"))
}

fn parse_fx_value(ty: &str, name: &str, q_ty: ValueType, v: &J) -> Option<Value> {
    match q_ty {
        ValueType::Bool => v.as_bool().map(Value::B),
        ValueType::Color => v.as_u64().map(|u| Value::C(Color::rgba8888(u as u32))),
        ValueType::Float | ValueType::Angle => {
            let x = v.as_f64()? as f32;
            if !x.is_finite() {
                return None;
            }
            let clamped = if let Some(pm) = crate::fx::meta(ty).and_then(|m| m.params.iter().find(|p| p.name == name)) {
                x.clamp(pm.min, pm.max)
            } else {
                x
            };
            Some(Value::F(clamped))
        }
        ValueType::Double => v.as_f64().filter(|x| x.is_finite()).map(Value::D),
    }
}

fn set_fx_prop(q: &mut Property, v: Value, t: f64, keyed: bool, is_group: bool, tw: Interp) {
    if is_group {
        q.track.clear();
        q.value = v;
        return;
    }
    if !q.track.is_empty() || keyed {
        let interp = if q.ty.continuous() { crate::edit::editor::interp_at(q, t, tw) } else { Interp::Hold };
        q.track.put(Key::new(t, v, interp));
    } else {
        q.value = v;
    }
}

/// Adds a new stacked effect node (`FX*`) to a layer or group (duplicates allowed!).
pub fn fx_add(st: &mut State, sym: &str, target: StackTarget<'_>, ty: &str, mix: Option<f32>, params: Option<&serde_json::Map<String, J>>) -> EditResult<String> {
    let fx_id = st.project.new_id("FX");
    let fx_id2 = fx_id.clone();
    with_stack_mut(st, sym, target, |stack, owner, _| {
        let mut e = build_effect_node(owner, &fx_id2, ty)?;
        if let Some(m) = mix {
            if m.is_finite() {
                e.mix.value = Value::F(m.clamp(0.0, 1.0));
            }
        }
        if let Some(map) = params {
            for (k, v) in map {
                if let Some(q) = e.params.get_mut(k.as_str()) {
                    if let Some(val) = parse_fx_value(ty, k, q.ty, v) {
                        q.value = val;
                    }
                }
            }
        }
        stack.append(e).map_err(super::EditError)?;
        Ok(())
    })?;
    Ok(fx_id)
}

/// Updates enabled, mix, and/or parameters on a stacked effect node (keyed or static).
pub fn fx_set(st: &mut State, sym: &str, target: StackTarget<'_>, fx_id: &str, c: &J, t: f64, keyed: bool) -> EditResult<()> {
    let tw = st.project.settings.tween_interp;
    with_stack_mut(st, sym, target, |stack, owner, is_group| {
        let Some(e) = stack.nodes.get_mut(fx_id) else { return fail("no such effect") };
        let ty = e.ty.clone();
        if let Some(en) = c["enabled"].as_bool() {
            e.enabled = en;
        }
        if let Some(m) = c["mix"].as_f64() {
            if m.is_finite() {
                set_fx_prop(&mut e.mix, Value::F(m.clamp(0.0, 1.0) as f32), t, keyed, is_group, tw);
            }
        }
        if let Some(map) = c["params"].as_object() {
            for (k, v) in map {
                // Ensure param exists if it's defined in fx::meta (e.g. brightness on legacy color.adjust)
                if !e.params.contains_key(k.as_str()) {
                    if let Some(pm) = crate::fx::meta(&ty).and_then(|m| m.params.iter().find(|p| p.name == k.as_str())) {
                        match pm.kind {
                            crate::fx::ParamKind::Color => {
                                e.param(owner, pm.name, ValueType::Color, Value::C(pm.default_c));
                            }
                            _ => {
                                e.param(owner, pm.name, ValueType::Float, Value::F(pm.default_f));
                            }
                        }
                    }
                }
                if let Some(q) = e.params.get_mut(k.as_str()) {
                    if let Some(val) = parse_fx_value(&ty, k, q.ty, v) {
                        set_fx_prop(q, val, t, keyed, is_group, tw);
                    }
                }
            }
        }
        Ok(())
    })
}

/// Toggles or sets a keyframe at `frame` for an effect parameter (or `mix`, or any `prop_id`).
/// Returns `(property_id, now_keyed)`.
pub fn fx_key(
    st: &mut State,
    sym: &str,
    layer: &str,
    fx_id: Option<&str>,
    param: Option<&str>,
    prop_id: Option<&str>,
    frame: i32,
    on: Option<bool>,
) -> EditResult<(String, bool)> {
    let fps = fps_of(st);
    let t = frame as f64 / fps;
    let tw = st.project.settings.tween_interp;
    if let Some(pid) = prop_id {
        let Some(q) = st.project.property_mut(pid) else { return fail("no such property") };
        return toggle_prop_key(q, frame, t, fps, tw, on);
    }
    let (Some(fid), Some(pname)) = (fx_id, param) else { return fail("which effect parameter?") };
    let Some(l) = st.project.layer_mut(sym, layer) else { return fail("no layer") };
    let owner = l.id.clone();
    let Some(e) = l.effects.nodes.get_mut(fid) else { return fail("no such effect") };
    let ty = e.ty.clone();
    if pname == "mix" {
        return toggle_prop_key(&mut e.mix, frame, t, fps, tw, on);
    }
    if !e.params.contains_key(pname) {
        if let Some(pm) = crate::fx::meta(&ty).and_then(|m| m.params.iter().find(|p| p.name == pname)) {
            match pm.kind {
                crate::fx::ParamKind::Color => {
                    e.param(&owner, pm.name, ValueType::Color, Value::C(pm.default_c));
                }
                _ => {
                    e.param(&owner, pm.name, ValueType::Float, Value::F(pm.default_f));
                }
            }
        }
    }
    let Some(q) = e.params.get_mut(pname) else { return fail(format!("no param {pname}")) };
    toggle_prop_key(q, frame, t, fps, tw, on)
}

fn toggle_prop_key(q: &mut Property, frame: i32, t: f64, fps: f64, tw: Interp, on: Option<bool>) -> EditResult<(String, bool)> {
    let idx = q.track.keys().iter().position(|k| (k.time * fps).round() as i32 == frame);
    let want = on.unwrap_or(idx.is_none());
    if want {
        if idx.is_none() {
            let v = q.key_value(t);
            let interp = if q.ty.continuous() { crate::edit::editor::interp_at(q, t, tw) } else { Interp::Hold };
            q.track.put(Key::new(t, v, interp));
        }
        Ok((q.id.clone(), true))
    } else {
        if let Some(i) = idx {
            let removed = q.track.keys()[i];
            q.track.remove_at(i);
            if q.track.is_empty() {
                q.value = removed.value;
            }
        }
        Ok((q.id.clone(), false))
    }
}

/// Reorders an effect node in a layer or group's `EffectStack` chain.
pub fn fx_reorder(st: &mut State, sym: &str, target: StackTarget<'_>, fx_id: &str, to: Option<usize>, dir: Option<i32>) -> EditResult<()> {
    with_stack_mut(st, sym, target, |stack, _, _| {
        let mut chain: Vec<String> = stack.chain().into_iter().map(|e| e.id.clone()).collect();
        let Some(pos) = chain.iter().position(|id| id == fx_id) else { return fail("no such effect") };
        let len = chain.len();
        if len <= 1 {
            return Ok(());
        }
        let target_idx = match to {
            Some(i) => i.min(len - 1),
            None => (pos as i32 + dir.unwrap_or(0)).clamp(0, len as i32 - 1) as usize,
        };
        if target_idx == pos {
            return Ok(());
        }
        let id = chain.remove(pos);
        chain.insert(target_idx, id);
        rewire_chain(stack, &chain);
        Ok(())
    })
}

/// Duplicates an effect node (including all tracks, eases, handles & modifiers)
/// right after the original in the chain. Returns the new effect node id.
pub fn fx_duplicate(st: &mut State, sym: &str, target: StackTarget<'_>, fx_id: &str) -> EditResult<String> {
    let new_id = st.project.new_id("FX");
    let new_id2 = new_id.clone();
    with_stack_mut(st, sym, target, |stack, owner, _| {
        let Some(src) = stack.nodes.get(fx_id).cloned() else { return fail("no such effect") };
        let mut chain: Vec<String> = stack.chain().into_iter().map(|e| e.id.clone()).collect();
        let pos = chain.iter().position(|id| id == fx_id).unwrap_or(chain.len().saturating_sub(1));
        let mut dup = src;
        dup.id = new_id2.clone();
        dup.mix.id = format!("{owner}/{new_id2}/mix");
        for (name, p) in dup.params.iter_mut() {
            p.id = format!("{owner}/{new_id2}/{name}");
        }
        stack.nodes.insert(new_id2.clone(), dup);
        chain.insert((pos + 1).min(chain.len()), new_id2);
        rewire_chain(stack, &chain);
        Ok(())
    })?;
    Ok(new_id)
}

/// Removes an effect node from a layer or group's `EffectStack` and rewires the chain.
pub fn fx_remove(st: &mut State, sym: &str, target: StackTarget<'_>, fx_id: &str) -> EditResult<()> {
    with_stack_mut(st, sym, target, |stack, _, _| {
        let Some(removed) = stack.nodes.shift_remove(fx_id) else { return fail("no such effect") };
        for n in stack.nodes.values_mut() {
            if n.inputs.iter().any(|x| x == fx_id) {
                let mut ins = Vec::new();
                for x in &n.inputs {
                    if x == fx_id {
                        ins.extend(removed.inputs.iter().cloned());
                    } else {
                        ins.push(x.clone());
                    }
                }
                n.inputs = ins;
            }
        }
        if stack.output.as_deref() == Some(fx_id) {
            stack.output = removed.inputs.first().cloned();
        }
        Ok(())
    })
}

/// Applies an array of stacked effects (`[{"type", "mix"?, "params"?}, ...]`) in one step.
pub fn fx_preset(st: &mut State, sym: &str, target: StackTarget<'_>, effects: &[J], replace: bool) -> EditResult<Vec<String>> {
    if replace {
        with_stack_mut(st, sym, target, |stack, _, _| {
            stack.nodes.clear();
            stack.output = None;
            Ok(())
        })?;
    }
    let mut added = Vec::with_capacity(effects.len());
    for item in effects {
        let Some(ty) = item["type"].as_str() else { continue };
        let mix = item["mix"].as_f64().map(|v| v as f32);
        let params = item["params"].as_object();
        let id = fx_add(st, sym, target, ty, mix, params)?;
        added.push(id);
    }
    Ok(added)
}
