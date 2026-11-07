//! 沿擦除路径切分笔划；行为真源参 spec/M5.md。
//!
//! 删除集合是“到擦除折线距离不超过半径”的闭区域，判定只用原始中心线（与 M4 命中一致）。
//! 切点是逐段二次方程的闭式解，不采样、不迭代，保证同一输入得到同一输出。
use crate::document::{Edit, ReplaceOp};
use crate::{
    Document, Error, FLAG_CUT_END, FLAG_PRESSURE, MAX_COORD, MAX_POINTS, MAX_STROKES, Point,
    Result, Sample, SampleAxes, Stroke,
};

/// 参数空间（段内 t）的忽略阈值：它以下的覆盖视为相切而非切分，
/// 避免生成零长度片段与重复顶点；页面尺度上不可见。
const CUT_EPS: f64 = 1e-12;
/// 判断二次项是否退化的一元判据（距离平方的二次项恒非负，只可能是数值噪声）。
const QUAD_EPS: f64 = 1e-12;

impl Document {
    /// 按擦除路径删除笔划被覆盖的部分；一次调用只产生一个撤销条目。
    pub fn erase_path(&mut self, revision: u64, path: &[Point], radius: f64) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if path.is_empty()
            || path.iter().any(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || p.x.abs() > MAX_COORD
                    || p.y.abs() > MAX_COORD
            })
            || !radius.is_finite()
            || radius <= 0.0
            || radius > MAX_COORD
        {
            return Err(Error::InvalidArgument);
        }

        let path_bounds = path_bounds(path);
        // 逐段的包围盒只算一次，供每个笔划段快速排除远处的擦除段；
        // 没有它，长路径 × 多点笔划是 O(点数 × 路径点数) 的纯平方开销。
        let segments = erase_segments(path);
        let radius_sq = radius * radius;
        let mut plans: Vec<Option<Vec<Stroke>>> = Vec::with_capacity(self.strokes.len());
        let mut affected = false;
        for stroke in &self.strokes {
            let plan = cut_stroke(stroke, &segments, path_bounds, radius, radius_sq)?;
            affected |= plan.is_some();
            plans.push(plan);
        }
        // 空手势不改 revision、不记历史：否则一次点空的擦除会污染撤销栈。
        if !affected {
            return Ok(());
        }

        // 预算含历史在内：切分同时保留原笔划（撤销用）与片段，两者都计入。
        let added_points: usize = plans
            .iter()
            .flatten()
            .flatten()
            .map(|stroke| stroke.points.len())
            .sum();
        let added_strokes: usize = plans.iter().flatten().map(Vec::len).sum();
        if self.retained_points + added_points > MAX_POINTS
            || self.retained_strokes + added_strokes > MAX_STROKES
        {
            return Err(Error::LimitExceeded);
        }

        self.edit(move |d| {
            d.discard_redo();
            let mut next = Vec::with_capacity(d.strokes.len() + added_strokes);
            let mut ops = Vec::new();
            for (stroke, plan) in std::mem::take(&mut d.strokes).into_iter().zip(plans) {
                match plan {
                    None => next.push(stroke),
                    Some(fragments) => {
                        // 片段占据原索引，其余笔划相对顺序不变，z 序因此保持。
                        let index = next.len();
                        let after_count = fragments.len();
                        next.extend(fragments);
                        ops.push(ReplaceOp {
                            index,
                            after_count,
                            before: Some(stroke),
                            after: None,
                        });
                    }
                }
            }
            d.strokes = next;
            d.retained_points += added_points;
            d.retained_strokes += added_strokes;
            d.undo.push(Edit::Replace(ops));
            Ok(())
        })
    }
}

fn path_bounds(path: &[Point]) -> [f64; 4] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in path {
        bounds[0] = bounds[0].min(p.x);
        bounds[1] = bounds[1].min(p.y);
        bounds[2] = bounds[2].max(p.x);
        bounds[3] = bounds[3].max(p.y);
    }
    bounds
}

