//! evaluate(project, symbol, t) builds a Scene (port of Java Evaluator).
//! Pure: reads the model, keeps no state. Nested symbols use an explicit
//! work stack, so self-referencing symbols cannot overflow the call stack.

pub mod affine;
pub mod pose;
pub mod resolve;
pub mod scene;

pub use affine::Affine;
pub use pose::FigurePose;
pub use resolve::Resolver;
pub use scene::*;

use crate::model::*;
pub use self::scene::Shape;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

/// Nesting levels allowed in total.
pub const DEPTH_CAP: usize = 64;
/// Maximum scene objects (shapes plus groups) per scene.
pub const PRIMITIVE_BUDGET: i32 = 250_000;

/// Where a group goes: a path of child indices from the root group.
type Slot = Vec<usize>;

struct Work<'p> {
    symbol: &'p Symbol,
    time: f64,
    xf: Affine,
    into: Slot,
    /// Symbols on the nesting path, innermost last.
    path: Vec<&'p str>,
}

struct Ctx<'p> {
    project: &'p Project,
    r: Resolver<'p>,
    primitives: i32,
    root: Group,
}

impl<'p> Ctx<'p> {
    fn spend(&mut self, what: &str) -> bool {
        if self.primitives >= PRIMITIVE_BUDGET {
            if !self.r.has(DiagnosticKind::PrimitiveBudget) {
                self.r.diag(DiagnosticKind::PrimitiveBudget, what);
            }
            return false;
        }
        self.primitives += 1;
        true
    }

    fn group_at(&mut self, slot: &[usize]) -> &mut Group {
        let mut g = &mut self.root;
        for &i in slot {
            g = match &mut g.children[i] {
                Item::Group(x) => x,
                Item::Shape(_) => unreachable!("slot points at a shape"),
            };
        }
        g
    }
}

/// The scene with the camera applied (as played and exported).
pub fn evaluate(project: &Project, symbol_id: &str, t: f64) -> Scene {
    evaluate_with(project, symbol_id, t, false, true)
}

/// onion_only: skip top-level layers whose onion flag is off.
/// apply_camera false: stage (world) space, for editing.
pub fn evaluate_with(project: &Project, symbol_id: &str, t: f64, onion_only: bool, apply_camera: bool) -> Scene {
    let mut ctx = Ctx { project, r: Resolver::new(project), primitives: 0, root: Group::new(symbol_id) };
    let Some(sym) = project.library.get(symbol_id) else {
        ctx.r.diag(DiagnosticKind::MissingRef, symbol_id);
        return Scene { root: ctx.root, time: t, diagnostics: ctx.r.diagnostics, primitives: 0 };
    };
    let view = if apply_camera { wobble(project, sym, t, &mut ctx.r).mul(&camera_view(sym, t, &mut ctx.r)) } else { Affine::IDENTITY };
    let mut stack = vec![Work { symbol: sym, time: t, xf: view, into: Vec::new(), path: vec![sym.id.as_str()] }];
    while let Some(w) = stack.pop() {
        let top = w.path.len() == 1;
        eval_symbol(&mut ctx, w, &mut stack, onion_only && top);
    }
    if !onion_only {
        background(&mut ctx, sym, t, apply_camera);
    }
    if apply_camera && !onion_only && letterbox(project, widescreen_at(project, sym, t, &mut ctx.r), &mut ctx.root) {
        for l in &sym.layers {
            let LayerKind::Text(tl) = &l.kind else { continue };
            if !tl.above_bars {
                continue;
            }
            let src = format!("{}/{}", sym.id, l.id);
            if let Some(i) = ctx.root.children.iter().position(|o| matches!(o, Item::Group(g) if g.source == src)) {
                let o = ctx.root.children.remove(i);
                ctx.root.children.push(o);
            }
        }
    }
    Scene { root: ctx.root, time: t, diagnostics: ctx.r.diagnostics, primitives: ctx.primitives }
}

