//! Path simplification: removes collinear points from a grid path.
//!
//! After A* finds a path on the occupancy grid, the raw path contains every
//! cell visited. This module reduces it to the minimal set of waypoints
//! (only direction-change points), which is what the renderer needs.

/// Remove collinear points from a grid path, keeping only corners.
///
/// A point is collinear if the direction from the previous point to it is
/// the same as the direction from it to the next point. Such points are
/// removed; only the first, last, and corner points remain.
///
/// For paths with ≤ 2 points, the input is returned unchanged.
pub fn simplify_path(points: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len());
    result.push(points[0]);

    for i in 1..points.len() - 1 {
        let prev = points[i - 1];
        let curr = points[i];
        let next = points[i + 1];

        let dir1 = (
            curr.0 as isize - prev.0 as isize,
            curr.1 as isize - prev.1 as isize,
        );
        let dir2 = (
            next.0 as isize - curr.0 as isize,
            next.1 as isize - curr.1 as isize,
        );

        // Keep only if direction changes (corner point).
        if dir1 != dir2 {
            result.push(curr);
        }
    }

    result.push(*points.last().unwrap());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_short_path() {
        let pts = vec![(0, 0), (5, 5)];
        assert_eq!(simplify_path(&pts), pts);
    }

    #[test]
    fn passthrough_single_point() {
        let pts = vec![(0, 0)];
        assert_eq!(simplify_path(&pts), pts);
    }

    #[test]
    fn removes_collinear_points() {
        // Horizontal line: (0,0) → (1,0) → (2,0) → (3,0)
        let pts = vec![(0, 0), (1, 0), (2, 0), (3, 0)];
        let simplified = simplify_path(&pts);
        assert_eq!(simplified, vec![(0, 0), (3, 0)]);
    }

    #[test]
    fn keeps_corner_points() {
        // L-shape: (0,0) → (2,0) → (2,3)
        let pts = vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2), (2, 3)];
        let simplified = simplify_path(&pts);
        assert_eq!(simplified, vec![(0, 0), (2, 0), (2, 3)]);
    }

    #[test]
    fn complex_path() {
        // Zigzag: right → down → right → down
        let pts = vec![
            (0, 0), (1, 0), (2, 0), // right
            (2, 1), (2, 2), // down
            (3, 2), (4, 2), // right
            (4, 3), (4, 4), // down
        ];
        let simplified = simplify_path(&pts);
        assert_eq!(simplified, vec![(0, 0), (2, 0), (2, 2), (4, 2), (4, 4)]);
    }

    #[test]
    fn preserves_endpoints() {
        let pts = vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)];
        let simplified = simplify_path(&pts);
        assert_eq!(*simplified.first().unwrap(), (0, 0));
        assert_eq!(*simplified.last().unwrap(), (2, 2));
    }
}