/// 擦除路径的一段及其包围盒；单点路径退化为一个零长度段（用作点擦除）。
struct EraseSegment {
    a: Point,
    b: Point,
    bounds: [f64; 4],
}

fn erase_segments(path: &[Point]) -> Vec<EraseSegment> {
    let mut segments: Vec<EraseSegment> = Vec::with_capacity(path.len().max(1));
    if path.len() == 1 {
        let a = path[0];
        segments.push(EraseSegment {
            a,
            b: a,
            bounds: [a.x, a.y, a.x, a.y],
        });
        return segments;
    }
    for pair in path.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        segments.push(EraseSegment {
            a,
            b,
            bounds: [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)],
        });
    }
    segments
}

/// 返回 `None` 表示完全不受影响；`Some(vec![])` 表示整笔被擦掉。
fn cut_stroke(
    stroke: &Stroke,
    path: &[EraseSegment],
    path_bounds: [f64; 4],
    radius: f64,
    radius_sq: f64,
) -> Result<Option<Vec<Stroke>>> {
    let points = &stroke.points;
    let Some(bounds) = stroke.bounds else {
        return Ok(None);
    };
    if disjoint(bounds, path_bounds, radius) {
        return Ok(None);
    }
    // 单点笔划没有线段可切：整体落在删除集合内就被擦掉，否则原样保留。
    if points.len() == 1 {
        let p = &points[0];
        return Ok(if distance_to_path_sq(p.x, p.y, path) <= radius_sq {
            Some(Vec::new())
        } else {
            None
        });
    }

    // 逐段求覆盖区间；covered[i] 已合并、裁剪到 [0, 1]，只保留有长度的区间。
    let mut covered: Vec<Vec<(f64, f64)>> = Vec::with_capacity(points.len() - 1);
    let mut any_cover = false;
    for pair in points.windows(2) {
        // 段级包围盒排除：长文档里绝大多数段离擦除路径很远，先廉价跳过。
        let a = &pair[0];
        let b = &pair[1];
        let segment = [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)];
        if disjoint(segment, path_bounds, radius) {
            covered.push(Vec::new());
            continue;
        }
        let intervals = segment_covers(a, b, path, radius, segment);
        any_cover |= !intervals.is_empty();
        covered.push(intervals);
    }
    if !any_cover {
        return Ok(None);
    }

    let axes = &stroke.axes;
    let mut fragments: Vec<Vec<(Sample, SampleAxes)>> = Vec::new();
    let mut current: Vec<(Sample, SampleAxes)> = Vec::new();
    for (i, intervals) in covered.iter().enumerate() {
        let a = &points[i];
        let b = &points[i + 1];
        let mut pieces = Vec::with_capacity(intervals.len() + 1);
        let mut cursor = 0.0;
        for &(lo, hi) in intervals {
            if lo > cursor {
                pieces.push((cursor, lo));
            }
            cursor = cursor.max(hi);
        }
        if cursor < 1.0 {
            pieces.push((cursor, 1.0));
        }
        let mut open_at_end = false;
        for (lo, hi) in pieces {
            // 端点吸附：让顶点两侧的存活判定一致，不产生 1e-12 量级的残段。
            let lo = if lo <= CUT_EPS { 0.0 } else { lo };
            let hi = if hi >= 1.0 - CUT_EPS { 1.0 } else { hi };
            if hi <= lo {
                continue;
            }
            if lo > 0.0 {
                flush(&mut fragments, &mut current);
                current.push(lerp_pair(a, axes[i], b, axes[i + 1], lo));
            } else if current.is_empty() {
                current.push((*a, axes[i]));
            }
            if hi < 1.0 {
                current.push(lerp_pair(a, axes[i], b, axes[i + 1], hi));
                flush(&mut fragments, &mut current);
            } else {
                open_at_end = true;
                // 整段保留时必须把本段终点并入当前片段：否则只有片段首尾两点进来，
                // 中间所有顶点都会被丢掉，片段就退化成一条直线弦（M17 之前的老毛病）。
                current.push((*b, axes[i + 1]));
                if i + 1 == points.len() - 1 {
                    // 末段的终点就是笔划终点，直接收尾。
                    flush(&mut fragments, &mut current);
                }
            }
        }
        if !open_at_end {
            flush(&mut fragments, &mut current);
        }
    }
    flush(&mut fragments, &mut current);

    let mut built = Vec::with_capacity(fragments.len());
    for pairs in fragments {
        let mut fragment = Stroke::new(stroke.width, stroke.color).map_err(|_| Error::Internal)?;
        let samples: Vec<Sample> = pairs.iter().map(|(sample, _)| *sample).collect();
        let fragment_axes: Vec<SampleAxes> = pairs.iter().map(|(_, axis)| *axis).collect();
        fragment.extend(&samples, Some(&fragment_axes));
        built.push(fragment);
    }
    Ok(Some(built))
}

