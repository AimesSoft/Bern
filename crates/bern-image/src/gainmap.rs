//! JPEGs with a gain map: an SDR picture plus a second, usually smaller
//! JPEG that says how much brighter each pixel is in the HDR rendition.
//!
//! The gain map's metadata is ISO 21496-1 (binary, in an APP2 segment of
//! the gain map, as Apple and Android write it today) or the XMP of
//! Google's Ultra HDR v1 (`hdrgm:` attributes). The pictures are located
//! through the MPF (CIPA DC-007) index of the primary image, or else as the
//! JPEG after the primary. The math follows Google's libultrahdr.

use crate::color::{self, Icc, Mat3, Primaries};
use crate::{Decoded, Error};
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

const ISO_URN: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";
const XMP_NS: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// The markers of one JPEG up to its first scan.
pub(crate) struct Segments<'a> {
    /// `(marker, payload)` of every APPn segment.
    app: Vec<(u8, &'a [u8], usize)>,
    /// Where the entropy-coded data of the first scan starts.
    scan: usize,
}

impl<'a> Segments<'a> {
    /// The segments of the JPEG starting at `data[0]`; `None` if it is not
    /// a JPEG.
    pub(crate) fn read(data: &'a [u8]) -> Option<Self> {
        if data.get(0..2)? != [0xFF, 0xD8] {
            return None;
        }
        let mut app = Vec::new();
        let mut at = 2;
        loop {
            // Fill bytes may precede a marker.
            while *data.get(at)? == 0xFF && *data.get(at + 1)? == 0xFF {
                at += 1;
            }
            if *data.get(at)? != 0xFF {
                return None;
            }
            let marker = *data.get(at + 1)?;
            let length = u16::from_be_bytes([*data.get(at + 2)?, *data.get(at + 3)?]) as usize;
            let payload = data.get(at + 4..at + 2 + length)?;
            if (0xE0..=0xEF).contains(&marker) {
                app.push((marker, payload, at + 4));
            }
            at += 2 + length;
            if marker == 0xDA {
                return Some(Self { app, scan: at });
            }
        }
    }

    fn app2<'s>(&'s self, prefix: &'s [u8]) -> impl Iterator<Item = (&'a [u8], usize)> + 's {
        self.app
            .iter()
            .filter(move |&&(marker, payload, _)| marker == 0xE2 && payload.starts_with(prefix))
            .map(move |&(_, payload, at)| (&payload[prefix.len()..], at + prefix.len()))
    }

    fn xmp(&self) -> Option<&'a str> {
        self.app.iter().find_map(|&(marker, payload, _)| {
            (marker == 0xE1 && payload.starts_with(XMP_NS))
                .then(|| std::str::from_utf8(&payload[XMP_NS.len()..]).ok())
                .flatten()
        })
    }

    /// The ICC profile, reassembled from its numbered chunks.
    fn icc(&self) -> Option<Vec<u8>> {
        let mut chunks: Vec<(u8, &[u8])> = self
            .app2(b"ICC_PROFILE\0")
            .filter_map(|(chunk, _)| Some((*chunk.first()?, chunk.get(2..)?)))
            .collect();
        if chunks.is_empty() {
            return None;
        }
        chunks.sort_by_key(|(sequence, _)| *sequence);
        Some(
            chunks
                .into_iter()
                .flat_map(|(_, data)| data.iter().copied())
                .collect(),
        )
    }

    /// The EXIF orientation, 1 when there is none.
    pub(crate) fn orientation(&self) -> u16 {
        self.app
            .iter()
            .find_map(|&(marker, payload, _)| {
                (marker == 0xE1 && payload.starts_with(b"Exif\0\0"))
                    .then(|| exif_orientation(&payload[6..]))
            })
            .flatten()
            .unwrap_or(1)
    }

    /// Whether this primary image announces a gain map.
    pub(crate) fn announces_gain_map(&self) -> bool {
        self.app2(ISO_URN).next().is_some()
            || self.xmp().is_some_and(|xmp| xmp.contains("hdrgm:Version"))
    }
}

