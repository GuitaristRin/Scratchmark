//! 无窗口、无文件 I/O 的笔划状态；行为真源参 spec/M0.md。
mod brush;
mod codec;
mod document;

pub use brush::{BrushOptions, BrushPoint};
pub use document::Document;
pub const MAX_BATCH: usize = 65_536;
pub const MAX_POINTS: usize = 1_000_000;
pub const MAX_STROKES: usize = 100_000;
pub const MAX_SNAPSHOT: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Error {
    InvalidArgument = 1,
    InvalidHandle = 2,
    InvalidState = 3,
    BufferTooSmall = 4,
    StaleRevision = 5,
    InvalidData = 6,
    UnsupportedVersion = 7,
    LimitExceeded = 8,
    Internal = 9,
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub time_us: u64,
    pub pressure: f32,
    pub flags: u32,
}

impl Sample {
    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.x.abs() <= 1e9
            && self.y.abs() <= 1e9
            && self.pressure.is_finite()
            && match self.flags {
                0 => self.pressure == 0.0,
                1 => (0.0..=1.0).contains(&self.pressure),
                _ => false,
            }
    }
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct State {
    pub revision: u64,
    pub stroke_count: u32,
    pub active_points: u32,
    pub has_active: u32,
    pub can_undo: u32,
    pub can_redo: u32,
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct StrokeInfo {
    pub width: f64,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub color: u32,
    pub point_count: u32,
    pub active: u32,
    pub reserved: u32,
}

#[derive(Debug)]
pub(crate) struct Stroke {
    pub width: f64,
    pub color: u32,
    pub points: Vec<Sample>,
    pub bounds: Option<[f64; 4]>,
    pub brush_cache: Option<brush::BrushCache>,
}

impl Stroke {
    pub fn new(width: f64, color: u32) -> Result<Self> {
        if !width.is_finite() || width <= 0.0 || width > 1e6 {
            return Err(Error::InvalidArgument);
        }
        Ok(Self {
            width,
            color,
            points: Vec::new(),
            bounds: None,
            brush_cache: None,
        })
    }

    pub fn extend(&mut self, samples: &[Sample]) {
        if let Some(cache) = &mut self.brush_cache {
            cache.extend(samples, self.width);
        }
        for p in samples {
            let r = self.width / 2.0;
            let b = self
                .bounds
                .get_or_insert([p.x - r, p.y - r, p.x + r, p.y + r]);
            b[0] = b[0].min(p.x - r);
            b[1] = b[1].min(p.y - r);
            b[2] = b[2].max(p.x + r);
            b[3] = b[3].max(p.y + r);
        }
        self.points.extend_from_slice(samples);
    }

    pub fn brush_points(&mut self, options: BrushOptions) -> &[BrushPoint] {
        if self
            .brush_cache
            .as_ref()
            .is_none_or(|cache| cache.options != options)
        {
            self.brush_cache = None;
            let mut cache = brush::BrushCache {
                options,
                points: Vec::new(),
            };
            cache.extend(&self.points, self.width);
            self.brush_cache = Some(cache);
        }
        &self.brush_cache.as_ref().expect("cache initialized").points
    }
}
