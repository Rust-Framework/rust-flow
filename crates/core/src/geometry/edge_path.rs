//! Edge path generation: algorithms ported from ReactFlow (@xyflow/xyflow).
//!
//! All functions return `Vec<PointF>`:
//! - `straight_path` → 2 points.
//! - `bezier_path` → exactly 4 points `[P0, ctrl1, ctrl2, P3]` (cubic Bézier).
//! - `step_path` / `smoothstep_path` → polyline points.
//!   `smoothstep_path` replaces each interior corner with sampled **quadratic**
//!   Bézier curve points (porting ReactFlow's `getBend` which uses SVG `Q` command).
//!
//! `loop_back_path` → polyline for loop-node back-edges (U-shape routing).

use crate::geometry::{PointF, RectF};
use crate::graph::PortSide;

// ---------------------------------------------------------------------------
// Helpers shared by step / smoothstep (ported from ReactFlow)
// ---------------------------------------------------------------------------

/// Unit vector pointing outward from a node for the given side.
fn outward(side: PortSide) -> PointF {
    match side {
        PortSide::Left => PointF::new(-1.0, 0.0),
        PortSide::Right => PointF::new(1.0, 0.0),
        PortSide::Top => PointF::new(0.0, -1.0),
        PortSide::Bottom => PointF::new(0.0, 1.0),
        PortSide::Auto => PointF::new(1.0, 0.0),
    }
}

/// Determine the dominant routing direction between source-gap and target-gap.
///
/// Ported from ReactFlow `getDirection` in `smoothstep-edge.ts`.
fn rf_direction(source: PointF, _source_pos: PortSide, target: PointF) -> PointF {
    // For horizontal handles, direction depends on relative X.
    // For vertical handles, direction depends on relative Y.
    if source.x != target.x {
        if source.x < target.x {
            PointF::new(1.0, 0.0)
        } else {
            PointF::new(-1.0, 0.0)
        }
    } else if source.y < target.y {
        PointF::new(0.0, 1.0)
    } else {
        PointF::new(0.0, -1.0)
    }
}

/// Compute orthogonal routing waypoints.
///
/// Direct port of ReactFlow `getPoints()` from `smoothstep-edge.ts`.
/// Handles:
/// - Opposite handle positions (Right→Left): S-curve with 2 bends
/// - Same-side positions (Right→Right / Left→Left): L-shape or adjusted
/// - Mixed positions (Right→Bottom etc.): L-shape through one corner
/// - Gap-offset prevention when points would overlap
fn rf_get_points(
    src: PointF,
    src_side: PortSide,
    dst: PointF,
    dst_side: PortSide,
    offset: f32,
) -> Vec<PointF> {
    let src_dir = outward(src_side);
    let dst_dir = outward(dst_side);

    let src_gapped = PointF::new(src.x + src_dir.x * offset, src.y + src_dir.y * offset);
    let dst_gapped = PointF::new(dst.x + dst_dir.x * offset, dst.y + dst_dir.y * offset);

    let dir = rf_direction(src_gapped, src_side, dst_gapped);
    let dir_is_x = dir.x != 0.0; // primary axis

    // Helper: extract the primary-axis component of a direction vector.
    let primary = |d: PointF| -> f32 {
        if dir_is_x { d.x } else { d.y }
    };

    let src_d = primary(src_dir);
    let dst_d = primary(dst_dir);
    let curr_d = primary(dir);

    // Compute midpoint(s) based on handle positions.
    let mid_points: Vec<PointF> =
    // ── Case 1: opposite handle positions (product ≈ -1) ──
    if (src_d * dst_d).abs() > 0.999 && src_d * dst_d < 0.0 {
        let (cx, cy) = if dir_is_x {
            (
                src_gapped.x + (dst_gapped.x - src_gapped.x) * 0.5,
                (src_gapped.y + dst_gapped.y) * 0.5,
            )
        } else {
            (
                (src_gapped.x + dst_gapped.x) * 0.5,
                src_gapped.y + (dst_gapped.y - src_gapped.y) * 0.5,
            )
        };

        let v_split = [PointF::new(cx, src_gapped.y), PointF::new(cx, dst_gapped.y)];
        let h_split = [PointF::new(src_gapped.x, cy), PointF::new(dst_gapped.x, cy)];

        if src_d == curr_d {
            if dir_is_x { v_split.to_vec() } else { h_split.to_vec() }
        } else {
            if dir_is_x { h_split.to_vec() } else { v_split.to_vec() }
        }
    }
    // ── Case 2: same-direction or mixed handle positions ──
    else {
        let source_target = PointF::new(src_gapped.x, dst_gapped.y);
        let target_source = PointF::new(dst_gapped.x, src_gapped.y);

        if dir_is_x {
            // Primary axis = X
            let mut mp = if (src_dir.x - curr_d).abs() < 0.001 {
                vec![target_source]
            } else {
                vec![source_target]
            };

            // Same-position special case
            if src_side == dst_side {
                let diff = (src.x - dst.x).abs();
                if diff <= offset && diff > 0.001 {
                    mp = if (src_dir.x - curr_d).abs() < 0.001 {
                        vec![source_target]
                    } else {
                        vec![target_source]
                    };
                }
            }

            // Mixed-position flip logic (e.g., Right→Bottom)
            if src_side != dst_side {
                let is_same_y = (src_dir.y - dst_dir.y).abs() < 0.001;
                let src_gt_opp = src_gapped.y > dst_gapped.y;
                let src_lt_opp = src_gapped.y < dst_gapped.y;

                let should_flip = (src_dir.x > 0.0
                    && ((!is_same_y && src_gt_opp) || (is_same_y && src_lt_opp)))
                    || (src_dir.x <= 0.0
                        && ((!is_same_y && src_lt_opp) || (is_same_y && src_gt_opp)));

                if should_flip {
                    mp = vec![source_target];
                }
            }

            mp
        } else {
            // Primary axis = Y
            let mut mp = if (src_dir.y - curr_d).abs() < 0.001 {
                vec![source_target]
            } else {
                vec![target_source]
            };

            // Same-position special case
            if src_side == dst_side {
                let diff = (src.y - dst.y).abs();
                if diff <= offset && diff > 0.001 {
                    mp = if (src_dir.y - curr_d).abs() < 0.001 {
                        vec![target_source]
                    } else {
                        vec![source_target]
                    };
                }
            }

            // Mixed-position flip logic
            if src_side != dst_side {
                let is_same_x = (src_dir.x - dst_dir.x).abs() < 0.001;
                let src_gt_opp = src_gapped.x > dst_gapped.x;
                let src_lt_opp = src_gapped.x < dst_gapped.x;

                let should_flip = (src_dir.y > 0.0
                    && ((!is_same_x && src_gt_opp) || (is_same_x && src_lt_opp)))
                    || (src_dir.y <= 0.0
                        && ((!is_same_x && src_lt_opp) || (is_same_x && src_gt_opp)));

                if should_flip {
                    mp = vec![target_source];
                }
            }

            mp
        }
    };

    // Assemble final path: [src, gapped_src?, …mid…, gapped_dst?, dst]
    let mut result = Vec::with_capacity(4 + mid_points.len());
    result.push(src);

    // Add gapped source only if it differs from the first midpoint (or there are no midpoints)
    let add_src_gap = mid_points.is_empty()
        || ((src_gapped.x - mid_points[0].x).abs() > 0.01
            || (src_gapped.y - mid_points[0].y).abs() > 0.01);
    if add_src_gap {
        result.push(src_gapped);
    }

    result.extend_from_slice(&mid_points);

    // Add gapped target only if it differs from the last midpoint
    let add_dst_gap = mid_points.is_empty()
        || ((dst_gapped.x - mid_points[mid_points.len() - 1].x).abs() > 0.01
            || (dst_gapped.y - mid_points[mid_points.len() - 1].y).abs() > 0.01);
    if add_dst_gap {
        result.push(dst_gapped);
    }

    result.push(dst);
    result
}

