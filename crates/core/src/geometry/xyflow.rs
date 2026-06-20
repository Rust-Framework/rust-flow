//! React Flow (@xyflow/system) handle positions and edge path utilities.
//! Ported from packages/system/src/utils/edges/smoothstep-edge.ts and bezier-edge.ts.

use crate::math::{Point, Size};
use crate::port::PortSide;

pub const DEFAULT_OFFSET: f32 = 18.0;
pub const DEFAULT_BORDER_RADIUS: f32 = 4.0;
pub const DEFAULT_CURVATURE: f32 = 0.25;

/// Handle center on the node bounding box edge (React Flow `getHandlePosition`).
pub fn handle_position(
    origin: Point,
    size: Size,
    side: PortSide,
    index: usize,
    total: usize,
) -> Point {
    let t = if total <= 1 {
        0.5
    } else {
        (index as f32 + 1.0) / (total as f32 + 1.0)
    };
    match side {
        PortSide::Left => Point::new(origin.x, origin.y + size.height * t),
        PortSide::Right => Point::new(origin.x + size.width, origin.y + size.height * t),
        PortSide::Top => Point::new(origin.x + size.width * t, origin.y),
        PortSide::Bottom => Point::new(origin.x + size.width * t, origin.y + size.height),
    }
}

fn side_vector(side: PortSide) -> Point {
    match side {
        PortSide::Left => Point::new(-1.0, 0.0),
        PortSide::Right => Point::new(1.0, 0.0),
        PortSide::Top => Point::new(0.0, -1.0),
        PortSide::Bottom => Point::new(0.0, 1.0),
    }
}

fn gap_point(x: f32, y: f32, side: PortSide, offset: f32) -> Point {
    let dir = side_vector(side);
    Point::new(x + dir.x * offset, y + dir.y * offset)
}

fn distance(a: Point, b: Point) -> f32 {
    ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt()
}

fn get_direction(source: Point, source_side: PortSide, target: Point) -> Point {
    match source_side {
        PortSide::Left | PortSide::Right => {
            if source.x < target.x {
                Point::new(1.0, 0.0)
            } else {
                Point::new(-1.0, 0.0)
            }
        }
        PortSide::Top | PortSide::Bottom => {
            if source.y < target.y {
                Point::new(0.0, 1.0)
            } else {
                Point::new(0.0, -1.0)
            }
        }
    }
}

fn dir_accessor(dir: Point) -> char {
    if dir.x != 0.0 {
        'x'
    } else {
        'y'
    }
}

fn dir_component(p: Point, accessor: char) -> f32 {
    if accessor == 'x' {
        p.x
    } else {
        p.y
    }
}

fn set_component(p: Point, accessor: char, value: f32) -> Point {
    if accessor == 'x' {
        Point::new(value, p.y)
    } else {
        Point::new(p.x, value)
    }
}

fn side_dir_component(side: PortSide, accessor: char) -> f32 {
    dir_component(side_vector(side), accessor)
}

