//! Edge path generation: 4 algorithms ported from ReactFlow.
//!
//! All functions return `Vec<PointF>`:
//! - `straight_path` / `step_path` / `smoothstep_path` → polyline points.
//! - `bezier_path` → exactly 4 points `[P0, ctrl1, ctrl2, P3]` (cubic Bézier);
//!   the renderer uses `curve_to` when `EdgeType::Bezier`.
//!
//! `loop_back_path` → polyline for loop-node back-edges (U-shape routing).

use crate::geometry::PointF;
use crate::graph::PortSide;

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

// ---------------------------------------------------------------------------
// Straight
// ---------------------------------------------------------------------------

/// Straight line: `[src, dst]`.
pub fn straight_path(src: PointF, dst: PointF) -> Vec<PointF> {
    vec![src, dst]
}

// ---------------------------------------------------------------------------
// Bézier (cubic)
// ---------------------------------------------------------------------------

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
/// `point` is the endpoint, `other` is the opposite endpoint.
/// `is_source` distinguishes source (distance = other - point) vs target.
fn bezier_control(
    point: PointF,
    other: PointF,
    side: PortSide,
    is_source: bool,
    curvature: f32,
) -> PointF {
    let dir = outward(side);
    // distance from `point` towards `other` along the side's axis
    let distance = if side.is_horizontal() {
        if is_source {
            other.x - point.x
        } else {
            point.x - other.x
        }
    } else {
        if is_source {
            other.y - point.y
        } else {
            point.y - other.y
        }
    };
    let offset = control_offset(distance, curvature);
    PointF::new(point.x + dir.x * offset, point.y + dir.y * offset)
}

// ---------------------------------------------------------------------------
// Step / SmoothStep (orthogonal)
// ---------------------------------------------------------------------------

/// Orthogonal corner points shared by `step_path` and `smoothstep_path`.
///
/// Exits `src` along `src_side` by `offset`, enters `dst` along `dst_side`,
/// connecting with at most 2 bends.
fn orthogonal_points(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    offset: f32,
) -> Vec<PointF> {
    let src_out = outward(src_side);
    let dst_out = outward(dst_side);
    // Exit source and approach target along their outward directions.
    let p1 = PointF::new(src.x + src_out.x * offset, src.y + src_out.y * offset);
    let p2 = PointF::new(dst.x + dst_out.x * offset, dst.y + dst_out.y * offset);

    let src_h = src_side.is_horizontal();
    let dst_h = dst_side.is_horizontal();

    let mut pts = vec![src, p1];

    if src_h == dst_h {
        // Both horizontal or both vertical: S-curve with a mid bend.
        if src_h {
            // Horizontal: bend at mid-x.
            let mid_x = (p1.x + p2.x) * 0.5;
            pts.push(PointF::new(mid_x, p1.y));
            pts.push(PointF::new(mid_x, p2.y));
        } else {
            // Vertical: bend at mid-y.
            let mid_y = (p1.y + p2.y) * 0.5;
            pts.push(PointF::new(p1.x, mid_y));
            pts.push(PointF::new(p2.x, mid_y));
        }
    } else {
        // Mixed (one horizontal, one vertical): L-shape through one corner.
        if src_h {
            // Source exits horizontally, target enters vertically.
            // Go from p1 horizontally to p2.x, then vertically to p2.
            pts.push(PointF::new(p2.x, p1.y));
        } else {
            // Source exits vertically, target enters horizontally.
            pts.push(PointF::new(p1.x, p2.y));
        }
    }

    pts.push(p2);
    pts.push(dst);
    pts
}

/// Step path: orthogonal with sharp 90° corners.
pub fn step_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
) -> Vec<PointF> {
    orthogonal_points(src, dst, src_side, dst_side, 20.0)
}

/// SmoothStep path: orthogonal with rounded corners.
///
/// Computes the same corner points as `step_path`, then replaces each interior
/// corner with sampled arc points of radius `border_radius`.
pub fn smoothstep_path(
    src: PointF,
    dst: PointF,
    src_side: PortSide,
    dst_side: PortSide,
    border_radius: f32,
) -> Vec<PointF> {
    let raw = orthogonal_points(src, dst, src_side, dst_side, 20.0);
    round_corners(&raw, border_radius)
}

