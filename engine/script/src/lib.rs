//! Barkfart plugins in Luau (docs/barkfart/SCRIPTING.md): a sandboxed
//! runtime. A plugin is one `plugin.luau` that returns a definition made with
//! the `@barkfart` API (`bf.effect`, `bf.modifier`, `bf.script`, `bf.tool`);
//! its `props` describe the values it shows in Properties.
//!
//! Ground rules kept here:
//! - only the safe libraries (math, string, table, utf8, bit32, buffer); no
//!   `io`, `os`, `debug`, no bytecode; globals are read-only (Luau's sandbox);
//! - every call into a plugin has a time budget (loading 200 ms, a modifier
//!   5 ms) and the whole runtime a memory cap; going over stops the call with
//!   an error the app shows, and the plugin can be switched off;
//! - no network: nothing here can reach outside the runtime.

use mlua::{Function, Lua, LuaOptions, StdLib, Table, Value, VmState};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// What a plugin is (SCRIPTING.md "Plugin kinds").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Effect,
    Modifier,
    Script,
    Tool,
}

impl Kind {
    fn from_name(s: &str) -> Option<Kind> {
        match s {
            "effect" => Some(Kind::Effect),
            "modifier" => Some(Kind::Modifier),
            "script" => Some(Kind::Script),
            "tool" => Some(Kind::Tool),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Effect => "effect",
            Kind::Modifier => "modifier",
            Kind::Script => "script",
            Kind::Tool => "tool",
        }
    }
}

/// One of a plugin's values, as Properties shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum PropKind {
    Number { default: f64, min: Option<f64>, max: Option<f64>, step: Option<f64> },
    Angle { default: f64 },
    Toggle { default: bool },
    /// Straight RGBA, 0..1.
    Colour { default: [f32; 4] },
    Choice { options: Vec<String>, default: usize },
    Text { default: String },
    Point { default: (f64, f64) },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    pub name: String,
    pub kind: PropKind,
    pub label: Option<String>,
    pub group: Option<String>,
}

/// A plugin's definition, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: Kind,
    pub author: Option<String>,
    pub description: Option<String>,
    /// In the order the plugin lists them (sorted by name: Luau tables keep no order).
    pub props: Vec<Prop>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScriptError {
    /// The code doesn't parse or failed while loading.
    Load(String),
    /// It ran, but what it returned isn't a plugin definition.
    Invalid(String),
    /// It ran past its time budget and was stopped.
    TooSlow,
    /// It asked for more memory than a plugin may use.
    TooMuchMemory,
    /// It failed while running.
    Runtime(String),
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptError::Load(m) => write!(f, "The plugin couldn't be loaded: {m}"),
            ScriptError::Invalid(m) => write!(f, "This isn't a Barkfart plugin: {m}"),
            ScriptError::TooSlow => write!(f, "The plugin took too long and was stopped."),
            ScriptError::TooMuchMemory => write!(f, "The plugin used too much memory and was stopped."),
            ScriptError::Runtime(m) => write!(f, "The plugin failed: {m}"),
        }
    }
}

impl std::error::Error for ScriptError {}

/// The time an instance is at, for a plugin (`t` in SCRIPTING.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Time {
    pub frame: f64,
    pub seconds: f64,
    pub fps: f64,
}

/// A value handed to a plugin.
#[derive(Clone, Debug, PartialEq)]
pub enum PropValue {
    Number(f64),
    Toggle(bool),
    Colour([f32; 4]),
    Text(String),
    Point(f64, f64),
}

const LOAD_BUDGET: Duration = Duration::from_millis(200);
const CALL_BUDGET: Duration = Duration::from_millis(5);
/// The memory one runtime may hold.
pub const MEMORY_CAP: usize = 32 * 1024 * 1024;
const OVER_TIME: &str = "barkfart: over the time budget";

/// A sandboxed Luau runtime for plugins. Not shared between threads.
pub struct Host {
    lua: Lua,
    deadline: Rc<Cell<Option<Instant>>>,
}

/// A loaded plugin: its definition and the table it returned.
pub struct Plugin {
    pub def: Definition,
    table: Table,
}

