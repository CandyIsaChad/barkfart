//! Scene to triangles (port of Java Tessellator, EarClip, DrawList).
//! Group opacity multiplies into vertex alpha on the flat path; `layered`
//! also builds the isolate tree (groups drawn offscreen and composited once).

use crate::eval::{Affine, Gradient, Group, Item, ResolvedEffect, Scene, Shape, ShapeKind};
use crate::model::{Blend, Color};
use std::f64::consts::PI;

/// Chord tolerance for round shapes, in pixels.
const TOLERANCE: f32 = 0.35;

/// Glyph metrics and atlas coordinates (a font rasterized at base_px).
pub trait GlyphSource {
    fn base_px(&self) -> f32;
    fn line_height(&self) -> f32;
    fn glyph(&self, c: char) -> Option<Glyph>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub texture: String,
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    /// Offset from the pen at the baseline (y up), size, advance; base pixels.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub advance: f32,
}

/// One draw call's worth of triangles: same blend, effect group and texture.
#[derive(Clone, Debug, Default)]
pub struct Batch {
    pub blend: Option<Blend>,
    pub effect_group: Option<String>,
    pub texture: Option<String>,
    pub xy: Vec<f32>,
    pub uv: Vec<f32>,
    /// Straight RGBA8888 per vertex.
    pub rgba: Vec<u32>,
    owner: Option<usize>,
}

impl Batch {
    pub fn vertex_count(&self) -> usize {
        self.rgba.len()
    }

    pub fn blend(&self) -> Blend {
        self.blend.unwrap_or(Blend::Normal)
    }
}

/// A group drawn in isolation (layered lists).
#[derive(Clone, Debug)]
pub struct Isolate {
    pub source: String,
    pub opacity: f32,
    pub blend: Blend,
    pub adjustment: bool,
    pub effects: Vec<ResolvedEffect>,
    /// Batches (by index into DrawList::batches) and isolates, in draw order.
    pub items: Vec<IsoItem>,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    parent: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsoItem {
    Batch(usize),
    Isolate(usize),
}

impl Isolate {
    pub fn empty(&self) -> bool {
        !(self.min_x <= self.max_x && self.min_y <= self.max_y)
    }

    fn include(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        if !(x0 <= x1) || !(y0 <= y1) {
            return;
        }
        self.min_x = self.min_x.min(x0);
        self.min_y = self.min_y.min(y0);
        self.max_x = self.max_x.max(x1);
        self.max_y = self.max_y.max(y1);
    }
}

#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub batches: Vec<Batch>,
    /// Isolates; index 0 is the root (layered lists only).
    pub isolates: Vec<Isolate>,
    current: Option<usize>,
}

impl DrawList {
    /// View options "Show filters": drops the effects `keep` refuses (on the
    /// stage only; None drops every filter, No blur the two blurs).
    pub fn without_effects(mut self, keep: impl Fn(&str) -> bool) -> DrawList {
        for i in &mut self.isolates {
            i.effects.retain(|e| keep(&e.ty));
        }
        self
    }

    pub fn triangle_count(&self) -> usize {
        self.batches.iter().map(|b| b.vertex_count() / 3).sum()
    }

    pub fn root(&self) -> Option<&Isolate> {
        self.isolates.first()
    }

    /// Puts another layered list (onion ghosts) underneath this one's
    /// contents: its root becomes the first child of this root.
    /// Like with_underlay, but drawn on top (onion skin in front).
    pub fn with_overlay(self, over: DrawList) -> DrawList {
        let n = self.isolates.first().map(|r| r.items.len()).unwrap_or(0);
        let mut out = self.with_underlay(over);
        if let Some(root) = out.isolates.first_mut() {
            if root.items.len() > n {
                let first = root.items.remove(0);
                root.items.push(first);
            }
        }
        out
    }

