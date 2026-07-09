//! Obstacle-aware edge routing using Grid A* pathfinding.
//!
//! Unlike ReactFlow's pure-geometric path algorithms (`smoothstep_path`,
//! `bezier_path`), which compute paths based solely on src/dst positions
//! and port sides, this module is aware of all nodes on the canvas and
//! routes edges around them.
//!
//! ## Algorithm
//!
//! 1. **Occupancy grid**: The canvas is discretized into a regular grid
//!    (default 10px per cell). Each node's bounding rectangle (expanded by
//!    a margin) is marked as a blocked cell.
//! 2. **A* search**: The A* algorithm finds the shortest orthogonal path
//!    (4-directional, no diagonals) from the source port to the destination
//!    port, avoiding blocked cells. A turn penalty discourages unnecessary
//!    direction changes.
//! 3. **Direction constraints**: The first step respects the source port's
//!    side (e.g., `Right` → first step goes right). The last step enters
//!    the destination port from the correct direction.
//! 4. **Simplification**: Collinear grid cells are removed, leaving only
//!    corner waypoints.
//! 5. **Fallback**: If A* fails with direction constraints (e.g., the
//!    constrained first step leads to a blocked cell), it retries without
//!    constraints. If still no path, the caller falls back to the geometric
//!    path algorithm.

pub mod astar;
pub mod grid;
pub mod simplify;

use crate::geometry::{PointF, RectF, SizeF};
use crate::graph::PortSide;

use astar::Direction;
use grid::OccupancyGrid;
use simplify::simplify_path;

/// Grid cell size in logical pixels. Smaller = finer paths but slower A*.
pub const GRID_CELL_SIZE: f32 = 10.0;

/// Margin (in logical pixels) added around each node's bounds when marking
/// obstacles. Ensures paths keep a visual gap from nodes.
pub const OBSTACLE_MARGIN: f32 = 15.0;

/// Penalty added per direction change in A*. Higher = fewer turns but
/// potentially longer paths.
pub const TURN_PENALTY: f32 = 2.0;

