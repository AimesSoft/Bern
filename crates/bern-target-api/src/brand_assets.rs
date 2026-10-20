//! tvOS application icons. A tvOS app shows a layered icon that tilts
//! under the focus, made of image stacks in a `.brandassets` folder of an
//! asset catalog: the home screen icon at 400 × 240 points, the App Store
//! icon at 1280 × 768, and the Top Shelf images shown when the app sits in
//! the top row. A declared `.brandassets` folder ships as it is; from a
//! square PNG (or the Bern logo) this module draws one: the icon on a back
//! layer of its own background color, so the art floats over its ground
//! when the icon tilts.

use std::path::{Path, PathBuf};

/// The name of the brand assets in the asset catalog, which `actool`
/// takes as the application icon.
pub const NAME: &str = "App Icon & Top Shelf Image";

/// An image in 8-bit RGBA, with straight alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Decode a PNG to 8-bit RGBA.
pub fn decode(bytes: &[u8]) -> Result<Raster, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("is not a PNG image: {error}"))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| "is too large".to_string())?;
    let mut buffer = vec![0; size];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("cannot be decoded: {error}"))?;
    buffer.truncate(info.buffer_size());
    let pixels = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        png::ColorType::Grayscale => buffer.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("kept its palette after decoding".into()),
    };
    Ok(Raster {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// Encode RGBA as a PNG.
pub fn encode(raster: &Raster) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, raster.width, raster.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("PNG header");
    writer
        .write_image_data(&raster.pixels)
        .expect("PNG image data");
    writer.finish().expect("PNG end");
    out
}

/// Premultiplied RGBA in 0…1, for resampling without dark fringes.
struct Linear {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

impl Linear {
    fn from(raster: &Raster) -> Linear {
        let pixels = raster
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let a = p[3] as f32 / 255.0;
                [
                    p[0] as f32 / 255.0 * a,
                    p[1] as f32 / 255.0 * a,
                    p[2] as f32 / 255.0 * a,
                    a,
                ]
            })
            .collect();
        Linear {
            width: raster.width as usize,
            height: raster.height as usize,
            pixels,
        }
    }

    /// Resample one axis to `size`: the average of the covered source
    /// pixels when shrinking, linear between pixel centers when growing.
    fn resize_axis(&self, size: usize, horizontal: bool) -> Linear {
        let (from, other) = if horizontal {
            (self.width, self.height)
        } else {
            (self.height, self.width)
        };
        let at = |along: usize, across: usize| {
            if horizontal {
                self.pixels[across * self.width + along]
            } else {
                self.pixels[along * self.width + across]
            }
        };
        let scale = from as f32 / size as f32;
        // Weights of the source pixels for each output pixel.
        let weights: Vec<Vec<(usize, f32)>> = (0..size)
            .map(|i| {
                if scale >= 1.0 {
                    let start = i as f32 * scale;
                    let end = start + scale;
                    let mut list = Vec::new();
                    let mut x = start.floor() as usize;
                    while (x as f32) < end && x < from {
                        let cover = (end.min(x as f32 + 1.0) - start.max(x as f32)).max(0.0);
                        if cover > 0.0 {
                            list.push((x, cover / scale));
                        }
                        x += 1;
                    }
                    list
                } else {
                    let center = (i as f32 + 0.5) * scale - 0.5;
                    let left = center.floor().clamp(0.0, (from - 1) as f32) as usize;
                    let right = (left + 1).min(from - 1);
                    let t = (center - left as f32).clamp(0.0, 1.0);
                    vec![(left, 1.0 - t), (right, t)]
                }
            })
            .collect();
        let (width, height) = if horizontal {
            (size, other)
        } else {
            (other, size)
        };
        let mut pixels = vec![[0.0; 4]; width * height];
        for across in 0..other {
            for (i, list) in weights.iter().enumerate() {
                let mut sum = [0.0f32; 4];
                for &(x, w) in list {
                    let p = at(x, across);
                    for c in 0..4 {
                        sum[c] += p[c] * w;
                    }
                }
                let index = if horizontal {
                    across * width + i
                } else {
                    i * width + across
                };
                pixels[index] = sum;
            }
        }
        Linear {
            width,
            height,
            pixels,
        }
    }

    fn resize(&self, width: usize, height: usize) -> Linear {
        self.resize_axis(width, true).resize_axis(height, false)
    }
}

