//! 按笔宽的点间距重采样；行为真源参 spec/M14.md。
//!
//! 只读派生：沿中心线按弧长等距取点（间距 = 笔宽 × 比例），末尾保留原始末点，
//! 并给出该位置按 M2 计算的逐点笔宽。不改原始样本、不进快照、不参与命中判定。
use crate::{BrushOptions, BrushPoint, Document, Error, MAX_POINTS, Result};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct ResampleOptions {
    pub spacing_ratio: f64,
    pub brush: BrushOptions,
    pub reserved: u32,
}

impl ResampleOptions {
    fn validate(&self) -> Result<()> {
        if !self.spacing_ratio.is_finite() || self.spacing_ratio <= 0.0 || self.reserved != 0 {
            return Err(Error::InvalidArgument);
        }
        self.brush.validate()
    }
}

impl Document {
    /// 重采样一笔；返回从首点到末点的等距点列（含末点）。
    pub fn resample_stroke(
        &mut self,
        revision: u64,
        index: u32,
        options: &ResampleOptions,
    ) -> Result<Vec<BrushPoint>> {
        options.validate()?;
        let (width, count) = {
            let (stroke, _) = self.stroke(revision, index)?;
            (stroke.width, stroke.points.len())
        };
        let spacing = width * options.spacing_ratio;
        if !spacing.is_finite() || spacing <= 0.0 {
            return Err(Error::InvalidArgument);
        }
        let brush = read_brush(self, revision, index, options.brush)?;

        // 单点或空笔划：只有一个位置。
        if count <= 1 {
            return Ok(brush.into_iter().take(1).collect());
        }

        let (samples, points) = {
            let (stroke, _) = self.stroke(revision, index)?;
            (stroke.points.clone(), stroke.points.len())
        };

        // 位置序列：0、spacing、2·spacing…，再补上终点（若终点不在步点上）。
        let mut positions = Vec::new();
        let mut total: f64 = 0.0;
        let mut lengths = Vec::with_capacity(points.saturating_sub(1));
        for pair in samples.windows(2) {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let length = (dx * dx + dy * dy).sqrt();
            lengths.push(length);
            total += length;
        }
        if total > 0.0 {
            let steps = (total / spacing).floor();
            let mut step: f64 = 0.0;
            while step <= steps {
                positions.push(step * spacing);
                step += 1.0;
                if positions.len() > MAX_POINTS {
                    return Err(Error::LimitExceeded);
                }
            }
            let last = *positions.last().unwrap_or(&0.0);
            if total - last > 1e-9 {
                positions.push(total);
            }
        } else {
            // 所有点重合：只有一个位置。
            positions.push(0.0);
        }

        // 沿弧长走一遍，把位置映射成坐标与笔宽。
        let mut output = Vec::with_capacity(positions.len());
        let mut segment = 0usize;
        let mut segment_start = 0.0;
        for position in positions {
            while segment + 1 < points - 1 && segment_start + lengths[segment] < position {
                segment_start += lengths[segment];
                segment += 1;
            }
            let length = lengths.get(segment).copied().unwrap_or(0.0);
            let t = if length > 0.0 {
                ((position - segment_start) / length).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let a = &samples[segment];
            let b = &samples[(segment + 1).min(samples.len() - 1)];
            let x = a.x + (b.x - a.x) * t;
            let y = a.y + (b.y - a.y) * t;
            let (wa, wb) = match (brush.get(segment), brush.get(segment + 1)) {
                (Some(a), Some(b)) => (a.width, b.width),
                (Some(a), None) => (a.width, a.width),
                _ => (width, width),
            };
            output.push(BrushPoint {
                x,
                y,
                width: wa + (wb - wa) * t,
            });
        }
        Ok(output)
    }
}

fn read_brush(
    document: &mut Document,
    revision: u64,
    index: u32,
    brush: BrushOptions,
) -> Result<Vec<BrushPoint>> {
    let count = {
        let (stroke, _) = document.stroke(revision, index)?;
        stroke.points.len()
    };
    let mut output = vec![BrushPoint::default(); count];
    let written = document.read_brush_points(revision, index, 0, brush, &mut output)?;
    output.truncate(written);
    Ok(output)
}