/// The orientation tag (0x0112) of a TIFF structure, as in EXIF.
pub(crate) fn exif_orientation(tiff: &[u8]) -> Option<u16> {
    let little = match tiff.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |at: usize| {
        let b = tiff.get(at..at + 2)?;
        Some(if little {
            u16::from_le_bytes([b[0], b[1]])
        } else {
            u16::from_be_bytes([b[0], b[1]])
        })
    };
    let u32_at = |at: usize| {
        let b = tiff.get(at..at + 4)?;
        let b = [b[0], b[1], b[2], b[3]];
        Some(if little {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    (0..count).find_map(|i| {
        let entry = ifd + 2 + i * 12;
        (u16_at(entry)? == 0x0112)
            .then(|| u16_at(entry + 8))
            .flatten()
    })
}

/// Where the gain map JPEG starts in `data`.
fn gain_map_start(data: &[u8], primary: &Segments) -> Option<usize> {
    // MPF: a TIFF structure whose MP entries give each image's offset from
    // the start of that TIFF header.
    if let Some((mpf, tiff_at)) = primary.app2(b"MPF\0").next() {
        let little = mpf.get(0..2)? == b"II";
        let u16_at = |at: usize| {
            let b = mpf.get(at..at + 2)?;
            Some(if little {
                u16::from_le_bytes([b[0], b[1]])
            } else {
                u16::from_be_bytes([b[0], b[1]])
            })
        };
        let u32_at = |at: usize| {
            let b = mpf.get(at..at + 4)?;
            let b = [b[0], b[1], b[2], b[3]];
            Some(if little {
                u32::from_le_bytes(b)
            } else {
                u32::from_be_bytes(b)
            })
        };
        let found = (|| {
            let ifd = u32_at(4)? as usize;
            let count = u16_at(ifd)? as usize;
            let entries = (0..count).find_map(|i| {
                let entry = ifd + 2 + i * 12;
                (u16_at(entry)? == 0xB002)
                    .then(|| u32_at(entry + 8))
                    .flatten()
            })? as usize;
            // The second 16-byte MP entry: attributes, size, offset.
            let offset = u32_at(entries + 16 + 8)? as usize;
            let start = tiff_at.checked_add(offset)?;
            (offset != 0 && data.get(start..start + 2)? == [0xFF, 0xD8]).then_some(start)
        })();
        if found.is_some() {
            return found;
        }
    }
    // Otherwise the next JPEG after the primary's first scan: entropy-coded
    // data never holds 0xFF followed by 0xD8.
    let rest = data.get(primary.scan..)?;
    rest.windows(3)
        .position(|w| w == [0xFF, 0xD8, 0xFF])
        .map(|i| primary.scan + i)
}

/// Gain map metadata, per channel, in the units of libultrahdr's float
/// form: boosts and capacities as log2, offsets and gamma as plain values.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Metadata {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub gamma: [f32; 3],
    pub offset_sdr: [f32; 3],
    pub offset_hdr: [f32; 3],
    pub capacity_min: f32,
    pub capacity_max: f32,
    /// Apply the gain in the base picture's primaries rather than the
    /// alternate (HDR) picture's.
    pub base_space: bool,
}

impl Metadata {
    /// ISO 21496-1 binary metadata (after the URN); `Ok(None)` for what
    /// Bern does not apply, such as an HDR base picture.
    pub(crate) fn iso(data: &[u8]) -> Result<Option<Self>, Error> {
        let mut at = 0;
        let mut take = |n: usize| -> Result<&[u8], Error> {
            let bytes = data
                .get(at..at + n)
                .ok_or_else(|| Error::new("gain map metadata is truncated"))?;
            at += n;
            Ok(bytes)
        };
        let u16v = |b: &[u8]| u16::from_be_bytes([b[0], b[1]]);
        let u32v = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        if u16v(take(2)?) != 0 {
            return Ok(None);
        }
        let _writer = take(2)?;
        let flags = take(1)?[0];
        let channels = if flags & 0x80 != 0 { 3 } else { 1 };
        let base_space = flags & 0x40 != 0;
        if flags & 0x04 != 0 {
            // The base picture is the HDR one.
            return Ok(None);
        }
        let shared = if flags & 0x08 != 0 {
            Some(u32v(take(4)?))
        } else {
            None
        };
        let mut read = |signed: bool, denominator: Option<u32>| -> Result<f32, Error> {
            let n = u32v(take(4)?);
            let d = match denominator {
                Some(d) => d,
                None => u32v(take(4)?),
            };
            if d == 0 {
                return Err(Error::new("gain map metadata has a zero denominator"));
            }
            let n = if signed { n as i32 as f64 } else { n as f64 };
            Ok((n / d as f64) as f32)
        };
        let capacity_min = read(false, shared)?;
        let capacity_max = read(false, shared)?;
        let mut metadata = Self {
            min: [0.0; 3],
            max: [0.0; 3],
            gamma: [1.0; 3],
            offset_sdr: [0.0; 3],
            offset_hdr: [0.0; 3],
            capacity_min,
            capacity_max,
            base_space,
        };
        for c in 0..channels {
            metadata.min[c] = read(true, shared)?;
            metadata.max[c] = read(true, shared)?;
            metadata.gamma[c] = read(false, shared)?;
            metadata.offset_sdr[c] = read(true, shared)?;
            metadata.offset_hdr[c] = read(true, shared)?;
        }
        if channels == 1 {
            metadata.spread();
        }
        Ok(Some(metadata))
    }

    /// The `hdrgm:` XMP attributes of Ultra HDR v1.
    pub(crate) fn xmp(xmp: &str) -> Option<Self> {
        let value = |name: &str| -> Option<f32> { xmp_value(xmp, name)?.trim().parse().ok() };
        xmp_value(xmp, "hdrgm:Version")?;
        if xmp_value(xmp, "hdrgm:BaseRenditionIsHDR")
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
        {
            return None;
        }
        let max = value("hdrgm:GainMapMax")?;
        let mut metadata = Self {
            min: [value("hdrgm:GainMapMin").unwrap_or(0.0); 3],
            max: [max; 3],
            gamma: [value("hdrgm:Gamma").unwrap_or(1.0); 3],
            offset_sdr: [value("hdrgm:OffsetSDR").unwrap_or(1.0 / 64.0); 3],
            offset_hdr: [value("hdrgm:OffsetHDR").unwrap_or(1.0 / 64.0); 3],
            capacity_min: value("hdrgm:HDRCapacityMin").unwrap_or(0.0),
            capacity_max: value("hdrgm:HDRCapacityMax")?,
            base_space: true,
        };
        metadata.spread();
        Some(metadata)
    }

    /// Copy channel 0 to all three.
    fn spread(&mut self) {
        for c in 1..3 {
            self.min[c] = self.min[0];
            self.max[c] = self.max[0];
            self.gamma[c] = self.gamma[0];
            self.offset_sdr[c] = self.offset_sdr[0];
            self.offset_hdr[c] = self.offset_hdr[0];
        }
    }

    /// How much of the gain to apply for a display with `headroom` (peak
    /// over SDR white); full gain without one.
    pub(crate) fn weight(&self, headroom: Option<f32>) -> f32 {
        let Some(headroom) = headroom else { return 1.0 };
        let log = headroom.max(1.0).log2();
        if log >= self.capacity_max {
            return 1.0;
        }
        if self.capacity_max <= self.capacity_min {
            return if log >= self.capacity_max { 1.0 } else { 0.0 };
        }
        ((log - self.capacity_min) / (self.capacity_max - self.capacity_min)).clamp(0.0, 1.0)
    }
}

/// An XMP property, written as an attribute `name="value"` or an element
/// `<name>value</name>`.
fn xmp_value<'a>(xmp: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(found) = xmp[from..].find(name) {
        let at = from + found;
        let rest = &xmp[at + name.len()..];
        let before = xmp[..at].chars().next_back();
        if let Some(rest) = rest.trim_start().strip_prefix('=') {
            let rest = rest.trim_start();
            let quote = rest.chars().next()?;
            if quote == '"' || quote == '\'' {
                let end = rest[1..].find(quote)?;
                return Some(&rest[1..1 + end]);
            }
        } else if before == Some('<')
            && let Some(rest) = rest.strip_prefix('>')
        {
            return Some(&rest[..rest.find('<')?]);
        }
        from = at + name.len();
    }
    None
}