/// React Flow `getPoints` — orthogonal step routing between gapped handles.
pub fn get_step_points(
    source_x: f32,
    source_y: f32,
    source_side: PortSide,
    target_x: f32,
    target_y: f32,
    target_side: PortSide,
    offset: f32,
    step_position: f32,
    center_x: Option<f32>,
    center_y: Option<f32>,
) -> Vec<Point> {
    let source = Point::new(source_x, source_y);
    let target = Point::new(target_x, target_y);
    let source_dir = side_vector(source_side);
    let _target_dir = side_vector(target_side);
    let source_gapped = gap_point(source_x, source_y, source_side, offset);
    let target_gapped = gap_point(target_x, target_y, target_side, offset);
    let dir = get_direction(source_gapped, source_side, target_gapped);
    let accessor = dir_accessor(dir);
    let curr_dir = dir_component(dir, accessor);

    let mut source_gap_offset = Point::default();
    let mut target_gap_offset = Point::default();

    let mut points: Vec<Point>;

    if side_dir_component(source_side, accessor) * side_dir_component(target_side, accessor) == -1.0 {
        let (center_x, center_y) = if accessor == 'x' {
            (
                center_x.unwrap_or(source_gapped.x + (target_gapped.x - source_gapped.x) * step_position),
                center_y.unwrap_or((source_gapped.y + target_gapped.y) * 0.5),
            )
        } else {
            (
                center_x.unwrap_or((source_gapped.x + target_gapped.x) * 0.5),
                center_y.unwrap_or(source_gapped.y + (target_gapped.y - source_gapped.y) * step_position),
            )
        };

        let vertical_split = vec![
            Point::new(center_x, source_gapped.y),
            Point::new(center_x, target_gapped.y),
        ];
        let horizontal_split = vec![
            Point::new(source_gapped.x, center_y),
            Point::new(target_gapped.x, center_y),
        ];

        points = if side_dir_component(source_side, accessor) == curr_dir {
            if accessor == 'x' {
                vertical_split
            } else {
                horizontal_split
            }
        } else if accessor == 'x' {
            horizontal_split
        } else {
            vertical_split
        };
    } else {
        let source_target = [Point::new(source_gapped.x, target_gapped.y)];
        let target_source = [Point::new(target_gapped.x, source_gapped.y)];

        points = if accessor == 'x' {
            if source_dir.x == curr_dir {
                target_source.to_vec()
            } else {
                source_target.to_vec()
            }
        } else if source_dir.y == curr_dir {
            source_target.to_vec()
        } else {
            target_source.to_vec()
        };

        if source_side == target_side {
            let diff = (dir_component(source, accessor) - dir_component(target, accessor)).abs();
            if diff <= offset {
                let gap_offset = (offset - 1.0).min(offset - diff);
                if side_dir_component(source_side, accessor) == curr_dir {
                    source_gap_offset = set_component(
                        source_gap_offset,
                        accessor,
                        if dir_component(source_gapped, accessor) > dir_component(source, accessor) {
                            -gap_offset
                        } else {
                            gap_offset
                        },
                    );
                } else {
                    target_gap_offset = set_component(
                        target_gap_offset,
                        accessor,
                        if dir_component(target_gapped, accessor) > dir_component(target, accessor) {
                            -gap_offset
                        } else {
                            gap_offset
                        },
                    );
                }
            }
        }

        if source_side != target_side {
            let opposite = if accessor == 'x' { 'y' } else { 'x' };
            let is_same_dir =
                side_dir_component(source_side, accessor) == side_dir_component(target_side, opposite);
            let source_gt = dir_component(source_gapped, opposite) > dir_component(target_gapped, opposite);
            let source_lt = dir_component(source_gapped, opposite) < dir_component(target_gapped, opposite);
            let flip = (side_dir_component(source_side, accessor) == 1.0
                && ((!is_same_dir && source_gt) || (is_same_dir && source_lt)))
                || (side_dir_component(source_side, accessor) != 1.0
                    && ((!is_same_dir && source_lt) || (is_same_dir && source_gt)));

            if flip {
                points = if accessor == 'x' {
                    source_target.to_vec()
                } else {
                    target_source.to_vec()
                };
            }
        }
    }

    let gapped_source = Point::new(
        source_gapped.x + source_gap_offset.x,
        source_gapped.y + source_gap_offset.y,
    );
    let gapped_target = Point::new(
        target_gapped.x + target_gap_offset.x,
        target_gapped.y + target_gap_offset.y,
    );

    let mut path_points = vec![source];
    if gapped_source.x != points[0].x || gapped_source.y != points[0].y {
        path_points.push(gapped_source);
    }
    path_points.extend(points.iter().copied());
    if let Some(last) = path_points.last().copied() {
        if gapped_target.x != last.x || gapped_target.y != last.y {
            path_points.push(gapped_target);
        }
    }
    path_points.push(target);
    path_points
}

/// One segment of a smooth-step path (line or quadratic bend).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmoothStepSegment {
    LineTo(Point),
    QuadTo { ctrl: Point, to: Point },
}

