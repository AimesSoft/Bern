//! The trace clock: the platform's monotonic clock in nanoseconds, read
//! directly so frontends can stamp their own events on the same clock —
//! `CLOCK_UPTIME_RAW` on Apple systems (display link and dispatch times),
//! `CLOCK_MONOTONIC` on Linux and Android (`System.nanoTime`, Choreographer,
//! `g_get_monotonic_time`), the performance counter on Windows, and
//! `performance.now()` on a web page (`requestAnimationFrame` times).

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "tvos"))]
pub fn now_ns() -> u64 {
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock: u32) -> u64;
    }
    const CLOCK_UPTIME_RAW: u32 = 8;
    // SAFETY: a libSystem function without preconditions.
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn now_ns() -> u64 {
    use std::os::raw::{c_int, c_long};
    #[repr(C)]
    struct Timespec {
        seconds: c_long,
        nanoseconds: c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: c_int, time: *mut Timespec) -> c_int;
    }
    const CLOCK_MONOTONIC: c_int = 1;
    let mut time = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    // SAFETY: `time` is writable for one `timespec`.
    unsafe { clock_gettime(CLOCK_MONOTONIC, &mut time) };
    time.seconds as u64 * 1_000_000_000 + time.nanoseconds as u64
}

#[cfg(windows)]
pub fn now_ns() -> u64 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn QueryPerformanceCounter(count: *mut i64) -> i32;
        fn QueryPerformanceFrequency(frequency: *mut i64) -> i32;
    }
    static FREQUENCY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut frequency = 0;
        // SAFETY: `frequency` is writable.
        unsafe { QueryPerformanceFrequency(&mut frequency) };
        frequency.max(1) as u64
    });
    let mut count = 0;
    // SAFETY: `count` is writable.
    unsafe { QueryPerformanceCounter(&mut count) };
    (count.max(0) as u128 * 1_000_000_000 / frequency as u128) as u64
}

#[cfg(target_arch = "wasm32")]
pub fn now_ns() -> u64 {
    #[link(wasm_import_module = "bern")]
    unsafe extern "C" {
        /// Milliseconds from the page's monotonic clock.
        fn now_ms() -> f64;
    }
    // SAFETY: the page provides `bern.now_ms` with this signature.
    (unsafe { now_ms() } * 1e6).max(0.0) as u64
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "tvos",
    target_os = "linux",
    target_os = "android",
    windows,
    target_arch = "wasm32"
)))]
pub fn now_ns() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos() as u64
}
