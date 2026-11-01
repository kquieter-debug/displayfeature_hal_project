#![allow(non_camel_case_types)]

mod color;
mod hdr_controller;
mod state_store;

use color::State;
use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[repr(C)] struct AIBinder(c_void);
#[repr(C)] struct AIBinder_Class(c_void);
#[repr(C)] struct AParcel(c_void);
#[repr(C)] struct AStatus(c_void);
type status_t = i32;
type code_t = u32;

const OK: status_t = 0;
const BAD_VALUE: status_t = -22;
const UNKNOWN: status_t = -74;
const HASH: code_t = 0x00ff_fffe;
const VERSION: code_t = 0x00ff_ffff;
const REGISTER_CALLBACK: code_t = 2;
const SET_FEATURE: code_t = 7;
const SF_SET_COLOR_TRANSFORM: code_t = 1015;
const SF_SET_SATURATION: code_t = 1022;

#[link(name = "binder_ndk")]
#[link(name = "log")]
unsafe extern "C" {
    fn AIBinder_Class_define(d: *const c_char, c: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>, dtor: Option<unsafe extern "C" fn(*mut c_void)>, tx: Option<unsafe extern "C" fn(*mut AIBinder, code_t, *const AParcel, *mut AParcel) -> status_t>) -> *mut AIBinder_Class;
    fn AIBinder_new(c: *const AIBinder_Class, args: *mut c_void) -> *mut AIBinder;
    fn AIBinder_getUserData(b: *mut AIBinder) -> *mut c_void;
    fn AIBinder_incStrong(b: *mut AIBinder);
    fn AIBinder_decStrong(b: *mut AIBinder);
    fn AIBinder_prepareTransaction(b: *mut AIBinder, input: *mut *mut AParcel) -> status_t;
    fn AIBinder_transact(b: *mut AIBinder, code: code_t, input: *mut *mut AParcel, output: *mut *mut AParcel, flags: u32) -> status_t;
    fn AParcel_delete(p: *mut AParcel);
    fn AParcel_readInt32(p: *const AParcel, v: *mut i32) -> status_t;
    fn AParcel_readStrongBinder(p: *const AParcel, b: *mut *mut AIBinder) -> status_t;
    fn AParcel_writeInt32(p: *mut AParcel, v: i32) -> status_t;
    fn AParcel_writeFloat(p: *mut AParcel, v: f32) -> status_t;
    fn AParcel_writeString(p: *mut AParcel, s: *const c_char, n: i32) -> status_t;
    fn AParcel_writeStatusHeader(p: *mut AParcel, status: *const AStatus) -> status_t;
    fn AStatus_newOk() -> *mut AStatus;
    fn AStatus_delete(status: *mut AStatus);
    fn AServiceManager_getService(n: *const c_char) -> *mut AIBinder;
    fn AServiceManager_addService(b: *mut AIBinder, n: *const c_char) -> status_t;
    fn ABinderProcess_setThreadPoolMaxThreadCount(n: u32);
    fn ABinderProcess_startThreadPool();
    fn ABinderProcess_joinThreadPool();
    fn __android_log_print(p: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

static DESC: &[u8] = b"vendor.xiaomi.hardware.displayfeature_aidl.IDisplayFeature\0";
static NAME: &[u8] = b"vendor.xiaomi.hardware.displayfeature_aidl.IDisplayFeature/default\0";
static SF_NAME: &[u8] = b"SurfaceFlinger\0";
static SF_DESC: &[u8] = b"android.ui.ISurfaceComposer\0";
static TAG: &[u8] = b"TB375FCDisplayCompatRust\0";
static HASH_VALUE: &[u8] = b"e57e364757bc7b63a3a7e586585c5827115966d7\0";

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

unsafe fn log(msg: &'static [u8]) { __android_log_print(4, TAG.as_ptr() as _, msg.as_ptr() as _); }
unsafe fn log_status(label: &'static [u8], status: status_t) {
    let fmt = b"%s status=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, label.as_ptr(), status);
}
unsafe fn write_ok(output: *mut AParcel) -> status_t {
    let s = AStatus_newOk();
    let r = AParcel_writeStatusHeader(output, s);
    AStatus_delete(s);
    r
}
unsafe extern "C" fn create(p: *mut c_void) -> *mut c_void { p }
unsafe extern "C" fn destroy(p: *mut c_void) { if !p.is_null() { drop(Box::from_raw(p as *mut State)); } }
unsafe extern "C" fn remote_create(_: *mut c_void) -> *mut c_void { ptr::null_mut() }
unsafe extern "C" fn remote_destroy(_: *mut c_void) {}
unsafe extern "C" fn remote_tx(_: *mut AIBinder, _: code_t, _: *const AParcel, _: *mut AParcel) -> status_t { UNKNOWN }

static RETRYING: AtomicBool = AtomicBool::new(false);

unsafe fn sf_binder() -> *mut AIBinder {
    AServiceManager_getService(SF_NAME.as_ptr() as _)
}

/// Applies the color state to an already-resolved SurfaceFlinger binder.
unsafe fn apply_with(sf: *mut AIBinder, s: State) -> bool {
    let class = AIBinder_Class_define(SF_DESC.as_ptr() as _, Some(remote_create), Some(remote_destroy), Some(remote_tx));
    if class.is_null() { return false; }
    let _ = AIBinder_associateClass(sf, class);
    let (matrix, saturation) = s.render();
    let mut input = ptr::null_mut();
    let mut output = ptr::null_mut();
    let mut status = AIBinder_prepareTransaction(sf, &mut input);
    if status == OK { status = AParcel_writeInt32(input, 1); }
    for v in matrix { if status == OK { status = AParcel_writeFloat(input, v); } }
    if status == OK { status = AIBinder_transact(sf, SF_SET_COLOR_TRANSFORM, &mut input, &mut output, 0); input = ptr::null_mut(); }
    if !output.is_null() { AParcel_delete(output); }
    if !input.is_null() { AParcel_delete(input); }
    log_status(b"SurfaceFlinger matrix\0", status);
    if status == OK {
        input = ptr::null_mut(); output = ptr::null_mut();
        status = AIBinder_prepareTransaction(sf, &mut input);
        if status == OK { status = AParcel_writeFloat(input, saturation); }
        if status == OK { status = AIBinder_transact(sf, SF_SET_SATURATION, &mut input, &mut output, 0); input = ptr::null_mut(); }
        if !output.is_null() { AParcel_delete(output); }
        if !input.is_null() { AParcel_delete(input); }
        log_status(b"SurfaceFlinger saturation\0", status);
    }
    status == OK
}

unsafe fn apply(s: State) -> bool {
    let sf = sf_binder();
    if sf.is_null() { log(b"SurfaceFlinger unavailable\0"); return false; }
    let ok = apply_with(sf, s);
    AIBinder_decStrong(sf);
    ok
}

/// Applies now, or retries in the background until SurfaceFlinger is registered.
/// Catches early setFeature calls that arrive before SurfaceFlinger is up.
fn apply_or_retry(s: State) {
    unsafe { if apply(s) { return; } }
    if RETRYING.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(move || {
        unsafe {
            for _ in 0..300 {
                let sf = sf_binder();
                if !sf.is_null() {
                    let ok = apply_with(sf, s);
                    AIBinder_decStrong(sf);
                    if ok { break; }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        RETRYING.store(false, Ordering::SeqCst);
    });
}

/// Restores the persisted state as soon as SurfaceFlinger accepts transactions,
/// then keeps re-applying once a second while the boot pipeline settles.
/// Skipped if the client already pushed newer values.
fn boot_restore(saved: State) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut applied = false;
    while std::time::Instant::now() < deadline {
        unsafe {
            let sf = sf_binder();
            if !sf.is_null() {
                let ok = apply_with(sf, saved);
                AIBinder_decStrong(sf);
                if ok {
                    applied = true;
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if applied { unsafe { log(b"boot restore applied\0") }; }

    // SurfaceFlinger / the display pipeline can reset the transform while the
    // system boots. Re-apply once a second during the first 45 seconds so the
    // window where the wrong color is visible is at most ~1s. Stop as soon as
    // the client pushes different values.
    let settle = std::time::Instant::now() + Duration::from_secs(45);
    while std::time::Instant::now() < settle {
        std::thread::sleep(Duration::from_millis(1000));
        let still_saved = match STATE.get().and_then(|m| m.lock().ok().map(|g| *g == saved)) {
            Some(v) => v,
            None => true,
        };
        if !still_saved {
            break;
        }
        unsafe {
            let sf = sf_binder();
            if !sf.is_null() {
                let _ = apply_with(sf, saved);
                AIBinder_decStrong(sf);
            }
        }
    }
}

unsafe extern "C" fn transact(b: *mut AIBinder, code: code_t, input: *const AParcel, output: *mut AParcel) -> status_t {
    if code == HASH { let r = write_ok(output); return if r == OK { AParcel_writeString(output, HASH_VALUE.as_ptr() as _, 40) } else { r }; }
    if code == VERSION { let r = write_ok(output); return if r == OK { AParcel_writeInt32(output, 2) } else { r }; }
    if code == REGISTER_CALLBACK {
        let mut id = 0; let mut cb = ptr::null_mut();
        if AParcel_readInt32(input, &mut id) != OK || AParcel_readStrongBinder(input, &mut cb) != OK { return BAD_VALUE; }
        if !cb.is_null() { AIBinder_decStrong(cb); }
        return write_ok(output);
    }
    if code == 1 {
        // oneway notifyBrightness(int): record for HDR diagnostics, reply unused.
        let mut v = 0;
        if AParcel_readInt32(input, &mut v) != OK { return BAD_VALUE; }
        hdr_controller::on_brightness(v);
        return OK;
    }
    if code != SET_FEATURE { return UNKNOWN; }
    let mut a = [0i32; 4];
    for x in &mut a { if AParcel_readInt32(input, x) != OK { return BAD_VALUE; } }
    if a[0] != 0 { return write_ok(output); }
    hdr_controller::on_set_feature(a[1], a[2], a[3]);
    let updated = match STATE.get_or_init(|| Mutex::new(State::default())).lock() {
        Ok(mut state) => {
            if state.update(a[1], a[2]) {
                let snapshot = *state;
                state_store::save(&snapshot);
                Some(snapshot)
            } else {
                None
            }
        }
        Err(_) => None,
    };
    if let Some(snapshot) = updated {
        apply_or_retry(snapshot);
    }
    let fmt = b"setFeature display=%d mode=%d value=%d cookie=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, a[0], a[1], a[2], a[3]);
    write_ok(output)
}

fn main() {
    unsafe {
        ABinderProcess_setThreadPoolMaxThreadCount(2);
        ABinderProcess_startThreadPool();
        let saved = state_store::load();
        let initial = saved.unwrap_or_default();
        let _ = STATE.set(Mutex::new(initial));
        let state = Box::new(initial);
        let class = AIBinder_Class_define(DESC.as_ptr() as _, Some(create), Some(destroy), Some(transact));
        if class.is_null() { log(b"class creation failed\0"); std::process::exit(1); }
        let binder = AIBinder_new(class, Box::into_raw(state) as _);
        if binder.is_null() { log(b"binder creation failed\0"); std::process::exit(1); }
        // Started from post-fs-data now, so servicemanager may still be settling.
        let mut status = -1;
        for _ in 0..50 {
            status = AServiceManager_addService(binder, NAME.as_ptr() as _);
            if status == OK { break; }
            std::thread::sleep(Duration::from_millis(100));
        }
        if status != OK { log_status(b"service registration\0", status); AIBinder_decStrong(binder); std::process::exit(1); }
        log(b"registered\0");
        if let Some(saved_state) = saved {
            std::thread::spawn(move || boot_restore(saved_state));
        }
        hdr_controller::start();
        ABinderProcess_joinThreadPool();
    }
}

#[allow(dead_code)]
unsafe extern "C" { fn AIBinder_associateClass(b: *mut AIBinder, c: *const AIBinder_Class) -> bool; }