impl Host {
    pub fn new() -> Result<Host, ScriptError> {
        let libs = StdLib::MATH | StdLib::STRING | StdLib::TABLE | StdLib::UTF8 | StdLib::BIT | StdLib::BUFFER;
        let lua = Lua::new_with(libs, LuaOptions::default()).map_err(|e| ScriptError::Load(e.to_string()))?;
        lua.set_memory_limit(MEMORY_CAP).map_err(|e| ScriptError::Load(e.to_string()))?;
        let deadline: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));
        let d = deadline.clone();
        lua.set_interrupt(move |_| match d.get() {
            Some(at) if Instant::now() > at => Err(mlua::Error::runtime(OVER_TIME)),
            _ => Ok(VmState::Continue),
        });
        let api = bf_api(&lua).map_err(|e| ScriptError::Load(e.to_string()))?;
        lua.register_module("@barkfart", api).map_err(|e| ScriptError::Load(e.to_string()))?;
        // read-only globals: plugins can't change the libraries or each other
        lua.sandbox(true).map_err(|e| ScriptError::Load(e.to_string()))?;
        Ok(Host { lua, deadline })
    }

    fn budget<T>(&self, d: Duration, f: impl FnOnce() -> mlua::Result<T>) -> Result<T, ScriptError> {
        self.deadline.set(Some(Instant::now() + d));
        let r = f();
        self.deadline.set(None);
        r.map_err(classify)
    }

    /// Runs a plugin's code and checks what it returns.
    pub fn load(&self, source: &str, chunk_name: &str) -> Result<Plugin, ScriptError> {
        let v: Value = self.budget(LOAD_BUDGET, || self.lua.load(source).set_name(chunk_name).eval()).map_err(|e| match e {
            ScriptError::Runtime(m) => ScriptError::Load(m),
            other => other,
        })?;
        let Value::Table(table) = v else {
            return Err(ScriptError::Invalid("plugin.luau must return bf.effect { ... }, bf.modifier { ... }, bf.script { ... } or bf.tool { ... }".into()));
        };
        let def = definition(&table)?;
        if def.kind == Kind::Modifier && !matches!(table.get::<Value>("value"), Ok(Value::Function(_))) {
            return Err(ScriptError::Invalid("a modifier needs value = function(v, p, t) ... end".into()));
        }
        Ok(Plugin { def, table })
    }

    /// A modifier's driven value: `value(v, p, t)`, where `v.value` is the
    /// keyed value at t and `v:at(frame)` the keyed value at another frame
    /// (`sample`). The result must be a number.
    pub fn modifier_value(&self, plugin: &Plugin, value: f64, props: &[(String, PropValue)], t: Time, sample: &dyn Fn(f64) -> f64) -> Result<f64, ScriptError> {
        let f: Function = plugin.table.get("value").map_err(|e| ScriptError::Invalid(e.to_string()))?;
        self.budget(CALL_BUDGET, || {
            self.lua.scope(|scope| {
                let v = self.lua.create_table()?;
                v.set("value", value)?;
                v.set("at", scope.create_function(|_, (_this, frame): (Value, f64)| Ok(sample(frame)))?)?;
                let p = props_table(&self.lua, props)?;
                let tt = self.lua.create_table()?;
                tt.set("frame", t.frame)?;
                tt.set("seconds", t.seconds)?;
                tt.set("fps", t.fps)?;
                let out: Value = f.call((v, p, tt))?;
                match out {
                    Value::Number(n) => Ok(n),
                    Value::Integer(i) => Ok(i as f64),
                    other => Err(mlua::Error::runtime(format!("value must return a number, not {}", other.type_name()))),
                }
            })
        })
    }
}

/// Sorts mlua's errors into the kinds the app reports.
fn classify(e: mlua::Error) -> ScriptError {
    let text = e.to_string();
    if text.contains(OVER_TIME) {
        return ScriptError::TooSlow;
    }
    match e {
        mlua::Error::MemoryError(_) => ScriptError::TooMuchMemory,
        mlua::Error::SyntaxError { message, .. } => ScriptError::Load(message),
        mlua::Error::CallbackError { ref cause, .. } if matches!(**cause, mlua::Error::MemoryError(_)) => ScriptError::TooMuchMemory,
        _ if text.contains("not enough memory") => ScriptError::TooMuchMemory,
        _ => ScriptError::Runtime(text.lines().next().unwrap_or("").to_string()),
    }
}

fn props_table(lua: &Lua, props: &[(String, PropValue)]) -> mlua::Result<Table> {
    let p = lua.create_table()?;
    for (k, v) in props {
        match v {
            PropValue::Number(n) => p.set(k.as_str(), *n)?,
            PropValue::Toggle(b) => p.set(k.as_str(), *b)?,
            PropValue::Text(s) => p.set(k.as_str(), s.as_str())?,
            PropValue::Colour(c) => {
                let t = lua.create_table()?;
                for (i, n) in ["r", "g", "b", "a"].iter().enumerate() {
                    t.set(*n, c[i])?;
                }
                p.set(k.as_str(), t)?;
            }
            PropValue::Point(x, y) => {
                let t = lua.create_table()?;
                t.set("x", *x)?;
                t.set("y", *y)?;
                p.set(k.as_str(), t)?;
            }
        }
    }
    Ok(p)
}

