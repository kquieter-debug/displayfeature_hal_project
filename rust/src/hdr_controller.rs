//! HDR display stack controller.
//!
//! Rust port of the community MiColor `HbmController` (hbm_controller.cpp /
//! hbm_policy.h). HyperOS drives HDR peak brightness inside the framework:
//! `HighBrightnessModeController` computes an HBM brightness once a
//! `DisplayDeviceConfig` carrying `highBrightnessMode` data is present, and
//! `DisplayPowerController` applies it through the normal backlight path.
//!
//! This module never competes with `DisplayPowerController` for backlight
//! ownership in production modes. It observes the authoritative framework HBM
//! state for diagnostics and exposes one explicit bring-up mode:
//!
//!   * mode 0 (monitor): passive observation only, never writes backlight.
//!   * mode 1 (frameworkOwned, default): DisplayPowerController remains the
//!     only backlight writer, preserving the full brightness slider while HDR
//!     is active.
//!   * mode 2 (forcePeak): bring-up/test mode that unconditionally drives the
//!     backlight to the panel maximum. Never enable for normal use.
//!
//! The mode is selected by the sysprop `persist.vendor.micolor.hbm.mode`
//! (0/1/2; unset or invalid defaults to 1) and re-read on every poll, so the
//! mode can be changed at runtime without restarting the service.
//!
//! Framework state source: `android.hardware.display.IDisplayManager`
//! `getBrightnessInfo` (Binder transaction 0x2e). The reply parcel layout is
//! identical to the community implementation: exception code, parcelable
//! presence header, brightness, adjustedBrightness, minimum, maximum,
//! highBrightnessMode, transitionPoint, maxReason, overridden.
//!
//! Xiaomi wire inputs (`notifyBrightness`, HDR-relevant `setFeature` cases)
//! are recorded and logged for protocol completeness and diagnosis, never
//! blindly applied. HDR brightness is not a color-matrix feature.
//!
//! Polling is adaptive: 500 ms while HBM is active or forcePeak is enabled,
//! 1 s otherwise; every sleep is broken into 10 steps so `stop()` stays
//! responsive.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[repr(C)] struct AIBinder(c_void);
#[repr(C)] struct AIBinder_Class(c_void);
#[repr(C)] struct AParcel(c_void);
type status_t = i32;

const OK: status_t = 0;
const UNKNOWN_TRANSACTION: status_t = -74;

static TAG: &[u8] = b"TB375FCHdr\0";
static DISPLAY_SERVICE: &[u8] = b"display\0";
static DISPLAY_MANAGER_DESC: &[u8] = b"android.hardware.display.IDisplayManager\0";
static MODE_PROP: &[u8] = b"persist.vendor.micolor.hbm.mode\0";
// Backlight interface differs per SoC: QCA-style panel0-backlight vs the
// MTK leds interface used by TB375FC. Resolved once at first access.
static BACKLIGHT_BASES: [&[u8]; 2] = [
    b"/sys/class/backlight/panel0-backlight\0",
    b"/sys/class/leds/lcd-backlight\0",
];
static BACKLIGHT_BASE: OnceLock<usize> = OnceLock::new();

// Transaction code for IDisplayManager#getBrightnessInfo. NOTE: this is
// MIUI-version-specific! HyperOS 4 / Android 16 (TB375FC) has
// TRANSACTION_getBrightnessInfo = 0x31 (0x2e is setLimitedFrameRate there;
// on AOSP/community-reference builds it used to be 0x2e). Verify against the
// device's framework.jar IDisplayManager$Stub when porting to another build.
const GET_BRIGHTNESS_INFO: u32 = 0x31;

// BrightnessInfo.HIGH_BRIGHTNESS_MODE_OFF (android.hardware.display.BrightnessInfo).
// Values: 0=OFF, 1=SUNLIGHT, 2=HDR.
const HBM_MODE_OFF: i32 = 0;

const POLL_INTERVAL_MS: u64 = 500;
const IDLE_POLL_INTERVAL_MS: u64 = 1000;

