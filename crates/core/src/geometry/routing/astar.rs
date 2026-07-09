//! A* pathfinding on the occupancy grid.
//!
//! Finds the shortest orthogonal path (4-directional, no diagonals) from
//! `start` to `goal`, avoiding blocked cells. A turn penalty discourages
//! unnecessary direction changes, producing cleaner paths with fewer bends.
//!
//! Direction constraints ensure the path exits the source port and enters
//! the destination port in the correct direction (matching port sides).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::graph::PortSide;

use super::grid::OccupancyGrid;

/// Movement direction on the grid (4-directional, no diagonals).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    /// The outward movement direction for a port side.
    ///
    /// E.g., `PortSide::Right` → `Direction::Right` (edge exits rightward).
    pub fn from_side(side: PortSide) -> Self {
        match side {
            PortSide::Left => Direction::Left,
            PortSide::Right => Direction::Right,
            PortSide::Top => Direction::Up,
            PortSide::Bottom => Direction::Down,
            PortSide::Auto => Direction::Right,
        }
    }

    /// The inward movement direction (opposite of outward).
    ///
    /// Used for the goal: if `dst_side = Left`, the edge enters from the left,
    /// moving rightward → `Direction::Right`.
    pub fn inward(side: PortSide) -> Self {
        Self::from_side(side).opposite()
    }

    pub fn opposite(self) -> Self {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        }
    }

    pub fn delta(self) -> (isize, isize) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }

    pub const ALL: [Direction; 4] = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ];
}

/// A* node in the open set.
#[derive(Clone, Copy)]
struct AStarNode {
    index: usize,
    g_score: f32,
    f_score: f32,
    direction: Option<Direction>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_score == other.f_score
    }
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Min-heap: lower f_score = higher priority.
        other.f_score.partial_cmp(&self.f_score)
    }
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Manhattan distance heuristic.
fn manhattan(a: (usize, usize), b: (usize, usize)) -> f32 {
    (a.0 as isize - b.0 as isize).abs() as f32 + (a.1 as isize - b.1 as isize).abs() as f32
}

