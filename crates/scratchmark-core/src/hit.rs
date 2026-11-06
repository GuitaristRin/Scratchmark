use crate::{Document, Error, MAX_COORD, Result, Stroke};

impl Document {
    pub fn hit_test(&self, revision: u64, x: f64, y: f64, tolerance: f64) -> Result<Option<u32>> {
        if revision != self.revision {
            return Err(Error::StaleRevision);
        }
        if !x.is_finite()
            || !y.is_finite()
            || x.abs() > MAX_COORD
            || y.abs() > MAX_COORD
            || !tolerance.is_finite()
            || !(0.0..=MAX_COORD).contains(&tolerance)
        {
            return Err(Error::InvalidArgument);
        }
        for (index, stroke) in self.strokes.iter().enumerate().rev() {
            if stroke_hit(stroke, x, y, tolerance) {
                return Ok(Some(index as u32));
            }
        }
        Ok(None)
    }
}

fn stroke_hit(stroke: &Stroke, x: f64, y: f64, tolerance: f64) -> bool {
    let Some(bounds) = stroke.bounds else {
        return false;
    };
    if x < bounds[0] - tolerance
        || x > bounds[2] + tolerance
        || y < bounds[1] - tolerance
        || y > bounds[3] + tolerance
    {
        return false;
    }
    let radius = stroke.width / 2.0 + tolerance;
    let limit = radius * radius;
    let points = &stroke.points;
    if points.is_empty() {
        return false;
    }
    if points.len() == 1 {
        let p = &points[0];
        return distance_sq(x, y, p.x, p.y) <= limit;
    }
    points
        .windows(2)
        .any(|pair| segment_distance_sq(x, y, &pair[0], &pair[1]) <= limit)
}

fn segment_distance_sq(x: f64, y: f64, a: &crate::Sample, b: &crate::Sample) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length_sq = dx * dx + dy * dy;
    let t = if length_sq == 0.0 {
        0.0
    } else {
        (((x - a.x) * dx + (y - a.y) * dy) / length_sq).clamp(0.0, 1.0)
    };
    distance_sq(x, y, a.x + t * dx, a.y + t * dy)
}

fn distance_sq(x: f64, y: f64, px: f64, py: f64) -> f64 {
    let dx = x - px;
    let dy = y - py;
    dx * dx + dy * dy
}
