//! The evaluated scene: groups of shapes in view space, y up.

use super::affine::Affine;
use crate::model::{Blend, Color};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Cycle,
    MissingRef,
    RecursionLimit,
    DepthCap,
    PrimitiveBudget,
    ParentCycle,
    BadValue,
    ResolveBudget,
}

impl DiagnosticKind {
    pub fn name(self) -> &'static str {
        match self {
            DiagnosticKind::Cycle => "CYCLE",
            DiagnosticKind::MissingRef => "MISSING_REF",
            DiagnosticKind::RecursionLimit => "RECURSION_LIMIT",
            DiagnosticKind::DepthCap => "DEPTH_CAP",
            DiagnosticKind::PrimitiveBudget => "PRIMITIVE_BUDGET",
            DiagnosticKind::ParentCycle => "PARENT_CYCLE",
            DiagnosticKind::BadValue => "BAD_VALUE",
            DiagnosticKind::ResolveBudget => "RESOLVE_BUDGET",
        }
    }
}

/// A problem found while evaluating (evaluation always finishes).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub subject: String,
}

/// Linear (p0 to p1) or radial (centre p0, first axis to p1, r2 across) ramp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    pub radial: bool,
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub r2: f32,
    pub end: Color,
}

impl Gradient {
    pub fn linear(x0: f32, y0: f32, x1: f32, y1: f32, end: Color) -> Gradient {
        Gradient { radial: false, x0, y0, x1, y1, r2: 0.0, end }
    }

    pub fn radial(cx: f32, cy: f32, r: f32, end: Color) -> Gradient {
        Gradient { radial: true, x0: cx, y0: cy, x1: cx + r, y1: cy, r2: r, end }
    }

    pub fn elliptical(cx: f32, cy: f32, ax: f32, ay: f32, rb: f32, end: Color) -> Gradient {
        Gradient { radial: true, x0: cx, y0: cy, x1: cx + ax, y1: cy + ay, r2: rb, end }
    }

    /// Ramp position 0..1 at a point.
    pub fn at(&self, x: f32, y: f32) -> f32 {
        let (dx, dy) = (self.x1 - self.x0, self.y1 - self.y0);
        let l2 = dx * dx + dy * dy;
        let t: f32 = if self.radial {
            if l2 <= 0.0 || self.r2 <= 0.0 {
                0.0
            } else {
                let u = (((x - self.x0) * dx + (y - self.y0) * dy) / l2) as f64;
                let v = ((-(x - self.x0) * dy + (y - self.y0) * dx) as f64) / (l2 as f64).sqrt() / self.r2 as f64;
                u.hypot(v) as f32
            }
        } else if l2 <= 0.0 {
            0.0
        } else {
            ((x - self.x0) * dx + (y - self.y0) * dy) / l2
        };
        t.clamp(0.0, 1.0)
    }
}

/// Bulge height of a curved segment per unit of Node.curve (fitted, NOTES.md).
pub const CURVE_SAGITTA: f32 = 0.9;

#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    /// Round-capped segment; bend in degrees makes it an arc.
    Line { x1: f32, y1: f32, x2: f32, y2: f32, width: f32, bend: f32 },
    /// Filled sector from start over span (radians).
    Circle { cx: f32, cy: f32, r: f32, start: f32, span: f32 },
    /// Band of the given width centred on radius r.
    Ring { cx: f32, cy: f32, r: f32, width: f32, start: f32, span: f32 },
    /// Filled polygon; convex = fan, else ear clipping.
    Polygon { points: Vec<f32>, convex: bool },
    /// Textured quad, corners BL, BR, TR, TL.
    Quad { asset: String, corners: [f32; 8] },
    /// Text laid out by the tessellator.
    Text { text: String, font: Option<String>, size: f32, xf: Affine, corner: i32, align: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub color: Color,
    pub source: String,
    pub gradient: Option<Gradient>,
    pub kind: ShapeKind,
}

impl Shape {
    pub fn new(kind: ShapeKind, color: Color, source: impl Into<String>) -> Shape {
        Shape { color, source: source.into(), gradient: None, kind }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    F(f32),
    D(f64),
    C(Color),
    B(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedEffect {
    pub ty: String,
    pub mix: f32,
    pub params: Vec<(String, ParamValue)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Shape(Shape),
    Group(Group),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub source: String,
    pub opacity: f32,
    pub blend: Blend,
    /// Adjustment layers: effects apply to everything before this group.
    pub adjustment: bool,
    /// A layer group with a blend: composited as one picture with that blend
    /// (a single layer's blend goes on each of its shapes, as Stick Nodes does).
    pub whole: bool,
    /// Optional explicit world-space bounds `[min_x, min_y, max_x, max_y]`
    /// (used to clip Adjustment layers to the camera shot in stage view).
    pub bounds: Option<[f32; 4]>,
    pub effects: Vec<ResolvedEffect>,
    pub children: Vec<Item>,
}

impl Group {
    pub fn new(source: impl Into<String>) -> Group {
        Group { source: source.into(), opacity: 1.0, blend: Blend::Normal, adjustment: false, whole: false, bounds: None, effects: Vec::new(), children: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub root: Group,
    pub time: f64,
    pub diagnostics: Vec<Diagnostic>,
    pub primitives: i32,
}

impl Scene {
    pub fn has(&self, k: DiagnosticKind) -> bool {
        self.diagnostics.iter().any(|d| d.kind == k)
    }
}
