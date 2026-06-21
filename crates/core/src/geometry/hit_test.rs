//! Hit-testing helpers for canvas interaction.

use crate::geometry::{PointF, RectF};

/// Point-in-rectangle test.
pub fn point_in_rect(p: PointF, rect: RectF) -> bool {
    rect.contains(p)
}

/// Distance from a point to a polyline (list of connected segments).
///
/// Returns `f32::MAX` for empty/single-point polylines.
pub fn point_to_polyline_distance(p: PointF, points: &[PointF]) -> f32 {
    if points.len() < 2 {
        return f32::MAX;
    }
    let mut min = f32::MAX;
    for w in points.windows(2) {
        let d = point_to_segment_distance(p, w[0], w[1]);
        if d < min {
            min = d;
        }
    }
    min
}

/// Distance from point `p` to segment `[a, b]`.
fn point_to_segment_distance(p: PointF, a: PointF, b: PointF) -> f32 {
    let ab = PointF::new(b.x - a.x, b.y - a.y);
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq < 1e-12 {
        return p.distance_to(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj = PointF::new(a.x + ab.x * t, a.y + ab.y * t);
    p.distance_to(proj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SizeF;

    #[test]
    fn point_in_rect_basic() {
        let r = RectF::new(PointF::new(0.0, 0.0), SizeF::new(10.0, 10.0));
        assert!(point_in_rect(PointF::new(5.0, 5.0), r));
        assert!(!point_in_rect(PointF::new(15.0, 5.0), r));
    }

    #[test]
    fn distance_to_polyline() {
        let pts = vec![PointF::new(0.0, 0.0), PointF::new(10.0, 0.0)];
        let d = point_to_polyline_distance(PointF::new(5.0, 3.0), &pts);
        assert!((d - 3.0).abs() < 1e-6);
    }
}
