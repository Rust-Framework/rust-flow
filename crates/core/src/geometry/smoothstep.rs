use crate::math::Point;

const CORNER_RADIUS: f32 = 10.0;

/// Build a smoothstep polyline: horizontal → vertical → horizontal with rounded corners.
pub fn smoothstep_points(from: Point, to: Point) -> Vec<Point> {
    let mid_x = (from.x + to.x) / 2.0;
    let r = CORNER_RADIUS;
    let cy_sign = if to.y > from.y { 1.0 } else { -1.0 };

    let mut pts = vec![from];

    let h1 = mid_x - r;
    if h1 > from.x + 2.0 {
        pts.push(Point::new(h1, from.y));
    }

    pts.push(Point::new(mid_x, from.y + cy_sign * r));

    let v_bot = to.y - cy_sign * r;
    if (v_bot - (from.y + cy_sign * r)).abs() > 2.0 {
        pts.push(Point::new(mid_x, v_bot));
    }

    pts.push(Point::new(mid_x + r, to.y));

    if mid_x + r < to.x - 2.0 {
        pts.push(to);
    } else if pts.last().copied() != Some(to) {
        pts.push(to);
    }

    pts
}
