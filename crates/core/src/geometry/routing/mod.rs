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
        // 9. 端点段对齐：A* 网格量化 + 方向约束放宽后，首尾段可能不严格垂直于
        //    端口面，导致箭头与入面不垂直。插入拐点强制首段垂直于 src_side、
        //    末段垂直于 dst_side，保证箭头方向规范。
        align_endpoints(&mut waypoints, src_side, dst_side);
    }

    Some(waypoints)
}

/// 对齐 A* 路径的端点段，确保起点段垂直于 `src_side`、终点段垂直于 `dst_side`，
/// 且端点段长度不小于 `MIN_LEN`，避免拐弯与箭头贴在一起。
///
/// A* 在网格上寻路，受网格量化与方向约束逐步放宽的影响，首尾段方向可能与端口
/// 面不严格垂直（如 dst 上方被障碍堵住时 `goal_dir` 被放宽，末段从侧方进入），
/// 且末段可能过短。本函数在首尾插入拐点，强制端点段沿端口法线方向并保证最小
/// 长度，使箭头方向规范、与拐弯分离。拐点落在端口附近（已由 `clear_obstacle`
/// 清理），不会穿障碍。
fn align_endpoints(
    waypoints: &mut Vec<PointF>,
    src_side: PortSide,
    dst_side: PortSide,
) {
    if waypoints.len() < 2 {
        return;
    }
    const MIN_LEN: f32 = 20.0;

    // 终点段对齐（先处理尾部，不影响头部索引）。
    let dst_pt = waypoints.pop().unwrap();
    let prev = *waypoints.last().unwrap();
    if let Some(corners) = align_tail(prev, dst_pt, dst_side, MIN_LEN) {
        for c in corners {
            waypoints.push(c);
        }
    }
    waypoints.push(dst_pt);

    // 起点段对齐。
    let src_pt = waypoints.remove(0);
    let second = waypoints[0];
    if let Some(corners) = align_head(src_pt, second, src_side, MIN_LEN) {
        let mut rebuilt = Vec::with_capacity(waypoints.len() + 1 + corners.len());
        rebuilt.push(src_pt);
        rebuilt.extend(corners);
        rebuilt.extend_from_slice(&waypoints);
        *waypoints = rebuilt;
    } else {
        waypoints.insert(0, src_pt);
    }
}

/// 计算终点段对齐所需的拐点（插入在 `prev` 与 `dst` 之间）。
///
/// 返回 `None` 表示末段已规范无需修正；`Some(corners)` 为需插入的拐点序列。
/// 当 `prev` 位于 dst 入面内侧（如 dst_side=Top 但 prev 在 dst 下方）时返回
/// `None`，避免拐点路径穿入 dst 节点。
fn align_tail(
    prev: PointF,
    dst: PointF,
    dst_side: PortSide,
    min_len: f32,
) -> Option<Vec<PointF>> {
    match dst_side {
        PortSide::Top => {
            // 末段垂直 x=dst.x，从上方进入。prev 应在 dst 上方（prev.y < dst.y）。
            if prev.y >= dst.y || (prev.x - dst.x).abs() < 0.5 {
                return None;
            }
            let corner_y = if dst.y - prev.y >= min_len {
                prev.y
            } else {
                dst.y - min_len
            };
            Some(tail_corners(prev.x, dst.x, corner_y, prev.y))
        }
        PortSide::Bottom => {
            if prev.y <= dst.y || (prev.x - dst.x).abs() < 0.5 {
                return None;
            }
            let corner_y = if prev.y - dst.y >= min_len {
                prev.y
            } else {
                dst.y + min_len
            };
            Some(tail_corners(prev.x, dst.x, corner_y, prev.y))
        }
        PortSide::Left => {
            if prev.x >= dst.x || (prev.y - dst.y).abs() < 0.5 {
                return None;
            }
            let corner_x = if dst.x - prev.x >= min_len {
                prev.x
            } else {
                dst.x - min_len
            };
            Some(tail_corners(prev.y, dst.y, corner_x, prev.x))
        }
        PortSide::Right => {
            if prev.x <= dst.x || (prev.y - dst.y).abs() < 0.5 {
                return None;
            }
            let corner_x = if prev.x - dst.x >= min_len {
                prev.x
            } else {
                dst.x + min_len
            };
            Some(tail_corners(prev.y, dst.y, corner_x, prev.x))
        }
        PortSide::Auto => None,
    }
}