    pub fn with_underlay(mut self, under: DrawList) -> DrawList {
        if self.isolates.is_empty() || under.isolates.is_empty() || under.isolates[0].empty() {
            return self;
        }
        let (bo, io) = (self.batches.len(), self.isolates.len());
        for mut b in under.batches {
            b.owner = b.owner.map(|o| o + io);
            self.batches.push(b);
        }
        let (x0, y0, x1, y1) = (under.isolates[0].min_x, under.isolates[0].min_y, under.isolates[0].max_x, under.isolates[0].max_y);
        for mut iso in under.isolates {
            iso.parent = Some(iso.parent.map(|p| p + io).unwrap_or(0));
            for it in iso.items.iter_mut() {
                *it = match *it {
                    IsoItem::Batch(b) => IsoItem::Batch(b + bo),
                    IsoItem::Isolate(i) => IsoItem::Isolate(i + io),
                };
            }
            self.isolates.push(iso);
        }
        self.isolates[0].include(x0, y0, x1, y1);
        self.isolates[0].items.insert(0, IsoItem::Isolate(io));
        self
    }

    pub(crate) fn open(&mut self, source: &str, opacity: f32, blend: Blend, adjustment: bool, effects: Vec<ResolvedEffect>) {
        let i = self.isolates.len();
        self.isolates.push(Isolate {
            source: source.into(),
            opacity,
            blend,
            adjustment,
            effects,
            items: Vec::new(),
            min_x: f32::INFINITY,
            min_y: f32::INFINITY,
            max_x: f32::NEG_INFINITY,
            max_y: f32::NEG_INFINITY,
            parent: self.current,
        });
        if let Some(c) = self.current {
            self.isolates[c].items.push(IsoItem::Isolate(i));
        }
        self.current = Some(i);
    }

    pub(crate) fn close(&mut self) {
        let done = self.current.expect("open isolate");
        self.current = self.isolates[done].parent;
        if let Some(c) = self.current {
            let d = &self.isolates[done];
            if !d.empty() {
                let (a, b, x, y) = (d.min_x, d.min_y, d.max_x, d.max_y);
                self.isolates[c].include(a, b, x, y);
            }
        }
    }

    /// The batch to append to for (blend, effect group, texture).
    pub(crate) fn batch(&mut self, blend: Blend, effects: Option<&str>, texture: Option<&str>) -> usize {
        let same = |b: &Batch| b.blend() == blend && b.effect_group.as_deref() == effects && b.texture.as_deref() == texture;
        if let Some(c) = self.current {
            if let Some(IsoItem::Batch(last)) = self.isolates[c].items.last() {
                if same(&self.batches[*last]) {
                    return *last;
                }
            }
            let i = self.batches.len();
            self.batches.push(Batch {
                blend: Some(blend),
                effect_group: effects.map(Into::into),
                texture: texture.map(Into::into),
                owner: Some(c),
                ..Default::default()
            });
            self.isolates[c].items.push(IsoItem::Batch(i));
            return i;
        }
        if let Some(last) = self.batches.last() {
            if same(last) {
                return self.batches.len() - 1;
            }
        }
        self.batches.push(Batch { blend: Some(blend), effect_group: effects.map(Into::into), texture: texture.map(Into::into), ..Default::default() });
        self.batches.len() - 1
    }

    pub(crate) fn add(&mut self, b: usize, x: f32, y: f32, c: u32, u: f32, v: f32) {
        let batch = &mut self.batches[b];
        if let Some(o) = batch.owner {
            if x.is_finite() && y.is_finite() {
                self.isolates[o].include(x, y, x, y);
            }
        }
        let batch = &mut self.batches[b];
        batch.xy.push(x);
        batch.xy.push(y);
        batch.uv.push(u);
        batch.uv.push(v);
        batch.rgba.push(c);
    }

    #[allow(clippy::too_many_arguments)]
    fn tri(&mut self, b: usize, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, c: u32) {
        self.add(b, x1, y1, c, 0.0, 0.0);
        self.add(b, x2, y2, c, 0.0, 0.0);
        self.add(b, x3, y3, c, 0.0, 0.0);
    }
}

/// Flat triangles (no isolates).
pub fn tessellate(scene: &Scene, glyphs: Option<&dyn GlyphSource>) -> DrawList {
    let mut out = DrawList::default();
    walk(&scene.root, 1.0, Blend::Normal, None, &mut out, glyphs);
    out
}

