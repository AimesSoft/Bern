//! HDR pictures for targets whose UI toolkit cannot show them.
//!
//! Recognizes PNGs whose cICP chunk names the PQ or HLG transfer and JPEGs
//! with a gain map (ISO 21496-1, or the XMP of Ultra HDR), and decodes them
//! to linear light where 1.0 is SDR white: 203 nits for PQ and HLG
//! (BT.2408), the base picture's white for gain maps. Every other picture is
//! left to the platform: [`decode`] returns `None` for it.
//!
//! The Windows target draws the result in an scRGB swap chain; the GTK
//! target gives it to GTK 4.16 or later with a linear BT.2020 color state,
//! and to older GTK as [`HdrPicture::to_sdr`].

mod color;
mod gainmap;
mod png_hdr;

pub use color::Primaries;

/// How a picture carries HDR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A PNG in the PQ transfer function.
    Pq,
    /// A PNG in the HLG transfer function.
    Hlg,
    /// A JPEG with a gain map.
    GainMap,
}

/// A decoded HDR picture.
#[derive(Clone, Debug)]
pub struct HdrPicture {
    pub width: u32,
    pub height: u32,
    /// RGBA rows, top to bottom, upright: linear light in `primaries` with
    /// straight alpha, 1.0 at SDR white.
    pub pixels: Vec<f32>,
    pub primaries: Primaries,
    /// The brightest color component, at least 1.
    pub peak: f32,
    pub kind: Kind,
}

/// Linear RGBA as stored, before its EXIF orientation is applied.
pub(crate) struct Decoded {
    width: usize,
    height: usize,
    pixels: Vec<f32>,
    orientation: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// How `data` carries HDR, read from its headers only; `None` for SDR
/// pictures and anything else.
pub fn kind(data: &[u8]) -> Option<Kind> {
    if let Some(transfer) = png_hdr::transfer(data) {
        return match transfer {
            16 => Some(Kind::Pq),
            18 => Some(Kind::Hlg),
            _ => None,
        };
    }
    gainmap::Segments::read(data)
        .filter(|segments| segments.announces_gain_map())
        .map(|_| Kind::GainMap)
}

/// Decode an HDR picture into linear light in `primaries`.
///
/// `headroom` is the display's peak over its SDR white: a gain map applies
/// as much of its gain as that allows (none at 1.0, so the SDR picture as
/// its author made it), and all of it without a value. PQ and HLG are
/// absolute and ignore it. `Ok(None)` for pictures that are not HDR.
pub fn decode(
    data: &[u8],
    headroom: Option<f32>,
    primaries: Primaries,
) -> Result<Option<HdrPicture>, Error> {
    let (kind, decoded) = match kind(data) {
        Some(Kind::GainMap) => (Kind::GainMap, gainmap::decode(data, headroom, primaries)?),
        Some(kind) => (kind, png_hdr::decode(data, primaries)?),
        None => return Ok(None),
    };
    let Some(decoded) = decoded else {
        return Ok(None);
    };
    let (width, height, pixels) = orient(
        decoded.width,
        decoded.height,
        decoded.pixels,
        decoded.orientation,
    );
    let peak = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[0], p[1], p[2]])
        .fold(1.0f32, f32::max);
    Ok(Some(HdrPicture {
        width: width as u32,
        height: height as u32,
        pixels,
        primaries,
        peak,
        kind,
    }))
}

/// Turn RGBA pixels upright by an EXIF orientation (1 to 8).
fn orient(
    width: usize,
    height: usize,
    pixels: Vec<f32>,
    orientation: u16,
) -> (usize, usize, Vec<f32>) {
    if !(2..=8).contains(&orientation) {
        return (width, height, pixels);
    }
    let turned = orientation >= 5;
    let (w, h) = if turned {
        (height, width)
    } else {
        (width, height)
    };
    let mut out = vec![0.0; pixels.len()];
    for y in 0..h {
        for x in 0..w {
            // Where the upright pixel (x, y) is stored.
            let (sx, sy) = match orientation {
                2 => (width - 1 - x, y),
                3 => (width - 1 - x, height - 1 - y),
                4 => (x, height - 1 - y),
                5 => (y, x),
                6 => (y, height - 1 - x),
                7 => (width - 1 - y, height - 1 - x),
                _ => (width - 1 - y, x),
            };
            let from = (sy * width + sx) * 4;
            let to = (y * w + x) * 4;
            out[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
        }
    }
    (w, h, out)
}

impl HdrPicture {
    /// The picture for an SDR display, as 8-bit sRGB RGBA with straight
    /// alpha. Highlights above SDR white are compressed into it, keeping
    /// each pixel's hue; a picture that does not exceed SDR white is only
    /// clipped to the sRGB gamut.
    pub fn to_sdr(&self) -> Vec<u8> {
        let to_srgb = color::conversion(&self.primaries.to_xyz(), Primaries::Bt709);
        // Identity below the knee, then a curve that reaches 1.0 at the
        // peak with a continuous slope.
        const KNEE: f32 = 0.75;
        let peak = self.peak;
        let a = (1.0 - KNEE) / (peak - 1.0).max(1e-6);
        let compress = |m: f32| -> f32 {
            if peak <= 1.0 || m <= KNEE {
                return m;
            }
            let t = (m - KNEE) / (peak - KNEE);
            KNEE + (1.0 - KNEE) * t * (1.0 + a) / (t + a)
        };
        self.pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| {
                let rgb = color::apply(&to_srgb, [p[0], p[1], p[2]]).map(|v| v.max(0.0));
                let m = rgb[0].max(rgb[1]).max(rgb[2]);
                let scale = if m > 0.0 { compress(m) / m } else { 1.0 };
                let code = |v: f32| {
                    (color::linear_to_srgb((v * scale).clamp(0.0, 1.0)) * 255.0).round() as u8
                };
                [
                    code(rgb[0]),
                    code(rgb[1]),
                    code(rgb[2]),
                    (p[3].clamp(0.0, 1.0) * 255.0).round() as u8,
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_moves_pixels_upright() {
        // A 2×1 picture: red then green, stored for orientation 6 (turned
        // 90° clockwise to show), becomes 1×2 with red on top.
        let red = [1.0, 0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0, 1.0];
        let pixels: Vec<f32> = red.iter().chain(green.iter()).copied().collect();
        let (w, h, out) = orient(2, 1, pixels.clone(), 6);
        assert_eq!((w, h), (1, 2));
        assert_eq!(&out[0..4], &red);
        let (w, h, out) = orient(2, 1, pixels, 2);
        assert_eq!((w, h), (2, 1));
        assert_eq!(&out[0..4], &green);
    }

    #[test]
    fn sdr_rendition_keeps_white_and_compresses_highlights() {
        let picture = |values: &[f32]| HdrPicture {
            width: values.len() as u32,
            height: 1,
            pixels: values.iter().flat_map(|&v| [v, v, v, 1.0]).collect(),
            primaries: Primaries::Bt709,
            peak: values.iter().copied().fold(1.0, f32::max),
            kind: Kind::Pq,
        };
        // Within SDR white nothing changes.
        assert_eq!(picture(&[1.0, 0.5]).to_sdr()[0], 255);
        // With highlights, the peak becomes white and SDR white stays
        // brighter than the knee.
        let sdr = picture(&[4.0, 1.0, 0.5]).to_sdr();
        assert_eq!(sdr[0], 255);
        assert!(sdr[4] > 225 && sdr[4] < 255, "{}", sdr[4]);
        assert!(sdr[8] < sdr[4]);
    }
}