/// Replace a sharp corner (`a → b → c`) with sampled quadratic-Bézier points.
///
/// This is a direct port of ReactFlow's `getBend()` from `smoothstep-edge.ts`.
/// ReactFlow emits an SVG `Q` (quadratic Bézier) command; here we sample the
/// curve into polyline points so the existing canvas renderer can consume them.
///
/// The control point is always at the corner vertex `b`.  The bend goes from
/// a point `bend_size` away along segment `(a→b)` to a point `bend_size` away
/// along segment `(b→c)`, rounding the turn.
fn rf_get_bend(a: PointF, b: PointF, c: PointF, size: f32) -> Vec<PointF> {
    let d_ab = a.distance_to(b);
    let d_bc = b.distance_to(c);
    let bend_size = size.min(d_ab * 0.5).min(d_bc * 0.5);

    // Collinear → no bend needed.
    if ((a.x - b.x).abs() < 0.001 && (b.x - c.x).abs() < 0.001)
        || ((a.y - b.y).abs() < 0.001 && (b.y - c.y).abs() < 0.001)
    {
        return vec![b];
    }

    const SAMPLES: u32 = 8; // points per bend (matches visual smoothness)
    let mut out = Vec::with_capacity(SAMPLES as usize + 2);

    // Determine orientation: is the incoming segment (a→b) horizontal?
    let horiz_in = (a.y - b.y).abs() < (a.x - b.x).abs();

    if horiz_in {
        // Incoming horizontal, outgoing vertical.
        //   a ---[start]--→ [ctrl=b] ↓ [end]
        //                  ╲       │
        //                   ╲      │
        //                    ╲     │
        //                     c ←──┘
        let x_dir = if a.x < c.x { -1.0 } else { 1.0 };
        let y_dir = if a.y < c.y { 1.0 } else { -1.0 };

        let start = PointF::new(b.x + bend_size * x_dir, b.y);
        let ctrl = b;
        let end = PointF::new(b.x, b.y + bend_size * y_dir);

        out.push(start);
        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;
            let u = 1.0 - t;
            out.push(PointF::new(
                u * u * start.x + 2.0 * u * t * ctrl.x + t * t * end.x,
                u * u * start.y + 2.0 * u * t * ctrl.y + t * t * end.y,
            ));
        }
        out.push(end);
    } else {
        // Incoming vertical, outgoing horizontal.
        //   a
        //   │
        //   │  [start]
        //   │    ╲
        //   │     ╲  [ctrl=b] ——→ [end]
        //   │      ╲            │
        //   └───────c ←────────┘
        let x_dir = if a.x < c.x { 1.0 } else { -1.0 };
        let y_dir = if a.y < c.y { -1.0 } else { 1.0 };

        let start = PointF::new(b.x, b.y + bend_size * y_dir);
        let ctrl = b;
        let end = PointF::new(b.x + bend_size * x_dir, b.y);

        out.push(start);
        for i in 1..SAMPLES {
            let t = i as f32 / SAMPLES as f32;
            let u = 1.0 - t;
            out.push(PointF::new(
                u * u * start.x + 2.0 * u * t * ctrl.x + t * t * end.x,
                u * u * start.y + 2.0 * u * t * ctrl.y + t * t * end.y,
            ));
        }
        out.push(end);
    }

    out
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Bézier control-point offset (ported from ReactFlow `calculateControlOffset`).
///
/// Normal connection (distance ≥ 0): half the distance.
/// Reverse connection (distance < 0): `curvature * 25 * sqrt(-distance)`,
/// preventing control-point collapse when target is behind source.
fn control_offset(distance: f32, curvature: f32) -> f32 {
    if distance >= 0.0 {
        0.5 * distance
    } else {
        curvature * 25.0 * (-distance).sqrt()
    }
}

