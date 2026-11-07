use crate::{
    buffers::{input, output, required},
    create, guard, with_doc,
};
use scratchmark_core::{
    Bounds, BrushKind, BrushOptions, BrushPoint, Error, MAX_BATCH, MAX_POINTS,
    MAX_PREDICTION_POINTS, MAX_SNAPSHOT, MAX_STROKES, Point, PredictionOptions,
    PredictionOptionsEx, ResampleOptions, Sample, SampleAxes, ShapeInfo, ShapeKind, State,
    StrokeInfo, Transform, Viewport, ViewportEx,
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
mod axes_tests;
#[cfg(test)]
mod brush_tests;
#[cfg(test)]
mod cut_tests;
#[cfg(test)]
mod erase_path_tests;
#[cfg(test)]
mod extent_tests;
#[cfg(test)]
mod hit_tests;
#[cfg(test)]
mod predict_ex_tests;
#[cfg(test)]
mod predict_tests;
#[cfg(test)]
mod preset_tests;
#[cfg(test)]
mod resample_tests;
#[cfg(test)]
mod select_tests;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transform_tests;
#[cfg(test)]
mod viewport_ex_tests;
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
/// 索引与变换结构必须有效且对齐；语义参 spec/M7.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_transform_strokes(
    handle: u64,
    revision: u64,
    indices: *const u32,
    count: u32,
    transform: *const Transform,
) -> u32 {
    guard(|| {
        required(transform)?;
        let list = unsafe { input(indices, count, MAX_STROKES)? };
        let transform = unsafe { transform.read() };
        with_doc(handle, |d| d.transform_strokes(revision, list, &transform))
    })
}

/// # Safety
/// 输出区必须有效、对齐且符合声明容量；NULL / 0 仅用于大小查询。语义参 spec/M7.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_copy_strokes(
    handle: u64,
    revision: u64,
    indices: *const u32,
    count: u32,
    out_data: *mut u8,
    capacity: u32,
    out_required: *mut u32,
) -> u32 {
    guard(|| {
        required(out_required)?;
        let list = unsafe { input(indices, count, MAX_STROKES)? };
        let slots = unsafe { output(out_data, capacity, MAX_SNAPSHOT)? };
        let bytes = with_doc(handle, |d| d.copy_strokes(revision, list))?;
        unsafe {
            out_required.write(bytes.len() as u32);
        }
        if slots.len() < bytes.len() {
            return Err(Error::BufferTooSmall);
        }
        slots[..bytes.len()].copy_from_slice(&bytes);
        Ok(())
    })
}

/// # Safety
/// 输入数据与变换结构必须在调用期间有效；语义参 spec/M7.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_paste(
    handle: u64,
    revision: u64,
    data: *const u8,
    length: u32,
    transform: *const Transform,
) -> u32 {
    guard(|| {
        let bytes = unsafe { input(data, length, MAX_SNAPSHOT)? };
        let transform = if transform.is_null() {
            None
        } else {
            unsafe { Some(transform.read()) }
        };
        with_doc(handle, |d| d.paste(revision, bytes, transform.as_ref()))
    })
}