#[link(name = "binder_ndk")]
#[link(name = "log")]
unsafe extern "C" {
    fn AIBinder_Class_define(d: *const c_char, c: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>, dtor: Option<unsafe extern "C" fn(*mut c_void)>, tx: Option<unsafe extern "C" fn(*mut AIBinder, u32, *const AParcel, *mut AParcel) -> status_t>) -> *mut AIBinder_Class;
    fn AIBinder_associateClass(b: *mut AIBinder, c: *const AIBinder_Class) -> bool;
    fn AIBinder_isAlive(b: *mut AIBinder) -> bool;
    fn AIBinder_decStrong(b: *mut AIBinder);
    fn AIBinder_prepareTransaction(b: *mut AIBinder, input: *mut *mut AParcel) -> status_t;
    fn AIBinder_transact(b: *mut AIBinder, code: u32, input: *mut *mut AParcel, output: *mut *mut AParcel, flags: u32) -> status_t;
    fn AParcel_delete(p: *mut AParcel);
    fn AParcel_readInt32(p: *const AParcel, v: *mut i32) -> status_t;
    fn AParcel_readFloat(p: *const AParcel, v: *mut f32) -> status_t;
    fn AParcel_readBool(p: *const AParcel, v: *mut bool) -> status_t;
    fn AParcel_writeInt32(p: *mut AParcel, v: i32) -> status_t;
    fn AServiceManager_checkService(n: *const c_char) -> *mut AIBinder;
    fn __android_log_print(p: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
    fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
}

unsafe fn log(msg: &'static [u8]) {
    __android_log_print(4, TAG.as_ptr() as _, msg.as_ptr() as _);
}

unsafe fn logi(msg: &'static [u8]) {
    __android_log_print(4, TAG.as_ptr() as _, msg.as_ptr() as _);
}

unsafe fn log_status(label: &'static [u8], status: status_t) {
    let fmt = b"%s status=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, label.as_ptr(), status);
}

// __android_log_print is a C variadic; floats promote to f64 like C.
unsafe fn log_hbm_transition(hbm_mode: i32, brightness: f64, backlight: i32, max_backlight: i32) {
    let fmt = b"HBM mode -> %d (brightness %.4f, backlight %d/%d)\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, hbm_mode, brightness, backlight, max_backlight);
}

unsafe fn log_status_line(mode: i32, hbm: i32, bright: f64, fw_ok: i32, bl: i32, max_bl: i32) {
    let fmt = b"state mode=%d hbm=%d bright=%.4f fwOk=%d backlight=%d/%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, mode, hbm, bright, fw_ok, bl, max_bl);
}

unsafe fn log_parse_failure(exception: i32, header: i32, bright: f64, max: f64, hbm: i32) {
    let fmt = b"parse fail exception=%d header=%d bright=%.4f max=%.4f hbm=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, exception, header, bright, max, hbm);
}

/// Backlight ownership policy, mirrors `HbmControlMode` / hbm_policy.h.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Passive observation, never writes backlight.
    Monitor = 0,
    /// Default. DisplayPowerController is the only backlight writer.
    FrameworkOwned = 1,
    /// Bring-up diagnostic: unconditionally drive backlight to panel max.
    ForcePeak = 2,
}

impl Mode {
    fn from_i32(v: i32) -> Mode {
        match v {
            0 => Mode::Monitor,
            2 => Mode::ForcePeak,
            // Unset or invalid property values default to frameworkOwned.
            _ => Mode::FrameworkOwned,
        }
    }
}

/// Production modes never override framework-selected HDR brightness. Only
/// the explicit diagnostic mode may write peak backlight; invalid samples
/// (negative current, non-positive maximum) never trigger a write. Semantics
/// are identical to the C++ `chooseHbmBacklightTarget` static asserts:
/// (kMonitor, 1303, 4095) -> -1, (kFrameworkOwned, 1303, 4095) -> -1,
/// (kForcePeak, 1303, 4095) -> 4095, (kForcePeak, 4095, 4095) -> -1,
/// (kForcePeak, -1, 4095) -> -1, (kForcePeak, 1303, -1) -> -1.
fn choose_hbm_backlight_target(mode: Mode, current: i32, maximum: i32) -> i32 {
    if mode != Mode::ForcePeak || current < 0 || maximum <= 0 || current == maximum {
        return -1;
    }
    maximum
}

