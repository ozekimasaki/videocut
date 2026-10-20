use videocut_model::{effect_ids, EffectInstance};

use crate::error::ColorError;
use crate::params::{effect, float_param, optional_float, optional_string, string_param};
use crate::rgb::Rgb;

pub const CUBE: &str = "cube";
pub const INTENSITY: &str = "intensity";
const MAX_LUT_SIZE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LutKind {
    OneD,
    ThreeD,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CubeLut {
    kind: LutKind,
    size: usize,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
    samples: Vec<Rgb>,
}

impl CubeLut {
    pub fn kind(&self) -> LutKind {
        self.kind
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn sample(&self, rgb: Rgb) -> Rgb {
        match self.kind {
            LutKind::OneD => self.sample_1d(rgb),
            LutKind::ThreeD => self.sample_3d(rgb),
        }
    }

    fn sample_1d(&self, rgb: Rgb) -> Rgb {
        Rgb::new(
            self.sample_1d_channel(rgb.r, 0, |sample| sample.r),
            self.sample_1d_channel(rgb.g, 1, |sample| sample.g),
            self.sample_1d_channel(rgb.b, 2, |sample| sample.b),
        )
    }

    fn sample_1d_channel(&self, value: f32, channel: usize, component: impl Fn(Rgb) -> f32) -> f32 {
        let t = normalize(value, self.domain_min[channel], self.domain_max[channel]);
        let scaled = t * (self.size - 1) as f32;
        let i0 = (scaled.floor() as usize).min(self.size - 1);
        let i1 = (i0 + 1).min(self.size - 1);
        let frac = scaled - i0 as f32;
        let a = component(self.samples[i0]);
        let b = component(self.samples[i1]);
        a + (b - a) * frac
    }

    fn sample_3d(&self, rgb: Rgb) -> Rgb {
        let r = self.lattice(rgb.r, 0);
        let g = self.lattice(rgb.g, 1);
        let b = self.lattice(rgb.b, 2);
        let mut accum = Rgb::splat(0.0);
        for (bi, bw) in [(b.index0, 1.0 - b.frac), (b.index1, b.frac)] {
            for (gi, gw) in [(g.index0, 1.0 - g.frac), (g.index1, g.frac)] {
                for (ri, rw) in [(r.index0, 1.0 - r.frac), (r.index1, r.frac)] {
                    accum = accum + self.samples[self.index(ri, gi, bi)] * (rw * gw * bw);
                }
            }
        }
        accum
    }

    fn lattice(&self, value: f32, channel: usize) -> Lattice {
        let t = normalize(value, self.domain_min[channel], self.domain_max[channel]);
        let scaled = t * (self.size - 1) as f32;
        let index0 = (scaled.floor() as usize).min(self.size - 1);
        let index1 = (index0 + 1).min(self.size - 1);
        Lattice {
            index0,
            index1,
            frac: scaled - index0 as f32,
        }
    }

    fn index(&self, r: usize, g: usize, b: usize) -> usize {
        r + self.size * (g + self.size * b)
    }
}

struct Lattice {
    index0: usize,
    index1: usize,
    frac: f32,
}

pub fn parse_cube(text: &str) -> Result<CubeLut, ColorError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut kind: Option<LutKind> = None;
    let mut size = 0usize;
    let mut domain_min = [0.0; 3];
    let mut domain_max = [1.0; 3];
    let mut samples = Vec::new();

    for (line_number, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens[0].parse::<f32>().is_ok() {
            if size == 0 {
                return Err(ColorError::InvalidCube(format!(
                    "line {}: data appears before the LUT size",
                    line_number + 1
                )));
            }
            if tokens.len() != 3 {
                return Err(ColorError::InvalidCube(format!(
                    "line {}: expected 3 sample values",
                    line_number + 1
                )));
            }
            let mut channels = [0.0; 3];
            for (index, token) in tokens.iter().enumerate() {
                channels[index] = parse_float(token, line_number)?;
            }
            samples.push(Rgb::new(channels[0], channels[1], channels[2]));
            continue;
        }

        match tokens[0].to_ascii_uppercase().as_str() {
            "TITLE" => {}
            "LUT_1D_SIZE" => {
                if kind.is_some() {
                    return Err(ColorError::InvalidCube(
                        "a cube LUT declares one size".into(),
                    ));
                }
                size = parse_size(&tokens, line_number)?;
                kind = Some(LutKind::OneD);
            }
            "LUT_3D_SIZE" => {
                if kind.is_some() {
                    return Err(ColorError::InvalidCube(
                        "a cube LUT declares one size".into(),
                    ));
                }
                size = parse_size(&tokens, line_number)?;
                kind = Some(LutKind::ThreeD);
            }
            "DOMAIN_MIN" => domain_min = parse_domain(&tokens, line_number)?,
            "DOMAIN_MAX" => domain_max = parse_domain(&tokens, line_number)?,
            "LUT_1D_INPUT_RANGE" | "LUT_3D_INPUT_RANGE" => {
                let range = parse_range(&tokens, line_number)?;
                domain_min = [range[0]; 3];
                domain_max = [range[1]; 3];
            }
            _ => {}
        }
    }

    let Some(kind) = kind else {
        return Err(ColorError::InvalidCube("missing LUT size".into()));
    };
    let expected = match kind {
        LutKind::OneD => size,
        LutKind::ThreeD => size.saturating_pow(3),
    };
    if samples.len() != expected {
        return Err(ColorError::InvalidCube(format!(
            "expected {expected} samples, found {}",
            samples.len()
        )));
    }
    for channel in 0..3 {
        if !(domain_max[channel] - domain_min[channel]).is_finite()
            || domain_max[channel] <= domain_min[channel]
        {
            return Err(ColorError::InvalidCube(
                "domain max must be greater than domain min".into(),
            ));
        }
    }
    Ok(CubeLut {
        kind,
        size,
        domain_min,
        domain_max,
        samples,
    })
}

pub fn lut_effect(cube_source: impl Into<String>) -> EffectInstance {
    effect(
        effect_ids::LUT,
        vec![
            string_param(CUBE, cube_source.into()),
            float_param(INTENSITY, 1.0),
        ],
    )
}

pub(crate) fn apply(effect: &EffectInstance, pixels: &mut [Rgb]) -> Result<(), ColorError> {
    let source = optional_string(effect, CUBE, "")?;
    if source.trim().is_empty() {
        return Ok(());
    }
    let intensity = optional_float(effect, INTENSITY, 1.0)?.clamp(0.0, 1.0);
    if intensity <= f32::EPSILON {
        return Ok(());
    }
    let lut = parse_cube(&source)?;
    for pixel in pixels {
        let graded = lut.sample(*pixel);
        *pixel = pixel.lerp(graded, intensity);
    }
    Ok(())
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(index) => &line[..index],
        None => line,
    }
}

