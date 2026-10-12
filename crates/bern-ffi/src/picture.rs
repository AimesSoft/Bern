//! HDR pictures for frontends that draw them themselves: the Windows
//! frontends, whose XAML `Image` has no HDR. Decoded by `bern-image`; a
//! panic while decoding counts as a picture that does not decode.

use std::panic::{AssertUnwindSafe, catch_unwind};

/// A decoded picture: RGBA rows, top to bottom, upright, in linear scRGB
/// (BT.709 primaries) with straight alpha and 1.0 at SDR white.
#[repr(C)]
pub struct BernPicture {
    pub width: u32,
    pub height: u32,
    /// The brightest color component, at least 1.
    pub peak: f32,
    pub pixels: *mut f32,
    pub len: usize,
}

/// How a picture carries HDR: 0 not at all (or not a picture), 1 PQ,
/// 2 HLG, 3 a gain map. Reads headers only.
///
/// # Safety
/// `data` must point to `len` readable bytes, or be null with `len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_picture_kind(data: *const u8, len: usize) -> u32 {
    // SAFETY: the caller's contract.
    let data = unsafe { bytes(data, len) };
    catch_unwind(|| match bern_image::kind(data) {
        None => 0,
        Some(bern_image::Kind::Pq) => 1,
        Some(bern_image::Kind::Hlg) => 2,
        Some(bern_image::Kind::GainMap) => 3,
    })
    .unwrap_or(0)
}

/// Decode an HDR picture into `out`. `headroom` is the display's peak over
/// its SDR white, which decides how much of a gain map applies; zero or
/// less applies all of it. False when the picture is not HDR or does not
/// decode; `out` is then untouched. Release a decoded picture with
/// `bern_picture_free`.
///
/// # Safety
/// `data` must point to `len` readable bytes, or be null with `len == 0`;
/// `out` must point to writable memory for one `BernPicture`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_picture_decode(
    data: *const u8,
    len: usize,
    headroom: f32,
    out: *mut BernPicture,
) -> bool {
    // SAFETY: the caller's contract.
    let data = unsafe { bytes(data, len) };
    let headroom = (headroom > 0.0).then_some(headroom);
    let decoded = catch_unwind(AssertUnwindSafe(|| {
        bern_image::decode(data, headroom, bern_image::Primaries::Bt709)
    }));
    let Ok(Ok(Some(picture))) = decoded else {
        return false;
    };
    let pixels = Box::into_raw(picture.pixels.into_boxed_slice());
    // SAFETY: the caller's contract.
    unsafe {
        out.write(BernPicture {
            width: picture.width,
            height: picture.height,
            peak: picture.peak,
            len: pixels.len(),
            pixels: pixels.cast(),
        });
    }
    true
}

/// Release the pixels of a picture from `bern_picture_decode`.
///
/// # Safety
/// `picture` must come from `bern_picture_decode` and not be freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_picture_free(picture: BernPicture) {
    if !picture.pixels.is_null() {
        // SAFETY: the pixels are the boxed slice `bern_picture_decode` made.
        drop(unsafe {
            Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                picture.pixels,
                picture.len,
            ))
        });
    }
}

unsafe fn bytes<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: the caller's contract.
        unsafe { std::slice::from_raw_parts(data, len) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_frees_through_the_c_abi() {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bern-image/tests/fixtures/hdr-pq.png"
        ))
        .unwrap();
        // SAFETY: valid buffers.
        unsafe {
            assert_eq!(bern_picture_kind(data.as_ptr(), data.len()), 1);
            assert_eq!(bern_picture_kind(std::ptr::null(), 0), 0);
            let mut picture = std::mem::MaybeUninit::<BernPicture>::uninit();
            assert!(bern_picture_decode(
                data.as_ptr(),
                data.len(),
                0.0,
                picture.as_mut_ptr()
            ));
            let picture = picture.assume_init();
            assert_eq!(
                (picture.width, picture.height, picture.len),
                (640, 360, 640 * 360 * 4)
            );
            assert!((picture.peak - 4.0).abs() < 0.02);
            bern_picture_free(picture);
        }
    }
}
