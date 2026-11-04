//! 页面坐标与视口坐标的纯数学映射；行为真源参 spec/M3.md。
use crate::{Error, MAX_COORD, Result};

/// 页面点；视口点共用此结构，方向由函数名标明。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// 页面局部边界；scale > 0 时角序在映射后保持。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Viewport {
    pub scale: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub reserved: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            reserved: 0,
        }
    }
}

impl Viewport {
    pub fn validate(self) -> Result<()> {
        if !self.scale.is_finite()
            || self.scale <= 0.0
            || !self.offset_x.is_finite()
            || !self.offset_y.is_finite()
            || self.reserved != 0
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }

    pub fn page_to_viewport(self, page: Point) -> Result<Point> {
        self.validate()?;
        page_value(page.x)?;
        page_value(page.y)?;
        Ok(Point {
            x: finite(page.x * self.scale + self.offset_x)?,
            y: finite(page.y * self.scale + self.offset_y)?,
        })
    }

    pub fn viewport_to_page(self, viewport: Point) -> Result<Point> {
        self.validate()?;
        finite(viewport.x)?;
        finite(viewport.y)?;
        Ok(Point {
            x: page_value((viewport.x - self.offset_x) / self.scale)?,
            y: page_value((viewport.y - self.offset_y) / self.scale)?,
        })
    }

    pub fn width_page_to_viewport(self, width: f64) -> Result<f64> {
        self.validate()?;
        positive(width)?;
        finite(width * self.scale)
    }

    pub fn width_viewport_to_page(self, width: f64) -> Result<f64> {
        self.validate()?;
        positive(width)?;
        finite(width / self.scale)
    }

    pub fn bounds_page_to_viewport(self, bounds: Bounds) -> Result<Bounds> {
        self.validate()?;
        page_value(bounds.min_x)?;
        page_value(bounds.min_y)?;
        page_value(bounds.max_x)?;
        page_value(bounds.max_y)?;
        Ok(Bounds {
            min_x: finite(bounds.min_x * self.scale + self.offset_x)?,
            min_y: finite(bounds.min_y * self.scale + self.offset_y)?,
            max_x: finite(bounds.max_x * self.scale + self.offset_x)?,
            max_y: finite(bounds.max_y * self.scale + self.offset_y)?,
        })
    }
}

fn finite(value: f64) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidArgument)
    }
}

// 页面域与 Sample 相同；变换输出越界时宿主将无法 append，这里提前拒绝。
fn page_value(value: f64) -> Result<f64> {
    if value.is_finite() && value.abs() <= MAX_COORD {
        Ok(value)
    } else {
        Err(Error::InvalidArgument)
    }
}

fn positive(value: f64) -> Result<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(Error::InvalidArgument)
    }
}