/// The topmost visible camera of the symbol at t.
pub fn active_camera<'s>(sym: &'s Symbol, t: f64, r: &mut Resolver) -> Option<&'s CameraLayer> {
    for l in sym.layers.iter().rev() {
        if let LayerKind::Camera(c) = &l.kind {
            if l.visible && r.b(&l.shown, t) {
                return Some(c);
            }
        }
    }
    None
}

fn background(ctx: &mut Ctx, sym: &Symbol, t: f64, apply_camera: bool) {
    let Some(cam) = active_camera(sym, t, &mut ctx.r) else { return };
    let top = ctx.r.color(&cam.background, t);
    if !(top.a > 0.0) {
        return;
    }
    let bottom = ctx.r.color(&cam.background2, t);
    let w = ctx.project.settings.width as f32 / 2.0;
    let h = ctx.project.settings.height as f32 / 2.0;
    let mut pts = vec![-w, -h, w, -h, w, h, -w, h];
    // "Expand gradient": the fade runs over three stage heights from the top
    let fps = if ctx.project.settings.fps > 0.0 { ctx.project.settings.fps as f64 } else { 30.0 };
    let expand = crate::timing::setting(&cam.bg_expand, fps, (t * fps + 1e-6).floor() as i32) > 0.5;
    let mut ends = [0.0, h, 0.0, if expand { -5.0 * h } else { -h }];
    if !apply_camera {
        let inv = camera_of(cam, t, &mut ctx.r).inverse();
        for i in (0..pts.len()).step_by(2) {
            map_point(&inv, &mut pts, i);
        }
        map_point(&inv, &mut ends, 0);
        map_point(&inv, &mut ends, 2);
    }
    let mut poly = Shape::new(ShapeKind::Polygon { points: pts, convex: true }, top, "background");
    if bottom.a > 0.0 {
        poly.gradient = Some(Gradient::linear(ends[0], ends[1], ends[2], ends[3], bottom));
    }
    let mut g = Group::new("background");
    g.children.push(Item::Shape(poly));
    ctx.root.children.insert(0, Item::Group(g));
}

fn map_point(m: &Affine, p: &mut [f32], i: usize) {
    let (x, y) = (p[i] as f64, p[i + 1] as f64);
    p[i] = (m.a * x + m.c * y + m.tx) as f32;
    p[i + 1] = (m.b * x + m.d * y + m.ty) as f32;
}

/// Camera Tools "Widescreen effect": bars to this shape on the frames that have it.
pub const WIDESCREEN: f32 = 2.35;

fn frame_of(project: &Project, t: f64) -> (f64, i32) {
    let fps = if project.settings.fps > 0.0 { project.settings.fps as f64 } else { 30.0 };
    (fps, (t * fps + 1e-6).floor() as i32)
}

fn widescreen_at(project: &Project, sym: &Symbol, t: f64, r: &mut Resolver) -> bool {
    let (fps, k) = frame_of(project, t);
    sym.id == project.root_symbol && active_camera(sym, t, r).is_some_and(|c| crate::timing::setting(&c.widescreen, fps, k) > 0.5)
}

/// Camera Tools "Wobble camera": a shake in screen space (playback and
/// export), smooth noise at the frame's wobble speed.
fn wobble(project: &Project, sym: &Symbol, t: f64, r: &mut Resolver) -> Affine {
    if sym.id != project.root_symbol {
        return Affine::IDENTITY;
    }
    let Some(c) = active_camera(sym, t, r) else { return Affine::IDENTITY };
    let (fps, k) = frame_of(project, t);
    let set = |p: &Property| crate::timing::setting(p, fps, k) as f64;
    let (xy, rot) = (set(&c.wobble_xy), set(&c.wobble_rot));
    if xy == 0.0 && rot == 0.0 {
        return Affine::IDENTITY;
    }
    let speed = if set(&c.wobble_speed) > 0.0 { set(&c.wobble_speed) } else { 3.0 };
    let zoom = if set(&c.wobble_no_zoom) > 0.5 { 1.0 } else { (r.f(&c.zoom, t) as f64).max(1e-6) };
    let u = t * speed;
    let (dx, dy) = (resolve::fbm(u, 11, 2) * xy * zoom, resolve::fbm(u, 23, 2) * xy * zoom);
    Affine::translate(dx, dy).mul(&Affine::rotate_deg(resolve::fbm(u, 37, 2) * rot))
}

