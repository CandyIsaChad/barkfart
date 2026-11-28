//! Property values at a time: keys, then modifiers (port of Java
//! PropertyResolver, Noise, SpeedIntegral). Memoised per evaluation; link
//! cycles return the keys-only value and record a CYCLE diagnostic.

use super::scene::{Diagnostic, DiagnosticKind};
use crate::model::*;
use super::scripts::{Driven, LayerOut};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MAX_LINK_DEPTH: usize = 256;
/// Cap on link and delay-follow lookups per resolver (delayed chains fan out).
pub const RESOLVE_BUDGET: usize = 200_000;

pub struct Resolver<'p> {
    project: &'p Project,
    index: HashMap<&'p str, &'p Property>,
    memo: HashMap<(String, u64), Value>,
    visiting: HashSet<(String, u64)>,
    link_depth: usize,
    resolves: usize,
    budget_reported: bool,
    /// Animation Tools "Is looping animation" (tweening on): between the last
    /// frame's time and the end, tracks keyed on the last frame tween into
    /// their value at frame 1; a repeat with "use frame tweening properties"
    /// blends into the frame it jumps to. (from, to, target time)
    wrap: Option<(f64, f64, f64)>,
    /// Frames per second, for plugin modifiers' `t` (K2).
    fps: f64,
    /// K2: the values script layers change (None: none, or a resolver that
    /// reads the keyed motion only, as scripts themselves do).
    scripts: Option<Arc<Driven>>,
    script_out: HashMap<usize, Arc<LayerOut>>,
    pub diagnostics: Vec<Diagnostic>,
}

fn wrap_of(project: &Project) -> Option<(f64, f64, f64)> {
    let st = &project.settings;
    if !(st.tweening && st.loop_tween) {
        return None;
    }
    let fps = if st.fps > 0.0 { st.fps as f64 } else { 30.0 };
    let frames = (project.root()?.duration * fps).round() as i64;
    (frames > 1).then(|| ((frames - 1) as f64 / fps, frames as f64 / fps, 0.0))
}

