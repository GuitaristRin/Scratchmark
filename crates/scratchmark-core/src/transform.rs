//! 选区仿射变换、复制与粘贴；行为真源参 spec/M7.md。
//!
//! 剪贴板不是引擎状态：复制产出格式 1 快照字节，粘贴再把字节解回来，
//! 因此宿主可以把同一份字节放进系统剪贴板、存盘或跨页使用。
use crate::codec;
use crate::document::{Edit, ReplaceOp};
use crate::{Document, Error, MAX_COORD, MAX_POINTS, MAX_STROKES, Result, Sample, Stroke};

/// 页面坐标到页面坐标的仿射变换：`x' = m00·x + m01·y + dx`，`y' = m10·x + m11·y + dy`。
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Transform {
    pub m00: f64,
    pub m01: f64,
    pub m10: f64,
    pub m11: f64,
    pub dx: f64,
    pub dy: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        m00: 1.0,
        m01: 0.0,
        m10: 0.0,
        m11: 1.0,
        dx: 0.0,
        dy: 0.0,
    };

    pub const fn determinant(&self) -> f64 {
        self.m00 * self.m11 - self.m01 * self.m10
    }

    pub fn translate(dx: f64, dy: f64) -> Self {
        Self {
            dx,
            dy,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            m00: sx,
            m11: sy,
            ..Self::IDENTITY
        }
    }

    /// 线宽只能用一个标量表达各向异性缩放，这里取等面积因子 `√|det|`。
    pub fn width_factor(&self) -> f64 {
        self.determinant().abs().sqrt()
    }

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.m00 * x + self.m01 * y + self.dx,
            self.m10 * x + self.m11 * y + self.dy,
        )
    }

    fn validate(&self) -> Result<()> {
        let finite = [self.m00, self.m01, self.m10, self.m11, self.dx, self.dy]
            .iter()
            .all(|value| value.is_finite());
        if !finite {
            return Err(Error::InvalidArgument);
        }
        let determinant = self.determinant();
        if !determinant.is_finite() || determinant.abs() < 1e-12 || determinant.abs() > 1e12 {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
}

fn map_stroke(stroke: &Stroke, transform: &Transform) -> Result<Stroke> {
    let mut mapped = Stroke::new(stroke.width * transform.width_factor(), stroke.color)
        .map_err(|_| Error::InvalidArgument)?;
    let mut samples = Vec::with_capacity(stroke.points.len());
    for point in &stroke.points {
        let (x, y) = transform.apply(point.x, point.y);
        if !x.is_finite() || !y.is_finite() || x.abs() > MAX_COORD || y.abs() > MAX_COORD {
            return Err(Error::InvalidArgument);
        }
        samples.push(Sample {
            x,
            y,
            time_us: point.time_us,
            pressure: point.pressure,
            flags: point.flags,
        });
    }
    mapped.extend(&samples);
    Ok(mapped)
}

impl Document {
    /// 原位变换选区：索引与 z 序不变，样本元数据不变，一次调用一个撤销条目。
    pub fn transform_strokes(
        &mut self,
        revision: u64,
        indices: &[u32],
        transform: &Transform,
    ) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        transform.validate()?;
        if indices.is_empty() {
            return Ok(());
        }
        let sorted = self.sorted_indices(indices)?;

        // 先全部算完再写入：任一点越界都让整次调用不生效。
        let mut replacements = Vec::with_capacity(sorted.len());
        for &index in &sorted {
            replacements.push((index, map_stroke(&self.strokes[index], transform)?));
        }
        // 变换把副本留在历史里：实时笔划点数不变，但“含历史在内”的预算要加上这份副本。
        let added_points: usize = replacements
            .iter()
            .map(|(_, stroke)| stroke.points.len())
            .sum();
        let added_strokes = replacements.len();

        self.edit(move |d| {
            d.discard_redo();
            let mut ops = Vec::with_capacity(replacements.len());
            for (index, mapped) in replacements {
                let before = std::mem::replace(&mut d.strokes[index], mapped);
                ops.push(ReplaceOp {
                    index,
                    after_count: 1,
                    before: Some(before),
                    after: None,
                });
            }
            d.retained_points += added_points;
            d.retained_strokes += added_strokes;
            d.undo.push(Edit::Replace(ops));
            Ok(())
        })
    }

    /// 导出选区为格式 1 快照字节；只读，不要求空闲。
    pub fn copy_strokes(&self, revision: u64, indices: &[u32]) -> Result<Vec<u8>> {
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        let sorted = self.sorted_indices(indices)?;
        let selected: Vec<&Stroke> = sorted.iter().map(|&index| &self.strokes[index]).collect();
        Ok(codec::encode_format1(&selected))
    }

    /// 粘贴格式 1 快照字节：追加到最上层，一次调用一个撤销条目。
    pub fn paste(
        &mut self,
        revision: u64,
        data: &[u8],
        transform: Option<&Transform>,
    ) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        let transform = transform.copied().unwrap_or(Transform::IDENTITY);
        transform.validate()?;

        let decoded = codec::decode(data)?;
        // 粘贴不是“恢复文档”：只接受复制产出的格式 1，避免把整份带页面标识的文档塞进当前页。
        if decoded.format != 1 {
            return Err(Error::InvalidData);
        }
        if decoded.strokes.is_empty() {
            return Ok(());
        }

        let mut added_points = 0;
        let mut batch = Vec::with_capacity(decoded.strokes.len());
        for stroke in &decoded.strokes {
            let mapped = map_stroke(stroke, &transform)?;
            added_points += mapped.points.len();
            batch.push(mapped);
        }
        let added_strokes = batch.len();
        if self.retained_points + added_points > MAX_POINTS
            || self.retained_strokes + added_strokes > MAX_STROKES
        {
            return Err(Error::LimitExceeded);
        }

        let index = self.strokes.len();
        let count = batch.len();
        self.edit(move |d| {
            d.discard_redo();
            d.strokes.extend(batch);
            d.retained_points += added_points;
            d.retained_strokes += added_strokes;
            d.undo.push(Edit::Insert {
                index,
                count,
                strokes: None,
            });
            Ok(())
        })
    }
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

    /// 原位变换把副本留在历史里，两个方向的计数都必须跟着走，否则下一次编辑会下溢。
    #[test]
    fn retained_budget_tracks_transform_copies() {
        let mut d = Document::default();
        line(&mut d);
        assert_eq!((d.retained_points, d.retained_strokes), (2, 1));

        d.transform_strokes(d.state().revision, &[0], &Transform::translate(1.0, 0.0))
            .unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (4, 2));

        d.undo().unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (4, 2));

        // 丢弃重做条目后只剩存活的那一份。
        d.transform_strokes(d.state().revision, &[0], &Transform::translate(2.0, 0.0))
            .unwrap();
        assert_eq!((d.retained_points, d.retained_strokes), (4, 2));
        d.begin(2.0, 0).unwrap();
        d.append(&[Sample {
            x: 50.0,
            y: 0.0,
            time_us: 0,
            pressure: 0.0,
            flags: 0,
        }])
        .unwrap();
        d.finish().unwrap();
        // 存活 3 点（2+1）+ 历史中上一次变换的副本 2 点 = 5；笔划 2 + 1 = 3。
        assert_eq!((d.retained_points, d.retained_strokes), (5, 3));
    }

    /// 粘贴的预算检查必须在写入之前：超限时文档、revision 与历史都不动。
    #[test]
    fn paste_budget_overflow_is_rejected_before_mutation() {
        let mut d = Document::default();
        line(&mut d);
        let before = d.state();
        let blob = d.copy_strokes(before.revision, &[0]).unwrap();
        let undo_len = d.undo.len();

        // 故障注入：不构造百万点文档，直接把计数器顶到上限。
        d.retained_points = MAX_POINTS - 1;
        assert_eq!(
            d.paste(before.revision, &blob, None),
            Err(Error::LimitExceeded)
        );
        assert_eq!(d.state().revision, before.revision);
        assert_eq!(d.state().stroke_count, before.stroke_count);
        assert_eq!(d.undo.len(), undo_len);
        assert!(d.redo.is_empty());
        assert_eq!(d.strokes.len(), 1);
    }
}
