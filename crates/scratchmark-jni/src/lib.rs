//! Scratchmark 的 JNI 封装：把引擎暴露给 Android 的 Kotlin 宿主。
//!
//! 设计要点：
//! - 只做**编组**，语义全部来自 `scratchmark-core`；不复制 C ABI 的逻辑，避免两套行为分叉。
//! - 句柄是 `u64`（对齐 C ABI 的 `SmHandle`），Kotlin 侧当 `Long` 用。
//! - 批量数据走 `ByteArray`（样本 32 字节 / 笔刷点 24 字节），避免逐字段 JNI 调用。
//! - 每次调用都过 `catch_unwind`：单个坏参数不能把宿主进程带走。
//!
//! 本文件在 Windows 上也能编译，因此“能不能编”在本地就有证据；运行时验证需要设备或模拟器。

use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JDoubleArray, JIntArray};
use jni::sys::{jbyteArray, jdouble, jdoubleArray, jint, jlong, jlongArray};
use scratchmark_core::{
    BrushKind, BrushOptions, BrushPoint, Document, Error, Point, Sample, State, Transform,
};
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex, OnceLock};

const SAMPLE_SIZE: usize = std::mem::size_of::<Sample>();
const BRUSH_POINT_SIZE: usize = std::mem::size_of::<BrushPoint>();
/// 与 C ABI 一致：bit 0–16。
const FEATURES: jlong = 0b1_1111_1111_1111_1111;

fn registry() -> &'static Mutex<HashMap<u64, Arc<Mutex<Document>>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, Arc<Mutex<Document>>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_handle() -> u64 {
    static NEXT: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut guard = NEXT
        .get_or_init(|| Mutex::new(1))
        .lock()
        .expect("handle counter");
    let value = *guard;
    *guard += 1;
    value
}

/// 取出句柄对应的文档；句柄无效时返回 None（Kotlin 侧得到错误码而不是崩溃）。
fn with_doc<R>(
    handle: jlong,
    f: impl FnOnce(&mut Document) -> scratchmark_core::Result<R>,
) -> Option<scratchmark_core::Result<R>> {
    let arc = {
        let map = registry().lock().ok()?;
        map.get(&(handle as u64))?.clone()
    };
    let mut document = arc.lock().ok()?;
    Some(f(&mut document))
}

/// 所有导出都过这层：panic 关在 Rust 侧，不跨 FFI 传播。
fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(value) => value,
        Err(_) => fallback,
    }
}

fn blank_sample() -> Sample {
    Sample {
        x: 0.0,
        y: 0.0,
        time_us: 0,
        pressure: 0.0,
        flags: 0,
    }
}

fn encode_samples(samples: &[Sample]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(samples.len() * SAMPLE_SIZE);
    for sample in samples {
        bytes.extend_from_slice(&sample.x.to_le_bytes());
        bytes.extend_from_slice(&sample.y.to_le_bytes());
        bytes.extend_from_slice(&sample.time_us.to_le_bytes());
        bytes.extend_from_slice(&sample.pressure.to_le_bytes());
        bytes.extend_from_slice(&sample.flags.to_le_bytes());
    }
    bytes
}

fn decode_samples(bytes: &[u8]) -> Option<Vec<Sample>> {
    if !bytes.len().is_multiple_of(SAMPLE_SIZE) {
        return None;
    }
    let mut samples = Vec::with_capacity(bytes.len() / SAMPLE_SIZE);
    for chunk in bytes.chunks_exact(SAMPLE_SIZE) {
        let x = f64::from_le_bytes(chunk[0..8].try_into().ok()?);
        let y = f64::from_le_bytes(chunk[8..16].try_into().ok()?);
        let time_us = u64::from_le_bytes(chunk[16..24].try_into().ok()?);
        let pressure = f32::from_le_bytes(chunk[24..28].try_into().ok()?);
        let flags = u32::from_le_bytes(chunk[28..32].try_into().ok()?);
        samples.push(Sample {
            x,
            y,
            time_us,
            pressure,
            flags,
        });
    }
    Some(samples)
}

fn encode_brush_points(points: &[BrushPoint]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(points.len() * BRUSH_POINT_SIZE);
    for point in points {
        bytes.extend_from_slice(&point.x.to_le_bytes());
        bytes.extend_from_slice(&point.y.to_le_bytes());
        bytes.extend_from_slice(&point.width.to_le_bytes());
    }
    bytes
}

fn encode_state(state: State) -> Vec<i64> {
    vec![
        state.revision as i64,
        state.stroke_count as i64,
        state.active_points as i64,
        state.has_active as i64,
        state.can_undo as i64,
        state.can_redo as i64,
    ]
}