/// Triangles plus the isolate tree: groups with effects, adjustment groups
/// and groups below full opacity are drawn offscreen and composited once.
pub fn layered(scene: &Scene, glyphs: Option<&dyn GlyphSource>) -> DrawList {
    let mut out = DrawList::default();
    out.open(&scene.root.source, 1.0, Blend::Normal, false, Vec::new());
    layered_walk(&scene.root, 1.0, Blend::Normal, &mut out, true, glyphs);
    out.close();
    out
}

pub fn isolates(g: &Group) -> bool {
    g.adjustment || g.opacity < 1.0 || g.effects.iter().any(|e| e.mix > 0.0)
}

fn layered_walk(g: &Group, opacity: f32, blend: Blend, out: &mut DrawList, is_root: bool, glyphs: Option<&dyn GlyphSource>) {
    if !is_root && isolates(g) {
        let gop = g.opacity;
        if !(gop > 0.0) {
            return;
        }
        let fx: Vec<ResolvedEffect> = g.effects.iter().filter(|e| e.mix > 0.0).cloned().collect();
        out.open(&g.source, (gop * opacity).min(1.0), g.blend, g.adjustment, fx);
        for o in &g.children {
            match o {
                Item::Group(c) => layered_walk(c, 1.0, Blend::Normal, out, false, glyphs),
                Item::Shape(s) => emit(s, 1.0, Blend::Normal, None, out, glyphs),
            }
        }
        out.close();
        return;
    }
    let op = opacity * if is_root { 1.0 } else { g.opacity };
    if op <= 0.0 {
        return;
    }
    let b = if g.blend != Blend::Normal { g.blend } else { blend };
    for o in &g.children {
        match o {
            Item::Group(c) => layered_walk(c, op, b, out, false, glyphs),
            Item::Shape(s) => emit(s, op, b, None, out, glyphs),
        }
    }
}

fn walk(g: &Group, opacity: f32, blend: Blend, fx: Option<&str>, out: &mut DrawList, glyphs: Option<&dyn GlyphSource>) {
    let op = opacity * g.opacity;
    if op <= 0.0 {
        return;
    }
    let b = if g.blend != Blend::Normal { g.blend } else { blend };
    let effects = if !g.effects.is_empty() { Some(g.source.as_str()) } else { fx };
    for o in &g.children {
        match o {
            Item::Group(c) => walk(c, op, b, effects, out, glyphs),
            Item::Shape(s) => emit(s, op, b, effects, out, glyphs),
        }
    }
}

