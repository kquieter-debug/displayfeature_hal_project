#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{Mutex, OnceLock};

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

#[derive(Clone, Copy)]
struct State { style: i32, temperature: i32, eye: i32 }
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

fn identity() -> [f32; 16] { [1.0,0.0,0.0,0.0, 0.0,1.0,0.0,0.0, 0.0,0.0,1.0,0.0, 0.0,0.0,0.0,1.0] }
fn render(s: State) -> ([f32; 16], f32) {
    let mut m = identity();
    let mut g = 1.0f32;
    let mut b = 1.0f32;
    if s.eye > 0 {
        let f = ((s.eye.clamp(50, 255) - 50) as f32) / 205.0;
        g = 1.0 - (5.0 + f * 25.0) / 255.0;
        b = 1.0 - (20.0 + f * 95.0) / 255.0;
    }
    let (r_style, g_style, b_style) = match s.temperature {
        1 => (1.0, 0.925, 0.84),
        3 => (0.88, 0.95, 1.0),
        _ => (1.0, 1.0, 1.0),
    };
    m[0] = r_style; m[5] = g_style * g; m[10] = b_style * b;
    let sat = match s.style { 1 => 1.2, 2 => 1.3, _ => 1.0 };
    (m, sat)
}

unsafe fn apply(s: State) {
    let sf = AServiceManager_getService(SF_NAME.as_ptr() as _);
    if sf.is_null() { log(b"SurfaceFlinger unavailable\0"); return; }
    let class = AIBinder_Class_define(SF_DESC.as_ptr() as _, Some(remote_create), Some(remote_destroy), Some(remote_tx));
    if class.is_null() { AIBinder_decStrong(sf); return; }
    let _ = AIBinder_associateClass(sf, class);
    let (matrix, saturation) = render(s);
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
    AIBinder_decStrong(sf);
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
    if code != SET_FEATURE { return UNKNOWN; }
    let mut a = [0i32; 4];
    for x in &mut a { if AParcel_readInt32(input, x) != OK { return BAD_VALUE; } }
    if a[0] != 0 { return write_ok(output); }
    if let Ok(mut state) = STATE.get_or_init(|| Mutex::new(State { style: 0, temperature: 2, eye: 0 })).lock() {
        match a[1] {
            0 | 1 | 2 => { state.style = a[1]; if (1..=3).contains(&a[2]) { state.temperature = a[2]; } }
            3 => { state.eye = if a[2] <= 0 { 0 } else { a[2].clamp(50, 255) }; }
            23 => { if a[2] == 1 { state.temperature = 1; } else if a[2] == 2 { state.temperature = 2; } else if a[2] == 3 { state.temperature = 3; } }
            _ => {}
        }
        let snapshot = *state;
        apply(snapshot);
        let fmt = b"setFeature display=%d mode=%d value=%d cookie=%d\0";
        __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, a[0], a[1], a[2], a[3]);
    }
    write_ok(output)
}

fn main() {
    unsafe {
        ABinderProcess_setThreadPoolMaxThreadCount(2);
        ABinderProcess_startThreadPool();
        let state = Box::new(State { style: 0, temperature: 2, eye: 0 });
        let class = AIBinder_Class_define(DESC.as_ptr() as _, Some(create), Some(destroy), Some(transact));
        if class.is_null() { log(b"class creation failed\0"); std::process::exit(1); }
        let binder = AIBinder_new(class, Box::into_raw(state) as _);
        if binder.is_null() { log(b"binder creation failed\0"); std::process::exit(1); }
        let status = AServiceManager_addService(binder, NAME.as_ptr() as _);
        if status != OK { log_status(b"service registration\0", status); AIBinder_decStrong(binder); std::process::exit(1); }
        log(b"registered\0");
        ABinderProcess_joinThreadPool();
    }
}

#[allow(dead_code)]
unsafe extern "C" { fn AIBinder_associateClass(b: *mut AIBinder, c: *const AIBinder_Class) -> bool; }
