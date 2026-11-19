//! Built-in figures for the figure picker. All are original designs made
//! here in code (CLEANROOM.md: the original app's bundled figures are not
//! copied); a plain stickman is a generic shape.

use crate::model::*;

/// (id, name) of every built-in figure, in picker order.
pub const ALL: [(&str, &str); 11] = [
    ("stickman", "Stickman"),
    ("thin", "Thin stickman"),
    ("hands", "Hands & feet"),
    ("chibi", "Chibi"),
    ("dog", "Dog"),
    ("bird", "Bird"),
    ("snake", "Snake"),
    ("rope", "Rope"),
    ("ball", "Ball"),
    ("stick", "Stick"),
    ("square", "Square"),
];

struct B {
    layer: String,
    nodes: Vec<Node>,
    thick: f32,
    color: Color,
}

impl B {
    fn new(layer: &str, thick: f32) -> B {
        let mut root = Node::new(layer, "main", None, Shape::None);
        root.draw_order = 0;
        B { layer: layer.into(), nodes: vec![root], thick, color: Color::BLACK }
    }

    fn seg(&mut self, id: &str, parent: &str, shape: Shape, rot: f32, len: f32) -> &mut Node {
        let mut n = Node::new(&self.layer, id, Some(parent), shape);
        n.rotation.value = Value::F(rot);
        n.length.value = Value::F(len);
        n.thickness.value = Value::F(self.thick);
        n.color.value = Value::C(self.color);
        n.draw_order = self.nodes.len() as i32;
        self.nodes.push(n);
        self.nodes.last_mut().expect("just pushed")
    }

    fn line(&mut self, id: &str, parent: &str, rot: f32, len: f32) -> &mut Node {
        self.seg(id, parent, Shape::Line, rot, len)
    }

    fn circle(&mut self, id: &str, parent: &str, rot: f32, len: f32) -> &mut Node {
        self.seg(id, parent, Shape::Circle, rot, len)
    }

    fn stickman(&mut self, head: f32, body: f32, arm: f32, leg: f32) {
        self.line("torso", "main", 90.0, body);
        self.circle("head", "torso", 0.0, head);
        self.line("armL1", "torso", -150.0, arm);
        self.line("armL2", "armL1", 20.0, arm);
        self.line("armR1", "torso", 150.0, arm);
        self.line("armR2", "armR1", -20.0, arm);
        self.line("legL1", "main", -110.0, leg);
        self.line("legL2", "legL1", 10.0, leg);
        self.line("legR1", "main", -70.0, leg);
        self.line("legR2", "legR1", -10.0, leg);
    }
}