/// Full smooth-step path with rounded corners (React Flow `getSmoothStepPath`).
pub fn get_smooth_step_path(
    source_x: f32,
    source_y: f32,
    source_side: PortSide,
    target_x: f32,
    target_y: f32,
    target_side: PortSide,
    border_radius: f32,
    offset: f32,
    center_x: Option<f32>,
    center_y: Option<f32>,
) -> Vec<SmoothStepSegment> {
    let points = get_step_points(
        source_x,
        source_y,
        source_side,
        target_x,
        target_y,
        target_side,
        offset,
        0.5,
        center_x,
        center_y,
    );

    if points.len() < 2 {
        return Vec::new();
    }

    let mut segments = Vec::new();
    for i in 1..points.len().saturating_sub(1) {
        let a = points[i - 1];
        let b = points[i];
        let c = points[i + 1];
        let bend_size = distance(a, b)
            .min(distance(b, c) * 0.5)
            .min(border_radius);

        if (a.x - b.x).abs() < 0.01 && (b.x - c.x).abs() < 0.01 {
            segments.push(SmoothStepSegment::LineTo(b));
            continue;
        }
        if (a.y - b.y).abs() < 0.01 && (b.y - c.y).abs() < 0.01 {
            segments.push(SmoothStepSegment::LineTo(b));
            continue;
        }

        if (a.y - b.y).abs() < 0.01 {
            let x_dir = if a.x < c.x { -1.0 } else { 1.0 };
            let y_dir = if a.y < c.y { 1.0 } else { -1.0 };
            segments.push(SmoothStepSegment::LineTo(Point::new(
                b.x + bend_size * x_dir,
                b.y,
            )));
            segments.push(SmoothStepSegment::QuadTo {
                ctrl: b,
                to: Point::new(b.x, b.y + bend_size * y_dir),
            });
        } else {
            segments.push(SmoothStepSegment::LineTo(Point::new(
                b.x,
                b.y + bend_size * if a.y < c.y { -1.0 } else { 1.0 },
            )));
            segments.push(SmoothStepSegment::QuadTo {
                ctrl: b,
                to: Point::new(
                    b.x + bend_size * if a.x < c.x { 1.0 } else { -1.0 },
                    b.y,
                ),
            });
        }
    }

    if let Some(&last) = points.last() {
        segments.push(SmoothStepSegment::LineTo(last));
    }

    segments
}

fn calculate_control_offset(dist: f32, curvature: f32) -> f32 {
    if dist >= 0.0 {
        0.5 * dist
    } else {
        curvature * 25.0 * (-dist).sqrt()
    }
}

fn control_with_curvature(pos: PortSide, x1: f32, y1: f32, x2: f32, y2: f32, c: f32) -> Point {
    match pos {
        PortSide::Left => Point::new(x1 - calculate_control_offset(x1 - x2, c), y1),
        PortSide::Right => Point::new(x1 + calculate_control_offset(x2 - x1, c), y1),
        PortSide::Top => Point::new(x1, y1 - calculate_control_offset(y1 - y2, c)),
        PortSide::Bottom => Point::new(x1, y1 + calculate_control_offset(y2 - y1, c)),
    }
}

/// Position-aware cubic bezier (React Flow `getBezierPath`).
pub fn get_bezier_path(
    source_x: f32,
    source_y: f32,
    source_side: PortSide,
    target_x: f32,
    target_y: f32,
    target_side: PortSide,
    curvature: f32,
) -> (Point, Point, Point, Point) {
    let from = Point::new(source_x, source_y);
    let to = Point::new(target_x, target_y);
    let cp1 = control_with_curvature(source_side, source_x, source_y, target_x, target_y, curvature);
    let cp2 = control_with_curvature(target_side, target_x, target_y, source_x, source_y, curvature);
    (from, to, cp1, cp2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_on_right_edge_center() {
        let p = handle_position(Point::new(0.0, 0.0), Size::new(200.0, 60.0), PortSide::Right, 0, 1);
        assert!((p.x - 200.0).abs() < 0.01);
        assert!((p.y - 30.0).abs() < 0.01);
    }

    #[test]
    fn smooth_step_right_to_left_same_row() {
        let points = get_step_points(
            260.0, 190.0, PortSide::Right,
            320.0, 190.0, PortSide::Left,
            20.0, 0.5, None, None,
        );
        assert!(points.len() >= 4);
        assert!((points[0].x - 260.0).abs() < 0.01);
        assert!((points.last().unwrap().x - 320.0).abs() < 0.01);
    }

    #[test]
    fn smooth_step_path_produces_segments() {
        let segs = get_smooth_step_path(
            260.0, 190.0, PortSide::Right,
            580.0, 190.0, PortSide::Left,
            5.0, 20.0, None, None,
        );
        assert!(!segs.is_empty());
    }
}
