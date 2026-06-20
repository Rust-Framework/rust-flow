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

/// Mind map branch bezier — mind-elixir `main()` style quadratic bezier.
/// `M x1 y1 Q x1 y2 x2 y2` — control point shares source x, target y.
/// Produces organic curved branches typical of mind maps.
pub fn mindmap_main_bezier(from: Point, to: Point) -> BezierPath {
    let cp = Point::new(from.x, to.y);
    // For cubic BezierPath representation, duplicate the quadratic control point.
    BezierPath {
        from,
        to,
        cp1: cp,
        cp2: cp,
    }
}

/// Mind map sub-branch bezier — mind-elixir `sub()` style cubic bezier.
/// `M x1 y1 C xMid y1 xMid±offset y2 x2 y2` — S-curve with horizontal mid-section.
/// `offset` controls the curvature strength based on vertical distance.
pub fn mindmap_sub_bezier(from: Point, to: Point, gap: f32) -> BezierPath {
    let mid_x = (from.x + to.x) / 2.0;
    // Offset scales with vertical distance, clamped to gap (mind-elixir formula)
    let dy = (to.y - from.y).abs();
    let offset = (dy / 300.0 * gap).min(gap);
    let direction = if to.x > from.x { 1.0 } else { -1.0 };
    let cp1 = Point::new(mid_x + offset * direction * 0.5, from.y);
    let cp2 = Point::new(mid_x - offset * direction * 0.5, to.y);
    BezierPath {
        from,
        to,
        cp1,
        cp2,
    }
}

/// Mind map branch for LR layout — horizontal cubic bezier with adaptive curvature.
/// Control points extend horizontally from source and target, producing smooth
/// horizontal S-curves ideal for left-right mind map branches.
pub fn mindmap_lr_bezier(from: Point, to: Point, curvature: f32) -> BezierPath {
    let dx = to.x - from.x;
    let horizontal_offset = dx.abs() * curvature;
    let direction = if dx > 0.0 { 1.0 } else { -1.0 };
    BezierPath {
        from,
        to,
        cp1: Point::new(from.x + horizontal_offset * direction, from.y),
        cp2: Point::new(to.x - horizontal_offset * direction, to.y),
    }
}

/// Mind map branch for TB layout — vertical cubic bezier with adaptive curvature.
pub fn mindmap_tb_bezier(from: Point, to: Point, curvature: f32) -> BezierPath {
    let dy = to.y - from.y;
    let vertical_offset = dy.abs() * curvature;
    let direction = if dy > 0.0 { 1.0 } else { -1.0 };
    BezierPath {
        from,
        to,
        cp1: Point::new(from.x, from.y + vertical_offset * direction),
        cp2: Point::new(to.x, to.y - vertical_offset * direction),
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

    #[test]
    fn mindmap_main_bezier_quadratic_control() {
        let from = Point::new(0.0, 0.0);
        let to = Point::new(100.0, 50.0);
        let path = mindmap_main_bezier(from, to);
        // Control point should share source x and target y (mind-elixir main style)
        assert!((path.cp1.x - 0.0).abs() < 0.01);
        assert!((path.cp1.y - 50.0).abs() < 0.01);
    }

    #[test]
    fn mindmap_lr_bezier_horizontal_control() {
        let from = Point::new(0.0, 0.0);
        let to = Point::new(200.0, 100.0);
        let path = mindmap_lr_bezier(from, to, 0.5);
        // Control points should be horizontally offset from endpoints
        assert!(path.cp1.x > from.x);
        assert!(path.cp2.x < to.x);
        assert_eq!(path.cp1.y, from.y);
        assert_eq!(path.cp2.y, to.y);
    }
}