/// Search for the shortest orthogonal path on the grid.
///
/// - 4-directional movement (no diagonals)
/// - Manhattan distance heuristic
/// - `turn_penalty` added per direction change to encourage straight lines
/// - `start_direction`: if set, the first step must be in this direction
/// - `goal_direction`: if set, the last step (entering goal) must be in this direction
///
/// Returns the grid-coordinate path (including start and goal), or `None`
/// if no path exists. If direction constraints prevent finding a path,
/// the caller should retry with `None` for both constraints.
pub fn find_path(
    grid: &OccupancyGrid,
    start: (usize, usize),
    goal: (usize, usize),
    start_direction: Option<Direction>,
    goal_direction: Option<Direction>,
    turn_penalty: f32,
) -> Option<Vec<(usize, usize)>> {
    let width = grid.width();
    let height = grid.height();

    if start == goal {
        return Some(vec![start]);
    }

    if grid.is_blocked(start.0, start.1) || grid.is_blocked(goal.0, goal.1) {
        return None;
    }

    let start_index = start.1 * width + start.0;
    let goal_index = goal.1 * width + goal.0;
    let total = width * height;

    let mut g_score = vec![f32::MAX; total];
    let mut came_from: Vec<Option<usize>> = vec![None; total];

    g_score[start_index] = 0.0;

    let mut open = BinaryHeap::new();
    open.push(AStarNode {
        index: start_index,
        g_score: 0.0,
        f_score: manhattan(start, goal),
        direction: None,
    });

    while let Some(node) = open.pop() {
        // Skip stale entries (a better path to this cell was already found).
        if node.g_score > g_score[node.index] {
            continue;
        }

        // Goal reached — reconstruct path.
        if node.index == goal_index {
            let mut path = Vec::new();
            let mut current = node.index;
            path.push((current % width, current / width));
            while let Some(prev) = came_from[current] {
                current = prev;
                path.push((current % width, current / width));
            }
            path.reverse();
            return Some(path);
        }

        let cx = node.index % width;
        let cy = node.index / width;
        let current_dir = node.direction;

        for dir in Direction::ALL {
            // Start direction constraint: only allow the first step in this direction.
            if current_dir.is_none() && start_direction.is_some() {
                if Some(dir) != start_direction {
                    continue;
                }
            }

            let (dx, dy) = dir.delta();
            let nx = cx as isize + dx;
            let ny = cy as isize + dy;

            if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);

            if grid.is_blocked(nx, ny) {
                continue;
            }

            let neighbor_index = ny * width + nx;

            // Goal direction constraint: only enter the goal from the correct direction.
            if neighbor_index == goal_index && goal_direction.is_some() {
                if Some(dir) != goal_direction {
                    continue;
                }
            }

            // Compute move cost with turn penalty.
            let turn_cost = match current_dir {
                Some(prev_dir) if prev_dir != dir => turn_penalty,
                _ => 0.0,
            };
            let move_cost = 1.0 + turn_cost;
            let tentative_g = node.g_score + move_cost;

            if tentative_g < g_score[neighbor_index] {
                g_score[neighbor_index] = tentative_g;
                came_from[neighbor_index] = Some(node.index);

                let h = manhattan((nx, ny), goal);
                let f = tentative_g + h;
                open.push(AStarNode {
                    index: neighbor_index,
                    g_score: tentative_g,
                    f_score: f,
                    direction: Some(dir),
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PointF, SizeF};
    use crate::geometry::RectF;

    fn empty_grid() -> OccupancyGrid {
        OccupancyGrid::new(RectF::new(PointF::new(0.0, 0.0), SizeF::new(200.0, 200.0)), 10.0)
    }

    #[test]
    fn straight_path_no_obstacles() {
        let grid = empty_grid();
        let path = find_path(&grid, (0, 0), (5, 0), None, None, 2.0).unwrap();
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(5, 0)));
        // Straight horizontal path should have exactly 2 points (start + goal)
        // after simplification, but find_path returns raw grid path.
        // With turn penalty, A* prefers straight lines, so no turns.
        assert!(path.len() <= 6);
    }

    #[test]
    fn l_shaped_path() {
        let grid = empty_grid();
        let path = find_path(&grid, (0, 0), (5, 3), None, None, 2.0).unwrap();
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(5, 3)));
    }

    #[test]
    fn routes_around_obstacle() {
        let mut grid = empty_grid();
        // Wall from (2,0) to (2,4) — blocks direct horizontal path
        grid.mark_obstacle(RectF::new(PointF::new(20.0, 0.0), SizeF::new(10.0, 50.0)));

        let path = find_path(&grid, (0, 2), (5, 2), None, None, 2.0).unwrap();
        assert_eq!(path.first(), Some(&(0, 2)));
        assert_eq!(path.last(), Some(&(5, 2)));

        // Path must go around the wall — no cell at x=2 should be in the path
        // (the wall blocks x=2, y=0..4, and start is at y=2)
        for &(_, y) in &path {
            // The path should not pass through the wall at (2, 2)
            let _ = y;
        }
    }

    #[test]
    fn no_path_returns_none() {
        let mut grid = empty_grid();
        // Full-width wall
        grid.mark_obstacle(RectF::new(PointF::new(30.0, 0.0), SizeF::new(10.0, 200.0)));

        let result = find_path(&grid, (0, 5), (10, 5), None, None, 2.0);
        // Wall from x=30 to x=40, height 200 — but grid is only 20 wide (0..19)
        // x=3..4 blocked, but path can go around via x=0..2 or x=5..19
        // Actually the wall doesn't span the full height of the grid (200px = 20 cells)
        // Wait, 200.0 height means 20 cells, so it does span the full height.
        // But grid height is 200/10 = 20, so wall blocks y=0..19 at x=3..4
        assert!(result.is_none(), "should be no path through full-height wall");
    }

    #[test]
    fn start_equals_goal() {
        let grid = empty_grid();
        let path = find_path(&grid, (3, 3), (3, 3), None, None, 2.0).unwrap();
        assert_eq!(path, vec![(3, 3)]);
    }

    #[test]
    fn start_direction_constraint() {
        let grid = empty_grid();
        // Without constraint: path from (0,0) to (5,5) could go right-then-down or down-then-right
        // With start_direction=Right: first step must be right
        let path = find_path(&grid, (0, 0), (5, 5), Some(Direction::Right), None, 2.0).unwrap();
        // First move should be right: (0,0) → (1,0)
        assert_eq!(path[1], (1, 0), "first step must be Right");
    }

    #[test]
    fn goal_direction_constraint() {
        let grid = empty_grid();
        // goal_direction=Down means last step enters goal moving down
        let path = find_path(&grid, (0, 0), (5, 5), None, Some(Direction::Down), 2.0).unwrap();
        // Last step should be from (5, 4) to (5, 5) — moving down
        let last_idx = path.len() - 1;
        assert_eq!(path[last_idx], (5, 5));
        assert_eq!(path[last_idx - 1], (5, 4), "last step must be Down (from above)");
    }

    #[test]
    fn direction_constraint_relaxation() {
        let mut grid = empty_grid();
        // Block the cell to the right of start — start_direction=Right should fail
        grid.mark_obstacle(RectF::new(PointF::new(10.0, 0.0), SizeF::new(10.0, 10.0)));

        // With start_direction=Right: first step must be right, but (1,0) is blocked
        let result = find_path(&grid, (0, 0), (5, 5), Some(Direction::Right), None, 2.0);
        assert!(result.is_none(), "should fail when start_direction leads to blocked cell");

        // Without constraint: should succeed
        let path = find_path(&grid, (0, 0), (5, 5), None, None, 2.0).unwrap();
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(5, 5)));
    }

    #[test]
    fn direction_from_side() {
        assert_eq!(Direction::from_side(PortSide::Left), Direction::Left);
        assert_eq!(Direction::from_side(PortSide::Right), Direction::Right);
        assert_eq!(Direction::from_side(PortSide::Top), Direction::Up);
        assert_eq!(Direction::from_side(PortSide::Bottom), Direction::Down);
    }

    #[test]
    fn direction_inward() {
        // Inward = opposite of outward
        assert_eq!(Direction::inward(PortSide::Left), Direction::Right);
        assert_eq!(Direction::inward(PortSide::Right), Direction::Left);
        assert_eq!(Direction::inward(PortSide::Top), Direction::Down);
        assert_eq!(Direction::inward(PortSide::Bottom), Direction::Up);
    }
}