// ---- Straight ----

/// Straight line: `[src, dst]`.
pub fn straight_path(src: PointF, dst: PointF) -> Vec<PointF> {
    vec![src, dst]
}

// ---- Bézier (cubic) ----

/// Cubic Bézier path. Returns `[P0, ctrl1, ctrl2, P3]`.
///
/// Each control point is offset from its endpoint along the endpoint's outward
/// side direction. The offset magnitude follows ReactFlow's `calculateControlOffset`.
pub fn bezier_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    curvature: f32,
) -> Vec<PointF> {
    let ctrl1 = bezier_control(src, dst, src_side, true, curvature);
    let ctrl2 = bezier_control(dst, src, dst_side, false, curvature);
    vec![src, ctrl1, ctrl2, dst]
}

/// Compute one Bézier control point.
fn bezier_control(
    point: PointF,
    other: PointF,
    side: PortSide,
    is_source: bool,
    curvature: f32,
) -> PointF {
    let dir = outward(side);
    let distance = if side.is_horizontal() {
        if is_source { other.x - point.x } else { point.x - other.x }
    } else {
        if is_source { other.y - point.y } else { point.y - other.y }
    };
    let offset = control_offset(distance, curvature);
    PointF::new(point.x + dir.x * offset, point.y + dir.y * offset)
}

// ---- Step (sharp orthogonal) ----

/// Step path: orthogonal with sharp 90° corners.
///
/// Uses ReactFlow's `getPoints()` routing algorithm with zero border radius.
pub fn step_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
) -> Vec<PointF> {
    rf_get_points(src, src_side, dst, dst_side, 20.0)
}

// ---- SmoothStep (rounded orthogonal) ----

/// Replace each sharp interior corner of a polyline with sampled quadratic
/// Bézier points, producing rounded corners.
///
/// This is the reusable corner-rounding logic extracted from
/// [`smoothstep_path`]. It applies [`rf_get_bend`] to every interior point
/// (indices `1..len-1`) of the input polyline.
///
/// - If `points` has fewer than 3 elements or `border_radius <= 0`, the
///   input is returned unchanged.
/// - The first and last points are always preserved exactly.
pub fn round_corners(points: &[PointF], border_radius: f32) -> Vec<PointF> {
    if points.len() < 3 || border_radius <= 0.0 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(points.len() * 10);
    result.push(points[0]);

    for i in 1..points.len() - 1 {
        let bend_pts = rf_get_bend(points[i - 1], points[i], points[i + 1], border_radius);
        result.extend_from_slice(&bend_pts);
    }

    result.push(*points.last().unwrap());
    result
}

/// SmoothStep path: orthogonal with rounded corners.
///
/// Uses ReactFlow's `getPoints()` for routing + `getBend()` for each corner.
/// Each interior corner is replaced by a sampled quadratic Bézier curve whose
/// control point sits at the corner vertex — exactly matching ReactFlow's SVG
/// `Q` command output.
pub fn smoothstep_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    border_radius: f32,
) -> Vec<PointF> {
    let raw = rf_get_points(src, src_side, dst, dst_side, 20.0);
    round_corners(&raw, border_radius)
}

// ---------------------------------------------------------------------------
// Loop-back (U-shape)
// ---------------------------------------------------------------------------

/// Loop back-edge routing: orthogonal U-shape path from the last loop body
/// node back to the Loop node's `loop_in` port.
/// **Both layouts** use the same 5-point path (routes BELOW the body group):
/// `src → (src.x, bottom_y) → (approach_x, bottom_y) → (approach_x, dst.y) → dst`
/// Goes DOWN → LEFT → UP → RIGHT.
///
/// - Source exits from its BOTTOM side（下出，循环体节点始终纵向编排）
/// - Path loops below the body group
/// - Enters the Loop's `loop_in` port from the LEFT（左进）
///
/// `node_bounds` should include the loop body area (Loop node + all loop body
/// nodes), so the path clears everything when routing around.
///
/// `horizontal` parameter is kept for API compatibility but no longer affects
/// the path — both layouts use the same below-routing algorithm.
pub fn loop_back_path(
    src: PointF,
    dst: PointF,
    _horizontal: bool,
    node_bounds: crate::geometry::RectF,
) -> Vec<PointF> {
    // approach_offset must exceed border_radius (12) + arrow_size (8) = 20.
    // Using 30 gives a 18px final segment after rounding, ample for the arrow.
    let approach_offset = 30.0;

    // Both layouts: DOWN → LEFT → UP → RIGHT (5-point U-shape below body group)
    let bottom_margin = 40.0;
    // bottom_y must be below both the node bounds and the source point
    let bottom_y = node_bounds.bottom().max(src.y) + bottom_margin;
    let approach_x = dst.x - approach_offset;
    vec![
        src,
        PointF::new(src.x, bottom_y),
        PointF::new(approach_x, bottom_y),
        PointF::new(approach_x, dst.y),
        dst,
    ]
}