/// Diagnostic snapshot, field-for-field mirror of `HbmController::Snapshot`.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub mode: Mode,
    pub framework_connected: bool,
    pub framework_call_failed: bool,
    /// BrightnessInfo.highBrightnessMode: 0=OFF, 1=SUNLIGHT, 2=HDR.
    pub hbm_mode: i32,
    pub brightness: f32,
    pub brightness_maximum: f32,
    pub backlight: i32,
    pub max_backlight: i32,
    pub last_target_backlight: i32,
    pub poll_count: u64,
    pub write_count: u64,
    pub skip_count: u64,
    pub last_notify_brightness: i32,
    pub last_hdr_feature_case: i32,
    pub last_hdr_feature_mode: i32,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            mode: Mode::Monitor,
            framework_connected: false,
            framework_call_failed: false,
            hbm_mode: 0,
            brightness: 0.0,
            brightness_maximum: 1.0,
            backlight: 0,
            max_backlight: -1,
            last_target_backlight: -1,
            poll_count: 0,
            write_count: 0,
            skip_count: 0,
            last_notify_brightness: -1,
            last_hdr_feature_case: -1,
            last_hdr_feature_mode: -1,
        }
    }
}

static SNAPSHOT: OnceLock<Mutex<Snapshot>> = OnceLock::new();
static STOPPING: AtomicBool = AtomicBool::new(false);
static THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

// AIBinder handles are plain pointers shared with libbinder; the binder NDK
// runtime is itself thread-safe. The handle is stored as usize inside a Mutex
// so the static is Sync; 0 means "not bound".
static DISPLAY_MANAGER: Mutex<usize> = Mutex::new(0);
static DISPLAY_MANAGER_CLASS: OnceLock<usize> = OnceLock::new();
static LAST_LOGGED_HBM_MODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

fn snapshot_mut() -> &'static Mutex<Snapshot> {
    SNAPSHOT.get_or_init(|| Mutex::new(Snapshot::default()))
}

fn update_state(f: impl FnOnce(&mut Snapshot)) -> Snapshot {
    match snapshot_mut().lock() {
        Ok(mut s) => {
            f(&mut s);
            *s
        }
        Err(_) => Snapshot::default(),
    }
}

unsafe extern "C" fn remote_create(_: *mut c_void) -> *mut c_void { ptr::null_mut() }
unsafe extern "C" fn remote_destroy(_: *mut c_void) {}
unsafe extern "C" fn remote_tx(_: *mut AIBinder, _: u32, _: *const AParcel, _: *mut AParcel) -> status_t {
    UNKNOWN_TRANSACTION
}

fn display_manager_class() -> *const AIBinder_Class {
    let slot = DISPLAY_MANAGER_CLASS.get_or_init(|| unsafe {
        AIBinder_Class_define(
            DISPLAY_MANAGER_DESC.as_ptr() as _,
            Some(remote_create),
            Some(remote_destroy),
            Some(remote_tx),
        ) as usize
    });
    *slot as *const AIBinder_Class
}

fn bound_display_manager() -> *mut AIBinder {
    if let Ok(slot) = DISPLAY_MANAGER.lock() {
        if *slot != 0 {
            let dm = *slot as *mut AIBinder;
            unsafe {
                if AIBinder_isAlive(dm) {
                    return dm;
                }
            }
        }
    }
    ptr::null_mut()
}

