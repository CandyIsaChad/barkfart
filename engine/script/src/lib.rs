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
    /// An effect's shader.
    pub shader: Option<Shader>,
    /// A script's targets: what it moves, picked by the user in Properties
    /// (sorted by name, like props).
    pub targets: Vec<Target>,
}

/// One thing a script moves (SCRIPTING.md "Targets, not names").
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub name: String,
    pub kind: TargetKind,
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// A stickfigure: the script turns its joints and moves it.
    Figure,
    /// Any value that can be keyed (a segment's length, an effect's
    /// strength, a plugin effect's setting): the script changes the number.
    Value,
}

impl TargetKind {
    pub fn name(self) -> &'static str {
        match self {
            TargetKind::Figure => "figure",
            TargetKind::Value => "value",
        }
    }

    pub fn from_name(s: &str) -> Option<TargetKind> {
        match s {
            "figure" => Some(TargetKind::Figure),
            "value" => Some(TargetKind::Value),
            _ => None,
        }
    }
}

/// What a script sees of one target on a frame (`scene.<name>`): the keyed
/// motion, before any script, so scripts never read each other's output.
#[derive(Clone, Debug, PartialEq)]
pub enum Seen {
    Figure {
        /// The main joint on the stage.
        x: f64,
        y: f64,
        /// Its turn on the stage, in degrees.
        rotation: f64,
        /// How fast the main joint moves, per second (0 on a still figure).
        vx: f64,
        vy: f64,
        /// Each joint by its name: on the stage, and its turn there.
        joints: Vec<(String, f64, f64, f64)>,
    },
    Value(f64),
}

/// What a script asked of one target on one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Moved {
    /// Joints by name, in degrees.
    pub rotate: Vec<(String, f64)>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// A value target's number.
    pub value: Option<f64>,
}

/// One run of a script (a block on its row).
pub struct Run<'a> {
    pub seed: u64,
    pub props: &'a [(String, PropValue)],
    pub fps: f64,
    /// The project frame the run starts on (0-based).
    pub first_frame: i64,
    /// What each target looks like on each of the run's frames.
    pub scene: &'a [Vec<(String, Seen)>],
}

