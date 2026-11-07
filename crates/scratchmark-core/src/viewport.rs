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
        positive(width * self.scale)
    }

    pub fn width_viewport_to_page(self, width: f64) -> Result<f64> {
        self.validate()?;
        positive(width)?;
        positive(width / self.scale)
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

/// 带旋转的视口：先缩放、再旋转、最后平移。行为真源参 spec/M9.md。
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct ViewportEx {
    pub scale: f64,
    pub angle: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub reserved: u32,
}

impl Default for ViewportEx {
    fn default() -> Self {
        Self {
            scale: 1.0,
            angle: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            reserved: 0,
        }
    }
}

impl ViewportEx {
    pub fn validate(self) -> Result<()> {
        if !self.scale.is_finite()
            || self.scale <= 0.0
            || !self.angle.is_finite()
            || !self.offset_x.is_finite()
            || !self.offset_y.is_finite()
            || self.reserved != 0
        {
            return Err(Error::InvalidArgument);
        }
        Ok(())
    }

    fn cos_sin(self) -> Result<(f64, f64)> {
        self.validate()?;
        let (sin, cos) = self.angle.sin_cos();
        finite(cos)?;
        finite(sin)?;
        Ok((cos, sin))
    }

    pub fn page_to_viewport(self, page: Point) -> Result<Point> {
        let (cos, sin) = self.cos_sin()?;
        page_value(page.x)?;
        page_value(page.y)?;
        Ok(Point {
            x: finite(self.scale * (cos * page.x - sin * page.y) + self.offset_x)?,
            y: finite(self.scale * (sin * page.x + cos * page.y) + self.offset_y)?,
        })
    }

    pub fn viewport_to_page(self, viewport: Point) -> Result<Point> {
        let (cos, sin) = self.cos_sin()?;
        finite(viewport.x)?;
        finite(viewport.y)?;
        let dx = (viewport.x - self.offset_x) / self.scale;
        let dy = (viewport.y - self.offset_y) / self.scale;
        Ok(Point {
            x: page_value(cos * dx + sin * dy)?,
            y: page_value(-sin * dx + cos * dy)?,
        })
    }

    pub fn width_page_to_viewport(self, width: f64) -> Result<f64> {
        self.validate()?;
        positive(width)?;
        positive(width * self.scale)
    }

    pub fn width_viewport_to_page(self, width: f64) -> Result<f64> {
        self.validate()?;
        positive(width)?;
        positive(width / self.scale)
    }

    /// 旋转后取轴对齐外接框，因此结果通常比原矩形大。
    pub fn bounds_page_to_viewport(self, bounds: Bounds) -> Result<Bounds> {
        let (cos, sin) = self.cos_sin()?;
        let corners = [
            (bounds.min_x, bounds.min_y),
            (bounds.max_x, bounds.min_y),
            (bounds.max_x, bounds.max_y),
            (bounds.min_x, bounds.max_y),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in corners {
            page_value(x)?;
            page_value(y)?;
            let vx = finite(self.scale * (cos * x - sin * y) + self.offset_x)?;
            let vy = finite(self.scale * (sin * x + cos * y) + self.offset_y)?;
            min_x = min_x.min(vx);
            min_y = min_y.min(vy);
            max_x = max_x.max(vx);
            max_y = max_y.max(vy);
        }
        Ok(Bounds {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    /// 以视口点为锚缩放：锚点下的页面点保持不变。
    pub fn zoom_at(self, anchor: Point, factor: f64) -> Result<Self> {
        self.validate()?;
        finite(anchor.x)?;
        finite(anchor.y)?;
        positive(factor)?;
        let scale = positive(self.scale * factor)?;
        Ok(Self {
            scale,
            angle: self.angle,
            offset_x: finite(anchor.x - (anchor.x - self.offset_x) * factor)?,
            offset_y: finite(anchor.y - (anchor.y - self.offset_y) * factor)?,
            reserved: 0,
        })
    }

    /// 让页面边界适配视口矩形：按旋转后的外接框等比缩放并居中。
    pub fn fit(
        page: Bounds,
        view_width: f64,
        view_height: f64,
        angle: f64,
        padding: f64,
    ) -> Result<Self> {
        page_value(page.min_x)?;
        page_value(page.min_y)?;
        page_value(page.max_x)?;
        page_value(page.max_y)?;
        if page.min_x > page.max_x || page.min_y > page.max_y {
            return Err(Error::InvalidArgument);
        }
        positive(view_width)?;
        positive(view_height)?;
        finite(angle)?;
        if !padding.is_finite() || padding < 0.0 {
            return Err(Error::InvalidArgument);
        }
        let usable_width = view_width - 2.0 * padding;
        let usable_height = view_height - 2.0 * padding;
        positive(usable_width)?;
        positive(usable_height)?;

        let (sin, cos) = angle.sin_cos();
        // 旋转后页面矩形的外接框（未缩放）。
        let half_x = (page.max_x - page.min_x) / 2.0;
        let half_y = (page.max_y - page.min_y) / 2.0;
        let box_width = 2.0 * (cos.abs() * half_x + sin.abs() * half_y);
        let box_height = 2.0 * (sin.abs() * half_x + cos.abs() * half_y);
        positive(box_width)?;
        positive(box_height)?;

        let scale = positive((usable_width / box_width).min(usable_height / box_height))?;
        let center_x = (page.min_x + page.max_x) / 2.0;
        let center_y = (page.min_y + page.max_y) / 2.0;
        let rotated_x = scale * (cos * center_x - sin * center_y);
        let rotated_y = scale * (sin * center_x + cos * center_y);
        Ok(Self {
            scale,
            angle,
            offset_x: finite(view_width / 2.0 - rotated_x)?,
            offset_y: finite(view_height / 2.0 - rotated_y)?,
            reserved: 0,
        })
    }
}
