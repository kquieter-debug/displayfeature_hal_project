#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::{Mutex, OnceLock};

#[repr(C)] struct AIBinder(c_void);
#[repr(C)] struct AIBinder_Class(c_void);
#[repr(C)] struct AParcel(c_void);
type status_t = i32;
type code_t = u32;

const OK: status_t = 0;
const BAD_VALUE: status_t = -22;
const UNKNOWN: status_t = -74;
const HASH: code_t = 0x00ff_fffe;
const VERSION: code_t = 0x00ff_ffff;
const REGISTER_CALLBACK: code_t = 2;
const SET_FEATURE: code_t = 7;
const PQ_SET_BLUE_LIGHT_STRENGTH: code_t = 23;
const PQ_ENABLE_BLUE_LIGHT: code_t = 67;
const PQ_SET_MODE: code_t = 88;
const PQ_SET_RGB_GAIN: code_t = 62;
const PQ_SET_COLOR_MATRIX: code_t = 51;
const PQ_SET_CCORR_INFO: code_t = 50;
const PQ_SET_GAMMA: code_t = 57;
const PQ_SET_COLOR_TRANSFORM: code_t = 54;
const EXPERT_MODE: i32 = 26;
const MATRIX_TEST_MODE: i32 = 90;
const RGB_BASELINE_MODE: i32 = 91;
const CCORR_TEST_MODE: i32 = 92;