fn into_raw_bytes(env: &mut JNIEnv, bytes: &[u8]) -> jbyteArray {
    match env.byte_array_from_slice(bytes) {
        Ok(array) => array.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn into_raw_longs(env: &mut JNIEnv, values: &[i64]) -> jlongArray {
    match env.new_long_array(values.len() as i32) {
        Ok(array) => match env.set_long_array_region(&array, 0, values) {
            Ok(()) => array.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

fn into_raw_doubles(env: &mut JNIEnv, values: &[f64]) -> jdoubleArray {
    match env.new_double_array(values.len() as i32) {
        Ok(array) => match env.set_double_array_region(&array, 0, values) {
            Ok(()) => array.into_raw(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

fn status_of(result: Option<scratchmark_core::Result<()>>) -> jint {
    match result {
        Some(Ok(())) => 0,
        Some(Err(Error::StaleRevision)) => 5,
        Some(Err(Error::InvalidArgument)) => 1,
        Some(Err(Error::InvalidState)) => 3,
        Some(Err(Error::LimitExceeded)) => 6,
        Some(Err(_)) | None => -1,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_abiVersion(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    1
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_features(_env: JNIEnv, _class: JClass) -> jlong {
    FEATURES
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_create(_env: JNIEnv, _class: JClass) -> jlong {
    guard(0, || {
        let handle = next_handle();
        match registry().lock() {
            Ok(mut map) => {
                map.insert(handle, Arc::new(Mutex::new(Document::default())));
                handle as jlong
            }
            Err(_) => 0,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_destroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    guard((), || {
        if let Ok(mut map) = registry().lock() {
            map.remove(&(handle as u64));
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_begin(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    width: jdouble,
    color: jint,
) -> jint {
    guard(-1, || {
        status_of(with_doc(handle, |d| d.begin(width, color as u32)))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_append(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    samples: JByteArray,
) -> jint {
    guard(-1, || {
        let Ok(bytes) = env.convert_byte_array(&samples) else {
            return -1;
        };
        let Some(points) = decode_samples(&bytes) else {
            return -1;
        };
        status_of(with_doc(handle, |d| d.append(&points)))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_finish(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    guard(-1, || status_of(with_doc(handle, |d| d.finish())))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_cancel(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    guard(-1, || status_of(with_doc(handle, |d| d.cancel())))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_undo(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    guard(-1, || status_of(with_doc(handle, |d| d.undo())))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_redo(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    guard(-1, || status_of(with_doc(handle, |d| d.redo())))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_state<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jlongArray {
    let values = guard(vec![0i64; 6], || {
        with_doc(handle, |d| Ok(d.state()))
            .and_then(|value| value.ok())
            .map(encode_state)
            .unwrap_or_else(|| vec![0i64; 6])
    });
    into_raw_longs(&mut env, &values)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_save<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jbyteArray {
    let bytes = guard(None, || {
        with_doc(handle, |d| d.save()).and_then(|value| value.ok())
    });
    match bytes {
        Some(bytes) => into_raw_bytes(&mut env, &bytes),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_load(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    payload: JByteArray,
) -> jint {
    guard(-1, || {
        let Ok(bytes) = env.convert_byte_array(&payload) else {
            return -1;
        };
        status_of(with_doc(handle, |d| d.load(&bytes)))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_points<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    revision: jlong,
    index: jint,
) -> jbyteArray {
    let bytes = guard(None, || {
        with_doc(handle, |d| {
            let count = d.stroke_info(revision as u64, index as u32)?.point_count as usize;
            let mut buffer = vec![blank_sample(); count];
            let written = d.read_points(revision as u64, index as u32, 0, &mut buffer)?;
            buffer.truncate(written);
            Ok(encode_samples(&buffer))
        })
        .and_then(|value| value.ok())
    });
    match bytes {
        Some(bytes) => into_raw_bytes(&mut env, &bytes),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_brushPoints<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    revision: jlong,
    index: jint,
    alpha: jdouble,
    min_width_ratio: jdouble,
    pressure_enabled: jint,
) -> jbyteArray {
    let options = BrushOptions {
        alpha,
        min_width_ratio,
        pressure_enabled: pressure_enabled as u32,
        reserved: 0,
    };
    let bytes = guard(None, || {
        with_doc(handle, |d| {
            let count = d.stroke_info(revision as u64, index as u32)?.point_count as usize;
            let mut buffer = vec![BrushPoint::default(); count];
            let written =
                d.read_brush_points(revision as u64, index as u32, 0, options, &mut buffer)?;
            buffer.truncate(written);
            Ok(encode_brush_points(&buffer))
        })
        .and_then(|value| value.ok())
    });
    match bytes {
        Some(bytes) => into_raw_bytes(&mut env, &bytes),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_hitTest(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    revision: jlong,
    x: jdouble,
    y: jdouble,
    tolerance: jdouble,
) -> jint {
    guard(-1, || {
        match with_doc(handle, |d| d.hit_test(revision as u64, x, y, tolerance)) {
            Some(Ok(Some(index))) => index as jint,
            _ => -1,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_eraseStroke(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    revision: jlong,
    index: jint,
) -> jint {
    guard(-1, || {
        status_of(with_doc(handle, |d| {
            d.erase_stroke(revision as u64, index as u32)
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_erasePath(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    revision: jlong,
    path: JDoubleArray,
    radius: jdouble,
) -> jint {
    guard(-1, || {
        let Ok(length) = env.get_array_length(&path) else {
            return -1;
        };
        if length < 2 || length.rem_euclid(2) != 0 {
            return -1;
        }
        let mut raw = vec![0f64; length as usize];
        if env.get_double_array_region(&path, 0, &mut raw).is_err() {
            return -1;
        }
        let points: Vec<Point> = raw
            .chunks_exact(2)
            .map(|pair| Point {
                x: pair[0],
                y: pair[1],
            })
            .collect();
        status_of(with_doc(handle, |d| {
            d.erase_path(revision as u64, &points, radius)
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_documentBounds<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    revision: jlong,
) -> jdoubleArray {
    let values = guard(vec![0f64; 6], || {
        with_doc(handle, |d| {
            if revision as u64 != d.state().revision {
                return Err(Error::StaleRevision);
            }
            let count = d.state().stroke_count as f64;
            Ok(match d.document_bounds() {
                Some(bounds) => vec![
                    bounds.min_x,
                    bounds.min_y,
                    bounds.max_x,
                    bounds.max_y,
                    count,
                    1.0,
                ],
                None => vec![0.0, 0.0, 0.0, 0.0, count, 0.0],
            })
        })
        .and_then(|value| value.ok())
        .unwrap_or_else(|| vec![0f64; 6])
    });
    into_raw_doubles(&mut env, &values)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_translateAll(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    revision: jlong,
    dx: jdouble,
    dy: jdouble,
) -> jint {
    guard(-1, || {
        status_of(with_doc(handle, |d| {
            d.translate_all(revision as u64, dx, dy)
        }))
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_brushPreset(
    _env: JNIEnv,
    _class: JClass,
    kind: jint,
    level: jint,
) -> jdouble {
    guard(0.0, || {
        BrushKind::from_u32(kind as u32)
            .and_then(|kind| kind.ratio(level as u32))
            .unwrap_or(0.0)
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_copyStrokes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    revision: jlong,
    indices: JIntArray<'local>,
) -> jbyteArray {
    let bytes = guard(None, || {
        let Ok(length) = env.get_array_length(&indices) else {
            return None;
        };
        let mut raw = vec![0i32; length as usize];
        if env.get_int_array_region(&indices, 0, &mut raw).is_err() {
            return None;
        }
        let list: Vec<u32> = raw.into_iter().map(|value| value as u32).collect();
        with_doc(handle, |d| d.copy_strokes(revision as u64, &list)).and_then(|value| value.ok())
    });
    match bytes {
        Some(bytes) => into_raw_bytes(&mut env, &bytes),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_cutStrokes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    revision: jlong,
    indices: JIntArray<'local>,
) -> jbyteArray {
    let bytes = guard(None, || {
        let Ok(length) = env.get_array_length(&indices) else {
            return None;
        };
        let mut raw = vec![0i32; length as usize];
        if env.get_int_array_region(&indices, 0, &mut raw).is_err() {
            return None;
        }
        let list: Vec<u32> = raw.into_iter().map(|value| value as u32).collect();
        with_doc(handle, |d| d.cut_strokes(revision as u64, &list)).and_then(|value| value.ok())
    });
    match bytes {
        Some(bytes) => into_raw_bytes(&mut env, &bytes),
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_scratchmark_Native_paste(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    revision: jlong,
    payload: JByteArray,
    dx: jdouble,
    dy: jdouble,
) -> jint {
    guard(-1, || {
        let Ok(bytes) = env.convert_byte_array(&payload) else {
            return -1;
        };
        let transform = if dx == 0.0 && dy == 0.0 {
            None
        } else {
            Some(Transform::translate(dx, dy))
        };
        status_of(with_doc(handle, |d| {
            d.paste(revision as u64, &bytes, transform.as_ref())
        }))
    })
}
