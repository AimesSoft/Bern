//! Color spaces used by the algorithm: 8-bit sRGB for output, OKLab and
//! OKLCH for perceptual adjustments, and relative luminance for contrast.
//!
//! The OKLab matrices are Björn Ottosson's published reference values.

use std::fmt;

/// An 8-bit sRGB color, the only encoding snapshots emit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Srgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Srgb {
    pub const WHITE: Srgb = Srgb::new(255, 255, 255);
    pub const BLACK: Srgb = Srgb::new(0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#RRGGBB`.
    pub fn parse(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }

    /// `#rrggbb`.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    fn linear(self) -> [f64; 3] {
        [self.r, self.g, self.b].map(|channel| to_linear(f64::from(channel) / 255.0))
    }

    /// Relative luminance from correctly linearized sRGB.
    pub fn luminance(self) -> f64 {
        let [r, g, b] = self.linear();
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub fn oklch(self) -> Oklch {
        Oklab::from_linear(self.linear()).lch()
    }
}

impl fmt::Display for Srgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

/// `(Y lighter + 0.05) / (Y darker + 0.05)`.
pub fn contrast(a: Srgb, b: Srgb) -> f64 {
    let (x, y) = (a.luminance(), b.luminance());
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn to_linear(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(value: f64) -> f64 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

impl Oklab {
    fn from_linear([r, g, b]: [f64; 3]) -> Self {
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        Self {
            l: 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
            a: 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
            b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
        }
    }

    fn to_linear(self) -> [f64; 3] {
        let l = (self.l + 0.396_337_777_4 * self.a + 0.215_803_757_3 * self.b).powi(3);
        let m = (self.l - 0.105_561_345_8 * self.a - 0.063_854_172_8 * self.b).powi(3);
        let s = (self.l - 0.089_484_177_5 * self.a - 1.291_485_548_0 * self.b).powi(3);
        [
            4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
            -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
            -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s,
        ]
    }

    fn lch(self) -> Oklch {
        let c = (self.a * self.a + self.b * self.b).sqrt();
        let h = self.b.atan2(self.a).to_degrees().rem_euclid(360.0);
        Oklch { l: self.l, c, h }
    }
}

/// Perceptual lightness `l` (0–1), chroma `c` and hue `h` in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

impl Oklch {
    pub const fn new(l: f64, c: f64, h: f64) -> Self {
        Self { l, c, h }
    }

    fn lab(self) -> Oklab {
        let h = self.h.to_radians();
        Oklab {
            l: self.l,
            a: self.c * h.cos(),
            b: self.c * h.sin(),
        }
    }

    fn in_gamut(self) -> bool {
        const EPSILON: f64 = 1e-6;
        self.lab()
            .to_linear()
            .iter()
            .all(|channel| (-EPSILON..=1.0 + EPSILON).contains(channel))
    }

    /// Map into sRGB by lowering chroma at the same lightness and hue,
    /// then encode as 8-bit sRGB. The result is the color that will be
    /// displayed; callers measure contrast on it, not on the OKLCH input.
    pub fn to_srgb(self) -> Srgb {
        let l = self.l.clamp(0.0, 1.0);
        let mut color = Oklch::new(l, self.c.max(0.0), self.h);
        if !color.in_gamut() {
            let (mut low, mut high) = (0.0, color.c);
            for _ in 0..32 {
                let middle = (low + high) / 2.0;
                if Oklch::new(l, middle, self.h).in_gamut() {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            color = Oklch::new(l, low, self.h);
        }
        let [r, g, b] = color
            .lab()
            .to_linear()
            .map(|channel| (from_linear(channel.clamp(0.0, 1.0)) * 255.0).round() as u8);
        Srgb::new(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_matches_the_wcag_reference_points() {
        assert!((contrast(Srgb::WHITE, Srgb::BLACK) - 21.0).abs() < 1e-9);
        assert!((contrast(Srgb::WHITE, Srgb::WHITE) - 1.0).abs() < 1e-9);
        // #767676 is the classic lightest grey with 4.5:1 on white.
        let grey = Srgb::parse("#767676").unwrap();
        assert!(contrast(grey, Srgb::WHITE) >= 4.5);
        assert!(contrast(Srgb::parse("#777777").unwrap(), Srgb::WHITE) < 4.5);
    }

    #[test]
    fn oklch_round_trips_in_gamut_colors() {
        for hex in [
            "#000000", "#ffffff", "#2f6fe4", "#e4572e", "#76b041", "#808080",
        ] {
            let color = Srgb::parse(hex).unwrap();
            assert_eq!(color.oklch().to_srgb(), color, "{hex}");
        }
        // Reference: sRGB red is about L 0.628, C 0.258, h 29.2.
        let red = Srgb::new(255, 0, 0).oklch();
        assert!((red.l - 0.628).abs() < 0.001 && (red.c - 0.258).abs() < 0.001);
        assert!((red.h - 29.23).abs() < 0.05);
    }

    #[test]
    fn out_of_gamut_colors_keep_lightness_and_hue() {
        let wanted = Oklch::new(0.7, 0.4, 145.0);
        let mapped = wanted.to_srgb().oklch();
        assert!((mapped.l - 0.7).abs() < 0.01);
        assert!((mapped.h - 145.0).abs() < 2.0);
        assert!(mapped.c < 0.4);
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(Srgb::parse("#2F6FE4"), Some(Srgb::new(0x2f, 0x6f, 0xe4)));
        assert_eq!(Srgb::parse("2f6fe4"), None);
        assert_eq!(Srgb::parse("#2f6fe"), None);
        assert_eq!(Srgb::parse("#2f6fgg"), None);
    }
}
