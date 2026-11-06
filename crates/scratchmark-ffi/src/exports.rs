use crate::{
    buffers::{input, output, required},
    create, guard, with_doc,
};
use scratchmark_core::{
    Bounds, BrushOptions, BrushPoint, Error, MAX_BATCH, MAX_POINTS, MAX_SNAPSHOT, MAX_STROKES,
    Point, Sample, State, StrokeInfo, Viewport,
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
mod erase_path_tests;
#[cfg(test)]
mod hit_tests;
#[cfg(test)]
mod select_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod viewport_tests;

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

/// # Safety
/// 输出指针必须满足 spec/M0.md 的可写、对齐与独占约束；语义参 spec/M4.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_hit_test(
    handle: u64,
    revision: u64,
    x: f64,
    y: f64,
    tolerance: f64,
    out_index: *mut u32,
    out_found: *mut u32,
) -> u32 {
    guard(|| {
        required(out_index)?;
        required(out_found)?;
        let hit = with_doc(handle, |d| d.hit_test(revision, x, y, tolerance))?;
        unsafe {
            out_index.write(hit.unwrap_or(0));
            out_found.write(u32::from(hit.is_some()));
        }
        Ok(())
    })
}

/// # Safety
/// 句柄必须有效；语义参 spec/M4.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_erase_stroke(handle: u64, revision: u64, index: u32) -> u32 {
    guard(|| with_doc(handle, |d| d.erase_stroke(revision, index)))
}

/// # Safety
/// 擦除路径缓冲区必须覆盖 count 个点，调用期间不得被其他线程修改；语义参 spec/M5.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_erase_path(
    handle: u64,
    revision: u64,
    path: *const Point,
    count: u32,
    radius: f64,
) -> u32 {
    guard(|| {
        let points = unsafe { input(path, count, MAX_BATCH)? };
        with_doc(handle, |d| d.erase_path(revision, points, radius))
    })
}

/// # Safety
/// 多边形与输出区必须满足 spec/M0.md 的指针、对齐与容量约束；语义参 spec/M6.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_lasso_hits(
    handle: u64,
    revision: u64,
    path: *const Point,
    count: u32,
    out_indices: *mut u32,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(out_written)?;
        let points = unsafe { input(path, count, MAX_BATCH)? };
        let slots = unsafe { output(out_indices, capacity, MAX_POINTS)? };
        let hits = with_doc(handle, |d| d.lasso_hits(revision, points))?;
        unsafe {
            out_written.write(hits.len() as u32);
        }
        if slots.len() < hits.len() {
            return Err(Error::BufferTooSmall);
        }
        for (slot, index) in slots.iter_mut().zip(&hits) {
            *slot = *index;
        }
        Ok(())
    })
}

/// # Safety
/// 索引缓冲区必须覆盖 count 个有效索引，调用期间不得被其他线程修改；语义参 spec/M6.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_erase_indices(
    handle: u64,
    revision: u64,
    indices: *const u32,
    count: u32,
) -> u32 {
    guard(|| {
        let list = unsafe { input(indices, count, MAX_STROKES)? };
        with_doc(handle, |d| d.erase_indices(revision, list))
    })
}

/// # Safety
/// 句柄必须有效；语义参 spec/M6.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_clear(handle: u64, revision: u64) -> u32 {
    guard(|| with_doc(handle, |d| d.clear(revision)))
}

/// # Safety
/// 输出指针必须满足 spec/M0.md 的可写、对齐与独占约束。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_get_page_id(handle: u64, out_page_id: *mut u64) -> u32 {
    guard(|| {
        required(out_page_id)?;
        let page_id = with_doc(handle, |d| Ok(d.page_id()))?;
        unsafe {
            out_page_id.write(page_id);
        }
        Ok(())
    })
}

/// # Safety
/// 句柄必须有效；页面标识语义参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_set_page_id(handle: u64, page_id: u64) -> u32 {
    guard(|| with_doc(handle, |d| d.set_page_id(page_id)))
}

/// # Safety
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_page_to_viewport(
    viewport: *const Viewport,
    input: *const Point,
    out_point: *mut Point,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(input)?;
        required(out_point)?;
        let point = unsafe { viewport.read() }.page_to_viewport(unsafe { input.read() })?;
        unsafe {
            out_point.write(point);
        }
        Ok(())
    })
}

/// # Safety
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_viewport_to_page(
    viewport: *const Viewport,
    input: *const Point,
    out_point: *mut Point,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(input)?;
        required(out_point)?;
        let point = unsafe { viewport.read() }.viewport_to_page(unsafe { input.read() })?;
        unsafe {
            out_point.write(point);
        }
        Ok(())
    })
}

/// # Safety
/// 视口与输出指针必须有效、对齐且互不重叠；约束参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_width_page_to_viewport(
    viewport: *const Viewport,
    width: f64,
    out_width: *mut f64,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(out_width)?;
        let width = unsafe { viewport.read() }.width_page_to_viewport(width)?;
        unsafe {
            out_width.write(width);
        }
        Ok(())
    })
}

/// # Safety
/// 视口与输出指针必须有效、对齐且互不重叠；约束参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_width_viewport_to_page(
    viewport: *const Viewport,
    width: f64,
    out_width: *mut f64,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(out_width)?;
        let width = unsafe { viewport.read() }.width_viewport_to_page(width)?;
        unsafe {
            out_width.write(width);
        }
        Ok(())
    })
}

/// # Safety
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M3.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_bounds_page_to_viewport(
    viewport: *const Viewport,
    input: *const Bounds,
    out_bounds: *mut Bounds,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(input)?;
        required(out_bounds)?;
        let bounds = unsafe { viewport.read() }.bounds_page_to_viewport(unsafe { input.read() })?;
        unsafe {
            out_bounds.write(bounds);
        }
        Ok(())
    })
}