fn letterbox(project: &Project, widescreen: bool, root: &mut Group) -> bool {
    let aspect = if widescreen { project.settings.letterbox.max(WIDESCREEN) } else { project.settings.letterbox };
    let (w, h) = (project.settings.width as f32, project.settings.height as f32);
    if !(aspect > 0.0) || !(w > 0.0) || !(h > 0.0) || aspect <= w / h {
        return false;
    }
    let (bar, x0, x1, y1) = ((h - w / aspect) / 2.0, -w / 2.0, w / 2.0, h / 2.0);
    let mut g = Group::new("letterbox");
    g.children.push(Item::Shape(Shape::new(
        ShapeKind::Polygon { points: vec![x0, y1 - bar, x1, y1 - bar, x1, y1, x0, y1], convex: true },
        Color::BLACK,
        "letterbox",
    )));
    g.children.push(Item::Shape(Shape::new(
        ShapeKind::Polygon { points: vec![x0, -y1, x1, -y1, x1, -y1 + bar, x0, -y1 + bar], convex: true },
        Color::BLACK,
        "letterbox",
    )));
    root.children.push(Item::Group(g));
    true
}

/// View transform of the topmost visible camera (identity without one).
pub fn camera_view(sym: &Symbol, t: f64, r: &mut Resolver) -> Affine {
    match active_camera(sym, t, r) {
        Some(c) => camera_of(c, t, r),
        None => Affine::IDENTITY,
    }
}

/// One camera's view transform (stage to shot).
pub fn camera_of(c: &CameraLayer, t: f64, r: &mut Resolver) -> Affine {
    let mut zoom = r.f(&c.zoom, t) as f64;
    if !(zoom > 1e-6) {
        zoom = 1e-6;
    }
    let rot = r.f(&c.rotation, t);
    let (x, y) = (r.f(&c.x, t), r.f(&c.y, t));
    Affine::scale(zoom, zoom).mul(&Affine::rotate_deg(-rot as f64)).mul(&Affine::translate(-x as f64, -y as f64))
}

/// The part of the camera's view a camera-locked layer undoes (parallax).
fn lock_view(c: &CameraLayer, parallax: f32, move_only: bool, t: f64, r: &mut Resolver) -> Affine {
    if move_only {
        let k = 1.0 - parallax as f64;
        let (x, y) = (r.f(&c.x, t), r.f(&c.y, t));
        return Affine::translate(-x as f64 * k, -y as f64 * k);
    }
    if parallax == 0.0 {
        return camera_of(c, t, r);
    }
    let k = 1.0 - parallax as f64;
    let mut zoom = r.f(&c.zoom, t) as f64;
    if !(zoom > 1e-6) {
        zoom = 1e-6;
    }
    zoom = zoom.powf(k);
    let rot = r.f(&c.rotation, t);
    let (x, y) = (r.f(&c.x, t), r.f(&c.y, t));
    Affine::scale(zoom, zoom).mul(&Affine::rotate_deg(-rot as f64 * k)).mul(&Affine::translate(-x as f64 * k, -y as f64 * k))
}

