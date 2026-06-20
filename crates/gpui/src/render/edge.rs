use rust_agent_flow::{
    BezierPath, EdgePath, Point as CorePoint, PortSide, ResolvedEdge, SmoothStepSegment,
};
use gpui::*;

use crate::coords::viewport_to_paint;

const BASE_EDGE_WIDTH: f32 = 1.1;
const BASE_ARROW_LEN: f32 = 7.0;
const BASE_ARROW_WING: f32 = 3.5;
const BASE_HANDLE_R: f32 = 4.0;

pub fn rgba_to_hsla(rgba: Rgba) -> Hsla {
    let (r, g, b, a) = (rgba.r, rgba.g, rgba.b, rgba.a);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let s = if max == min {
        0.0
    } else if l <= 0.5 {
        (max - min) / (max + min)
    } else {
        (max - min) / (2.0 - max - min)
    };
    let h = if max == min {
        0.0
    } else if max == r {
        60.0 * (((g - b) / (max - min)) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / (max - min) + 2.0)
    } else {
        60.0 * ((r - g) / (max - min) + 4.0)
    };
    Hsla {
        h: h as f32,
        s,
        l,
        a,
    }
}

/// Convert viewport-local flow coords to GPUI window paint coords.
fn paint_pt(x: f32, y: f32, origin: Point<Pixels>) -> Point<Pixels> {
    let ox: f32 = origin.x.into();
    let oy: f32 = origin.y.into();
    let (wx, wy) = viewport_to_paint(x, y, ox, oy);
    point(px(wx), px(wy))
}

fn side_inward(side: PortSide) -> (f32, f32) {
    match side {
        PortSide::Top => (0.0, 1.0),
        PortSide::Bottom => (0.0, -1.0),
        PortSide::Left => (1.0, 0.0),
        PortSide::Right => (-1.0, 0.0),
    }
}

pub fn paint_edge_with_decorations(
    edge: &ResolvedEdge,
    color: Rgba,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    zoom: f32,
) {
    let origin = bounds.origin;
    let end = path_end_point(&edge.path);
    paint_stroke_path(&edge.path, color, origin, window, zoom);
    paint_arrow_head(end, edge.to_side, color, origin, window, zoom);
}

fn path_end_point(path: &EdgePath) -> CorePoint {
    match path {
        EdgePath::Bezier(BezierPath { to, .. }) => *to,
        EdgePath::Catmull { segments, .. } => segments.last().map(|s| s.end).unwrap_or_default(),
        EdgePath::Polyline(pts) => pts.last().copied().unwrap_or_default(),
        EdgePath::SmoothStep { segments, .. } => last_smooth_step_point(segments),
    }
}

fn last_smooth_step_point(segments: &[SmoothStepSegment]) -> CorePoint {
    for seg in segments.iter().rev() {
        match seg {
            SmoothStepSegment::LineTo(pt) => return *pt,
            SmoothStepSegment::QuadTo { to, .. } => return *to,
        }
    }
    CorePoint::default()
}

fn paint_stroke_path(
    path: &EdgePath,
    color: Rgba,
    origin: Point<Pixels>,
    window: &mut Window,
    zoom: f32,
) {
    let hsla = rgba_to_hsla(color);
    let mut b = PathBuilder::stroke(px(BASE_EDGE_WIDTH * zoom));

    match path {
        EdgePath::Bezier(BezierPath { from, to, cp1, cp2 }) => {
            b.move_to(paint_pt(from.x, from.y, origin));
            b.cubic_bezier_to(
                paint_pt(to.x, to.y, origin),
                paint_pt(cp1.x, cp1.y, origin),
                paint_pt(cp2.x, cp2.y, origin),
            );
        }
        EdgePath::Catmull { start, segments } => {
            b.move_to(paint_pt(start.x, start.y, origin));
            for seg in segments {
                b.cubic_bezier_to(
                    paint_pt(seg.end.x, seg.end.y, origin),
                    paint_pt(seg.cp1.x, seg.cp1.y, origin),
                    paint_pt(seg.cp2.x, seg.cp2.y, origin),
                );
            }
        }
        EdgePath::Polyline(pts) => {
            if let Some(first) = pts.first() {
                b.move_to(paint_pt(first.x, first.y, origin));
                for pt in &pts[1..] {
                    b.line_to(paint_pt(pt.x, pt.y, origin));
                }
            }
        }
        EdgePath::SmoothStep { start, segments } => {
            b.move_to(paint_pt(start.x, start.y, origin));
            for seg in segments {
                match seg {
                    SmoothStepSegment::LineTo(pt) => {
                        b.line_to(paint_pt(pt.x, pt.y, origin));
                    }
                    SmoothStepSegment::QuadTo { ctrl, to } => {
                        b.curve_to(
                            paint_pt(to.x, to.y, origin),
                            paint_pt(ctrl.x, ctrl.y, origin),
                        );
                    }
                }
            }
        }
    }

    if let Ok(stroke) = b.build() {
        window.paint_path(stroke, hsla);
    }
}

fn paint_arrow_head(
    tip: CorePoint,
    to_side: PortSide,
    color: Rgba,
    origin: Point<Pixels>,
    window: &mut Window,
    zoom: f32,
) {
    let arrow_len = BASE_ARROW_LEN * zoom;
    let wing = BASE_ARROW_WING * zoom;
    let (ix, iy) = side_inward(to_side);
    let base_x = tip.x - ix * arrow_len;
    let base_y = tip.y - iy * arrow_len;
    let (px, py) = if ix.abs() > iy.abs() {
        (0.0, 1.0)
    } else {
        (1.0, 0.0)
    };

    let tip_pt = paint_pt(tip.x, tip.y, origin);
    let left_pt = paint_pt(base_x + px * wing, base_y + py * wing, origin);
    let right_pt = paint_pt(base_x - px * wing, base_y - py * wing, origin);

    let mut b = PathBuilder::fill();
    b.move_to(tip_pt);
    b.line_to(left_pt);
    b.line_to(right_pt);
    b.close();
    if let Ok(path) = b.build() {
        window.paint_path(path, rgba_to_hsla(color));
    }
}

pub fn paint_edge_path(
    path: &EdgePath,
    color: Rgba,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    zoom: f32,
) {
    paint_stroke_path(path, color, bounds.origin, window, zoom);
}

pub fn paint_handle_dot(
    center: CorePoint,
    fill: Rgba,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    zoom: f32,
) {
    let r = BASE_HANDLE_R * zoom;
    let o = bounds.origin;
    window.paint_quad(quad(
        Bounds {
            origin: paint_pt(center.x - r, center.y - r, o),
            size: size(px(r * 2.0), px(r * 2.0)),
        },
        px(r),
        rgba_to_hsla(fill),
        Edges::<Pixels>::default(),
        rgba_to_hsla(Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }),
        Default::default(),
    ));
}