fn flush(fragments: &mut Vec<Vec<(Sample, SampleAxes)>>, current: &mut Vec<(Sample, SampleAxes)>) {
    if current.len() >= 2 {
        fragments.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

/// 切点样本与轴向一起插值：轴向按两侧样本线性插值（角度不做环绕处理，仍在值域内）。
fn lerp_pair(
    a: &Sample,
    a_axes: SampleAxes,
    b: &Sample,
    b_axes: SampleAxes,
    t: f64,
) -> (Sample, SampleAxes) {
    let sample = lerp_sample(a, b, t);
    let blend = t as f32;
    let axes = SampleAxes {
        tilt: a_axes.tilt + (b_axes.tilt - a_axes.tilt) * blend,
        orientation: a_axes.orientation + (b_axes.orientation - a_axes.orientation) * blend,
    };
    (sample, axes)
}

fn disjoint(stroke: [f64; 4], path: [f64; 4], radius: f64) -> bool {
    stroke[0] > path[2] + radius
        || stroke[2] < path[0] - radius
        || stroke[1] > path[3] + radius
        || stroke[3] < path[1] - radius
}

fn lerp_sample(a: &Sample, b: &Sample, t: f64) -> Sample {
    let time = a.time_us as f64 + (b.time_us as f64 - a.time_us as f64) * t;
    let time_us = if time <= a.time_us as f64 {
        a.time_us
    } else if time >= b.time_us as f64 {
        b.time_us
    } else {
        time.round() as u64
    };
    // bit0 未置位时 pressure 必须为 0；只有两端都带真实压力时才插值压力。
    let (pressure, carry) = if a.flags & FLAG_PRESSURE != 0 && b.flags & FLAG_PRESSURE != 0 {
        (
            a.pressure + (b.pressure - a.pressure) * t as f32,
            FLAG_PRESSURE,
        )
    } else {
        (0.0, 0)
    };
    // 切出来的端点带 bit1：宿主据此把这一端画成圆缺口（参 spec/M17.md）。
    let flags = carry | FLAG_CUT_END;
    Sample {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        time_us,
        pressure,
        flags,
    }
}
fn distance_to_path_sq(x: f64, y: f64, path: &[EraseSegment]) -> f64 {
    let mut best = f64::INFINITY;
    for segment in path {
        best = best.min(point_segment_distance_sq(x, y, segment.a, segment.b));
    }
    best
}

fn point_segment_distance_sq(x: f64, y: f64, e0: Point, e1: Point) -> f64 {
    let dx = e1.x - e0.x;
    let dy = e1.y - e0.y;
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((x - e0.x) * dx + (y - e0.y) * dy) / length_sq).clamp(0.0, 1.0)
    };
    let px = e0.x + t * dx;
    let py = e0.y + t * dy;
    (x - px) * (x - px) + (y - py) * (y - py)
}

