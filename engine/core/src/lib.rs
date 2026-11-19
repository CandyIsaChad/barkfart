//! Barkfart engine core: the project model, evaluator and tessellator.
//!
//! Pure Rust, no GPU and no platform code, so everything here runs in plain
//! `cargo test` on any machine. This is the start of the port from the Java
//! version (git tag `java-reference`), which stays the reference until this
//! crate matches its outputs.

pub mod tess;

pub use tess::{DrawList, Vertex};

/// A demo stick figure (the rendering spike): a body, two arms (the right
/// one waving with `t` seconds), two legs and a head, around the origin, y up.
pub fn demo_figure(t: f32) -> DrawList {
    let mut d = DrawList::default();
    let black = [0, 0, 0, 255];
    let w = 10.0;
    let hip = (0.0, -10.0);
    let neck = (0.0, 50.0);
    d.line(hip, neck, w, black);
    let wave = (t * std::f32::consts::TAU).sin() * 35.0;
    let elbow_r = polar(neck, -30.0 + wave * 0.5, 40.0);
    d.line(neck, elbow_r, w, black);
    d.line(elbow_r, polar(elbow_r, 20.0 + wave, 35.0), w, black);
    let elbow_l = polar(neck, 210.0, 40.0);
    d.line(neck, elbow_l, w, black);
    d.line(elbow_l, polar(elbow_l, 250.0, 35.0), w, black);
    d.line(hip, polar(hip, -70.0, 55.0), w, black);
    d.line(hip, polar(hip, -110.0, 55.0), w, black);
    d.circle((0.0, 75.0), 22.0, black);
    d
}

fn polar(from: (f32, f32), deg: f32, len: f32) -> (f32, f32) {
    let r = deg.to_radians();
    (from.0 + len * r.cos(), from.1 + len * r.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_figure_is_triangles_within_bounds() {
        let d = demo_figure(0.25);
        assert!(!d.vertices.is_empty());
        assert_eq!(d.vertices.len() % 3, 0);
        for v in &d.vertices {
            assert!(v.pos[0].abs() < 150.0 && v.pos[1].abs() < 150.0);
        }
    }
}
