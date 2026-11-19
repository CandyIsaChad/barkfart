//! Forward kinematics (port of Java FigurePose) and node outlines (Outline).

use super::resolve::Resolver;
use crate::model::*;
use std::f64::consts::PI;

/// Joint positions and world angles (degrees), indexed like Figure::nodes.
pub struct FigurePose {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub angle: Vec<f64>,
    /// Parent index, -1 for the root.
    pub parent: Vec<i32>,
}

impl FigurePose {
    pub fn compute(f: &Figure, t: f64, r: &mut Resolver) -> FigurePose {
        let nodes = f.nodes();
        let n = nodes.len();
        let mut p = FigurePose { x: vec![0.0; n], y: vec![0.0; n], angle: vec![0.0; n], parent: vec![0; n] };
        for (i, node) in nodes.iter().enumerate() {
            let rot = r.f(&node.rotation, t) as f64;
            let Some(parent) = &node.parent else {
                p.parent[i] = -1;
                p.angle[i] = rot;
                continue;
            };
            let pi = f.index_of(parent).map(|v| v as i32).unwrap_or(-1);
            p.parent[i] = pi;
            if pi < 0 {
                p.angle[i] = rot;
                continue;
            }
            let pi = pi as usize;
            p.angle[i] = base(nodes, &p, i, pi) + rot;
            let len = (r.f(&node.length, t) * r.f(&node.scale, t)) as f64;
            let rad = p.angle[i].to_radians();
            p.x[i] = p.x[pi] + len * rad.cos();
            p.y[i] = p.y[pi] + len * rad.sin();
        }
        p
    }

    /// World angle node i's rotation is measured from.
    pub fn base_angle(&self, nodes: &[Node], i: usize) -> f64 {
        if self.parent[i] < 0 {
            0.0
        } else {
            base(nodes, self, i, self.parent[i] as usize)
        }
    }
}

fn base(nodes: &[Node], p: &FigurePose, i: usize, pi: usize) -> f64 {
    let lock = nodes[i].angle_lock;
    if lock == LOCK_ABSOLUTE {
        return 0.0;
    }
    if lock == LOCK_RELATIVE {
        let mut a = pi as i32;
        while a >= 0 && nodes[a as usize].is_static && p.parent[a as usize] >= 0 {
            a = p.parent[a as usize];
        }
        return p.angle[a as usize];
    }
    p.angle[pi]
}

/// Convex outline of a node shape from A (parent) to B (node), or None.
pub fn outline(node: &Node, ax: f32, ay: f32, bx: f32, by: f32, width: f32) -> Option<Vec<f32>> {
    let (dx, dy) = (bx - ax, by - ay);
    let len = (dx * dx + dy * dy).sqrt();
    if node.shape == Shape::Polygon {
        if len < 1e-6 {
            return None;
        }
        let n = node.sides();
        let (cx, cy, r) = ((ax + bx) / 2.0, (ay + by) / 2.0, len / 2.0);
        let a0 = ((by - cy) as f64).atan2((bx - cx) as f64);
        let mut p = Vec::with_capacity(n as usize * 2);
        for i in 0..n {
            let a = a0 + 2.0 * PI * i as f64 / n as f64;
            p.push(cx + r * a.cos() as f32);
            p.push(cy + r * a.sin() as f32);
        }
        return Some(p);
    }
    if node.shape == Shape::Trapezoid {
        if len < 1e-6 || width <= 0.0 {
            return None;
        }
        let (ux, uy) = (dx / len, dy / len);
        let w0 = width / 2.0;
        let w1 = width * node.trapezoid_ratio.max(0.0) / 2.0;
        let mut pts = Vec::new();
        add_end(&mut pts, ax, ay, ux, uy, w0, node.trapezoid_round_start, false);
        add_end(&mut pts, bx, by, ux, uy, w1, node.trapezoid_round_end, true);
        return Some(pts);
    }
    outline_basic(node.shape, ax, ay, bx, by, width, node.triangle_upside_down, node.right_triangle)
}

#[allow(clippy::too_many_arguments)]
fn add_end(pts: &mut Vec<f32>, x: f32, y: f32, ux: f32, uy: f32, hw: f32, round: bool, at_b: bool) {
    let (nx, ny) = (-uy, ux);
    if !round || hw <= 0.0 {
        if !at_b {
            pts.extend([x + nx * hw, y + ny * hw, x - nx * hw, y - ny * hw]);
        } else {
            pts.extend([x - nx * hw, y - ny * hw, x + nx * hw, y + ny * hw]);
        }
        return;
    }
    let n = 12;
    let base = (ny as f64).atan2(nx as f64);
    for i in 0..=n {
        let a = if at_b { base + PI + PI * i as f64 / n as f64 } else { base + PI * i as f64 / n as f64 };
        pts.push(x + hw * a.cos() as f32);
        pts.push(y + hw * a.sin() as f32);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn outline_basic(shape: Shape, mut ax: f32, mut ay: f32, mut bx: f32, mut by: f32, width: f32, upside_down: bool, mut right: i32) -> Option<Vec<f32>> {
    if shape == Shape::Triangle && upside_down {
        std::mem::swap(&mut ax, &mut bx);
        std::mem::swap(&mut ay, &mut by);
        right = -right;
    }
    let (dx, dy) = (bx - ax, by - ay);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 || width <= 0.0 {
        return None;
    }
    let (ux, uy) = (dx / len, dy / len);
    let (nx, ny) = (-uy * width / 2.0, ux * width / 2.0);
    match shape {
        Shape::SquareLine => Some(vec![ax + nx, ay + ny, bx + nx, by + ny, bx - nx, by - ny, ax - nx, ay - ny]),
        Shape::Triangle => Some(if right > 0 {
            vec![ax + nx, ay + ny, bx + nx, by + ny, ax - nx, ay - ny]
        } else if right < 0 {
            vec![ax + nx, ay + ny, bx - nx, by - ny, ax - nx, ay - ny]
        } else {
            vec![ax + nx, ay + ny, bx, by, ax - nx, ay - ny]
        }),
        Shape::Ellipse => {
            let (cx, cy, ra, rb) = ((ax + bx) / 2.0, (ay + by) / 2.0, len / 2.0, width / 2.0);
            let n = 12.max(96.min((ra.max(rb) / 1.5) as i32 + 12));
            let mut p = Vec::with_capacity(n as usize * 2);
            for i in 0..n {
                let a = 2.0 * PI * i as f64 / n as f64;
                let (ca, sa) = (a.cos() as f32 * ra, a.sin() as f32 * rb);
                p.push(cx + ux * ca - uy * sa);
                p.push(cy + uy * ca + ux * sa);
            }
            Some(p)
        }
        _ => None,
    }
}
