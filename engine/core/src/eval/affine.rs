/// Immutable 2D affine transform: x' = a x + c y + tx, y' = b x + d y + ty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn translate(x: f64, y: f64) -> Affine {
        Affine { tx: x, ty: y, ..Affine::IDENTITY }
    }

    pub fn rotate_deg(deg: f64) -> Affine {
        let r = deg.to_radians();
        let (cs, sn) = (r.cos(), r.sin());
        Affine { a: cs, b: sn, c: -sn, d: cs, tx: 0.0, ty: 0.0 }
    }

    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine { a: sx, b: 0.0, c: 0.0, d: sy, tx: 0.0, ty: 0.0 }
    }

    /// self * o: applies o first, then self.
    pub fn mul(&self, o: &Affine) -> Affine {
        Affine {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }

    pub fn map_x(&self, x: f64, y: f64) -> f64 {
        self.a * x + self.c * y + self.tx
    }

    pub fn map_y(&self, x: f64, y: f64) -> f64 {
        self.b * x + self.d * y + self.ty
    }

    /// Uniform scale estimate (stroke widths, radii).
    pub fn scale_factor(&self) -> f64 {
        (self.a * self.d - self.b * self.c).abs().sqrt()
    }

    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    pub fn inverse(&self) -> Affine {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-12 {
            return Affine::IDENTITY;
        }
        let (ia, ib, ic, id) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Affine { a: ia, b: ib, c: ic, d: id, tx: -(ia * self.tx + ic * self.ty), ty: -(ib * self.tx + id * self.ty) }
    }

    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty].iter().all(|v| v.is_finite())
    }
}