/// 终点段拐点构造（统一处理 Top/Bottom 与 Left/Right 两种轴向）。
///
/// 对于垂直入面（Top/Bottom）：`a`=`prev.x`、`b`=`dst.x`、`c`=`corner_y`、
/// `p`=`prev.y`，拐点形如 `(b, c)`，不足最小长度时补 `(a, c)`。
/// 对于水平入面（Left/Right）：`a`=`prev.y`、`b`=`dst.y`、`c`=`corner_x`、
/// `p`=`prev.x`，拐点形如 `(c, b)`，不足时补 `(c, a)`。
fn tail_corners(a: f32, b: f32, c: f32, p: f32) -> Vec<PointF> {
    // 单拐点：corner 与 prev 同轴坐标（c == p），直接连到 dst 入面。
    if (c - p).abs() < 0.5 {
        vec![PointF::new(b, c)]
    } else {
        // 双拐点：prev → (a, c) → (b, c) → dst，末段长 min_len。
        vec![PointF::new(a, c), PointF::new(b, c)]
    }
}

/// 计算起点段对齐所需的拐点（插入在 `src` 与 `second` 之间）。
///
/// 返回 `None` 表示首段已规范；`Some(corners)` 为需插入的拐点序列。
/// 当 `second` 位于 src 出面内侧时返回 `None`，避免穿入 src 节点。
fn align_head(
    src: PointF,
    second: PointF,
    src_side: PortSide,
    min_len: f32,
) -> Option<Vec<PointF>> {
    match src_side {
        PortSide::Right => {
            // 首段水平 y=src.y，向右出。second 应在 src 右侧（second.x > src.x）。
            if second.x <= src.x || (second.y - src.y).abs() < 0.5 {
                return None;
            }
            let corner_x = if second.x - src.x >= min_len {
                second.x
            } else {
                src.x + min_len
            };
            Some(head_corners(src.y, second.y, corner_x, second.x))
        }
        PortSide::Left => {
            if second.x >= src.x || (second.y - src.y).abs() < 0.5 {
                return None;
            }
            let corner_x = if src.x - second.x >= min_len {
                second.x
            } else {
                src.x - min_len
            };
            Some(head_corners(src.y, second.y, corner_x, second.x))
        }
        PortSide::Bottom => {
            if second.y <= src.y || (second.x - src.x).abs() < 0.5 {
                return None;
            }
            let corner_y = if second.y - src.y >= min_len {
                second.y
            } else {
                src.y + min_len
            };
            Some(head_corners(src.x, second.x, corner_y, second.y))
        }
        PortSide::Top => {
            if second.y >= src.y || (second.x - src.x).abs() < 0.5 {
                return None;
            }
            let corner_y = if src.y - second.y >= min_len {
                second.y
            } else {
                src.y - min_len
            };
            Some(head_corners(src.x, second.x, corner_y, second.y))
        }
        PortSide::Auto => None,
    }
}

/// 起点段拐点构造。对于水平出面（Right/Left）：`a`=`src.y`、`b`=`second.y`、
/// `c`=`corner_x`、`p`=`second.x`，拐点形如 `(c, a)`，不足时补 `(c, b)`。
fn head_corners(a: f32, b: f32, c: f32, p: f32) -> Vec<PointF> {
    if (c - p).abs() < 0.5 {
        vec![PointF::new(c, a)]
    } else {
        vec![PointF::new(c, a), PointF::new(c, b)]
    }
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