/// A new figure layer of the given kind (None for an unknown id).
pub fn build(kind: &str, layer_id: &str, name: &str) -> Option<Layer> {
    let mut b = B::new(layer_id, 12.0);
    let mut scale = 2.0;
    match kind {
        "stickman" => b.stickman(50.0, 60.0, 45.0, 55.0),
        "thin" => {
            b.thick = 5.0;
            b.stickman(44.0, 62.0, 46.0, 56.0);
        }
        "hands" => {
            b.stickman(50.0, 60.0, 45.0, 55.0);
            b.thick = 12.0;
            b.circle("handL", "armL2", 0.0, 9.0).is_static = true;
            b.circle("handR", "armR2", 0.0, 9.0).is_static = true;
            b.thick = 10.0;
            b.line("footL", "legL2", 80.0, 14.0).is_static = true;
            b.line("footR", "legR2", -80.0, 14.0).is_static = true;
        }
        "chibi" => {
            b.thick = 14.0;
            b.stickman(84.0, 34.0, 26.0, 30.0);
        }
        "dog" => {
            b.thick = 10.0;
            b.line("back", "main", 0.0, 70.0);
            b.line("neck", "back", 50.0, 28.0);
            b.circle("head", "neck", -40.0, 30.0);
            b.seg("snout", "head", Shape::Line, -30.0, 18.0);
            b.seg("ear", "head", Shape::Triangle, 120.0, 14.0);
            b.line("frontL1", "back", -110.0, 30.0);
            b.line("frontL2", "frontL1", 10.0, 28.0);
            b.line("frontR1", "back", -80.0, 30.0);
            b.line("frontR2", "frontR1", 10.0, 28.0);
            b.line("backL1", "main", -110.0, 30.0);
            b.line("backL2", "backL1", 20.0, 28.0);
            b.line("backR1", "main", -80.0, 30.0);
            b.line("backR2", "backR1", 20.0, 28.0);
            b.line("tail1", "main", 140.0, 22.0);
            b.thick = 7.0;
            b.line("tail2", "tail1", 20.0, 20.0);
        }
        "bird" => {
            b.thick = 9.0;
            b.seg("body", "main", Shape::Ellipse, 0.0, 60.0);
            b.circle("head", "body", 30.0, 30.0);
            b.seg("beak", "head", Shape::Triangle, -20.0, 16.0);
            b.line("wingL1", "body", 120.0, 38.0);
            b.line("wingL2", "wingL1", -30.0, 32.0);
            b.line("wingR1", "body", 100.0, 38.0);
            b.line("wingR2", "wingR1", -30.0, 32.0);
            b.thick = 5.0;
            b.line("legL", "main", -100.0, 26.0);
            b.line("legR", "main", -80.0, 26.0);
            b.line("tail", "main", 170.0, 26.0);
        }
        "snake" => {
            let mut parent = "main".to_string();
            for i in 0..12 {
                b.thick = 18.0 - i as f32 * 1.2;
                let id = format!("s{}", i + 1);
                b.line(&id, &parent, if i == 0 { 0.0 } else if i % 2 == 0 { 18.0 } else { -18.0 }, 18.0);
                parent = id;
            }
            b.thick = 22.0;
            b.circle("head", "main", 180.0, 26.0);
        }
        "rope" => {
            b.thick = 6.0;
            let mut parent = "main".to_string();
            for i in 0..10 {
                let id = format!("r{}", i + 1);
                b.line(&id, &parent, if i == 0 { -90.0 } else { 0.0 }, 14.0);
                parent = id;
            }
            scale = 1.5;
        }
        "ball" => {
            b.circle("ball", "main", 90.0, 40.0);
        }
        "stick" => {
            b.line("stick", "main", 90.0, 80.0);
        }
        "square" => {
            let n = b.seg("square", "main", Shape::Polygon, 90.0, 60.0);
            n.polygon_sides = 4;
        }
        _ => return None,
    }
    let mut l = Layer::figure(layer_id, Some(name));
    l.transform.scale_x.value = Value::F(scale);
    l.transform.scale_y.value = Value::F(scale);
    let LayerKind::Figure(f) = &mut l.kind else { unreachable!() };
    f.set_nodes(b.nodes).ok()?;
    Some(l)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_figure_builds_and_draws() {
        for (id, name) in super::ALL {
            let l = super::build(id, "L9", name).unwrap_or_else(|| panic!("{id}"));
            let mut p = crate::session::new_project();
            let root = p.root_symbol.clone();
            p.root_mut().unwrap().layers.push(std::sync::Arc::new(l));
            let text = crate::codec::encode(&p);
            let back = crate::codec::decode(&text).unwrap();
            let again = crate::codec::encode(&back);
            if again != text {
                let line = text.lines().zip(again.lines()).position(|(a, b)| a != b).unwrap_or(0);
                panic!("{id} saves and loads: line {}: {:?} / {:?}", line + 1, text.lines().nth(line), again.lines().nth(line));
            }
            let list = crate::tess::tessellate(&crate::eval::evaluate(&back, &root, 0.0), None);
            assert!(list.triangle_count() > 20, "{id} draws");
        }
    }
}