fn eval_symbol<'p>(ctx: &mut Ctx<'p>, w: Work<'p>, stack: &mut Vec<Work<'p>>, onion_only: bool) {
    let sym = w.symbol;
    let t = w.time;
    let any_solo = sym.layers.iter().any(|l| l.solo);
    for l in &sym.layers {
        if !l.visible || (onion_only && !l.onion) {
            continue;
        }
        if matches!(l.kind, LayerKind::Camera(_) | LayerKind::Audio(_)) {
            continue;
        }
        let adjustment = matches!(l.kind, LayerKind::Adjustment);
        if any_solo && !l.solo && !adjustment {
            continue;
        }
        if !ctx.r.b(&l.shown, t) {
            continue;
        }
        if l.vanish_in_tween && sym.id == ctx.project.root_symbol {
            // between frame k and k + 1, where it is not on frame k + 1
            let fps = if ctx.project.settings.fps > 0.0 { ctx.project.settings.fps as f64 } else { 30.0 };
            let k = (t * fps + 1e-6).floor();
            if t * fps - k > 1e-6 && !ctx.r.b(&l.shown, (k + 1.0) / fps) && (k + 1.0) / fps < sym.duration - 1e-9 {
                continue;
            }
        }
        if !ctx.spend(&l.id) {
            break;
        }
        let mut g = Group::new(format!("{}/{}", sym.id, l.id));
        let op = ctx.r.f(&l.opacity, t);
        g.opacity = op.clamp(0.0, 1.0);
        if op.is_nan() {
            g.opacity = op;
        }
        g.blend = l.blend;
        resolve_effects(l, sym, t, &mut ctx.r, &mut g);
        let chain = layer_chain(sym, l, t, &mut ctx.r);
        let xf = w.xf.mul(&chain);
        if !xf.is_finite() {
            ctx.r.diag(DiagnosticKind::BadValue, format!("{}/transform", l.id));
            continue;
        }
        if adjustment {
            g.adjustment = true;
        }
        let into = ctx.group_at(&w.into);
        into.children.push(Item::Group(g));
        let mut slot = w.into.clone();
        slot.push(into.children.len() - 1);
        match &l.kind {
            LayerKind::Figure(f) => emit_figure(ctx, &l.id, f, &xf, t, &slot),
            LayerKind::Sprite(s) => emit_sprite(ctx, &l.id, s, &xf, t, &slot),
            LayerKind::Symbol(s) => push_instance(ctx, &l.id, s, xf, t, slot, &w.path, stack),
            LayerKind::Text(tl) => {
                if let Some(text) = &tl.text {
                    if !text.is_empty() && tl.size > 0.0 && ctx.spend(&l.id) {
                        let color = ctx.r.color(&tl.color, t);
                        let shape = Shape::new(
                            ShapeKind::Text { text: text.clone(), font: tl.font.clone(), size: tl.size.min(10000.0), xf, corner: tl.corner, align: tl.align },
                            color,
                            l.id.clone(),
                        );
                        ctx.group_at(&slot).children.push(Item::Shape(shape));
                    }
                }
            }
            _ => {}
        }
    }
}

fn resolve_effects(l: &Layer, sym: &Symbol, t: f64, r: &mut Resolver, g: &mut Group) {
    if l.inherit_effects && l.parent.is_some() {
        let mut chain: Vec<&Layer> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();
        seen.insert(&l.id);
        let mut cur = l;
        while cur.inherit_effects {
            let Some(pid) = &cur.parent else { break };
            let Some(p) = sym.layer(pid) else { break };
            if !seen.insert(&p.id) {
                break;
            }
            chain.insert(0, p);
            cur = p;
        }
        for p in chain {
            resolve_own(p, t, r, g);
        }
    }
    resolve_own(l, t, r, g);
}

fn resolve_own(l: &Layer, t: f64, r: &mut Resolver, g: &mut Group) {
    if l.effects.is_empty() {
        return;
    }
    for e in l.effects.chain() {
        if !e.enabled {
            continue;
        }
        let mix = r.f(&e.mix, t);
        let mut params = Vec::new();
        for (name, p) in &e.params {
            let v = match r.resolve(p, t) {
                Value::F(x) => ParamValue::F(x),
                Value::D(x) => ParamValue::D(x),
                Value::C(x) => ParamValue::C(x),
                Value::B(x) => ParamValue::B(x),
            };
            params.push((name.clone(), v));
        }
        g.effects.push(ResolvedEffect { ty: e.ty.clone(), mix, params });
    }
}

