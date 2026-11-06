use crate::{
    BrushOptions, BrushPoint, Error, MAX_BATCH, MAX_POINTS, MAX_STROKES, Result, Sample, State,
    Stroke, StrokeInfo,
};

/// 一次“按下标替换”：`index` 是**施加时刻**的索引，`before` / `after` 恰好一份存活。
///
/// 局部擦除把一笔切成多笔，无法用“尾插 / 尾删”表达，因此历史必须记住位置与两侧内容。
#[derive(Debug)]
pub(crate) struct ReplaceOp {
    pub index: usize,
    pub after_count: usize,
    pub before: Option<Stroke>,
    pub after: Option<Vec<Stroke>>,
}

#[derive(Debug)]
pub(crate) enum Edit {
    Add(Option<Stroke>),
    Erase {
        index: usize,
        stroke: Option<Stroke>,
    },
    Replace(Vec<ReplaceOp>),
}

impl Edit {
    /// 该历史条目当前持有的点数与笔划数（用于“含历史在内”的预算）。
    pub(crate) fn retained(&self) -> (usize, usize) {
        match self {
            Edit::Add(Some(stroke))
            | Edit::Erase {
                stroke: Some(stroke),
                ..
            } => (stroke.points.len(), 1),
            Edit::Add(None) | Edit::Erase { stroke: None, .. } => (0, 0),
            Edit::Replace(ops) => ops.iter().fold((0, 0), |(points, strokes), op| {
                let before = op.before.as_ref().map_or((0, 0), |s| (s.points.len(), 1));
                let after = op.after.as_ref().map_or((0, 0), |list| {
                    (
                        list.iter().map(|s| s.points.len()).sum::<usize>(),
                        list.len(),
                    )
                });
                (points + before.0 + after.0, strokes + before.1 + after.1)
            }),
        }
    }
}

#[derive(Debug)]
pub struct Document {
    pub(crate) strokes: Vec<Stroke>,
    pub(crate) active: Option<Stroke>,
    pub(crate) undo: Vec<Edit>,
    pub(crate) redo: Vec<Edit>,
    pub(crate) retained_points: usize,
    pub(crate) retained_strokes: usize,
    pub(crate) revision: u64,
    pub(crate) page_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            active: None,
            undo: Vec::new(),
            redo: Vec::new(),
            retained_points: 0,
            retained_strokes: 0,
            revision: 1,
            page_id: 0,
        }
    }
}

impl Document {
    pub(crate) fn edit(&mut self, operation: impl FnOnce(&mut Self) -> Result<()>) -> Result<()> {
        let next = self.revision.checked_add(1).ok_or(Error::LimitExceeded)?;
        operation(self)?;
        self.revision = next;
        Ok(())
    }

    pub(crate) fn discard_redo(&mut self) {
        for edit in self.redo.drain(..) {
            let (points, strokes) = edit.retained();
            self.retained_points -= points;
            self.retained_strokes -= strokes;
        }
    }

    pub fn state(&self) -> State {
        State {
            revision: self.revision,
            stroke_count: self.strokes.len() as u32,
            active_points: self.active.as_ref().map_or(0, |s| s.points.len() as u32),
            has_active: u32::from(self.active.is_some()),
            can_undo: u32::from(self.active.is_none() && !self.undo.is_empty()),
            can_redo: u32::from(self.active.is_none() && !self.redo.is_empty()),
            reserved: 0,
        }
    }

