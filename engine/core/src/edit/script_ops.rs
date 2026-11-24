//! Script layers (K2, SCRIPTING.md "Script layers"): put one in, pick what it
//! moves, change its settings, its runs (the blocks on its row), and Bake.

use super::{fail, EditResult, State};
use crate::eval::scripts::{drivable, layer_out};
use crate::model::*;
use barkfart_script::{PropValue, TargetKind};
use std::sync::Arc;

fn fps_of(st: &State) -> f64 {
    if st.project.settings.fps > 0.0 {
        st.project.settings.fps as f64
    } else {
        30.0
    }
}

/// A run's own random numbers, from its ids: a copied block has a new id,
/// so a new seed.
fn seed_of(layer: &str, run: &str, salt: u32) -> u32 {
    let mut h: u32 = 0x811c9dc5 ^ salt;
    for b in layer.bytes().chain([0]).chain(run.bytes()) {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h & 0x7fff_ffff
}

fn script<'a>(st: &'a mut State, sym: &str, layer: &str) -> EditResult<&'a mut ScriptLayer> {
    match st.project.layer_mut(sym, layer).map(|l| &mut l.kind) {
        Some(LayerKind::Script(sc)) => Ok(sc),
        _ => fail("That isn't a script layer."),
    }
}

/// "Add a script": a Script layer at the front, running plugin `plugin` from
/// frame `from` up to (not including) `to`. Its figure targets start on
/// `figure`. Returns the new layer's id.
pub fn add(st: &mut State, sym: &str, plugin: &str, figure: Option<&str>, from: i64, to: i64) -> EditResult<String> {
    let Some(def) = crate::plugins::definition(plugin) else { return fail("That plugin isn't installed.") };
    if def.kind != barkfart_script::Kind::Script {
        return fail(format!("{} isn't a script.", def.name));
    }
    let fps = fps_of(st);
    let id = st.project.new_id("L");
    let rid = st.project.new_id("R");
    let figure = figure.filter(|f| st.project.symbol(sym).and_then(|s| s.layer(f)).is_some_and(|l| l.as_figure().is_some()));
    let targets = def.targets.iter().map(|t| ScriptTarget { name: t.name.clone(), kind: t.kind, to: if t.kind == TargetKind::Figure { figure.map(str::to_string) } else { None } }).collect();
    let props = crate::plugins::fill_props(&def, &[]);
    let (from, to) = (from.max(0), to.max(from.max(0) + 1));
    let run = ScriptRun::new(&id, &rid, from as f64 / fps, to as f64 / fps, seed_of(&id, &rid, 0));
    let sc = ScriptLayer { plugin: def.id.clone(), version: def.version.clone(), props, targets, runs: vec![run] };
    let Some(s) = st.project.symbol_mut(sym) else { return fail("no symbol") };
    s.layers.push(Arc::new(Layer::script(&id, Some(&def.name), sc)));
    Ok(id)
}

/// The script's settings (`props`, by name) and targets (`target`: its name
/// and what it's set to, null to clear).
pub fn set(st: &mut State, sym: &str, layer: &str, c: &serde_json::Value) -> EditResult<()> {
    // a target must be the right kind of thing
    let target = match c.get("target") {
        Some(t) => {
            let Some(name) = t["name"].as_str() else { return fail("which target?") };
            let to = t["to"].as_str().map(str::to_string);
            Some((name.to_string(), to))
        }
        None => None,
    };
    if let Some((name, Some(to))) = &target {
        let kind = match st.project.symbol(sym).and_then(|s| s.layer(layer)).map(|l| &l.kind) {
            Some(LayerKind::Script(sc)) => sc.targets.iter().find(|x| &x.name == name).map(|x| x.kind),
            _ => None,
        };
        match kind {
            None => return fail("The script has no such target."),
            Some(TargetKind::Figure) => {
                if !st.project.symbol(sym).and_then(|s| s.layer(to)).is_some_and(|l| l.as_figure().is_some()) {
                    return fail("Pick a stickfigure for this.");
                }
            }
            Some(TargetKind::Value) => {
                if !st.project.property(to).is_some_and(drivable) {
                    return fail("A script changes numbers; pick a value that is one.");
                }
            }
        }
    }
    let sc = script(st, sym, layer)?;
    if let Some((name, to)) = target {
        if let Some(t) = sc.targets.iter_mut().find(|x| x.name == name) {
            t.to = to;
        }
    }
    if let Some(set) = c.get("props") {
        merge_props(&mut sc.props, set);
    }
    Ok(())
}

/// Plugin settings from the app (`{name: value}`) into a plugin's list.
pub fn merge_props(props: &mut Vec<(String, PropValue)>, set: &serde_json::Value) {
    use serde_json::Value as J;
    let J::Object(set) = set else { return };
    for (k, v) in set {
        let pv = match v {
            J::Bool(b) => PropValue::Toggle(*b),
            J::String(t) => PropValue::Text(t.clone()),
            J::Number(n) => PropValue::Number(n.as_f64().unwrap_or(0.0)),
            J::Array(a) if a.len() == 4 => PropValue::Colour([0, 1, 2, 3].map(|i| a[i].as_f64().unwrap_or(1.0) as f32)),
            J::Array(a) if a.len() == 2 => PropValue::Point(a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0)),
            _ => continue,
        };
        match props.iter_mut().find(|(n, _)| n == k) {
            Some(slot) => slot.1 = pv,
            None => props.push((k.clone(), pv)),
        }
    }
}