impl<'p> Resolver<'p> {
    pub fn new(project: &'p Project) -> Resolver<'p> {
        let mut r = Resolver::plain(project);
        r.scripts = super::scripts::driven(project);
        r
    }

    /// A resolver that leaves script layers out: keys and modifiers only.
    pub fn plain(project: &'p Project) -> Resolver<'p> {
        Resolver {
            project,
            scripts: None,
            script_out: HashMap::new(),
            index: project.property_index(),
            memo: HashMap::new(),
            visiting: HashSet::new(),
            link_depth: 0,
            resolves: 0,
            budget_reported: false,
            wrap: wrap_of(project),
            fps: if project.settings.fps > 0.0 { project.settings.fps as f64 } else { 30.0 },
            diagnostics: Vec::new(),
        }
    }

    pub fn diag(&mut self, kind: DiagnosticKind, subject: impl Into<String>) {
        self.diagnostics.push(Diagnostic { kind, subject: subject.into() });
    }

    pub fn has(&self, kind: DiagnosticKind) -> bool {
        self.diagnostics.iter().any(|d| d.kind == kind)
    }

    /// Blends the in-betweens of one frame into another (a repeat's jump).
    pub fn with_redirect(mut self, r: Option<crate::timing::Redirect>) -> Self {
        if r.is_some() {
            self.wrap = r;
            self.memo.clear();
        }
        self
    }

    pub fn f(&mut self, p: &Property, t: f64) -> f32 {
        self.resolve(p, t).f()
    }

    pub fn d(&mut self, p: &Property, t: f64) -> f64 {
        self.resolve(p, t).d()
    }

    pub fn b(&mut self, p: &Property, t: f64) -> bool {
        self.resolve(p, t).b()
    }

    pub fn color(&mut self, p: &Property, t: f64) -> Color {
        self.resolve(p, t).color()
    }

    /// A colour that jumps at each key instead of blending (a figure with
    /// "Tween colours" off): the value of the last key at or before t. The
    /// keys themselves are left as they are, so turning the switch back on
    /// gives the blend (and the easing) they had.
    pub fn color_held(&mut self, p: &Property, t: f64) -> Color {
        let keys = p.track.keys();
        if keys.len() < 2 || !p.modifiers.is_empty() {
            return self.color(p, t);
        }
        keys.iter().rev().find(|k| k.time <= t + 1e-9).unwrap_or(&keys[0]).value.color()
    }

    pub fn resolve(&mut self, p: &Property, t: f64) -> Value {
        let key = (p.id.clone(), t.to_bits());
        if let Some(v) = self.memo.get(&key) {
            return *v;
        }
        if self.visiting.contains(&key) || self.link_depth >= MAX_LINK_DEPTH {
            self.diag(DiagnosticKind::Cycle, p.id.clone());
            return p.key_value(t);
        }
        self.visiting.insert(key.clone());
        self.link_depth += 1;
        let v = self.compute(p, t);
        self.link_depth -= 1;
        self.visiting.remove(&key);
        self.memo.insert(key, v);
        v
    }

    fn compute(&mut self, p: &Property, t: f64) -> Value {
        let mods = &p.modifiers;
        let mut kt = t;
        let mut start = 0;
        if let Some(Modifier { enabled, kind: ModifierKind::Loop { mode, start: s, end } }) = mods.first() {
            if *enabled {
                kt = loop_remap(*mode, *s, *end, t);
            }
            start = 1;
        }
        let mut v = p.key_value(kt);
        if let Some((a, e, target)) = self.wrap {
            if kt > a && kt < e {
                // only tracks keyed on that frame, tweening there
                if let Ok(i) = p.track.index_of(a) {
                    let k = &p.track.keys()[i];
                    if k.interp != Interp::Hold && p.ty.continuous() {
                        v = p.ty.lerp(&k.value, &p.key_value(target), ((kt - a) / (e - a)) as f32);
                    }
                }
            }
        }
        for m in &mods[start..] {
            if !m.enabled {
                continue;
            }
            match &m.kind {
                ModifierKind::Wiggle { frequency, amplitude, seed, octaves } => {
                    v = p.ty.add(&v, *amplitude as f64 * fbm(t * *frequency as f64, *seed, *octaves));
                }
                ModifierKind::Sine { frequency, amplitude, phase } => {
                    v = p.ty.add(&v, *amplitude as f64 * (2.0 * std::f64::consts::PI * (*frequency as f64 * t + *phase as f64)).sin());
                }
                ModifierKind::Link { source, scale, offset } => {
                    v = self.follow(p, v, source, t, *scale, *offset);
                }
                ModifierKind::DelayFollow { source, delay, scale, offset } => {
                    v = self.follow(p, v, source, t - delay, *scale, *offset);
                }
                ModifierKind::Loop { .. } => {}
                ModifierKind::Plugin { id, props, .. } => {
                    // K2: numbers only; v:at(frame) reads the keys (nothing re-enters)
                    let n = match v {
                        Value::F(x) => x as f64,
                        Value::D(x) => x,
                        _ => continue,
                    };
                    let fps = self.fps;
                    let loop_spec = match mods.first() {
                        Some(Modifier { enabled: true, kind: ModifierKind::Loop { mode, start: s, end } }) => Some((*mode, *s, *end)),
                        _ => None,
                    };
                    let sample = |frame: f64| {
                        let st = match loop_spec {
                            Some((mode, s, e)) => loop_remap(mode, s, e, frame / fps),
                            None => frame / fps,
                        };
                        ValueType::to_scalar(&p.key_value(st))
                    };
                    let time = barkfart_script::Time { frame: t * fps, seconds: t, fps };
                    match crate::plugins::modify(id, &p.id, n, props, time, &sample) {
                        Some(x) => {
                            v = match v {
                                Value::D(_) => Value::D(x),
                                _ => Value::F(x as f32),
                            }
                        }
                        None => {
                            if crate::plugins::definition(id).is_none() {
                                self.diag(DiagnosticKind::MissingRef, format!("plugin:{id}"));
                            }
                        }
                    }
                }
            }
        }
        // K2: script layers, on top of keys and modifiers
        if let Some(d) = &self.scripts {
            let base_id = p.id.split_once('#').map_or(p.id.as_str(), |(a, _)| a);
            if let Some(ls) = d.by_prop.get(base_id) {
                let d = d.clone();
                let mut x = ValueType::to_scalar(&v);
                for &li in ls.clone().iter() {
                    let out = match self.script_out.get(&li) {
                        Some(o) => o.clone(),
                        None => {
                            let (sym, layer) = &d.layers[li];
                            let o = super::scripts::layer_out(self.project, sym, layer);
                            if o.missing {
                                self.diag(DiagnosticKind::MissingRef, format!("script:{layer}"));
                            }
                            self.script_out.insert(li, o.clone());
                            o
                        }
                    };
                    x = out.apply(base_id, x, t * self.fps);
                }
                v = p.ty.from_scalar(x, &v);
            }
        }
        self.sanitize(p, v)
    }

    fn follow(&mut self, p: &Property, current: Value, source: &str, t: f64, scale: f32, offset: f32) -> Value {
        let Some(src) = self.index.get(source).copied() else {
            self.diag(DiagnosticKind::MissingRef, format!("{}->{}", p.id, source));
            return current;
        };
        if self.resolves >= RESOLVE_BUDGET {
            if !self.budget_reported {
                self.diag(DiagnosticKind::ResolveBudget, p.id.clone());
            }
            self.budget_reported = true;
            return current;
        }
        self.resolves += 1;
        let s = self.resolve(src, t);
        if src.ty == p.ty && (!p.ty.continuous() || p.ty == ValueType::Color) {
            return s;
        }
        p.ty.from_scalar(ValueType::to_scalar(&s) * scale as f64 + offset as f64, &current)
    }

    fn sanitize(&mut self, p: &Property, v: Value) -> Value {
        let x = match v {
            Value::F(f) => f as f64,
            Value::D(d) => d,
            _ => return v,
        };
        if !x.is_finite() {
            self.diag(DiagnosticKind::BadValue, p.id.clone());
            return p.ty.from_scalar(0.0, &v);
        }
        v
    }
}

/// Deterministic 1D value noise, fractal sum, in [-1, 1].
pub fn fbm(x: f64, seed: i32, octaves: i32) -> f64 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..octaves {
        sum += amp * noise_value(x * f, seed.wrapping_add(o.wrapping_mul(1013)));
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

fn noise_value(x: f64, seed: i32) -> f64 {
    let fl = x.floor();
    let i = fl as i64;
    let mut u = x - fl;
    u = u * u * (3.0 - 2.0 * u);
    let (a, b) = (noise_hash(i, seed), noise_hash(i.wrapping_add(1), seed));
    a + (b - a) * u
}

fn noise_hash(i: i64, seed: i32) -> f64 {
    let mut h: i64 = i.wrapping_mul(0x9E3779B97F4A7C15u64 as i64).wrapping_add((seed as i64).wrapping_mul(0xBF58476D1CE4E5B9u64 as i64));
    h ^= ((h as u64) >> 31) as i64;
    h = h.wrapping_mul(0x94D049BB133111EBu64 as i64);
    h ^= ((h as u64) >> 29) as i64;
    (((h as u64) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)) * 2.0 - 1.0
}

const STEP: f64 = 1.0 / 240.0;
const MAX_STEPS: f64 = 4096.0;

/// Local time contributed by an instance's speed: the integral of speed from 0 to t.
pub fn speed_elapsed(speed: &Property, t: f64, r: &mut Resolver) -> f64 {
    let tr = &speed.track;
    if tr.is_empty() && speed.modifiers.is_empty() {
        return speed.value.d() * t;
    }
    if !speed.modifiers.is_empty() || tr.keys().iter().any(|k| matches!(k.interp, Interp::Bezier | Interp::Smooth)) {
        let steps = MAX_STEPS.min(1f64.max((t.abs() / STEP).ceil())) as i32;
        let h = t / steps as f64;
        let mut sum = 0.0;
        let mut prev = r.d(speed, 0.0);
        for i in 1..=steps {
            let cur = r.d(speed, i as f64 * h);
            sum += (prev + cur) / 2.0 * h;
            prev = cur;
        }
        return sum;
    }
    exact_integral(tr, t)
}

fn exact_integral(tr: &Track, t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    let (a, b) = (t.min(0.0), t.max(0.0));
    let mut sum = 0.0;
    let n = tr.len();
    let mut prev = a;
    for i in 0..=n {
        let next = if i < n { tr.keys()[i].time.max(a).min(b) } else { b };
        if next > prev {
            let e = (next - prev) * 1e-9;
            let (v0, v1) = (tr.sample(prev + e).d(), tr.sample(next - e).d());
            sum += (next - prev) * (v0 + v1) / 2.0;
        }
        prev = prev.max(next);
    }
    if t < 0.0 {
        -sum
    } else {
        sum
    }
}
