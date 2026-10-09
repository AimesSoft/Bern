//! Deterministic OKLCH palette generation and contrast checks on the final
//! quantized sRGB colors sent to CSS. No parallel JavaScript color algorithm.

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub(crate) struct Theme {
    algorithm_version: u32,
    mode: String,
    tokens: BTreeMap<&'static str, String>,
}

#[derive(Clone, Copy, Debug)]
struct Color(u8, u8, u8);

impl Color {
    fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    fn luminance(self) -> f64 {
        fn linear(channel: u8) -> f64 {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2)
    }

    fn contrast(self, other: Self) -> f64 {
        let a = self.luminance();
        let b = other.luminance();
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

fn linear_from_oklch(lightness: f64, chroma: f64, hue: f64) -> [f64; 3] {
    let angle = hue.to_radians();
    let a = chroma * angle.cos();
    let b = chroma * angle.sin();
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}

fn oklch(lightness: f64, chroma: f64, hue: f64) -> Color {
    let mut mapped_chroma = chroma;
    let mut rgb = linear_from_oklch(lightness, mapped_chroma, hue);
    // Keep hue/lightness while reducing chroma into the display gamut.
    for _ in 0..48 {
        if rgb.iter().all(|value| (0.0..=1.0).contains(value)) {
            break;
        }
        mapped_chroma *= 0.85;
        rgb = linear_from_oklch(lightness, mapped_chroma, hue);
    }
    fn encode(value: f64) -> u8 {
        let value = value.clamp(0.0, 1.0);
        let srgb = if value <= 0.003_130_8 {
            12.92 * value
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u8
    }
    Color(encode(rgb[0]), encode(rgb[1]), encode(rgb[2]))
}

/// Find the nearest lightness that meets the contrast target against every
/// surface where this foreground is used. Failure is explicit, never silent.
fn readable(
    lightness: f64,
    chroma: f64,
    hue: f64,
    surfaces: &[Color],
    minimum: f64,
) -> Result<Color, String> {
    let acceptable = |color: Color| {
        surfaces
            .iter()
            .all(|surface| color.contrast(*surface) >= minimum)
    };
    let initial = oklch(lightness, chroma, hue);
    if acceptable(initial) {
        return Ok(initial);
    }
    for step in 1..=200 {
        let delta = f64::from(step) / 200.0;
        for candidate in [lightness - delta, lightness + delta] {
            if !(0.0..=1.0).contains(&candidate) {
                continue;
            }
            let color = oklch(candidate, chroma, hue);
            if acceptable(color) {
                return Ok(color);
            }
        }
    }
    Err("无法生成满足对比度的主题".into())
}

fn palette(mode: &str) -> Result<BTreeMap<&'static str, Color>, String> {
    let dark = match mode {
        "light" => false,
        "dark" => true,
        _ => return Err("未知外观模式".into()),
    };
    let background = oklch(if dark { 0.17 } else { 0.98 }, 0.006, 95.0);
    let surface = oklch(if dark { 0.215 } else { 0.998 }, 0.004, 95.0);
    let muted_surface = oklch(if dark { 0.255 } else { 0.951 }, 0.008, 95.0);
    let surfaces = [background, surface, muted_surface];
    let text = readable(if dark { 0.94 } else { 0.255 }, 0.012, 80.0, &surfaces, 7.0)?;
    let muted_text = readable(if dark { 0.76 } else { 0.53 }, 0.015, 80.0, &surfaces, 4.5)?;
    let accent = readable(if dark { 0.76 } else { 0.47 }, 0.11, 161.0, &surfaces, 4.5)?;
    let on_accent = readable(if dark { 0.16 } else { 0.998 }, 0.004, 95.0, &[accent], 4.5)?;
    let accent_hover = readable(if dark { 0.81 } else { 0.42 }, 0.11, 161.0, &[surface], 4.5)?;
    let on_accent_hover = readable(
        if dark { 0.16 } else { 0.998 },
        0.004,
        95.0,
        &[accent_hover],
        4.5,
    )?;
    let error = readable(if dark { 0.77 } else { 0.47 }, 0.15, 25.0, &surfaces, 4.5)?;
    let border = oklch(if dark { 0.35 } else { 0.89 }, 0.008, 95.0);
    let control_border = readable(0.61, 0.014, 95.0, &[surface, muted_surface], 3.0)?;
    Ok(BTreeMap::from([
        ("background", background),
        ("surface", surface),
        ("surface-muted", muted_surface),
        ("text", text),
        ("text-muted", muted_text),
        ("accent", accent),
        ("on-accent", on_accent),
        ("accent-hover", accent_hover),
        ("on-accent-hover", on_accent_hover),
        ("error", error),
        ("border", border),
        ("control-border", control_border),
        ("focus", accent),
    ]))
}

pub(crate) fn generate(mode: &str) -> Result<Theme, String> {
    Ok(Theme {
        algorithm_version: 1,
        mode: mode.into(),
        tokens: palette(mode)?
            .into_iter()
            .map(|(name, color)| (name, color.css()))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_css_colors_meet_their_declared_surface_contrasts() {
        for mode in ["light", "dark"] {
            let palette = palette(mode).unwrap();
            for surface in ["background", "surface", "surface-muted"] {
                assert!(
                    palette["text"].contrast(palette[surface]) >= 7.0,
                    "{mode}/{surface}"
                );
                for foreground in ["text-muted", "accent", "error", "focus"] {
                    assert!(
                        palette[foreground].contrast(palette[surface]) >= 4.5,
                        "{mode}/{foreground}/{surface}"
                    );
                }
            }
            for surface in ["surface", "surface-muted"] {
                assert!(palette["control-border"].contrast(palette[surface]) >= 3.0);
            }
            assert!(palette["on-accent"].contrast(palette["accent"]) >= 4.5);
            assert!(palette["on-accent-hover"].contrast(palette["accent-hover"]) >= 4.5);
        }
    }

    #[test]
    fn palette_is_deterministic_css_and_modes_are_distinct() {
        let first = serde_json::to_string(&generate("light").unwrap()).unwrap();
        assert_eq!(
            first,
            serde_json::to_string(&generate("light").unwrap()).unwrap()
        );
        assert_ne!(
            first,
            serde_json::to_string(&generate("dark").unwrap()).unwrap()
        );
        for value in generate("light").unwrap().tokens.values() {
            assert_eq!(value.len(), 7);
            assert!(value.starts_with('#'));
            assert!(value[1..].bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
        assert!(generate("system").is_err());
    }
}