/// The icon's ground: the color most of its opaque pixels have (to five
/// bits a channel), and whether its edge is opaque, so that the icon is a
/// full square rather than a shape on transparency.
fn ground(raster: &Raster) -> ([u8; 3], bool) {
    let mut buckets = std::collections::HashMap::<u16, (u32, [u64; 3])>::new();
    for &[r, g, b, a] in raster.pixels.as_chunks::<4>().0 {
        if a < 230 {
            continue;
        }
        let key = ((r as u16 >> 3) << 10) | ((g as u16 >> 3) << 5) | (b as u16 >> 3);
        let entry = buckets.entry(key).or_insert((0, [0; 3]));
        entry.0 += 1;
        for (sum, channel) in entry.1.iter_mut().zip([r, g, b]) {
            *sum += channel as u64;
        }
    }
    let color = buckets
        .values()
        .max_by_key(|(count, _)| *count)
        .map(|(count, sum)| sum.map(|c| (c / *count as u64) as u8))
        .unwrap_or([0x2C, 0x2C, 0x2E]);
    let (w, h) = (raster.width as usize, raster.height as usize);
    let ring = (w.min(h) / 50).max(1);
    let (mut opaque, mut total) = (0u64, 0u64);
    for y in 0..h {
        for x in 0..w {
            if x < ring || y < ring || x >= w - ring || y >= h - ring {
                opaque += raster.pixels[(y * w + x) * 4 + 3] as u64;
                total += 255;
            }
        }
    }
    (color, total > 0 && opaque * 100 >= total * 95)
}

