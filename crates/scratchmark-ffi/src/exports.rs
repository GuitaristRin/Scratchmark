use crate::{
    buffers::{input, output, required},
    create, guard, with_doc,
};
use scratchmark_core::{
    BrushOptions, BrushPoint, Error, MAX_BATCH, MAX_POINTS, MAX_SNAPSHOT, Sample, State, StrokeInfo,
};

/// # Safety
/// 输出指针必须满足 spec/M0.md 的可写、对齐与独占约束。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_create(out_handle: *mut u64) -> u32 {
    guard(|| {
        required(out_handle)?;
        let handle = create()?;
        unsafe {
            out_handle.write(handle);
        }
        Ok(())
    })
}

/// # Safety
/// 输入缓冲区必须覆盖 count 个有效样本，调用期间不得被其他线程修改。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_append(handle: u64, samples: *const Sample, count: u32) -> u32 {
    guard(|| {
        let points = unsafe { input(samples, count, MAX_BATCH)? };
        with_doc(handle, |d| d.append(points))
    })
}

/// # Safety
/// 输出指针必须满足 spec/M0.md 的可写、对齐与独占约束。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_get_state(handle: u64, out_state: *mut State) -> u32 {
    guard(|| {
        required(out_state)?;
        let state = with_doc(handle, |d| Ok(d.state()))?;
        unsafe {
            out_state.write(state);
        }
        Ok(())
    })
}

/// # Safety
/// 输出指针必须满足 spec/M0.md 的可写、对齐与独占约束。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_get_stroke(
    handle: u64,
    revision: u64,
    index: u32,
    out_info: *mut StrokeInfo,
) -> u32 {
    guard(|| {
        required(out_info)?;
        let info = with_doc(handle, |d| d.stroke_info(revision, index))?;
        unsafe {
            out_info.write(info);
        }
        Ok(())
    })
}

/// # Safety
/// 两个输出缓冲区必须有效且互不重叠；数组大小至少为 capacity。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_read_points(
    handle: u64,
    revision: u64,
    index: u32,
    start: u32,
    out_points: *mut Sample,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(out_written)?;
        let points = unsafe { output(out_points, capacity, MAX_POINTS)? };
        let count = with_doc(handle, |d| d.read_points(revision, index, start, points))?;
        unsafe {
            out_written.write(count as u32);
        }
        Ok(())
    })
}

/// # Safety
/// 输出区必须有效、互不重叠且符合声明容量；NULL / 0 仅用于数据区查询。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_save(
    handle: u64,
    out_data: *mut u8,
    capacity: u32,
    out_required: *mut u32,
) -> u32 {
    guard(|| {
        required(out_required)?;
        let output = unsafe { output(out_data, capacity, MAX_SNAPSHOT)? };
        let bytes = with_doc(handle, |d| d.save())?;
        if output.len() < bytes.len() {
            unsafe {
                out_required.write(bytes.len() as u32);
            }
            return Err(Error::BufferTooSmall);
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        unsafe {
            out_required.write(bytes.len() as u32);
        }
        Ok(())
    })
}

/// # Safety
/// 输入数据区必须在整个调用期间有效，大小至少为 length 字节。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_load(handle: u64, data: *const u8, length: u32) -> u32 {
    guard(|| {
        let bytes = unsafe { input(data, length, MAX_SNAPSHOT)? };
        with_doc(handle, |d| d.load(bytes))
    })
}

#[cfg(test)]
mod brush_tests;
#[cfg(test)]
mod tests;

/// # Safety
/// 配置有效、对齐，输出区独占且互不重叠；约束参 spec/M2.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_read_brush_points(
    handle: u64,
    revision: u64,
    index: u32,
    start: u32,
    options: *const BrushOptions,
    out_points: *mut BrushPoint,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(options)?;
        required(out_written)?;
        let options = unsafe { options.read() };
        let points = unsafe { output(out_points, capacity, MAX_POINTS)? };
        let n = with_doc(handle, |d| {
            d.read_brush_points(revision, index, start, options, points)
        })?;
        unsafe {
            out_written.write(n as u32);
        }
        Ok(())
    })
}
