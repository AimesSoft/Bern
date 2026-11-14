//! In-process C ABI and shared scene cache for `bern-svg` players.
//!
//! Parsed `Scene`s are cached once per resource key across the process; each
//! live `Svg` view owns a lightweight [`BernSvgPlayer`] with its own timeline,
//! interaction state, and reusable pixel buffer.

use bern_svg::{Config, Fit, PixelFormat, Player, PointerEvent, Scene, StepOutcome};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, LazyLock, Mutex};

pub struct BernSvgPlayer {
    pub player: Player,
    last_ptr: *const u8,
    last_len: usize,
}

static SCENES: LazyLock<Mutex<HashMap<String, Arc<Scene>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Load or reuse a cached [`Scene`] for `key` from precompiled `BSVG` bytes.
pub fn open_scene(key: &str, bytes: &[u8]) -> Result<Arc<Scene>, String> {
    if !key.is_empty()
        && let Ok(guard) = SCENES.lock()
        && let Some(cached) = guard.get(key)
    {
        return Ok(Arc::clone(cached));
    }
    let scene = Arc::new(Scene::from_bytes(bytes)?);
    if !key.is_empty()
        && let Ok(mut guard) = SCENES.lock()
    {
        guard.insert(key.to_owned(), Arc::clone(&scene));
    }
    Ok(scene)
}

/// Check whether a [`Scene`] for `key` is already cached in this process.
///
/// # Safety
/// `key` must point to `key_len` readable bytes, or be null with `key_len == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_cached(key: *const u8, key_len: usize) -> u8 {
    let key = unsafe { str_slice(key, key_len) };
    if key.is_empty() {
        return 0;
    }
    u8::from(SCENES.lock().is_ok_and(|g| g.contains_key(key)))
}

/// Open a new [`BernSvgPlayer`] for resource `key`. If `key` was already cached
/// in this process, `data` may be empty (`null`, `0`). Returns null on invalid
/// input. Free with [`bern_svg_free`].
///
/// # Safety
/// Pointer/length pairs must be valid readable slices (or null with length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_open(
    key: *const u8,
    key_len: usize,
    data: *const u8,
    data_len: usize,
) -> *mut BernSvgPlayer {
    let key = unsafe { str_slice(key, key_len) };
    let data = unsafe { byte_slice(data, data_len) };
    catch_unwind(AssertUnwindSafe(|| {
        let scene = open_scene(key, data).ok()?;
        Some(Box::into_raw(Box::new(BernSvgPlayer {
            player: Player::new(scene),
            last_ptr: std::ptr::null(),
            last_len: 0,
        })))
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// Release a [`BernSvgPlayer`] created by [`bern_svg_open`].
///
/// # Safety
/// `player` must be null or come from [`bern_svg_open`] and not be freed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_free(player: *mut BernSvgPlayer) {
    if !player.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(player));
        }));
    }
}

/// Update bound playback and appearance state. Returns status bits:
/// `bit 0` = redraw needed, `bit 1` = animation active (keep frame clock running).
///
/// `fit`: `0 = contain`, `1 = cover`, `2 = fill`.
/// `text_rgba`: packed `0xRRGGBBAA` foreground color for `currentColor`.
///
/// # Safety
/// `player` must be null or a live pointer from [`bern_svg_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_configure(
    player: *mut BernSvgPlayer,
    playing: u8,
    speed: f64,
    restart: f64,
    fit: u8,
    interactive: u8,
    dark: u8,
    reduced_motion: u8,
    text_rgba: u32,
) -> u8 {
    let Some(p) = (unsafe { player.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let cfg = Config {
            playing: playing != 0,
            speed,
            restart,
            fit: match fit {
                1 => Fit::Cover,
                2 => Fit::Fill,
                _ => Fit::Contain,
            },
            interactive: interactive != 0,
            dark: dark != 0,
            reduced_motion: reduced_motion != 0,
            text_rgba: text_rgba.to_be_bytes(),
        };
        p.player.configure(cfg).to_bits()
    }))
    .unwrap_or(0)
}

/// Deliver a pointer event (`kind`: `0 = click`, `1 = double_click`,
/// `2 = move`, `3 = leave`, `4 = down`, `5 = up`; `x`, `y` normalized in `0..1`).
/// Returns status bits (`bit 0 = changed`, `bit 1 = active`).
///
/// # Safety
/// `player` must be null or a live pointer from [`bern_svg_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_pointer(
    player: *mut BernSvgPlayer,
    kind: u8,
    x: f32,
    y: f32,
) -> u8 {
    let Some(p) = (unsafe { player.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let ev = match kind {
            0 => PointerEvent::Click(x, y),
            1 => PointerEvent::DoubleClick(x, y),
            2 => PointerEvent::Move(x, y),
            3 => PointerEvent::Leave,
            4 => PointerEvent::Down(x, y),
            5 => PointerEvent::Up(x, y),
            _ => return StepOutcome::default().to_bits(),
        };
        p.player.pointer(ev).to_bits()
    }))
    .unwrap_or(0)
}

/// Advance to `now_secs` and render at `width_px × height_px` in `format`
/// (`0 = RgbaPremul`, `1 = BgraPremul`, `2 = RgbaStraight`, `3 = BgraStraight`).
/// Returns status bits (`bit 0 = changed`, `bit 1 = active`). Read the rendered
/// buffer via [`bern_svg_pixels_ptr`] and [`bern_svg_pixels_len`].
///
/// # Safety
/// `player` must be null or a live pointer from [`bern_svg_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_render(
    player: *mut BernSvgPlayer,
    now_secs: f64,
    width_px: u32,
    height_px: u32,
    format: u8,
) -> u8 {
    let Some(p) = (unsafe { player.as_mut() }) else {
        return 0;
    };
    catch_unwind(AssertUnwindSafe(|| {
        let fmt = match format {
            1 => PixelFormat::BgraPremul,
            2 => PixelFormat::RgbaStraight,
            3 => PixelFormat::BgraStraight,
            _ => PixelFormat::RgbaPremul,
        };
        let (outcome, pixels) = p.player.render(now_secs, width_px, height_px, fmt);
        p.last_ptr = pixels.as_ptr();
        p.last_len = pixels.len();
        outcome.to_bits()
    }))
    .unwrap_or(0)
}

/// Pointer to the most recently rendered pixel buffer of `player` (valid until
/// the next `bern_svg_render` or `bern_svg_free` on `player`).
///
/// # Safety
/// `player` must be null or a live pointer from [`bern_svg_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_pixels_ptr(player: *const BernSvgPlayer) -> *const u8 {
    unsafe { player.as_ref() }.map_or(std::ptr::null(), |p| p.last_ptr)
}

/// Byte length of the most recently rendered pixel buffer of `player`.
///
/// # Safety
/// `player` must be null or a live pointer from [`bern_svg_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_svg_pixels_len(player: *const BernSvgPlayer) -> usize {
    unsafe { player.as_ref() }.map_or(0, |p| p.last_len)
}

unsafe fn byte_slice<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}

unsafe fn str_slice<'a>(ptr: *const u8, len: usize) -> &'a str {
    std::str::from_utf8(unsafe { byte_slice(ptr, len) }).unwrap_or("")
}