#[link(name = "binder_ndk")]
#[link(name = "log")]
unsafe extern "C" {
    fn AIBinder_Class_define(d: *const c_char, c: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>, dtor: Option<unsafe extern "C" fn(*mut c_void)>, tx: Option<unsafe extern "C" fn(*mut AIBinder, code_t, *const AParcel, *mut AParcel) -> status_t>) -> *mut AIBinder_Class;
    fn AIBinder_new(c: *const AIBinder_Class, args: *mut c_void) -> *mut AIBinder;
    fn AIBinder_associateClass(b: *mut AIBinder, c: *const AIBinder_Class) -> bool;
    fn AIBinder_getUserData(b: *mut AIBinder) -> *mut c_void;
    fn AIBinder_incStrong(b: *mut AIBinder);
    fn AIBinder_decStrong(b: *mut AIBinder);
    fn AServiceManager_getService(n: *const c_char) -> *mut AIBinder;
    fn AIBinder_prepareTransaction(b: *mut AIBinder, input: *mut *mut AParcel) -> status_t;
    fn AIBinder_transact(b: *mut AIBinder, code: code_t, input: *mut *mut AParcel, output: *mut *mut AParcel, flags: u32) -> status_t;
    fn AParcel_delete(p: *mut AParcel);
    fn AParcel_readInt32(p: *const AParcel, v: *mut i32) -> status_t;
    fn AParcel_readStrongBinder(p: *const AParcel, b: *mut *mut AIBinder) -> status_t;
    fn AParcel_writeInt32(p: *mut AParcel, v: i32) -> status_t;
    fn AParcel_writeBool(p: *mut AParcel, v: bool) -> status_t;
    fn AParcel_writeInt32Array(p: *mut AParcel, data: *const i32, len: i32) -> status_t;
    fn AParcel_writeFloatArray(p: *mut AParcel, data: *const f32, len: i32) -> status_t;
    fn AParcel_writeParcelableArray(p: *mut AParcel, data: *const c_void, len: i32, writer: Option<unsafe extern "C" fn(*mut AParcel, *const c_void, usize) -> status_t>) -> status_t;
    fn AParcel_writeString(p: *mut AParcel, s: *const c_char, n: i32) -> status_t;
    fn AServiceManager_addService(b: *mut AIBinder, n: *const c_char) -> status_t;
    fn ABinderProcess_setThreadPoolMaxThreadCount(n: u32);
    fn ABinderProcess_startThreadPool();
    fn ABinderProcess_joinThreadPool();
    fn __android_log_print(p: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

static DESC: &[u8] = b"vendor.xiaomi.hardware.displayfeature_aidl.IDisplayFeature\0";
static NAME: &[u8] = b"vendor.xiaomi.hardware.displayfeature_aidl.IDisplayFeature/default\0";
static TAG: &[u8] = b"TB375FCDisplayCompatRust\0";
static HASH_VALUE: &[u8] = b"tb375fc-display-compat-rust-v1\0";
static PQ_DESC: &[u8] = b"vendor.mediatek.hardware.pq_aidl.IPictureQuality_AIDL\0";

struct State { callback: *mut AIBinder }

static EXPERT_VALUES: OnceLock<Mutex<[i32; 9]>> = OnceLock::new();
// HyperOS screen_color_level: 1=warm, 2=natural, 3=cool. Keep the selected
// scheme and temperature together so the production backend preserves the
// same mode/value semantics validated during device bring-up.
static COLOR_STATE: OnceLock<Mutex<[i32; 2]>> = OnceLock::new();

unsafe fn log(msg: &'static [u8]) { __android_log_print(4, TAG.as_ptr() as _, msg.as_ptr() as _); }

unsafe fn log_status(label: &'static [u8], status: status_t) {
    let fmt = b"%s status=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, label.as_ptr(), status);
}

unsafe fn log_args(a: [i32; 4], enabled: i32) {
    let fmt = b"feature display=%d mode=%d value=%d cookie=%d enabled=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, a[0], a[1], a[2], a[3], enabled);
}

unsafe fn log_feature(a: [i32; 4]) {
    let fmt = b"feature display=%d mode=%d value=%d cookie=%d\0";
    __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, a[0], a[1], a[2], a[3]);
}

unsafe extern "C" fn destroy(p: *mut c_void) {
    if !p.is_null() { let s = Box::from_raw(p as *mut State); if !s.callback.is_null() { AIBinder_decStrong(s.callback); } }
}

unsafe extern "C" fn create(p: *mut c_void) -> *mut c_void {
    p
}

unsafe extern "C" fn pq_transact(_b: *mut AIBinder, _code: code_t, _input: *const AParcel, _output: *mut AParcel) -> status_t {
    UNKNOWN
}

unsafe extern "C" fn pq_create(p: *mut c_void) -> *mut c_void {
    p
}

unsafe extern "C" fn pq_destroy(_p: *mut c_void) {}

unsafe extern "C" fn write_matrix_row(p: *mut AParcel, data: *const c_void, index: usize) -> status_t {
    let rows = data as *const [[i32; 3]; 3];
    if rows.is_null() || index >= 3 {
        return BAD_VALUE;
    }
    let row = &(*rows)[index];
    // The generated MTK client serializes each nested array as an int32 array.
    AParcel_writeInt32Array(p, row.as_ptr(), 3)
}

unsafe extern "C" fn transact(b: *mut AIBinder, code: code_t, input: *const AParcel, output: *mut AParcel) -> status_t {
    if code == HASH { return AParcel_writeString(output, HASH_VALUE.as_ptr() as _, (HASH_VALUE.len() - 1) as _); }
    if code == VERSION { return AParcel_writeInt32(output, 2); }
    let s = AIBinder_getUserData(b) as *mut State;
    if s.is_null() { return BAD_VALUE; }
    if code == REGISTER_CALLBACK {
        let mut id = 0; let mut cb = ptr::null_mut();
        if AParcel_readInt32(input, &mut id) != OK || AParcel_readStrongBinder(input, &mut cb) != OK { return BAD_VALUE; }
        if !(*s).callback.is_null() { AIBinder_decStrong((*s).callback); }
        (*s).callback = cb; if !cb.is_null() { AIBinder_incStrong(cb); }
        log(b"registerCallback\0"); let _ = id; return OK;
    }
    if code == SET_FEATURE {
        let mut a = [0i32; 4]; for x in &mut a { if AParcel_readInt32(input, x) != OK { return BAD_VALUE; } }
        log_feature(a); apply_feature(a); return OK;
    }
    log(b"unknown transaction\0"); UNKNOWN
}

unsafe fn pq_call2(pq: *mut AIBinder, class: *const AIBinder_Class, code: code_t, a: i32, b: i32) -> status_t {
    if pq.is_null() {
        return BAD_VALUE;
    }
    let associated = AIBinder_associateClass(pq, class);
    if !associated {
        log(b"PQ associateClass failed\0");
        return BAD_VALUE;
    }
    let mut input = ptr::null_mut();
    let prepared = AIBinder_prepareTransaction(pq, &mut input);
    if prepared != OK || input.is_null() {
        log_status(b"PQ prepareTransaction\0", prepared);
        return BAD_VALUE;
    }
    if AParcel_writeInt32(input, a) != OK || AParcel_writeInt32(input, b) != OK {
        AParcel_delete(input);
        return BAD_VALUE;
    }
    let mut output = ptr::null_mut();
    let status = AIBinder_transact(pq, code, &mut input, &mut output, 0);
    log_status(b"PQ transact\0", status);
    if !output.is_null() {
        AParcel_delete(output);
    }
    status
}

unsafe fn pq_call4(pq: *mut AIBinder, class: *const AIBinder_Class, code: code_t, values: [i32; 4]) -> status_t {
    if pq.is_null() || !AIBinder_associateClass(pq, class) {
        return BAD_VALUE;
    }
    let mut input = ptr::null_mut();
    let prepared = AIBinder_prepareTransaction(pq, &mut input);
    if prepared != OK || input.is_null() {
        log_status(b"PQ prepareTransaction4\0", prepared);
        return BAD_VALUE;
    }
    for value in values {
        if AParcel_writeInt32(input, value) != OK {
            AParcel_delete(input);
            return BAD_VALUE;
        }
    }
    let mut output = ptr::null_mut();
    let status = AIBinder_transact(pq, code, &mut input, &mut output, 0);
    log_status(b"PQ transact4\0", status);
    if !output.is_null() {
        AParcel_delete(output);
    }
    status
}

unsafe fn apply_expert(pq: *mut AIBinder, class: *const AIBinder_Class, a: [i32; 4]) {
    let display_id = a[0];
    let cookie = a[3];
    if (0..9).contains(&cookie) {
        // ExpertData defaults: gamut=0, RGB=255, hue/saturation/value/contrast=0, gamma=220.
        let values = EXPERT_VALUES.get_or_init(|| Mutex::new([0, 255, 255, 255, 0, 0, 0, 0, 220]));
        if let Ok(mut current) = values.lock() {
            // ExpertData cookies: 0 gamut, 1/2/3 RGB, 4 hue, 5 saturation,
            // 6 value, 7 contrast, 8 gamma. Do not feed non-RGB values into
            // RGB gain: that changes luminance and was the previous bug.
            let value = if (1..=3).contains(&cookie) { a[2].clamp(50, 255) } else { a[2] };
            current[cookie as usize] = value;
            if (1..=3).contains(&cookie) {
                let rgb = [current[1], current[2], current[3], display_id];
                let status = pq_call4(pq, class, PQ_SET_RGB_GAIN, rgb);
                log_status(b"PQ expert RGB transact\0", status);
            } else if (4..=7).contains(&cookie) {
                let hue = current[4].clamp(-127, 127) as f32 / 127.0;
                let saturation = 1.0 + current[5].clamp(-50, 50) as f32 / 100.0;
                let value_scale = 1.0 + current[6].clamp(-255, 255) as f32 / 255.0;
                let contrast = 1.0 + current[7].clamp(-100, 100) as f32 / 100.0;
                let angle = hue * 0.75;
                let c = angle.cos();
                let s = angle.sin();
                let mut matrix = [[0.0_f32; 4]; 4];
                let luma = [0.2126_f32, 0.7152_f32, 0.0722_f32];
                for row in 0..3 {
                    for col in 0..3 {
                        let identity = if row == col { 1.0 } else { 0.0 };
                        let cross = if (row + 1) % 3 == col { s } else if (col + 1) % 3 == row { -s } else { 0.0 };
                        let rotated = identity * c + cross * 0.5;
                        matrix[row][col] = (luma[row] * (1.0 - saturation) + rotated * saturation) * contrast * value_scale;
                    }
                    matrix[row][3] = 0.0;
                }
                matrix[3] = [0.0, 0.0, 0.0, 1.0];
                let status = pq_call_color_transform(pq, class, matrix, display_id, cookie);
                log_status(b"PQ expert color-transform transact\0", status);
            } else if cookie == 8 {
                let gamma = current[8].clamp(170, 270);
                let status = pq_call2(pq, class, PQ_SET_GAMMA, gamma, display_id);
                log_status(b"PQ expert gamma transact\0", status);
            } else {
                let fmt = b"expert cookie=%d value=%d\0";
                __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, cookie, value);
            }
        }
    } else {
        // Compatibility test/reset path: a neutral RGB gain must preserve
        // panel brightness and always use displayId as the fourth argument.
        let gain = if a[2] == 0 { 255 } else { a[2].clamp(50, 255) };
        let status = pq_call4(pq, class, PQ_SET_RGB_GAIN, [gain, gain, gain, display_id]);
        log_status(b"PQ expert RGB transact\0", status);
    }
}

unsafe fn pq_call_matrix(pq: *mut AIBinder, class: *const AIBinder_Class, matrix: [[i32; 3]; 3], display_id: i32) -> status_t {
    if pq.is_null() || !AIBinder_associateClass(pq, class) {
        return BAD_VALUE;
    }
    let mut input = ptr::null_mut();
    let prepared = AIBinder_prepareTransaction(pq, &mut input);
    if prepared != OK || input.is_null() {
        log_status(b"PQ prepareMatrix\0", prepared);
        return BAD_VALUE;
    }
    let rows_status = AParcel_writeParcelableArray(input, &matrix as *const _ as _, 3, Some(write_matrix_row));
    if rows_status != OK || AParcel_writeInt32(input, display_id) != OK {
        AParcel_delete(input);
        return BAD_VALUE;
    }
    let mut output = ptr::null_mut();
    let status = AIBinder_transact(pq, PQ_SET_COLOR_MATRIX, &mut input, &mut output, 0);
    log_status(b"PQ matrix transact\0", status);
    if !output.is_null() {
        AParcel_delete(output);
    }
    status
}

unsafe fn pq_call_ccorr(pq: *mut AIBinder, class: *const AIBinder_Class, matrix: [[i32; 3]; 3], enabled: bool) -> status_t {
    if pq.is_null() || !AIBinder_associateClass(pq, class) {
        return BAD_VALUE;
    }
    let mut input = ptr::null_mut();
    let prepared = AIBinder_prepareTransaction(pq, &mut input);
    if prepared != OK || input.is_null() {
        log_status(b"PQ prepareCCORR\0", prepared);
        return BAD_VALUE;
    }
    let rows = AParcel_writeParcelableArray(input, &matrix as *const _ as _, 3, Some(write_matrix_row));
    let flag = AParcel_writeBool(input, enabled);
    if rows != OK || flag != OK {
        AParcel_delete(input);
        return BAD_VALUE;
    }
    let mut output = ptr::null_mut();
    let status = AIBinder_transact(pq, PQ_SET_CCORR_INFO, &mut input, &mut output, 0);
    log_status(b"PQ CCORR transact\0", status);
    if !output.is_null() {
        AParcel_delete(output);
    }
    status
}

unsafe extern "C" fn write_float_row(p: *mut AParcel, data: *const c_void, index: usize) -> status_t {
    let rows = data as *const [[f32; 4]; 4];
    if rows.is_null() || index >= 4 {
        return BAD_VALUE;
    }
    AParcel_writeFloatArray(p, (*rows)[index].as_ptr(), 4)
}

unsafe fn pq_call_color_transform(pq: *mut AIBinder, class: *const AIBinder_Class, matrix: [[f32; 4]; 4], display_id: i32, cookie: i32) -> status_t {
    if pq.is_null() || !AIBinder_associateClass(pq, class) {
        return BAD_VALUE;
    }
    let mut input = ptr::null_mut();
    let prepared = AIBinder_prepareTransaction(pq, &mut input);
    if prepared != OK || input.is_null() {
        log_status(b"PQ prepareColorTransform\0", prepared);
        return BAD_VALUE;
    }
    let matrix_status = AParcel_writeParcelableArray(input, &matrix as *const _ as _, 4, Some(write_float_row));
    let display_status = AParcel_writeInt32(input, display_id);
    let cookie_status = AParcel_writeInt32(input, cookie);
    if matrix_status != OK || display_status != OK || cookie_status != OK {
        AParcel_delete(input);
        return BAD_VALUE;
    }
    let mut output = ptr::null_mut();
    let status = AIBinder_transact(pq, PQ_SET_COLOR_TRANSFORM, &mut input, &mut output, 0);
    log_status(b"PQ color-transform transact\0", status);
    if !output.is_null() {
        AParcel_delete(output);
    }
    status
}

unsafe fn apply_feature(a: [i32; 4]) {
    // Mode 3 is the verified HyperOS eye-care path. MTK accepts display id and
    // strength for setBlueLightStrength; enableBlueLight uses display id/bool.
    if a[1] != 3 && (a[1] < 0 || a[1] > 2) && a[1] != EXPERT_MODE && a[1] != MATRIX_TEST_MODE && a[1] != RGB_BASELINE_MODE && a[1] != CCORR_TEST_MODE {
        return;
    }
    let pq_name = b"vendor.mediatek.hardware.pq_aidl.IPictureQuality_AIDL/default\0";
    let pq = AServiceManager_getService(pq_name.as_ptr() as _);
    if pq.is_null() {
        log(b"PQ service unavailable\0");
        return;
    }
    let pq_class = AIBinder_Class_define(PQ_DESC.as_ptr() as _, Some(pq_create), Some(pq_destroy), Some(pq_transact));
    if pq_class.is_null() {
        AIBinder_decStrong(pq);
        log(b"PQ class creation failed\0");
        return;
    }
    let enabled = if a[2] > 0 { 1 } else { 0 };
    if a[1] == EXPERT_MODE {
        apply_expert(pq, pq_class, a);
        AIBinder_decStrong(pq);
        return;
    }
    if a[1] == RGB_BASELINE_MODE {
        let r = pq_call4(pq, pq_class, PQ_SET_RGB_GAIN, [255, 255, 255, a[0]]);
        log_status(b"PQ RGB baseline transact\0", r);
        AIBinder_decStrong(pq);
        return;
    }
    if a[1] == MATRIX_TEST_MODE {
        // Matrix test presets: value 0 restores identity; non-zero applies a
        // modest red/blue exchange for visible validation.
        let matrix = if a[2] == 0 {
            [[1024, 0, 0], [0, 1024, 0], [0, 0, 1024]]
        } else {
            [[900, 0, 124], [0, 1024, 0], [124, 0, 900]]
        };
        let r = pq_call_matrix(pq, pq_class, matrix, a[0]);
        log_status(b"PQ matrix test\0", r);
        AIBinder_decStrong(pq);
        return;
    }
    if a[1] == CCORR_TEST_MODE {
        let matrix = if a[2] == 0 {
            [[1024, 0, 0], [0, 1024, 0], [0, 0, 1024]]
        } else {
            [[900, 0, 124], [0, 1024, 0], [124, 0, 900]]
        };
        let r = pq_call_ccorr(pq, pq_class, matrix, a[2] != 0);
        log_status(b"PQ CCORR test\0", r);
        AIBinder_decStrong(pq);
        return;
    }
    if a[1] >= 0 && a[1] <= 2 {
        // HyperOS mode 0/1/2 selects the color-scheme branch. Its value is
        // screen_color_level: 1 warm, 2 natural, 3 cool. MTK receives the
        // same value through setPQMode(value, displayId).
        let state = COLOR_STATE.get_or_init(|| Mutex::new([0, 2]));
        if let Ok(mut current) = state.lock() {
            current[0] = a[1];
            if (1..=3).contains(&a[2]) {
                current[1] = a[2];
            }
            let fmt = b"color scheme=%d temperature=%d\0";
            __android_log_print(4, TAG.as_ptr() as _, fmt.as_ptr() as _, current[0], current[1]);
        }
        let r = pq_call2(pq, pq_class, PQ_SET_MODE, a[2], a[0]);
        log_status(b"PQ color-mode transact\0", r);
        AIBinder_decStrong(pq);
        return;
    }
    log_args(a, enabled);
    // enableBlueLight(bool enabled, int displayId).
    let r1 = pq_call2(pq, pq_class, PQ_ENABLE_BLUE_LIGHT, enabled, a[0]);
    // setBlueLightStrength(int strength, int displayId).
    let r2 = pq_call2(pq, pq_class, PQ_SET_BLUE_LIGHT_STRENGTH, a[2].max(0), a[0]);
    if r1 != OK || r2 != OK {
        log(b"PQ blue-light transaction failed\0");
    }
    AIBinder_decStrong(pq);
}

fn main() {
    unsafe {
        ABinderProcess_setThreadPoolMaxThreadCount(2);
        ABinderProcess_startThreadPool();
        let state = Box::new(State { callback: ptr::null_mut() });
        let class = AIBinder_Class_define(DESC.as_ptr() as _, Some(create), Some(destroy), Some(transact));
        if class.is_null() { log(b"class creation failed\0"); std::process::exit(1); }
        let binder = AIBinder_new(class, Box::into_raw(state) as _);
        if binder.is_null() { log(b"binder creation failed\0"); std::process::exit(1); }
        let status = AServiceManager_addService(binder, NAME.as_ptr() as _);
        if status != OK {
            if status == -22 { log(b"service registration BAD_VALUE\0"); }
            else if status == -17 { log(b"service registration ALREADY_EXISTS\0"); }
            else { log(b"service registration failed\0"); }
            AIBinder_decStrong(binder);
            std::process::exit(1);
        }
        log(b"registered\0"); ABinderProcess_joinThreadPool();
    }
}
