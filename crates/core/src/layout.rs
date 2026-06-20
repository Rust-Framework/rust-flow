//! Node layout constants (React Flow default-node proportions).

pub const ACCENT_H: f32 = 3.0;
pub const TITLE_H: f32 = 22.0;
pub const NODE_PAD: f32 = 4.0;
/// Handle radius — 8px diameter, half sits outside the node edge.
pub const HANDLE_R: f32 = 4.0;
pub const DOT_R: f32 = HANDLE_R;
pub const MIN_W: f32 = 150.0;
pub const MIN_SCREEN_W: f32 = 80.0;
pub const MIN_SCREEN_H: f32 = 36.0;
pub const VISUAL_HEIGHT: f32 = ACCENT_H + TITLE_H + NODE_PAD;

use crate::math::Point;

/// Node-local top-left of a handle dot; `center` is on the node border (React Flow Handle).
#[inline]
pub fn handle_dot_origin(center: Point, radius: f32) -> Point {
    Point::new(center.x - radius, center.y - radius)
}

#[inline]
pub fn scaled(value: f32, zoom: f32) -> f32 {
    value * zoom
}