// ---------------------------------------------------------------------------
// Channel-based obstacle avoidance routing (基于 dagre rank 的通道分配)
// ---------------------------------------------------------------------------

/// 通道与节点的安全间距（逻辑像素）。
const CHANNEL_MARGIN: f32 = 30.0;

/// 基于 dagre rank 的通道分配正交避障路由。
///
/// 对于跨层边（跨越 2+ 个 rank），在中间 rank 的节点间隙中分配通道，
/// 生成避障正交路径。对于无障碍的边，回退到 smoothstep 路由。
///
/// 参数：
/// - `src`, `dst`: 源/目标端口坐标
/// - `src_side`, `dst_side`: 源/目标端口侧
/// - `obstacles_by_rank`: 中间层节点矩形列表（已排除 src 和 dst 节点），按 rank 分组
/// - `horizontal`: true=横向布局（通道为 Y 坐标），false=纵向布局（通道为 X 坐标）
/// - `border_radius`: 圆角半径（0.0 = 直角）
///
/// 返回：正交路径点列表（已圆角化）
pub fn route_with_channels(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    obstacles_by_rank: &[Vec<RectF>],
    horizontal: bool,
    border_radius: f32,
) -> Vec<PointF> {
    // 无中间层障碍 → 回退到 smoothstep
    if obstacles_by_rank.is_empty() || obstacles_by_rank.iter().all(|v| v.is_empty()) {
        return smoothstep_path(src, dst, src_side, dst_side, border_radius);
    }

    // 对每个中间层，找到通道坐标和过渡坐标
    let channels = find_channels(src, dst, obstacles_by_rank, horizontal);

    // 生成正交路径
    let raw_path = build_orthogonal_path(src, dst, &channels, horizontal);

    // 圆角化
    round_corners(&raw_path, border_radius)
}

