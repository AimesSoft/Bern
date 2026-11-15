use std::{
    ffi::c_void,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use gpui::FrameRequestSource;
use gpui_util::ResultExt;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Dwm::{DWM_TIMING_INFO, DwmFlush, DwmGetCompositionTimingInfo},
            Gdi::{DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW},
        },
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryA},
            Performance::QueryPerformanceFrequency,
        },
    },
    core::{PCWSTR, s},
};

static QPC_TICKS_PER_SECOND: LazyLock<u64> = LazyLock::new(|| {
    let mut frequency = 0;
    // On systems that run Windows XP or later, the function will always succeed and
    // will thus never return zero.
    unsafe { QueryPerformanceFrequency(&mut frequency).unwrap() };
    frequency as u64
});

/// Threshold below which a compositor/vblank wait is considered an instant
/// non-blocking return (e.g. display asleep or occluded), rather than a genuine
/// sub-millisecond remainder on a 240Hz–480Hz display. Only consecutive
/// instant returns trigger fallback sleeping.
const INSTANT_WAIT_THRESHOLD: Duration = Duration::from_micros(100);
const MIN_VALID_REFRESH_INTERVAL: Duration = Duration::from_micros(500);
const DEFAULT_VSYNC_INTERVAL: Duration = Duration::from_micros(16_666); // ~60Hz
const CONSECUTIVE_SHORT_WAITS_BEFORE_SLEEP: u32 = 4;

type BoostCompositorClockFn = unsafe extern "system" fn(i32) -> i32;
type WaitForCompositorClockFn = unsafe extern "system" fn(u32, *const *mut c_void, u32) -> i32;

struct DCompClockApi {
    boost: Option<BoostCompositorClockFn>,
    wait: Option<WaitForCompositorClockFn>,
}

static DCOMP_CLOCK_API: LazyLock<DCompClockApi> = LazyLock::new(|| unsafe {
    let Ok(module) = LoadLibraryA(s!("dcomp.dll")) else {
        return DCompClockApi {
            boost: None,
            wait: None,
        };
    };
    let boost = GetProcAddress(module, s!("DCompositionBoostCompositorClock"))
        .map(|f| std::mem::transmute::<_, BoostCompositorClockFn>(f));
    let wait = GetProcAddress(module, s!("DCompositionWaitForCompositorClock"))
        .map(|f| std::mem::transmute::<_, WaitForCompositorClockFn>(f));
    DCompClockApi { boost, wait }
});

static COMPOSITOR_BOOST_COUNT: Mutex<u32> = Mutex::new(0);

pub(crate) fn compositor_clock_acquire() {
    let Some(boost) = DCOMP_CLOCK_API.boost else {
        return;
    };
    if let Ok(mut count) = COMPOSITOR_BOOST_COUNT.lock() {
        *count += 1;
        if *count == 1 {
            unsafe {
                let _ = boost(1);
            }
        }
    }
}