fn local(l: &Layer, t: f64, r: &mut Resolver) -> Affine {
    let tr = &l.transform;
    let (x, y) = (r.f(&tr.x, t), r.f(&tr.y, t));
    let rot = r.f(&tr.rotation, t);
    let (sx, sy) = (r.f(&tr.scale_x, t), r.f(&tr.scale_y, t));
    let (ax, ay) = (r.f(&tr.anchor_x, t), r.f(&tr.anchor_y, t));
    Affine::translate(x as f64, y as f64)
        .mul(&Affine::rotate_deg(rot as f64))
        .mul(&Affine::scale(sx as f64, sy as f64))
        .mul(&Affine::translate(-ax as f64, -ay as f64))
}

/// The transform a layer's own values are expressed in: its parent chain,
/// joint included (stage space).
pub fn parent_frame(project: &Project, sym: &Symbol, l: &Layer, t: f64) -> Affine {
    let mut r = Resolver::new(project);
    let local = local(l, t, &mut r);
    layer_chain(sym, l, t, &mut r).mul(&local.inverse())
}

/// The layer's parent-chain transform without the camera (stage space).
pub fn layer_stage(project: &Project, sym: &Symbol, l: &Layer, t: f64) -> Affine {
    let mut r = Resolver::new(project);
    layer_chain(sym, l, t, &mut r)
}

/// The layer's own transform (position, rotation, scale, anchor) at t.
pub fn local_transform(project: &Project, l: &Layer, t: f64) -> Affine {
    let mut r = Resolver::new(project);
    local(l, t, &mut r)
}

/// The layer's own transform composed with its parent chain (stage space).
pub fn layer_chain(sym: &Symbol, l: &Layer, t: f64, r: &mut Resolver) -> Affine {
    let mut acc = local(l, t, r);
    let mut seen: HashSet<&str> = HashSet::new();
    seen.insert(&l.id);
    let mut cur = l;
    while let Some(pid) = &cur.parent {
        let Some(p) = sym.layer(pid) else {
            r.diag(DiagnosticKind::MissingRef, format!("{}/parent", cur.id));
            break;
        };
        if !seen.insert(&p.id) {
            r.diag(DiagnosticKind::ParentCycle, l.id.clone());
            break;
        }
        if let LayerKind::Camera(c) = &p.kind {
            acc = lock_view(c, cur.parallax, cur.lock_move_only, t, r).inverse().mul(&acc);
            cur = p;
            continue;
        }
        let joint = match &cur.parent_node {
            Some(nid) => node_frame(p, nid, cur.parent_node_rotate, t, r, &cur.id),
            None => None,
        };
        let lp = local(p, t, r);
        acc = lp.mul(&match joint {
            Some(j) => j.mul(&acc),
            None => acc,
        });
        cur = p;
    }
    acc
}

/// A joint's frame inside its figure: at the joint, turned by its world angle.
fn node_frame(p: &Layer, node_id: &str, rotate: bool, t: f64, r: &mut Resolver, who: &str) -> Option<Affine> {
    let LayerKind::Figure(f) = &p.kind else { return None };
    let Some(i) = f.index_of(node_id) else {
        r.diag(DiagnosticKind::MissingRef, format!("{who}/parentNode"));
        return None;
    };
    let pose = FigurePose::compute(f, t, r);
    let a = Affine::translate(pose.x[i], pose.y[i]);
    Some(if rotate { a.mul(&Affine::rotate_deg(pose.angle[i])) } else { a })
}

