//! Application icons. Without `[app].icon` every target uses the Bern logo;
//! with it, the declared square PNG. `[app.icon]` can also give a platform
//! its own icon: another square PNG, or the icon in the platform's own form
//! (an `.icns` file, an `.ico` file, an `.appiconset` folder, an Icon
//! Composer `.icon` folder for macOS and iOS, a tvOS `.brandassets` folder,
//! an Android `res` folder, an icon theme folder, a folder of page icons),
//! which is shipped as it is. The platform
//! containers (`.ico` on Windows, `.icns` on macOS) hold PNG images as they
//! are, so nothing here decodes or scales pixels: the logo ships
//! pre-rendered at every size the platforms ask for, and a declared PNG is
//! used at its own size. Only tvOS's layered icon is drawn from the square
//! one, in [`crate::brand_assets`].

use bern_ir::Application;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Platforms that can have an icon of their own under `[app.icon]`, named
/// as `[targets]` names them.
pub const PLATFORMS: [&str; 7] = [
    "macos", "ios", "tvos", "windows", "linux", "android", "browser",
];

/// Where one platform's icon comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// The Bern logo.
    Logo,
    /// A square PNG, used at its own size.
    Png(PathBuf),
    /// The icon in the platform's own form, shipped as it is; see
    /// [`check_native`].
    Native(PathBuf),
}

/// The icon written for `platform`: its own entry under `[app.icon]`, else
/// the one for every platform. Paths are relative to `Bern.toml`.
pub fn declared<'a>(app: &'a Application, platform: &str) -> Option<&'a str> {
    app.app
        .platform_icons
        .get(platform)
        .or(app.app.icon.as_ref())
        .map(String::as_str)
}

/// Where `platform`'s icon comes from.
pub fn source(app: &Application, platform: &str, project_dir: &Path) -> Source {
    match declared(app, platform) {
        None => Source::Logo,
        Some(name) if is_png_name(name) => Source::Png(project_dir.join(name)),
        Some(name) => Source::Native(project_dir.join(name)),
    }
}

/// The `[app.alternate_icons]` name a `WindowSet(icon:)` variant names:
/// `Skin` is `skin`, `DarkSkin` is `dark_skin`.
pub fn alternate_name(variant: &str) -> String {
    let mut name = String::new();
    for (index, c) in variant.chars().enumerate() {
        if c.is_ascii_uppercase() && index > 0 {
            name.push('_');
        }
        name.push(c.to_ascii_lowercase());
    }
    name
}

