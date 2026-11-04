use crate::{
    BrushOptions, BrushPoint, Error, MAX_BATCH, MAX_POINTS, MAX_STROKES, Result, Sample, State,
    Stroke, StrokeInfo,
};

#[derive(Debug)]
pub struct Document {
    pub(crate) strokes: Vec<Stroke>,
    pub(crate) active: Option<Stroke>,
    pub(crate) redo: Vec<Stroke>,
    pub(crate) retained_points: usize,
    pub(crate) revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            active: None,
            redo: Vec::new(),
            retained_points: 0,
            revision: 1,
        }
    }
}

impl Document {
    fn edit(&mut self, operation: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        let next = self.revision.checked_add(1).ok_or(Error::LimitExceeded)?;
        operation(self)?;
        self.revision = next;
        Ok(())
    }

    pub fn state(&self) -> State {
        State {
            revision: self.revision,
            stroke_count: self.strokes.len() as u32,
            active_points: self.active.as_ref().map_or(0, |s| s.points.len() as u32),
            has_active: u32::from(self.active.is_some()),
            can_undo: u32::from(self.active.is_none() && !self.strokes.is_empty()),
            can_redo: u32::from(self.active.is_none() && !self.redo.is_empty()),
            reserved: 0,
        }
    }

    pub fn begin(&mut self, width: f64, color: u32) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            if d.strokes.len() + d.redo.len() >= MAX_STROKES {
                return Err(Error::LimitExceeded);
            }
            d.active = Some(Stroke::new(width, color)?);
            Ok(())
        })
    }

    pub fn append(&mut self, samples: &[Sample]) -> Result<()> {
        let active = self.active.as_ref().ok_or(Error::InvalidState)?;
        if samples.len() > MAX_BATCH || self.retained_points + samples.len() > MAX_POINTS {
            return Err(Error::LimitExceeded);
        }
        let mut last = active.points.last().map(|p| p.time_us);
        for p in samples {
            if !p.is_valid() || last.is_some_and(|t| p.time_us < t) {
                return Err(Error::InvalidArgument);
            }
            last = Some(p.time_us);
        }
        if samples.is_empty() {
            return Ok(());
        }
        self.edit(|d| {
            d.active
                .as_mut()
                .ok_or(Error::InvalidState)?
                .extend(samples);
            d.retained_points += samples.len();
            Ok(())
        })
    }

    pub fn finish(&mut self) -> Result<()> {
        self.edit(|d| {
            if d.active.as_ref().is_none_or(|s| s.points.is_empty()) {
                return Err(Error::InvalidState);
            }
            let stroke = d.active.take().ok_or(Error::InvalidState)?;
            d.retained_points -= d.redo.iter().map(|s| s.points.len()).sum::<usize>();
            d.redo.clear();
            d.strokes.push(stroke);
            Ok(())
        })
    }

    pub fn cancel(&mut self) -> Result<()> {
        self.edit(|d| {
            let stroke = d.active.take().ok_or(Error::InvalidState)?;
            d.retained_points -= stroke.points.len();
            Ok(())
        })
    }

    pub fn undo(&mut self) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            d.redo.push(d.strokes.pop().ok_or(Error::InvalidState)?);
            Ok(())
        })
    }

    pub fn redo(&mut self) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            d.strokes.push(d.redo.pop().ok_or(Error::InvalidState)?);
            Ok(())
        })
    }

    pub(crate) fn ensure_idle(&self) -> Result<()> {
        if self.active.is_some() {
            Err(Error::InvalidState)
        } else {
            Ok(())
        }
    }

    fn stroke(&self, revision: u64, index: u32) -> Result<(&Stroke, bool)> {
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if let Some(s) = self.strokes.get(index as usize) {
            return Ok((s, false));
        }
        if index as usize == self.strokes.len() {
            if let Some(s) = &self.active {
                return Ok((s, true));
            }
        }
        Err(Error::InvalidArgument)
    }

    pub fn stroke_info(&self, revision: u64, index: u32) -> Result<StrokeInfo> {
        let (s, active) = self.stroke(revision, index)?;
        let [min_x, min_y, max_x, max_y] = s.bounds.unwrap_or([0.0; 4]);
        Ok(StrokeInfo {
            width: s.width,
            color: s.color,
            point_count: s.points.len() as u32,
            active: u32::from(active),
            min_x,
            min_y,
            max_x,
            max_y,
            reserved: 0,
        })
    }

    pub fn read_points(
        &self,
        revision: u64,
        index: u32,
        start: u32,
        output: &mut [Sample],
    ) -> Result<usize> {
        let (s, _) = self.stroke(revision, index)?;
        let points = s
            .points
            .get(start as usize..)
            .ok_or(Error::InvalidArgument)?;
        let n = points.len().min(output.len());
        output[..n].copy_from_slice(&points[..n]);
        Ok(n)
    }

    pub fn read_brush_points(
        &mut self,
        revision: u64,
        index: u32,
        start: u32,
        options: BrushOptions,
        output: &mut [BrushPoint],
    ) -> Result<usize> {
        options.validate()?;
        let count = self.stroke(revision, index)?.0.points.len();
        if start as usize > count {
            return Err(Error::InvalidArgument);
        }
        let n = output.len().min(count - start as usize);
        if n == 0 {
            return Ok(0);
        }
        let stroke = if (index as usize) < self.strokes.len() {
            &mut self.strokes[index as usize]
        } else {
            self.active.as_mut().ok_or(Error::InvalidArgument)?
        };
        let points = stroke.brush_points(options);
        output[..n].copy_from_slice(&points[start as usize..start as usize + n]);
        Ok(n)
    }

    pub fn load(&mut self, bytes: &[u8]) -> Result<()> {
        self.ensure_idle()?;
        let (strokes, retained_points) = crate::codec::decode(bytes)?;
        self.edit(|d| {
            d.strokes = strokes;
            d.retained_points = retained_points;
            d.redo.clear();
            Ok(())
        })
    }
}
