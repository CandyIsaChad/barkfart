//! Triangles with per-vertex colour, the renderer's input (like the Java
//! `geom.Tessellator` / `DrawList`).

use std::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 2],
    /// Straight RGBA8.
    pub rgba: [u8; 4],
}

/// A flat triangle list (every three vertices are one triangle).
#[derive(Default, Debug, Clone)]
pub struct DrawList {
    pub vertices: Vec<Vertex>,
}

/// Chord tolerance for arcs, in drawing units.
const TOLERANCE: f32 = 0.25;

fn segments(r: f32, span: f32) -> usize {
    if r <= TOLERANCE {
        return 6;
    }
    let step = 2.0 * (1.0 - TOLERANCE / r).acos();
    ((span / step).ceil() as usize).clamp(6, 256)
}

impl DrawList {
    pub fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), rgba: [u8; 4]) {
        for p in [a, b, c] {
            self.vertices.push(Vertex { pos: [p.0, p.1], rgba });
        }
    }

    /// A fan from the centre over an arc (a disc when span is 2 pi).
    pub fn fan(&mut self, c: (f32, f32), r: f32, start: f32, span: f32, rgba: [u8; 4]) {
        let n = segments(r, span);
        let mut prev = (c.0 + r * start.cos(), c.1 + r * start.sin());
        for i in 1..=n {
            let a = start + span * i as f32 / n as f32;
            let p = (c.0 + r * a.cos(), c.1 + r * a.sin());
            self.tri(c, prev, p, rgba);
            prev = p;
        }
    }

    pub fn circle(&mut self, c: (f32, f32), r: f32, rgba: [u8; 4]) {
        self.fan(c, r, 0.0, 2.0 * PI, rgba);
    }

    /// A thick line with round caps (a dot when the ends meet).
    pub fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, rgba: [u8; 4]) {
        let hw = width / 2.0;
        if hw <= 0.0 {
            return;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            self.circle(a, hw, rgba);
            return;
        }
        let (nx, ny) = (-dy / len * hw, dx / len * hw);
        self.tri((a.0 + nx, a.1 + ny), (b.0 + nx, b.1 + ny), (b.0 - nx, b.1 - ny), rgba);
        self.tri((a.0 + nx, a.1 + ny), (b.0 - nx, b.1 - ny), (a.0 - nx, a.1 - ny), rgba);
        let ang = ny.atan2(nx);
        self.fan(a, hw, ang, PI, rgba);
        self.fan(b, hw, ang + PI, PI, rgba);
    }
}