pub(crate) fn compositor_clock_release() {
    let Some(boost) = DCOMP_CLOCK_API.boost else {
        return;
    };
    if let Ok(mut count) = COMPOSITOR_BOOST_COUNT.lock()
        && *count > 0
    {
        *count -= 1;
        if *count == 0 {
            unsafe {
                let _ = boost(0);
            }
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateWaitableTimerExW(
        lpTimerAttributes: *const c_void,
        lpTimerName: *const u16,
        dwFlags: u32,
        dwDesiredAccess: u32,
    ) -> *mut c_void;
    fn SetWaitableTimer(
        hTimer: *mut c_void,
        lpDueTime: *const i64,
        lPeriod: i32,
        pfnCompletionRoutine: *const c_void,
        lpArgToCompletionRoutine: *const c_void,
        fResume: i32,
    ) -> i32;
    fn WaitForSingleObject(hHandle: *mut c_void, dwMilliseconds: u32) -> u32;
    fn CloseHandle(hObject: *mut c_void) -> i32;
}

const CREATE_WAITABLE_TIMER_HIGH_RESOLUTION_FLAG: u32 = 0x0000_0002;
const TIMER_ALL_ACCESS_MASK: u32 = 0x001F_0003;

struct HighResTimer(*mut c_void);

unsafe impl Send for HighResTimer {}
unsafe impl Sync for HighResTimer {}

impl HighResTimer {
    fn new() -> Self {
        let handle = unsafe {
            CreateWaitableTimerExW(
                std::ptr::null(),
                std::ptr::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION_FLAG,
                TIMER_ALL_ACCESS_MASK,
            )
        };
        Self(handle)
    }

    fn sleep(&self, duration: Duration) {
        if duration.is_zero() {
            return;
        }
        if !self.0.is_null() {
            let due_100ns = -(duration.as_nanos().min(i64::MAX as u128) as i64 / 100).max(1);
            unsafe {
                if SetWaitableTimer(self.0, &due_100ns, 0, std::ptr::null(), std::ptr::null(), 0)
                    != 0
                {
                    let _ = WaitForSingleObject(self.0, 100);
                    return;
                }
            }
        }
        std::thread::sleep(duration);
    }
}

impl Drop for HighResTimer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub(crate) struct VSyncProvider {
    interval: Option<Duration>,
    short_waits: AtomicU32,
    timer: HighResTimer,
    f: Box<dyn Fn() -> bool>,
}

impl VSyncProvider {
    pub(crate) fn new() -> Self {
        let _ = current_vsync_interval();
        let f: Box<dyn Fn() -> bool> = if let Some(wait) = DCOMP_CLOCK_API.wait {
            Box::new(move || unsafe { wait(0, std::ptr::null(), 80) >= 0 })
        } else {
            Box::new(|| unsafe { DwmFlush().is_ok() })
        };
        Self {
            interval: None,
            short_waits: AtomicU32::new(0),
            timer: HighResTimer::new(),
            f,
        }
    }

    pub(crate) fn wait_for_vsync(&self) -> FrameRequestSource {
        let vsync_start = Instant::now();
        let wait_succeeded = (self.f)();
        let elapsed = vsync_start.elapsed();
        if !wait_succeeded {
            let interval = self.interval.unwrap_or_else(current_vsync_interval);
            self.timer.sleep(interval);
            return FrameRequestSource::LocalSchedule;
        }
        if elapsed < INSTANT_WAIT_THRESHOLD {
            let consecutive = self.short_waits.fetch_add(1, Ordering::Relaxed) + 1;
            if consecutive >= CONSECUTIVE_SHORT_WAITS_BEFORE_SLEEP {
                log::trace!(
                    "VSyncProvider::wait_for_vsync() returned under threshold repeatedly; pacing locally"
                );
                let interval = self.interval.unwrap_or_else(current_vsync_interval);
                self.timer.sleep(interval);
                return FrameRequestSource::LocalSchedule;
            }
        } else {
            self.short_waits.store(0, Ordering::Relaxed);
        }
        FrameRequestSource::NativeCallback
    }
}

pub(crate) fn current_vsync_interval() -> Duration {
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    if unsafe { EnumDisplaySettingsW(PCWSTR::null(), ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool()
        && mode.dmDisplayFrequency > 1
    {
        return Duration::from_nanos(1_000_000_000 / u64::from(mode.dmDisplayFrequency));
    }
    get_dwm_interval()
        .context("Failed to get DWM interval")
        .log_err()
        .unwrap_or(DEFAULT_VSYNC_INTERVAL)
}

fn get_dwm_interval() -> Result<Duration> {
    let mut timing_info = DWM_TIMING_INFO {
        cbSize: std::mem::size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing_info) }?;
    let interval = retrieve_duration(timing_info.qpcRefreshPeriod, *QPC_TICKS_PER_SECOND);
    // Check for interval values that are impossibly low. A 29 microsecond
    // interval was seen (from a qpcRefreshPeriod of 60).
    if interval < MIN_VALID_REFRESH_INTERVAL {
        Ok(retrieve_duration(
            timing_info.rateRefresh.uiDenominator as u64,
            timing_info.rateRefresh.uiNumerator as u64,
        ))
    } else {
        Ok(interval)
    }
}

#[inline]
fn retrieve_duration(counts: u64, ticks_per_second: u64) -> Duration {
    let ticks_per_microsecond = (ticks_per_second / 1_000_000).max(1);
    Duration::from_micros(counts / ticks_per_microsecond)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_compositor_wait_reports_native_callback() {
        let provider = VSyncProvider {
            interval: Some(Duration::ZERO),
            short_waits: AtomicU32::new(0),
            timer: HighResTimer(std::ptr::null_mut()),
            f: Box::new(|| {
                std::thread::sleep(Duration::from_millis(2));
                true
            }),
        };

        assert_eq!(
            provider.wait_for_vsync(),
            FrameRequestSource::NativeCallback
        );
    }

    #[test]
    fn sub_millisecond_high_refresh_vblank_does_not_fall_back_to_sleep() {
        let provider = VSyncProvider {
            interval: Some(Duration::ZERO),
            short_waits: AtomicU32::new(0),
            timer: HighResTimer(std::ptr::null_mut()),
            f: Box::new(|| {
                std::thread::sleep(Duration::from_micros(350));
                true
            }),
        };

        for _ in 0..8 {
            assert_eq!(
                provider.wait_for_vsync(),
                FrameRequestSource::NativeCallback
            );
        }
    }

    #[test]
    fn failed_compositor_wait_reports_local_schedule_even_after_threshold() {
        let provider = VSyncProvider {
            interval: Some(Duration::ZERO),
            short_waits: AtomicU32::new(0),
            timer: HighResTimer(std::ptr::null_mut()),
            f: Box::new(|| {
                std::thread::sleep(Duration::from_millis(2));
                false
            }),
        };

        assert_eq!(provider.wait_for_vsync(), FrameRequestSource::LocalSchedule);
    }
}
