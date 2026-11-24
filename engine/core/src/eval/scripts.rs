//! Script layers at work (K2, SCRIPTING.md "Script layers").
//!
//! A script's output is a number on top of a value: a joint's turn, a
//! figure's position, an effect's strength, a plugin effect's setting. It is
//! applied where every value is worked out (the Resolver), after keys and
//! modifiers, so the stage, joins and the canvas all see the same thing.
//!
//! - A run steps once per frame at the project's frame rate, from its block's
//!   first frame, in order; between frames its output is blended.
//! - A script sees the keyed motion (keys and modifiers), never another
//!   script's output, so scripts never feed back into each other.
//! - Runs are worked out whole and kept per thread until anything in the
//!   project changes (or the installed plugins do); a re-run gives the same
//!   result, so the cache only saves time.

use super::{layer_chain, FigurePose, Resolver};
use crate::model::*;
use barkfart_script::{Moved, Seen, TargetKind};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// The project's values that scripts change, and which script layers do.
#[derive(Debug, Default)]
pub struct Driven {
    pub by_prop: HashMap<String, Vec<usize>>,
    /// (symbol id, layer id) of each script layer that changes something.
    pub layers: Vec<(String, String)>,
}

/// Which values the project's (shown) script layers change. None when there
/// are none: the Resolver then has nothing to do for scripts.
pub fn driven(project: &Project) -> Option<Arc<Driven>> {
    let mut d = Driven::default();
    for sym in project.library.values() {
        for l in &sym.layers {
            let LayerKind::Script(sc) = &l.kind else { continue };
            if !l.visible || sc.runs.is_empty() {
                continue;
            }
            let li = d.layers.len();
            let mut any = false;
            for tg in &sc.targets {
                let Some(to) = &tg.to else { continue };
                let ids: Vec<String> = match tg.kind {
                    TargetKind::Figure => match sym.layer(to) {
                        Some(fl) => match &fl.kind {
                            LayerKind::Figure(f) => f.nodes().iter().map(|n| n.rotation.id.clone()).chain([fl.transform.x.id.clone(), fl.transform.y.id.clone()]).collect(),
                            _ => Vec::new(),
                        },
                        None => Vec::new(),
                    },
                    TargetKind::Value => match project.property(to) {
                        Some(q) if drivable(q) => vec![q.id.clone()],
                        _ => Vec::new(),
                    },
                };
                for id in ids {
                    let v = d.by_prop.entry(id).or_default();
                    if !v.contains(&li) {
                        v.push(li);
                    }
                    any = true;
                }
            }
            if any {
                d.layers.push((sym.id.clone(), l.id.clone()));
            }
        }
    }
    (!d.layers.is_empty()).then(|| Arc::new(d))
}

/// Whether a script can change this value: numbers only (not on/off
/// switches, choices' text or colours).
pub fn drivable(q: &Property) -> bool {
    matches!(q.ty, ValueType::Float | ValueType::Angle | ValueType::Double)
}

/// One run worked out: each changed value's number on each of its frames
/// (NaN: the script left it alone on that frame), and the mix.
#[derive(Debug)]
pub struct RunOut {
    pub run: String,
    pub first: i64,
    pub frames: usize,
    pub replace: bool,
    pub keep: bool,
    pub mix: Vec<f64>,
    pub values: HashMap<String, Vec<f64>>,
}

/// A script layer worked out: its runs, or why it does nothing.
#[derive(Debug, Default)]
pub struct LayerOut {
    pub runs: Vec<RunOut>,
    /// The plugin isn't installed (its settings are kept).
    pub missing: bool,
    /// It failed and was switched off.
    pub error: Option<String>,
}

impl LayerOut {
    /// A value at project frame `fk` (fractional between frames), with this
    /// layer's runs applied to `v`.
    pub fn apply(&self, id: &str, v: f64, fk: f64) -> f64 {
        (0..self.runs.len()).fold(v, |v, ri| self.apply_run(ri, id, v, fk))
    }

    /// The same, for run `ri` alone.
    pub fn apply_run(&self, ri: usize, id: &str, v: f64, fk: f64) -> f64 {
        let eps = 1e-6;
        let run = &self.runs[ri];
        let Some(vals) = run.values.get(id) else { return v };
        let end = (run.first + run.frames as i64) as f64;
        let next = self.runs.get(ri + 1).map(|r| r.first as f64);
        let (x, m) = if fk >= run.first as f64 - eps && fk < end - eps {
            let u = (fk - run.first as f64).max(0.0);
            let i = (u.floor() as usize).min(run.frames - 1);
            let f = u - i as f64;
            (blend(vals[i], vals.get(i + 1).copied(), f), blend(run.mix[i], run.mix.get(i + 1).copied(), f))
        } else if run.keep && fk >= end - eps && next.is_none_or(|s| fk < s - eps) {
            match vals.iter().rposition(|x| !x.is_nan()) {
                Some(i) => (vals[i], run.mix[i]),
                None => return v,
            }
        } else {
            return v;
        };
        if x.is_nan() {
            return v;
        }
        if run.replace {
            v + m * (x - v)
        } else {
            v + m * x
        }
    }
}

