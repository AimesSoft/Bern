//! Application icons. Without `[app].icon` every target uses the Bern logo;
//! with it, the declared square PNG. The platform containers (`.ico` on
//! Windows, `.icns` on macOS) hold PNG images as they are, so nothing here
//! decodes or scales pixels: the logo ships pre-rendered at every size the
//! platforms ask for, and a declared icon is used at its own size.

use bern_ir::Application;
use std::borrow::Cow;
use std::path::Path;

/// How a platform shows application icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// As drawn, transparent around the shape: Windows, Linux and the web.
    AsDrawn,
    /// A full square that the system cuts to its own shape: macOS shows
    /// application icons as rounded squares and puts any other shape on a
    /// grey plate.
    Masked,
}

/// One PNG image of an icon.
#[derive(Clone, Debug)]
pub struct Image {
    /// Width and height in pixels.
    pub size: u32,
    pub png: Cow<'static, [u8]>,
}

/// The Bern logo, the default icon, as SVG.
pub const DEFAULT_SVG: &str = include_str!("../assets/icon/bern.svg");

/// The logo rendered from `Docs/Assets/logo/bern-logo.svg` (round) and
/// `bern-logo-square.svg` (square) with `rsvg-convert -w N -h N`.
const ROUND: &[(u32, &[u8])] = &[
    (16, include_bytes!("../assets/icon/round-16.png")),
    (20, include_bytes!("../assets/icon/round-20.png")),
    (24, include_bytes!("../assets/icon/round-24.png")),
    (32, include_bytes!("../assets/icon/round-32.png")),
    (40, include_bytes!("../assets/icon/round-40.png")),
    (48, include_bytes!("../assets/icon/round-48.png")),
    (64, include_bytes!("../assets/icon/round-64.png")),
    (128, include_bytes!("../assets/icon/round-128.png")),
    (256, include_bytes!("../assets/icon/round-256.png")),
    (512, include_bytes!("../assets/icon/round-512.png")),
];
const SQUARE: &[(u32, &[u8])] = &[
    (16, include_bytes!("../assets/icon/square-16.png")),
    (32, include_bytes!("../assets/icon/square-32.png")),
    (64, include_bytes!("../assets/icon/square-64.png")),
    (128, include_bytes!("../assets/icon/square-128.png")),
    (256, include_bytes!("../assets/icon/square-256.png")),
    (512, include_bytes!("../assets/icon/square-512.png")),
    (1024, include_bytes!("../assets/icon/square-1024.png")),
];

/// Sizes a declared icon may have: the large sizes every platform scales
/// down from.
pub const DECLARED_SIZES: [u32; 3] = [256, 512, 1024];

/// Whether the application uses the Bern logo.
pub fn is_default(app: &Application) -> bool {
    app.app.icon.is_none()
}

/// The logo in `shape`, smallest first.
pub fn default_images(shape: Shape) -> Vec<Image> {
    let set = match shape {
        Shape::AsDrawn => ROUND,
        Shape::Masked => SQUARE,
    };
    set.iter()
        .map(|(size, png)| Image {
            size: *size,
            png: Cow::Borrowed(*png),
        })
        .collect()
}

/// The application's icon, smallest first: the logo in `shape`, or the
/// declared PNG (relative to the project directory) at its own size.
pub fn images(app: &Application, project_dir: &Path, shape: Shape) -> Result<Vec<Image>, String> {
    let Some(name) = &app.app.icon else {
        return Ok(default_images(shape));
    };
    let path = project_dir.join(name);
    let png = std::fs::read(&path)
        .map_err(|error| format!("cannot read the icon {}: {error}", path.display()))?;
    let size = check(&png).map_err(|message| format!("icon `{name}`: {message}"))?;
    Ok(vec![Image {
        size,
        png: Cow::Owned(png),
    }])
}

/// The largest image.
pub fn largest(images: &[Image]) -> &Image {
    images
        .iter()
        .max_by_key(|image| image.size)
        .expect("an icon has at least one image")
}

/// Width and height of a PNG, from its header.
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let read =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((read(16), read(20)))
}

/// Check a declared icon: a square PNG of 256, 512 or 1024 pixels. Returns
/// its size.
pub fn check(bytes: &[u8]) -> Result<u32, String> {
    let (width, height) = png_size(bytes).ok_or_else(|| "is not a PNG image".to_string())?;
    if width != height {
        return Err(format!("is {width} × {height} pixels; an icon is square"));
    }
    if !DECLARED_SIZES.contains(&width) {
        return Err(format!(
            "is {width} × {width} pixels; use 256, 512 or 1024 (1024 is best)"
        ));
    }
    Ok(width)
}

