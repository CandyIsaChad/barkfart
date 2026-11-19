//! The project model (port of the Java `app.barkfart.model` package).
//! Field names follow the Java ones so the two stay easy to compare.

use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;

/// Java's `Math.round(float)`: floor(v + 1/2), NaN to 0, clamped to i32.
pub fn java_round(v: f32) -> i32 {
    if v.is_nan() {
        return 0;
    }
    let r = (v as f64 + 0.5).floor();
    r.clamp(i32::MIN as f64, i32::MAX as f64) as i32
}

/// Straight (non-premultiplied) sRGB colour, channels 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const CLEAR: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color { r, g, b, a }
    }

    /// From 0xRRGGBBAA.
    pub fn rgba8888(v: u32) -> Color {
        Color::new(
            ((v >> 24) & 0xff) as f32 / 255.0,
            ((v >> 16) & 0xff) as f32 / 255.0,
            ((v >> 8) & 0xff) as f32 / 255.0,
            (v & 0xff) as f32 / 255.0,
        )
    }

    pub fn to_rgba8888(&self) -> u32 {
        fn ch(v: f32) -> u32 {
            java_round(v * 255.0).clamp(0, 255) as u32
        }
        (ch(self.r) << 24) | (ch(self.g) << 16) | (ch(self.b) << 8) | ch(self.a)
    }

    pub fn lerp(&self, o: &Color, t: f32) -> Color {
        Color::new(
            self.r + (o.r - self.r) * t,
            self.g + (o.g - self.g) * t,
            self.b + (o.b - self.b) * t,
            self.a + (o.a - self.a) * t,
        )
    }

    pub fn with_alpha(&self, a: f32) -> Color {
        Color::new(self.r, self.g, self.b, a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Float,
    /// Degrees, unwrapped, interpolated numerically.
    Angle,
    /// Times (symbol time offset, speed).
    Double,
    Color,
    /// Always held.
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    F(f32),
    D(f64),
    C(Color),
    B(bool),
}

impl Value {
    pub fn f(&self) -> f32 {
        match *self {
            Value::F(v) => v,
            Value::D(v) => v as f32,
            Value::B(b) => b as i32 as f32,
            Value::C(_) => 0.0,
        }
    }

    pub fn d(&self) -> f64 {
        match *self {
            Value::D(v) => v,
            Value::F(v) => v as f64,
            Value::B(b) => b as i32 as f64,
            Value::C(_) => 0.0,
        }
    }

    pub fn b(&self) -> bool {
        matches!(*self, Value::B(true))
    }

    pub fn color(&self) -> Color {
        match *self {
            Value::C(c) => c,
            _ => Color::BLACK,
        }
    }
}

impl ValueType {
    pub fn continuous(self) -> bool {
        self != ValueType::Bool
    }

    pub fn lerp(self, a: &Value, b: &Value, t: f32) -> Value {
        match (self, a, b) {
            (ValueType::Float | ValueType::Angle, Value::F(x), Value::F(y)) => Value::F(x + (y - x) * t),
            (ValueType::Double, Value::D(x), Value::D(y)) => Value::D(x + (y - x) * t as f64),
            (ValueType::Color, Value::C(x), Value::C(y)) => Value::C(x.lerp(y, t)),
            _ => *a,
        }
    }

    /// Adds a scalar offset (additive modifiers); other types unchanged.
    pub fn add(self, v: &Value, d: f64) -> Value {
        match (self, v) {
            (ValueType::Float | ValueType::Angle, Value::F(x)) => Value::F((*x as f64 + d) as f32),
            (ValueType::Double, Value::D(x)) => Value::D(x + d),
            _ => *v,
        }
    }

    pub fn from_scalar(self, s: f64, fallback: &Value) -> Value {
        match self {
            ValueType::Float | ValueType::Angle => Value::F(s as f32),
            ValueType::Double => Value::D(s),
            ValueType::Bool => Value::B(s >= 0.5),
            ValueType::Color => *fallback,
        }
    }

    pub fn to_scalar(v: &Value) -> f64 {
        match *v {
            Value::F(x) => x as f64,
            Value::D(x) => x,
            Value::B(b) => {
                if b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::C(c) => (c.r + c.g + c.b) as f64 / 3.0,
        }
    }

    pub fn checked(self, v: &Value) -> bool {
        matches!(
            (self, v),
            (ValueType::Float | ValueType::Angle, Value::F(_))
                | (ValueType::Double, Value::D(_))
                | (ValueType::Color, Value::C(_))
                | (ValueType::Bool, Value::B(_))
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            ValueType::Float => "FLOAT",
            ValueType::Angle => "ANGLE",
            ValueType::Double => "DOUBLE",
            ValueType::Color => "COLOR",
            ValueType::Bool => "BOOL",
        }
    }

    pub fn from_name(s: &str) -> Option<ValueType> {
        Some(match s {
            "FLOAT" => ValueType::Float,
            "ANGLE" => ValueType::Angle,
            "DOUBLE" => ValueType::Double,
            "COLOR" => ValueType::Color,
            "BOOL" => ValueType::Bool,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interp {
    Hold,
    Linear,
    Bezier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EaseKind {
    Cubic,
    BounceOut,
    BounceIn,
    ElasticOut,
    ElasticIn,
}

impl EaseKind {
    pub fn name(self) -> &'static str {
        match self {
            EaseKind::Cubic => "CUBIC",
            EaseKind::BounceOut => "BOUNCE_OUT",
            EaseKind::BounceIn => "BOUNCE_IN",
            EaseKind::ElasticOut => "ELASTIC_OUT",
            EaseKind::ElasticIn => "ELASTIC_IN",
        }
    }

    pub fn from_name(s: &str) -> Option<EaseKind> {
        [EaseKind::Cubic, EaseKind::BounceOut, EaseKind::BounceIn, EaseKind::ElasticOut, EaseKind::ElasticIn]
            .into_iter()
            .find(|k| k.name() == s)
    }
}

/// Timing curve: CSS cubic-bezier through (x1, y1), (x2, y2), or a named
/// curve a cubic cannot draw (bounce, elastic).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub kind: EaseKind,
}

fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

impl Ease {
    pub const LINEAR: Ease = Ease { x1: 0.0, y1: 0.0, x2: 1.0, y2: 1.0, kind: EaseKind::Cubic };

    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Ease {
        Ease { x1: clamp01(x1), y1, x2: clamp01(x2), y2, kind: EaseKind::Cubic }
    }

    pub fn named(kind: EaseKind) -> Ease {
        Ease { x1: 0.0, y1: 0.0, x2: 1.0, y2: 1.0, kind }
    }

    fn bounce_out(mut u: f32) -> f32 {
        let n = 7.5625f32;
        let d = 2.75f32;
        if u < 1.0 / d {
            return n * u * u;
        }
        if u < 2.0 / d {
            u -= 1.5 / d;
            return n * u * u + 0.75;
        }
        if u < 2.5 / d {
            u -= 2.25 / d;
            return n * u * u + 0.9375;
        }
        u -= 2.625 / d;
        n * u * u + 0.984375
    }

    fn elastic_out(u: f32) -> f32 {
        (2f64.powf(-10.0 * u as f64) * ((u as f64 * 10.0 - 0.75) * (2.0 * std::f64::consts::PI / 3.0)).sin() + 1.0) as f32
    }

    /// Value fraction for time fraction u.
    pub fn apply(&self, u: f32) -> f32 {
        if u <= 0.0 {
            return 0.0;
        }
        if u >= 1.0 {
            return 1.0;
        }
        match self.kind {
            EaseKind::BounceOut => return Self::bounce_out(u),
            EaseKind::BounceIn => return 1.0 - Self::bounce_out(1.0 - u),
            EaseKind::ElasticOut => return Self::elastic_out(u),
            EaseKind::ElasticIn => return 1.0 - Self::elastic_out(1.0 - u),
            EaseKind::Cubic => {}
        }
        let (x1, x2, y1, y2) = (self.x1, self.x2, self.y1, self.y2);
        let mut s = u;
        for _ in 0..8 {
            let x = bez(s, x1, x2) - u;
            if x.abs() < 1e-6 {
                return bez(s, y1, y2);
            }
            let dx = dbez(s, x1, x2);
            if dx.abs() < 1e-6 {
                break;
            }
            s -= x / dx;
            if !(0.0..=1.0).contains(&s) {
                break;
            }
        }
        let (mut lo, mut hi) = (0f32, 1f32);
        s = u;
        for _ in 0..40 {
            let x = bez(s, x1, x2);
            if (x - u).abs() < 1e-6 {
                break;
            }
            if x < u {
                lo = s;
            } else {
                hi = s;
            }
            s = (lo + hi) / 2.0;
        }
        bez(s, y1, y2)
    }
}

fn bez(s: f32, p1: f32, p2: f32) -> f32 {
    let v = 1.0 - s;
    3.0 * v * v * s * p1 + 3.0 * v * s * s * p2 + s * s * s
}

fn dbez(s: f32, p1: f32, p2: f32) -> f32 {
    let v = 1.0 - s;
    3.0 * v * v * p1 + 6.0 * v * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub time: f64,
    pub value: Value,
    pub interp: Interp,
    pub ease: Ease,
}

impl Key {
    pub fn new(time: f64, value: Value, interp: Interp) -> Key {
        Key { time, value, interp, ease: Ease::LINEAR }
    }
}

/// Sparse, time-sorted keys with unique times.
#[derive(Clone, Debug)]
pub struct Track {
    pub ty: ValueType,
    keys: Vec<Key>,
}

impl Track {
    pub fn new(ty: ValueType) -> Track {
        Track { ty, keys: Vec::new() }
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Inserts or replaces the key at the same time; non-continuous types always hold.
    pub fn put(&mut self, k: Key) {
        let stored = if self.ty.continuous() { k } else { Key { interp: Interp::Hold, ..k } };
        match self.index_of(k.time) {
            Ok(i) => self.keys[i] = stored,
            Err(i) => self.keys.insert(i, stored),
        }
    }

    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Exact-time search: Ok(index) or Err(insertion point).
    pub fn index_of(&self, time: f64) -> Result<usize, usize> {
        let (mut lo, mut hi) = (0isize, self.keys.len() as isize - 1);
        while lo <= hi {
            let mid = ((lo + hi) as usize) >> 1;
            let t = self.keys[mid].time;
            if t < time {
                lo = mid as isize + 1;
            } else if t > time {
                hi = mid as isize - 1;
            } else {
                return Ok(mid);
            }
        }
        Err(lo as usize)
    }

    /// Value at t; end values hold outside the keys. Panics on an empty track.
    pub fn sample(&self, t: f64) -> Value {
        let n = self.keys.len();
        let first = &self.keys[0];
        if t <= first.time || n == 1 {
            return first.value;
        }
        let last = &self.keys[n - 1];
        if t >= last.time {
            return last.value;
        }
        let hi = match self.index_of(t) {
            Ok(i) => return self.keys[i].value,
            Err(i) => i,
        };
        let (a, b) = (&self.keys[hi - 1], &self.keys[hi]);
        match a.interp {
            Interp::Hold => a.value,
            Interp::Linear => self.ty.lerp(&a.value, &b.value, ((t - a.time) / (b.time - a.time)) as f32),
            Interp::Bezier => {
                let u = ((t - a.time) / (b.time - a.time)) as f32;
                self.ty.lerp(&a.value, &b.value, a.ease.apply(u))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    Cycle,
    PingPong,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ModifierKind {
    /// v + amplitude * noise(seed, t * frequency).
    Wiggle { frequency: f32, amplitude: f32, seed: i32, octaves: i32 },
    /// v + amplitude * sin(2 pi (frequency t + phase)).
    Sine { frequency: f32, amplitude: f32, phase: f32 },
    /// Remaps time into [start, end] before the keys. Always first.
    Loop { mode: LoopMode, start: f64, end: f64 },
    /// source(t - delay) * scale + offset.
    DelayFollow { source: String, delay: f64, scale: f32, offset: f32 },
    /// source(t) * scale + offset.
    Link { source: String, scale: f32, offset: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub enabled: bool,
    pub kind: ModifierKind,
}

impl Modifier {
    pub fn new(kind: ModifierKind) -> Modifier {
        let kind = match kind {
            ModifierKind::Wiggle { frequency, amplitude, seed, octaves } => {
                ModifierKind::Wiggle { frequency, amplitude, seed, octaves: octaves.clamp(1, 8) }
            }
            k => k,
        };
        Modifier { enabled: true, kind }
    }
}

/// Loop remap (Java Modifier.Loop.remap).
pub fn loop_remap(mode: LoopMode, start: f64, end: f64, t: f64) -> f64 {
    let len = end - start;
    if !(len > 0.0) || t <= start {
        return if t < start { t } else { start };
    }
    let u = (t - start) / len;
    if mode == LoopMode::Cycle {
        return start + (u - u.floor()) * len;
    }
    let k = u - 2.0 * (u / 2.0).floor();
    start + (if k <= 1.0 { k } else { 2.0 - k }) * len
}

/// An animatable value: static value, sparse track, modifiers after the keys.
/// The id is a project-unique path ("L3/n7/rotation") modifiers link to.
#[derive(Clone, Debug)]
pub struct Property {
    pub id: String,
    pub ty: ValueType,
    pub value: Value,
    pub track: Track,
    pub modifiers: Vec<Modifier>,
}

impl Property {
    pub fn new(id: impl Into<String>, ty: ValueType, value: Value) -> Property {
        Property { id: id.into(), ty, value, track: Track::new(ty), modifiers: Vec::new() }
    }

    pub fn float(id: impl Into<String>, v: f32) -> Property {
        Property::new(id, ValueType::Float, Value::F(v))
    }

    pub fn angle(id: impl Into<String>, v: f32) -> Property {
        Property::new(id, ValueType::Angle, Value::F(v))
    }

    pub fn double(id: impl Into<String>, v: f64) -> Property {
        Property::new(id, ValueType::Double, Value::D(v))
    }

    pub fn color(id: impl Into<String>, c: Color) -> Property {
        Property::new(id, ValueType::Color, Value::C(c))
    }

    pub fn boolean(id: impl Into<String>, b: bool) -> Property {
        Property::new(id, ValueType::Bool, Value::B(b))
    }

    /// A Loop modifier goes first (it remaps time); only one.
    pub fn add_modifier(&mut self, m: Modifier) -> Result<(), String> {
        if matches!(m.kind, ModifierKind::Loop { .. }) {
            if matches!(self.modifiers.first(), Some(Modifier { kind: ModifierKind::Loop { .. }, .. })) {
                return Err("only one Loop modifier per property".into());
            }
            self.modifiers.insert(0, m);
        } else {
            self.modifiers.push(m);
        }
        Ok(())
    }

    /// Keys-only value at t.
    pub fn key_value(&self, t: f64) -> Value {
        if self.track.is_empty() {
            self.value
        } else {
            self.track.sample(t)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Transform {
    pub x: Property,
    pub y: Property,
    pub rotation: Property,
    pub scale_x: Property,
    pub scale_y: Property,
    pub anchor_x: Property,
    pub anchor_y: Property,
}

impl Transform {
    pub fn new(owner: &str) -> Transform {
        Transform {
            x: Property::float(format!("{owner}/x"), 0.0),
            y: Property::float(format!("{owner}/y"), 0.0),
            rotation: Property::angle(format!("{owner}/rotation"), 0.0),
            scale_x: Property::float(format!("{owner}/scaleX"), 1.0),
            scale_y: Property::float(format!("{owner}/scaleY"), 1.0),
            anchor_x: Property::float(format!("{owner}/anchorX"), 0.0),
            anchor_y: Property::float(format!("{owner}/anchorY"), 0.0),
        }
    }

    pub fn all(&self) -> [&Property; 7] {
        [&self.x, &self.y, &self.rotation, &self.scale_x, &self.scale_y, &self.anchor_x, &self.anchor_y]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    None,
    Line,
    Circle,
    SquareLine,
    Ellipse,
    Triangle,
    Trapezoid,
    Polygon,
}

impl Shape {
    pub const ALL: [Shape; 8] =
        [Shape::None, Shape::Line, Shape::Circle, Shape::SquareLine, Shape::Ellipse, Shape::Triangle, Shape::Trapezoid, Shape::Polygon];

    pub fn name(self) -> &'static str {
        match self {
            Shape::None => "NONE",
            Shape::Line => "LINE",
            Shape::Circle => "CIRCLE",
            Shape::SquareLine => "SQUARE_LINE",
            Shape::Ellipse => "ELLIPSE",
            Shape::Triangle => "TRIANGLE",
            Shape::Trapezoid => "TRAPEZOID",
            Shape::Polygon => "POLYGON",
        }
    }

    pub fn from_name(s: &str) -> Option<Shape> {
        Shape::ALL.into_iter().find(|x| x.name() == s)
    }
}

pub const LOCK_NONE: i32 = 0;
pub const LOCK_ABSOLUTE: i32 = 1;
pub const LOCK_RELATIVE: i32 = 2;
/// drawOrder "not set": Figure::add puts the node in front.
pub const UNASSIGNED: i32 = i32::MIN;

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub parent: Option<String>,
    pub shape: Shape,
    pub draw_order: i32,
    pub rotation: Property,
    pub length: Property,
    pub thickness: Property,
    pub scale: Property,
    pub color: Property,
    pub shown: Property,
    pub curve: f32,
    pub triangle_upside_down: bool,
    pub right_triangle: i32,
    pub gradient: bool,
    pub gradient_reverse: bool,
    pub gradient_sideways: bool,
    pub gradient_linear: bool,
    pub gradient_color: Property,
    pub is_static: bool,
    pub stretchy: bool,
    pub smart_stretch: bool,
    pub no_smart_stretch: bool,
    pub rest_length: f32,
    pub circle_hollow: bool,
    pub circle_outline: bool,
    pub outline_color: Property,
    pub half_arc: i32,
    pub polygon_sides: i32,
    pub trapezoid_ratio: f32,
    pub trapezoid_round_start: bool,
    pub trapezoid_round_end: bool,
    pub angle_lock: i32,
    pub drag_lock: i32,
    pub pinned: bool,
    /// Legacy fields not yet understood, kept for re-export ("i:3", "f:1.5", ...).
    pub legacy_extras: IndexMap<String, String>,
}

impl Node {
    pub fn new(layer_id: &str, id: &str, parent: Option<&str>, shape: Shape) -> Node {
        let p = format!("{layer_id}/{id}/");
        Node {
            id: id.to_string(),
            parent: parent.map(str::to_string),
            shape,
            draw_order: UNASSIGNED,
            rotation: Property::angle(format!("{p}rotation"), 0.0),
            length: Property::float(format!("{p}length"), 0.0),
            thickness: Property::float(format!("{p}thickness"), 10.0),
            scale: Property::float(format!("{p}scale"), 1.0),
            color: Property::color(format!("{p}color"), Color::BLACK),
            shown: Property::boolean(format!("{p}shown"), true),
            curve: 0.0,
            triangle_upside_down: false,
            right_triangle: 0,
            gradient: false,
            gradient_reverse: false,
            gradient_sideways: false,
            gradient_linear: false,
            gradient_color: Property::color(format!("{p}gradientColor"), Color::WHITE),
            is_static: false,
            stretchy: false,
            smart_stretch: false,
            no_smart_stretch: false,
            rest_length: 0.0,
            circle_hollow: false,
            circle_outline: false,
            outline_color: Property::color(format!("{p}outlineColor"), Color::BLACK),
            half_arc: 0,
            polygon_sides: 0,
            trapezoid_ratio: 1.0,
            trapezoid_round_start: false,
            trapezoid_round_end: false,
            angle_lock: 0,
            drag_lock: 0,
            pinned: false,
            legacy_extras: IndexMap::new(),
        }
    }

    pub fn sides(&self) -> i32 {
        if self.polygon_sides <= 0 {
            5
        } else {
            self.polygon_sides.clamp(3, 16)
        }
    }

    pub fn properties(&self) -> [&Property; 8] {
        [&self.rotation, &self.length, &self.thickness, &self.scale, &self.color, &self.shown, &self.gradient_color, &self.outline_color]
    }
}

#[derive(Clone, Debug)]
pub struct Polyfill {
    pub id: String,
    pub anchor: String,
    pub nodes: Vec<String>,
    pub use_own_color: bool,
    pub color: Property,
}

impl Polyfill {
    pub fn new(layer_id: &str, id: &str) -> Polyfill {
        Polyfill {
            id: id.into(),
            anchor: String::new(),
            nodes: Vec::new(),
            use_own_color: false,
            color: Property::color(format!("{layer_id}/{id}/color"), Color::BLACK),
        }
    }
}

/// Nodes in parent-first order, with an id index.
#[derive(Clone, Debug, Default)]
pub struct Figure {
    nodes: Vec<Node>,
    pub polyfills: Vec<Polyfill>,
    index: HashMap<String, usize>,
}

impl Figure {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn nodes_mut(&mut self) -> &mut [Node] {
        &mut self.nodes
    }

    pub fn root(&self) -> Option<&Node> {
        self.nodes.first()
    }

    /// Nodes and polyfills to change together.
    pub fn parts_mut(&mut self) -> (&mut [Node], &mut Vec<Polyfill>) {
        (&mut self.nodes, &mut self.polyfills)
    }

    pub fn node(&self, id: &str) -> Option<&Node> {
        self.index.get(id).map(|&i| &self.nodes[i])
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// Adds a node; its parent must already be there (the first is the root).
    pub fn add(&mut self, mut n: Node) -> Result<(), String> {
        if self.index.contains_key(&n.id) {
            return Err(format!("duplicate node id {}", n.id));
        }
        if self.nodes.is_empty() {
            if n.parent.is_some() {
                return Err("first node must be the root".into());
            }
        } else if !n.parent.as_ref().is_some_and(|p| self.index.contains_key(p)) {
            return Err(format!("parent {:?} must be added before {}", n.parent, n.id));
        }
        if n.draw_order == UNASSIGNED {
            n.draw_order = if self.nodes.is_empty() { 0 } else { self.max_draw_order() + 1 };
        }
        self.index.insert(n.id.clone(), self.nodes.len());
        self.nodes.push(n);
        Ok(())
    }

    fn max_draw_order(&self) -> i32 {
        self.nodes.iter().map(|n| n.draw_order).max().unwrap_or(i32::MIN)
    }

    /// Node indices in draw order (draw order, then position).
    pub fn draw_sequence(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.nodes.len()).collect();
        idx.sort_by(|&a, &b| self.nodes[a].draw_order.cmp(&self.nodes[b].draw_order).then(a.cmp(&b)));
        idx
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Normal,
    Add,
    Multiply,
    Screen,
}

impl Blend {
    pub fn name(self) -> &'static str {
        match self {
            Blend::Normal => "NORMAL",
            Blend::Add => "ADD",
            Blend::Multiply => "MULTIPLY",
            Blend::Screen => "SCREEN",
        }
    }

    pub fn from_name(s: &str) -> Option<Blend> {
        [Blend::Normal, Blend::Add, Blend::Multiply, Blend::Screen].into_iter().find(|b| b.name() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Node,
    NodeSet,
    Figure,
    Layer,
    SymbolInstance,
    Composition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scope {
    pub kind: ScopeKind,
    pub target: Option<String>,
    pub node: Option<String>,
}

#[derive(Clone, Debug)]
pub struct EffectNode {
    pub id: String,
    pub ty: String,
    pub enabled: bool,
    pub mix: Property,
    pub params: IndexMap<String, Property>,
    pub inputs: Vec<String>,
    pub mask: Option<Scope>,
}

impl EffectNode {
    pub fn new(owner: &str, id: &str, ty: &str) -> EffectNode {
        EffectNode {
            id: id.into(),
            ty: ty.into(),
            enabled: true,
            mix: Property::float(format!("{owner}/{id}/mix"), 1.0),
            params: IndexMap::new(),
            inputs: Vec::new(),
            mask: None,
        }
    }

    pub fn param(&mut self, owner: &str, name: &str, ty: ValueType, value: Value) -> &mut Property {
        let p = Property::new(format!("{owner}/{}/{name}", self.id), ty, value);
        self.params.insert(name.to_string(), p);
        self.params.get_mut(name).expect("just inserted")
    }
}

#[derive(Clone, Debug, Default)]
pub struct EffectStack {
    pub nodes: IndexMap<String, EffectNode>,
    pub output: Option<String>,
}

impl EffectStack {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn append(&mut self, mut e: EffectNode) -> Result<(), String> {
        if self.nodes.contains_key(&e.id) {
            return Err(format!("duplicate effect id {}", e.id));
        }
        e.inputs.clear();
        if let Some(o) = &self.output {
            e.inputs.push(o.clone());
        }
        self.output = Some(e.id.clone());
        self.nodes.insert(e.id.clone(), e);
        Ok(())
    }

    /// The chain from the first input to the output (cycles and gaps end it).
    pub fn chain(&self) -> Vec<&EffectNode> {
        let mut rev: Vec<&EffectNode> = Vec::new();
        let mut cur = self.output.as_deref();
        let mut guard = self.nodes.len() as isize;
        while let Some(c) = cur {
            if guard < 0 {
                break;
            }
            guard -= 1;
            let Some(e) = self.nodes.get(c) else { break };
            if rev.iter().any(|x| std::ptr::eq(*x, e)) {
                break;
            }
            rev.push(e);
            cur = e.inputs.first().map(String::as_str);
        }
        rev.reverse();
        rev
    }
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub id: String,
    pub asset: Option<String>,
    pub start: f64,
    pub in_: f64,
    pub out: f64,
    pub loop_: bool,
    pub fade_in: f64,
    pub fade_out: f64,
    pub pitch: f64,
    pub volume: Property,
    pub pan: Property,
}

impl Clip {
    pub fn new(layer_id: &str, id: &str, asset: Option<String>, start: f64, in_: f64, out: f64) -> Clip {
        Clip {
            id: id.into(),
            asset,
            start,
            in_,
            out,
            loop_: false,
            fade_in: 0.0,
            fade_out: 0.0,
            pitch: 1.0,
            volume: Property::float(format!("{layer_id}/{id}/volume"), 1.0),
            pan: Property::float(format!("{layer_id}/{id}/pan"), 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolLoop {
    Once,
    Loop,
    PingPong,
}

#[derive(Clone, Debug)]
pub struct SpriteLayer {
    pub asset: Option<String>,
    pub width: f32,
    pub height: f32,
    pub tint: Property,
    /// Corner offsets BL, BR, TR, TL as x, y pairs.
    pub distort: [Property; 8],
}

impl SpriteLayer {
    pub fn distorted(&self) -> bool {
        self.distort.iter().any(|q| !q.track.is_empty() || q.value.f() != 0.0)
    }
}

#[derive(Clone, Debug)]
pub struct TextLayer {
    pub text: Option<String>,
    pub font: Option<String>,
    pub size: f32,
    pub color: Property,
    /// 0 centre, 1 TL, 2 TR, 3 BL, 4 BR.
    pub corner: i32,
    pub above_bars: bool,
}

#[derive(Clone, Debug)]
pub struct SymbolLayer {
    pub symbol: String,
    pub time_offset: Property,
    pub speed: Property,
    pub loop_mode: SymbolLoop,
    pub recursion_limit: i32,
}

#[derive(Clone, Debug)]
pub struct CameraLayer {
    pub x: Property,
    pub y: Property,
    pub zoom: Property,
    pub rotation: Property,
    pub background: Property,
    pub background2: Property,
}

#[derive(Clone, Debug)]
pub enum LayerKind {
    Figure(Figure),
    Sprite(SpriteLayer),
    Text(TextLayer),
    Symbol(SymbolLayer),
    Adjustment,
    Audio(Vec<Clip>),
    Camera(CameraLayer),
}

impl LayerKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            LayerKind::Figure(_) => "FIGURE",
            LayerKind::Sprite(_) => "SPRITE",
            LayerKind::Text(_) => "TEXT",
            LayerKind::Symbol(_) => "SYMBOL",
            LayerKind::Adjustment => "ADJUSTMENT",
            LayerKind::Audio(_) => "AUDIO",
            LayerKind::Camera(_) => "CAMERA",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub id: String,
    pub name: Option<String>,
    pub visible: bool,
    pub locked: bool,
    pub solo: bool,
    pub onion: bool,
    pub shown: Property,
    pub opacity: Property,
    pub blend: Blend,
    pub parent: Option<String>,
    pub parent_node: Option<String>,
    pub parent_node_rotate: bool,
    pub inherit_effects: bool,
    pub parallax: f32,
    pub transform: Transform,
    pub effects: EffectStack,
    pub kind: LayerKind,
}

impl Layer {
    fn base(id: &str, name: Option<&str>, kind: LayerKind) -> Layer {
        Layer {
            id: id.into(),
            name: name.map(str::to_string),
            visible: true,
            locked: false,
            solo: false,
            onion: true,
            shown: Property::boolean(format!("{id}/shown"), true),
            opacity: Property::float(format!("{id}/opacity"), 1.0),
            blend: Blend::Normal,
            parent: None,
            parent_node: None,
            parent_node_rotate: true,
            inherit_effects: false,
            parallax: 0.0,
            transform: Transform::new(id),
            effects: EffectStack::default(),
            kind,
        }
    }

    pub fn figure(id: &str, name: Option<&str>) -> Layer {
        Layer::base(id, name, LayerKind::Figure(Figure::default()))
    }

    pub fn sprite(id: &str, name: Option<&str>) -> Layer {
        let distort = std::array::from_fn(|i| Property::float(format!("{id}/distort/{}{}", i / 2, if i % 2 == 0 { "x" } else { "y" }), 0.0));
        Layer::base(
            id,
            name,
            LayerKind::Sprite(SpriteLayer { asset: None, width: 0.0, height: 0.0, tint: Property::color(format!("{id}/tint"), Color::WHITE), distort }),
        )
    }

    pub fn text(id: &str, name: Option<&str>) -> Layer {
        Layer::base(
            id,
            name,
            LayerKind::Text(TextLayer {
                text: Some(String::new()),
                font: None,
                size: 32.0,
                color: Property::color(format!("{id}/color"), Color::BLACK),
                corner: 0,
                above_bars: false,
            }),
        )
    }

    pub fn symbol(id: &str, name: Option<&str>, symbol: &str) -> Layer {
        Layer::base(
            id,
            name,
            LayerKind::Symbol(SymbolLayer {
                symbol: symbol.into(),
                time_offset: Property::double(format!("{id}/timeOffset"), 0.0),
                speed: Property::double(format!("{id}/speed"), 1.0),
                loop_mode: SymbolLoop::Loop,
                recursion_limit: 8,
            }),
        )
    }

    pub fn adjustment(id: &str, name: Option<&str>) -> Layer {
        Layer::base(id, name, LayerKind::Adjustment)
    }

    pub fn audio(id: &str, name: Option<&str>) -> Layer {
        Layer::base(id, name, LayerKind::Audio(Vec::new()))
    }

    pub fn camera(id: &str, name: Option<&str>) -> Layer {
        Layer::base(
            id,
            name,
            LayerKind::Camera(CameraLayer {
                x: Property::float(format!("{id}/cam/x"), 0.0),
                y: Property::float(format!("{id}/cam/y"), 0.0),
                zoom: Property::float(format!("{id}/cam/zoom"), 1.0),
                rotation: Property::angle(format!("{id}/cam/rotation"), 0.0),
                background: Property::color(format!("{id}/cam/bg"), Color::CLEAR),
                background2: Property::color(format!("{id}/cam/bg2"), Color::CLEAR),
            }),
        )
    }

    /// Every animatable property this layer owns (same order as Java).
    pub fn properties(&self) -> Vec<&Property> {
        let mut out: Vec<&Property> = vec![&self.shown, &self.opacity];
        out.extend(self.transform.all());
        for e in self.effects.nodes.values() {
            out.push(&e.mix);
            out.extend(e.params.values());
        }
        match &self.kind {
            LayerKind::Figure(f) => {
                for n in f.nodes() {
                    out.extend(n.properties());
                }
                for pf in &f.polyfills {
                    out.push(&pf.color);
                }
            }
            LayerKind::Sprite(s) => {
                out.push(&s.tint);
                out.extend(s.distort.iter());
            }
            LayerKind::Text(t) => out.push(&t.color),
            LayerKind::Symbol(s) => {
                out.push(&s.time_offset);
                out.push(&s.speed);
            }
            LayerKind::Adjustment => {}
            LayerKind::Audio(clips) => {
                for c in clips {
                    out.push(&c.volume);
                    out.push(&c.pan);
                }
            }
            LayerKind::Camera(c) => out.extend([&c.x, &c.y, &c.zoom, &c.rotation, &c.background, &c.background2]),
        }
        out
    }

    /// The layer's properties, mutable (same order as properties()).
    pub fn properties_mut(&mut self) -> Vec<&mut Property> {
        let mut out: Vec<&mut Property> = vec![&mut self.shown, &mut self.opacity];
        let t = &mut self.transform;
        out.extend([&mut t.x, &mut t.y, &mut t.rotation, &mut t.scale_x, &mut t.scale_y, &mut t.anchor_x, &mut t.anchor_y]);
        for e in self.effects.nodes.values_mut() {
            out.push(&mut e.mix);
            out.extend(e.params.values_mut());
        }
        match &mut self.kind {
            LayerKind::Figure(f) => {
                let (nodes, fills) = f.parts_mut();
                for n in nodes {
                    out.extend([&mut n.rotation, &mut n.length, &mut n.thickness, &mut n.scale, &mut n.color, &mut n.shown, &mut n.gradient_color, &mut n.outline_color]);
                }
                for pf in fills {
                    out.push(&mut pf.color);
                }
            }
            LayerKind::Sprite(s) => {
                out.push(&mut s.tint);
                out.extend(s.distort.iter_mut());
            }
            LayerKind::Text(t) => out.push(&mut t.color),
            LayerKind::Symbol(s) => {
                out.push(&mut s.time_offset);
                out.push(&mut s.speed);
            }
            LayerKind::Adjustment => {}
            LayerKind::Audio(clips) => {
                for c in clips {
                    out.push(&mut c.volume);
                    out.push(&mut c.pan);
                }
            }
            LayerKind::Camera(c) => out.extend([&mut c.x, &mut c.y, &mut c.zoom, &mut c.rotation, &mut c.background, &mut c.background2]),
        }
        out
    }

    pub fn as_figure(&self) -> Option<&Figure> {
        match &self.kind {
            LayerKind::Figure(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_camera(&self) -> Option<&CameraLayer> {
        match &self.kind {
            LayerKind::Camera(c) => Some(c),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub time: f64,
    pub name: Option<String>,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct ScopedStack {
    pub scope: Scope,
    pub stack: EffectStack,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub id: String,
    pub name: Option<String>,
    pub duration: f64,
    /// Shared until edited (undo snapshots copy only what changes).
    pub layers: Vec<Arc<Layer>>,
    pub markers: Vec<Marker>,
    pub scoped_effects: Vec<ScopedStack>,
}

impl Symbol {
    pub fn new(id: &str, name: Option<&str>, duration: f64) -> Symbol {
        Symbol { id: id.into(), name: name.map(str::to_string), duration, layers: Vec::new(), markers: Vec::new(), scoped_effects: Vec::new() }
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id).map(|l| &**l)
    }

    pub fn layer_index(&self, id: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    /// A layer to change (copied first if a snapshot shares it).
    pub fn layer_mut(&mut self, id: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id).map(Arc::make_mut)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetKind {
    Image,
    Audio,
    Font,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NodeSet {
    pub id: String,
    pub name: Option<String>,
    pub members: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub fps: f32,
    pub width: i32,
    pub height: i32,
    pub background: Color,
    /// Widescreen bars: the shot's aspect ratio, or 0.
    pub letterbox: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { fps: 30.0, width: 1280, height: 720, background: Color::WHITE, letterbox: 0.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub frame: i32,
    pub onion_before: i32,
    pub onion_after: i32,
    pub onion_in_front: bool,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 1.0, pan_x: 0.0, pan_y: 0.0, frame: 0, onion_before: 2, onion_after: 1, onion_in_front: false }
    }
}

pub const FORMAT_VERSION: i64 = 1;

#[derive(Clone, Debug, Default)]
pub struct Project {
    pub settings: Settings,
    pub view: View,
    /// Symbols, shared until edited.
    pub library: IndexMap<String, Arc<Symbol>>,
    pub root_symbol: String,
    pub assets: IndexMap<String, Asset>,
    pub node_sets: IndexMap<String, NodeSet>,
    pub id_counter: i32,
}

impl Project {
    pub fn root(&self) -> Option<&Symbol> {
        self.library.get(&self.root_symbol).map(|s| &**s)
    }

    pub fn symbol(&self, id: &str) -> Option<&Symbol> {
        self.library.get(id).map(|s| &**s)
    }

    /// A symbol to change (copied first if a snapshot shares it).
    pub fn symbol_mut(&mut self, id: &str) -> Option<&mut Symbol> {
        self.library.get_mut(id).map(Arc::make_mut)
    }

    pub fn root_mut(&mut self) -> Option<&mut Symbol> {
        let id = self.root_symbol.clone();
        self.symbol_mut(&id)
    }

    /// A layer of a symbol to change.
    pub fn layer_mut(&mut self, symbol: &str, layer: &str) -> Option<&mut Layer> {
        self.symbol_mut(symbol)?.layer_mut(layer)
    }

    pub fn add_symbol(&mut self, s: Symbol) {
        self.library.insert(s.id.clone(), Arc::new(s));
    }

    /// A new id (prefix + base-36 counter), skipping ids already in use (Java Project.newId).
    pub fn new_id(&mut self, prefix: &str) -> String {
        loop {
            let id = format!("{prefix}{}", to_base36(self.id_counter.max(0) as u32));
            self.id_counter += 1;
            let taken = self.library.contains_key(&id)
                || self.assets.contains_key(&id)
                || self.node_sets.contains_key(&id)
                || self.library.values().any(|s| s.layer(&id).is_some());
            if !taken {
                return id;
            }
        }
    }

    /// Where a property lives: (symbol id, layer id), or (symbol id, "") for scoped effects.
    pub fn locate_property(&self, id: &str) -> Option<(String, String)> {
        for s in self.library.values() {
            for l in &s.layers {
                if l.properties().iter().any(|p| p.id == id) {
                    return Some((s.id.clone(), l.id.clone()));
                }
            }
        }
        None
    }

    pub fn property(&self, id: &str) -> Option<&Property> {
        let (s, l) = self.locate_property(id)?;
        self.symbol(&s)?.layer(&l)?.properties().into_iter().find(|p| p.id == id)
    }

    /// A property to change (its layer is copied first if a snapshot shares it).
    pub fn property_mut(&mut self, id: &str) -> Option<&mut Property> {
        let (s, l) = self.locate_property(id)?;
        self.layer_mut(&s, &l)?.properties_mut().into_iter().find(|p| p.id == id)
    }

    /// Every property in the project by id (for modifiers that link).
    pub fn property_index(&self) -> HashMap<&str, &Property> {
        let mut m = HashMap::new();
        for s in self.library.values() {
            for l in &s.layers {
                for p in l.properties() {
                    m.insert(p.id.as_str(), p);
                }
            }
            for ss in &s.scoped_effects {
                for e in ss.stack.nodes.values() {
                    m.insert(e.mix.id.as_str(), &e.mix);
                    for p in e.params.values() {
                        m.insert(p.id.as_str(), p);
                    }
                }
            }
        }
        m
    }
}

fn to_base36(mut v: u32) -> String {
    if v == 0 {
        return "0".into();
    }
    let mut s = Vec::new();
    while v > 0 {
        s.push(b"0123456789abcdefghijklmnopqrstuvwxyz"[(v % 36) as usize]);
        v /= 36;
    }
    s.reverse();
    String::from_utf8(s).expect("ascii")
}