fn bind_display_manager() -> bool {
    if !bound_display_manager().is_null() {
        return true;
    }
    unbind_display_manager();
    // checkService (not getService): display may come up after us; the
    // poll loop retries binding every cycle until the framework is up.
    unsafe {
        let dm = AServiceManager_checkService(DISPLAY_SERVICE.as_ptr() as _);
        if dm.is_null() {
            return false;
        }
        if !AIBinder_associateClass(dm, display_manager_class()) {
            log(b"display service descriptor association failed\0");
            AIBinder_decStrong(dm);
            return false;
        }
        if let Ok(mut slot) = DISPLAY_MANAGER.lock() {
            // Another thread may have bound concurrently; keep both alive.
            if *slot != 0 {
                AIBinder_decStrong(dm);
                return true;
            }
            *slot = dm as usize;
            return true;
        }
        AIBinder_decStrong(dm);
        false
    }
}

fn unbind_display_manager() {
    if let Ok(mut slot) = DISPLAY_MANAGER.lock() {
        let dm = *slot;
        *slot = 0;
        if dm != 0 {
            unsafe { AIBinder_decStrong(dm as *mut AIBinder) };
        }
    }
}

fn resolve_backlight_base() -> Option<&'static [u8]> {
    let idx = *BACKLIGHT_BASE.get_or_init(|| {
        BACKLIGHT_BASES
            .iter()
            .position(|base| {
                let path = match std::str::from_utf8(&base[..base.len() - 1]) {
                    Ok(p) => p,
                    Err(_) => return false,
                };
                std::path::Path::new(&format!("{}/max_brightness", path)).is_file()
            })
            .unwrap_or(usize::MAX)
    });
    BACKLIGHT_BASES.get(idx).copied()
}

fn read_int_file(path: &[u8], fallback: i32) -> i32 {
    let path = match std::str::from_utf8(&path[..path.len() - 1]) {
        Ok(p) => p,
        Err(_) => return fallback,
    };
    match std::fs::read_to_string(path) {
        Ok(s) => match s.trim().parse::<i32>() {
            Ok(v) => v,
            Err(_) => fallback,
        },
        Err(_) => fallback,
    }
}

fn read_mode() -> Mode {
    unsafe {
        let mut value = [0u8; 92]; // PROP_VALUE_MAX
        let n = __system_property_get(MODE_PROP.as_ptr() as _, value.as_mut_ptr() as *mut c_char);
        if n <= 0 {
            return Mode::FrameworkOwned;
        }
        let text = std::str::from_utf8(&value[..n as usize]).unwrap_or("");
        match text.trim().parse::<i32>() {
            Ok(v) => Mode::from_i32(v),
            Err(_) => Mode::FrameworkOwned,
        }
    }
}