/// The icons `WindowSet(icon:)` chooses among, in its enum's order: each
/// variant's name, and the alternate icon it shows (`None`: `[app.icon]`,
/// for `Default`). Empty when the window set does not choose.
pub fn choices(app: &Application) -> Vec<(String, Option<String>)> {
    let Some(bern_protocol::Type::Enum { name }) = app.windows.icon.as_ref().map(|icon| &icon.ty)
    else {
        return Vec::new();
    };
    app.types
        .enum_def(name)
        .map(|def| {
            def.variants
                .iter()
                .map(|variant| {
                    let alternate =
                        (variant.name != "Default").then(|| alternate_name(&variant.name));
                    (variant.name.clone(), alternate)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Where the alternate icon `name` comes from on `platform`: its own entry
/// under `[app.alternate_icons.<name>]`, else its `default`; `None` when it
/// has neither.
pub fn alternate_source(
    app: &Application,
    name: &str,
    platform: &str,
    project_dir: &Path,
) -> Option<Source> {
    let entries = app.app.alternate_icons.get(name)?;
    let declared = entries.get(platform).or_else(|| entries.get("default"))?;
    Some(if is_png_name(declared) {
        Source::Png(project_dir.join(declared))
    } else {
        Source::Native(project_dir.join(declared))
    })
}

/// The alternate icons `WindowSet(icon:)` switches to on `platform`, by
/// name, with where each comes from; the error names one with nothing for
/// the platform.
pub fn alternates(
    app: &Application,
    platform: &str,
    project_dir: &Path,
) -> Result<Vec<(String, Source)>, String> {
    choices(app)
        .into_iter()
        .filter_map(|(_, alternate)| alternate)
        .map(|name| match alternate_source(app, &name, platform, project_dir) {
            Some(source) => Ok((name, source)),
            None => Err(format!(
                "[app.alternate_icons.{name}] has no `{platform}` or `default` entry; the {platform} app cannot switch to it"
            )),
        })
        .collect()
}

/// Whether a declared icon is a PNG by its name.
pub fn is_png_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".png")
}

/// Whether a declared icon is an Icon Composer document, the layered
/// `.icon` folder that macOS 26 and iOS 26 draw in Liquid Glass. `actool`
/// compiles it, and draws flat images from it for earlier systems.
pub fn is_icon_document(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("icon"))
}

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

/// Whether `platform` uses the Bern logo.
pub fn is_default(app: &Application, platform: &str) -> bool {
    declared(app, platform).is_none()
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

/// `platform`'s icon as PNG images, smallest first: the logo in `shape`, or
/// the declared PNG (relative to the project directory) at its own size. A
/// platform whose icon is in its own form has no images here; read it with
/// [`source`].
pub fn images(
    app: &Application,
    platform: &str,
    project_dir: &Path,
    shape: Shape,
) -> Result<Vec<Image>, String> {
    match source(app, platform, project_dir) {
        Source::Logo => Ok(default_images(shape)),
        Source::Png(path) => {
            let png = std::fs::read(&path)
                .map_err(|error| format!("cannot read the icon {}: {error}", path.display()))?;
            let size =
                check(&png).map_err(|message| format!("icon {}: {message}", path.display()))?;
            Ok(vec![Image {
                size,
                png: Cow::Owned(png),
            }])
        }
        Source::Native(path) => Err(format!(
            "the {platform} icon {} is not a PNG",
            path.display()
        )),
    }
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

/// What `platform` takes under `[app.icon]`, for messages.
pub fn accepted(platform: &str) -> &'static str {
    match platform {
        "macos" => "a square PNG, an `.icns` file or an Icon Composer `.icon` folder",
        "ios" => "a square PNG, an `.appiconset` folder or an Icon Composer `.icon` folder",
        "tvos" => "a square PNG or a `.brandassets` folder",
        "windows" => "a square PNG or an `.ico` file",
        "linux" => "a square PNG or an icon theme folder laid out like `hicolor`",
        "android" => "a square PNG or a `res` folder with `mipmap-*/ic_launcher`",
        "browser" => "a square PNG, an SVG or a folder of page icons",
        _ => "a square PNG",
    }
}

/// Check an icon in `platform`'s own form at `path`, a file or a folder:
/// an `.icns` file (macOS), an `.ico` file (Windows), an `.appiconset`
/// folder (iOS), an Icon Composer `.icon` folder (macOS and iOS), a
/// `.brandassets` folder (tvOS), a `res` folder (Android), an icon theme folder (Linux), or
/// an SVG or a folder of page icons (browser).
pub fn check_native(platform: &str, path: &Path) -> Result<(), String> {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let folder = path.is_dir();
    let read = || std::fs::read(path).map_err(|error| format!("cannot be read: {error}"));
    match (platform, folder, extension.as_str()) {
        ("macos", false, "icns") => check_icns(&read()?),
        ("windows", false, "ico") => check_ico(&read()?),
        ("browser", false, "svg") => {
            if String::from_utf8_lossy(&read()?).contains("<svg") {
                Ok(())
            } else {
                Err("is not an SVG image".into())
            }
        }
        ("macos" | "ios", true, "icon") => icon_document(path).map(drop),
        ("ios", true, _) => app_icon_set(path).map(drop),
        ("tvos", true, _) => crate::brand_assets::check(path).map(drop),
        ("android", true, _) => android_res(path).map(drop),
        ("linux", true, _) => theme_icons(path).map(drop),
        ("browser", true, _) => page_icons(path).map(drop),
        _ => Err(format!("is not {}", accepted(platform))),
    }
}

/// Check a macOS icon file: entries that add up to the length it states.
pub fn check_icns(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 8 || &bytes[..4] != b"icns" {
        return Err("is not an .icns file".into());
    }
    let length = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    if length != bytes.len() {
        return Err(format!(
            "says it is {length} bytes long but is {}",
            bytes.len()
        ));
    }
    let mut at = 8;
    let mut entries = 0;
    while at < bytes.len() {
        let entry = bytes
            .get(at + 4..at + 8)
            .map(|size| u32::from_be_bytes([size[0], size[1], size[2], size[3]]) as usize)
            .filter(|size| *size >= 8 && at + size <= bytes.len())
            .ok_or_else(|| format!("has a broken entry at byte {at}"))?;
        at += entry;
        entries += 1;
    }
    if entries == 0 {
        return Err("holds no images".into());
    }
    Ok(())
}

/// Check a Windows icon file: images inside the file.
pub fn check_ico(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 6 || bytes[..4] != [0, 0, 1, 0] {
        return Err("is not an .ico file".into());
    }
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    if count == 0 {
        return Err("holds no images".into());
    }
    for index in 0..count {
        let entry = bytes
            .get(6 + 16 * index..6 + 16 * (index + 1))
            .ok_or_else(|| "is cut short".to_string())?;
        let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
        if size == 0 || offset + size > bytes.len() {
            return Err(format!("has a broken image {}", index + 1));
        }
    }
    Ok(())
}

/// The files below `dir`, relative to it and sorted. A symbolic link is an
/// error: an icon names the files it ships.
pub fn files_in(dir: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(dir: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot read {}: {error}", entry.path().display()))?;
            let name = entry.file_name();
            // Hidden files, and Explorer's own.
            if name.to_string_lossy().starts_with('.')
                || name == "Thumbs.db"
                || name == "desktop.ini"
            {
                continue;
            }
            let inside = relative.join(&name);
            if kind.is_symlink() {
                return Err(format!(
                    "{} is a symbolic link; put the file itself there",
                    entry.path().display()
                ));
            } else if kind.is_dir() {
                walk(&entry.path(), &inside, out)?;
            } else {
                out.push(inside);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, Path::new(""), &mut out)?;
    out.sort();
    Ok(out)
}

/// The `/`-separated form of a relative path, for messages and matching.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The files of an iOS `.appiconset` folder: `Contents.json` and the images
/// it names, all at the top of the folder.
pub fn app_icon_set(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let files = files_in(dir)?;
    if let Some(nested) = files.iter().find(|file| file.components().count() > 1) {
        return Err(format!(
            "has {}; an .appiconset holds its files at its top",
            slashed(nested)
        ));
    }
    let contents = std::fs::read(dir.join("Contents.json"))
        .map_err(|_| "has no Contents.json; an .appiconset names its images there".to_string())?;
    let contents: serde_json::Value = serde_json::from_slice(&contents)
        .map_err(|error| format!("has a Contents.json that is not JSON: {error}"))?;
    let images = contents
        .get("images")
        .and_then(|images| images.as_array())
        .ok_or_else(|| "has a Contents.json without an `images` list".to_string())?;
    let mut named = 0;
    for image in images {
        let Some(name) = image.get("filename").and_then(|name| name.as_str()) else {
            continue;
        };
        if !files.iter().any(|file| file == Path::new(name)) {
            return Err(format!("names {name} in Contents.json, which is not there"));
        }
        named += 1;
    }
    if named == 0 {
        return Err("names no image in Contents.json".into());
    }
    Ok(files)
}

/// The files of an Icon Composer document (an `.icon` folder):
/// `icon.json` and the layer images it names, which sit in `Assets`.
pub fn icon_document(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let files = files_in(dir)?;
    let contents = std::fs::read(dir.join("icon.json"))
        .map_err(|_| "has no icon.json; an .icon folder describes its layers there".to_string())?;
    let document: serde_json::Value = serde_json::from_slice(&contents)
        .map_err(|error| format!("has an icon.json that is not JSON: {error}"))?;
    let groups = document
        .get("groups")
        .and_then(|groups| groups.as_array())
        .ok_or_else(|| "has an icon.json without a `groups` list".to_string())?;
    let mut named = 0;
    let layers = groups
        .iter()
        .filter_map(|group| group.get("layers")?.as_array())
        .flatten();
    for layer in layers {
        // A layer names its image, and may name others per appearance.
        let specialized = layer
            .get("image-name-specializations")
            .and_then(|values| values.as_array())
            .into_iter()
            .flatten()
            .filter_map(|value| value.get("value"));
        for name in layer.get("image-name").into_iter().chain(specialized) {
            let Some(name) = name.as_str() else { continue };
            if !files
                .iter()
                .any(|file| *file == Path::new("Assets").join(name))
            {
                return Err(format!("names {name} in icon.json, which is not in Assets"));
            }
            named += 1;
        }
    }
    if named == 0 {
        return Err("names no layer image in icon.json".into());
    }
    Ok(files)
}

/// The files of an Android `res` folder with a launcher icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AndroidRes {
    /// Relative to the folder, as `mipmap-hdpi/ic_launcher.png`.
    pub files: Vec<PathBuf>,
    /// Whether it has a round icon, `ic_launcher_round`.
    pub round: bool,
}

/// Read an Android `res` folder: resource folders (`mipmap*`, `drawable*`,
/// `values*`) whose files make the launcher icon `@mipmap/ic_launcher`.
/// Files Bern writes itself (`values*/strings.xml`, `values*/themes.xml`)
/// cannot be in it.
pub fn android_res(dir: &Path) -> Result<AndroidRes, String> {
    let files = files_in(dir)?;
    let mut launcher = false;
    let mut round = false;
    for file in &files {
        let parts: Vec<String> = file
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        let [folder, name] = parts.as_slice() else {
            return Err(format!(
                "has {}; a res folder holds resource folders such as mipmap-hdpi",
                slashed(file)
            ));
        };
        let kind = folder.split('-').next().unwrap_or_default();
        if !matches!(kind, "mipmap" | "drawable" | "values") {
            return Err(format!(
                "has {folder}; an icon's res folder holds mipmap, drawable and values folders"
            ));
        }
        if kind == "values" && matches!(name.as_str(), "strings.xml" | "themes.xml") {
            return Err(format!(
                "has {folder}/{name}, which Bern writes itself; keep only the icon's resources"
            ));
        }
        let stem = name
            .rsplit_once('.')
            .map_or(name.as_str(), |(stem, _)| stem);
        if kind == "mipmap" && stem == "ic_launcher" {
            launcher = true;
        }
        if kind == "mipmap" && stem == "ic_launcher_round" {
            round = true;
        }
    }
    if !launcher {
        return Err("has no mipmap-*/ic_launcher; the launcher icon is @mipmap/ic_launcher".into());
    }
    Ok(AndroidRes { files, round })
}

/// One application icon in an icon theme folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeIcon {
    /// Relative to the folder, as `48x48/apps/name.png`.
    pub path: PathBuf,
    /// The folder it goes in, as `48x48/apps`.
    pub folder: PathBuf,
    /// Width and height for a PNG; `None` for SVG.
    pub size: Option<u32>,
    /// Whether it is the symbolic (single-colour) form, `name-symbolic.svg`.
    pub symbolic: bool,
    /// `png` or `svg`.
    pub extension: String,
}

/// Read an icon theme folder laid out like `hicolor`: `<size>/apps/<name>`
/// files, with `<size>` such as `48x48`, `48x48@2`, `scalable` or
/// `symbolic`, all of one name (the symbolic one ends in `-symbolic.svg`).
/// At least one PNG of a size. The name is replaced by the application's.
pub fn theme_icons(dir: &Path) -> Result<Vec<ThemeIcon>, String> {
    let mut icons = Vec::new();
    let mut name: Option<String> = None;
    for file in files_in(dir)? {
        let shown = slashed(&file);
        let parts: Vec<String> = file
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        if parts.len() == 1 && parts[0] == "index.theme" {
            continue;
        }
        let [size, context, file_name] = parts.as_slice() else {
            return Err(format!(
                "has {shown}; an icon theme holds <size>/apps/<name>.png files"
            ));
        };
        if context != "apps" {
            return Err(format!(
                "has {shown}; an application icon goes in an apps folder"
            ));
        }
        let pixels = size
            .split('@')
            .next()
            .and_then(|side| side.split_once('x'))
            .and_then(|(width, height)| {
                let width: u32 = width.parse().ok()?;
                (height.parse::<u32>().ok()? == width).then_some(width)
            });
        let (stem, extension) = file_name
            .rsplit_once('.')
            .map(|(stem, extension)| (stem, extension.to_ascii_lowercase()))
            .unwrap_or((file_name.as_str(), String::new()));
        let symbolic = stem.ends_with("-symbolic");
        match (pixels, size.as_str(), extension.as_str()) {
            (Some(_), _, "png") | (None, "scalable", "svg") | (None, "symbolic", "svg") => {}
            _ => {
                return Err(format!(
                    "has {shown}; sized folders hold PNG files, scalable and symbolic hold SVG"
                ));
            }
        }
        if symbolic != (size == "symbolic") {
            return Err(format!(
                "has {shown}; symbolic/apps holds the one <name>-symbolic.svg"
            ));
        }
        let base = stem.strip_suffix("-symbolic").unwrap_or(stem);
        match &name {
            None => name = Some(base.to_string()),
            Some(name) if name != base => {
                return Err(format!(
                    "has icons named both {name} and {base}; an application has one icon"
                ));
            }
            Some(_) => {}
        }
        let size = match pixels {
            Some(pixels) => {
                let scale: u32 = size
                    .split_once('@')
                    .map_or(Some(1), |(_, scale)| scale.parse().ok())
                    .ok_or_else(|| format!("has {shown}; a scaled folder is named like 48x48@2"))?;
                let bytes = std::fs::read(dir.join(&file))
                    .map_err(|error| format!("cannot read {shown}: {error}"))?;
                match png_size(&bytes) {
                    Some((width, height)) if width == height && width == pixels * scale => {
                        Some(width)
                    }
                    Some((width, height)) => {
                        return Err(format!(
                            "has {shown}, which is {width} × {height} pixels, not {0} × {0}",
                            pixels * scale
                        ));
                    }
                    None => return Err(format!("has {shown}, which is not a PNG")),
                }
            }
            None => None,
        };
        icons.push(ThemeIcon {
            folder: file.parent().map(Path::to_path_buf).unwrap_or_default(),
            path: file,
            size,
            symbolic,
            extension,
        });
    }
    if !icons.iter().any(|icon| icon.size.is_some()) {
        return Err("has no PNG icon of a size, such as 256x256/apps/<name>.png".into());
    }
    Ok(icons)
}

/// Read a folder of page icons: files at its top, of the kinds a page
/// links (`.svg`, `.ico`, `.png`, and a `.webmanifest` or `manifest.json`
/// naming the icons beside it). At least one image.
pub fn page_icons(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let files = files_in(dir)?;
    let mut images = 0;
    for file in &files {
        let shown = slashed(file);
        if file.components().count() > 1 {
            return Err(format!(
                "has {shown}; page icons sit at the top of their folder"
            ));
        }
        // The page links them by name.
        bern_protocol::check_resource_name(&shown)?;
        let extension = file
            .extension()
            .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "svg" | "ico" | "png" => images += 1,
            "webmanifest" => {}
            "json" if shown == "manifest.json" => {}
            _ => {
                return Err(format!(
                    "has {shown}; page icons are .svg, .ico and .png files and a .webmanifest"
                ));
            }
        }
    }
    if images == 0 {
        return Err("has no .svg, .ico or .png icon".into());
    }
    Ok(files)
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