fn emit(s: &Shape, op: f32, b: Blend, effects: Option<&str>, out: &mut DrawList, glyphs: Option<&dyn GlyphSource>) {
    if let ShapeKind::Text { text: t, size, xf, corner, align, .. } = &s.kind {
        if let Some(gs) = glyphs {
            text(gs, t, *size, xf, *corner, *align, &s.color, op, b, effects, out);
        }
        return;
    }
    let texture = match &s.kind {
        ShapeKind::Quad { asset, .. } => Some(asset.as_str()),
        _ => None,
    };
    let bi = out.batch(b, effects, texture);
    let first = out.batches[bi].vertex_count();
    let c = s.color;
    let rgba = Color::new(c.r, c.g, c.b, c.a * op).to_rgba8888();
    if rgba & 0xff == 0 && s.gradient.is_none() {
        return;
    }
    match &s.kind {
        ShapeKind::Line { x1, y1, x2, y2, width, bend } => line(*x1, *y1, *x2, *y2, *width, *bend, rgba, bi, out),
        ShapeKind::Circle { cx, cy, r, start, span } => fan(*cx, *cy, *r, *start, *span, rgba, bi, out),
        ShapeKind::Ring { cx, cy, r, width, start, span } => ring(*cx, *cy, *r, *width, *start, *span, rgba, bi, out),
        ShapeKind::Polygon { points: q, convex } => {
            if *convex && s.gradient.is_some_and(|g| g.radial) {
                let n = q.len() / 2;
                let (mut cx, mut cy) = (0f32, 0f32);
                for i in 0..n {
                    cx += q[i * 2];
                    cy += q[i * 2 + 1];
                }
                cx /= n as f32;
                cy /= n as f32;
                for i in 0..n {
                    let j = (i + 1) % n;
                    out.tri(bi, cx, cy, q[i * 2], q[i * 2 + 1], q[j * 2], q[j * 2 + 1], rgba);
                }
            } else if *convex {
                let mut i = 2;
                while i + 3 < q.len() {
                    out.tri(bi, q[0], q[1], q[i], q[i + 1], q[i + 2], q[i + 3], rgba);
                    i += 2;
                }
            } else {
                let idx = ear_clip(q);
                let mut i = 0;
                while i + 2 < idx.len() {
                    let (a, b2, c2) = (idx[i] * 2, idx[i + 1] * 2, idx[i + 2] * 2);
                    out.tri(bi, q[a], q[a + 1], q[b2], q[b2 + 1], q[c2], q[c2 + 1], rgba);
                    i += 3;
                }
            }
        }
        ShapeKind::Quad { corners: q, .. } => {
            let ex = q[0] + q[4] - q[2] - q[6];
            let ey = q[1] + q[5] - q[3] - q[7];
            let span = (q[4] - q[0]).abs() + (q[5] - q[1]).abs() + (q[6] - q[2]).abs() + (q[7] - q[3]).abs();
            let n = if ex.abs() + ey.abs() <= 1e-4 * span.max(1.0) { 1 } else { 12 };
            for j in 0..n {
                for i in 0..n {
                    let (u0, u1, v0, v1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32, j as f32 / n as f32, (j + 1) as f32 / n as f32);
                    for (u, v) in [(u0, v0), (u1, v0), (u1, v1), (u0, v0), (u1, v1), (u0, v1)] {
                        let (bx, by) = (q[0] + (q[2] - q[0]) * u, q[1] + (q[3] - q[1]) * u);
                        let (tx, ty) = (q[6] + (q[4] - q[6]) * u, q[7] + (q[5] - q[7]) * u);
                        out.add(bi, bx + (tx - bx) * v, by + (ty - by) * v, rgba, u, 1.0 - v);
                    }
                }
            }
        }
        ShapeKind::Text { .. } => {}
    }
    if let Some(g) = &s.gradient {
        recolor(&mut out.batches[bi], first, &c, g, op);
    }
}

fn recolor(b: &mut Batch, first: usize, c0: &Color, g: &Gradient, op: f32) {
    for i in first..b.vertex_count() {
        let t = g.at(b.xy[i * 2], b.xy[i * 2 + 1]);
        let m = c0.lerp(&g.end, t);
        b.rgba[i] = Color::new(m.r, m.g, m.b, m.a * op).to_rgba8888();
    }
}

/// Segments for an arc of radius r and angle span.
pub fn segments(r: f32, span: f32) -> i32 {
    if r <= TOLERANCE {
        return 3;
    }
    let step = 2.0 * ((1.0 - TOLERANCE / r) as f64).acos();
    let n = (span as f64 / step).ceil() as i32;
    3.max(n.min(256))
}

#[allow(clippy::too_many_arguments)]
fn line(x1: f32, y1: f32, x2: f32, y2: f32, width: f32, bend: f32, c: u32, b: usize, out: &mut DrawList) {
    let hw = width / 2.0;
    if hw <= 0.0 {
        return;
    }
    if bend.abs() > 0.01 {
        arc(x1, y1, x2, y2, bend, hw, c, b, out);
        return;
    }
    let (dx, dy) = (x2 - x1, y2 - y1);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        fan(x1, y1, hw, 0.0, (2.0 * PI) as f32, c, b, out);
        return;
    }
    let (nx, ny) = (-dy / len * hw, dx / len * hw);
    out.tri(b, x1 + nx, y1 + ny, x2 + nx, y2 + ny, x2 - nx, y2 - ny, c);
    out.tri(b, x1 + nx, y1 + ny, x2 - nx, y2 - ny, x1 - nx, y1 - ny, c);
    let a = (ny as f64).atan2(nx as f64) as f32;
    fan(x1, y1, hw, a, PI as f32, c, b, out);
    fan(x2, y2, hw, a + PI as f32, PI as f32, c, b, out);
}

