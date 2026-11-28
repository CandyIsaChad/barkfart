//! Installed plugins (SCRIPTING.md, K2): one list for the whole app, so every
//! project, export and thumbnail sees the same ones. Each thread compiles
//! them into its own Luau runtime on first use, again when the list changes.
//! A plugin that fails (too slow, too much memory, an error) is switched off
//! with its message; installing it again switches it back on.

use barkfart_script::{Definition, Host, Kind, Moved, Plugin, PropKind, PropValue, Run, Seen, Time};
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

/// Goes up whenever the installed plugins change (so worked-out script runs
/// are done again).
pub fn generation() -> u64 {
    registry().generation
}

/// Why an installed plugin was switched off (None: it runs, or isn't installed).
pub fn off(id: &str) -> Option<String> {
    registry().list.iter().find(|x| x.def.id == id).and_then(|x| x.off.clone())
}

pub fn list() -> Vec<Listed> {
    registry().list.iter().map(|x| Listed { def: x.def.clone(), off: x.off.clone() }).collect()
}

/// A plugin's code as it was installed (the Workshop's peek).
pub fn source(id: &str) -> Option<String> {
    registry().list.iter().find(|x| x.def.id == id).map(|x| x.source.clone())
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
        r.generation += 1;
    }
}

/// A plugin's settings as it receives them: the plugin's defaults fill any it
/// doesn't have; ones it no longer knows are left out.
pub fn fill_props(def: &Definition, props: &[(String, PropValue)]) -> Vec<(String, PropValue)> {
    def.props.iter().map(|d| (d.name.clone(), props.iter().find(|(n, _)| *n == d.name).map(|(_, v)| v.clone()).unwrap_or_else(|| default_value(&d.kind)))).collect()
}

/// Runs `f` with this thread's runtime and plugin `id`, compiling the
/// installed plugins first if the list changed. None when it isn't loaded.
fn with_plugin<T>(generation: u64, id: &str, f: impl FnOnce(&Host, &Plugin) -> T) -> Option<T> {
    LOADED.with(|cell| {
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
        Some(f(host, plugin))
    })
}

/// A script layer's run (K2): what the script asks of its targets on each
/// frame. Err: why it can't run (it is then switched off if it failed).
pub fn run_script(id: &str, props: &[(String, PropValue)], seed: u64, fps: f64, first_frame: i64, scene: &[Vec<(String, Seen)>]) -> Result<Vec<Vec<(String, Moved)>>, String> {
    let generation = {
        let r = registry();
        let Some(x) = r.list.iter().find(|x| x.def.id == id) else { return Err("That plugin isn't installed.".into()) };
        if let Some(why) = &x.off {
            return Err(why.clone());
        }
        if x.def.kind != Kind::Script {
            return Err(format!("{} isn't a script.", x.def.name));
        }
        r.generation
    };
    let outcome = with_plugin(generation, id, |host, plugin| host.script_run(plugin, &Run { seed, props, fps, first_frame, scene })).ok_or_else(|| "The plugin couldn't be loaded.".to_string())?;
    outcome.map_err(|e| {
        let why = e.to_string();
        switch_off(id, why.clone());
        why
    })
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
    let props = fill_props(&def, props);
    let outcome = with_plugin(generation, id, |host, plugin| host.modifier_value(plugin, key, v, &props, t, sample))?;
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
