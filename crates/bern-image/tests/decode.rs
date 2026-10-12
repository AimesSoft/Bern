//! Decoding the fixtures, made with ImageIO by `fixtures/make-fixtures.swift`:
//! 640×360, the top two thirds SDR white on the left and four times as
//! bright on the right, a ramp from 0 to 4 below.

use bern_image::{Kind, Primaries, decode, kind};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// The RGB of the upright pixel at (x, y).
fn at(picture: &bern_image::HdrPicture, x: u32, y: u32) -> [f32; 3] {
    let i = ((y * picture.width + x) * 4) as usize;
    [
        picture.pixels[i],
        picture.pixels[i + 1],
        picture.pixels[i + 2],
    ]
}

fn near(value: [f32; 3], expected: f32, tolerance: f32) -> bool {
    value.iter().all(|v| (v - expected).abs() <= tolerance)
}

#[test]
fn pq_png_decodes_to_reference_white_units() {
    let data = fixture("hdr-pq.png");
    assert_eq!(kind(&data), Some(Kind::Pq));
    let picture = decode(&data, None, Primaries::Bt709).unwrap().unwrap();
    assert_eq!((picture.width, picture.height), (640, 360));
    assert!(
        near(at(&picture, 160, 120), 1.0, 0.01),
        "{:?}",
        at(&picture, 160, 120)
    );
    assert!(
        near(at(&picture, 480, 120), 4.0, 0.02),
        "{:?}",
        at(&picture, 480, 120)
    );
    assert!(
        near(at(&picture, 320, 300), 2.0, 0.02),
        "{:?}",
        at(&picture, 320, 300)
    );
    assert!((picture.peak - 4.0).abs() < 0.02);
    // The same picture in BT.2020 is still neutral and as bright.
    let wide = decode(&data, None, Primaries::Bt2020).unwrap().unwrap();
    assert!(
        near(at(&wide, 480, 120), 4.0, 0.02),
        "{:?}",
        at(&wide, 480, 120)
    );
}

#[test]
fn gain_map_applies_as_much_gain_as_the_display_allows() {
    let data = fixture("hdr-gainmap.jpg");
    assert_eq!(kind(&data), Some(Kind::GainMap));
    // At headroom 1 the picture is its SDR base, which ImageIO reads as code
    // values 166 and 237 (linear 0.381 and 0.847).
    let sdr = decode(&data, Some(1.0), Primaries::Bt709).unwrap().unwrap();
    assert!(
        near(at(&sdr, 160, 120), 0.381, 0.01),
        "{:?}",
        at(&sdr, 160, 120)
    );
    assert!(
        near(at(&sdr, 480, 120), 0.847, 0.01),
        "{:?}",
        at(&sdr, 480, 120)
    );
    // With all of the gain (log2 2, four times at most) SDR white is white
    // again, and the right half is its base four times over.
    let full = decode(&data, None, Primaries::Bt709).unwrap().unwrap();
    assert!(
        near(at(&full, 160, 120), 1.0, 0.02),
        "{:?}",
        at(&full, 160, 120)
    );
    assert!(
        near(at(&full, 480, 120), 4.0 * 0.847, 0.05),
        "{:?}",
        at(&full, 480, 120)
    );
    // Half of the headroom in log2 terms gives half the gain.
    let half = decode(&data, Some(2.0), Primaries::Bt709).unwrap().unwrap();
    let ratio = at(&half, 480, 120)[0] / at(&sdr, 480, 120)[0];
    assert!((ratio - 2.0).abs() < 0.05, "{ratio}");
    assert_eq!(full.kind, Kind::GainMap);
}

#[test]
fn gain_map_metadata_may_be_ultra_hdr_xmp() {
    // Replace the gain map's ISO 21496-1 segment with the equivalent XMP
    // of Ultra HDR v1. The gain map is the last image, so the MPF offsets
    // still hold.
    let data = fixture("hdr-gainmap.jpg");
    let second = data
        .windows(3)
        .skip(2)
        .position(|w| w == [0xFF, 0xD8, 0xFF])
        .unwrap()
        + 2;
    let urn = b"urn:iso:std:iso:ts:21496:-1\0";
    let urn_at = data[second..]
        .windows(urn.len())
        .position(|w| w == urn)
        .unwrap()
        + second;
    let segment = urn_at - 4;
    let length = u16::from_be_bytes([data[segment + 2], data[segment + 3]]) as usize;
    let offset = 53687.0 / 536870912.0;
    let xmp = format!(
        "http://ns.adobe.com/xap/1.0/\0<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF><rdf:Description \
         hdrgm:Version=\"1.0\" hdrgm:GainMapMin=\"0\" hdrgm:GainMapMax=\"2\" hdrgm:Gamma=\"1\" \
         hdrgm:OffsetSDR=\"{offset}\" hdrgm:OffsetHDR=\"{offset}\" hdrgm:HDRCapacityMin=\"0\" \
         hdrgm:HDRCapacityMax=\"2\"/></rdf:RDF></x:xmpmeta>"
    );
    let mut edited = data[..segment].to_vec();
    edited.extend_from_slice(&[0xFF, 0xE1]);
    edited.extend_from_slice(&((xmp.len() + 2) as u16).to_be_bytes());
    edited.extend_from_slice(xmp.as_bytes());
    edited.extend_from_slice(&data[segment + 2 + length..]);
    let iso = decode(&data, None, Primaries::Bt709).unwrap().unwrap();
    let xmp = decode(&edited, None, Primaries::Bt709).unwrap().unwrap();
    for (x, y) in [(160, 120), (480, 120), (320, 300)] {
        let (a, b) = (at(&iso, x, y), at(&xmp, x, y));
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4),
            "{a:?} {b:?}"
        );
    }
}

#[test]
fn sdr_pictures_are_left_to_the_platform() {
    for name in ["sdr.png", "sdr-turned.jpg"] {
        let data = fixture(name);
        assert_eq!(kind(&data), None, "{name}");
        assert!(
            decode(&data, None, Primaries::Bt709).unwrap().is_none(),
            "{name}"
        );
    }
    assert_eq!(kind(b"not a picture"), None);
}

#[test]
fn sdr_rendition_of_hdr_pictures() {
    let picture = decode(&fixture("hdr-pq.png"), None, Primaries::Bt709)
        .unwrap()
        .unwrap();
    let sdr = picture.to_sdr();
    assert_eq!(sdr.len(), (picture.width * picture.height * 4) as usize);
    let code = |x: u32, y: u32| sdr[((y * picture.width + x) * 4) as usize];
    // The highlight becomes white and SDR white stays light but below it.
    assert_eq!(code(480, 120), 255);
    assert!(
        code(160, 120) > 225 && code(160, 120) < 255,
        "{}",
        code(160, 120)
    );
}

#[test]
fn damaged_pictures_fail_without_panicking() {
    for name in ["hdr-gainmap.jpg", "hdr-pq.png"] {
        let data = fixture(name);
        // Cut short anywhere.
        for len in (0..data.len()).step_by(211) {
            let _ = kind(&data[..len]);
            let _ = decode(&data[..len], None, Primaries::Bt709);
        }
        // Bytes changed, deterministically.
        let mut seed = 0x2545_f491_u32;
        for _ in 0..40 {
            let mut damaged = data.clone();
            for _ in 0..8 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let at = seed as usize % damaged.len();
                damaged[at] = (seed >> 24) as u8;
            }
            let _ = decode(&damaged, None, Primaries::Bt709);
        }
    }
}
