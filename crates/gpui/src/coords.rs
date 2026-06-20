//! Coordinate contracts between rust-agent-flow (viewport-local) and GPUI canvas (window).
//!
//! # Invariants (do not break)
//!
//! - **Nodes / handles**: `ResolvedNode.screen_pos` and `port_anchors` are viewport-local
//!   (origin = top-left of the editor canvas area, below the toolbar).
//! - **Canvas paint**: GPUI `paint_path` / `paint_quad` use window coordinates. Always add
//!   `bounds.origin` when painting flow geometry; see [`viewport_to_paint`].
//! - **Mouse**: subtract `bounds.origin` from window mouse position before hit-testing.
//! - **Edges**: `SceneFrame` resolves paths from `port_anchors`; path start/end must equal
//!   handle centers (`rust_agent_flow::check_frame`).

/// Map a viewport-local point to GPUI window paint coordinates.
#[inline]
pub fn viewport_to_paint(x: f32, y: f32, origin_x: f32, origin_y: f32) -> (f32, f32) {
    (x + origin_x, y + origin_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_to_paint_adds_canvas_origin() {
        assert_eq!(viewport_to_paint(260.0, 190.0, 100.0, 36.0), (360.0, 226.0));
    }

    #[test]
    fn roundtrip_subtract_origin() {
        let (wx, wy) = viewport_to_paint(60.0, 160.0, 80.0, 36.0);
        assert!((wx - 80.0 - 60.0).abs() < f32::EPSILON);
        assert!((wy - 36.0 - 160.0).abs() < f32::EPSILON);
    }
}
