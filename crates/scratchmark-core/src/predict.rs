//! 预测笔迹：按活动笔划已收到的样本外推一小段；行为真源参 spec/M10.md。
//!
//! 预测不是文档内容：不写文档、不进快照、不改 revision，也不参与命中与编辑。
use crate::{BrushOptions, BrushPoint, Document, Error, MAX_COORD, Result};

/// 单次预测最多返回的点数。
pub const MAX_PREDICTION_POINTS: u32 = 64;
/// 速度估计使用的尾段样本数上限。
const TAIL: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct PredictionOptions {
    pub horizon_us: f64,
    pub max_distance: f64,
    pub count: u32,
    pub reserved: u32,
    /// 与 M2 相同的笔刷配置：预测宽度必须和已绘制部分一致。
    pub brush: BrushOptions,
}

impl PredictionOptions {
    fn validate(&self) -> Result<()> {
        if !self.horizon_us.is_finite()
            || self.horizon_us <= 0.0
            || !self.max_distance.is_finite()
            || self.max_distance <= 0.0
            || self.count == 0
            || self.count > MAX_PREDICTION_POINTS
            || self.reserved != 0
        {
            return Err(Error::InvalidArgument);
        }
        self.brush.validate()
    }
}

impl Document {
    /// 只读预测：返回从笔尖向前、升序的笔刷点；没有可预测的活动笔划时返回空。
    pub fn predict(
        &mut self,
        revision: u64,
        options: &PredictionOptions,
    ) -> Result<Vec<BrushPoint>> {
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        options.validate()?;
        let Some(active) = self.active.as_mut() else {
            return Ok(Vec::new());
        };
        if active.points.len() < 2 {
            return Ok(Vec::new());
        }

        let points = &active.points;
        let last = points[points.len() - 1];
        let previous = points[points.len() - 2];
        let tail_first = points[points.len().saturating_sub(TAIL)];
        // 尾段跨度不为正时无法估计速度：宁可不预测。
        let tail_span = last.time_us.saturating_sub(tail_first.time_us) as f64;
        if tail_span <= 0.0 {
            return Ok(Vec::new());
        }
        let tail_vx = (last.x - tail_first.x) / tail_span;
        let tail_vy = (last.y - tail_first.y) / tail_span;
        let pair_span = last.time_us.saturating_sub(previous.time_us) as f64;
        let (pair_vx, pair_vy) = if pair_span > 0.0 {
            (
                (last.x - previous.x) / pair_span,
                (last.y - previous.y) / pair_span,
            )
        } else {
            (tail_vx, tail_vy)
        };
        // 固定对半合成，不引入可调参数。
        let velocity_x = 0.5 * (tail_vx + pair_vx);
        let velocity_y = 0.5 * (tail_vy + pair_vy);
        if !velocity_x.is_finite() || !velocity_y.is_finite() {
            return Ok(Vec::new());
        }

        let travel_x = velocity_x * options.horizon_us;
        let travel_y = velocity_y * options.horizon_us;
        let length = (travel_x * travel_x + travel_y * travel_y).sqrt();
        let shrink = if length > options.max_distance {
            options.max_distance / length
        } else {
            1.0
        };
        let count = options.count as f64;
        let step_x = travel_x * shrink / count;
        let step_y = travel_y * shrink / count;

        let fallback_width = active.width;
        let computed_width = active
            .brush_points(options.brush)
            .last()
            .map(|point| point.width);
        let width = computed_width.unwrap_or(fallback_width);

        let mut predicted = Vec::with_capacity(options.count as usize);
        for step in 1..=options.count {
            let x = last.x + step_x * step as f64;
            let y = last.y + step_y * step as f64;
            // 越出页面域就在该点截断：预测是提示，宁可少画。
            if !x.is_finite() || !y.is_finite() || x.abs() > MAX_COORD || y.abs() > MAX_COORD {
                break;
            }
            predicted.push(BrushPoint { x, y, width });
        }
        Ok(predicted)
    }
}