#[allow(clippy::too_many_arguments)]
fn arc(x1: f32, y1: f32, x2: f32, y2: f32, bend: f32, hw: f32, c: u32, b: usize, out: &mut DrawList) {
    let (dx, dy) = ((x2 - x1) as f64, (y2 - y1) as f64);
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-6 {
        fan(x1, y1, hw, 0.0, (2.0 * PI) as f32, c, b, out);
        return;
    }
    let theta = (bend.clamp(-350.0, 350.0) as f64).to_radians();
    let r = chord / (2.0 * (theta.abs() / 2.0).sin());
    let (mx, my) = (((x1 + x2) / 2.0) as f64, ((y1 + y2) / 2.0) as f64);
    let (nx, ny) = (-dy / chord, dx / chord);
    let h = r * (theta / 2.0).cos();
    let sgn = if theta > 0.0 { -1.0 } else { 1.0 };
    let (cx, cy) = (mx + sgn * nx * h, my + sgn * ny * h);
    let a0 = (y1 as f64 - cy).atan2(x1 as f64 - cx);
    let sweep = if theta > 0.0 { -theta.abs() } else { theta.abs() };
    let n = 6.max(96.min(segments((r + hw as f64) as f32, theta.abs() as f32)));
    let (mut px0, mut py0, mut px1, mut py1) = (0f32, 0f32, 0f32, 0f32);
    for i in 0..=n {
        let a = a0 + sweep * i as f64 / n as f64;
        let (ux, uy) = (a.cos(), a.sin());
        let (ox, oy) = ((cx + (r + hw as f64) * ux) as f32, (cy + (r + hw as f64) * uy) as f32);
        let (ix, iy) = ((cx + (r - hw as f64) * ux) as f32, (cy + (r - hw as f64) * uy) as f32);
        if i > 0 {
            out.tri(b, px0, py0, ox, oy, ix, iy, c);
            out.tri(b, px0, py0, ix, iy, px1, py1, c);
        }
        px0 = ox;
        py0 = oy;
        px1 = ix;
        py1 = iy;
    }
    let t0 = a0 + if sweep > 0.0 { -PI / 2.0 } else { PI / 2.0 };
    let a1 = a0 + sweep;
    let t1 = a1 + if sweep > 0.0 { PI / 2.0 } else { -PI / 2.0 };
    fan(x1, y1, hw, (t0 - PI / 2.0) as f32, PI as f32, c, b, out);
    fan(x2, y2, hw, (t1 - PI / 2.0) as f32, PI as f32, c, b, out);
}

#[allow(clippy::too_many_arguments)]
fn ring(cx: f32, cy: f32, r: f32, width: f32, start: f32, span: f32, c: u32, b: usize, out: &mut DrawList) {
    let hw = width / 2.0;
    if hw <= 0.0 {
        return;
    }
    let (r0, r1) = ((r - hw).max(0.0), r + hw);
    if r <= 0.0 && r0 <= 0.0 {
        fan(cx, cy, r1, start, span, c, b, out);
        return;
    }
    let n = segments(r1, span);
    for i in 0..n {
        let a0 = (start + span * i as f32 / n as f32) as f64;
        let a1 = (start + span * (i + 1) as f32 / n as f32) as f64;
        let (c0, s0, c1, s1) = (a0.cos() as f32, a0.sin() as f32, a1.cos() as f32, a1.sin() as f32);
        let (ix0, iy0, ox0, oy0) = (cx + r0 * c0, cy + r0 * s0, cx + r1 * c0, cy + r1 * s0);
        let (ix1, iy1, ox1, oy1) = (cx + r0 * c1, cy + r0 * s1, cx + r1 * c1, cy + r1 * s1);
        out.tri(b, ix0, iy0, ox0, oy0, ox1, oy1, c);
        out.tri(b, ix0, iy0, ox1, oy1, ix1, iy1, c);
    }
}

