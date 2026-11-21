//! Stick Nodes files (port of Java app.barkfart.legacy): .nodes figures
//! (read and write, byte for byte), .nodemc movieclips (version 330), and
//! the figure library of .stknds projects. Plus the mappers to and from
//! Barkfart layers and symbols.

use crate::model::*;
use indexmap::IndexMap;
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct LegacyError(pub String);

impl std::fmt::Display for LegacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LegacyError {}

type R<T> = Result<T, LegacyError>;

fn err<T>(s: impl Into<String>) -> R<T> {
    Err(LegacyError(s.into()))
}

// ---------------------------------------------------------------- binary io

struct In<'a> {
    b: &'a [u8],
    little: bool,
    pos: usize,
}

impl<'a> In<'a> {
    fn need(&self, n: usize) -> R<()> {
        if self.pos + n > self.b.len() {
            return err(format!("unexpected end of file at {}", self.pos));
        }
        Ok(())
    }

    fn i8(&mut self) -> R<i32> {
        self.need(1)?;
        self.pos += 1;
        Ok(self.b[self.pos - 1] as i8 as i32)
    }

    fn u8(&mut self) -> R<i32> {
        self.need(1)?;
        self.pos += 1;
        Ok(self.b[self.pos - 1] as i32)
    }

    fn i16(&mut self) -> R<i32> {
        self.need(2)?;
        let p = [self.b[self.pos], self.b[self.pos + 1]];
        self.pos += 2;
        Ok(if self.little { i16::from_le_bytes(p) } else { i16::from_be_bytes(p) } as i32)
    }

    fn i32(&mut self) -> R<i32> {
        self.need(4)?;
        let p = [self.b[self.pos], self.b[self.pos + 1], self.b[self.pos + 2], self.b[self.pos + 3]];
        self.pos += 4;
        Ok(if self.little { i32::from_le_bytes(p) } else { i32::from_be_bytes(p) })
    }

    /// A count plausible for the remaining bytes (each item at least min_bytes).
    fn count(&mut self, min_bytes: usize) -> R<usize> {
        let n = self.i32()?;
        if n < 0 || (n as u64) * (min_bytes.max(1) as u64) > (self.b.len() - self.pos) as u64 {
            return err(format!("bad count {n} at {}", self.pos));
        }
        Ok(n as usize)
    }

    fn take(&mut self, n: usize) -> R<Vec<u8>> {
        self.need(n)?;
        self.pos += n;
        Ok(self.b[self.pos - n..self.pos].to_vec())
    }

    fn str(&mut self) -> R<String> {
        let n = self.count(1)?;
        let s = String::from_utf8_lossy(&self.b[self.pos..self.pos + n]).into_owned();
        self.pos += n;
        Ok(s)
    }

    fn at_end(&self) -> bool {
        self.pos == self.b.len()
    }
}

struct Out {
    o: Vec<u8>,
    little: bool,
}

impl Out {
    fn i8(&mut self, v: i32) {
        self.o.push(v as u8);
    }

    fn i16(&mut self, v: i32) {
        let b = (v as i16).to_be_bytes();
        if self.little {
            self.o.extend([b[1], b[0]]);
        } else {
            self.o.extend(b);
        }
    }

    fn i32(&mut self, v: i32) {
        if self.little {
            self.o.extend(v.to_le_bytes());
        } else {
            self.o.extend(v.to_be_bytes());
        }
    }
}

// ---------------------------------------------------------------- .nodes

pub const MAX_VERSION: i32 = 431;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    I8,
    U8,
    I16,
    I32,
    F32,
}

/// A schema field with its version / build window.
pub struct Field {
    pub name: &'static str,
    pub kind: Kind,
    min_version: i32,
    min_build: i32,
    max_build: i32,
    not_build36: bool,
}

const ANY: i32 = i32::MAX;

impl Field {
    pub fn present(&self, version: i32, build: i32) -> bool {
        if version < self.min_version {
            return false;
        }
        if self.min_version >= 403 && (build < self.min_build || build > self.max_build) {
            return false;
        }
        !(self.not_build36 && build == 36)
    }
}

const fn f(name: &'static str, kind: Kind) -> Field {
    Field { name, kind, min_version: 0, min_build: 0, max_build: ANY, not_build36: false }
}

const fn v(name: &'static str, kind: Kind, min_version: i32) -> Field {
    Field { name, kind, min_version, min_build: 0, max_build: ANY, not_build36: false }
}

const fn b(name: &'static str, kind: Kind, min_build: i32, max_build: i32) -> Field {
    Field { name, kind, min_version: 403, min_build, max_build, not_build36: false }
}

use Kind::*;

/// Node fields in file order.
pub static NODE: &[Field] = &[
    f("type", I8),
    f("drawIndex", I32),
    f("static", U8),
    f("stretchy", U8),
    b("floaty", U8, 48, ANY),
    v("smartStretch", U8, 248),
    v("noSmartStretch", U8, 252),
    b("smartStretchResetImpulse", U8, 50, ANY),
    f("useSegmentColor", U8),
    v("circleOutline", U8, 256),
    b("circleHollow", U8, 21, ANY),
    v("gradient", U8, 176),
    v("reverseGradient", U8, 176),
    b("gradientMode", I16, 20, ANY),
    f("useSegmentScale", U8),
    f("localX", F32),
    f("localY", F32),
    f("scale", F32),
    f("defaultLength", F32),
    f("length", F32),
    f("defaultThickness", I32),
    f("thickness", I32),
    v("curveRadius", I32, 320),
    b("curveCircularize", U8, 20, ANY),
    b("curvePolyfillPrecision", I16, 21, ANY),
    v("halfArc", U8, 256),
    v("rightTriangleDirection", I16, 256),
    v("triangleUpsideDown", U8, 300),
    b("trapezoidStartI", I32, 36, 63),
    b("trapezoidEndI", I32, 36, 63),
    b("trapezoidStartF", F32, 64, ANY),
    b("trapezoidEndF", F32, 64, ANY),
    b("unused36a", I32, 36, 36),
    b("unused36b", I32, 36, 36),
    b("useTrapezoidStart", U8, 36, 36),
    b("useTrapezoidEnd", U8, 36, 36),
    Field { name: "trapezoidTopRatio", kind: F32, min_version: 256, min_build: 0, max_build: ANY, not_build36: true },
    b("trapezoidRoundedStart", U8, 36, ANY),
    b("trapezoidRoundedEnd", U8, 36, ANY),
    v("polygonVertices", I16, 256),
    v("defaultLocalAngle", F32, 248),
    f("localAngle", F32),
    v("defaultAngle", F32, 248),
    f("color", I32),
    v("gradientColor", I32, 176),
    v("circleOutlineColor", I32, 256),
    b("angleLocked", U8, 39, ANY),
    b("unused39to50", F32, 39, 50),
    b("angleLockMain", U8, 51, ANY),
    b("angleLockMinuend", F32, 51, 56),
    b("angleLockSubtrahend", F32, 51, 56),
    b("angleLockOffset", F32, 57, ANY),
    b("angleLockRelativeStart", F32, 63, ANY),
    b("angleLockFigureStart", F32, 67, ANY),
    b("angleLockRelativeMultiplier", I8, 63, ANY),
    b("dragLockedShort", I16, 39, 40),
    b("dragLocked", U8, 41, ANY),
    b("unused41to45s", I16, 41, 45),
    b("dragLockAngle", F32, 46, ANY),
    b("smartStretchMultiplier", F32, 41, ANY),
    b("unused41to45b", U8, 41, 45),
];