/// 单个笔划段上的覆盖区间集合（合并、裁剪、去掉零长度）。
fn segment_covers(
    a: &Sample,
    b: &Sample,
    path: &[EraseSegment],
    radius: f64,
    segment: [f64; 4],
) -> Vec<(f64, f64)> {
    let mut intervals: Vec<(f64, f64)> = Vec::new();
    for erase in path {
        // 逐段包围盒排除：斜向路径上大多数擦除段离本段很远，先跳过再解方程。
        // 距离 ≤ 半径意味着交点同时落在两个（其中一个已扩张半径的）包围盒内。
        if disjoint(segment, erase.bounds, radius) {
            continue;
        }
        if let Some(interval) = segment_cover(a, b, erase.a, erase.b, radius * radius) {
            intervals.push(interval);
        }
    }
    merge(intervals)
}

fn merge(mut intervals: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    intervals.sort_by(|l, r| l.0.total_cmp(&r.0));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(intervals.len());
    for (lo, hi) in intervals {
        let lo = lo.max(0.0);
        let hi = hi.min(1.0);
        if hi - lo <= 0.0 {
            continue;
        }
        match merged.last_mut() {
            Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    merged
}

/// 点到单条擦除线段的“距离平方 ≤ r²”在笔划段参数 t 上的解。
///
/// 点到凸集的距离是凸函数，与线性参数化复合后仍凸，因此解集必为单个闭区间；
/// 这里按最近点落在擦除段的前端点 / 内部 / 后端点三个区域分别解二次方程，
/// 三个区域的并取外包区间即可。
fn segment_cover(
    a: &Sample,
    b: &Sample,
    e0: Point,
    e1: Point,
    radius_sq: f64,
) -> Option<(f64, f64)> {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let vv = vx * vx + vy * vy;
    let dx = e1.x - e0.x;
    let dy = e1.y - e0.y;
    let ll = dx * dx + dy * dy;

    // 退化笔划段：整段就是同一个点，要么全擦要么全留。
    if vv == 0.0 {
        let d = point_segment_distance_sq(a.x, a.y, e0, e1);
        return (d <= radius_sq).then_some((0.0, 1.0));
    }

    let wx = a.x - e0.x;
    let wy = a.y - e0.y;
    let w = wx * wx + wy * wy;

    // 擦除段退化为点：最近点固定为 e0。
    if ll == 0.0 {
        let c = vx * wx + vy * wy;
        let interval = quadratic_interval(vv, 2.0 * c, w - radius_sq)?;
        return intersect(interval, (0.0, 1.0));
    }

    let c = vx * wx + vy * wy;
    let u0 = (wx * dx + wy * dy) / ll;
    let uv = (vx * dx + vy * dy) / ll;
    let mut spans: Vec<(f64, f64)> = Vec::new();

    // 区域 A：最近点是 e0（u ≤ 0）。
    if let Some(region) = region_where(u0, uv, f64::NEG_INFINITY, 0.0) {
        if let Some(interval) = quadratic_interval(vv, 2.0 * c, w - radius_sq) {
            if let Some(hit) = intersect(region, interval) {
                spans.push(hit);
            }
        }
    }
    // 区域 B：最近点在 e1（u ≥ 1）。
    let wx1 = a.x - e1.x;
    let wy1 = a.y - e1.y;
    if let Some(region) = region_where(u0, uv, 1.0, f64::INFINITY) {
        let c1 = vx * wx1 + vy * wy1;
        let w1 = wx1 * wx1 + wy1 * wy1;
        if let Some(interval) = quadratic_interval(vv, 2.0 * c1, w1 - radius_sq) {
            if let Some(hit) = intersect(region, interval) {
                spans.push(hit);
            }
        }
    }
    // 区域 C：最近点在 e0 + u·d（0 ≤ u ≤ 1），距离平方为二次式减投影项。
    if let Some(region) = region_where(u0, uv, 0.0, 1.0) {
        let qa = (vv - ll * uv * uv).max(0.0);
        let qb = 2.0 * (c - ll * u0 * uv);
        let qc = w - ll * u0 * u0;
        if let Some(interval) = quadratic_interval(qa, qb, qc - radius_sq) {
            if let Some(hit) = intersect(region, interval) {
                spans.push(hit);
            }
        }
    }
    if spans.is_empty() {
        return None;
    }
    let lo = spans.iter().fold(f64::INFINITY, |acc, s| acc.min(s.0));
    let hi = spans.iter().fold(f64::NEG_INFINITY, |acc, s| acc.max(s.1));
    intersect((lo, hi), (0.0, 1.0))
}

/// `u0 + uv·t` 落在 [lo, hi] 的 t 范围；uv = 0 时按常数处理。
fn region_where(u0: f64, uv: f64, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if uv == 0.0 {
        return (lo <= u0 && u0 <= hi).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let t = |u: f64| (u - u0) / uv;
    let (a, b) = (t(lo), t(hi));
    Some((a.min(b), a.max(b)))
}

fn intersect(l: (f64, f64), r: (f64, f64)) -> Option<(f64, f64)> {
    let lo = l.0.max(r.0);
    let hi = l.1.min(r.1);
    (hi > lo).then_some((lo, hi))
}

/// 解 `a·t² + b·t + c ≤ 0`（a ≥ 0）。
fn quadratic_interval(a: f64, b: f64, c: f64) -> Option<(f64, f64)> {
    if a <= QUAD_EPS {
        if b.abs() <= QUAD_EPS {
            return (c <= 0.0).then_some((f64::NEG_INFINITY, f64::INFINITY));
        }
        let t = -c / b;
        return Some(if b > 0.0 {
            (f64::NEG_INFINITY, t)
        } else {
            (t, f64::INFINITY)
        });
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return None;
    }
    let root = disc.sqrt();
    let t0 = (-b - root) / (2.0 * a);
    let t1 = (-b + root) / (2.0 * a);
    Some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(x: f64, y: f64) -> Sample {
        Sample {
            x,
            y,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        }
    }

    fn line(d: &mut Document) {
        d.begin(2.0, 0).unwrap();
        d.append(&[sample(0.0, 0.0), sample(10.0, 0.0)]).unwrap();
        d.finish().unwrap();
    }

    /// 预算按“含历史在内”统计：切分同时保留切分前的原笔划（供撤销）与片段。
    #[test]
    fn retained_budget_tracks_both_copies_and_releases_on_redo_discard() {
        let mut d = Document::default();
        line(&mut d);
        assert_eq!((d.retained_points, d.retained_strokes), (2, 1));

        let revision = d.state().revision;
        d.erase_path(revision, &[Point { x: 5.0, y: 0.0 }], 2.0)
            .unwrap();
        // 两段各 2 点（4）+ 撤销历史中的原笔划（2）。
        assert_eq!((d.retained_points, d.retained_strokes), (6, 3));

        d.undo().unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (6, 3));

        // 新编辑丢弃重做条目，释放它持有的片段。
        d.begin(2.0, 0).unwrap();
        d.append(&[sample(50.0, 0.0)]).unwrap();
        d.finish().unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (3, 2));
    }

    /// 预算检查必须发生在变更之前：超限时文档、revision 与历史都不能动。
    #[test]
    fn retained_budget_overflow_is_rejected_before_mutation() {
        let mut d = Document::default();
        line(&mut d);
        let before = d.state();
        // 故障注入：不构造百万点文献，直接把计数器顶到上限附近，
        // 以验证上限确实在变更前被检查，而不是事后截断。
        d.retained_points = MAX_POINTS - 1;
        let revision = d.state().revision;
        let undo_len = d.undo.len();

        assert_eq!(
            d.erase_path(revision, &[Point { x: 5.0, y: 0.0 }], 2.0),
            Err(Error::LimitExceeded)
        );
        assert_eq!(d.state().revision, before.revision);
        assert_eq!(d.state().stroke_count, before.stroke_count);
        assert_eq!(d.state().can_undo, before.can_undo);
        assert_eq!(d.strokes.len(), 1);
        assert_eq!(d.strokes[0].points.len(), 2);
        assert_eq!(d.undo.len(), undo_len);
        assert!(d.redo.is_empty());
    }
}