#[allow(clippy::too_many_arguments)]
fn fan(cx: f32, cy: f32, r: f32, start: f32, span: f32, c: u32, b: usize, out: &mut DrawList) {
    if r <= 0.0 {
        return;
    }
    let n = segments(r, span);
    let (mut px, mut py) = (cx + r * (start as f64).cos() as f32, cy + r * (start as f64).sin() as f32);
    for i in 1..=n {
        let a = start + span * i as f32 / n as f32;
        let (x, y) = (cx + r * (a as f64).cos() as f32, cy + r * (a as f64).sin() as f32);
        out.tri(b, cx, cy, px, py, x, y, c);
        px = x;
        py = y;
    }
}

#[allow(clippy::too_many_arguments)]
fn text(gs: &dyn GlyphSource, t: &str, size: f32, xf: &Affine, corner: i32, align: i32, c: &Color, op: f32, b: Blend, effects: Option<&str>, out: &mut DrawList) {
    if !(gs.base_px() > 0.0) {
        return;
    }
    let rgba = Color::new(c.r, c.g, c.b, c.a * op).to_rgba8888();
    if rgba & 0xff == 0 {
        return;
    }
    let (k, lh) = (size / gs.base_px(), gs.line_height());
    let lines: Vec<&str> = t.split('\n').collect();
    let total = lines.len() as f32 * lh;
    let left = corner == 1 || corner == 3;
    let right = corner == 2 || corner == 4;
    let top = if corner == 1 || corner == 2 {
        0.0
    } else if corner == 3 || corner == 4 {
        total
    } else {
        total / 2.0
    };
    // A textfield's own alignment aligns each line inside the block.
    let widths: Vec<f32> = lines
        .iter()
        .map(|line| line.chars().take(10000).map(|ch| gs.glyph(ch).map(|g| g.advance).unwrap_or(lh * 0.3)).sum())
        .collect();
    let block = widths.iter().fold(0f32, |m, w| m.max(*w));
    let block_left = if left {
        0.0
    } else if right {
        -block
    } else {
        -block / 2.0
    };
    for (li, line) in lines.iter().enumerate().take(500) {
        let chars: Vec<char> = line.chars().take(10000).collect();
        let width = widths[li];
        let mut pen = if align < 0 {
            if left {
                0.0
            } else if right {
                -width
            } else {
                -width / 2.0
            }
        } else {
            block_left
                + match align {
                    0 => 0.0,
                    1 => (block - width) / 2.0,
                    _ => block - width,
                }
        };
        let base = top - (li as i32 + 1) as f32 * lh + lh * 0.25;
        for &ch in &chars {
            let Some(g) = gs.glyph(ch) else {
                pen += lh * 0.3;
                continue;
            };
            if g.w > 0.0 && g.h > 0.0 {
                let (x0, y0) = ((pen + g.x) * k, (base + g.y) * k);
                let (x1, y1) = (x0 + g.w * k, y0 + g.h * k);
                let bi = out.batch(b, effects, Some(&g.texture));
                let m = |x: f32, y: f32| (xf.map_x(x as f64, y as f64) as f32, xf.map_y(x as f64, y as f64) as f32);
                let (pa, pb, pc, pd) = (m(x0, y0), m(x1, y0), m(x1, y1), m(x0, y1));
                out.add(bi, pa.0, pa.1, rgba, g.u0, g.v0);
                out.add(bi, pb.0, pb.1, rgba, g.u1, g.v0);
                out.add(bi, pc.0, pc.1, rgba, g.u1, g.v1);
                out.add(bi, pa.0, pa.1, rgba, g.u0, g.v0);
                out.add(bi, pc.0, pc.1, rgba, g.u1, g.v1);
                out.add(bi, pd.0, pd.1, rgba, g.u0, g.v1);
            }
            pen += g.advance;
        }
    }
}