/// The `@barkfart` module: constructors that tag tables for `definition`.
fn bf_api(lua: &Lua) -> mlua::Result<Table> {
    let bf = lua.create_table()?;
    for kind in ["effect", "modifier", "script", "tool"] {
        bf.set(
            kind,
            lua.create_function(move |_, t: Table| {
                t.set("__kind", kind)?;
                Ok(t)
            })?,
        )?;
    }
    for prop in ["number", "angle", "toggle", "colour", "text", "point"] {
        bf.set(
            prop,
            lua.create_function(move |lua, t: Option<Table>| {
                let t = match t {
                    Some(t) => t,
                    None => lua.create_table()?,
                };
                t.set("__prop", prop)?;
                Ok(t)
            })?,
        )?;
    }
    // bf.choice { "Ripple", "Twist", default = "Ripple" }
    bf.set(
        "choice",
        lua.create_function(|_, t: Table| {
            t.set("__prop", "choice")?;
            Ok(t)
        })?,
    )?;
    bf.set(
        "rgb",
        lua.create_function(|lua, (r, g, b, a): (f64, f64, f64, Option<f64>)| {
            let t = lua.create_table()?;
            t.set("r", r)?;
            t.set("g", g)?;
            t.set("b", b)?;
            t.set("a", a.unwrap_or(1.0))?;
            Ok(t)
        })?,
    )?;
    bf.set(
        "vec2",
        lua.create_function(|lua, (x, y): (f64, f64)| {
            let t = lua.create_table()?;
            t.set("x", x)?;
            t.set("y", y)?;
            Ok(t)
        })?,
    )?;
    for lang in ["glsl", "wgsl"] {
        bf.set(
            lang,
            lua.create_function(move |lua, src: String| {
                let t = lua.create_table()?;
                t.set("__shader", lang)?;
                t.set("source", src)?;
                Ok(t)
            })?,
        )?;
    }
    for target in ["figure", "source"] {
        bf.set(
            target,
            lua.create_function(move |lua, t: Option<Table>| {
                let t = match t {
                    Some(t) => t,
                    None => lua.create_table()?,
                };
                t.set("__target", target)?;
                Ok(t)
            })?,
        )?;
    }
    Ok(bf)
}

fn text(t: &Table, k: &str) -> Option<String> {
    match t.get::<Value>(k) {
        Ok(Value::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn num(t: &Table, k: &str) -> Option<f64> {
    match t.get::<Value>(k) {
        Ok(Value::Number(n)) => Some(n),
        Ok(Value::Integer(i)) => Some(i as f64),
        _ => None,
    }
}

/// Checks a returned table and reads it into a `Definition`.
fn definition(t: &Table) -> Result<Definition, ScriptError> {
    let kind = text(t, "__kind").and_then(|k| Kind::from_name(&k)).ok_or_else(|| ScriptError::Invalid("return bf.effect { ... }, bf.modifier { ... }, bf.script { ... } or bf.tool { ... }".into()))?;
    let need = |k: &str| text(t, k).filter(|s| !s.trim().is_empty()).ok_or_else(|| ScriptError::Invalid(format!("it needs {k} = \"...\"")));
    let id = need("id")?;
    if !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        return Err(ScriptError::Invalid(format!("its id \"{id}\" may use only letters, digits, dots, dashes and underscores")));
    }
    let mut props = Vec::new();
    if let Ok(Value::Table(pt)) = t.get::<Value>("props") {
        for pair in pt.pairs::<String, Table>() {
            let (name, p) = pair.map_err(|e| ScriptError::Invalid(format!("props: {e}")))?;
            let kind = match text(&p, "__prop").as_deref() {
                Some("number") => PropKind::Number { default: num(&p, "default").unwrap_or(0.0), min: num(&p, "min"), max: num(&p, "max"), step: num(&p, "step") },
                Some("angle") => PropKind::Angle { default: num(&p, "default").unwrap_or(0.0) },
                Some("toggle") => PropKind::Toggle { default: matches!(p.get::<Value>("default"), Ok(Value::Boolean(true))) },
                Some("text") => PropKind::Text { default: text(&p, "default").unwrap_or_default() },
                Some("colour") => {
                    let c = match p.get::<Value>("default") {
                        Ok(Value::Table(c)) => [num(&c, "r").unwrap_or(1.0) as f32, num(&c, "g").unwrap_or(1.0) as f32, num(&c, "b").unwrap_or(1.0) as f32, num(&c, "a").unwrap_or(1.0) as f32],
                        _ => [1.0; 4],
                    };
                    PropKind::Colour { default: c }
                }
                Some("point") => {
                    let d = match p.get::<Value>("default") {
                        Ok(Value::Table(c)) => (num(&c, "x").unwrap_or(0.0), num(&c, "y").unwrap_or(0.0)),
                        _ => (0.0, 0.0),
                    };
                    PropKind::Point { default: d }
                }
                Some("choice") => {
                    let options: Vec<String> = p.sequence_values::<String>().filter_map(Result::ok).collect();
                    if options.is_empty() {
                        return Err(ScriptError::Invalid(format!("props.{name}: a choice needs its options")));
                    }
                    let default = text(&p, "default").and_then(|d| options.iter().position(|o| *o == d)).unwrap_or(0);
                    PropKind::Choice { options, default }
                }
                _ => return Err(ScriptError::Invalid(format!("props.{name}: use bf.number, bf.angle, bf.toggle, bf.colour, bf.choice, bf.text or bf.point"))),
            };
            props.push(Prop { name, kind, label: text(&p, "label"), group: text(&p, "group") });
        }
    }
    props.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Definition { name: text(t, "name").filter(|s| !s.trim().is_empty()).unwrap_or_else(|| id.clone()), version: need("version")?, id, kind, author: text(t, "author"), description: text(t, "description"), props })
}