/// Between frame values `a` and `b` (the next frame's, if any), `f` of the way.
fn blend(a: f64, b: Option<f64>, f: f64) -> f64 {
    match b {
        Some(b) if f > 1e-9 && !a.is_nan() && !b.is_nan() => a + (b - a) * f,
        Some(b) if a.is_nan() && f > 1e-9 => b,
        _ => a,
    }
}

/// What the cache checks a run against: the whole project as it was.
struct Fingerprint {
    library: Vec<Arc<Symbol>>,
    settings: Settings,
    root: String,
    plugins: u64,
}

impl Fingerprint {
    fn of(p: &Project) -> Fingerprint {
        Fingerprint { library: p.library.values().cloned().collect(), settings: p.settings.clone(), root: p.root_symbol.clone(), plugins: crate::plugins::generation() }
    }

    fn matches(&self, p: &Project) -> bool {
        self.plugins == crate::plugins::generation()
            && self.root == p.root_symbol
            && self.library.len() == p.library.len()
            && self.library.iter().zip(p.library.values()).all(|(a, b)| Arc::ptr_eq(a, b))
            && self.settings == p.settings
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(String, String), (Fingerprint, Arc<LayerOut>)>> = RefCell::new(HashMap::new());
}

/// A script layer's runs, from the cache or worked out now.
pub fn layer_out(project: &Project, sym_id: &str, layer_id: &str) -> Arc<LayerOut> {
    let key = (sym_id.to_string(), layer_id.to_string());
    let hit = CACHE.with(|c| c.borrow().get(&key).filter(|(fp, _)| fp.matches(project)).map(|(_, o)| o.clone()));
    if let Some(o) = hit {
        return o;
    }
    let out = Arc::new(work_out(project, sym_id, layer_id));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 64 {
            c.clear();
        }
        c.insert(key, (Fingerprint::of(project), out.clone()));
    });
    out
}

fn fps_of(project: &Project) -> f64 {
    if project.settings.fps > 0.0 {
        project.settings.fps as f64
    } else {
        30.0
    }
}

/// The longest run worked out, in frames.
const MAX_FRAMES: i64 = 20_000;

fn work_out(project: &Project, sym_id: &str, layer_id: &str) -> LayerOut {
    let Some(sym) = project.symbol(sym_id) else { return LayerOut::default() };
    let Some(l) = sym.layer(layer_id) else { return LayerOut::default() };
    let LayerKind::Script(sc) = &l.kind else { return LayerOut::default() };
    let Some(def) = crate::plugins::definition(&sc.plugin) else {
        return LayerOut { missing: true, ..LayerOut::default() };
    };
    let props = crate::plugins::fill_props(&def, &sc.props);
    let fps = fps_of(project);
    let mut r = Resolver::plain(project);
    let mut out = LayerOut::default();
    for run in &sc.runs {
        let first = (run.start * fps).round() as i64;
        let frames = ((run.end * fps).round() as i64 - first).clamp(1, MAX_FRAMES) as usize;
        let time = |k: i64| (first + k) as f64 / fps;
        // what each target looks like on each frame (the frame before the
        // run too, for velocities)
        let mut scene: Vec<Vec<(String, Seen)>> = vec![Vec::new(); frames];
        for tg in &def.targets {
            let picked = sc.targets.iter().find(|x| x.name == tg.name && x.kind == tg.kind).and_then(|x| x.to.clone());
            match tg.kind {
                TargetKind::Figure => {
                    let fl = picked.as_deref().and_then(|id| sym.layer(id)).filter(|x| x.as_figure().is_some());
                    let mut prev = fl.map(|fl| figure_seen(sym, fl, time(-1), &mut r).0);
                    for (k, sc_k) in scene.iter_mut().enumerate() {
                        let seen = match fl {
                            Some(fl) => {
                                let ((x, y), rotation, joints) = figure_seen(sym, fl, time(k as i64), &mut r);
                                let (px, py) = prev.unwrap_or((x, y));
                                prev = Some((x, y));
                                Seen::Figure { x, y, rotation, vx: (x - px) * fps, vy: (y - py) * fps, joints }
                            }
                            None => Seen::Figure { x: 0.0, y: 0.0, rotation: 0.0, vx: 0.0, vy: 0.0, joints: Vec::new() },
                        };
                        sc_k.push((tg.name.clone(), seen));
                    }
                }
                TargetKind::Value => {
                    let q = picked.as_deref().and_then(|id| project.property(id)).filter(|q| drivable(q));
                    for (k, sc_k) in scene.iter_mut().enumerate() {
                        let v = q.map(|q| ValueType::to_scalar(&r.resolve(q, time(k as i64)))).unwrap_or(0.0);
                        sc_k.push((tg.name.clone(), Seen::Value(v)));
                    }
                }
            }
        }
        let mix: Vec<f64> = (0..frames).map(|k| (r.f(&run.mix, time(k as i64)) as f64).clamp(0.0, 1.0)).collect();
        let moved = match crate::plugins::run_script(&sc.plugin, &props, run.seed as u64, fps, first, &scene) {
            Ok(m) => m,
            Err(e) => {
                out.error = Some(e);
                out.runs.clear();
                return out;
            }
        };
        let mut values: HashMap<String, Vec<f64>> = HashMap::new();
        for (k, frame) in moved.iter().enumerate() {
            for (name, m) in frame {
                let Some(tg) = sc.targets.iter().find(|x| &x.name == name) else { continue };
                let Some(to) = &tg.to else { continue };
                for (id, x) in targets_of(project, sym, tg.kind, to, m) {
                    if x.is_finite() {
                        values.entry(id).or_insert_with(|| vec![f64::NAN; frames])[k] = x;
                    }
                }
            }
        }
        out.runs.push(RunOut { run: run.id.clone(), first, frames, replace: run.replace, keep: run.keep, mix, values });
    }
    out
}

