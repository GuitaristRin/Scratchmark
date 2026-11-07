use crate::{
    Document, Error, MAX_POINTS, MAX_SNAPSHOT, MAX_STROKES, Result, Sample, SampleAxes, Stroke,
};

const MAGIC: &[u8; 8] = b"SCRATCH\0";
// 格式 2 用独立魔数而非版本递增：旧魔数仅接受版本 1，未知版本仍被既有断言拒绝。
const MAGIC_V2: &[u8; 8] = b"SCRATCH2";
// 格式 3 在格式 2 之上给每个样本追加倾斜与朝向（各 4 字节），用于携带轴向通道。
const MAGIC_V3: &[u8; 8] = b"SCRATCH3";

fn write_header(bytes: &mut Vec<u8>, magic: &[u8; 8], version: u32, strokes: usize) {
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(strokes as u32).to_le_bytes());
}

/// 写入一笔；`with_axes` 为真时每个样本后追加 tilt / orientation。
pub(crate) fn write_stroke(bytes: &mut Vec<u8>, stroke: &Stroke, with_axes: bool) {
    bytes.extend_from_slice(&stroke.width.to_le_bytes());
    bytes.extend_from_slice(&stroke.color.to_le_bytes());
    bytes.extend_from_slice(&(stroke.points.len() as u32).to_le_bytes());
    for (index, p) in stroke.points.iter().enumerate() {
        bytes.extend_from_slice(&p.x.to_le_bytes());
        bytes.extend_from_slice(&p.y.to_le_bytes());
        bytes.extend_from_slice(&p.time_us.to_le_bytes());
        bytes.extend_from_slice(&p.pressure.to_le_bytes());
        bytes.extend_from_slice(&p.flags.to_le_bytes());
        if with_axes {
            let axes = stroke
                .axes
                .get(index)
                .copied()
                .unwrap_or_else(SampleAxes::default);
            bytes.extend_from_slice(&axes.tilt.to_le_bytes());
            bytes.extend_from_slice(&axes.orientation.to_le_bytes());
        }
    }
}

fn has_axes(stroke: &Stroke) -> bool {
    stroke.axes.iter().any(|axes| !axes.is_default())
}

/// 选区复制用的编码：页面标识恒为 0；笔划带轴向时用格式 3，否则仍是格式 1。
pub(crate) fn encode_copy(strokes: &[&Stroke]) -> Vec<u8> {
    let with_axes = strokes.iter().any(|stroke| has_axes(stroke));
    let mut bytes = Vec::new();
    if with_axes {
        write_header(&mut bytes, MAGIC_V3, 3, strokes.len());
        bytes.extend_from_slice(&0u64.to_le_bytes());
    } else {
        write_header(&mut bytes, MAGIC, 1, strokes.len());
    }
    for stroke in strokes {
        write_stroke(&mut bytes, stroke, with_axes);
    }
    bytes
}

impl Document {
    pub fn save(&self) -> Result<Vec<u8>> {
        self.ensure_idle()?;
        // 只有真正带轴向的文档才升到格式 3，避免无谓地让旧宿主读不了新文件。
        let with_axes = self.strokes.iter().any(has_axes);
        let mut bytes = Vec::new();
        if with_axes {
            write_header(&mut bytes, MAGIC_V3, 3, self.strokes.len());
            bytes.extend_from_slice(&self.page_id.to_le_bytes());
        } else if self.page_id != 0 {
            write_header(&mut bytes, MAGIC_V2, 2, self.strokes.len());
            bytes.extend_from_slice(&self.page_id.to_le_bytes());
        } else {
            write_header(&mut bytes, MAGIC, 1, self.strokes.len());
        }
        for stroke in &self.strokes {
            write_stroke(&mut bytes, stroke, with_axes);
        }
        Ok(bytes)
    }
}

struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let value = self.0.get(..N).ok_or(Error::InvalidData)?;
        let value = value.try_into().map_err(|_| Error::InvalidData)?;
        self.0 = &self.0[N..];
        Ok(value)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take()?))
    }
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

pub(crate) struct Decoded {
    pub strokes: Vec<Stroke>,
    pub total: usize,
    pub page_id: u64,
    /// 1 / 2 / 3；粘贴需要区分“文档恢复”与“剪贴板粘贴”。
    pub format: u32,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Decoded> {
    if bytes.len() > MAX_SNAPSHOT {
        return Err(Error::LimitExceeded);
    }
    let mut r = Reader(bytes);
    let magic = r.take::<8>()?;
    let format = if magic == *MAGIC {
        1
    } else if magic == *MAGIC_V2 {
        2
    } else if magic == *MAGIC_V3 {
        3
    } else {
        return Err(Error::InvalidData);
    };
    if r.u32()? != format {
        return Err(Error::UnsupportedVersion);
    }
    let count = r.u32()? as usize;
    if count > MAX_STROKES {
        return Err(Error::LimitExceeded);
    }
    let page_id = if format >= 2 { r.u64()? } else { 0 };
    let stride = if format >= 3 { 40 } else { 32 };
    let mut strokes = Vec::new();
    let mut total = 0;
    for _ in 0..count {
        let width = r.f64()?;
        let color = r.u32()?;
        let n = r.u32()? as usize;
        if n == 0 {
            return Err(Error::InvalidData);
        }
        if n > MAX_POINTS - total {
            return Err(Error::LimitExceeded);
        }
        if n > r.0.len() / stride {
            return Err(Error::InvalidData);
        }
        let mut s = Stroke::new(width, color).map_err(|_| Error::InvalidData)?;
        let mut samples = Vec::with_capacity(n);
        let mut axes = Vec::with_capacity(n);
        let mut last = 0;
        for _ in 0..n {
            let p = Sample {
                x: r.f64()?,
                y: r.f64()?,
                time_us: u64::from_le_bytes(r.take()?),
                pressure: f32::from_le_bytes(r.take()?),
                flags: r.u32()?,
            };
            if !p.is_valid() || p.time_us < last {
                return Err(Error::InvalidData);
            }
            last = p.time_us;
            samples.push(p);
            if format >= 3 {
                let value = SampleAxes {
                    tilt: r.f32()?,
                    orientation: r.f32()?,
                };
                if !value.is_valid() {
                    return Err(Error::InvalidData);
                }
                axes.push(value);
            }
        }
        if format >= 3 {
            s.extend(&samples, Some(&axes));
        } else {
            s.extend(&samples, None);
        }
        total += n;
        strokes.push(s);
    }
    if !r.0.is_empty() {
        return Err(Error::InvalidData);
    }
    Ok(Decoded {
        strokes,
        total,
        page_id,
        format,
    })
}
