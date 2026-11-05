//! The window a disk image opens to: the app on the left, Applications on
//! the right, a thin arrow between them. Finder keeps such a layout in a
//! `.DS_Store`, which is written here rather than asked of Finder, so
//! packaging needs no window session and comes out the same on a build
//! server.
//!
//! Three formats meet in that file: its buddy allocator and one-node
//! B-tree, the binary property lists of the window and its view, and the
//! alias that leads the view to its background. An alias normally carries
//! the file's catalog number, unknown before the image exists; Finder finds
//! the file by the path it also holds.

/// The background, drawn by `templates/dmg/background.swift`.
pub const BACKGROUND: &[u8] = include_bytes!("../templates/dmg/background.tiff");
/// Where the background goes in the image.
pub const BACKGROUND_PATH: &str = ".background/background.tiff";
/// Where the volume's own icon goes in the image.
pub const VOLUME_ICON: &str = ".VolumeIcon.icns";
/// A folder's Finder information with its custom icon flag set, as
/// `xattr -wx` takes it: the volume then shows [`VOLUME_ICON`].
pub const CUSTOM_ICON: &str = "00 00 00 00 00 00 00 00 04 00 00 00 00 00 00 00 \
                               00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00";

const WINDOW: (u32, u32) = (640, 400);
const WINDOW_ORIGIN: (u32, u32) = (240, 180);
/// The centres of the icons, level with the background's arrow.
const APP_AT: (u32, u32) = (180, 180);
const APPLICATIONS_AT: (u32, u32) = (460, 180);
const ICON_SIZE: f64 = 128.0;

/// Block widths are powers of two, addressed as `offset | width`.
const HEADER_SIZE: usize = 32;
const TABLE_AT: usize = 2048;
const TABLE_WIDTH: u32 = 11;
const NODE_AT: usize = 4096;
const NODE_WIDTH: u32 = 12;
const PAGE_SIZE: u32 = 0x1000;

/// The `.DS_Store` of a volume called `volume` holding the app `app`, the
/// bundle's file name, and a link called Applications.
pub fn ds_store(volume: &str, app: &str) -> Vec<u8> {
    let mut records = vec![
        record(".", b"bwsp", b"blob", &blob(&window_settings())),
        record(".", b"icvp", b"blob", &blob(&view_settings(volume))),
        record(".", b"vSrn", b"long", &1u32.to_be_bytes()),
        record(".", b"vstl", b"type", b"icnv"),
        record("Applications", b"Iloc", b"blob", &location(APPLICATIONS_AT)),
        record(app, b"Iloc", b"blob", &location(APP_AT)),
    ];
    // By name without regard to case, then by code.
    records.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    let mut node = Vec::new();
    node.extend(0u32.to_be_bytes());
    node.extend((records.len() as u32).to_be_bytes());
    for (.., bytes) in &records {
        node.extend(bytes);
    }
    assert!(node.len() <= 1 << NODE_WIDTH, "the layout fits one node");

    // Blocks: 0 is the table below, 1 the tree's header, 2 its one node.
    let blocks = [
        TABLE_AT as u32 | TABLE_WIDTH,
        HEADER_SIZE as u32 | 5,
        NODE_AT as u32 | NODE_WIDTH,
    ];
    let mut table = Vec::new();
    table.extend((blocks.len() as u32).to_be_bytes());
    table.extend(0u32.to_be_bytes());
    for block in blocks {
        table.extend(block.to_be_bytes());
    }
    table.resize(table.len() + (256 - blocks.len()) * 4, 0);
    table.extend(1u32.to_be_bytes());
    table.push(4);
    table.extend(b"DSDB");
    table.extend(1u32.to_be_bytes());
    // The free lists, by width: what the blocks above leave of 2 GB.
    for width in 0..32u32 {
        let free = (6..=10).contains(&width) || (13..=30).contains(&width);
        table.extend(u32::from(free).to_be_bytes());
        if free {
            table.extend((1u32 << width).to_be_bytes());
        }
    }
    assert!(table.len() <= 1 << TABLE_WIDTH, "the table fits its block");

    let mut space = vec![0; NODE_AT + (1 << NODE_WIDTH)];
    let mut header = Vec::new();
    header.extend(b"Bud1");
    for value in [TABLE_AT as u32, table.len() as u32, TABLE_AT as u32] {
        header.extend(value.to_be_bytes());
    }
    header.extend([0, 0, 0x10, 0x0c]);
    space[..header.len()].copy_from_slice(&header);

    let mut tree = Vec::new();
    for value in [2, 0, records.len() as u32, 1, PAGE_SIZE] {
        tree.extend(value.to_be_bytes());
    }
    space[HEADER_SIZE..HEADER_SIZE + tree.len()].copy_from_slice(&tree);
    space[TABLE_AT..TABLE_AT + table.len()].copy_from_slice(&table);
    space[NODE_AT..NODE_AT + node.len()].copy_from_slice(&node);

    let mut file = 1u32.to_be_bytes().to_vec();
    file.extend(space);
    file
}

