mod bezier;
mod catmull;
mod dagre_path;
mod hit;
mod smoothstep;
mod xyflow;

pub use bezier::{
    bezier_control_points, mindmap_lr_bezier, mindmap_main_bezier, mindmap_sub_bezier,
    mindmap_tb_bezier, BezierPath,
};
pub use catmull::{catmull_rom_segments, CatmullSegment};
pub use dagre_path::{
    arrival_side, departure_side, edge_path_from_dagre, label_pos_from_dagre,
    normalize_dagre_polyline,
};
pub use hit::{hit_node_at, hit_port_at, PORT_HIT_RADIUS};
pub use smoothstep::smoothstep_points;
pub use xyflow::{
    get_bezier_path, get_bezier_edge_center, get_edge_center, get_smooth_step_path,
    get_smooth_step_path_with_step, get_smooth_step_label_center, get_step_points,
    handle_position, SmoothStepSegment, DEFAULT_BORDER_RADIUS, DEFAULT_CURVATURE, DEFAULT_OFFSET,
};

use crate::edge::{EdgeShape, EdgeStroke};
use crate::math::Point;
use crate::port::PortSide;

#[derive(Debug, Clone, PartialEq)]
pub enum EdgePath {
    Bezier(BezierPath),
    Catmull {
        start: Point,
        segments: Vec<CatmullSegment>,
    },
    Polyline(Vec<Point>),
    SmoothStep {
        start: Point,
        segments: Vec<SmoothStepSegment>,
    },
}

pub fn build_edge_path(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    shape: EdgeShape,
) -> EdgePath {
    build_edge_path_with_route(
        from,
        from_side,
        to,
        to_side,
        shape,
        crate::auto_layout::EdgeRouteOffset::default(),
        1.0,
    )
}

pub fn build_edge_path_with_route(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    shape: EdgeShape,
    route: crate::auto_layout::EdgeRouteOffset,
    zoom: f32,
) -> EdgePath {
    let offset = effective_edge_offset(from, from_side, to, to_side, zoom);
    let border_radius = DEFAULT_BORDER_RADIUS * zoom;
    let (center_x, center_y) = route_bend_center(from, from_side, to, to_side, route);
    match shape {
        EdgeShape::SmoothStep => EdgePath::SmoothStep {
            start: from,
            segments: get_smooth_step_path(
                from.x,
                from.y,
                from_side,
                to.x,
                to.y,
                to_side,
                border_radius,
                offset,
                center_x,
                center_y,
            ),
        },
        EdgeShape::Bezier => {
            let (a, b, cp1, cp2) = get_bezier_path(
                from.x,
                from.y,
                from_side,
                to.x,
                to.y,
                to_side,
                DEFAULT_CURVATURE,
            );
            EdgePath::Bezier(BezierPath {
                from: a,
                to: b,
                cp1,
                cp2,
            })
        }
        EdgeShape::Natural => {
            let mid = Point::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
            EdgePath::Catmull {
                start: from,
                segments: catmull_rom_segments(&[from, mid, to]),
            }
        }
        EdgeShape::Straight => EdgePath::Polyline(vec![from, to]),
    }
}

/// Orthogonal routing polyline for obstacle checks (screen space).
pub fn edge_step_polyline(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    route: crate::auto_layout::EdgeRouteOffset,
    zoom: f32,
) -> Vec<Point> {
    let offset = effective_edge_offset(from, from_side, to, to_side, zoom);
    let (center_x, center_y) = route_bend_center(from, from_side, to, to_side, route);
    get_step_points(
        from.x,
        from.y,
        from_side,
        to.x,
        to.y,
        to_side,
        offset,
        0.5,
        center_x,
        center_y,
    )
}

fn effective_edge_offset(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    zoom: f32,
) -> f32 {
    let base = DEFAULT_OFFSET * zoom;
    let extra = match (from_side, to_side) {
        (PortSide::Bottom, PortSide::Top) if to.y < from.y - 2.0 => (from.y - to.y) * 0.4,
        (PortSide::Top, PortSide::Bottom) if to.y > from.y + 2.0 => (to.y - from.y) * 0.4,
        (PortSide::Right, PortSide::Left) if to.x < from.x - 2.0 => (from.x - to.x) * 0.4,
        (PortSide::Left, PortSide::Right) if to.x > from.x + 2.0 => (to.x - from.x) * 0.4,
        _ => 0.0,
    };
    base + extra
}

fn route_bend_center(
    from: Point,
    from_side: PortSide,
    to: Point,
    to_side: PortSide,
    route: crate::auto_layout::EdgeRouteOffset,
) -> (Option<f32>, Option<f32>) {
    let spread = route.from_shift + route.to_shift;
    if spread.abs() < 0.01
        && route.center_nudge_x.abs() < 0.01
        && route.center_nudge_y.abs() < 0.01
    {
        return (None, None);
    }

    match (from_side, to_side) {
        (PortSide::Bottom, PortSide::Top)
        | (PortSide::Top, PortSide::Bottom)
        | (PortSide::Bottom, PortSide::Bottom)
        | (PortSide::Top, PortSide::Top) => {
            let cx = (from.x + to.x) * 0.5 + spread + route.center_nudge_x;
            let cy = if route.center_nudge_y.abs() > 0.01 {
                Some((from.y + to.y) * 0.5 + route.center_nudge_y)
            } else {
                None
            };
            (Some(cx), cy)
        }
        (PortSide::Right, PortSide::Left)
        | (PortSide::Left, PortSide::Right)
        | (PortSide::Right, PortSide::Right)
        | (PortSide::Left, PortSide::Left) => {
            let cy = (from.y + to.y) * 0.5 + spread + route.center_nudge_y;
            let cx = if route.center_nudge_x.abs() > 0.01 {
                Some((from.x + to.x) * 0.5 + route.center_nudge_x)
            } else {
                None
            };
            (cx, Some(cy))
        }
        _ => (
            Some((from.x + to.x) * 0.5 + spread * 0.5 + route.center_nudge_x),
            Some((from.y + to.y) * 0.5 + spread * 0.5 + route.center_nudge_y),
        ),
    }
}

pub fn edge_stroke_dash(stroke: EdgeStroke) -> Option<Vec<f32>> {
    match stroke {
        EdgeStroke::Solid => None,
        EdgeStroke::Dashed => Some(vec![8.0, 4.0]),
        EdgeStroke::Dotted => Some(vec![2.0, 4.0]),
    }
}

/// First and last point of a built edge path (must match handle centers).
pub fn edge_path_endpoints(path: &EdgePath) -> (Point, Point) {
    match path {
        EdgePath::Bezier(b) => (b.from, b.to),
        EdgePath::Polyline(pts) => {
            let first = pts.first().copied().unwrap_or_default();
            let last = pts.last().copied().unwrap_or(first);
            (first, last)
        }
        EdgePath::Catmull { start, segments } => {
            let end = segments.last().map(|s| s.end).unwrap_or(*start);
            (*start, end)
        }
        EdgePath::SmoothStep { start, segments } => {
            let mut end = *start;
            for seg in segments {
                end = match seg {
                    SmoothStepSegment::LineTo(p) => *p,
                    SmoothStepSegment::QuadTo { to, .. } => *to,
                };
            }
            (*start, end)
        }
    }
}