/// # Safety
/// 输出指针必须有效且对齐；`flags` 只允许 bit 0。语义参 spec/M8.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_regularize_stroke(
    handle: u64,
    revision: u64,
    index: u32,
    kind: u32,
    flags: u32,
    out_info: *mut ShapeInfo,
) -> u32 {
    guard(|| {
        required(out_info)?;
        if flags & !1 != 0 {
            return Err(Error::InvalidArgument);
        }
        let kind = ShapeKind::from_u32(kind)?;
        let preview = flags & 1 != 0;
        let info = with_doc(handle, |d| {
            d.regularize_stroke(revision, index, kind, preview)
        })?;
        unsafe {
            out_info.write(info);
        }
        Ok(())
    })
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

/// # Safety
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_page_to_viewport_ex(
    viewport: *const ViewportEx,
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
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_viewport_to_page_ex(
    viewport: *const ViewportEx,
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
/// 视口与输出指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_width_page_to_viewport_ex(
    viewport: *const ViewportEx,
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
/// 视口与输出指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_width_viewport_to_page_ex(
    viewport: *const ViewportEx,
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
/// 视口、输入与输出指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_bounds_page_to_viewport_ex(
    viewport: *const ViewportEx,
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

/// # Safety
/// 视口、锚点与输出视口指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_viewport_ex_zoom_at(
    viewport: *const ViewportEx,
    anchor: *const Point,
    factor: f64,
    out_viewport: *mut ViewportEx,
) -> u32 {
    guard(|| {
        required(viewport)?;
        required(anchor)?;
        required(out_viewport)?;
        let zoomed = unsafe { viewport.read() }.zoom_at(unsafe { anchor.read() }, factor)?;
        unsafe {
            out_viewport.write(zoomed);
        }
        Ok(())
    })
}

/// # Safety
/// 页面边界与输出视口指针必须有效、对齐且互不重叠；约束参 spec/M9.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_viewport_ex_fit(
    page_bounds: *const Bounds,
    view_width: f64,
    view_height: f64,
    angle: f64,
    padding: f64,
    out_viewport: *mut ViewportEx,
) -> u32 {
    guard(|| {
        required(page_bounds)?;
        required(out_viewport)?;
        let fitted = ViewportEx::fit(
            unsafe { page_bounds.read() },
            view_width,
            view_height,
            angle,
            padding,
        )?;
        unsafe {
            out_viewport.write(fitted);
        }
        Ok(())
    })
}

/// # Safety
/// 输出区与选项指针必须满足 spec/M0.md 的指针与容量约束；语义参 spec/M10.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_predict(
    handle: u64,
    revision: u64,
    options: *const PredictionOptions,
    out_points: *mut BrushPoint,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(options)?;
        required(out_written)?;
        let options = unsafe { options.read() };
        let slots = unsafe { output(out_points, capacity, MAX_PREDICTION_POINTS as usize)? };
        let predicted = with_doc(handle, |d| d.predict(revision, &options))?;
        unsafe {
            out_written.write(predicted.len() as u32);
        }
        if slots.len() < predicted.len() {
            return Err(Error::BufferTooSmall);
        }
        slots[..predicted.len()].copy_from_slice(&predicted);
        Ok(())
    })
}

/// # Safety
/// 样本与轴向缓冲区都必须覆盖 count 个元素且有效；语义参 spec/M11.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_append_axes(
    handle: u64,
    samples: *const Sample,
    axes: *const SampleAxes,
    count: u32,
) -> u32 {
    guard(|| {
        let points = unsafe { input(samples, count, MAX_BATCH)? };
        let values = unsafe { input(axes, count, MAX_BATCH)? };
        with_doc(handle, |d| d.append_with_axes(points, Some(values)))
    })
}

/// # Safety
/// 输出区必须有效、对齐且容量足够；语义参 spec/M11.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_read_axes(
    handle: u64,
    revision: u64,
    index: u32,
    start: u32,
    out_axes: *mut SampleAxes,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(out_written)?;
        let slots = unsafe { output(out_axes, capacity, MAX_POINTS)? };
        let count = with_doc(handle, |d| d.read_axes(revision, index, start, slots))?;
        unsafe {
            out_written.write(count as u32);
        }
        Ok(())
    })
}

/// # Safety
/// 三个输出指针都必须有效且对齐；语义参 spec/M12.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_document_bounds(
    handle: u64,
    revision: u64,
    out_bounds: *mut Bounds,
    out_strokes: *mut u32,
    out_found: *mut u32,
) -> u32 {
    guard(|| {
        required(out_bounds)?;
        required(out_strokes)?;
        required(out_found)?;
        let (bounds, strokes) = with_doc(handle, |d| {
            if revision != d.state().revision {
                return Err(Error::StaleRevision);
            }
            let count = d.state().stroke_count;
            Ok((d.document_bounds(), count))
        })?;
        let (value, found) = match bounds {
            Some(bounds) => (bounds, 1u32),
            None => (Bounds::default(), 0u32),
        };
        unsafe {
            out_bounds.write(value);
            out_strokes.write(strokes);
            out_found.write(found);
        }
        Ok(())
    })
}