/// A record and what it sorts by.
fn record(name: &str, code: &[u8; 4], kind: &[u8; 4], data: &[u8]) -> (String, [u8; 4], Vec<u8>) {
    let units: Vec<u16> = name.encode_utf16().collect();
    let mut bytes = (units.len() as u32).to_be_bytes().to_vec();
    for unit in units {
        bytes.extend(unit.to_be_bytes());
    }
    bytes.extend(code);
    bytes.extend(kind);
    bytes.extend(data);
    (name.to_lowercase(), *code, bytes)
}

fn blob(data: &[u8]) -> Vec<u8> {
    let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
    bytes.extend(data);
    bytes
}

/// An icon's place in the window.
fn location((x, y): (u32, u32)) -> Vec<u8> {
    let mut bytes = x.to_be_bytes().to_vec();
    bytes.extend(y.to_be_bytes());
    bytes.extend([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0]);
    blob(&bytes)
}

fn window_settings() -> Vec<u8> {
    let (x, y) = WINDOW_ORIGIN;
    let (width, height) = WINDOW;
    let bounds = format!("{{{{{x}, {y}}}, {{{width}, {height}}}}}");
    plist(&[
        ("ContainerShowSidebar", Value::Bool(false)),
        ("PreviewPaneVisibility", Value::Bool(false)),
        ("ShowPathbar", Value::Bool(false)),
        ("ShowSidebar", Value::Bool(false)),
        ("ShowStatusBar", Value::Bool(false)),
        ("ShowTabView", Value::Bool(false)),
        ("ShowToolbar", Value::Bool(false)),
        ("SidebarWidth", Value::Int(0)),
        ("WindowBounds", Value::Text(&bounds)),
    ])
}

fn view_settings(volume: &str) -> Vec<u8> {
    let background = alias(volume, BACKGROUND_PATH);
    plist(&[
        ("arrangeBy", Value::Text("none")),
        ("backgroundColorBlue", Value::Real(1.0)),
        ("backgroundColorGreen", Value::Real(1.0)),
        ("backgroundColorRed", Value::Real(1.0)),
        ("backgroundImageAlias", Value::Data(&background)),
        ("backgroundType", Value::Int(2)),
        ("gridOffsetX", Value::Real(0.0)),
        ("gridOffsetY", Value::Real(0.0)),
        ("gridSpacing", Value::Real(100.0)),
        ("iconSize", Value::Real(ICON_SIZE)),
        ("labelOnBottom", Value::Bool(true)),
        ("scrollPositionX", Value::Real(0.0)),
        ("scrollPositionY", Value::Real(0.0)),
        ("showIconPreview", Value::Bool(true)),
        ("showItemInfo", Value::Bool(false)),
        ("textSize", Value::Real(13.0)),
        ("viewOptionsVersion", Value::Int(1)),
    ])
}

/// `text` cut to `limit` bytes at a character, as a Pascal string padded to
/// `limit + 1` bytes.
fn pascal(text: &str, limit: usize) -> Vec<u8> {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut bytes = vec![end as u8];
    bytes.extend(&text.as_bytes()[..end]);
    bytes.resize(limit + 1, 0);
    bytes
}

/// A tagged part of an alias, padded to an even length.
fn tag(number: i16, data: &[u8]) -> Vec<u8> {
    let mut bytes = number.to_be_bytes().to_vec();
    bytes.extend((data.len() as u16).to_be_bytes());
    bytes.extend(data);
    if data.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn unicode_tag(number: i16, text: &str) -> Vec<u8> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut data = (units.len() as u16).to_be_bytes().to_vec();
    for unit in units {
        data.extend(unit.to_be_bytes());
    }
    tag(number, &data)
}