/// Ear clipping (either winding); a stuck self-intersecting rest is fanned.
pub fn ear_clip(p: &[f32]) -> Vec<usize> {
    let n = p.len() / 2;
    if n < 3 {
        return Vec::new();
    }
    let ccw = area(p) > 0.0;
    let mut v: Vec<usize> = (0..n).map(|i| if ccw { i } else { n - 1 - i }).collect();
    let cap = (n - 2) * 3;
    let mut out = Vec::with_capacity(cap);
    let mut count = n;
    let mut guard = (2 * n * n) as i64;
    let mut i = 0usize;
    while count > 3 && guard > 0 {
        guard -= 1;
        let (a, b, c) = (v[(i + count - 1) % count], v[i % count], v[(i + 1) % count]);
        if is_ear(p, &v, count, a, b, c) {
            out.extend([a, b, c]);
            let k = i % count;
            v.copy_within(k + 1..count, k);
            count -= 1;
            guard = (2 * count * count) as i64;
        } else {
            i += 1;
        }
    }
    let mut k = 1;
    while k + 1 < count && out.len() + 3 <= cap {
        out.extend([v[0], v[k], v[k + 1]]);
        k += 1;
    }
    out
}

fn area(p: &[f32]) -> f64 {
    let n = p.len() / 2;
    let mut s = 0.0;
    let mut j = n - 1;
    for i in 0..n {
        s += p[j * 2] as f64 * p[i * 2 + 1] as f64 - p[i * 2] as f64 * p[j * 2 + 1] as f64;
        j = i;
    }
    s / 2.0
}

fn cross(p: &[f32], a: usize, b: usize, c: usize) -> f64 {
    (p[b * 2] as f64 - p[a * 2] as f64) * (p[c * 2 + 1] as f64 - p[a * 2 + 1] as f64)
        - (p[b * 2 + 1] as f64 - p[a * 2 + 1] as f64) * (p[c * 2] as f64 - p[a * 2] as f64)
}

fn is_ear(p: &[f32], v: &[usize], count: usize, a: usize, b: usize, c: usize) -> bool {
    if cross(p, a, b, c) <= 0.0 {
        return false;
    }
    for &q in &v[..count] {
        if q == a || q == b || q == c {
            continue;
        }
        if cross(p, a, b, q) >= 0.0 && cross(p, b, c, q) >= 0.0 && cross(p, c, a, q) >= 0.0 {
            return false;
        }
    }
    true
}

/// Onion skin: tinted ghosts of the scene at earlier and later frames,
/// fading with distance (port of Java OnionSkin). Layers with onion off are
/// left out.
#[derive(Clone, Debug)]
pub struct OnionSkin {
    pub before: i32,
    pub after: i32,
    /// Ghost colours (0xRRGGBB) for past and future frames.
    pub past_rgb: u32,
    pub future_rgb: u32,
    pub opacity: f32,
    pub falloff: f32,
    /// Ghosts in stage space (editing view) unless set.
    pub apply_camera: bool,
}

impl Default for OnionSkin {
    fn default() -> Self {
        OnionSkin { before: 2, after: 1, past_rgb: 0xd04040, future_rgb: 0x3070d0, opacity: 0.35, falloff: 0.6, apply_camera: false }
    }
}

impl OnionSkin {
    /// Ghosts as an isolate tree, farthest first: each ghost drawn solid in its
    /// tint, then faded as one piece (its own overlaps do not show darker).
    pub fn build_layered(&self, p: &crate::model::Project, symbol: &str, t: f64) -> DrawList {
        let mut out = DrawList::default();
        out.open(symbol, 1.0, Blend::Normal, false, Vec::new());
        let fps = if p.settings.fps > 0.0 { p.settings.fps as f64 } else { 30.0 };
        let mut k = self.before.max(self.after);
        while k >= 1 {
            let a = self.opacity * (self.falloff as f64).powi(k - 1) as f32;
            for side in 0..2 {
                if if side == 0 { k > self.before } else { k > self.after } {
                    continue;
                }
                let at = if side == 0 { t - k as f64 / fps } else { t + k as f64 / fps };
                let ghost = tessellate(&crate::eval::evaluate_with(p, symbol, at, true, self.apply_camera), None);
                out.open("ghost", a.min(1.0), Blend::Normal, false, Vec::new());
                let rgb = if side == 0 { self.past_rgb } else { self.future_rgb };
                for b in &ghost.batches {
                    let o = out.batch(Blend::Normal, None, None);
                    for i in 0..b.vertex_count() {
                        let src_a = b.rgba[i] & 0xff;
                        out.add(o, b.xy[i * 2], b.xy[i * 2 + 1], (rgb << 8) | src_a, 0.0, 0.0);
                    }
                }
                out.close();
            }
            k -= 1;
        }
        out.close();
        out
    }
}