fn emit_figure(ctx: &mut Ctx, layer_id: &str, f: &Figure, xf: &Affine, t: f64, slot: &[usize]) {
    let nodes = f.nodes();
    let pose = FigurePose::compute(f, t, &mut ctx.r);
    let (px, py, parent) = (&pose.x, &pose.y, &pose.parent);
    let sf = xf.scale_factor();
    let mirrored = xf.a * xf.d - xf.b * xf.c < 0.0;
    let mut fills: HashMap<&str, Vec<&Polyfill>> = HashMap::new();
    for pf in &f.polyfills {
        fills.entry(pf.anchor.as_str()).or_default().push(pf);
    }
    let mut out: Vec<Item> = Vec::new();
    let mut budget_out = false;
    for i in f.draw_sequence() {
        let node = &nodes[i];
        let pi = parent[i];
        if let Some(list) = fills.get(node.id.as_str()) {
            for pf in list {
                if pf.nodes.len() < 2 || !ctx.spend(layer_id) {
                    continue;
                }
                let mut ring = vec![i as i32];
                let mut ok = true;
                for id in &pf.nodes {
                    match f.index_of(id) {
                        Some(k) => ring.push(k as i32),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    ctx.r.diag(DiagnosticKind::MissingRef, format!("{layer_id}/{}", pf.id));
                    continue;
                }
                let mut q: Vec<f32> = Vec::new();
                for k in 0..ring.len() {
                    let (a, b) = (ring[k] as usize, ring[(k + 1) % ring.len()] as usize);
                    let (x0, y0) = (xf.map_x(px[a], py[a]) as f32, xf.map_y(px[a], py[a]) as f32);
                    let (x1, y1) = (xf.map_x(px[b], py[b]) as f32, xf.map_y(px[b], py[b]) as f32);
                    q.push(x0);
                    q.push(y0);
                    let child = if parent[b] == a as i32 {
                        b as i32
                    } else if parent[a] == b as i32 {
                        a as i32
                    } else {
                        -1
                    };
                    if child >= 0 && nodes[child as usize].curve != 0.0 && nodes[child as usize].shape == crate::model::Shape::Line {
                        let mut h = ((nodes[child as usize].curve * CURVE_SAGITTA) as f64 * sf) as f32;
                        if mirrored {
                            h = -h;
                        }
                        if child == a as i32 {
                            h = -h;
                        }
                        arc_interior(&mut q, x0, y0, x1, y1, h);
                    }
                }
                let pc = if pf.use_own_color { ctx.r.color(&pf.color, t) } else { ctx.r.color(&node.color, t) };
                out.push(Item::Shape(Shape::new(ShapeKind::Polygon { points: q, convex: false }, pc, format!("{layer_id}/{}", pf.id))));
            }
        }
        if pi < 0 || node.shape == crate::model::Shape::None || !ctx.r.b(&node.shown, t) {
            continue;
        }
        if !ctx.spend(layer_id) {
            budget_out = true;
            break;
        }
        let pi = pi as usize;
        let mut c = ctx.r.color(&node.color, t);
        let src = format!("{layer_id}/{}", node.id);
        let (ax, ay) = (xf.map_x(px[pi], py[pi]) as f32, xf.map_y(px[pi], py[pi]) as f32);
        let (bx, by) = (xf.map_x(px[i], py[i]) as f32, xf.map_y(px[i], py[i]) as f32);
        let w = (ctx.r.f(&node.thickness, t).abs() as f64 * sf) as f32;
        let mut g_end: Option<Color> = None;
        if node.gradient {
            let mut e = ctx.r.color(&node.gradient_color, t);
            if node.gradient_reverse {
                std::mem::swap(&mut c, &mut e);
            }
            g_end = Some(e);
        }
        let before = out.len();
        match node.shape {
            crate::model::Shape::Line => {
                let mut bend = 0f32;
                if node.curve != 0.0 {
                    let chord = ((bx - ax) as f64).hypot((by - ay) as f64);
                    let h = (node.curve * CURVE_SAGITTA) as f64 * sf;
                    bend = if chord < 1e-6 { 0.0 } else { (4.0 * (2.0 * h / chord).atan()).to_degrees() as f32 };
                    if mirrored {
                        bend = -bend;
                    }
                }
                out.push(Item::Shape(Shape::new(ShapeKind::Line { x1: ax, y1: ay, x2: bx, y2: by, width: w, bend }, c, src.clone())));
            }
            crate::model::Shape::Circle => {
                let (cx, cy) = ((ax + bx) / 2.0, (ay + by) / 2.0);
                let rim = (((bx - ax) as f64).hypot((by - ay) as f64) / 2.0) as f32;
                // the fill reaches out by half the thickness, like a stroke around the disc
                // (user-observed on the original, 2026-11-21)
                let rad2 = rim + w / 2.0;
                let (mut start, mut span) = (0f32, (2.0 * PI) as f32);
                if node.half_arc != 0 {
                    let to_b = ((by - cy) as f64).atan2((bx - cx) as f64) as f32;
                    span = PI as f32;
                    start = if node.half_arc > 0 { to_b } else { to_b + PI as f32 };
                    if mirrored {
                        start += PI as f32;
                    }
                }
                if !node.circle_hollow {
                    out.push(Item::Shape(Shape::new(ShapeKind::Circle { cx, cy, r: rad2, start, span }, c, src.clone())));
                }
                if node.circle_outline || node.circle_hollow {
                    let oc = if node.circle_outline { ctx.r.color(&node.outline_color, t) } else { c };
                    let ow = w.max(sf as f32);
                    out.push(Item::Shape(Shape::new(ShapeKind::Ring { cx, cy, r: rim, width: ow, start, span }, oc, src.clone())));
                }
            }
            _ => {
                if let Some(pts) = pose::outline(node, ax, ay, bx, by, w) {
                    let convex = node.shape != crate::model::Shape::Trapezoid;
                    out.push(Item::Shape(Shape::new(ShapeKind::Polygon { points: pts, convex }, c, src.clone())));
                }
            }
        }
        if let (Some(end), true) = (g_end, out.len() > before) {
            let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
            use crate::model::Shape as S;
            let round = matches!(node.shape, S::Circle | S::Polygon | S::Ellipse);
            let hyp = ((bx - ax) as f64).hypot((by - ay) as f64);
            let grad = if round && node.gradient_linear {
                let len = 1e-6f64.max(hyp) as f32;
                let half = if node.shape == S::Ellipse { w / 2.0 } else { len / 2.0 };
                if node.gradient_sideways {
                    let (nx, ny) = (-(by - ay) / len * half, (bx - ax) / len * half);
                    Gradient::linear(mx - nx, my - ny, mx + nx, my + ny, end)
                } else {
                    Gradient::linear(ax, ay, bx, by, end)
                }
            } else if matches!(node.shape, S::Circle | S::Polygon) {
                Gradient::radial(mx, my, (hyp / 2.0) as f32, end)
            } else if node.shape == S::Ellipse {
                Gradient::elliptical(mx, my, (bx - ax) / 2.0, (by - ay) / 2.0, w / 2.0, end)
            } else if node.gradient_sideways {
                let len = 1e-6f64.max(hyp) as f32;
                let (nx, ny) = (-(by - ay) / len * w / 2.0, (bx - ax) / len * w / 2.0);
                Gradient::linear(mx - nx, my - ny, mx + nx, my + ny, end)
            } else {
                Gradient::linear(ax, ay, bx, by, end)
            };
            if let Item::Shape(s) = &mut out[before] {
                s.gradient = Some(grad);
            }
        }
    }
    let _ = budget_out;
    ctx.group_at(slot).children.extend(out);
}

/// Points strictly between the ends on the arc with bulge h to the left.
fn arc_interior(out: &mut Vec<f32>, x0: f32, y0: f32, x1: f32, y1: f32, h: f32) {
    let (dx, dy) = ((x1 - x0) as f64, (y1 - y0) as f64);
    let c = dx.hypot(dy);
    let h = h as f64;
    if c < 1e-6 || h.abs() < 1e-6 {
        return;
    }
    let (nx, ny) = (-dy / c, dx / c);
    let (mx, my) = (((x0 + x1) / 2.0) as f64, ((y0 + y1) / 2.0) as f64);
    let big_r = (c * c / 4.0 + h * h) / (2.0 * h.abs());
    let side = java_signum(h);
    let (cx, cy) = (mx + nx * side * (h.abs() - big_r), my + ny * side * (h.abs() - big_r));
    let a0 = (y0 as f64 - cy).atan2(x0 as f64 - cx);
    let a1 = (y1 as f64 - cy).atan2(x1 as f64 - cx);
    let sweep = a1 - a0;
    let (apx, apy) = (mx + nx * h, my + ny * h);
    let am = (apy - cy).atan2(apx - cx);
    let s1 = norm(sweep);
    let s2 = s1 - java_signum(s1) * 2.0 * PI;
    let to_apex = norm(am - a0);
    let sweep = if java_signum(to_apex) == java_signum(s1) && to_apex.abs() < s1.abs() { s1 } else { s2 };
    let n = 2.max(64.min((sweep.abs() * big_r / 2.0).ceil() as i32));
    for k in 1..n {
        let a = a0 + sweep * k as f64 / n as f64;
        out.push((cx + big_r * a.cos()) as f32);
        out.push((cy + big_r * a.sin()) as f32);
    }
}

fn java_signum(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        v
    }
}