/// An alias to `path` on the volume, as Finder's view settings hold one.
fn alias(volume: &str, path: &str) -> Vec<u8> {
    const VOLUME_NAME: usize = 27;
    const FILE_NAME: usize = 63;
    const EJECTABLE: u16 = 5;
    let (folder, file) = path.rsplit_once('/').unwrap_or(("", path));
    let mut body = Vec::new();
    body.extend(0u16.to_be_bytes()); // a file
    body.extend(pascal(volume, VOLUME_NAME));
    body.extend(0u32.to_be_bytes()); // the volume's date
    body.extend(b"H+");
    body.extend(EJECTABLE.to_be_bytes());
    body.extend(0u32.to_be_bytes()); // the folder's catalog number
    body.extend(pascal(file, FILE_NAME));
    body.extend([0; 16]); // the file's catalog number, date, creator and type
    body.extend((-1i16).to_be_bytes()); // levels from and to
    body.extend((-1i16).to_be_bytes());
    body.extend([0; 16]); // volume attributes, file system, reserved
    body.extend(tag(0, folder.as_bytes()));
    body.extend(tag(
        2,
        format!("{volume}:{}", path.replace('/', ":")).as_bytes(),
    ));
    body.extend(unicode_tag(14, file));
    body.extend(unicode_tag(15, volume));
    body.extend(tag(18, format!("/{path}").as_bytes()));
    body.extend(tag(19, format!("/Volumes/{volume}").as_bytes()));
    body.extend((-1i16).to_be_bytes());
    body.extend(0u16.to_be_bytes());

    let mut bytes = vec![0; 4];
    bytes.extend(((body.len() + 8) as u16).to_be_bytes());
    bytes.extend(2u16.to_be_bytes());
    bytes.extend(body);
    bytes
}