fn parse_size(tokens: &[&str], line_number: usize) -> Result<usize, ColorError> {
    if tokens.len() != 2 {
        return Err(ColorError::InvalidCube(format!(
            "line {}: LUT size needs one integer",
            line_number + 1
        )));
    }
    let size: usize = tokens[1].parse().map_err(|_| {
        ColorError::InvalidCube(format!(
            "line {}: LUT size is not an integer",
            line_number + 1
        ))
    })?;
    if !(2..=MAX_LUT_SIZE).contains(&size) {
        return Err(ColorError::InvalidCube(format!(
            "LUT size must be from 2 to {MAX_LUT_SIZE}"
        )));
    }
    Ok(size)
}

fn parse_domain(tokens: &[&str], line_number: usize) -> Result<[f32; 3], ColorError> {
    if tokens.len() != 4 {
        return Err(ColorError::InvalidCube(format!(
            "line {}: domain needs 3 values",
            line_number + 1
        )));
    }
    Ok([
        parse_float(tokens[1], line_number)?,
        parse_float(tokens[2], line_number)?,
        parse_float(tokens[3], line_number)?,
    ])
}

fn parse_range(tokens: &[&str], line_number: usize) -> Result<[f32; 2], ColorError> {
    if tokens.len() != 3 {
        return Err(ColorError::InvalidCube(format!(
            "line {}: input range needs 2 values",
            line_number + 1
        )));
    }
    Ok([
        parse_float(tokens[1], line_number)?,
        parse_float(tokens[2], line_number)?,
    ])
}

fn parse_float(token: &str, line_number: usize) -> Result<f32, ColorError> {
    let value: f32 = token.parse().map_err(|_| {
        ColorError::InvalidCube(format!(
            "line {}: `{token}` is not a number",
            line_number + 1
        ))
    })?;
    if !value.is_finite() {
        return Err(ColorError::InvalidCube(format!(
            "line {}: values must be finite",
            line_number + 1
        )));
    }
    Ok(value)
}