/// An effect's shader (SCRIPTING.md "The shader"): WGSL text that defines
/// `fn effect(uv: vec2<f32>, u: Props) -> vec4<f32>`.
#[derive(Clone, Debug, PartialEq)]
pub struct Shader {
    pub source: String,
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
/// A whole script run, every frame of it.
const RUN_BUDGET: Duration = Duration::from_millis(2000);
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
        // one plugin can't break the API for the others
        api.set_readonly(true);
        lua.register_module("@barkfart", api).map_err(|e| ScriptError::Load(e.to_string()))?;
        // "same picture everywhere": no hidden random state that depends on
        // the order frames are drawn in (plugins get a seeded t.random)
        let setup = || -> mlua::Result<()> {
            let math: Table = lua.globals().get("math")?;
            math.set("random", Value::Nil)?;
            math.set("randomseed", Value::Nil)?;
            Ok(())
        };
        setup().map_err(|e| ScriptError::Load(e.to_string()))?;
        // read-only globals: plugins can't change the libraries or each other
        lua.sandbox(true).map_err(|e| ScriptError::Load(e.to_string()))?;
        Ok(Host { lua, deadline })
    }

    /// Runs `f` with a time budget. A call inside another keeps the earlier
    /// deadline if that comes first, and puts it back afterwards.
    fn budget<T>(&self, d: Duration, f: impl FnOnce() -> mlua::Result<T>) -> Result<T, ScriptError> {
        let prev = self.deadline.get();
        let mine = Instant::now() + d;
        self.deadline.set(Some(prev.map_or(mine, |p| p.min(mine))));
        let r = f();
        self.deadline.set(prev);
        r.map_err(classify)
    }

    /// Runs a plugin's code and checks what it returns.
    pub fn load(&self, source: &str, chunk_name: &str) -> Result<Plugin, ScriptError> {
        // each plugin has its own globals (reads fall through to the
        // libraries), so one plugin's names never reach another's
        let env = (|| -> mlua::Result<Table> {
            let env = self.lua.create_table()?;
            let meta = self.lua.create_table()?;
            meta.set("__index", self.lua.globals())?;
            env.set_metatable(Some(meta))?;
            Ok(env)
        })()
        .map_err(|e| ScriptError::Load(e.to_string()))?;
        let v: Value = self.budget(LOAD_BUDGET, || self.lua.load(source).set_name(chunk_name).set_environment(env).eval()).map_err(|e| match e {
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
        if def.kind == Kind::Script && !matches!(table.get::<Value>("step"), Ok(Value::Function(_))) {
            return Err(ScriptError::Invalid("a script needs step = function(s, p, t, scene) ... end".into()));
        }
        if def.kind == Kind::Script && !matches!(table.get::<Value>("start"), Ok(Value::Function(_)) | Ok(Value::Nil)) {
            return Err(ScriptError::Invalid("start must be a function(s, p, t, scene)".into()));
        }
        Ok(Plugin { def, table })
    }

    /// A modifier's driven value: `value(v, p, t)`, where `v.value` is the
    /// keyed value at t and `v:at(frame)` the keyed value at another frame
    /// (`sample`). The result must be a number.
    /// `seed_key` tells apart the values the plugin drives (the property's id).
    pub fn modifier_value(&self, plugin: &Plugin, seed_key: &str, value: f64, props: &[(String, PropValue)], t: Time, sample: &dyn Fn(f64) -> f64) -> Result<f64, ScriptError> {
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
                // t.random(): the same numbers for the same plugin, value and
                // frame, in whatever order frames are drawn
                let state = Cell::new(seed(&plugin.def.id, seed_key, t.frame));
                tt.set(
                    "random",
                    scope.create_function(move |_, (a, b): (Option<f64>, Option<f64>)| {
                        let u = next_random(&state);
                        Ok(match (a, b) {
                            (Some(a), Some(b)) => a + (b - a) * u,
                            (Some(b), None) => b * u,
                            _ => u,
                        })
                    })?,
                )?;
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

impl Host {
    /// Runs a script over one block (SCRIPTING.md "Script layers"): `start`
    /// on the first frame, then `step` on every frame in order, with one
    /// state table `s` kept across them. `t.random` is seeded by the run, so
    /// the same run always gives the same result. Returns what the script
    /// asked of each target on each frame.
    pub fn script_run(&self, plugin: &Plugin, run: &Run) -> Result<Vec<Vec<(String, Moved)>>, ScriptError> {
        let step: Function = plugin.table.get("step").map_err(|e| ScriptError::Invalid(e.to_string()))?;
        let start: Option<Function> = plugin.table.get("start").ok();
        let targets = plugin.def.targets.clone();
        let prev = self.deadline.get();
        let run_end = Instant::now() + RUN_BUDGET;
        let fps = if run.fps > 0.0 { run.fps } else { 30.0 };
        let r = self.lua.scope(|scope| {
            let state = Cell::new(seed(&plugin.def.id, "run", run.seed as f64));
            let random = scope.create_function(move |_, (a, b): (Option<f64>, Option<f64>)| {
                let u = next_random(&state);
                Ok(match (a, b) {
                    (Some(a), Some(b)) => a + (b - a) * u,
                    (Some(b), None) => b * u,
                    _ => u,
                })
            })?;
            let s = self.lua.create_table()?;
            let p = props_table(&self.lua, run.props)?;
            let mut out = Vec::with_capacity(run.scene.len());
            for (k, seen) in run.scene.iter().enumerate() {
                let frame = (run.first_frame + k as i64) as f64;
                let tt = self.lua.create_table()?;
                tt.set("frame", frame)?;
                tt.set("seconds", frame / fps)?;
                tt.set("fps", fps)?;
                tt.set("dt", 1.0 / fps)?;
                tt.set("elapsed", k as f64 / fps)?;
                tt.set("random", &random)?;
                let sc = scene_table(&self.lua, seen)?;
                let call = |f: &Function| -> mlua::Result<Value> {
                    self.deadline.set(Some(run_end.min(Instant::now() + CALL_BUDGET)));
                    f.call((&s, &p, &tt, &sc))
                };
                if k == 0 {
                    if let Some(f) = &start {
                        call(f)?;
                    }
                }
                let v = call(&step)?;
                out.push(moved(&v, &targets)?);
            }
            Ok(out)
        });
        self.deadline.set(prev);
        r.map_err(classify)
    }
}

fn scene_table(lua: &Lua, seen: &[(String, Seen)]) -> mlua::Result<Table> {
    let sc = lua.create_table()?;
    for (name, x) in seen {
        match x {
            Seen::Value(v) => sc.set(name.as_str(), *v)?,
            Seen::Figure { x, y, rotation, vx, vy, joints } => {
                let t = lua.create_table()?;
                t.set("x", *x)?;
                t.set("y", *y)?;
                t.set("rotation", *rotation)?;
                let v = lua.create_table()?;
                v.set("x", *vx)?;
                v.set("y", *vy)?;
                t.set("velocity", v)?;
                let js = lua.create_table()?;
                for (n, jx, jy, ja) in joints {
                    let j = lua.create_table()?;
                    j.set("x", *jx)?;
                    j.set("y", *jy)?;
                    j.set("rotation", *ja)?;
                    js.set(n.as_str(), j)?;
                }
                t.set("joints", js)?;
                sc.set(name.as_str(), t)?;
            }
        }
    }
    Ok(sc)
}

fn number(v: &Value, what: &str) -> mlua::Result<f64> {
    match v {
        Value::Number(n) => Ok(*n),
        Value::Integer(i) => Ok(*i as f64),
        other => Err(mlua::Error::runtime(format!("{what} must be a number, not {}", other.type_name()))),
    }
}

/// Reads what `step` returned: `{ cape = { rotate = { root = 5 }, move = { x = 2 } }, glow = 0.4 }`.
fn moved(v: &Value, targets: &[Target]) -> mlua::Result<Vec<(String, Moved)>> {
    let t = match v {
        Value::Nil => return Ok(Vec::new()),
        Value::Table(t) => t,
        other => return Err(mlua::Error::runtime(format!("step must return a table (or nothing), not {}", other.type_name()))),
    };
    let mut out = Vec::new();
    for tg in targets {
        let v: Value = t.get(tg.name.as_str())?;
        if v.is_nil() {
            continue;
        }
        let mut m = Moved::default();
        match tg.kind {
            TargetKind::Value => m.value = Some(number(&v, &tg.name)?),
            TargetKind::Figure => {
                let Value::Table(ft) = v else {
                    return Err(mlua::Error::runtime(format!("{} must be a table like {{ rotate = {{ root = 10 }} }}", tg.name)));
                };
                if let Value::Table(rt) = ft.get::<Value>("rotate")? {
                    for pair in rt.pairs::<String, Value>() {
                        let (joint, a) = pair?;
                        m.rotate.push((joint.clone(), number(&a, &format!("{}.rotate.{joint}", tg.name))?));
                    }
                    m.rotate.sort_by(|a, b| a.0.cmp(&b.0));
                }
                if let Value::Table(mt) = ft.get::<Value>("move")? {
                    let x: Value = mt.get("x")?;
                    let y: Value = mt.get("y")?;
                    m.x = if x.is_nil() { None } else { Some(number(&x, &format!("{}.move.x", tg.name))?) };
                    m.y = if y.is_nil() { None } else { Some(number(&y, &format!("{}.move.y", tg.name))?) };
                }
            }
        }
        out.push((tg.name.clone(), m));
    }
    Ok(out)
}

/// A seed from the plugin, the value it drives and the frame (FNV-1a).
fn seed(id: &str, key: &str, frame: f64) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in id.bytes().chain([0]).chain(key.bytes()).chain([0]).chain(frame.to_bits().to_le_bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h | 1
}

/// xorshift64*: a number in [0, 1).
fn next_random(state: &Cell<u64>) -> f64 {
    let mut x = state.get();
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    state.set(x);
    (x.wrapping_mul(0x2545F4914F6CDD1D) >> 11) as f64 / (1u64 << 53) as f64
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
    for target in ["figure", "value", "source"] {
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
    // an effect's shader: bf.wgsl("...") with the shader text itself
    let shader = match t.get::<Value>("shader") {
        Ok(Value::Table(sh)) => match text(&sh, "__shader").as_deref() {
            Some("wgsl") => {
                let source = text(&sh, "source").unwrap_or_default();
                if source.trim().ends_with(".wgsl") && !source.contains('{') {
                    return Err(ScriptError::Invalid("put the shader's text in bf.wgsl(...) itself; shader files next to the plugin come later".into()));
                }
                if !source.contains("fn effect") {
                    return Err(ScriptError::Invalid("its shader needs fn effect(uv: vec2<f32>, u: Props) -> vec4<f32>".into()));
                }
                Some(Shader { source })
            }
            Some("glsl") => return Err(ScriptError::Invalid("GLSL shaders aren't supported yet; write the shader in WGSL with bf.wgsl(...)".into())),
            _ => return Err(ScriptError::Invalid("shader = bf.wgsl(\"...\")".into())),
        },
        _ => None,
    };
    let mut targets = Vec::new();
    if let Ok(Value::Table(tt)) = t.get::<Value>("targets") {
        for pair in tt.pairs::<String, Table>() {
            let (name, x) = pair.map_err(|e| ScriptError::Invalid(format!("targets: {e}")))?;
            let kind = match text(&x, "__target").as_deref() {
                Some("figure") => TargetKind::Figure,
                Some("value") => TargetKind::Value,
                _ => return Err(ScriptError::Invalid(format!("targets.{name}: use bf.figure() or bf.value()"))),
            };
            targets.push(Target { name, kind, label: text(&x, "label") });
        }
    }
    targets.sort_by(|a, b| a.name.cmp(&b.name));
    if kind == Kind::Script && targets.is_empty() {
        return Err(ScriptError::Invalid("a script needs targets = { name = bf.figure() } (or bf.value())".into()));
    }
    if kind == Kind::Effect && shader.is_none() {
        return Err(ScriptError::Invalid("an effect needs shader = bf.wgsl(\"...\")".into()));
    }
    if kind == Kind::Effect && props.len() > 38 {
        return Err(ScriptError::Invalid("an effect can have at most 38 settings".into()));
    }
    if kind == Kind::Effect && props.iter().any(|p| matches!(p.kind, PropKind::Text { .. })) {
        return Err(ScriptError::Invalid("an effect's settings can't be text (a shader has no use for it)".into()));
    }
    Ok(Definition {
        shader, targets, name: text(t, "name").filter(|s| !s.trim().is_empty()).unwrap_or_else(|| id.clone()), version: need("version")?, id, kind, author: text(t, "author"), description: text(t, "description"), props })
}