/// A Windows icon (`.ico`) holding the images as PNG. Images larger than
/// 256 pixels are kept only when there is nothing smaller, as Windows reads
/// icons up to 256 pixels.
pub fn ico(images: &[Image]) -> Vec<u8> {
    let mut chosen: Vec<&Image> = images.iter().filter(|image| image.size <= 256).collect();
    if chosen.is_empty() {
        chosen.push(largest(images));
    }
    let count = chosen.len() as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // icon
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * chosen.len() as u32;
    for image in &chosen {
        // 0 stands for 256 pixels or more.
        let side = if image.size >= 256 {
            0
        } else {
            image.size as u8
        };
        out.push(side);
        out.push(side);
        out.push(0); // no palette
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(image.png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += image.png.len() as u32;
    }
    for image in &chosen {
        out.extend_from_slice(&image.png);
    }
    out
}

/// A macOS icon (`.icns`) holding the images as PNG, each under the entry
/// types for its size (the plain and the Retina slot).
pub fn icns(images: &[Image]) -> Vec<u8> {
    let types = |size: u32| -> &'static [&'static [u8; 4]] {
        match size {
            16 => &[b"icp4"],
            32 => &[b"icp5", b"ic11"],
            64 => &[b"icp6", b"ic12"],
            128 => &[b"ic07"],
            256 => &[b"ic08", b"ic13"],
            512 => &[b"ic09", b"ic14"],
            1024 => &[b"ic10"],
            _ => &[],
        }
    };
    let mut body = Vec::new();
    for image in images {
        for kind in types(image.size) {
            body.extend_from_slice(*kind);
            body.extend_from_slice(&(8 + image.png.len() as u32).to_be_bytes());
            body.extend_from_slice(&image.png);
        }
    }
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(8 + body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_is_rendered_at_every_size() {
        for shape in [Shape::AsDrawn, Shape::Masked] {
            for image in default_images(shape) {
                assert_eq!(png_size(&image.png), Some((image.size, image.size)));
            }
        }
        assert!(DEFAULT_SVG.contains("<svg"));
    }

    #[test]
    fn declared_icons_are_square_pngs_of_a_large_size() {
        let png = |size: u32| {
            let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            bytes.extend_from_slice(&size.to_be_bytes());
            bytes.extend_from_slice(&size.to_be_bytes());
            bytes
        };
        assert_eq!(check(&png(1024)), Ok(1024));
        assert!(check(&png(300)).unwrap_err().contains("1024"));
        assert!(check(b"GIF89a").unwrap_err().contains("PNG"));
        let mut wide = png(512);
        wide[16..20].copy_from_slice(&1024u32.to_be_bytes());
        assert!(check(&wide).unwrap_err().contains("square"));
    }

    #[test]
    fn ico_holds_the_small_images_as_png() {
        let ico = ico(&default_images(Shape::AsDrawn));
        assert_eq!(&ico[..4], &[0, 0, 1, 0]);
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        assert_eq!(count, 9, "16 to 256 pixels");
        // The last entry is 256 pixels, written as 0, and points at a PNG.
        let entry = &ico[6 + 16 * (count - 1)..6 + 16 * count];
        assert_eq!(entry[0], 0);
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        assert_eq!(png_size(&ico[offset..]), Some((256, 256)));
    }

    #[test]
    fn icns_files_every_image_under_its_types() {
        let icns = icns(&default_images(Shape::Masked));
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes([icns[4], icns[5], icns[6], icns[7]]) as usize,
            icns.len()
        );
        let mut kinds = Vec::new();
        let mut at = 8;
        while at < icns.len() {
            let length =
                u32::from_be_bytes([icns[at + 4], icns[at + 5], icns[at + 6], icns[at + 7]])
                    as usize;
            kinds.push(String::from_utf8_lossy(&icns[at..at + 4]).into_owned());
            assert!(png_size(&icns[at + 8..]).is_some());
            at += length;
        }
        assert_eq!(
            kinds,
            [
                "icp4", "icp5", "ic11", "icp6", "ic12", "ic07", "ic08", "ic13", "ic09", "ic14",
                "ic10"
            ]
        );
    }
}
