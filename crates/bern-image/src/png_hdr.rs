//! PNGs whose cICP chunk names the PQ or HLG transfer function.

use crate::color::{self, Primaries};
use crate::{Decoded, Error};
use std::io::Cursor;

/// The transfer function a PNG's cICP chunk names, read without decoding
/// pixels: 16 is PQ, 18 is HLG.
pub(crate) fn transfer(data: &[u8]) -> Option<u8> {
    if data.get(0..8)? != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut at = 8;
    loop {
        let length = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as usize;
        let kind = data.get(at + 4..at + 8)?;
        if kind == b"cICP" {
            return data.get(at + 9).copied();
        }
        if kind == b"IDAT" || kind == b"IEND" {
            return None;
        }
        at = at.checked_add(12 + length)?;
    }
}

/// The picture as linear RGBA in `primaries`, with its EXIF orientation;
/// `Ok(None)` when it is not PQ or HLG in primaries Bern knows.
pub(crate) fn decode(data: &[u8], primaries: Primaries) -> Result<Option<Decoded>, Error> {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|error| Error::new(format!("PNG: {error}")))?;
    let info = reader.info();
    let Some(cicp) = info.coding_independent_code_points else {
        return Ok(None);
    };
    let pq = match cicp.transfer_function {
        16 => true,
        18 => false,
        _ => return Ok(None),
    };
    let Some(source) = Primaries::from_cicp(cicp.color_primaries) else {
        return Ok(None);
    };
    if cicp.matrix_coefficients != 0 {
        return Ok(None);
    }
    let full_range = cicp.is_video_full_range_image;
    let orientation = info
        .exif_metadata
        .as_deref()
        .and_then(crate::gainmap::exif_orientation)
        .unwrap_or(1);
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| Error::new("PNG is too large"))?;
    let mut buffer = vec![0; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|error| Error::new(format!("PNG: {error}")))?;
    let (w, h) = (frame.width as usize, frame.height as usize);
    let channels = match frame.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err(Error::new("PNG palette was not expanded")),
    };
    let sixteen = frame.bit_depth == png::BitDepth::Sixteen;
    let (max, low, span) = if sixteen {
        (65535.0, 4096.0, 56064.0)
    } else {
        (255.0, 16.0, 219.0)
    };
    let sample = |row: &[u8], i: usize| -> f32 {
        let v = if sixteen {
            u16::from_be_bytes([row[i * 2], row[i * 2 + 1]]) as f32
        } else {
            row[i] as f32
        };
        if full_range {
            v / max
        } else {
            (v - low) / span
        }
    };
    let to_out = color::conversion(&source.to_xyz(), primaries);
    let luma = source.to_xyz()[1];
    let mut pixels = vec![0.0f32; w * h * 4];
    for y in 0..h {
        let row = &buffer[y * frame.line_size..(y + 1) * frame.line_size];
        for x in 0..w {
            let base = x * channels;
            let (rgb, alpha) = match channels {
                1 => ([sample(row, base); 3], 1.0),
                2 => ([sample(row, base); 3], sample(row, base + 1)),
                3 => (
                    [
                        sample(row, base),
                        sample(row, base + 1),
                        sample(row, base + 2),
                    ],
                    1.0,
                ),
                _ => (
                    [
                        sample(row, base),
                        sample(row, base + 1),
                        sample(row, base + 2),
                    ],
                    sample(row, base + 3),
                ),
            };
            let linear = if pq {
                rgb.map(color::pq_to_linear)
            } else {
                color::hlg_to_linear(rgb, luma)
            };
            let out = color::apply(&to_out, linear);
            let o = (y * w + x) * 4;
            pixels[o..o + 3].copy_from_slice(&out);
            pixels[o + 3] = alpha.clamp(0.0, 1.0);
        }
    }
    Ok(Some(Decoded {
        width: w,
        height: h,
        pixels,
        orientation,
    }))
}