/// One run's settings: `from` / `to` (frames; `to` is where it stops),
/// `replace`, `keep`, `mix` (0..1, at the run's start), `newSeed`. Runs stay
/// in order and never overlap: a block stops where the next one starts.
pub fn run_set(st: &mut State, sym: &str, layer: &str, run: &str, c: &serde_json::Value) -> EditResult<()> {
    let fps = fps_of(st);
    let sc = script(st, sym, layer)?;
    let Some(i) = sc.runs.iter().position(|r| r.id == run) else { return fail("There is no such run.") };
    let lo = if i == 0 { 0 } else { (sc.runs[i - 1].end * fps).round() as i64 };
    let hi = sc.runs.get(i + 1).map(|r| (r.start * fps).round() as i64);
    let r = &mut sc.runs[i];
    let (mut a, mut b) = ((r.start * fps).round() as i64, (r.end * fps).round() as i64);
    match (c["from"].as_f64(), c["to"].as_f64()) {
        // a move keeps the length
        (Some(f), None) if c["move"].as_bool() == Some(true) => {
            let len = b - a;
            a = (f.round() as i64).max(lo);
            if let Some(h) = hi {
                a = a.min(h - len);
            }
            a = a.max(lo);
            b = a + len;
            if let Some(h) = hi {
                b = b.min(h);
            }
        }
        (f, t) => {
            if let Some(f) = f {
                a = (f.round() as i64).clamp(lo, b - 1);
            }
            if let Some(t) = t {
                b = (t.round() as i64).max(a + 1);
                if let Some(h) = hi {
                    b = b.min(h);
                }
            }
        }
    }
    if b <= a {
        return fail("A run needs at least one frame.");
    }
    r.start = a as f64 / fps;
    r.end = b as f64 / fps;
    if let Some(v) = c["replace"].as_bool() {
        r.replace = v;
    }
    if let Some(v) = c["keep"].as_bool() {
        r.keep = v;
    }
    if let Some(m) = c["mix"].as_f64() {
        let m = m.clamp(0.0, 1.0) as f32;
        if r.mix.track.is_empty() {
            r.mix.value = Value::F(m);
        } else {
            let t = r.start;
            r.mix.track.put(Key::new(t, Value::F(m), Interp::Linear));
        }
    }
    if c["newSeed"].as_bool() == Some(true) {
        r.seed = seed_of(layer, &r.id, r.seed.wrapping_add(1));
    }
    Ok(())
}

/// Another run on the row at `frame`, `len` frames long (shorter if the
/// next run starts sooner). Returns its id.
pub fn run_add(st: &mut State, sym: &str, layer: &str, frame: i64, len: i64) -> EditResult<String> {
    let fps = fps_of(st);
    let rid = st.project.new_id("R");
    let sc = script(st, sym, layer)?;
    let a = frame.max(0);
    if sc.runs.iter().any(|r| a >= (r.start * fps).round() as i64 && a < (r.end * fps).round() as i64) {
        return fail("There's a run on that frame already.");
    }
    let next = sc.runs.iter().map(|r| (r.start * fps).round() as i64).filter(|s| *s > a).min();
    let b = next.map_or(a + len.max(1), |n| n.min(a + len.max(1)));
    let run = ScriptRun::new(layer, &rid, a as f64 / fps, b as f64 / fps, seed_of(layer, &rid, 0));
    sc.runs.push(run);
    sc.runs.sort_by(|x, y| x.start.total_cmp(&y.start));
    Ok(rid)
}

pub fn run_remove(st: &mut State, sym: &str, layer: &str, run: &str) -> EditResult<()> {
    let sc = script(st, sym, layer)?;
    let n = sc.runs.len();
    sc.runs.retain(|r| r.id != run);
    if sc.runs.len() == n {
        return fail("There is no such run.");
    }
    Ok(())
}

/// "Bake to keys": the run's result becomes ordinary keys, one on each of
/// its frames, on every value it changed; the run goes. With "Keeps the
/// last frame", the last frame's result is keyed where the block stops too.
pub fn bake(st: &mut State, sym: &str, layer: &str, run: &str) -> EditResult<()> {
    let fps = fps_of(st);
    let out = layer_out(&st.project, sym, layer);
    if out.missing {
        return fail("The plugin isn't installed, so there's nothing to bake.");
    }
    if let Some(e) = &out.error {
        return fail(e.clone());
    }
    let Some(ri) = out.runs.iter().position(|r| r.run == run) else { return fail("There is no such run.") };
    let ro = &out.runs[ri];
    // what each value is on each frame, this run included (others not)
    let mut keys: Vec<(String, f64, f64)> = Vec::new();
    {
        let mut r = crate::eval::Resolver::plain(&st.project);
        let mut ids: Vec<&String> = ro.values.keys().collect();
        ids.sort();
        let last = ro.first + ro.frames as i64;
        let frames: Vec<i64> = (ro.first..last).chain(ro.keep.then_some(last)).collect();
        for id in ids {
            let Some(q) = st.project.property(id) else { continue };
            for &k in &frames {
                let t = k as f64 / fps;
                let v = ValueType::to_scalar(&r.resolve(q, t));
                keys.push((id.clone(), t, out.apply_run(ri, id, v, k as f64)));
            }
        }
    }
    for (id, t, v) in keys {
        let Some(q) = st.project.property_mut(&id) else { continue };
        let val = q.ty.from_scalar(v, &q.value);
        q.track.put(Key::new(t, val, Interp::Linear));
    }
    run_remove(st, sym, layer, run)
}