/// 在中间层节点间隙中查找通道坐标和过渡坐标。
///
/// 返回 `Vec<(transition, channel)>`：
/// - **横向布局**：`transition` = 过渡 X（路径在此处垂直切换到通道 Y），
///   `channel` = 通道 Y（避开该层障碍物的 Y 坐标）
/// - **纵向布局**：`transition` = 过渡 Y，`channel` = 通道 X
///
/// 过渡坐标选择障碍物的前导边缘（沿 flow 方向），确保水平/垂直段
/// 不穿过障碍物。空层使用均匀分布的过渡坐标 + 自然路径通道坐标。
fn find_channels(
    src: PointF,
    dst: PointF,
    obstacles_by_rank: &[Vec<RectF>],
    horizontal: bool,
) -> Vec<(f32, f32)> {
    let n = obstacles_by_rank.len();
    let mut result = Vec::with_capacity(n);

    // flow 方向：横向看 X，纵向看 Y
    let flow_l2r = if horizontal {
        src.x < dst.x
    } else {
        src.y < dst.y
    };

    for (i, rank_obstacles) in obstacles_by_rank.iter().enumerate() {
        // 自然路径在该层的插值参数 t
        let t = (i + 1) as f32 / (n + 1) as f32;
        let desired = if horizontal {
            src.y + (dst.y - src.y) * t
        } else {
            src.x + (dst.x - src.x) * t
        };

        // ===== 计算通道坐标（避开障碍物的交叉轴坐标） =====
        let channel = if rank_obstacles.is_empty() {
            // 该层无节点，用自然路径坐标
            desired
        } else {
            // 按交叉轴坐标排序节点，收集 (min, max) 区间
            let mut sorted: Vec<(f32, f32)> = rank_obstacles
                .iter()
                .map(|r| {
                    if horizontal {
                        (r.top(), r.bottom())
                    } else {
                        (r.left(), r.right())
                    }
                })
                .collect();
            sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            // 收集所有间隙 (lo, hi)
            let mut gaps: Vec<(f32, f32)> = Vec::with_capacity(sorted.len() + 1);
            gaps.push((f32::MIN, sorted[0].0 - CHANNEL_MARGIN));
            for w in sorted.windows(2) {
                gaps.push((w[0].1 + CHANNEL_MARGIN, w[1].0 - CHANNEL_MARGIN));
            }
            gaps.push((sorted.last().unwrap().1 + CHANNEL_MARGIN, f32::MAX));

            // 找到离 desired 最近的间隙
            gaps.iter()
                .map(|(lo, hi)| {
                    if desired >= *lo && desired <= *hi {
                        desired
                    } else {
                        if *lo == f32::MIN {
                            *hi
                        } else if *hi == f32::MAX {
                            *lo
                        } else {
                            (*lo + *hi) * 0.5
                        }
                    }
                })
                .min_by(|a, b| {
                    (a - desired)
                        .abs()
                        .partial_cmp(&(b - desired).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(desired)
        };

        // ===== 计算过渡坐标（障碍物前导边缘，确保不穿过障碍物） =====
        let transition = if rank_obstacles.is_empty() {
            // 空层：均匀分布
            if horizontal {
                src.x + (dst.x - src.x) * t
            } else {
                src.y + (dst.y - src.y) * t
            }
        } else {
            // 有障碍物：取前导边缘（沿 flow 方向的入口侧）
            if horizontal {
                if flow_l2r {
                    // 左→右：过渡 X 在障碍物左侧
                    let min_left = rank_obstacles.iter().map(|r| r.left()).fold(f32::MAX, f32::min);
                    min_left - CHANNEL_MARGIN
                } else {
                    // 右→左：过渡 X 在障碍物右侧
                    let max_right = rank_obstacles.iter().map(|r| r.right()).fold(f32::MIN, f32::max);
                    max_right + CHANNEL_MARGIN
                }
            } else {
                if flow_l2r {
                    // 上→下：过渡 Y 在障碍物上方
                    let min_top = rank_obstacles.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
                    min_top - CHANNEL_MARGIN
                } else {
                    // 下→上：过渡 Y 在障碍物下方
                    let max_bottom = rank_obstacles.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
                    max_bottom + CHANNEL_MARGIN
                }
            }
        };

        result.push((transition, channel));
    }

    result
}

/// 根据通道坐标构建正交路径。
///
/// `channels` 为 `[(transition, channel)]` 对：
/// - 横向布局：`transition` = 过渡 X，`channel` = 通道 Y
/// - 纵向布局：`transition` = 过渡 Y，`channel` = 通道 X
fn build_orthogonal_path(
    src: PointF,
    dst: PointF,
    channels: &[(f32, f32)],
    horizontal: bool,
) -> Vec<PointF> {
    if channels.is_empty() {
        return vec![src, dst];
    }

    if horizontal {
        build_orthogonal_horizontal(src, dst, channels)
    } else {
        build_orthogonal_vertical(src, dst, channels)
    }
}

/// 横向布局正交路径构建。
///
/// 通道为 Y 坐标，过渡点为 X 坐标。路径模式（H/V 交替，保证正交）：
/// ```text
/// src → (tx0, src.y) → (tx0, ch0) → (tx1, ch0) → (tx1, ch1) → ... → (dst.x, chN) → dst
/// ```
fn build_orthogonal_horizontal(src: PointF, dst: PointF, channels: &[(f32, f32)]) -> Vec<PointF> {
    let n = channels.len();
    if n == 0 {
        return vec![src, dst];
    }

    let mut points = Vec::with_capacity(n * 2 + 2);
    points.push(src);

    let mut prev_y = src.y;
    for &(tx, cy) in channels {
        // 水平移动到过渡 X（保持前一通道 Y）
        points.push(PointF::new(tx, prev_y));
        // 垂直移动到当前通道 Y
        points.push(PointF::new(tx, cy));
        prev_y = cy;
    }

    // 水平移动到 dst.x，再垂直移动到 dst.y
    points.push(PointF::new(dst.x, prev_y));
    points.push(dst);

    dedup_consecutive(&mut points);
    points
}

/// 纵向布局正交路径构建。
///
/// 通道为 X 坐标，过渡点为 Y 坐标。路径模式（V/H 交替，保证正交）：
/// ```text
/// src → (src.x, ty0) → (ch0, ty0) → (ch0, ty1) → (ch1, ty1) → ... → (chN, dst.y) → dst
/// ```
fn build_orthogonal_vertical(src: PointF, dst: PointF, channels: &[(f32, f32)]) -> Vec<PointF> {
    let n = channels.len();
    if n == 0 {
        return vec![src, dst];
    }

    let mut points = Vec::with_capacity(n * 2 + 2);
    points.push(src);

    let mut prev_x = src.x;
    for &(ty, cx) in channels {
        // 垂直移动到过渡 Y（保持前一通道 X）
        points.push(PointF::new(prev_x, ty));
        // 水平移动到当前通道 X
        points.push(PointF::new(cx, ty));
        prev_x = cx;
    }

    // 垂直移动到 dst.y，再水平移动到 dst.x
    points.push(PointF::new(prev_x, dst.y));
    points.push(dst);

    dedup_consecutive(&mut points);
    points
}

/// 去除连续重复的点。
fn dedup_consecutive(points: &mut Vec<PointF>) {
    if points.len() < 2 {
        return;
    }
    let mut write = 1;
    for read in 1..points.len() {
        let prev = points[write - 1];
        let curr = points[read];
        if (prev.x - curr.x).abs() > 0.01 || (prev.y - curr.y).abs() > 0.01 {
            points[write] = points[read];
            write += 1;
        }
    }
    points.truncate(write);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_two_points() {
        let pts = straight_path(PointF::new(0.0, 0.0), PointF::new(10.0, 10.0));
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0], PointF::new(0.0, 0.0));
        assert_eq!(pts[1], PointF::new(10.0, 10.0));
    }

    #[test]
    fn bezier_returns_four_points() {
        let pts = bezier_path(
            PointF::new(0.0, 0.0),
            PointF::new(200.0, 0.0),
            PortSide::Right,
            PortSide::Left,
            0.25,
        );
        assert_eq!(pts.len(), 4, "bezier must return [P0, ctrl1, ctrl2, P3]");
        assert_eq!(pts[0], PointF::new(0.0, 0.0));
        assert_eq!(pts[3], PointF::new(200.0, 0.0));
        // ctrl1 should be to the right of src.
        assert!(pts[1].x > 0.0);
        // ctrl2 should be to the left of dst.
        assert!(pts[2].x < 200.0);
    }

    #[test]
    fn step_opposite_horizontal() {
        let pts = step_path(
            PointF::new(0.0, 0.0),
            PointF::new(200.0, 100.0),
            PortSide::Right,
            PortSide::Left,
        );
        // src, p1, mid1, mid2, p2, dst = 6 points
        assert_eq!(pts.len(), 6);
        assert_eq!(pts.first().unwrap().x, 0.0);
        assert_eq!(pts.last().unwrap().x, 200.0);
    }

    #[test]
    fn smoothstep_more_points_than_step() {
        let step = step_path(
            PointF::new(0.0, 0.0),
            PointF::new(200.0, 100.0),
            PortSide::Right,
            PortSide::Left,
        );
        let smooth = smoothstep_path(
            PointF::new(0.0, 0.0),
            PointF::new(200.0, 100.0),
            PortSide::Right,
            PortSide::Left,
            8.0,
        );
        // Rounded corners insert extra arc-sample points.
        assert!(smooth.len() > step.len());
        // Endpoints preserved.
        assert_eq!(*smooth.first().unwrap(), PointF::new(0.0, 0.0));
        assert_eq!(*smooth.last().unwrap(), PointF::new(200.0, 100.0));
    }

    #[test]
    fn loop_back_horizontal_routes_below() {
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        // Horizontal layout: path must route BELOW the node bounds.
        let pts = loop_back_path(
            PointF::new(300.0, 140.0),
            PointF::new(100.0, 140.0),
            true,
            bounds,
        );
        let bottom = bounds.bottom();
        let has_below = pts.iter().any(|p| p.y >= bottom);
        assert!(has_below, "horizontal U-shape must route below the node");
        // Endpoints preserved.
        assert_eq!(*pts.first().unwrap(), PointF::new(300.0, 140.0));
        assert_eq!(*pts.last().unwrap(), PointF::new(100.0, 140.0));
    }

    #[test]
    fn loop_back_vertical_routes_below() {
        // Both layouts now route BELOW (5-point path).
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        let src = PointF::new(190.0, 300.0);
        let dst = PointF::new(100.0, 140.0);
        let pts = loop_back_path(src, dst, false, bounds);
        let bottom = bounds.bottom().max(src.y);
        let has_below = pts.iter().any(|p| p.y >= bottom);
        assert!(has_below, "vertical U-shape must route below the node");
        // 5 points for both layouts
        assert_eq!(pts.len(), 5, "vertical path should have 5 points (down→left→up→right)");
        // Endpoints preserved.
        assert_eq!(*pts.first().unwrap(), src);
        assert_eq!(*pts.last().unwrap(), dst);
    }

    #[test]
    fn loop_back_clears_source_below_node() {
        // Source (last loop body node) is below the Loop node — path must go
        // even lower to clear it.
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        let src = PointF::new(300.0, 300.0); // well below the node (bottom=180)
        let pts = loop_back_path(src, PointF::new(100.0, 140.0), true, bounds);
        let max_y = pts.iter().map(|p| p.y).fold(0.0f32, f32::max);
        assert!(max_y > src.y, "path must go below the source point");
    }

    #[test]
    fn loop_back_horizontal_exits_bottom() {
        // Horizontal layout: src exits BOTTOM (下出), path has a downward
        // first segment before going LEFT. 5-point U-shape.
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        let src = PointF::new(400.0, 140.0); // right of bounds (right=280)
        let pts = loop_back_path(src, PointF::new(100.0, 140.0), true, bounds);
        // 5 points: src, (src.x, bottom_y), (approach_x, bottom_y),
        // (approach_x, dst.y), dst
        assert_eq!(pts.len(), 5, "horizontal path should have 5 points (down→left→up→right)");
        // Second point should be directly below src (same x) — "bottom" exit segment.
        assert_eq!(pts[1].x, src.x, "second point should be at same x as src (downward exit)");
        assert!(pts[1].y > src.y, "second point should be below src");
    }

    #[test]
    fn loop_back_vertical_exits_bottom() {
        // Both layouts: src exits BOTTOM (下出), 5-point U-shape.
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        let src = PointF::new(190.0, 300.0);
        let dst = PointF::new(100.0, 140.0);
        let pts = loop_back_path(src, dst, false, bounds);
        // 5 points: src, (src.x, bottom_y), (approach_x, bottom_y),
        // (approach_x, dst.y), dst
        assert_eq!(pts.len(), 5, "vertical path should have 5 points");
        // Second point should be directly below src (same x) — "bottom" exit.
        assert_eq!(pts[1].x, src.x, "second point should be at same x as src");
        assert!(pts[1].y > src.y, "second point should be below src");
    }

    #[test]
    fn round_corners_preserves_endpoints_and_adds_points() {
        // L-shape polyline: (0,0) → (100,0) → (100,100)
        let raw = vec![PointF::new(0.0, 0.0), PointF::new(100.0, 0.0), PointF::new(100.0, 100.0)];
        let rounded = round_corners(&raw, 12.0);
        // Endpoints preserved.
        assert_eq!(*rounded.first().unwrap(), PointF::new(0.0, 0.0));
        assert_eq!(*rounded.last().unwrap(), PointF::new(100.0, 100.0));
        // Rounded version has more points than raw (bend sampling).
        assert!(rounded.len() > raw.len(), "rounded should have more points");
    }

    #[test]
    fn round_corners_passthrough_for_short_polyline() {
        // 2-point polyline → returned unchanged (no interior corners).
        let raw = vec![PointF::new(0.0, 0.0), PointF::new(10.0, 10.0)];
        let rounded = round_corners(&raw, 12.0);
        assert_eq!(rounded, raw);
    }

    #[test]
    fn round_corners_passthrough_for_zero_radius() {
        let raw = vec![
            PointF::new(0.0, 0.0),
            PointF::new(100.0, 0.0),
            PointF::new(100.0, 100.0),
        ];
        let rounded = round_corners(&raw, 0.0);
        assert_eq!(rounded, raw);
    }

    #[test]
    fn loop_back_path_round_corners_produces_bends() {
        // Simulate realistic loop-back geometry from the demo:
        // Loop node at (100, 100) size (180, 120), Process body node below.
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 120.0),
        );
        let src = PointF::new(190.0, 300.0); // bottom-center of Process node
        let dst = PointF::new(100.0, 140.0); // left port of Loop node

        let raw = loop_back_path(src, dst, false, bounds);
        // 5-point U-shape: src → down → left → up → right → dst
        assert_eq!(raw.len(), 5);

        // All segments must be long enough to accommodate bend_size=12.
        for i in 1..raw.len() {
            let d = raw[i - 1].distance_to(raw[i]);
            assert!(d >= 30.0, "segment {}→{} too short ({}), need >= 30 for radius 12", i - 1, i, d);
        }

        let rounded = round_corners(&raw, 12.0);
        // 3 corners × ~10 sample points each → significantly more than 5.
        assert!(
            rounded.len() > raw.len() + 10,
            "rounded should have significantly more points than raw ({}), got {}",
            raw.len(),
            rounded.len()
        );
        // Endpoints preserved.
        assert_eq!(*rounded.first().unwrap(), src);
        assert_eq!(*rounded.last().unwrap(), dst);
    }

    // ===== route_with_channels / build_orthogonal_path tests =====

    /// 辅助：构造矩形障碍物。
    fn make_rect(x: f32, y: f32, w: f32, h: f32) -> RectF {
        RectF::new(PointF::new(x, y), crate::geometry::SizeF::new(w, h))
    }

    /// 辅助：验证路径中所有中间点（排除 src/dst）不在任何障碍物矩形内。
    fn assert_path_avoids_obstacles(path: &[PointF], obstacles: &[Vec<RectF>]) {
        for p in &path[1..path.len() - 1] {
            for rank_obs in obstacles {
                for obs in rank_obs {
                    assert!(
                        !obs.contains(*p),
                        "point ({:.1}, {:.1}) is inside obstacle {:?}",
                        p.x,
                        p.y,
                        obs
                    );
                }
            }
        }
    }

    /// 辅助：验证路径端点保留（首=src，尾=dst）。
    fn assert_endpoints(path: &[PointF], src: PointF, dst: PointF) {
        assert_eq!(*path.first().unwrap(), src, "first point must be src");
        assert_eq!(*path.last().unwrap(), dst, "last point must be dst");
    }

    /// 辅助：验证路径正交性（相邻点共享 X 或 Y 坐标）。
    fn assert_orthogonal(path: &[PointF]) {
        for w in path.windows(2) {
            let same_x = (w[0].x - w[1].x).abs() < 0.01;
            let same_y = (w[0].y - w[1].y).abs() < 0.01;
            assert!(
                same_x || same_y,
                "segment ({:.1},{:.1})→({:.1},{:.1}) is not orthogonal",
                w[0].x,
                w[0].y,
                w[1].x,
                w[1].y
            );
        }
    }

    #[test]
    fn route_no_obstacles_falls_back_to_smoothstep() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(400.0, 100.0);
        let obstacles: Vec<Vec<RectF>> = vec![];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            12.0,
        );
        let smooth = smoothstep_path(src, dst, PortSide::Right, PortSide::Left, 12.0);

        assert_eq!(routed, smooth, "no obstacles should fall back to smoothstep");
    }

    #[test]
    fn route_all_empty_ranks_falls_back_to_smoothstep() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(400.0, 100.0);
        let obstacles: Vec<Vec<RectF>> = vec![vec![], vec![]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            12.0,
        );
        let smooth = smoothstep_path(src, dst, PortSide::Right, PortSide::Left, 12.0);

        assert_eq!(
            routed, smooth,
            "all empty ranks should fall back to smoothstep"
        );
    }

    #[test]
    fn route_single_rank_obstacle_horizontal_avoids_obstacle() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(400.0, 100.0);
        // Obstacle blocks the natural path Y=100: top=80, bottom=140
        let obstacle = make_rect(180.0, 80.0, 40.0, 60.0);
        let obstacles = vec![vec![obstacle]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
        assert_orthogonal(&routed);

        // CHANNEL_MARGIN=30: gaps are (-inf, 50) and (170, inf)
        // desired=100, nearest gap center is 50 (above) or 170 (below)
        let channel_ys: Vec<f32> = routed
            .iter()
            .map(|p| p.y)
            .filter(|&y| (y - 50.0).abs() < 1.0 || (y - 170.0).abs() < 1.0)
            .collect();
        assert!(
            !channel_ys.is_empty(),
            "path must route through a channel (Y≈50 or Y≈170)"
        );
    }

    #[test]
    fn route_single_rank_obstacle_vertical_avoids_obstacle() {
        let src = PointF::new(100.0, 0.0);
        let dst = PointF::new(100.0, 400.0);
        // Obstacle blocks the natural path X=100: left=80, right=140
        let obstacle = make_rect(80.0, 180.0, 60.0, 40.0);
        let obstacles = vec![vec![obstacle]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Bottom,
            PortSide::Top,
            &obstacles,
            false,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
        assert_orthogonal(&routed);

        // CHANNEL_MARGIN=30: gaps are (-inf, 50) and (170, inf)
        // desired=100, nearest gap center is 50 (left) or 170 (right)
        let channel_xs: Vec<f32> = routed
            .iter()
            .map(|p| p.x)
            .filter(|&x| (x - 50.0).abs() < 1.0 || (x - 170.0).abs() < 1.0)
            .collect();
        assert!(
            !channel_xs.is_empty(),
            "path must route through a channel (X≈50 or X≈170)"
        );
    }

    #[test]
    fn route_multi_rank_obstacles_horizontal() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(600.0, 100.0);
        // Two intermediate ranks, each with an obstacle blocking Y=100
        let obs1 = make_rect(180.0, 80.0, 40.0, 60.0);
        let obs2 = make_rect(380.0, 80.0, 40.0, 60.0);
        let obstacles = vec![vec![obs1], vec![obs2]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
        assert_orthogonal(&routed);

        // Multi-rank path must have intermediate routing points
        assert!(
            routed.len() > 4,
            "multi-rank path should have more than 4 points, got {}",
            routed.len()
        );
    }

    #[test]
    fn route_channel_uses_natural_path_when_unobstructed() {
        // Natural path Y=50 doesn't intersect obstacle (Y range [100, 160])
        let src = PointF::new(0.0, 50.0);
        let dst = PointF::new(400.0, 50.0);
        let obstacle = make_rect(180.0, 100.0, 40.0, 60.0);
        let obstacles = vec![vec![obstacle]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );

        assert_endpoints(&routed, src, dst);

        // desired Y=50 is in gap (-inf, 70), so channel should be 50
        let has_natural_y = routed.iter().any(|p| (p.y - 50.0).abs() < 0.01);
        assert!(
            has_natural_y,
            "channel should use natural Y=50 when unobstructed"
        );
    }

    #[test]
    fn route_two_obstacles_same_rank_uses_gap_between() {
        // Two obstacles in same rank with a gap between them at Y≈100
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(600.0, 100.0);
        // Obs1: Y [20, 60], Obs2: Y [140, 180]
        // Gap with MARGIN=30: (90, 110) → center=100
        let obs1 = make_rect(180.0, 20.0, 40.0, 40.0);
        let obs2 = make_rect(180.0, 140.0, 40.0, 40.0);
        let obstacles = vec![vec![obs1, obs2]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);

        // Natural Y=100 is in the gap (90, 110), channel should be 100
        let has_gap_y = routed.iter().any(|p| (p.y - 100.0).abs() < 1.0);
        assert!(
            has_gap_y,
            "channel should use Y=100 (gap between two obstacles)"
        );
    }

    #[test]
    fn route_empty_rank_among_obstacles_uses_natural() {
        // Mix: first rank has obstacle, second rank is empty
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(600.0, 100.0);
        let obstacle = make_rect(180.0, 80.0, 40.0, 60.0);
        let obstacles = vec![vec![obstacle], vec![]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
        assert_orthogonal(&routed);

        // Second rank (empty) should use natural interpolation
        // t = 2/(2+1) ≈ 0.667, desired Y = 100 (src.y == dst.y)
        // So second channel Y should be 100
        let has_natural_y = routed.iter().any(|p| (p.y - 100.0).abs() < 0.01);
        assert!(
            has_natural_y,
            "empty rank should use natural Y=100"
        );
    }

    #[test]
    fn route_rounded_corners_adds_points() {
        let src = PointF::new(0.0, 100.0);
        let dst = PointF::new(400.0, 100.0);
        let obstacle = make_rect(180.0, 80.0, 40.0, 60.0);
        let obstacles = vec![vec![obstacle]];

        let raw = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            0.0,
        );
        let rounded = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            12.0,
        );

        assert!(
            rounded.len() > raw.len(),
            "rounded ({} pts) should have more points than raw ({} pts)",
            rounded.len(),
            raw.len()
        );
        assert_endpoints(&rounded, src, dst);
    }

    #[test]
    fn route_preserves_endpoints_with_offset_src_dst() {
        // src and dst at different Y values, obstacle in between
        let src = PointF::new(10.0, 50.0);
        let dst = PointF::new(500.0, 200.0);
        let obstacle = make_rect(200.0, 80.0, 40.0, 80.0);
        let obstacles = vec![vec![obstacle]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Right,
            PortSide::Left,
            &obstacles,
            true,
            12.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
    }

    #[test]
    fn route_vertical_multi_rank_obstacles() {
        let src = PointF::new(100.0, 0.0);
        let dst = PointF::new(100.0, 600.0);
        let obs1 = make_rect(80.0, 180.0, 60.0, 40.0);
        let obs2 = make_rect(80.0, 380.0, 60.0, 40.0);
        let obstacles = vec![vec![obs1], vec![obs2]];

        let routed = route_with_channels(
            src,
            dst,
            PortSide::Bottom,
            PortSide::Top,
            &obstacles,
            false,
            0.0,
        );

        assert_endpoints(&routed, src, dst);
        assert_path_avoids_obstacles(&routed, &obstacles);
        assert_orthogonal(&routed);
        assert!(
            routed.len() > 4,
            "vertical multi-rank path should have more than 4 points, got {}",
            routed.len()
        );
    }
}
