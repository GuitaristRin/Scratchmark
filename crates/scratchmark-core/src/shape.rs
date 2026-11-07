//! 形状规整：把手写笔划拟合成直线 / 矩形 / 圆 / 箭头；行为真源参 spec/M8.md。
//!
//! 种类由宿主显式指定，引擎不做自动分类；`error` 是残差，采纳与否由宿主的阈值决定。
use crate::document::{Edit, ReplaceOp};
use crate::{Document, Error, MAX_COORD, MAX_POINTS, MAX_STROKES, Result, Sample, Stroke};

/// 箭头半角与相对杆长：固定常数，避免引擎里出现“看起来差不多”的可调启发式。
const ARROW_HALF_ANGLE: f64 = 28.0 * std::f64::consts::PI / 180.0;
const ARROW_HEAD_RATIO: f64 = 0.18;
/// 圆的固定采样段数：闭合回路共 65 点（末点等于首点）。
const CIRCLE_SEGMENTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ShapeKind {
    Line = 1,
    Rectangle = 2,
    Circle = 3,
    Arrow = 4,
}

impl ShapeKind {
    pub fn from_u32(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Line),
            2 => Ok(Self::Rectangle),
            3 => Ok(Self::Circle),
            4 => Ok(Self::Arrow),
            _ => Err(Error::InvalidArgument),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct ShapeInfo {
    pub kind: u32,
    pub stroke_count: u32,
    pub point_count: u32,
    pub reserved: u32,
    pub error: f64,
}

struct Plan {
    segments: Vec<Vec<(f64, f64)>>,
    error: f64,
}

impl Document {
    /// 规整单笔；`preview` 为真时只测量，不写文档、不写历史。
    pub fn regularize_stroke(
        &mut self,
        revision: u64,
        index: u32,
        kind: ShapeKind,
        preview: bool,
    ) -> Result<ShapeInfo> {
        if !preview {
            self.ensure_idle()?;
        }
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        let stroke = self
            .strokes
            .get(index as usize)
            .ok_or(Error::InvalidArgument)?;
        let plan = fit(stroke, kind)?;
        let point_count: usize = plan.segments.iter().map(Vec::len).sum();
        let info = ShapeInfo {
            kind: kind as u32,
            stroke_count: plan.segments.len() as u32,
            point_count: point_count as u32,
            reserved: 0,
            error: plan.error,
        };
        if preview {
            return Ok(info);
        }

        // 生成笔划：继承颜色与线宽，样本归零压力，时间在原始首末之间线性分布。
        let (time_from, time_to) = (
            stroke.points.first().map_or(0, |p| p.time_us),
            stroke.points.last().map_or(0, |p| p.time_us),
        );
        let mut batch = Vec::with_capacity(plan.segments.len());
        for segment in &plan.segments {
            let mut generated =
                Stroke::new(stroke.width, stroke.color).map_err(|_| Error::Internal)?;
            let last = segment.len().saturating_sub(1).max(1);
            let mut samples = Vec::with_capacity(segment.len());
            for (i, (x, y)) in segment.iter().enumerate() {
                let time = time_from
                    + ((time_to - time_from) as f64 * i as f64 / last as f64).round() as u64;
                samples.push(Sample {
                    x: *x,
                    y: *y,
                    time_us: time,
                    pressure: 0.0,
                    flags: 0,
                });
            }
            generated.extend(&samples, None);
            batch.push(generated);
        }

        if self.retained_points + point_count > MAX_POINTS
            || self.retained_strokes + batch.len() > MAX_STROKES
        {
            return Err(Error::LimitExceeded);
        }

        let at = index as usize;
        let after_count = batch.len();
        self.edit(move |d| {
            d.discard_redo();
            let before = std::mem::replace(&mut d.strokes[at], batch.remove(0));
            let ops = vec![ReplaceOp {
                index: at,
                after_count,
                before: Some(before),
                after: None,
            }];
            d.strokes.splice(at + 1..at + 1, batch);
            d.retained_points += point_count;
            d.retained_strokes += after_count;
            d.undo.push(Edit::Replace(ops));
            Ok(())
        })?;
        Ok(info)
    }
}

fn fit(stroke: &Stroke, kind: ShapeKind) -> Result<Plan> {
    match kind {
        ShapeKind::Line => fit_line(stroke),
        ShapeKind::Rectangle => fit_rectangle(stroke),
        ShapeKind::Circle => fit_circle(stroke),
        ShapeKind::Arrow => fit_arrow(stroke),
    }
}

fn centroid(stroke: &Stroke) -> (f64, f64) {
    let n = stroke.points.len().max(1) as f64;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for p in &stroke.points {
        cx += p.x;
        cy += p.y;
    }
    (cx / n, cy / n)
}

/// 质心 + 协方差主方向的轴线。
struct Axis {
    center: (f64, f64),
    direction: (f64, f64),
    min_t: f64,
    max_t: f64,
}

fn axis(stroke: &Stroke) -> Result<Axis> {
    if stroke.points.len() < 2 {
        return Err(Error::InvalidArgument);
    }
    let center = centroid(stroke);
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    let mut syy = 0.0;
    for p in &stroke.points {
        let dx = p.x - center.0;
        let dy = p.y - center.1;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    // 全部点重合：主方向无定义。
    if sxx <= 0.0 && syy <= 0.0 {
        return Err(Error::InvalidArgument);
    }
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let direction = (theta.cos(), theta.sin());
    let mut min_t = f64::INFINITY;
    let mut max_t = f64::NEG_INFINITY;
    for p in &stroke.points {
        let t = (p.x - center.0) * direction.0 + (p.y - center.1) * direction.1;
        min_t = min_t.min(t);
        max_t = max_t.max(t);
    }
    if max_t - min_t <= 0.0 {
        return Err(Error::InvalidArgument);
    }
    Ok(Axis {
        center,
        direction,
        min_t,
        max_t,
    })
}

fn line_rms(stroke: &Stroke, origin: (f64, f64), direction: (f64, f64)) -> f64 {
    let mut sum = 0.0;
    for p in &stroke.points {
        let dx = p.x - origin.0;
        let dy = p.y - origin.1;
        // 点到直线距离 = |叉积|
        let cross = dx * direction.1 - dy * direction.0;
        sum += cross * cross;
    }
    (sum / stroke.points.len() as f64).sqrt()
}

fn fit_line(stroke: &Stroke) -> Result<Plan> {
    let Axis {
        center,
        direction,
        min_t,
        max_t,
    } = axis(stroke)?;
    let start = (
        center.0 + direction.0 * min_t,
        center.1 + direction.1 * min_t,
    );
    let end = (
        center.0 + direction.0 * max_t,
        center.1 + direction.1 * max_t,
    );
    check_domain(&start)?;
    check_domain(&end)?;
    let error = line_rms(stroke, center, direction);
    Ok(Plan {
        segments: vec![vec![start, end]],
        error,
    })
}

fn fit_rectangle(stroke: &Stroke) -> Result<Plan> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for p in &stroke.points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    if max_x - min_x <= 0.0 || max_y - min_y <= 0.0 {
        return Err(Error::InvalidArgument);
    }
    let corners = [
        (min_x, min_y),
        (max_x, min_y),
        (max_x, max_y),
        (min_x, max_y),
    ];
    for corner in corners {
        check_domain(&corner)?;
    }
    let outline = [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ];
    let mut sum = 0.0;
    for p in &stroke.points {
        let mut best = f64::INFINITY;
        for (a, b) in outline {
            best = best.min(segment_distance_sq(p.x, p.y, a, b));
        }
        sum += best;
    }
    let error = (sum / stroke.points.len() as f64).sqrt();
    let mut closed = corners.to_vec();
    closed.push(corners[0]);
    Ok(Plan {
        segments: vec![closed],
        error,
    })
}

fn fit_circle(stroke: &Stroke) -> Result<Plan> {
    if stroke.points.len() < 3 {
        return Err(Error::InvalidArgument);
    }
    // 先平移到质心，避免 1e9 量级坐标让正规方程丢精度。
    let center = centroid(stroke);
    let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
    let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
    let (mut sxz, mut syz) = (0.0, 0.0);
    for p in &stroke.points {
        let x = p.x - center.0;
        let y = p.y - center.1;
        let z = x * x + y * y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
        sz += z;
        sxz += x * z;
        syz += y * z;
    }
    let n = stroke.points.len() as f64;
    // [Sxx Sxy Sx][A]   [Sxz]
    // [Sxy Syy Sy][B] = [Syz]
    // [Sx  Sy  n ][C]   [Sz ]
    let matrix = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let det = determinant(&matrix);
    let scale = (sxx + syy + n).max(1.0);
    if !det.is_finite() || det.abs() < 1e-12 * scale * scale * scale {
        return Err(Error::InvalidArgument);
    }
    let a = determinant(&column_replaced(&matrix, 0, [sxz, syz, sz])) / det;
    let b = determinant(&column_replaced(&matrix, 1, [sxz, syz, sz])) / det;
    let c = determinant(&column_replaced(&matrix, 2, [sxz, syz, sz])) / det;
    let cx = center.0 + a / 2.0;
    let cy = center.1 + b / 2.0;
    let radius_sq = c + (a * a + b * b) / 4.0;
    if !radius_sq.is_finite() || radius_sq <= 0.0 {
        return Err(Error::InvalidArgument);
    }
    let radius = radius_sq.sqrt();
    if !radius.is_finite() || !cx.is_finite() || !cy.is_finite() {
        return Err(Error::InvalidArgument);
    }

    let mut points = Vec::with_capacity(CIRCLE_SEGMENTS + 1);
    for step in 0..=CIRCLE_SEGMENTS {
        let angle = std::f64::consts::TAU * step as f64 / CIRCLE_SEGMENTS as f64;
        let point = (cx + radius * angle.cos(), cy + radius * angle.sin());
        check_domain(&point)?;
        points.push(point);
    }
    let mut sum = 0.0;
    for p in &stroke.points {
        let dx = p.x - cx;
        let dy = p.y - cy;
        let delta = (dx * dx + dy * dy).sqrt() - radius;
        sum += delta * delta;
    }
    let error = (sum / stroke.points.len() as f64).sqrt();
    Ok(Plan {
        segments: vec![points],
        error,
    })
}

fn fit_arrow(stroke: &Stroke) -> Result<Plan> {
    let Axis {
        center,
        direction,
        min_t,
        max_t,
    } = axis(stroke)?;
    let start = (
        center.0 + direction.0 * min_t,
        center.1 + direction.1 * min_t,
    );
    let end = (
        center.0 + direction.0 * max_t,
        center.1 + direction.1 * max_t,
    );
    let length = max_t - min_t;
    let head = ARROW_HEAD_RATIO * length;
    let cos = ARROW_HALF_ANGLE.cos();
    let sin = ARROW_HALF_ANGLE.sin();
    // 箭头两边：沿 -u 方向退回 head·cosα，再沿法向 ±head·sinα 张开。
    let left = (
        end.0 - direction.0 * head * cos - direction.1 * head * sin,
        end.1 - direction.1 * head * cos + direction.0 * head * sin,
    );
    let right = (
        end.0 - direction.0 * head * cos + direction.1 * head * sin,
        end.1 - direction.1 * head * cos - direction.0 * head * sin,
    );
    for point in [start, end, left, right] {
        check_domain(&point)?;
    }
    let error = line_rms(stroke, center, direction);
    Ok(Plan {
        segments: vec![vec![start, end], vec![end, left], vec![end, right]],
        error,
    })
}

fn check_domain(point: &(f64, f64)) -> Result<()> {
    if !point.0.is_finite()
        || !point.1.is_finite()
        || point.0.abs() > MAX_COORD
        || point.1.abs() > MAX_COORD
    {
        return Err(Error::InvalidArgument);
    }
    Ok(())
}

fn determinant(m: &[[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

/// Cramer 法则：把第 `column` 列换成右端项，用来解 3×3 线性方程。
fn column_replaced(m: &[[f64; 3]; 3], column: usize, rhs: [f64; 3]) -> [[f64; 3]; 3] {
    let mut replaced = *m;
    for (row, value) in rhs.iter().enumerate() {
        replaced[row][column] = *value;
    }
    replaced
}

fn segment_distance_sq(px: f64, py: f64, a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((px - a.0) * dx + (py - a.1) * dy) / length_sq).clamp(0.0, 1.0)
    };
    let cx = a.0 + t * dx;
    let cy = a.1 + t * dy;
    (px - cx) * (px - cx) + (py - cy) * (py - cy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(d: &mut Document) {
        d.begin(2.0, 0).unwrap();
        d.append(&[
            Sample {
                x: 0.0,
                y: 0.0,
                time_us: 0,
                pressure: 0.0,
                flags: 0,
            },
            Sample {
                x: 10.0,
                y: 0.0,
                time_us: 1,
                pressure: 0.0,
                flags: 0,
            },
        ])
        .unwrap();
        d.finish().unwrap();
    }

    /// 规整把生成的笔划留在实时、原笔划留在历史，预算必须两个方向都对。
    #[test]
    fn retained_budget_counts_generated_strokes() {
        let mut d = Document::default();
        line(&mut d);
        assert_eq!((d.retained_points, d.retained_strokes), (2, 1));

        d.regularize_stroke(d.state().revision, 0, ShapeKind::Arrow, false)
            .unwrap();
        // 实时：箭杆 2 + 两条头边 2+2 = 6 点 3 笔；历史：原手写 2 点 1 笔。
        assert_eq!((d.retained_points, d.retained_strokes), (8, 4));

        d.undo().unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (8, 4));
    }

    /// 预算超限必须在替换之前拦下：文档、revision 与历史都不能动。
    #[test]
    fn budget_overflow_is_rejected_before_mutation() {
        let mut d = Document::default();
        line(&mut d);
        let before = d.state();
        let undo_len = d.undo.len();

        d.retained_points = MAX_POINTS - 1;
        assert_eq!(
            d.regularize_stroke(before.revision, 0, ShapeKind::Arrow, false),
            Err(Error::LimitExceeded)
        );
        assert_eq!(d.state().revision, before.revision);
        assert_eq!(d.state().stroke_count, before.stroke_count);
        assert_eq!(d.undo.len(), undo_len);
        assert!(d.redo.is_empty());
    }
}