/// Polls IDisplayManager#getBrightnessInfo (0x2e) for the authoritative HBM
/// state. AIBinder_prepareTransaction writes the interface token from the
/// descriptor attached via AIBinder_associateClass (AParcel_writeInterfaceToken
/// is a platform-only API not exported by libbinder_ndk), so only the
/// displayId argument is written here. Returns true when the framework state
/// was refreshed.
fn refresh_framework_state() -> bool {
    if !bind_display_manager() {
        update_state(|s| {
            s.framework_call_failed = true;
            if s.poll_count & 0x1f == 0 {
                // Warn at most once per 32 polls: bind phase failure.
                unsafe { log(b"display service bind failed\0") };
            }
        });
        return false;
    }
    unsafe {
        let dm = bound_display_manager();
        if dm.is_null() {
            return false;
        }
        let mut input = ptr::null_mut();
        let mut output = ptr::null_mut();
        let mut status = AIBinder_prepareTransaction(dm, &mut input);
        if status == OK {
            status = AParcel_writeInt32(input, 0); // displayId 0
        }
        if status == OK {
            status = AIBinder_transact(dm, GET_BRIGHTNESS_INFO, &mut input, &mut output, 0);
            input = ptr::null_mut(); // AIBinder_transact takes ownership.
        }
        if !input.is_null() {
            AParcel_delete(input);
        }
        if status != OK {
            unbind_display_manager();
            update_state(|s| {
                s.framework_call_failed = true;
                if s.poll_count & 0x1f == 0 {
                    // Warn at most once per 32 polls.
                    log_status(b"getBrightnessInfo transact failed\0", status);
                }
            });
            return false;
        }

        let mut exception: i32 = 1;
        let mut object_header: i32 = 0;
        let mut brightness = 0.0f32;
        let mut adjusted_brightness = 0.0f32;
        let mut minimum = 0.0f32;
        let mut maximum = 1.0f32;
        let mut hbm_mode: i32 = 0;
        let mut transition_point = 0.0f32;
        let mut max_reason: i32 = 0;
        let mut overridden = false;

        if !output.is_null() {
            if AParcel_readInt32(output, &mut exception) != OK {
                exception = 1;
            }
            if exception == 0 {
                AParcel_readInt32(output, &mut object_header); // presence header
                AParcel_readFloat(output, &mut brightness);
                AParcel_readFloat(output, &mut adjusted_brightness);
                AParcel_readFloat(output, &mut minimum);
                AParcel_readFloat(output, &mut maximum);
                AParcel_readInt32(output, &mut hbm_mode);
                AParcel_readFloat(output, &mut transition_point);
                AParcel_readInt32(output, &mut max_reason);
                AParcel_readBool(output, &mut overridden);
            }
            AParcel_delete(output);
        }
        let _ = (adjusted_brightness, minimum, transition_point, max_reason, overridden);

        if exception != 0 || object_header == 0 {
            unbind_display_manager();
            update_state(|s| {
                s.framework_call_failed = true;
                if s.poll_count & 0x1f == 0 {
                    unsafe {
                        log_parse_failure(
                            exception,
                            object_header,
                            brightness as f64,
                            maximum as f64,
                            hbm_mode,
                        );
                    }
                }
            });
            return false;
        }

        update_state(|s| {
            s.framework_connected = true;
            s.framework_call_failed = false;
            s.hbm_mode = hbm_mode;
            s.brightness = brightness;
            s.brightness_maximum = if maximum > 0.0 { maximum } else { 1.0 };
        });
        true
    }
}

fn refresh_backlight() {
    let base = match resolve_backlight_base() {
        Some(b) => b,
        None => return,
    };
    let mut max_path = base[..base.len() - 1].to_vec();
    max_path.extend_from_slice(b"/max_brightness\0");
    let mut cur_path = base[..base.len() - 1].to_vec();
    cur_path.extend_from_slice(b"/brightness\0");
    let max = read_int_file(&max_path, -1);
    let current = read_int_file(&cur_path, -1);
    update_state(|s| {
        s.max_backlight = max;
        s.backlight = current;
    });
}

fn write_backlight(value: i32) {
    let base = match resolve_backlight_base() {
        Some(b) => b,
        None => {
            unsafe { log(b"no backlight interface found\0") };
            return;
        }
    };
    let mut path = base[..base.len() - 1].to_vec();
    path.extend_from_slice(b"/brightness\0");
    let path = match std::str::from_utf8(&path[..path.len() - 1]) {
        Ok(p) => p,
        Err(_) => return,
    };
    let file = OpenOptions::new().write(true).truncate(true).open(path);
    match file {
        Ok(mut f) => {
            let result = f.write_all(value.to_string().as_bytes()).and_then(|_| f.flush());
            match result {
                Ok(()) => {
                    update_state(|s| {
                        s.last_target_backlight = value;
                        s.write_count += 1;
                    });
                    unsafe {
                        let fmt = b"backlight set to %d\0";
                        __android_log_print(6, TAG.as_ptr() as _, fmt.as_ptr() as _, value);
                    }
                }
                Err(_) => unsafe { log(b"backlight write failed\0") },
            }
        }
        Err(_) => unsafe { log(b"cannot open backlight node\0") },
    }
}