fn norm(mut a: f64) -> f64 {
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

fn emit_sprite(ctx: &mut Ctx, layer_id: &str, l: &SpriteLayer, xf: &Affine, t: f64, slot: &[usize]) {
    let Some(asset) = &l.asset else { return };
    if !ctx.spend(layer_id) {
        return;
    }
    let (w, h) = (l.width as f64, l.height as f64);
    let mut pts = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    for (i, p) in pts.iter_mut().enumerate() {
        p[0] += ctx.r.f(&l.distort[i * 2], t) as f64;
        p[1] += ctx.r.f(&l.distort[i * 2 + 1], t) as f64;
    }
    let mut c = [0f32; 8];
    for i in 0..4 {
        c[i * 2] = xf.map_x(pts[i][0], pts[i][1]) as f32;
        c[i * 2 + 1] = xf.map_y(pts[i][0], pts[i][1]) as f32;
    }
    let tint = ctx.r.color(&l.tint, t);
    ctx.group_at(slot).children.push(Item::Shape(Shape::new(ShapeKind::Quad { asset: asset.clone(), corners: c }, tint, layer_id)));
}

#[allow(clippy::too_many_arguments)]
fn push_instance<'p>(
    ctx: &mut Ctx<'p>,
    layer_id: &str,
    l: &'p SymbolLayer,
    xf: Affine,
    t: f64,
    slot: Slot,
    path: &[&'p str],
    stack: &mut Vec<Work<'p>>,
) {
    let Some(target) = ctx.project.library.get(&l.symbol) else {
        ctx.r.diag(DiagnosticKind::MissingRef, format!("{layer_id}->{}", l.symbol));
        return;
    };
    let depth = path.len() - 1;
    if depth + 1 >= DEPTH_CAP {
        ctx.r.diag(DiagnosticKind::DepthCap, layer_id);
        return;
    }
    let occurrences = path.iter().filter(|s| **s == target.id).count() as i32;
    if occurrences > l.recursion_limit.max(0) {
        ctx.r.diag(DiagnosticKind::RecursionLimit, layer_id);
        return;
    }
    let local = instance_time(l, target, t, &mut ctx.r);
    let mut p = path.to_vec();
    p.push(target.id.as_str());
    stack.push(Work { symbol: target, time: local, xf, into: slot, path: p });
}

/// The time an instance shows inside its symbol at parent time t.
pub fn instance_time(l: &SymbolLayer, target: &Symbol, t: f64, r: &mut Resolver) -> f64 {
    let off = r.d(&l.time_offset, t);
    let el = resolve::speed_elapsed(&l.speed, t, r);
    local_time(off + el, target.duration, l.loop_mode)
}

/// Applies the instance loop mode against the symbol duration.
pub fn local_time(lt: f64, duration: f64, mode: SymbolLoop) -> f64 {
    if !lt.is_finite() {
        return 0.0;
    }
    if !(duration > 0.0) {
        return lt.min(0.0).max(0.0);
    }
    match mode {
        SymbolLoop::Once => lt.clamp(0.0, duration),
        SymbolLoop::Loop => {
            let m = lt % duration;
            if m < 0.0 {
                m + duration
            } else {
                m
            }
        }
        SymbolLoop::PingPong => {
            let mut p = lt % (2.0 * duration);
            if p < 0.0 {
                p += 2.0 * duration;
            }
            if p <= duration {
                p
            } else {
                2.0 * duration - p
            }
        }
    }
}
