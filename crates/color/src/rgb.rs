/// Display-referred RGB sample. Channels are linear-light and are not clamped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    pub const fn splat(value: f32) -> Self {
        Self::new(value, value, value)
    }

    pub fn mul_rgb(self, other: Self) -> Self {
        Self::new(self.r * other.r, self.g * other.g, self.b * other.b)
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self::new(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
        )
    }

    pub fn max_abs_diff(self, other: Self) -> f32 {
        (self.r - other.r)
            .abs()
            .max((self.g - other.g).abs())
            .max((self.b - other.b).abs())
    }
}

impl std::ops::Add for Rgb {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self::new(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b)
    }
}

impl std::ops::Mul<f32> for Rgb {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self::new(self.r * rhs, self.g * rhs, self.b * rhs)
    }
}

/// Rec.709 luma. Used only to place a pixel on the shadow, midtone, and highlight masks.
pub fn luma(rgb: Rgb) -> f32 {
    0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b
}
