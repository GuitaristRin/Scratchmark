use crate::{Document, Error, MAX_POINTS, MAX_SNAPSHOT, MAX_STROKES, Result, Sample, Stroke};

const MAGIC: &[u8; 8] = b"SCRATCH\0";
// 格式 2 用独立魔数而非版本递增：旧魔数仅接受版本 1，未知版本仍被既有断言拒绝。
const MAGIC_V2: &[u8; 8] = b"SCRATCH2";

fn write_header(bytes: &mut Vec<u8>, magic: &[u8; 8], version: u32, strokes: usize) {
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&(strokes as u32).to_le_bytes());
}

pub(crate) fn write_stroke(bytes: &mut Vec<u8>, stroke: &Stroke) {
    bytes.extend_from_slice(&stroke.width.to_le_bytes());
    bytes.extend_from_slice(&stroke.color.to_le_bytes());
    bytes.extend_from_slice(&(stroke.points.len() as u32).to_le_bytes());
    for p in &stroke.points {
        bytes.extend_from_slice(&p.x.to_le_bytes());
        bytes.extend_from_slice(&p.y.to_le_bytes());
        bytes.extend_from_slice(&p.time_us.to_le_bytes());
        bytes.extend_from_slice(&p.pressure.to_le_bytes());
        bytes.extend_from_slice(&p.flags.to_le_bytes());
    }
}

/// 选区复制用的编码：始终是格式 1（无页面标识），宿主可把它当剪贴板字节跨页使用。
pub(crate) fn encode_format1(strokes: &[&Stroke]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_header(&mut bytes, MAGIC, 1, strokes.len());
    for stroke in strokes {
        write_stroke(&mut bytes, stroke);
    }
    bytes
}

impl Document {
    pub fn save(&self) -> Result<Vec<u8>> {
        self.ensure_idle()?;
        let mut bytes = Vec::new();
        if self.page_id == 0 {
            write_header(&mut bytes, MAGIC, 1, self.strokes.len());
        } else {
            write_header(&mut bytes, MAGIC_V2, 2, self.strokes.len());
            bytes.extend_from_slice(&self.page_id.to_le_bytes());
        }
        for stroke in &self.strokes {
            write_stroke(&mut bytes, stroke);
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
}

pub(crate) struct Decoded {
    pub strokes: Vec<Stroke>,
    pub total: usize,
    pub page_id: u64,
    /// 1 或 2；粘贴需要区分“文档恢复”与“剪贴板粘贴”。
    pub format: u32,
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Decoded> {
    if bytes.len() > MAX_SNAPSHOT {
        return Err(Error::LimitExceeded);
    }
    let mut r = Reader(bytes);
    let magic = r.take::<8>()?;
    let mut page_id = 0u64;
    let format;
    if magic == *MAGIC {
        if r.u32()? != 1 {
            return Err(Error::UnsupportedVersion);
        }
        format = 1;
    } else if magic == *MAGIC_V2 {
        if r.u32()? != 2 {
            return Err(Error::UnsupportedVersion);
        }
        format = 2;
    } else {
        return Err(Error::InvalidData);
    }
    let count = r.u32()? as usize;
    if count > MAX_STROKES {
        return Err(Error::LimitExceeded);
    }
    if format == 2 {
        page_id = r.u64()?;
    }
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
        if n > r.0.len() / 32 {
            return Err(Error::InvalidData);
        }
        let mut s = Stroke::new(width, color).map_err(|_| Error::InvalidData)?;
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
            s.extend(&[p]);
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
