//! Installed plugins (SCRIPTING.md, K2): one list for the whole app, so every
//! project, export and thumbnail sees the same ones. Each thread compiles
//! them into its own Luau runtime on first use, again when the list changes.
//! A plugin that fails (too slow, too much memory, an error) is switched off
//! with its message; installing it again switches it back on.

use barkfart_script::{Definition, Host, Kind, Plugin, PropKind, PropValue, Time};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;

struct Installed {
    def: Definition,
    source: String,
    off: Option<String>,
}

struct Registry {
    generation: u64,
    list: Vec<Installed>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry { generation: 0, list: Vec::new() });

thread_local! {
    static LOADED: RefCell<Option<(u64, Host, HashMap<String, Plugin>)>> = const { RefCell::new(None) };
}

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

/// A plugin as the app lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub def: Definition,
    /// Why it was switched off (None: it runs).
    pub off: Option<String>,
}

/// Checks and installs a plugin (replacing one with the same id, and
/// switching it back on). Returns its definition, or why it can't be used.
pub fn install(source: &str, name: &str) -> Result<Definition, String> {
    let host = Host::new().map_err(|e| e.to_string())?;
    let p = host.load(source, name).map_err(|e| e.to_string())?;
    let def = p.def.clone();
    let mut r = registry();
    r.list.retain(|x| x.def.id != def.id);
    r.list.push(Installed { def: def.clone(), source: source.to_string(), off: None });
    r.generation += 1;
    Ok(def)
}

pub fn uninstall(id: &str) {
    let mut r = registry();
    r.list.retain(|x| x.def.id != id);
    r.generation += 1;
}

pub fn list() -> Vec<Listed> {
    registry().list.iter().map(|x| Listed { def: x.def.clone(), off: x.off.clone() }).collect()
}

pub fn definition(id: &str) -> Option<Definition> {
    registry().list.iter().find(|x| x.def.id == id).map(|x| x.def.clone())
}

/// The renderer reports a shader that won't compile (K2 effects).
pub fn report_failure(id: &str, why: String) {
    switch_off(id, why);
}

/// An installed effect plugin that runs: its definition (with its shader) and
/// the list's generation, so a renderer can tell when to compile again.
pub fn effect(id: &str) -> Option<(Definition, u64)> {
    let r = registry();
    let x = r.list.iter().find(|x| x.def.id == id)?;
    if x.off.is_some() || x.def.kind != Kind::Effect {
        return None;
    }
    Some((x.def.clone(), r.generation))
}

fn switch_off(id: &str, why: String) {
    let mut r = registry();
    if let Some(x) = r.list.iter_mut().find(|x| x.def.id == id) {
        x.off = Some(why);
    }
}

/// A prop's starting value, as the plugin receives it (a choice by its name).
pub fn default_value(kind: &PropKind) -> PropValue {
    match kind {
        PropKind::Number { default, .. } | PropKind::Angle { default } => PropValue::Number(*default),
        PropKind::Toggle { default } => PropValue::Toggle(*default),
        PropKind::Colour { default } => PropValue::Colour(*default),
        PropKind::Choice { options, default } => PropValue::Text(options.get(*default).cloned().unwrap_or_default()),
        PropKind::Text { default } => PropValue::Text(default.clone()),
        PropKind::Point { default } => PropValue::Point(default.0, default.1),
    }
}

/// A plugin modifier's value: `v` changed by plugin `id` with `props` (the
/// plugin's defaults fill any it doesn't have; ones it no longer knows are
/// left out). None when the plugin isn't installed, is switched off, or
/// fails now (it is then switched off): the value passes through.
pub fn modify(id: &str, key: &str, v: f64, props: &[(String, PropValue)], t: Time, sample: &dyn Fn(f64) -> f64) -> Option<f64> {
    let (generation, def) = {
        let r = registry();
        let x = r.list.iter().find(|x| x.def.id == id)?;
        if x.off.is_some() || x.def.kind != Kind::Modifier {
            return None;
        }
        (r.generation, x.def.clone())
    };
    let props: Vec<(String, PropValue)> = def.props.iter().map(|d| (d.name.clone(), props.iter().find(|(n, _)| *n == d.name).map(|(_, v)| v.clone()).unwrap_or_else(|| default_value(&d.kind)))).collect();
    let outcome = LOADED.with(|cell| {
        let mut c = cell.borrow_mut();
        if c.as_ref().is_none_or(|x| x.0 != generation) {
            let host = Host::new().ok()?;
            let sources: Vec<(String, String)> = registry().list.iter().map(|x| (x.def.id.clone(), x.source.clone())).collect();
            let mut loaded = HashMap::new();
            for (pid, src) in sources {
                if let Ok(p) = host.load(&src, &pid) {
                    loaded.insert(pid, p);
                }
            }
            *c = Some((generation, host, loaded));
        }
        let (_, host, loaded) = c.as_ref()?;
        let plugin = loaded.get(id)?;
        Some(host.modifier_value(plugin, key, v, &props, t, sample))
    })?;
    match outcome {
        Ok(x) if x.is_finite() => Some(x),
        Ok(_) => {
            switch_off(id, "The plugin gave a value that isn't a number.".into());
            None
        }
        Err(e) => {
            switch_off(id, e.to_string());
            None
        }
    }
}
