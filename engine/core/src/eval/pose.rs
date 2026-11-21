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
        let links = connector_links(f);
        // a connector aimed at a joint later in the list needs that joint first: a second pass
        let passes = if links.iter().any(|(c, (e, _))| e > c) { 2 } else { 1 };
        for pass in 0..passes {
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
            // a connector sits on the line from its parent to another joint
            // (Stick Nodes connectors; IK rigs are built from them)
            if let Some(&(ei, spec)) = links.get(&i) {
                if ei < i || pass == 1 {
                    let (ax, ay, ex, ey) = (p.x[pi], p.y[pi], p.x[ei], p.y[ei]);
                    let (dx, dy) = (ex - ax, ey - ay);
                    let d = dx.hypot(dy);
                    let (x, y) = if !spec.by_value {
                        let f = spec.percent as f64;
                        if spec.reversed { (ex - dx * f, ey - dy * f) } else { (ax + dx * f, ay + dy * f) }
                    } else if d < 1e-9 {
                        (ax, ay)
                    } else {
                        let v = (r.f(&node.length, t) * r.f(&node.scale, t)) as f64;
                        if spec.reversed { (ex - dx / d * v, ey - dy / d * v) } else { (ax + dx / d * v, ay + dy / d * v) }
                    };
                    p.x[i] = x;
                    p.y[i] = y;
                    p.angle[i] = if d < 1e-9 { base(nodes, &p, i, pi) + rot } else { dy.atan2(dx).to_degrees() };
                    continue;
                }
            }
            p.angle[i] = base(nodes, &p, i, pi) + rot;
            let len = (r.f(&node.length, t) * r.f(&node.scale, t)) as f64;
            let rad = p.angle[i].to_radians();
            p.x[i] = p.x[pi] + len * rad.cos();
            p.y[i] = p.y[pi] + len * rad.sin();
        }
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

/// How a connector is placed (from its imported Stick Nodes data).
#[derive(Clone, Copy, Debug)]
pub struct ConnectorSpec {
    pub by_value: bool,
    pub percent: f32,
    pub value: f32,
    pub reversed: bool,
}

fn extra_num(n: &Node, k: &str) -> Option<f64> {
    let v = n.legacy_extras.get(k)?;
    let (_, num) = v.split_once(':')?;
    num.parse::<f64>().ok()
}

/// Connector node index -> (endpoint node index, placement), from the
/// figure's imported connector table ("fig.connections" on the main node).
pub fn connector_links(f: &Figure) -> std::collections::HashMap<usize, (usize, ConnectorSpec)> {
    let mut out = std::collections::HashMap::new();
    let nodes = f.nodes();
    let Some(table) = nodes.first().and_then(|r| r.legacy_extras.get("fig.connections")) else { return out };
    let table = table.strip_prefix("s:").unwrap_or(table);
    for pair in table.split(';') {
        let Some((a, b)) = pair.split_once(',') else { continue };
        let (Some(ci), Some(ei)) = (f.index_of(&format!("n{a}")), f.index_of(&format!("n{b}"))) else { continue };
        let n = &nodes[ci];
        if n.legacy_extras.get("connector").map(String::as_str) != Some("b:true") {
            continue;
        }
        let spec = ConnectorSpec {
            // measured against the original (2026-11-21): method 0 places the connector
            // its own length from the endpoint (reversed) or parent, method 1 by percent
            by_value: extra_num(n, "C.cMethod").unwrap_or(0.0) as i32 == 0,
            percent: extra_num(n, "C.cPercent").unwrap_or(0.5) as f32,
            value: extra_num(n, "C.cValue").unwrap_or(0.0) as f32,
            reversed: extra_num(n, "C.cReversed").unwrap_or(0.0) != 0.0,
        };
        out.insert(ci, (ei, spec));
    }
    out
}

/// Stretch multiplier of a "reset stretch impulse" node (None when not reset).
pub fn stretch_reset(n: &Node) -> Option<f32> {
    if extra_num(n, "L.smartStretchResetImpulse").unwrap_or(0.0) != 0.0 {
        Some(extra_num(n, "L.smartStretchMultiplier").unwrap_or(1.0) as f32)
    } else {
        None
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