/// View options "Outline selection": the shapes of one layer (group source
/// ending in "/<layer>"), each fattened by `pad` stage units and drawn in
/// `rgb`, then the shapes themselves again on top, so a thin band of that
/// colour follows the figure's silhouette. Editing view only.
pub fn selection_outline(scene: &Scene, layer: &str, pad: f32, rgb: u32) -> DrawList {
    fn find<'a>(g: &'a Group, suffix: &str) -> Option<&'a Group> {
        if g.source.ends_with(suffix) {
            return Some(g);
        }
        g.children.iter().find_map(|i| if let Item::Group(c) = i { find(c, suffix) } else { None })
    }
    fn shapes<'a>(g: &'a Group, out: &mut Vec<&'a Shape>) {
        for i in &g.children {
            match i {
                Item::Shape(s) => out.push(s),
                Item::Group(c) => shapes(c, out),
            }
        }
    }
    let out = DrawList::default();
    let Some(g) = find(&scene.root, &format!("/{layer}")) else { return out };
    let mut own = Vec::new();
    shapes(g, &mut own);
    let color = Color { r: ((rgb >> 16) & 0xff) as f32 / 255.0, g: ((rgb >> 8) & 0xff) as f32 / 255.0, b: (rgb & 0xff) as f32 / 255.0, a: 1.0 };
    let mut fat = Group::new("outline");
    let edge = |fat: &mut Group, p: &[f32], closed: bool| {
        let n = p.len() / 2;
        for i in 0..n {
            if !closed && i + 1 == n {
                break;
            }
            let j = (i + 1) % n;
            fat.children.push(Item::Shape(Shape::new(ShapeKind::Line { x1: p[i * 2], y1: p[i * 2 + 1], x2: p[j * 2], y2: p[j * 2 + 1], width: pad * 2.0, bend: 0.0 }, color, "outline")));
        }
    };
    for s in &own {
        let kind = match &s.kind {
            ShapeKind::Line { x1, y1, x2, y2, width, bend } => ShapeKind::Line { x1: *x1, y1: *y1, x2: *x2, y2: *y2, width: width + pad * 2.0, bend: *bend },
            ShapeKind::Circle { cx, cy, r, start, span } => {
                if *span < std::f32::consts::TAU - 1e-3 {
                    // a part of a disc: its edges as well
                    let (a, b) = (*start, start + span);
                    edge(&mut fat, &[cx + r * a.cos(), cy + r * a.sin(), *cx, *cy, cx + r * b.cos(), cy + r * b.sin()], false);
                }
                ShapeKind::Ring { cx: *cx, cy: *cy, r: *r, width: pad * 2.0, start: *start, span: *span }
            }
            ShapeKind::Ring { cx, cy, r, width, start, span } => ShapeKind::Ring { cx: *cx, cy: *cy, r: *r, width: width + pad * 2.0, start: *start, span: *span },
            ShapeKind::Polygon { points, .. } => {
                edge(&mut fat, points, true);
                continue;
            }
            ShapeKind::Quad { corners, .. } => {
                edge(&mut fat, corners, true);
                continue;
            }
            _ => continue,
        };
        fat.children.push(Item::Shape(Shape::new(kind, color, "outline")));
    }
    let mut top = Group::new("outline");
    top.children.push(Item::Group(fat));
    top.children.push(Item::Group(g.clone()));
    layered(&Scene { root: top, time: scene.time, diagnostics: Vec::new(), primitives: 0 }, None)
}