/// # Safety
/// 句柄必须有效；语义参 spec/M12.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_translate_all(handle: u64, revision: u64, dx: f64, dy: f64) -> u32 {
    guard(|| with_doc(handle, |d| d.translate_all(revision, dx, dy)))
}

/// # Safety
/// 输出指针必须有效且对齐；语义参 spec/M13.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_brush_preset(kind: u32, level: u32, out_ratio: *mut f64) -> u32 {
    guard(|| {
        required(out_ratio)?;
        let ratio = BrushKind::from_u32(kind)?.ratio(level)?;
        unsafe {
            out_ratio.write(ratio);
        }
        Ok(())
    })
}

/// # Safety
/// 输出指针必须有效且对齐；语义参 spec/M13.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_brush_preset_width(
    kind: u32,
    level: u32,
    max_width: f64,
    out_width: *mut f64,
) -> u32 {
    guard(|| {
        required(out_width)?;
        let width = BrushKind::from_u32(kind)?.width(level, max_width)?;
        unsafe {
            out_width.write(width);
        }
        Ok(())
    })
}

/// # Safety
/// 选项与输出区必须满足 spec/M0.md 的指针与容量约束；语义参 spec/M14.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_resample_stroke(
    handle: u64,
    revision: u64,
    index: u32,
    options: *const ResampleOptions,
    out_points: *mut BrushPoint,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(options)?;
        required(out_written)?;
        let options = unsafe { options.read() };
        let slots = unsafe { output(out_points, capacity, MAX_POINTS)? };
        let points = with_doc(handle, |d| d.resample_stroke(revision, index, &options))?;
        unsafe {
            out_written.write(points.len() as u32);
        }
        if slots.len() < points.len() {
            return Err(Error::BufferTooSmall);
        }
        slots[..points.len()].copy_from_slice(&points);
        Ok(())
    })
}

/// # Safety
/// 输出区与选项指针必须满足 spec/M0.md 的指针与容量约束；语义参 spec/M15.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_predict_ex(
    handle: u64,
    revision: u64,
    options: *const PredictionOptionsEx,
    out_points: *mut BrushPoint,
    capacity: u32,
    out_written: *mut u32,
) -> u32 {
    guard(|| {
        required(options)?;
        required(out_written)?;
        let options = unsafe { options.read() };
        let slots = unsafe { output(out_points, capacity, MAX_PREDICTION_POINTS as usize)? };
        let predicted = with_doc(handle, |d| d.predict_ex(revision, &options))?;
        unsafe {
            out_written.write(predicted.len() as u32);
        }
        if slots.len() < predicted.len() {
            return Err(Error::BufferTooSmall);
        }
        slots[..predicted.len()].copy_from_slice(&predicted);
        Ok(())
    })
}

/// # Safety
/// 输入索引与输出区必须满足 spec/M0.md 的指针与容量约束；语义参 spec/M16.md。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_cut_strokes(
    handle: u64,
    revision: u64,
    indices: *const u32,
    count: u32,
    out_data: *mut u8,
    capacity: u32,
    out_required: *mut u32,
) -> u32 {
    guard(|| {
        required(out_required)?;
        let list = unsafe { input(indices, count, MAX_STROKES)? };
        let slots = unsafe { output(out_data, capacity, MAX_SNAPSHOT)? };
        // 先用只读复制算出需要多少字节：容量不足时绝不能已经剪掉。
        let probe = with_doc(handle, |d| d.copy_strokes(revision, list))?;
        unsafe {
            out_required.write(probe.len() as u32);
        }
        if slots.len() < probe.len() {
            return Err(Error::BufferTooSmall);
        }
        let bytes = with_doc(handle, |d| d.cut_strokes(revision, list))?;
        slots[..bytes.len()].copy_from_slice(&bytes);
        Ok(())
    })
}