enum Value<'a> {
    Bool(bool),
    Int(u64),
    Real(f64),
    Text(&'a str),
    Data(&'a [u8]),
}

/// The marker of an object and its length, which goes in an integer of its
/// own from 15 on.
fn sized(kind: u8, length: usize) -> Vec<u8> {
    if length < 15 {
        return vec![kind | length as u8];
    }
    let mut bytes = vec![kind | 0x0f, 0x11];
    bytes.extend((length as u16).to_be_bytes());
    bytes
}

/// A binary property list of one dictionary, of ASCII keys and texts.
fn plist(entries: &[(&str, Value)]) -> Vec<u8> {
    let count = 1 + entries.len() * 2;
    assert!(count < 256, "object numbers take one byte");
    let mut objects: Vec<Vec<u8>> = Vec::with_capacity(count);
    let mut root = sized(0xd0, entries.len());
    root.extend(1..=entries.len() as u8);
    root.extend((entries.len() as u8 + 1)..=(entries.len() * 2) as u8);
    objects.push(root);
    for (key, _) in entries {
        let mut object = sized(0x50, key.len());
        object.extend(key.as_bytes());
        objects.push(object);
    }
    for (_, value) in entries {
        objects.push(match value {
            Value::Bool(value) => vec![if *value { 0x09 } else { 0x08 }],
            Value::Int(value) => {
                let mut object = vec![0x13];
                object.extend(value.to_be_bytes());
                object
            }
            Value::Real(value) => {
                let mut object = vec![0x23];
                object.extend(value.to_be_bytes());
                object
            }
            Value::Text(text) => {
                let mut object = sized(0x50, text.len());
                object.extend(text.as_bytes());
                object
            }
            Value::Data(data) => {
                let mut object = sized(0x40, data.len());
                object.extend(*data);
                object
            }
        });
    }

    let mut bytes = b"bplist00".to_vec();
    let mut offsets = Vec::with_capacity(count);
    for object in &objects {
        offsets.push(bytes.len() as u16);
        bytes.extend(object);
    }
    let table = bytes.len() as u64;
    for offset in offsets {
        bytes.extend(offset.to_be_bytes());
    }
    bytes.extend([0; 6]);
    bytes.extend([2, 1]); // bytes of an offset, of an object number
    bytes.extend((count as u64).to_be_bytes());
    bytes.extend(0u64.to_be_bytes());
    bytes.extend(table.to_be_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    /// Where a block lies in the file: addresses count from after the
    /// first four bytes.
    fn block(file: &[u8], number: usize) -> (usize, usize) {
        let table = word(file, 8) as usize + 4;
        let address = word(file, table + 8 + number * 4);
        ((address & !31) as usize + 4, 1 << (address & 31))
    }

    #[test]
    fn the_allocator_finds_the_tree() {
        let file = ds_store("Hello", "Hello.app");
        assert_eq!(file.len(), 8196, "the size Finder's own files have");
        assert_eq!(word(&file, 0), 1);
        assert_eq!(&file[4..8], b"Bud1");
        assert_eq!(
            word(&file, 8),
            word(&file, 16),
            "the header names the table twice"
        );

        let table = word(&file, 8) as usize + 4;
        let directory = table + 8 + 256 * 4;
        assert_eq!(word(&file, directory), 1);
        assert_eq!(&file[directory + 4..directory + 9], b"\x04DSDB");
        assert_eq!(word(&file, directory + 9), 1, "DSDB is block 1");

        let (tree, _) = block(&file, 1);
        let root = word(&file, tree) as usize;
        assert_eq!(root, 2);
        assert_eq!(word(&file, tree + 8), 6, "records");
        assert_eq!(word(&file, tree + 16), PAGE_SIZE);
        let (node, size) = block(&file, root);
        assert_eq!(size, 4096);
        assert_eq!(word(&file, node), 0, "a leaf");
        assert_eq!(word(&file, node + 4), 6);
    }

    #[test]
    fn no_two_blocks_share_space_with_the_free_ones() {
        let file = ds_store("Hello", "Hello.app");
        let mut taken = vec![(0, 32)];
        for number in 0..3 {
            let (at, size) = block(&file, number);
            taken.push((at - 4, size));
        }
        let table = word(&file, 8) as usize + 4;
        let mut at = table + 8 + 256 * 4 + 4 + 5 + 4;
        for width in 0..32 {
            let count = word(&file, at) as usize;
            at += 4;
            for _ in 0..count {
                let offset = word(&file, at) as usize;
                at += 4;
                assert_eq!(offset % (1 << width), 0, "aligned to its width");
                taken.push((offset, 1 << width));
            }
        }
        taken.sort();
        let mut end = 0;
        for (offset, size) in taken {
            assert_eq!(offset, end, "the blocks cover the space without a gap");
            end = offset + size;
        }
        assert_eq!(end, 1 << 31);
    }

    #[test]
    fn records_sort_by_name_then_code() {
        let file = ds_store("Hello", "Zebra.app");
        let (node, _) = block(&file, 2);
        let mut at = node + 8;
        let mut seen = Vec::new();
        for _ in 0..word(&file, node + 4) {
            let length = word(&file, at) as usize;
            let name: Vec<u16> = (0..length)
                .map(|i| u16::from_be_bytes([file[at + 4 + i * 2], file[at + 5 + i * 2]]))
                .collect();
            at += 4 + length * 2;
            let code = String::from_utf8_lossy(&file[at..at + 4]).into_owned();
            let size = match &file[at + 4..at + 8] {
                b"blob" => 4 + word(&file, at + 8) as usize,
                _ => 4,
            };
            at += 8 + size;
            seen.push((String::from_utf16(&name).unwrap(), code));
        }
        let expected = [
            (".", "bwsp"),
            (".", "icvp"),
            (".", "vSrn"),
            (".", "vstl"),
            ("Applications", "Iloc"),
            ("Zebra.app", "Iloc"),
        ];
        assert_eq!(
            seen,
            expected.map(|(name, code)| (name.to_string(), code.to_string()))
        );
    }

    #[test]
    fn a_long_name_is_cut_at_a_character() {
        let name = "应用".repeat(10);
        let bytes = pascal(&name, 27);
        assert_eq!(bytes.len(), 28);
        assert_eq!(bytes[0], 27, "nine characters of three bytes");
        assert!(std::str::from_utf8(&bytes[1..28]).is_ok());
        assert_eq!(pascal("Hi", 27)[..4], [2, b'H', b'i', 0]);
    }

    #[test]
    fn an_alias_gives_its_length_and_path() {
        let bytes = alias("你好", BACKGROUND_PATH);
        assert_eq!(
            usize::from(u16::from_be_bytes([bytes[4], bytes[5]])),
            bytes.len()
        );
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("/.background/background.tiff"));
        assert!(text.contains("/Volumes/你好"));
        assert!(text.contains("你好:.background:background.tiff"));
        assert_eq!(bytes[bytes.len() - 4..], [0xff, 0xff, 0, 0]);
    }

    #[test]
    fn the_background_stays_small() {
        assert_eq!(&BACKGROUND[..2], b"MM", "a TIFF");
        assert!(BACKGROUND.len() < 64 * 1024, "{}", BACKGROUND.len());
    }

    /// The system's own reader takes the property lists.
    #[cfg(target_os = "macos")]
    #[test]
    fn plutil_reads_the_settings() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        for (bytes, expected) in [
            (window_settings(), "{{240, 180}, {640, 400}}"),
            (view_settings("Hello"), "backgroundImageAlias"),
        ] {
            let mut plutil = Command::new("plutil")
                .args(["-convert", "xml1", "-o", "-", "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            plutil.stdin.take().unwrap().write_all(&bytes).unwrap();
            let output = plutil.wait_with_output().unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(expected), "{text}");
        }
    }
}