    pub fn begin(&mut self, width: f64, color: u32) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            if d.retained_strokes >= MAX_STROKES {
                return Err(Error::LimitExceeded);
            }
            d.active = Some(Stroke::new(width, color)?);
            d.retained_strokes += 1;
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
            d.discard_redo();
            d.strokes.push(stroke);
            d.undo.push(Edit::Add(None));
            Ok(())
        })
    }

    pub fn cancel(&mut self) -> Result<()> {
        self.edit(|d| {
            let stroke = d.active.take().ok_or(Error::InvalidState)?;
            d.retained_points -= stroke.points.len();
            d.retained_strokes -= 1;
            Ok(())
        })
    }

    pub fn undo(&mut self) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            let edit = d.undo.pop().ok_or(Error::InvalidState)?;
            match edit {
                Edit::Add(stroke) => {
                    debug_assert!(stroke.is_none());
                    let stroke = d.strokes.pop().ok_or(Error::Internal)?;
                    d.redo.push(Edit::Add(Some(stroke)));
                }
                Edit::Erase { index, stroke } => {
                    let stroke = stroke.ok_or(Error::Internal)?;
                    if index > d.strokes.len() {
                        return Err(Error::Internal);
                    }
                    d.strokes.insert(index, stroke);
                    d.redo.push(Edit::Erase {
                        index,
                        stroke: None,
                    });
                }
                Edit::Replace(mut ops) => {
                    // 逆序回放：每个 index 只有在“该条目刚施加完”的状态下才成立。
                    for op in ops.iter_mut().rev() {
                        let before = op.before.take().ok_or(Error::Internal)?;
                        if op.index + op.after_count > d.strokes.len() {
                            return Err(Error::Internal);
                        }
                        let after: Vec<Stroke> = d
                            .strokes
                            .drain(op.index..op.index + op.after_count)
                            .collect();
                        d.strokes.insert(op.index, before);
                        op.after = Some(after);
                    }
                    d.redo.push(Edit::Replace(ops));
                }
            }
            Ok(())
        })
    }

    pub fn redo(&mut self) -> Result<()> {
        self.edit(|d| {
            d.ensure_idle()?;
            let edit = d.redo.pop().ok_or(Error::InvalidState)?;
            match edit {
                Edit::Add(stroke) => {
                    let stroke = stroke.ok_or(Error::Internal)?;
                    d.strokes.push(stroke);
                    d.undo.push(Edit::Add(None));
                }
                Edit::Erase { index, stroke } => {
                    debug_assert!(stroke.is_none());
                    if index >= d.strokes.len() {
                        return Err(Error::Internal);
                    }
                    let stroke = d.strokes.remove(index);
                    d.undo.push(Edit::Erase {
                        index,
                        stroke: Some(stroke),
                    });
                }
                Edit::Replace(mut ops) => {
                    for op in ops.iter_mut() {
                        let after = op.after.take().ok_or(Error::Internal)?;
                        if op.index >= d.strokes.len() {
                            return Err(Error::Internal);
                        }
                        let before = d.strokes.remove(op.index);
                        op.after_count = after.len();
                        d.strokes.splice(op.index..op.index, after);
                        op.before = Some(before);
                    }
                    d.undo.push(Edit::Replace(ops));
                }
            }
            Ok(())
        })
    }

    pub fn erase_stroke(&mut self, revision: u64, index: u32) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if index as usize >= self.strokes.len() {
            return Err(Error::InvalidArgument);
        }
        self.edit(|d| {
            let stroke = d.strokes.remove(index as usize);
            d.discard_redo();
            d.undo.push(Edit::Erase {
                index: index as usize,
                stroke: Some(stroke),
            });
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

    pub fn page_id(&self) -> u64 {
        self.page_id
    }

    pub fn set_page_id(&mut self, id: u64) -> Result<()> {
        self.ensure_idle()?;
        if self.page_id == id {
            return Ok(());
        }
        self.edit(|d| {
            d.page_id = id;
            Ok(())
        })
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
        let decoded = crate::codec::decode(bytes)?;
        if self.page_id != 0 && decoded.page_id != 0 && self.page_id != decoded.page_id {
            return Err(Error::InvalidData);
        }
        self.edit(|d| {
            d.strokes = decoded.strokes;
            d.retained_points = decoded.total;
            d.retained_strokes = d.strokes.len();
            d.page_id = decoded.page_id;
            d.undo.clear();
            d.undo.extend((0..d.strokes.len()).map(|_| Edit::Add(None)));
            d.redo.clear();
            Ok(())
        })
    }
}
