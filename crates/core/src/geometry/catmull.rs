use crate::math::Point;

/// One cubic Bézier segment (gpui-component Catmull-Rom conversion).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatmullSegment {
    pub end: Point,
    pub cp1: Point,
    pub cp2: Point,
}

/// Convert a Catmull-Rom spline through `points` into cubic Bézier segments.
///
/// Algorithm ported from gpui-component `plot/shape/line.rs` (`StrokeStyle::Natural`).
pub fn catmull_rom_segments(points: &[Point]) -> Vec<CatmullSegment> {
    if points.len() < 2 {
        return Vec::new();
    }

    let n = points.len();
    let mut segments = Vec::with_capacity(n - 1);

    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n { points[i + 2] } else { points[n - 1] };

        let cp1 = p1 + (p2 - p0) / 6.0;
        let cp2 = p2 - (p3 - p1) / 6.0;

        segments.push(CatmullSegment { end: p2, cp1, cp2 });
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catmull_straight_line_degenerates_to_linear() {
        let pts = vec![
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(200.0, 0.0),
        ];
        let segs = catmull_rom_segments(&pts);
        assert_eq!(segs.len(), 2);
        assert!((segs[0].end.x - 100.0).abs() < 0.01);
        assert!((segs[1].end.x - 200.0).abs() < 0.01);
    }
}
