use crate::{Document, Error, MAX_POINTS, MAX_SNAPSHOT, MAX_STROKES, Result, Sample, Stroke};

const MAGIC: &[u8; 8] = b"SCRATCH\0";

impl Document {
    pub fn save(&self) -> Result<Vec<u8>> {
        self.ensure_idle()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(self.strokes.len() as u32).to_le_bytes());
        for s in &self.strokes {
            bytes.extend_from_slice(&s.width.to_le_bytes());
            bytes.extend_from_slice(&s.color.to_le_bytes());
            bytes.extend_from_slice(&(s.points.len() as u32).to_le_bytes());
            for p in &s.points {
                bytes.extend_from_slice(&p.x.to_le_bytes());
                bytes.extend_from_slice(&p.y.to_le_bytes());
                bytes.extend_from_slice(&p.time_us.to_le_bytes());
                bytes.extend_from_slice(&p.pressure.to_le_bytes());
                bytes.extend_from_slice(&p.flags.to_le_bytes());
            }
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
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take()?))
    }
}

pub(crate) fn decode(bytes: &[u8]) -> Result<(Vec<Stroke>, usize)> {
    if bytes.len() > MAX_SNAPSHOT {
        return Err(Error::LimitExceeded);
    }
    let mut r = Reader(bytes);
    if &r.take::<8>()? != MAGIC {
        return Err(Error::InvalidData);
    }
    if r.u32()? != 1 {
        return Err(Error::UnsupportedVersion);
    }
    let count = r.u32()? as usize;
    if count > MAX_STROKES {
        return Err(Error::LimitExceeded);
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
    Ok((strokes, total))
}
