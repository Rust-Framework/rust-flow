use crate::math::Point;

/// Cubic Bézier path between two port endpoints (ReactFlow-style).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BezierPath {
    pub from: Point,
    pub to: Point,
    pub cp1: Point,
    pub cp2: Point,
}

/// Default curvature factor (ReactFlow default).
pub const DEFAULT_CURVATURE: f32 = 0.25;

/// Compute direction-aware control points for a horizontal port layout.
pub fn bezier_control_points(from: Point, to: Point) -> (Point, Point) {
    bezier_control_points_with_curvature(from, to, DEFAULT_CURVATURE)
}

pub fn bezier_control_points_with_curvature(
    from: Point,
    to: Point,
    curvature: f32,
) -> (Point, Point) {
    let dx = (to.x - from.x).abs();
    (
        Point::new(from.x + dx * curvature, from.y),
        Point::new(to.x - dx * curvature, to.y),
    )
}

impl BezierPath {
    pub fn from_endpoints(from: Point, to: Point) -> Self {
        let (cp1, cp2) = bezier_control_points(from, to);
        Self { from, to, cp1, cp2 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_control_points_horizontal() {
        let from = Point::new(0.0, 0.0);
        let to = Point::new(200.0, 0.0);
        let (cp1, cp2) = bezier_control_points(from, to);
        assert!((cp1.x - 50.0).abs() < 0.01);
        assert!((cp2.x - 150.0).abs() < 0.01);
        assert_eq!(cp1.y, 0.0);
        assert_eq!(cp2.y, 0.0);
    }
}
