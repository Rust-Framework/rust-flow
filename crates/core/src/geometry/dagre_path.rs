//! Dagre orthogonal edge paths — Mermaid `dagre-wrapper` + `curve: linear`.

use crate::math::Point;
use crate::port::PortSide;
use crate::viewport::Viewport;

use super::EdgePath;

const EPS: f32 = 0.5;

/// Build a strict H/V polyline from Dagre `edge.points` (world space).
pub fn edge_path_from_dagre(world_points: &[Point], viewport: &Viewport) -> EdgePath {
    let world = normalize_dagre_polyline(world_points);
    if world.len() < 2 {
        let screen = world
            .iter()
            .map(|p| viewport.world_to_screen(*p))
            .collect();
        return EdgePath::Polyline(screen);
    }

    let points: Vec<Point> = world
        .iter()
        .map(|p| viewport.world_to_screen(*p))
        .collect();

    EdgePath::Polyline(points)
}

pub fn label_pos_from_dagre(world: Point, viewport: &Viewport) -> Point {
    viewport.world_to_screen(world)
}

/// Port side where the path leaves the source node (first segment).
pub fn departure_side(path: &EdgePath) -> Option<PortSide> {
    polyline_segment_side(path, true)
}

/// Port side where the path meets the target node (last segment).
pub fn arrival_side(path: &EdgePath) -> Option<PortSide> {
    polyline_segment_side(path, false)
}

fn polyline_segment_side(path: &EdgePath, at_start: bool) -> Option<PortSide> {
    let pts = match path {
        EdgePath::Polyline(pts) => pts,
        _ => return None,
    };
    if pts.len() < 2 {
        return None;
    }
    let (a, b) = if at_start {
        (pts[0], pts[1])
    } else {
        (pts[pts.len() - 2], pts[pts.len() - 1])
    };
    Some(segment_direction_side(a, b, at_start))
}

/// `at_start`: side on the node where the segment originates; else where it terminates.
fn segment_direction_side(from: Point, to: Point, at_start: bool) -> PortSide {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dy.abs() > dx.abs() {
        if at_start {
            if dy > 0.0 {
                PortSide::Bottom
            } else {
                PortSide::Top
            }
        } else if dy > 0.0 {
            PortSide::Top
        } else {
            PortSide::Bottom
        }
    } else if at_start {
        if dx > 0.0 {
            PortSide::Right
        } else {
            PortSide::Left
        }
    } else if dx > 0.0 {
        PortSide::Left
    } else {
        PortSide::Right
    }
}

/// Clean Dagre route: dedupe → snap to axis → drop collinear joints.
pub fn normalize_dagre_polyline(points: &[Point]) -> Vec<Point> {
    if points.is_empty() {
        return Vec::new();
    }

    let mut pts: Vec<Point> = points.to_vec();
    dedupe_adjacent(&mut pts, EPS);
    if pts.len() < 2 {
        return pts;
    }

    pts = snap_segments_to_axis(pts);
    dedupe_adjacent(&mut pts, EPS);
    remove_collinear(&mut pts);
    dedupe_adjacent(&mut pts, EPS);
    pts
}

fn snap_segments_to_axis(points: Vec<Point>) -> Vec<Point> {
    let mut out = vec![points[0]];
    for &p in points.iter().skip(1) {
        let prev = out.last().copied().unwrap();
        let dx = p.x - prev.x;
        let dy = p.y - prev.y;
        let next = if dx.abs() < EPS {
            Point::new(prev.x, p.y)
        } else if dy.abs() < EPS {
            Point::new(p.x, prev.y)
        } else if dx.abs() >= dy.abs() {
            Point::new(p.x, prev.y)
        } else {
            Point::new(prev.x, p.y)
        };
        if (next.x - prev.x).abs() > EPS || (next.y - prev.y).abs() > EPS {
            out.push(next);
        }
    }
    out
}

fn remove_collinear(points: &mut Vec<Point>) {
    if points.len() < 3 {
        return;
    }
    let mut i = 1;
    while i + 1 < points.len() {
        let a = points[i - 1];
        let b = points[i];
        let c = points[i + 1];
        let same_x = (a.x - b.x).abs() < EPS && (b.x - c.x).abs() < EPS;
        let same_y = (a.y - b.y).abs() < EPS && (b.y - c.y).abs() < EPS;
        if same_x || same_y {
            points.remove(i);
        } else {
            i += 1;
        }
    }
}

fn dedupe_adjacent(pts: &mut Vec<Point>, eps: f32) {
    let mut i = 1;
    while i < pts.len() {
        let a = pts[i - 1];
        let b = pts[i];
        if (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps {
            pts.remove(i);
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::Viewport;

    fn is_strict_orthogonal(pts: &[Point]) -> bool {
        for i in 1..pts.len() {
            let dx = pts[i].x - pts[i - 1].x;
            let dy = pts[i].y - pts[i - 1].y;
            if dx.abs() > EPS && dy.abs() > EPS {
                return false;
            }
        }
        true
    }

    #[test]
    fn normalize_bus_route_is_orthogonal() {
        let raw = vec![
            Point::new(100.0, 40.0),
            Point::new(100.0, 80.0),
            Point::new(100.01, 80.0),
            Point::new(200.0, 80.02),
            Point::new(200.0, 120.0),
        ];
        let norm = normalize_dagre_polyline(&raw);
        assert!(is_strict_orthogonal(&norm));
        assert_eq!(norm.len(), 4);
    }

    #[test]
    fn dagre_path_screen_space_orthogonal() {
        let vp = Viewport::default();
        let world = vec![
            Point::new(100.0, 40.0),
            Point::new(100.0, 80.0),
            Point::new(200.0, 80.0),
            Point::new(200.0, 120.0),
        ];
        let path = edge_path_from_dagre(&world, &vp);
        let pts = match &path {
            EdgePath::Polyline(p) => p,
            _ => panic!("expected polyline"),
        };
        assert!(is_strict_orthogonal(&pts));
        let (s, e) = crate::geometry::edge_path_endpoints(&path);
        assert!((s.x - 100.0).abs() < 0.1);
        assert!((e.x - 200.0).abs() < 0.1);
    }

    #[test]
    fn arrival_side_tb_down_into_top() {
        let path = EdgePath::Polyline(vec![
            Point::new(100.0, 50.0),
            Point::new(100.0, 80.0),
        ]);
        assert_eq!(arrival_side(&path), Some(PortSide::Top));
    }
}