/// A decoded JPEG: 8-bit RGB.
struct Rgb8 {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

fn decode_rgb(data: &[u8]) -> Result<Rgb8, Error> {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    let pixels = decoder
        .decode()
        .map_err(|error| Error::new(format!("JPEG: {error:?}")))?;
    let info = decoder
        .info()
        .ok_or_else(|| Error::new("JPEG without a frame"))?;
    let (width, height) = (info.width as usize, info.height as usize);
    // A greyscale JPEG may stay greyscale whatever was asked for.
    let pixels = match pixels.len() / (width * height).max(1) {
        3 => pixels,
        1 => pixels.iter().flat_map(|&v| [v, v, v]).collect(),
        _ => return Err(Error::new("JPEG decoded to an unexpected layout")),
    };
    Ok(Rgb8 {
        width,
        height,
        pixels,
    })
}

/// The HDR rendition of a JPEG with a gain map, as linear RGBA in
/// `primaries`; `Ok(None)` when it has no gain map Bern can apply.
pub(crate) fn decode(
    data: &[u8],
    headroom: Option<f32>,
    primaries: Primaries,
) -> Result<Option<Decoded>, Error> {
    let Some(primary) = Segments::read(data) else {
        return Ok(None);
    };
    if !primary.announces_gain_map() {
        return Ok(None);
    }
    let Some(start) = gain_map_start(data, &primary) else {
        return Ok(None);
    };
    let gain_data = &data[start..];
    let Some(gain) = Segments::read(gain_data) else {
        return Ok(None);
    };
    let metadata = match gain.app2(ISO_URN).next() {
        Some((iso, _)) => Metadata::iso(iso)?,
        None => gain.xmp().and_then(Metadata::xmp),
    };
    let Some(metadata) = metadata else {
        return Ok(None);
    };

    let base_icc = primary
        .icc()
        .and_then(|icc| Icc::parse(&icc))
        .unwrap_or_else(Icc::srgb);
    // The gain applies in the base picture's primaries or the alternate
    // picture's; the result goes to `primaries`.
    let (into_gain_space, out): (Option<Mat3>, Mat3) =
        match gain.icc().and_then(|icc| Icc::parse(&icc)) {
            Some(alternate) if !metadata.base_space => (
                Some(color::mul(
                    &color::invert(&alternate.to_xyz),
                    &base_icc.to_xyz,
                )),
                color::conversion(&alternate.to_xyz, primaries),
            ),
            _ => (None, color::conversion(&base_icc.to_xyz, primaries)),
        };

    let base = decode_rgb(data)?;
    let map = decode_rgb(gain_data)?;
    let weight = metadata.weight(headroom);
    // Each channel's linear base value, by code value.
    let lut: [Vec<f32>; 3] = std::array::from_fn(|c| {
        (0..256)
            .map(|v| base_icc.curves[c].linear(v as f32 / 255.0))
            .collect()
    });
    // Each channel's gain factor, by gain map code value.
    let factors: [Vec<f32>; 3] = std::array::from_fn(|c| {
        (0..256)
            .map(|v| {
                let mut g = v as f32 / 255.0;
                if metadata.gamma[c] != 1.0 {
                    g = g.powf(1.0 / metadata.gamma[c]);
                }
                let log = metadata.min[c] * (1.0 - g) + metadata.max[c] * g;
                (log * weight).exp2()
            })
            .collect()
    });

    let (w, h) = (base.width, base.height);
    let mut pixels = vec![0.0f32; w * h * 4];
    let scale_x = map.width as f32 / w as f32;
    let scale_y = map.height as f32 / h as f32;
    for y in 0..h {
        // Bilinear sampling of the (usually smaller) gain map.
        let gy = ((y as f32 + 0.5) * scale_y - 0.5).clamp(0.0, (map.height - 1) as f32);
        let y0 = gy as usize;
        let y1 = (y0 + 1).min(map.height - 1);
        let ty = gy - y0 as f32;
        for x in 0..w {
            let gx = ((x as f32 + 0.5) * scale_x - 0.5).clamp(0.0, (map.width - 1) as f32);
            let x0 = gx as usize;
            let x1 = (x0 + 1).min(map.width - 1);
            let tx = gx - x0 as f32;
            let sample = |c: usize| {
                let at = |xx: usize, yy: usize| map.pixels[(yy * map.width + xx) * 3 + c] as f32;
                let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
                let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
                top * (1.0 - ty) + bottom * ty
            };
            let i = (y * w + x) * 3;
            let mut rgb = [0, 1, 2].map(|c| lut[c][base.pixels[i + c] as usize]);
            if let Some(m) = &into_gain_space {
                rgb = color::apply(m, rgb);
            }
            let mut hdr = [0.0; 3];
            for c in 0..3 {
                let code = sample(c);
                let low = factors[c][code as usize];
                let high = factors[c][(code as usize + 1).min(255)];
                let factor = low + (high - low) * code.fract();
                hdr[c] = (rgb[c] + metadata.offset_sdr[c]) * factor - metadata.offset_hdr[c];
            }
            let out_rgb = color::apply(&out, hdr);
            let o = (y * w + x) * 4;
            pixels[o..o + 3].copy_from_slice(&out_rgb);
            pixels[o + 3] = 1.0;
        }
    }
    Ok(Some(Decoded {
        width: w,
        height: h,
        pixels,
        orientation: primary.orientation(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmp_values_are_found_as_attributes_and_elements() {
        let xmp = r#"<rdf:Description hdrgm:Version="1.0" hdrgm:GainMapMax = '2.5'>
            <hdrgm:HDRCapacityMax>2.5</hdrgm:HDRCapacityMax></rdf:Description>"#;
        assert_eq!(xmp_value(xmp, "hdrgm:Version"), Some("1.0"));
        assert_eq!(xmp_value(xmp, "hdrgm:GainMapMax"), Some("2.5"));
        assert_eq!(xmp_value(xmp, "hdrgm:HDRCapacityMax"), Some("2.5"));
        let metadata = Metadata::xmp(xmp).unwrap();
        assert_eq!(metadata.min, [0.0; 3]);
        assert_eq!(metadata.offset_sdr, [1.0 / 64.0; 3]);
        assert_eq!(metadata.weight(None), 1.0);
        assert_eq!(metadata.weight(Some(1.0)), 0.0);
        assert!((metadata.weight(Some(2.5f32.exp2().sqrt())) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn iso_metadata_follows_libultrahdr() {
        // Version 0, single channel in the base space, separate fractions:
        // base headroom 0/1, alternate 2/1, min 0/1, max 2/1, gamma 1/1,
        // offsets 1/64.
        let mut data = vec![0, 0, 0, 0, 0x40];
        for (n, d) in [
            (0i32, 1u32),
            (2, 1),
            (0, 1),
            (2, 1),
            (1, 1),
            (1, 64),
            (1, 64),
        ] {
            data.extend_from_slice(&n.to_be_bytes());
            data.extend_from_slice(&d.to_be_bytes());
        }
        let metadata = Metadata::iso(&data).unwrap().unwrap();
        assert!(metadata.base_space);
        assert_eq!(metadata.capacity_max, 2.0);
        assert_eq!(metadata.max, [2.0; 3]);
        assert_eq!(metadata.offset_hdr, [1.0 / 64.0; 3]);
        // An HDR base picture is left alone.
        data[4] |= 0x04;
        assert_eq!(Metadata::iso(&data).unwrap(), None);
    }
}
