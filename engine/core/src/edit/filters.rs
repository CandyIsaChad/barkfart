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

/// The panel's order (top to bottom).
const ORDER: [&str; 9] = [INVERT_ID, ADJUST_ID, PIXELATE_ID, TINT_ID, OUTLINE_ID, BLUR_ID, MOTION_ID, GLOW_ID, SHADOW_ID];

fn fps_of(st: &State) -> f64 {
    if st.project.settings.fps > 0.0 {
        st.project.settings.fps as f64
    } else {
        30.0
    }
}

fn key_at(p: &mut Property, t: f64, v: Value) {
    let interp = if p.ty.continuous() { Interp::Linear } else { Interp::Hold };
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

/// Makes sure the filter's effect node exists, silent (mix 0) on every frame.
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
    for i in 0..n {
        e.mix.track.put(Key::new(i as f64 / fps, Value::F(0.0), Interp::Linear));
    }
    if l.effects.append(e).is_ok() {
        reorder(l);
    }
}

/// Filters run in the panel's order; anything else stays in front of them.
fn reorder(l: &mut Layer) {
    let ours: std::collections::HashSet<&str> = ORDER.into_iter().collect();
    let pre = l
        .effects
        .nodes
        .values()
        .filter(|e| ours.contains(e.id.as_str()))
        .filter_map(|e| e.inputs.first().cloned())
        .find(|x| !ours.contains(x.as_str()));
    let mut prev = pre;
    for id in ORDER {
        if let Some(e) = l.effects.nodes.get_mut(id) {
            e.inputs = prev.iter().cloned().collect();
            prev = Some(id.to_string());
        }
    }
    l.effects.output = prev;
}

fn set_mix(l: &mut Layer, id: &str, t: f64, v: f32) {
    if let Some(e) = l.effects.nodes.get_mut(id) {
        key_at(&mut e.mix, t, Value::F(v.clamp(0.0, 1.0)));
    }
}

fn set_param(l: &mut Layer, id: &str, name: &str, ty: ValueType, t: f64, v: Value) {
    let owner = l.id.clone();
    let Some(e) = l.effects.nodes.get_mut(id) else { return };
    let p = match e.params.get_mut(name) {
        Some(p) => p,
        None => e.param(&owner, name, ty, v.clone()),
    };
    key_at(p, t, v);
}

/// Applies the controls present in the command ({"op":"filter", ...}).
pub fn apply(st: &mut State, sym: &str, id: &str, c: &J, t: f64) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
    if l.as_figure().is_none() {
        return fail("not a figure");
    }
    if let Some(v) = c["opacity"].as_f64() {
        key_every_frame(&mut l.opacity, n, fps);
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

/// Clear all filters of the figure: the filter nodes go and opacity is 1.
pub fn clear(st: &mut State, sym: &str, id: &str, t: f64) -> EditResult<()> {
    let (n, fps) = (super::frame_ops::frame_count(st), fps_of(st));
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
    if l.as_figure().is_none() {
        return fail("not a figure");
    }
    let ids: Vec<String> = ORDER.iter().filter(|x| l.effects.nodes.keys().any(|k| k.as_str() == **x)).map(|x| (*x).to_string()).collect();
    for x in ids {
        super::structure::remove_effect(st, sym, id, &x)?;
    }
    let Some(l) = st.project.layer_mut(sym, id) else { return fail("no figure") };
    key_every_frame(&mut l.opacity, n, fps);
    key_at(&mut l.opacity, t, Value::F(1.0));
    Ok(())
}

/// A copied filter set: the layer's opacity and its filter nodes at time t.
#[derive(Clone, Debug)]
pub struct Snapshot {
    opacity: f32,
    nodes: Vec<SnapNode>,
}

#[derive(Clone, Debug)]
struct SnapNode {
    id: String,
    ty: String,
    mix: f32,
    params: Vec<(String, ValueType, Value)>,
}

pub fn snapshot(p: &Project, sym: &str, id: &str, t: f64) -> Option<Snapshot> {
    let l = p.symbol(sym)?.layer(id)?;
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
    let n = |x: &str| l.effects.nodes.get(x);
    let (invert, adjust, pixel, tint, outline, blur, motion, glow, shadow) =
        (n(INVERT_ID), n(ADJUST_ID), n(PIXELATE_ID), n(TINT_ID), n(OUTLINE_ID), n(BLUR_ID), n(MOTION_ID), n(GLOW_ID), n(SHADOW_ID));
    let (dx, dy) = (num(&mut r, shadow, "dx", -7.07), num(&mut r, shadow, "dy", 7.07));
    let distance = (dx * dx + dy * dy).sqrt();
    serde_json::json!({
        "opacity": round2(r.f(&l.opacity, t)),
        "invert": mix(&mut r, invert, 0.0) > 0.5,
        "hue": round2(num(&mut r, adjust, "hue", 0.0)),
        "saturation": round2(num(&mut r, adjust, "saturation", 1.0)),
        "pixelate": round2(num(&mut r, pixel, "size", 1.0)),
        "tint": col(&mut r, tint, "color", 0xffffffff),
        "tintAmount": round2(num(&mut r, tint, "amount", 0.0)),
        "outline": col(&mut r, outline, "color", 0xff00ffff),
        "outlineAmount": round2(mix(&mut r, outline, 1.0)),
        "outlineThickness": round2(num(&mut r, outline, "width", 4.0)),
        "blur": round2(num(&mut r, blur, "radius", 0.0)),
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
