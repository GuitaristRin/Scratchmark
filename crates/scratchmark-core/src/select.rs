//! 套索命中与按索引批量删除、清空；行为真源参 spec/M6.md。
//!
//! 引擎不保存选区：命中是只读查询，删除按调用时给出的索引执行，
//! 因此宿主可以随时重算选区而不会让引擎产生隐式状态。
use crate::document::{Edit, ReplaceOp};
use crate::{Document, Error, MAX_COORD, Point, Result, Stroke};

impl Document {
    /// 命中查询：返回与闭合多边形有交集的已提交笔划索引（升序）。
    pub fn lasso_hits(&self, revision: u64, path: &[Point]) -> Result<Vec<u32>> {
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if path.len() < 3
            || path.iter().any(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || p.x.abs() > MAX_COORD
                    || p.y.abs() > MAX_COORD
            })
        {
            return Err(Error::InvalidArgument);
        }
        let bounds = polygon_bounds(path);
        let mut hits = Vec::new();
        for (index, stroke) in self.strokes.iter().enumerate() {
            if polygon_hits_stroke(stroke, path, bounds) {
                hits.push(index as u32);
            }
        }
        Ok(hits)
    }

    /// 按索引批量删除：一次调用只产生一个撤销条目。
    pub fn erase_indices(&mut self, revision: u64, indices: &[u32]) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if indices.is_empty() {
            return Ok(());
        }
        let mut sorted: Vec<usize> = indices.iter().map(|&index| index as usize).collect();
        sorted.sort_unstable();
        // 重复索引不猜宿主意图：直接拒绝，避免“删两次”语义含混。
        if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::InvalidArgument);
        }
        if sorted
            .last()
            .is_some_and(|&index| index >= self.strokes.len())
        {
            return Err(Error::InvalidArgument);
        }

        self.edit(move |d| {
            d.discard_redo();
            let mut ops = Vec::with_capacity(sorted.len());
            for (removed, &index) in sorted.iter().enumerate() {
                // 升序删除时前面的删除已经让下标前移，这里记施加时刻的下标。
                let at = index - removed;
                let stroke = d.strokes.remove(at);
                ops.push(ReplaceOp {
                    index: at,
                    after_count: 0,
                    before: Some(stroke),
                    after: None,
                });
            }
            d.undo.push(Edit::Replace(ops));
            Ok(())
        })
    }

    /// 清空全部已提交笔划，只占一个撤销条目。
    pub fn clear(&mut self, revision: u64) -> Result<()> {
        self.ensure_idle()?;
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if self.strokes.is_empty() {
            return Ok(());
        }
        let all: Vec<u32> = (0..self.strokes.len() as u32).collect();
        self.erase_indices(revision, &all)
    }
}

fn polygon_bounds(path: &[Point]) -> [f64; 4] {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for p in path {
        bounds[0] = bounds[0].min(p.x);
        bounds[1] = bounds[1].min(p.y);
        bounds[2] = bounds[2].max(p.x);
        bounds[3] = bounds[3].max(p.y);
    }
    bounds
}

fn polygon_hits_stroke(stroke: &Stroke, path: &[Point], bounds: [f64; 4]) -> bool {
    let Some(inner) = stroke.bounds else {
        return false;
    };
    // 包围盒先排除：笔划包围盒含半个笔宽，只会更宽松，不会漏选。
    if inner[0] > bounds[2] || inner[2] < bounds[0] || inner[1] > bounds[3] || inner[3] < bounds[1]
    {
        return false;
    }

    let points = &stroke.points;
    if points.is_empty() {
        return false;
    }
    if points.len() == 1 {
        return point_in_polygon(points[0].x, points[0].y, path);
    }
    if points.iter().any(|p| point_in_polygon(p.x, p.y, path)) {
        return true;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let n = path.len();
        for i in 0..n {
            let edge = (path[i], path[(i + 1) % n]);
            if segments_intersect(a.x, a.y, b.x, b.y, edge.0.x, edge.0.y, edge.1.x, edge.1.y) {
                return true;
            }
        }
    }
    false
}

/// 穿越计数（even-odd）内部判定；自交多边形因此可能把某个环判为外部。
fn point_in_polygon(x: f64, y: f64, path: &[Point]) -> bool {
    let mut inside = false;
    let n = path.len();
    let mut previous = n - 1;
    for current in 0..n {
        let (xi, yi) = (path[current].x, path[current].y);
        let (xj, yj) = (path[previous].x, path[previous].y);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn orientation(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

fn on_segment(ax: f64, ay: f64, bx: f64, by: f64, px: f64, py: f64) -> bool {
    px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by)
}

#[allow(clippy::too_many_arguments)]
fn segments_intersect(
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
    dx: f64,
    dy: f64,
) -> bool {
    let d1 = orientation(cx, cy, dx, dy, ax, ay);
    let d2 = orientation(cx, cy, dx, dy, bx, by);
    let d3 = orientation(ax, ay, bx, by, cx, cy);
    let d4 = orientation(ax, ay, bx, by, dx, dy);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }
    // 共线与端点接触按“相交”处理：选区边界上的笔划算选中。
    (d1 == 0.0 && on_segment(cx, cy, dx, dy, ax, ay))
        || (d2 == 0.0 && on_segment(cx, cy, dx, dy, bx, by))
        || (d3 == 0.0 && on_segment(ax, ay, bx, by, cx, cy))
        || (d4 == 0.0 && on_segment(ax, ay, bx, by, dx, dy))
}