/// The values one target's output goes to: (value id, number).
fn targets_of(project: &Project, sym: &Symbol, kind: TargetKind, to: &str, m: &Moved) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    match kind {
        TargetKind::Value => {
            if let (Some(v), Some(q)) = (m.value, project.property(to)) {
                if drivable(q) {
                    out.push((q.id.clone(), v));
                }
            }
        }
        TargetKind::Figure => {
            let Some(fl) = sym.layer(to) else { return out };
            let Some(f) = fl.as_figure() else { return out };
            for (joint, a) in &m.rotate {
                if let Some(i) = joint_index(f, joint) {
                    out.push((f.nodes()[i].rotation.id.clone(), *a));
                }
            }
            if let Some(x) = m.x {
                out.push((fl.transform.x.id.clone(), x));
            }
            if let Some(y) = m.y {
                out.push((fl.transform.y.id.clone(), y));
            }
        }
    }
    out
}

/// A joint by the name a script uses: "root" (the main joint), the part's
/// name (D35), or its id.
pub fn joint_index(f: &Figure, name: &str) -> Option<usize> {
    if name == "root" {
        return (!f.nodes().is_empty()).then_some(0);
    }
    f.nodes().iter().position(|n| n.name.as_deref() == Some(name)).or_else(|| f.index_of(name))
}

/// The name a script knows a joint by.
pub fn joint_name(f: &Figure, i: usize) -> String {
    if i == 0 {
        return "root".into();
    }
    let n = &f.nodes()[i];
    n.name.clone().unwrap_or_else(|| n.id.clone())
}

/// A figure on the stage at t: its main joint, its turn, and every joint.
#[allow(clippy::type_complexity)]
fn figure_seen(sym: &Symbol, fl: &Layer, t: f64, r: &mut Resolver) -> ((f64, f64), f64, Vec<(String, f64, f64, f64)>) {
    let Some(f) = fl.as_figure() else { return ((0.0, 0.0), 0.0, Vec::new()) };
    let chain = layer_chain(sym, fl, t, r);
    let pose = FigurePose::compute(f, t, r);
    let turn = (chain.map_y(1.0, 0.0) - chain.map_y(0.0, 0.0)).atan2(chain.map_x(1.0, 0.0) - chain.map_x(0.0, 0.0)).to_degrees();
    let joints = (0..f.nodes().len()).map(|i| (joint_name(f, i), chain.map_x(pose.x[i], pose.y[i]), chain.map_y(pose.x[i], pose.y[i]), pose.angle[i] + turn)).collect();
    let root = if pose.x.is_empty() { (chain.map_x(0.0, 0.0), chain.map_y(0.0, 0.0)) } else { (chain.map_x(pose.x[0], pose.y[0]), chain.map_y(pose.x[0], pose.y[0])) };
    (root, turn + pose.angle.first().copied().unwrap_or(0.0), joints)
}
