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

use crate::geometry::PointF;
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

    // Apply getBend to every interior point (indices 1..len-1).
    if raw.len() < 3 || border_radius <= 0.0 {
        return raw;
    }

    let mut result = Vec::with_capacity(raw.len() * 10);
    result.push(raw[0]);

    for i in 1..raw.len() - 1 {
        let bend_pts = rf_get_bend(raw[i - 1], raw[i], raw[i + 1], border_radius);
        result.extend_from_slice(&bend_pts);
    }

    result.push(*raw.last().unwrap());
    result
}

// ---------------------------------------------------------------------------
// Loop-back (U-shape)
// ---------------------------------------------------------------------------

/// Loop-back path: U-shape routing **below** `node_bounds` for loop nodes.
///
/// Always routes below the node (regardless of main layout direction):
/// `src → right → down → left → up → dst`
///
/// `node_bounds` should include the loop body area (Loop node + all loop body
/// nodes), so the path clears everything when routing below.
///
/// **Design rationale**: the loop body chain is always vertical (top-in /
/// bottom-out), arranged downward from `loop_body` (right side of Loop node).
/// The back-edge from the last loop body node's bottom port must go down,
/// around, and back up to `loop_in` (left side of Loop node), never crossing
/// through the node.
pub fn loop_back_path(
    src: PointF,
    dst: PointF,
    _horizontal: bool, // ignored — always route below
    node_bounds: crate::geometry::RectF,
) -> Vec<PointF> {
    let margin = 40.0;
    // bottom_y must be below both the node bounds and the source point
    // (source is the last loop body node's bottom port, which may be below
    // the Loop node itself).
    let bottom_y = node_bounds.bottom().max(src.y) + margin;
    let right_x = src.x + margin;
    let left_x = dst.x - margin;
    vec![
        src,
        PointF::new(right_x, src.y),
        PointF::new(right_x, bottom_y),
        PointF::new(left_x, bottom_y),
        PointF::new(left_x, dst.y),
        dst,
    ]
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
    fn loop_back_always_routes_below() {
        let bounds = crate::geometry::RectF::new(
            PointF::new(100.0, 100.0),
            crate::geometry::SizeF::new(180.0, 80.0),
        );
        // Test both horizontal=true and horizontal=false — both must route below.
        for horizontal in [true, false] {
            let pts = loop_back_path(
                PointF::new(300.0, 140.0),
                PointF::new(100.0, 140.0),
                horizontal,
                bounds,
            );
            // The bottom routing segment must be below the node.
            let bottom = bounds.bottom();
            let has_below = pts.iter().any(|p| p.y >= bottom);
            assert!(has_below, "U-shape must route below the node (horizontal={horizontal})");
            // Endpoints preserved.
            assert_eq!(*pts.first().unwrap(), PointF::new(300.0, 140.0));
            assert_eq!(*pts.last().unwrap(), PointF::new(100.0, 140.0));
        }
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
}