/// Route an edge from `src` to `dst`, avoiding the given obstacle rectangles.
///
/// - `src`, `dst`: port positions in logical coordinates
/// - `src_side`, `dst_side`: port sides (determine exit/entry direction)
/// - `obstacles`: node bounding rectangles to avoid (should already be
///   expanded by `OBSTACLE_MARGIN` by the caller)
/// - `grid_size`: cell size for the occupancy grid
///
/// Returns waypoints in logical coordinates (including `src` and `dst` as
/// the first and last points), or `None` if no path can be found.
pub fn route_edge(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    obstacles: &[RectF],
    grid_size: f32,
) -> Option<Vec<PointF>> {
    if src.distance_to(dst) < 1.0 {
        return Some(vec![src, dst]);
    }

    // 1. Compute grid bounds: union of obstacles + src/dst, expanded.
    let mut bounds: Option<RectF> = None;
    for &obs in obstacles {
        bounds = Some(match bounds {
            Some(b) => b.union(obs),
            None => obs,
        });
    }
    let pt_rect = |p: PointF| RectF::from_center(p, SizeF::new(1.0, 1.0));
    bounds = Some(match bounds {
        Some(b) => b.union(pt_rect(src)).union(pt_rect(dst)),
        None => pt_rect(src).union(pt_rect(dst)),
    });
    let bounds = bounds.unwrap().expand(2.0 * grid_size);

    // 2. Build occupancy grid and mark obstacles.
    let mut grid = OccupancyGrid::new(bounds, grid_size);
    for &obs in obstacles {
        grid.mark_obstacle(obs);
    }

    // 3. Clear obstacles around src and dst so they're reachable even if
    //    a margin-expanded obstacle covers them.
    let clear_size = SizeF::new(grid_size * 4.0, grid_size * 4.0);
    grid.clear_obstacle(RectF::from_center(src, clear_size));
    grid.clear_obstacle(RectF::from_center(dst, clear_size));

    // 4. Convert to grid coordinates.
    let start = grid.to_grid(src);
    let goal = grid.to_grid(dst);

    // 5. Compute direction constraints.
    let start_dir = Some(Direction::from_side(src_side));
    let goal_dir = Some(Direction::inward(dst_side));

    // 6. A* with progressive constraint relaxation.
    //    Try with both constraints first, then relax one at a time.
    let grid_path = astar::find_path(&grid, start, goal, start_dir, goal_dir, TURN_PENALTY)
        .or_else(|| astar::find_path(&grid, start, goal, start_dir, None, TURN_PENALTY))
        .or_else(|| astar::find_path(&grid, start, goal, None, goal_dir, TURN_PENALTY))
        .or_else(|| astar::find_path(&grid, start, goal, None, None, TURN_PENALTY));

    let grid_path = grid_path?;

    // 7. Simplify: remove collinear points.
    let simplified = simplify_path(&grid_path);

    // 8. Convert to logical coordinates, replacing endpoints with exact
    //    port positions (grid cell centers may differ slightly).
    let mut waypoints: Vec<PointF> = simplified
        .iter()
        .map(|&(x, y)| grid.to_logical(x, y))
        .collect();
    if waypoints.len() >= 2 {
        waypoints[0] = src;
        *waypoints.last_mut().unwrap() = dst;
    }

    Some(waypoints)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_obstacle(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF::new(PointF::new(x, y), SizeF::new(w, h))
    }

    #[test]
    fn route_with_no_obstacles() {
        let src = PointF::new(0.0, 50.0);
        let dst = PointF::new(200.0, 50.0);
        let path = route_edge(src, dst, PortSide::Right, PortSide::Left, &[], 10.0).unwrap();

        assert_eq!(path.first(), Some(&src));
        assert_eq!(path.last(), Some(&dst));
        // With no obstacles, path should be a straight line (2 points after simplification).
        assert_eq!(path.len(), 2, "no-obstacle path should be a straight line");
    }

    #[test]
    fn route_around_obstacle() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(300.0, 100.0);
        // Wall in the middle blocking direct horizontal path
        let obstacle = make_obstacle(130.0, 50.0, 40.0, 150.0);

        let path = route_edge(src, dst, PortSide::Right, PortSide::Left, &[obstacle], 10.0)
            .expect("should find a path around the obstacle");

        assert_eq!(path.first(), Some(&src));
        assert_eq!(path.last(), Some(&dst));
        // Path should have more than 2 points (it goes around the obstacle)
        assert!(path.len() > 2, "path should bend around the obstacle");
    }

    #[test]
    fn route_respects_start_direction() {
        let src = PointF::new(50.0, 50.0);
        let dst = PointF::new(200.0, 200.0);
        let path = route_edge(src, dst, PortSide::Right, PortSide::Top, &[], 10.0).unwrap();

        // First waypoint after src should be to the right (start_direction = Right).
        let first_move = path[1];
        assert!(
            first_move.x > src.x,
            "first move should be rightward, got {:?}",
            first_move
        );
    }

    #[test]
    fn route_respects_goal_direction() {
        let src = PointF::new(50.0, 50.0);
        let dst = PointF::new(200.0, 200.0);
        let path = route_edge(src, dst, PortSide::Right, PortSide::Top, &[], 10.0).unwrap();

        // Last move before dst should be downward (goal_direction = Down for Top port).
        // PortSide::Top → inward = Down → last step enters from above, moving down.
        let last_move = path[path.len() - 2];
        assert!(
            last_move.y < dst.y,
            "last move should be from above (moving down), got {:?}",
            last_move
        );
    }

    #[test]
    fn route_very_close_points() {
        let src = PointF::new(50.0, 50.0);
        let dst = PointF::new(50.5, 50.5);
        let path = route_edge(src, dst, PortSide::Right, PortSide::Left, &[], 10.0).unwrap();
        assert_eq!(path, vec![src, dst]);
    }

    #[test]
    fn route_relaxes_constraints_when_blocked() {
        let src = PointF::new(50.0, 50.0);
        let dst = PointF::new(200.0, 200.0);
        // Obstacle right next to src on the right side — blocks start_direction=Right
        let obstacle = make_obstacle(60.0, 40.0, 30.0, 30.0);

        let path = route_edge(src, dst, PortSide::Right, PortSide::Top, &[obstacle], 10.0);
        assert!(path.is_some(), "should find path by relaxing start direction");
    }
}
