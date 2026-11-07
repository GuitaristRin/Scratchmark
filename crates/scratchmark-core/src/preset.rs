//! 笔型尺寸预置：`(笔型, 档位) → 归一化比例` 与换算成实际线宽；行为真源参 spec/M13.md。
//!
//! 数值是本引擎自有的默认分档，不照抄既有基线的调参值（理由见 spec/M13.md）；
//! 预置表不进快照、不影响任何几何判定，宿主可以完全无视它。
use crate::{Error, Result};

/// 每个笔型的档位数量。
pub const BRUSH_LEVELS: u32 = 5;
/// 笔宽上限，与 `Stroke::new` 一致。
const MAX_WIDTH: f64 = 1e6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum BrushKind {
    Pencil = 1,
    Marker = 2,
    Ballpen = 3,
    Pen = 4,
    Eraser = 5,
}

/// 各笔型自档位 1 到 5 的归一化比例；同一行内严格递增。
const LADDERS: [[f64; BRUSH_LEVELS as usize]; 5] = [
    [0.08, 0.16, 0.26, 0.40, 0.60],
    [0.12, 0.24, 0.38, 0.55, 0.75],
    [0.06, 0.12, 0.20, 0.30, 0.45],
    [0.10, 0.20, 0.32, 0.46, 0.62],
    [0.08, 0.13, 0.20, 0.29, 0.40],
];

impl BrushKind {
    pub fn from_u32(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::Pencil),
            2 => Ok(Self::Marker),
            3 => Ok(Self::Ballpen),
            4 => Ok(Self::Pen),
            5 => Ok(Self::Eraser),
            _ => Err(Error::InvalidArgument),
        }
    }

    fn ladder(self) -> &'static [f64; BRUSH_LEVELS as usize] {
        &LADDERS[(self as usize) - 1]
    }

    /// 归一化比例，落在 `(0, 1]`。
    pub fn ratio(self, level: u32) -> Result<f64> {
        if level == 0 || level > BRUSH_LEVELS {
            return Err(Error::InvalidArgument);
        }
        Ok(self.ladder()[(level - 1) as usize])
    }

    /// 档位换算成实际线宽：`max_width × ratio`。
    pub fn width(self, level: u32, max_width: f64) -> Result<f64> {
        if !max_width.is_finite() || max_width <= 0.0 || max_width > MAX_WIDTH {
            return Err(Error::InvalidArgument);
        }
        let width = max_width * self.ratio(level)?;
        if !width.is_finite() || width <= 0.0 || width > MAX_WIDTH {
            return Err(Error::InvalidArgument);
        }
        Ok(width)
    }
}
