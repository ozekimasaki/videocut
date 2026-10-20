use std::ops::Mul;

use serde::{Deserialize, Serialize};

/// Column-vector affine transform: `x' = a x + c y + tx`, `y' = b x + d y + ty`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Affine {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: x,
            ty: y,
        }
    }

    pub fn scale(x: f64, y: f64) -> Self {
        Self {
            a: x,
            b: 0.0,
            c: 0.0,
            d: y,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn rotate(degrees: f64) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Skew along x. `degrees` is the shear angle.
    pub fn skew_x(degrees: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: degrees.to_radians().tan(),
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.tx,
            self.b * x + self.d * y + self.ty,
        )
    }
}

impl Mul for Affine {
    type Output = Self;

    /// `self * other` applies `other` to the point first.
    fn mul(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            tx: self.a * other.tx + self.c * other.ty + self.tx,
            ty: self.b * other.tx + self.d * other.ty + self.ty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_then_translate() {
        let m = Affine::translate(1.0, 2.0) * Affine::translate(3.0, 4.0);
        let (x, y) = m.apply(0.0, 0.0);
        assert_close(x, 4.0);
        assert_close(y, 6.0);
    }

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }
}
