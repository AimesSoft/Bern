//! HDR pictures: PQ and HLG PNGs and JPEGs with a gain map, decoded by
//! `bern-image`, since GTK's loaders do not apply gain maps and GTK 4.14
//! knows no color spaces.
//!
//! GTK shows HDR only when it presents in a color-managed way: GTK 4.16 or
//! later on Wayland, with the color management protocol on (by default in
//! 4.16 to 4.18.0, with `GDK_DEBUG=color-mgmt` since 4.18.1). Then the
//! picture goes to GTK in linear BT.2020, whose 1.0 is 203 nits as in
//! `bern-image`. Everywhere else GTK clips what is brighter than SDR white,
//! so the picture is made SDR here first: a gain map gives the SDR picture
//! its author made, PQ and HLG are tone mapped.

use gtk::gdk;
use gtk::glib;
use gtk::glib::translate::{FromGlibPtrFull, IntoGlib, ToGlibPtr};
use gtk::prelude::*;
use std::ffi::{CStr, c_int, c_void};
use std::path::Path;

/// A texture for an HDR picture; `None` for SDR pictures, which GTK's
/// loaders show as they are, and for pictures that fail to decode.
pub(crate) fn texture(path: &Path) -> Option<gdk::Texture> {
    let data = std::fs::read(path).ok()?;
    bern_image::kind(&data)?;
    let result = if presents_hdr() {
        hdr(&data).map(|texture| texture.or_else(|| sdr(&data).ok().flatten()))
    } else {
        sdr(&data)
    };
    result.unwrap_or_else(|error| {
        eprintln!("bern: picture {}: {error}", path.display());
        None
    })
}

/// Whether GTK presents in a color-managed way, so that light above SDR
/// white reaches the screen.
fn presents_hdr() -> bool {
    let version = (
        gtk::major_version(),
        gtk::minor_version(),
        gtk::micro_version(),
    );
    if version < (4, 16, 0) {
        return false;
    }
    let wayland = gdk::Display::default()
        .is_some_and(|display| display.type_().name() == "GdkWaylandDisplay");
    let flag = |variable: &str| {
        std::env::var(variable).is_ok_and(|value| {
            value.split([',', ':', ';', ' ', '\t']).any(|flag| {
                flag.eq_ignore_ascii_case("color-mgmt") || flag.eq_ignore_ascii_case("all")
            })
        })
    };
    let on = if version < (4, 18, 1) {
        !flag("GDK_DISABLE")
    } else {
        flag("GDK_DEBUG")
    };
    wayland && on
}

/// The picture for SDR presentation, as 8-bit sRGB.
fn sdr(data: &[u8]) -> Result<Option<gdk::Texture>, bern_image::Error> {
    let Some(picture) = bern_image::decode(data, Some(1.0), bern_image::Primaries::Bt709)? else {
        return Ok(None);
    };
    let bytes = glib::Bytes::from_owned(picture.to_sdr());
    let texture = gdk::MemoryTexture::new(
        picture.width as i32,
        picture.height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        picture.width as usize * 4,
    );
    Ok(Some(texture.upcast()))
}

/// The picture in linear BT.2020 with all of its gain, through GTK 4.16's
/// `GdkMemoryTextureBuilder`, looked up at run time so that the same
/// program runs on older GTK.
fn hdr(data: &[u8]) -> Result<Option<gdk::Texture>, bern_image::Error> {
    let Some(api) = Builder::find() else {
        return Ok(None);
    };
    let Some(picture) = bern_image::decode(data, None, bern_image::Primaries::Bt2020)? else {
        return Ok(None);
    };
    let bytes: Vec<u8> = picture
        .pixels
        .iter()
        .flat_map(|v| v.to_ne_bytes())
        .collect();
    let bytes = glib::Bytes::from_owned(bytes);
    // SAFETY: the functions are GTK's, with the signatures of its headers;
    // the builder is released after building, the texture is owned.
    unsafe {
        let builder = (api.new)();
        let raw: *const glib::ffi::GBytes = bytes.to_glib_none().0;
        (api.set_bytes)(builder, raw as *mut c_void);
        (api.set_stride)(builder, picture.width as usize * 16);
        (api.set_width)(builder, picture.width as c_int);
        (api.set_height)(builder, picture.height as c_int);
        (api.set_format)(builder, gdk::MemoryFormat::R32g32b32a32Float.into_glib());
        (api.set_color_state)(builder, (api.rec2100_linear)());
        let texture = (api.build)(builder);
        glib::gobject_ffi::g_object_unref(builder as *mut glib::gobject_ffi::GObject);
        if texture.is_null() {
            return Ok(None);
        }
        Ok(Some(gdk::Texture::from_glib_full(
            texture as *mut gdk::ffi::GdkTexture,
        )))
    }
}

/// The GTK 4.16 functions that build a texture with a color state.
struct Builder {
    new: unsafe extern "C" fn() -> *mut c_void,
    set_bytes: unsafe extern "C" fn(*mut c_void, *mut c_void),
    set_stride: unsafe extern "C" fn(*mut c_void, usize),
    set_width: unsafe extern "C" fn(*mut c_void, c_int),
    set_height: unsafe extern "C" fn(*mut c_void, c_int),
    set_format: unsafe extern "C" fn(*mut c_void, c_int),
    set_color_state: unsafe extern "C" fn(*mut c_void, *mut c_void),
    build: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    rec2100_linear: unsafe extern "C" fn() -> *mut c_void,
}

impl Builder {
    fn find() -> Option<Self> {
        /// The function `name` in the loaded libraries, as `F`.
        ///
        /// # Safety
        /// `F` must be the function's signature.
        unsafe fn function<F: Copy>(name: &CStr) -> Option<F> {
            // SAFETY: looking a name up in the loaded libraries.
            let found = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
            if found.is_null() || size_of::<F>() != size_of::<*mut c_void>() {
                return None;
            }
            // SAFETY: the caller's contract.
            Some(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&found) })
        }
        // SAFETY: each field's type is the signature in GTK's headers.
        unsafe {
            Some(Self {
                new: function(c"gdk_memory_texture_builder_new")?,
                set_bytes: function(c"gdk_memory_texture_builder_set_bytes")?,
                set_stride: function(c"gdk_memory_texture_builder_set_stride")?,
                set_width: function(c"gdk_memory_texture_builder_set_width")?,
                set_height: function(c"gdk_memory_texture_builder_set_height")?,
                set_format: function(c"gdk_memory_texture_builder_set_format")?,
                set_color_state: function(c"gdk_memory_texture_builder_set_color_state")?,
                build: function(c"gdk_memory_texture_builder_build")?,
                rec2100_linear: function(c"gdk_color_state_get_rec2100_linear")?,
            })
        }
    }
}