/// A layer: the icon `art` scaled to `side` pixels, centered on a
/// transparent `width` × `height` canvas, or the plain `fill`.
fn layer(art: Option<(&Linear, usize)>, fill: Option<[u8; 3]>, width: u32, height: u32) -> Raster {
    let (w, h) = (width as usize, height as usize);
    let mut pixels = vec![0u8; w * h * 4];
    if let Some(color) = fill {
        for p in pixels.as_chunks_mut::<4>().0 {
            *p = [color[0], color[1], color[2], 255];
        }
    }
    if let Some((art, side)) = art {
        let scaled = art.resize(side, side);
        let left = (w as isize - side as isize) / 2;
        let top = (h as isize - side as isize) / 2;
        for y in 0..side {
            for x in 0..side {
                let (tx, ty) = (left + x as isize, top + y as isize);
                if tx < 0 || ty < 0 || tx >= w as isize || ty >= h as isize {
                    continue;
                }
                let s = scaled.pixels[y * side + x];
                let index = (ty as usize * w + tx as usize) * 4;
                let d = &mut pixels[index..index + 4];
                // Source over the canvas, in premultiplied terms.
                let da = d[3] as f32 / 255.0;
                let out_a = s[3] + da * (1.0 - s[3]);
                for c in 0..3 {
                    let dc = d[c] as f32 / 255.0 * da;
                    let oc = s[c] + dc * (1.0 - s[3]);
                    d[c] = if out_a > 0.0 {
                        (oc / out_a * 255.0).round().clamp(0.0, 255.0) as u8
                    } else {
                        0
                    };
                }
                d[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Raster {
        width,
        height,
        pixels,
    }
}

const INFO: &str = "\"info\" : {\n    \"author\" : \"bern\",\n    \"version\" : 1\n  }";

/// An image set of `images` (file name, scale).
fn image_set(images: &[(&str, &str)]) -> String {
    let list: Vec<String> = images
        .iter()
        .map(|(file, scale)| {
            format!(
                "    {{\n      \"filename\" : \"{file}\",\n      \"idiom\" : \"tv\",\n      \"scale\" : \"{scale}\"\n    }}"
            )
        })
        .collect();
    format!(
        "{{\n  \"images\" : [\n{}\n  ],\n  {INFO}\n}}\n",
        list.join(",\n")
    )
}

/// The files of a `.brandassets` folder drawn from a square PNG icon,
/// relative to the folder: two image stacks of a back and a front layer,
/// and the two Top Shelf images.
pub fn generate(icon: &[u8]) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let raster = decode(icon)?;
    if raster.width != raster.height {
        return Err(format!(
            "is {} × {} pixels; an icon is square",
            raster.width, raster.height
        ));
    }
    let (color, full) = ground(&raster);
    let art = Linear::from(&raster);
    // A full square fills the height, its edge meeting the ground; a shape
    // on transparency keeps a margin, as the safe zone of the icon asks.
    let share = if full { 1.0 } else { 0.8 };
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let text = |path: &str, contents: String| (PathBuf::from(path), contents.into_bytes());
    files.push(text(
        "Contents.json",
        format!(
            "{{\n  \"assets\" : [\n    {{\n      \"filename\" : \"App Icon - App Store.imagestack\",\n      \"idiom\" : \"tv\",\n      \"role\" : \"primary-app-icon\",\n      \"size\" : \"1280x768\"\n    }},\n    {{\n      \"filename\" : \"App Icon.imagestack\",\n      \"idiom\" : \"tv\",\n      \"role\" : \"primary-app-icon\",\n      \"size\" : \"400x240\"\n    }},\n    {{\n      \"filename\" : \"Top Shelf Image Wide.imageset\",\n      \"idiom\" : \"tv\",\n      \"role\" : \"top-shelf-image-wide\",\n      \"size\" : \"2320x720\"\n    }},\n    {{\n      \"filename\" : \"Top Shelf Image.imageset\",\n      \"idiom\" : \"tv\",\n      \"role\" : \"top-shelf-image\",\n      \"size\" : \"1920x720\"\n    }}\n  ],\n  {INFO}\n}}\n"
        ),
    ));
    // Image stacks: the home screen icon at 1x and 2x, the App Store one
    // at 1x.
    for (stack, width, height, scales) in [
        ("App Icon.imagestack", 400u32, 240u32, &[1u32, 2][..]),
        ("App Icon - App Store.imagestack", 1280, 768, &[1][..]),
    ] {
        files.push(text(
            &format!("{stack}/Contents.json"),
            format!(
                "{{\n  {INFO},\n  \"layers\" : [\n    {{\n      \"filename\" : \"Front.imagestacklayer\"\n    }},\n    {{\n      \"filename\" : \"Back.imagestacklayer\"\n    }}\n  ]\n}}\n"
            ),
        ));
        for name in ["Front", "Back"] {
            let dir = format!("{stack}/{name}.imagestacklayer");
            files.push(text(
                &format!("{dir}/Contents.json"),
                format!("{{\n  {INFO}\n}}\n"),
            ));
            let mut images = Vec::new();
            for &scale in scales {
                let (w, h) = (width * scale, height * scale);
                let file = if scale == 1 {
                    format!("{}.png", name.to_lowercase())
                } else {
                    format!("{}@{scale}x.png", name.to_lowercase())
                };
                let raster = if name == "Front" {
                    let side = (h as f32 * share).round() as usize;
                    layer(Some((&art, side)), None, w, h)
                } else {
                    layer(None, Some(color), w, h)
                };
                files.push((
                    PathBuf::from(format!("{dir}/Content.imageset/{file}")),
                    encode(&raster),
                ));
                images.push((file, format!("{scale}x")));
            }
            let images: Vec<(&str, &str)> = images
                .iter()
                .map(|(file, scale)| (file.as_str(), scale.as_str()))
                .collect();
            files.push(text(
                &format!("{dir}/Content.imageset/Contents.json"),
                image_set(&images),
            ));
        }
    }
    // Top Shelf images: the icon at half the height on its ground.
    for (set, width, height) in [
        ("Top Shelf Image.imageset", 1920u32, 720u32),
        ("Top Shelf Image Wide.imageset", 2320, 720),
    ] {
        let mut images = Vec::new();
        for scale in [1u32, 2] {
            let (w, h) = (width * scale, height * scale);
            let file = if scale == 1 {
                "top-shelf.png".to_string()
            } else {
                format!("top-shelf@{scale}x.png")
            };
            let side = (h as f32 * 0.5 * share).round() as usize;
            files.push((
                PathBuf::from(format!("{set}/{file}")),
                encode(&layer(Some((&art, side)), Some(color), w, h)),
            ));
            images.push((file, format!("{scale}x")));
        }
        let images: Vec<(&str, &str)> = images
            .iter()
            .map(|(file, scale)| (file.as_str(), scale.as_str()))
            .collect();
        files.push(text(&format!("{set}/Contents.json"), image_set(&images)));
    }
    Ok(files)
}

/// Read a `Contents.json` below the brand assets.
fn contents(dir: &Path, relative: &str) -> Result<serde_json::Value, String> {
    let path = dir.join(relative).join("Contents.json");
    let bytes = std::fs::read(&path).map_err(|_| {
        format!(
            "has no {}/Contents.json",
            relative.trim_end_matches('/').trim_start_matches("./")
        )
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("has a {relative}/Contents.json that is not JSON: {error}"))
}

/// The images an image set names, each checked to be there.
fn check_image_set(dir: &Path, set: &str) -> Result<(), String> {
    let contents = contents(dir, set)?;
    let images = contents
        .get("images")
        .and_then(|images| images.as_array())
        .ok_or_else(|| format!("has a {set}/Contents.json without an `images` list"))?;
    let mut named = 0;
    for image in images {
        if let Some(name) = image.get("filename").and_then(|name| name.as_str()) {
            if !dir.join(set).join(name).is_file() {
                return Err(format!("names {set}/{name}, which is not there"));
            }
            named += 1;
        }
    }
    if named == 0 {
        return Err(format!("has no images in {set}"));
    }
    Ok(())
}

/// Check a declared `.brandassets` folder: its `Contents.json` lists the
/// home screen icon (a `primary-app-icon` image stack of 400 × 240) and
/// names only folders that are there; each image stack has two to five
/// layers with their images, each image set its images. Returns its files,
/// relative to the folder.
pub fn check(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let files = crate::icon::files_in(dir)?;
    let top = contents(dir, ".")?;
    let assets = top
        .get("assets")
        .and_then(|assets| assets.as_array())
        .ok_or_else(|| "has a Contents.json without an `assets` list".to_string())?;
    let mut home = false;
    for asset in assets {
        let Some(name) = asset.get("filename").and_then(|name| name.as_str()) else {
            continue;
        };
        if !dir.join(name).is_dir() {
            return Err(format!("names {name} in Contents.json, which is not there"));
        }
        let role = asset.get("role").and_then(|role| role.as_str());
        let size = asset.get("size").and_then(|size| size.as_str());
        if name.ends_with(".imagestack") {
            let stack = contents(dir, name)?;
            let layers = stack
                .get("layers")
                .and_then(|layers| layers.as_array())
                .ok_or_else(|| format!("has a {name}/Contents.json without `layers`"))?;
            if !(2..=5).contains(&layers.len()) {
                return Err(format!(
                    "has {} layers in {name}; an image stack has two to five",
                    layers.len()
                ));
            }
            for layer in layers {
                let file = layer
                    .get("filename")
                    .and_then(|file| file.as_str())
                    .ok_or_else(|| format!("has a layer without a file name in {name}"))?;
                check_image_set(dir, &format!("{name}/{file}/Content.imageset"))?;
            }
            if role == Some("primary-app-icon") && size == Some("400x240") {
                home = true;
            }
        } else if name.ends_with(".imageset") {
            check_image_set(dir, name)?;
        }
    }
    if !home {
        return Err(
            "lists no home screen icon: a `primary-app-icon` image stack of size 400x240".into(),
        );
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(size: u32, color: [u8; 4], disc: Option<[u8; 4]>) -> Vec<u8> {
        let mut pixels = Vec::new();
        let r = size as f32 / 2.0;
        for y in 0..size {
            for x in 0..size {
                let inside =
                    ((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt() < r * 0.5;
                pixels.extend_from_slice(&match disc {
                    Some(disc) if inside => disc,
                    _ => color,
                });
            }
        }
        encode(&Raster {
            width: size,
            height: size,
            pixels,
        })
    }

    #[test]
    fn a_full_square_floats_over_its_own_ground() {
        let icon = square(256, [10, 120, 200, 255], Some([255, 255, 255, 255]));
        let files = generate(&icon).unwrap();
        let back = files
            .iter()
            .find(|(path, _)| {
                path.ends_with("App Icon.imagestack/Back.imagestacklayer/Content.imageset/back.png")
            })
            .unwrap();
        let back = decode(&back.1).unwrap();
        assert_eq!((back.width, back.height), (400, 240));
        // The ground is the square's own color, to the five bits a channel
        // it is counted in.
        assert_eq!(back.pixels[3], 255);
        assert!(back.pixels[0].abs_diff(10) <= 8 && back.pixels[2].abs_diff(200) <= 8);
        let front = files
            .iter()
            .find(|(path, _)| {
                path.ends_with(
                    "App Icon.imagestack/Front.imagestacklayer/Content.imageset/front@2x.png",
                )
            })
            .unwrap();
        let front = decode(&front.1).unwrap();
        assert_eq!((front.width, front.height), (800, 480));
        // Transparent beside the square, white at its center.
        assert_eq!(front.pixels[3], 0);
        let center = ((240 * 800 + 400) * 4) as usize;
        assert_eq!(&front.pixels[center..center + 4], &[255, 255, 255, 255]);
        let store = files
            .iter()
            .find(|(path, _)| path.ends_with("App Icon - App Store.imagestack/Front.imagestacklayer/Content.imageset/front.png"))
            .unwrap();
        assert_eq!(png_size(&store.1), (1280, 768));
        for (set, size) in [
            ("Top Shelf Image.imageset/top-shelf@2x.png", (3840, 1440)),
            ("Top Shelf Image Wide.imageset/top-shelf.png", (2320, 720)),
        ] {
            let file = files.iter().find(|(path, _)| path.ends_with(set)).unwrap();
            assert_eq!(png_size(&file.1), size);
        }
    }

    fn png_size(bytes: &[u8]) -> (u32, u32) {
        crate::icon::png_size(bytes).unwrap()
    }

    #[test]
    fn a_shape_on_transparency_keeps_a_margin() {
        let icon = square(256, [0, 0, 0, 0], Some([200, 40, 60, 255]));
        let files = generate(&icon).unwrap();
        let back = files
            .iter()
            .find(|(path, _)| {
                path.ends_with("App Icon.imagestack/Back.imagestacklayer/Content.imageset/back.png")
            })
            .unwrap();
        let back = decode(&back.1).unwrap();
        // The disc's color is the ground.
        assert_eq!(&back.pixels[..4], &[200, 40, 60, 255]);
    }

    #[test]
    fn generated_brand_assets_pass_the_check() {
        let dir = std::env::temp_dir().join(format!("bern-brandassets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, contents) in generate(&square(256, [10, 120, 200, 255], None)).unwrap() {
            std::fs::create_dir_all(dir.join(&path).parent().unwrap()).unwrap();
            std::fs::write(dir.join(&path), contents).unwrap();
        }
        let files = check(&dir).unwrap();
        assert!(files.iter().any(|file| file.ends_with("Contents.json")));
        std::fs::remove_file(dir.join("Top Shelf Image.imageset/top-shelf.png")).unwrap();
        assert!(check(&dir).unwrap_err().contains("not there"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