fn loop_body() -> Duration {
    let ok = refresh_framework_state();
    let mode = read_mode();

    // Backlight sampling is only meaningful (and only permitted by policy)
    // for the forcePeak diagnostic mode; production modes must not touch
    // the backlight interface at all.
    if mode == Mode::ForcePeak {
        refresh_backlight();
    }

    let state = update_state(|s| {
        s.mode = mode;
        s.poll_count += 1;
    });

    // One-shot transition log whenever the observed framework HBM state
    // changes, mirroring the C++ lastLoggedMode behavior.
    let last_logged = LAST_LOGGED_HBM_MODE.swap(state.hbm_mode, Ordering::Relaxed);
    if ok && state.hbm_mode != last_logged {
        unsafe {
            log_hbm_transition(
                state.hbm_mode,
                state.brightness as f64,
                state.backlight,
                state.max_backlight,
            );
        }
    }

    // Heartbeat diagnostic: one line per 32 polls (~32 s idle / ~16 s active)
    // so the observed framework state is always attributable in logs.
    if state.poll_count % 32 == 1 {
        unsafe {
            log_status_line(
                mode as i32,
                state.hbm_mode,
                state.brightness as f64,
                if state.framework_connected && !state.framework_call_failed { 1 } else { 0 },
                state.backlight,
                state.max_backlight,
            );
        }
    }

    let target = choose_hbm_backlight_target(mode, state.backlight, state.max_backlight);
    if target >= 0 {
        write_backlight(target);
    } else if mode == Mode::ForcePeak
        && state.backlight == state.max_backlight
        && state.max_backlight > 0
    {
        update_state(|s| s.skip_count += 1);
    }

    // Stay responsive while HBM is active (or peak is explicitly forced),
    // and reduce binder wakeups otherwise.
    let interval_ms = if mode == Mode::ForcePeak || state.hbm_mode != HBM_MODE_OFF {
        POLL_INTERVAL_MS
    } else {
        IDLE_POLL_INTERVAL_MS
    };
    Duration::from_millis(interval_ms / 10)
}

fn run_loop() {
    loop {
        if STOPPING.load(Ordering::Relaxed) {
            return;
        }
        let step = loop_body();
        // 10 sub-steps per poll interval, checking stop between each.
        for _ in 0..10 {
            if STOPPING.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(step);
        }
    }
}

/// Starts the poll thread. Repeated calls are no-ops.
pub fn start() {
    let mut handle = match THREAD.lock() {
        Ok(h) => h,
        Err(_) => return,
    };
    if handle.is_some() {
        return;
    }
    STOPPING.store(false, Ordering::Relaxed);
    *handle = Some(thread::spawn(run_loop));
    unsafe { logi(b"hdr controller started\0") };
}

/// Stops the poll thread and releases the DisplayManager binder.
pub fn stop() {
    STOPPING.store(true, Ordering::Relaxed);
    if let Ok(mut handle) = THREAD.lock() {
        if let Some(h) = handle.take() {
            let _ = h.join();
        }
    }
    unbind_display_manager();
}

/// Records Xiaomi wire tx1 `notifyBrightness(int)` (oneway). Diagnostic only:
/// it is a state notification to vendor display algorithms, not permission to
/// bypass the framework backlight path.
pub fn on_brightness(brightness: i32) {
    update_state(|s| s.last_notify_brightness = brightness);
}

/// Records HDR-relevant setFeature requests (case 7 and case >= 100; color
/// cases are owned by the color state). Recorded and logged, never applied.
pub fn on_set_feature(case_id: i32, mode_id: i32, _cookie: i32) {
    if case_id != 7 && case_id < 100 {
        return;
    }
    update_state(|s| {
        s.last_hdr_feature_case = case_id;
        s.last_hdr_feature_mode = mode_id;
    });
    unsafe {
        let fmt = b"HDR-relevant setFeature case=%d mode=%d\0";
        __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, case_id, mode_id);
    }
}

/// Reads a consistent diagnostic snapshot (e.g. for a Binder dump).
pub fn snapshot() -> Snapshot {
    match snapshot_mut().lock() {
        Ok(s) => *s,
        Err(_) => Snapshot::default(),
    }
}