/// Connector data, before a connector node's own fields.
pub static CONNECTOR: &[Field] = &[
    b("cLocalX", F32, 0, ANY),
    b("cLocalY", F32, 0, ANY),
    b("cPercent", F32, 0, ANY),
    b("cPercentDefault", F32, 44, ANY),
    b("cValue", F32, 0, ANY),
    b("cMethod", I32, 0, ANY),
    b("cReversed", U8, 0, ANY),
    b("cSmartStretchAncestral", F32, 65, ANY),
];

/// A legacy field value: an integer, or float bits (kept exactly).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LV {
    I(i32),
    F(u32),
}

impl LV {
    pub fn i(self) -> i32 {
        match self {
            LV::I(v) => v,
            LV::F(b) => f32::from_bits(b) as i32,
        }
    }

    pub fn f(self) -> f32 {
        match self {
            LV::I(v) => v as f32,
            LV::F(b) => f32::from_bits(b),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct LegacyNode {
    pub fields: IndexMap<String, LV>,
    pub connector: IndexMap<String, LV>,
    pub is_connector: bool,
    pub children: Vec<usize>,
}

impl LegacyNode {
    pub fn i(&self, k: &str) -> i32 {
        self.fields.get(k).map(|v| v.i()).unwrap_or(0)
    }

    pub fn f(&self, k: &str) -> f32 {
        self.fields.get(k).map(|v| v.f()).unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct LegacyPolyfill {
    pub anchor: i32,
    pub color: i32,
    pub use_color: i32,
    pub nodes: Vec<i32>,
}

/// A figure: nodes in an arena, nodes[0] the root; children by index.
#[derive(Clone, Debug, Default)]
pub struct LegacyFigure {
    pub version: i32,
    pub build: i32,
    pub scale: u32,
    pub color: i32,
    pub little_endian: bool,
    pub compressed: bool,
    pub nodes: Vec<LegacyNode>,
    pub polyfills: Vec<LegacyPolyfill>,
    pub connections: Vec<(i32, i32)>,
}

impl LegacyFigure {
    /// Node indices depth first (hierarchy order).
    pub fn all(&self) -> Vec<usize> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let mut stack = vec![0usize];
        while let Some(n) = stack.pop() {
            out.push(n);
            for &c in self.nodes[n].children.iter().rev() {
                stack.push(c);
            }
        }
        out
    }
}

fn read_value(inp: &mut In, k: Kind) -> R<LV> {
    Ok(match k {
        I8 => LV::I(inp.i8()?),
        U8 => LV::I(inp.u8()?),
        I16 => LV::I(inp.i16()?),
        I32 => LV::I(inp.i32()?),
        F32 => LV::F(inp.i32()? as u32),
    })
}

fn inflate(z: &[u8]) -> R<Vec<u8>> {
    let mut d = flate2::read::ZlibDecoder::new(z).take(64 * 1024 * 1024 + 1);
    let mut out = Vec::new();
    d.read_to_end(&mut out).map_err(|e| LegacyError(e.to_string()))?;
    if out.len() > 64 * 1024 * 1024 {
        return err("figure too large");
    }
    Ok(out)
}

/// Reads a .nodes file (zlib little-endian for old versions).
pub fn read_nodes(file: &[u8]) -> R<LegacyFigure> {
    let compressed = file.len() > 2 && file[0] == 0x78 && ((file[0] as u32) << 8 | file[1] as u32) % 31 == 0;
    let body = if compressed { inflate(file)? } else { file.to_vec() };
    let mut inp = In { b: &body, little: compressed, pos: 0 };
    let mut fig = read_figure(&mut inp)?;
    if !inp.at_end() {
        return err(format!("trailing data at {} of {}", inp.pos, body.len()));
    }
    fig.compressed = compressed;
    Ok(fig)
}

fn read_node(inp: &mut In, fig: &LegacyFigure, is_connector: bool) -> R<LegacyNode> {
    let mut n = LegacyNode { is_connector, ..Default::default() };
    if is_connector {
        for fd in CONNECTOR {
            if fd.present(fig.version, fig.build) {
                n.connector.insert(fd.name.into(), read_value(inp, fd.kind)?);
            }
        }
    }
    for fd in NODE {
        if fd.present(fig.version, fig.build) {
            n.fields.insert(fd.name.into(), read_value(inp, fd.kind)?);
        }
    }
    let count = inp.count(4)?;
    n.fields.insert("~children".into(), LV::I(count as i32));
    if fig.version >= 403 && fig.build > 38 {
        for i in 0..count {
            n.fields.insert(format!("~connector{i}"), LV::I(inp.i32()?));
        }
    }
    Ok(n)
}

fn read_figure(inp: &mut In) -> R<LegacyFigure> {
    let mut fig = LegacyFigure { little_endian: inp.little, ..Default::default() };
    fig.version = inp.i32()?;
    if fig.version < 0 || fig.version > MAX_VERSION {
        return err(format!("unsupported .nodes version {}", fig.version));
    }
    if fig.version >= 403 {
        fig.build = inp.i32()?;
    }
    fig.scale = inp.i32()? as u32;
    fig.color = inp.i32()?;
    let root = read_node(inp, &fig, false)?;
    fig.nodes.push(root);
    // depth first with an explicit stack: (node, next child slot)
    let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
    while let Some(&(parent, next)) = stack.last() {
        let count = fig.nodes[parent].i("~children") as usize;
        if next >= count {
            stack.pop();
            continue;
        }
        stack.last_mut().expect("top").1 += 1;
        let flag = fig.nodes[parent].fields.get(&format!("~connector{next}")).map(|v| v.i()).unwrap_or(0);
        let child = read_node(inp, &fig, flag != 0)?;
        fig.nodes.push(child);
        let ci = fig.nodes.len() - 1;
        fig.nodes[parent].children.push(ci);
        if fig.nodes.len() > 100_000 {
            return err("too many nodes");
        }
        stack.push((ci, 0));
    }
    if fig.version >= 230 && !fig.little_endian {
        let n = inp.count(13)?;
        for _ in 0..n {
            let mut p = LegacyPolyfill { anchor: inp.i32()?, color: inp.i32()?, use_color: inp.u8()?, nodes: Vec::new() };
            let m = inp.count(4)?;
            for _ in 0..m {
                p.nodes.push(inp.i32()?);
            }
            fig.polyfills.push(p);
        }
    }
    if fig.version >= 403 && fig.build >= 38 {
        let n = inp.count(8)?;
        for _ in 0..n {
            fig.connections.push((inp.i32()?, inp.i32()?));
        }
    }
    Ok(fig)
}

fn write_value(out: &mut Out, k: Kind, v: Option<&LV>) {
    let v = v.copied().unwrap_or(LV::I(0));
    match k {
        I8 | U8 => out.i8(v.i()),
        I16 => out.i16(v.i()),
        I32 => out.i32(v.i()),
        F32 => out.i32(match v {
            LV::F(bits) => bits as i32,
            LV::I(i) => (i as f32).to_bits() as i32,
        }),
    }
}

fn write_figure(out: &mut Out, fig: &LegacyFigure) {
    out.i32(fig.version);
    if fig.version >= 403 {
        out.i32(fig.build);
    }
    out.i32(fig.scale as i32);
    out.i32(fig.color);
    for n in fig.all() {
        let node = &fig.nodes[n];
        if node.is_connector {
            for fd in CONNECTOR {
                if fd.present(fig.version, fig.build) {
                    write_value(out, fd.kind, node.connector.get(fd.name));
                }
            }
        }
        for fd in NODE {
            if fd.present(fig.version, fig.build) {
                write_value(out, fd.kind, node.fields.get(fd.name));
            }
        }
        out.i32(node.children.len() as i32);
        if fig.version >= 403 && fig.build > 38 {
            for &c in &node.children {
                out.i32(if fig.nodes[c].is_connector { 1 } else { 0 });
            }
        }
    }
    if fig.version >= 230 && !fig.little_endian {
        out.i32(fig.polyfills.len() as i32);
        for p in &fig.polyfills {
            out.i32(p.anchor);
            out.i32(p.color);
            out.i8(p.use_color);
            out.i32(p.nodes.len() as i32);
            for &k in &p.nodes {
                out.i32(k);
            }
        }
    }
    if fig.version >= 403 && fig.build >= 38 {
        out.i32(fig.connections.len() as i32);
        for &(a, b) in &fig.connections {
            out.i32(a);
            out.i32(b);
        }
    }
}

/// The figure as .nodes file bytes (compressed when it was read compressed).
pub fn write_nodes(fig: &LegacyFigure) -> Vec<u8> {
    let mut out = Out { o: Vec::new(), little: fig.little_endian };
    write_figure(&mut out, fig);
    if fig.compressed {
        // Java's DeflaterOutputStream: zlib, default level
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&out.o).expect("in memory");
        z.finish().expect("in memory")
    } else {
        out.o
    }
}

// ---------------------------------------------------------------- figure mapper

/// Legacy colour int: bytes A, B, G, R from the high byte down.
pub fn legacy_color(c: i32) -> Color {
    let c = c as u32;
    let (a, b, g, r) = ((c >> 24) & 0xff, (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff);
    Color::rgba8888((r << 24) | (g << 16) | (b << 8) | a)
}

pub fn to_legacy_color(c: &Color) -> i32 {
    let v = c.to_rgba8888();
    let (r, g, b, a) = ((v >> 24) & 0xff, (v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
    ((a << 24) | (b << 16) | (g << 8) | r) as i32
}

/// Typed extras, spelled like the Java version ("i:3", "f:1.5", "b:true", "s:x").
fn ex_int(v: i32) -> String {
    format!("i:{v}")
}

fn ex_float(v: f32) -> String {
    format!("f:{}", crate::json::java_float(v))
}

fn ex_lv(v: LV) -> String {
    match v {
        LV::I(i) => ex_int(i),
        LV::F(b) => ex_float(f32::from_bits(b)),
    }
}

fn ex_get<'a>(n: &'a Node, k: &str) -> Option<&'a str> {
    n.legacy_extras.get(k).map(String::as_str)
}

fn ex_parse(s: &str) -> Option<LV> {
    let (t, body) = s.split_at(2.min(s.len()));
    match t {
        "i:" => body.parse::<i32>().ok().map(LV::I),
        "f:" => parse_java_float(body).map(|f| LV::F(f.to_bits())),
        _ => None,
    }
}

fn parse_java_float(s: &str) -> Option<f32> {
    match s {
        "NaN" => Some(f32::NAN),
        "Infinity" => Some(f32::INFINITY),
        "-Infinity" => Some(f32::NEG_INFINITY),
        _ => s.parse::<f32>().ok(),
    }
}

/// Approximations made on import (shapes Barkfart cannot draw yet).
#[derive(Default, Debug, Clone)]
pub struct Report {
    pub approximations: Vec<String>,
}

fn shape_of(t: i32, report: &mut Report) -> Shape {
    match t {
        -1 => Shape::None,
        0 => Shape::Line,
        2 | 4 => Shape::Circle,
        1 => Shape::SquareLine,
        3 => Shape::Triangle,
        5 => Shape::Ellipse,
        6 => Shape::Trapezoid,
        7 => Shape::Polygon,
        _ => {
            let what = format!("unknown type {t}");
            if !report.approximations.contains(&what) {
                report.approximations.push(what);
            }
            Shape::Line
        }
    }
}

/// Trapezoid width at the node over width at the parent.
fn trapezoid_ratio(ln: &LegacyNode) -> f32 {
    if ln.fields.contains_key("trapezoidTopRatio") && ln.f("trapezoidTopRatio") > 0.0 {
        return ln.f("trapezoidTopRatio");
    }
    let (a, b) = (ln.f("trapezoidStartF"), ln.f("trapezoidEndF"));
    if a > 0.0 && b > 0.0 {
        return b / a;
    }
    1.0
}

/// A figure layer from a legacy figure (every legacy field kept in extras).
pub fn to_layer(p: &mut Project, fig: &LegacyFigure, name: &str, report: &mut Report) -> Layer {
    let id = new_id(p, "L");
    let mut layer = Layer::figure(&id, Some(name));
    layer.transform.scale_x.value = Value::F(f32::from_bits(fig.scale));
    layer.transform.scale_y.value = Value::F(f32::from_bits(fig.scale));
    let fig_color = legacy_color(fig.color);
    let LayerKind::Figure(figure) = &mut layer.kind else { unreachable!() };
    let mut stack: Vec<(usize, Option<String>)> = vec![(0, None)];
    while let Some((li, parent)) = stack.pop() {
        let ln = &fig.nodes[li];
        let t = ln.i("type");
        let mut n = Node::new(&id, &format!("n{}", ln.i("drawIndex")), parent.as_deref(), shape_of(t, report));
        n.draw_order = ln.i("drawIndex");
        n.rotation.value = Value::F(ln.f("localAngle"));
        n.length.value = Value::F(ln.f("length"));
        if ln.i("useSegmentScale") != 0 {
            n.scale.value = Value::F(ln.f("scale"));
        }
        n.thickness.value = Value::F(ln.i("thickness") as f32);
        n.curve = ln.i("curveRadius") as f32;
        n.triangle_upside_down = ln.i("triangleUpsideDown") != 0;
        n.right_triangle = ln.i("rightTriangleDirection").signum();
        n.gradient = ln.i("gradient") != 0;
        n.gradient_reverse = (ln.i("reverseGradient") != 0) ^ (t == 0);
        n.gradient_sideways = ln.i("gradientMode") == 1;
        n.is_static = ln.i("static") != 0;
        n.stretchy = ln.i("stretchy") != 0;
        n.smart_stretch = ln.i("smartStretch") != 0;
        n.no_smart_stretch = ln.i("noSmartStretch") != 0;
        n.rest_length = ln.f("defaultLength");
        n.circle_hollow = ln.i("circleHollow") != 0;
        n.circle_outline = ln.i("circleOutline") != 0;
        if ln.fields.contains_key("circleOutlineColor") {
            n.outline_color.value = Value::C(legacy_color(ln.i("circleOutlineColor")));
        }
        n.half_arc = if ln.i("halfArc") != 0 { 1 } else { 0 };
        n.polygon_sides = ln.i("polygonVertices");
        n.trapezoid_ratio = trapezoid_ratio(ln);
        n.trapezoid_round_start = ln.i("trapezoidRoundedStart") != 0;
        n.trapezoid_round_end = ln.i("trapezoidRoundedEnd") != 0;
        if ln.fields.contains_key("gradientColor") {
            n.gradient_color.value = Value::C(legacy_color(ln.i("gradientColor")));
        }
        n.color.value = Value::C(if ln.i("useSegmentColor") != 0 { legacy_color(ln.i("color")) } else { fig_color });
        for (k, v) in &ln.fields {
            if !k.starts_with('~') {
                n.legacy_extras.insert(format!("L.{k}"), ex_lv(*v));
            }
        }
        if ln.is_connector {
            n.legacy_extras.insert("connector".into(), "b:true".into());
            for (k, v) in &ln.connector {
                n.legacy_extras.insert(format!("C.{k}"), ex_lv(*v));
            }
        }
        if parent.is_none() {
            n.legacy_extras.insert("fig.version".into(), ex_int(fig.version));
            n.legacy_extras.insert("fig.build".into(), ex_int(fig.build));
            n.legacy_extras.insert("fig.color".into(), ex_int(fig.color));
            n.legacy_extras.insert("fig.scale".into(), ex_float(f32::from_bits(fig.scale)));
            n.legacy_extras.insert("fig.little".into(), format!("b:{}", fig.little_endian));
            n.legacy_extras.insert("fig.compressed".into(), format!("b:{}", fig.compressed));
            let cn: Vec<String> = fig.connections.iter().map(|(a, b)| format!("{a},{b}")).collect();
            n.legacy_extras.insert("fig.connections".into(), format!("s:{}", cn.join(";")));
        }
        let nid = n.id.clone();
        // duplicate draw indices make the figure unreadable as a tree; the Java version refuses too
        let _ = figure.add(n);
        for &c in ln.children.iter().rev() {
            stack.push((c, Some(nid.clone())));
        }
    }
    for (k, lp) in fig.polyfills.iter().enumerate() {
        let mut pf = Polyfill::new(&id, &format!("pf{k}"));
        pf.anchor = format!("n{}", lp.anchor);
        pf.nodes = lp.nodes.iter().map(|n| format!("n{n}")).collect();
        pf.use_own_color = lp.use_color != 0;
        pf.color.value = Value::C(legacy_color(lp.color));
        figure.polyfills.push(pf);
    }
    layer
}

/// The project's next id (Project::new_id).
pub fn new_id(p: &mut Project, prefix: &str) -> String {
    p.new_id(prefix)
}

fn default_value(fd: &Field) -> LV {
    if fd.kind == F32 {
        if matches!(fd.name, "scale" | "smartStretchMultiplier" | "trapezoidTopRatio") {
            return LV::F(1f32.to_bits());
        }
        return LV::F(0f32.to_bits());
    }
    if fd.name == "useSegmentScale" {
        return LV::I(1);
    }
    LV::I(0)
}

fn set_if_changed(ln: &mut LegacyNode, k: &str, v: f32) {
    if ln.f(k) != v {
        ln.fields.insert(k.into(), LV::F(v.to_bits()));
    }
}

fn sync_flag(ln: &mut LegacyNode, k: &str, v: bool) {
    if ln.fields.contains_key(k) && (ln.i(k) != 0) != v {
        ln.fields.insert(k.into(), LV::I(v as i32));
    }
}

fn legacy_type(n: &Node, previous: i32) -> i32 {
    match n.shape {
        Shape::Circle => {
            if previous == 2 || previous == 4 {
                previous
            } else {
                2
            }
        }
        Shape::SquareLine => 1,
        Shape::Triangle => 3,
        Shape::Ellipse => 5,
        Shape::Trapezoid => 6,
        Shape::Polygon => 7,
        Shape::Line => {
            if previous == 0 || previous == 6 || previous == 7 {
                previous
            } else {
                0
            }
        }
        Shape::None => previous,
    }
}

fn refresh_offset(ln: &mut LegacyNode, n: &Node, world: f64) {
    let (lx, ly, len) = (ln.f("localX"), ln.f("localY"), n.length.value.f());
    let h = (lx as f64).hypot(ly as f64);
    let ratio = if h > 1e-6 && len > 1e-6 {
        if (h / len as f64 - 1.0).abs() < 0.01 {
            1.0
        } else {
            0.5
        }
    } else {
        0.5
    };
    let want = ratio * len as f64;
    let ang = (ly as f64).atan2(lx as f64).to_degrees();
    let d_ang = (((ang - world) % 360.0 + 540.0) % 360.0 - 180.0).abs();
    if (h - want).abs() <= 0.01 * want.max(1.0) && (h < 1e-6 || d_ang < 0.05) {
        return;
    }
    let rad = world.to_radians();
    ln.fields.insert("localX".into(), LV::F(((want * rad.cos()) as f32).to_bits()));
    ln.fields.insert("localY".into(), LV::F(((want * rad.sin()) as f32).to_bits()));
}

/// A legacy figure from a layer's static values (keys ignored).
pub fn to_legacy(layer: &Layer) -> R<LegacyFigure> {
    let Some(figure) = layer.as_figure() else { return err("not a figure") };
    let nodes = figure.nodes();
    let Some(root) = nodes.first() else { return err("empty figure") };
    let ex_i = |k: &str, d: i32| ex_get(root, k).and_then(|s| s.strip_prefix("i:")).and_then(|s| s.parse().ok()).unwrap_or(d);
    let ex_b = |k: &str, d: bool| ex_get(root, k).and_then(|s| s.strip_prefix("b:")).map(|s| s == "true").unwrap_or(d);
    let mut fig = LegacyFigure::default();
    fig.version = ex_i("fig.version", 420);
    fig.build = ex_i("fig.build", if fig.version >= 403 { 62 } else { 0 });
    fig.little_endian = ex_b("fig.little", fig.version < 160);
    fig.compressed = ex_b("fig.compressed", fig.version < 160);
    let scale = layer.transform.scale_x.value.f();
    let old = ex_get(root, "fig.scale").and_then(|s| s.strip_prefix("f:")).and_then(parse_java_float);
    fig.scale = match old {
        Some(o) if o == scale => o.to_bits(),
        _ => scale.to_bits(),
    };
    fig.color = ex_i("fig.color", to_legacy_color(&Color::BLACK));
    let fig_color_model = legacy_color(fig.color);
    let mut by_id: std::collections::HashMap<String, usize> = Default::default();
    let mut world_angle: std::collections::HashMap<String, f64> = Default::default();
    for n in nodes {
        let mut ln = LegacyNode::default();
        let is_root = n.parent.is_none();
        for fd in NODE {
            if !fd.present(fig.version, fig.build) {
                continue;
            }
            let v = ex_get(n, &format!("L.{}", fd.name)).and_then(ex_parse).unwrap_or_else(|| default_value(fd));
            ln.fields.insert(fd.name.into(), v);
        }
        ln.is_connector = ex_get(n, "connector") == Some("b:true");
        if ln.is_connector {
            for fd in CONNECTOR {
                if !fd.present(fig.version, fig.build) {
                    continue;
                }
                let v = ex_get(n, &format!("C.{}", fd.name)).and_then(ex_parse).unwrap_or_else(|| default_value(fd));
                ln.connector.insert(fd.name.into(), v);
            }
        }
        let prev_type = ln.i("type");
        ln.fields.insert("type".into(), LV::I(if is_root { -1 } else { legacy_type(n, prev_type) }));
        ln.fields.insert("drawIndex".into(), LV::I(if is_root { 0 } else { n.draw_order }));
        set_if_changed(&mut ln, "localAngle", n.rotation.value.f());
        set_if_changed(&mut ln, "length", n.length.value.f());
        let sc = n.scale.value.f();
        if ln.fields.contains_key("useSegmentScale") {
            if ln.i("useSegmentScale") != 0 {
                set_if_changed(&mut ln, "scale", sc);
            } else if sc != 1.0 {
                ln.fields.insert("useSegmentScale".into(), LV::I(1));
                ln.fields.insert("scale".into(), LV::F(sc.to_bits()));
            }
        }
        if ln.fields.contains_key("curveRadius") && ln.i("curveRadius") != java_round(n.curve) {
            ln.fields.insert("curveRadius".into(), LV::I(java_round(n.curve)));
        }
        if ln.fields.contains_key("triangleUpsideDown") && (ln.i("triangleUpsideDown") != 0) != n.triangle_upside_down {
            ln.fields.insert("triangleUpsideDown".into(), LV::I(n.triangle_upside_down as i32));
        }
        if ln.fields.contains_key("rightTriangleDirection") && ln.i("rightTriangleDirection").signum() != n.right_triangle {
            ln.fields.insert("rightTriangleDirection".into(), LV::I(n.right_triangle));
        }
        if ln.fields.contains_key("gradient") && (ln.i("gradient") != 0) != n.gradient {
            ln.fields.insert("gradient".into(), LV::I(n.gradient as i32));
        }
        let legacy_reverse = n.gradient_reverse ^ (ln.i("type") == 0);
        if ln.fields.contains_key("reverseGradient") && (ln.i("reverseGradient") != 0) != legacy_reverse {
            ln.fields.insert("reverseGradient".into(), LV::I(legacy_reverse as i32));
        }
        if ln.fields.contains_key("gradientMode") && (ln.i("gradientMode") == 1) != n.gradient_sideways {
            ln.fields.insert("gradientMode".into(), LV::I(n.gradient_sideways as i32));
        }
        sync_flag(&mut ln, "static", n.is_static);
        sync_flag(&mut ln, "stretchy", n.stretchy);
        sync_flag(&mut ln, "smartStretch", n.smart_stretch);
        sync_flag(&mut ln, "noSmartStretch", n.no_smart_stretch);
        if ln.fields.contains_key("defaultLength") {
            set_if_changed(&mut ln, "defaultLength", n.rest_length);
        }
        sync_flag(&mut ln, "circleHollow", n.circle_hollow);
        sync_flag(&mut ln, "circleOutline", n.circle_outline);
        if ln.fields.contains_key("circleOutlineColor") && Value::C(legacy_color(ln.i("circleOutlineColor"))) != n.outline_color.value {
            ln.fields.insert("circleOutlineColor".into(), LV::I(to_legacy_color(&n.outline_color.value.color())));
        }
        sync_flag(&mut ln, "halfArc", n.half_arc != 0);
        if ln.fields.contains_key("polygonVertices") && ln.i("polygonVertices") != n.polygon_sides {
            ln.fields.insert("polygonVertices".into(), LV::I(n.polygon_sides));
        }
        if trapezoid_ratio(&ln) != n.trapezoid_ratio {
            if ln.fields.contains_key("trapezoidTopRatio") {
                ln.fields.insert("trapezoidTopRatio".into(), LV::F(n.trapezoid_ratio.to_bits()));
            }
            if ln.fields.contains_key("trapezoidEndF") && ln.f("trapezoidStartF") > 0.0 {
                let v = ln.f("trapezoidStartF") * n.trapezoid_ratio;
                ln.fields.insert("trapezoidEndF".into(), LV::F(v.to_bits()));
            }
        }
        sync_flag(&mut ln, "trapezoidRoundedStart", n.trapezoid_round_start);
        sync_flag(&mut ln, "trapezoidRoundedEnd", n.trapezoid_round_end);
        if ln.fields.contains_key("gradientColor") && Value::C(legacy_color(ln.i("gradientColor"))) != n.gradient_color.value {
            ln.fields.insert("gradientColor".into(), LV::I(to_legacy_color(&n.gradient_color.value.color())));
        }
        let thick = java_round(n.thickness.value.f());
        if ln.i("thickness") != thick {
            ln.fields.insert("thickness".into(), LV::I(thick));
        }
        let c = n.color.value.color();
        if ln.i("useSegmentColor") != 0 {
            if legacy_color(ln.i("color")) != c {
                ln.fields.insert("color".into(), LV::I(to_legacy_color(&c)));
            }
        } else if c != fig_color_model {
            ln.fields.insert("useSegmentColor".into(), LV::I(1));
            ln.fields.insert("color".into(), LV::I(to_legacy_color(&c)));
        }
        let world = if is_root { 0.0 } else { world_angle.get(n.parent.as_deref().unwrap_or("")).copied().unwrap_or(0.0) } + n.rotation.value.f() as f64;
        world_angle.insert(n.id.clone(), world);
        if !is_root {
            refresh_offset(&mut ln, n, world);
        }
        fig.nodes.push(ln);
        let idx = fig.nodes.len() - 1;
        by_id.insert(n.id.clone(), idx);
        if let Some(pid) = &n.parent {
            let pi = by_id[pid];
            fig.nodes[pi].children.push(idx);
        }
    }
    let draw_index = |nid: &str| -> R<i32> {
        match figure.node(nid) {
            Some(n) => Ok(if n.parent.is_none() { 0 } else { n.draw_order }),
            None => err(format!("polyfill refers to missing node {nid}")),
        }
    };
    for mp in &figure.polyfills {
        let mut p = LegacyPolyfill { anchor: draw_index(&mp.anchor)?, color: to_legacy_color(&mp.color.value.color()), use_color: mp.use_own_color as i32, nodes: Vec::new() };
        for nid in &mp.nodes {
            p.nodes.push(draw_index(nid)?);
        }
        fig.polyfills.push(p);
    }
    if let Some(cn) = ex_get(root, "fig.connections").and_then(|s| s.strip_prefix("s:")) {
        for part in cn.split(';').filter(|s| !s.is_empty()) {
            let mut q = part.split(',');
            let a = q.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            let b = q.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            fig.connections.push((a, b));
        }
    }
    Ok(fig)
}

// ---------------------------------------------------------------- .nodemc

const MAGIC: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
pub const VERIFIED_VERSION: i32 = 330;
const FIGURE_HEADER_BYTES: usize = 97;
const FRAME_TRAILER_BYTES: usize = 4;
const NODE_RECORD_BYTES: usize = 58;

fn be32(b: &[u8], p: usize) -> i32 {
    i32::from_be_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]])
}

fn bef(b: &[u8], p: usize) -> f32 {
    f32::from_bits(be32(b, p) as u32)
}

#[derive(Clone, Debug)]
pub struct NamedFigure {
    pub name: String,
    pub figure: LegacyFigure,
}

#[derive(Clone, Debug)]
pub struct FrameFigure {
    pub library_id: usize,
    pub header: Vec<u8>,
    pub nodes: Vec<Vec<u8>>,
    /// Movieclip version 4xx layout (131-byte header, other offsets).
    pub v4: bool,
}

impl FrameFigure {
    pub fn instance_id(&self) -> i32 {
        be32(&self.header, 0)
    }