fn normalize(value: f32, min: f32, max: f32) -> f32 {
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use videocut_model::ParamValue;

    const IDENTITY_3D: &str = "\
LUT_3D_SIZE 2
0 0 0
1 0 0
0 1 0
1 1 0
0 0 1
1 0 1
0 1 1
1 1 1
";

    fn apply_one(effect: &EffectInstance, pixel: Rgb) -> Rgb {
        let mut pixels = [pixel];
        apply(effect, &mut pixels).unwrap();
        pixels[0]
    }

    #[test]
    fn parses_comments_title_and_domain() {
        let text = "\
# comment
TITLE \"grade\"
DOMAIN_MIN 0 0 0
DOMAIN_MAX 1 1 1
LUT_3D_SIZE 2
0 0 0
1 0 0
0 1 0
1 1 0
0 0 1
1 0 1
0 1 1
1 1 1
";
        let lut = parse_cube(text).unwrap();
        assert_eq!(lut.kind(), LutKind::ThreeD);
        assert_eq!(lut.size(), 2);
        assert!(
            lut.sample(Rgb::new(1.0, 0.0, 0.0))
                .max_abs_diff(Rgb::new(1.0, 0.0, 0.0))
                < 1e-5
        );
        assert!(lut.sample(Rgb::splat(0.5)).max_abs_diff(Rgb::splat(0.5)) < 1e-5);
    }

    #[test]
    fn trilinear_corner_uses_red_as_the_fastest_axis() {
        let text = "\
LUT_3D_SIZE 2
0 0 0
0 0 1
0 1 0
1 1 0
0 0 1
1 0 1
0 1 1
1 1 1
";
        let lut = parse_cube(text).unwrap();
        assert!(
            lut.sample(Rgb::new(1.0, 0.0, 0.0))
                .max_abs_diff(Rgb::new(0.0, 0.0, 1.0))
                < 1e-5
        );
    }

    #[test]
    fn one_d_lut_inverts_each_channel() {
        let lut = parse_cube("LUT_1D_SIZE 2\n1 1 1\n0 0 0\n").unwrap();
        assert_eq!(lut.kind(), LutKind::OneD);
        assert!(
            lut.sample(Rgb::new(0.0, 0.25, 1.0))
                .max_abs_diff(Rgb::new(1.0, 0.75, 0.0))
                < 1e-5
        );
    }

    #[test]
    fn domain_scales_the_lookup() {
        let text = format!("LUT_3D_INPUT_RANGE 0 2\n{IDENTITY_3D}");
        let lut = parse_cube(&text).unwrap();
        assert!(lut.sample(Rgb::splat(1.0)).max_abs_diff(Rgb::splat(0.5)) < 1e-5);
    }

    #[test]
    fn rejects_bad_cubes() {
        assert!(parse_cube("").is_err());
        assert!(parse_cube("LUT_3D_SIZE 2\n0 0 0\n").is_err());
        assert!(parse_cube("LUT_1D_SIZE 2\nLUT_3D_SIZE 2\n").is_err());
        assert!(parse_cube("0 0 0\n").is_err());
    }

    #[test]
    fn empty_cube_and_zero_intensity_are_identity() {
        let pixel = Rgb::new(0.2, 0.4, 0.8);
        assert_eq!(apply_one(&lut_effect(""), pixel), pixel);
        let mut effect = lut_effect(IDENTITY_3D);
        let intensity = effect
            .params
            .iter_mut()
            .find(|param| param.id == INTENSITY)
            .unwrap();
        intensity.value = ParamValue::Float(0.0);
        assert_eq!(apply_one(&effect, pixel), pixel);
    }

    #[test]
    fn intensity_mixes_toward_the_lut() {
        let mut effect = lut_effect("LUT_1D_SIZE 2\n0 0 0\n0 0 0\n");
        let intensity = effect
            .params
            .iter_mut()
            .find(|param| param.id == INTENSITY)
            .unwrap();
        intensity.value = ParamValue::Float(0.5);
        let graded = apply_one(&effect, Rgb::splat(1.0));
        assert!(graded.max_abs_diff(Rgb::splat(0.5)) < 1e-5);
    }
}
