use crate::{Error, Result, Sample};

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct BrushOptions {
    pub alpha: f64,
    pub min_width_ratio: f64,
    pub pressure_enabled: u32,
    pub reserved: u32,
}

impl Default for BrushOptions {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            min_width_ratio: 1.0,
            pressure_enabled: 0,
            reserved: 0,
        }
    }
}

impl BrushOptions {
    pub fn validate(self) -> Result<()> {
        if !self.alpha.is_finite()
            || self.alpha <= 0.0
            || self.alpha > 1.0
            || !self.min_width_ratio.is_finite()
            || self.min_width_ratio <= 0.0
            || self.min_width_ratio > 1.0
            || self.pressure_enabled > 1
            || self.reserved != 0
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct BrushPoint {
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

#[derive(Debug)]
pub(crate) struct BrushCache {
    pub options: BrushOptions,
    pub points: Vec<BrushPoint>,
}

impl BrushCache {
    pub fn extend(&mut self, samples: &[Sample], max_width: f64) {
        for p in samples {
            let mut result = BrushPoint {
                x: p.x,
                y: p.y,
                width: max_width,
            };
            if self.options.pressure_enabled == 1 && p.flags == 1 {
                let ratio = self.options.min_width_ratio;
                // 正线宽下溢仍保持可表示的正数，不扩大到超过原笔宽。
                result.width = (max_width * (ratio + (1.0 - ratio) * f64::from(p.pressure)))
                    .max(f64::from_bits(1));
            }
            if self.options.alpha != 1.0 {
                if let Some(previous) = self.points.last() {
                    result.x = previous.x + self.options.alpha * (p.x - previous.x);
                    result.y = previous.y + self.options.alpha * (p.y - previous.y);
                }
            }
            self.points.push(result);
        }
    }
}
