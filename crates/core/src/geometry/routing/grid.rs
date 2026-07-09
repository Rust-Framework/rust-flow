//! Occupancy grid for obstacle-aware edge routing.
//!
//! Discretizes the canvas into a regular grid where each cell is either free
//! or blocked (obstacle). The A* pathfinder searches on this grid to find
//! obstacle-avoiding orthogonal paths.

use crate::geometry::{PointF, RectF};

/// A 2D grid where each cell is marked as free (`false`) or blocked (`true`).
///
/// The grid covers a rectangular region of logical space starting at `origin`,
/// with cells of size `cell_size`. Cell `(x, y)` covers the logical region:
/// `[origin.x + x*cell_size, origin.x + (x+1)*cell_size) ×
///  [origin.y + y*cell_size, origin.y + (y+1)*cell_size)`
pub struct OccupancyGrid {
    cells: Vec<bool>,
    width: usize,
    height: usize,
    origin: PointF,
    cell_size: f32,
}

impl OccupancyGrid {
    /// Create a grid covering the given bounding rectangle.
    ///
    /// All cells are initially free (`false`). The grid dimensions are
    /// `ceil(bounds.w / cell_size) × ceil(bounds.h / cell_size)`.
    pub fn new(bounds: RectF, cell_size: f32) -> Self {
        let cell_size = cell_size.max(1.0);
        let width = ((bounds.size.w / cell_size).ceil() as usize).max(1);
        let height = ((bounds.size.h / cell_size).ceil() as usize).max(1);
        Self {
            cells: vec![false; width * height],
            width,
            height,
            origin: bounds.origin,
            cell_size,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Convert a logical coordinate to grid cell coordinates.
    ///
    /// Uses `floor` so cell `i` covers the half-open interval
    /// `[origin + i*cell, origin + (i+1)*cell)`. This keeps cell midpoints
    /// (e.g. `origin + 0.5*cell`) in their own cell and makes `to_grid` the
    /// inverse of `to_logical` (which returns cell centers).
    /// Out-of-bounds points are clamped to the grid edges.
    pub fn to_grid(&self, p: PointF) -> (usize, usize) {
        let x = ((p.x - self.origin.x) / self.cell_size).floor() as isize;
        let y = ((p.y - self.origin.y) / self.cell_size).floor() as isize;
        let x = x.clamp(0, (self.width - 1) as isize) as usize;
        let y = y.clamp(0, (self.height - 1) as isize) as usize;
        (x, y)
    }

    /// Convert a grid cell to logical coordinates (center of the cell).
    pub fn to_logical(&self, x: usize, y: usize) -> PointF {
        PointF::new(
            self.origin.x + (x as f32 + 0.5) * self.cell_size,
            self.origin.y + (y as f32 + 0.5) * self.cell_size,
        )
    }

    /// Mark all cells overlapping the given rectangle as blocked.
    /// Cells outside the grid are silently ignored.
    pub fn mark_obstacle(&mut self, rect: RectF) {
        let (x0, y0) = self.to_grid(rect.origin);
        let (x1, y1) = self.to_grid(PointF::new(
            rect.origin.x + rect.size.w,
            rect.origin.y + rect.size.h,
        ));
        let x0 = x0.min(x1);
        let x1 = x0.max(x1);
        let y0 = y0.min(y1);
        let y1 = y0.max(y1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x < self.width && y < self.height {
                    self.cells[y * self.width + x] = true;
                }
            }
        }
    }

    /// Clear (un-block) all cells overlapping the given rectangle.
    pub fn clear_obstacle(&mut self, rect: RectF) {
        let (x0, y0) = self.to_grid(rect.origin);
        let (x1, y1) = self.to_grid(PointF::new(
            rect.origin.x + rect.size.w,
            rect.origin.y + rect.size.h,
        ));
        let x0 = x0.min(x1);
        let x1 = x0.max(x1);
        let y0 = y0.min(y1);
        let y1 = y0.max(y1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x < self.width && y < self.height {
                    self.cells[y * self.width + x] = false;
                }
            }
        }
    }

    /// Check if a cell is blocked. Out-of-bounds cells are treated as blocked.
    pub fn is_blocked(&self, x: usize, y: usize) -> bool {
        if x >= self.width || y >= self.height {
            return true;
        }
        self.cells[y * self.width + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SizeF;

    #[test]
    fn grid_dimensions() {
        let bounds = RectF::new(PointF::new(0.0, 0.0), SizeF::new(100.0, 50.0));
        let grid = OccupancyGrid::new(bounds, 10.0);
        assert_eq!(grid.width(), 10);
        assert_eq!(grid.height(), 5);
    }

    #[test]
    fn mark_and_check_obstacle() {
        let bounds = RectF::new(PointF::new(0.0, 0.0), SizeF::new(100.0, 100.0));
        let mut grid = OccupancyGrid::new(bounds, 10.0);

        // Mark a 30×30 obstacle at (20, 20)
        let obstacle = RectF::new(PointF::new(20.0, 20.0), SizeF::new(30.0, 30.0));
        grid.mark_obstacle(obstacle);

        // Cells inside the obstacle should be blocked
        assert!(grid.is_blocked(3, 3)); // center of obstacle
        assert!(grid.is_blocked(2, 2)); // edge
        assert!(grid.is_blocked(4, 4)); // edge

        // Cells outside should be free
        assert!(!grid.is_blocked(0, 0));
        assert!(!grid.is_blocked(9, 9));
        assert!(!grid.is_blocked(8, 3));
    }

    #[test]
    fn clear_obstacle() {
        let bounds = RectF::new(PointF::new(0.0, 0.0), SizeF::new(100.0, 100.0));
        let mut grid = OccupancyGrid::new(bounds, 10.0);

        let obstacle = RectF::new(PointF::new(20.0, 20.0), SizeF::new(30.0, 30.0));
        grid.mark_obstacle(obstacle);
        assert!(grid.is_blocked(3, 3));

        grid.clear_obstacle(obstacle);
        assert!(!grid.is_blocked(3, 3));
    }

    #[test]
    fn coordinate_conversion() {
        let bounds = RectF::new(PointF::new(100.0, 200.0), SizeF::new(100.0, 100.0));
        let grid = OccupancyGrid::new(bounds, 10.0);

        // Logical (105, 205) → grid (0, 0)
        let (x, y) = grid.to_grid(PointF::new(105.0, 205.0));
        assert_eq!(x, 0);
        assert_eq!(y, 0);

        // Grid (5, 5) → logical center (155, 255)
        let p = grid.to_logical(5, 5);
        assert!((p.x - 155.0).abs() < 0.01);
        assert!((p.y - 255.0).abs() < 0.01);
    }

    #[test]
    fn out_of_bounds_treated_as_blocked() {
        let bounds = RectF::new(PointF::new(0.0, 0.0), SizeF::new(50.0, 50.0));
        let grid = OccupancyGrid::new(bounds, 10.0);
        assert!(grid.is_blocked(100, 100)); // way out of bounds
    }
}