/// Replace sharp corners in a polyline with sampled arc points.
fn round_corners(points: &[PointF], radius: f32) -> Vec<PointF> {
    if points.len() < 3 || radius <= 0.0 {
        return points.to_vec();
    }
    let mut result = vec![points[0]];
    for i in 1..points.len() - 1 {
        let prev = points[i - 1];
        let curr = points[i];
        let next = points[i + 1];

        let d_prev = curr.distance_to(prev);
        let d_next = curr.distance_to(next);
        // Clamp radius to half the shorter adjacent segment.
        let r = radius.min(d_prev * 0.5).min(d_next * 0.5);
        if r < 1.0 {
            result.push(curr);
            continue;
        }

        // Tangent points on each segment at distance r from the corner.
        let t_prev = r / d_prev;
        let t_next = r / d_next;
        let p_in = curr.lerp(prev, t_prev);
        let p_out = curr.lerp(next, t_next);

        // Arc centre = corner + r * (unit_to_prev + unit_to_next).
        // For 90° corners this places the centre at distance r from both tangents.
        let unit_prev = PointF::new(
            (prev.x - curr.x) / d_prev,
            (prev.y - curr.y) / d_prev,
        );
        let unit_next = PointF::new(
            (next.x - curr.x) / d_next,
            (next.y - curr.y) / d_next,
        );
        let center = PointF::new(
            curr.x + r * (unit_prev.x + unit_next.x),
            curr.y + r * (unit_prev.y + unit_next.y),
        );

        let start_ang = (p_in.y - center.y).atan2(p_in.x - center.x);
        let end_ang = (p_out.y - center.y).atan2(p_out.x - center.x);
        let mut delta = end_ang - start_ang;
        if delta > std::f32::consts::PI {
            delta -= 2.0 * std::f32::consts::PI;
        }
        if delta < -std::f32::consts::PI {
            delta += 2.0 * std::f32::consts::PI;
        }

        result.push(p_in);
        let n = 6;
        for j in 1..n {
            let t = j as f32 / n as f32;
            let ang = start_ang + delta * t;
            result.push(PointF::new(
                center.x + r * ang.cos(),
                center.y + r * ang.sin(),
            ));
        }
        result.push(p_out);
    }
    result.push(*points.last().unwrap());
    result
}

// ---------------------------------------------------------------------------
// Loop-back (U-shape)
// ---------------------------------------------------------------------------

/// Loop-back path: U-shape routing around `node_bounds` for loop nodes.
///
/// Horizontal: exit right → down → left around the node → up → enter left.
/// Vertical: exit bottom → left → up around the node → right → enter top.
pub fn loop_back_path(
    src: PointF,
    dst: PointF,
    horizontal: bool,
    node_bounds: crate::geometry::RectF,
) -> Vec<PointF> {
    let margin = 40.0;
    if horizontal {
        // Route below the node: right → down → left → up to dst (left side).
        let bottom_y = node_bounds.bottom() + margin;
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
    } else {
        // Route left of the node: down → left → up → right to dst (top side).
        let left_x = node_bounds.left() - margin;
        let bottom_y = src.y + margin;
        let top_y = dst.y - margin;
        vec![
            src,
            PointF::new(src.x, bottom_y),
            PointF::new(left_x, bottom_y),
            PointF::new(left_x, top_y),
            PointF::new(dst.x, top_y),
            dst,
        ]
    }
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
        let pts = loop_back_path(
            PointF::new(300.0, 140.0),
            PointF::new(100.0, 140.0),
            true,
            bounds,
        );
        // The bottom routing segment must be below the node.
        let bottom = bounds.bottom();
        let has_below = pts
            .iter()
            .any(|p| p.y >= bottom);
        assert!(has_below, "U-shape must route below the node");
        // Endpoints preserved.
        assert_eq!(*pts.first().unwrap(), PointF::new(300.0, 140.0));
        assert_eq!(*pts.last().unwrap(), PointF::new(100.0, 140.0));
    }
}