    pub fn scale(&self) -> f32 {
        bef(&self.header, if self.v4 { 8 } else { 4 })
    }

    pub fn opacity(&self) -> f32 {
        bef(&self.header, if self.v4 { 12 } else { 8 })
    }

    pub fn stage_x(&self) -> f32 {
        bef(&self.header, if self.v4 { 91 } else { 67 })
    }

    pub fn stage_y(&self) -> f32 {
        bef(&self.header, if self.v4 { 95 } else { 71 })
    }

    pub fn color(&self) -> u32 {
        be32(&self.header, if self.v4 { 99 } else { 75 }) as u32
    }

    /// A joint's local angle in its record.
    pub fn node_angle(&self, r: &[u8]) -> f32 {
        bef(r, if self.v4 { 19 } else { 23 })
    }
}

/// Movieclip version 431 (Stick Nodes 4.3.x), measured 2026-11-21 on a clip
/// saved by the original: per figure a 131-byte header, 83-byte node records
/// (connectors: 20 more bytes after), 1-byte frame trailer.
pub const V4_VERSION: i32 = 431;
const V4_FIGURE_HEADER_BYTES: usize = 131;
const V4_NODE_RECORD_BYTES: usize = 83;
const V4_CONNECTOR_PREFIX_BYTES: usize = 20;
const V4_FRAME_TRAILER_BYTES: usize = 1;

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub sound_volume: u32,
    pub sound_pan: u32,
    pub sound_pitch: u32,
    pub sound_id: i32,
    pub figures: Vec<FrameFigure>,
    pub trailer: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct LegacyMovieclip {
    pub version: i32,
    pub name: String,
    pub preview_count: i32,
    pub preview_width: i32,
    pub preview_height: i32,
    pub preview: Vec<u8>,
    pub fps: i32,
    pub tweened_frames: i32,
    pub tweening: i32,
    pub loop_protection: i32,
    pub unique_figure_id: i32,
    pub library: Vec<NamedFigure>,
    pub sounds: Vec<String>,
    pub frames: Vec<Frame>,
}

/// Unsupported layout (as opposed to a damaged file).
pub fn is_unsupported(e: &LegacyError) -> bool {
    e.0.contains("not supported yet")
}

fn unpack(file: &[u8]) -> R<Vec<u8>> {
    if file.len() < MAGIC.len() + 2 || file[..MAGIC.len()] != MAGIC {
        return err("not a movieclip");
    }
    let mut g = flate2::read::GzDecoder::new(&file[MAGIC.len()..]).take(512 * 1024 * 1024 + 1);
    let mut out = Vec::new();
    g.read_to_end(&mut out).map_err(|e| LegacyError(e.to_string()))?;
    if out.len() > 512 * 1024 * 1024 {
        return err("movieclip too large");
    }
    Ok(out)
}

pub fn read_movieclip(file: &[u8]) -> R<LegacyMovieclip> {
    let body = unpack(file)?;
    let mut inp = In { b: &body, little: false, pos: 0 };
    let mc = read_mc_body(&mut inp)?;
    if !inp.at_end() {
        return err(format!("trailing data at {} of {}", inp.pos, body.len()));
    }
    Ok(mc)
}

fn read_mc_body(inp: &mut In) -> R<LegacyMovieclip> {
    let mut mc = LegacyMovieclip { version: inp.i32()?, ..Default::default() };
    let v4 = mc.version == V4_VERSION;
    if mc.version != VERIFIED_VERSION && !v4 {
        return err(format!("movieclip version {} is not supported yet (verified: {VERIFIED_VERSION}, {V4_VERSION})", mc.version));
    }
    if v4 {
        let _build = inp.i32()?;
    }
    mc.name = inp.str()?;
    let frames = inp.count(20)?;
    let figs = inp.count(16)?;
    let sounds = inp.count(4)?;
    if v4 {
        let extra = inp.i32()?;
        if extra != 0 {
            return err(format!("movieclip version {}: sprites in movieclips are not supported yet", mc.version));
        }
    }
    mc.preview_count = inp.i32()?;
    mc.preview_width = inp.i32()?;
    mc.preview_height = inp.i32()?;
    let px = mc.preview_count as i64 * mc.preview_width as i64 * mc.preview_height as i64 * 4;
    if mc.preview_count < 0 || mc.preview_width < 0 || mc.preview_height < 0 || px > (inp.b.len() - inp.pos) as i64 {
        return err("bad preview size");
    }
    mc.preview = inp.take(px as usize)?;
    mc.fps = inp.i32()?;
    mc.tweened_frames = inp.i32()?;
    mc.tweening = inp.u8()?;
    mc.loop_protection = inp.u8()?;
    mc.unique_figure_id = inp.i32()?;
    for _ in 0..figs {
        let name = inp.str()?;
        let figure = read_figure(inp)?;
        mc.library.push(NamedFigure { name, figure });
    }
    for _ in 0..sounds {
        mc.sounds.push(inp.str()?);
    }
    for _ in 0..frames {
        let mut fr = Frame { sound_volume: inp.i32()? as u32, sound_pan: inp.i32()? as u32, sound_pitch: inp.i32()? as u32, sound_id: inp.i32()?, ..Default::default() };
        let assets = inp.count(4 + FIGURE_HEADER_BYTES)?;
        for _ in 0..assets {
            let lib = inp.i32()?;
            if lib < 0 || lib as usize >= mc.library.len() {
                return err(format!("bad library id {lib}"));
            }
            let header = inp.take(if v4 { V4_FIGURE_HEADER_BYTES } else { FIGURE_HEADER_BYTES })?;
            let lib_nodes = &mc.library[lib as usize].figure.nodes;
            let mut nodes = Vec::with_capacity(lib_nodes.len());
            for ln in lib_nodes.iter() {
                if v4 {
                    nodes.push(inp.take(V4_NODE_RECORD_BYTES)?);
                    if ln.is_connector {
                        inp.take(V4_CONNECTOR_PREFIX_BYTES)?;
                    }
                } else {
                    nodes.push(inp.take(NODE_RECORD_BYTES)?);
                }
            }
            fr.figures.push(FrameFigure { library_id: lib as usize, header, nodes, v4 });
        }
        fr.trailer = inp.take(if v4 { V4_FRAME_TRAILER_BYTES } else { FRAME_TRAILER_BYTES })?;
        mc.frames.push(fr);
    }
    Ok(mc)
}

/// Legacy movieclip to a symbol: one figure layer per instance, every frame
/// keying the pose, position, scale, opacity and visibility.
pub fn movieclip_to_symbol(p: &mut Project, mc: &LegacyMovieclip, report: &mut Report) -> String {
    const STAGE_W: f32 = 1920.0;
    const STAGE_H: f32 = 1080.0;
    let fps = if mc.fps > 0 { mc.fps as f64 } else { 10.0 };
    let sid = new_id(p, "S");
    let mut sym = Symbol::new(&sid, Some(&mc.name), mc.frames.len() as f64 / fps);
    let interp = if mc.tweening != 0 { Interp::Linear } else { Interp::Hold };
    let mut layers: IndexMap<i32, Layer> = IndexMap::new();
    // the symbol is not in the library yet: ids made below must not collide with it
    p.add_symbol(Symbol::new(&sid, None, 0.0));
    for fr in &mc.frames {
        for ff in &fr.figures {
            if layers.contains_key(&ff.instance_id()) {
                continue;
            }
            let nf = &mc.library[ff.library_id];
            let l = to_layer(p, &nf.figure, &nf.name, report);
            layers.insert(ff.instance_id(), l);
        }
    }
    let key = |prop: &mut Property, t: f64, v: f32| prop.track.put(Key::new(t, Value::F(v), interp));
    for (f, fr) in mc.frames.iter().enumerate() {
        let t = f as f64 / fps;
        let mut present = std::collections::HashSet::new();
        for ff in &fr.figures {
            present.insert(ff.instance_id());
            let lib_fig = &mc.library[ff.library_id].figure;
            let lib = lib_fig.all();
            let l = layers.get_mut(&ff.instance_id()).expect("made above");
            l.shown.track.put(Key::new(t, Value::B(true), Interp::Hold));
            key(&mut l.transform.x, t, ff.stage_x() - STAGE_W / 2.0);
            key(&mut l.transform.y, t, ff.stage_y() - STAGE_H / 2.0);
            key(&mut l.transform.scale_x, t, ff.scale());
            key(&mut l.transform.scale_y, t, ff.scale());
            key(&mut l.opacity, t, ff.opacity());
            let fig_color = legacy_color(ff.color() as i32);
            let LayerKind::Figure(fig) = &mut l.kind else { continue };
            let nodes = fig.nodes_mut();
            for i in 0..nodes.len().min(ff.nodes.len()) {
                let r = &ff.nodes[i];
                let n = &mut nodes[i];
                let ln = &lib_fig.nodes[lib[i]];
                key(&mut n.rotation, t, ff.node_angle(r));
                key(&mut n.length, t, bef(r, 11));
                if ln.i("useSegmentScale") != 0 {
                    key(&mut n.scale, t, bef(r, 7));
                }
                key(&mut n.thickness, t, be32(r, 15) as f32);
                let c = if ln.i("useSegmentColor") != 0 { legacy_color(be32(r, 31)) } else { fig_color };
                n.color.track.put(Key::new(t, Value::C(c), interp));
                if n.circle_outline {
                    n.outline_color.track.put(Key::new(t, Value::C(legacy_color(be32(r, 39))), interp));
                }
                if n.gradient {
                    n.gradient_color.track.put(Key::new(t, Value::C(legacy_color(be32(r, 35))), interp));
                }
            }
        }
        for (id, l) in layers.iter_mut() {
            if !present.contains(id) {
                l.shown.track.put(Key::new(t, Value::B(false), Interp::Hold));
            }
        }
    }
    sym.layers.extend(layers.into_values().map(std::sync::Arc::new));
    p.add_symbol(sym);
    sid
}

// ---------------------------------------------------------------- .stknds

#[derive(Clone, Debug, Default)]
pub struct StkndsHeader {
    pub version: i32,
    pub build: i32,
    pub name: String,
    pub fps: i32,
    pub tweened_frames: i32,
    pub tweening: bool,
    pub looping: bool,
    pub figures: Vec<NamedFigure>,
}

fn skip_ints(inp: &mut In) -> R<()> {
    let n = inp.count(4)?;
    inp.pos += 4 * n;
    Ok(())
}

/// Header and stickfigure library of a .stknds project (its frames are not decoded yet).
pub fn read_stknds_header(file: &[u8]) -> R<StkndsHeader> {
    let body = unpack(file)?;
    let mut inp = In { b: &body, little: false, pos: 0 };
    let mut h = StkndsHeader::default();
    let v = inp.i32()?;
    h.version = v;
    if !(0..=MAX_VERSION).contains(&v) {
        return err(format!("Stick Nodes project version {v} is not supported yet"));
    }
    let b = if v >= 401 { inp.i32()? } else { 0 };
    h.build = b;
    h.name = inp.str()?;
    if v >= 401 && b >= 16 {
        inp.u8()?;
    }
    if v >= 244 {
        inp.u8()?;
    }
    if v >= 220 {
        let n = inp.i32()?;
        if n > 0 {
            if n as usize > inp.b.len() - inp.pos {
                return err("bad watermark length");
            }
            inp.pos += n as usize;
        }
        inp.i32()?;
    }
    h.fps = inp.i32()?;
    if v >= 176 {
        h.tweened_frames = inp.i32()?;
    }
    if v >= 127 {
        inp.u8()?;
    }
    if v >= 113 {
        inp.u8()?;
    }
    inp.u8()?;
    if v >= 220 {
        inp.u8()?;
    }
    h.tweening = inp.u8()? != 0;
    if v >= 127 {
        h.looping = inp.u8()? != 0;
    }
    if (140..200).contains(&v) {
        inp.u8()?;
    }
    inp.i32()?;
    if v >= 176 {
        inp.i32()?;
    }
    if v >= 300 {
        inp.i32()?;
        inp.i32()?;
    }
    if v >= 400 {
        let fonts = inp.count(1)?;
        inp.pos += fonts;
    }
    if v >= 300 {
        skip_ints(&mut inp)?;
        skip_ints(&mut inp)?;
    }
    if v >= 400 {
        skip_ints(&mut inp)?;
    }
    if v >= 300 {
        let dest = inp.count(8)?;
        for _ in 0..dest {
            inp.i32()?;
            let src = inp.count(8)?;
            for _ in 0..src {
                inp.i32()?;
                let pairs = inp.count(8)?;
                inp.pos += 8 * pairs;
            }
        }
    }
    if (140..400).contains(&v) {
        inp.u8()?;
        if v < 170 {
            inp.u8()?;
        } else {
            inp.i32()?;
            inp.i32()?;
        }
        let (w, hh) = (inp.i32()?, inp.i32()?);
        let px = w as i64 * hh as i64 * 3;
        if w < 0 || hh < 0 || px > (inp.b.len() - inp.pos) as i64 {
            return err("bad background image");
        }
        inp.pos += px as usize;
    }
    let figs = inp.count(8)?;
    for _ in 0..figs {
        let name = inp.str()?;
        let figure = read_figure(&mut inp)?;
        h.figures.push(NamedFigure { name, figure });
    }
    Ok(h)
}
