//! Viewport math: pan offset + zoom scale, with screen↔logical transforms.

use crate::geometry::PointF;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// Screen-space offset of the logical origin (pan).
    pub offset: PointF,
    /// Zoom factor (1.0 = 100%).
    pub scale: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            offset: PointF::zero(),
            scale: 1.0,
        }
    }
}

impl Viewport {
    pub const MIN_SCALE: f32 = 0.2;
    pub const MAX_SCALE: f32 = 3.0;

    pub fn new(offset: PointF, scale: f32) -> Self {
        Self { offset, scale }
    }

    /// Logical → screen: `screen = logical * scale + offset`.
    pub fn to_screen(self, logical: PointF) -> PointF {
        PointF::new(
            logical.x * self.scale + self.offset.x,
            logical.y * self.scale + self.offset.y,
        )
    }

    /// Screen → logical: `logical = (screen - offset) / scale`.
    pub fn to_logical(self, screen: PointF) -> PointF {
        PointF::new(
            (screen.x - self.offset.x) / self.scale,
            (screen.y - self.offset.y) / self.scale,
        )
    }

    /// Clamp scale into `[MIN_SCALE, MAX_SCALE]`.
    pub fn clamp_scale(scale: f32) -> f32 {
        scale.clamp(Self::MIN_SCALE, Self::MAX_SCALE)
    }

    /// Zoom around an anchor (screen-space point), keeping the anchor fixed.
    pub fn zoom_around(self, anchor_screen: PointF, new_scale: f32) -> Self {
        let new_scale = Self::clamp_scale(new_scale);
        // anchor_screen = anchor_logical * new_scale + new_offset
        // anchor_logical = (anchor_screen - old_offset) / old_scale
        // => new_offset = anchor_screen - anchor_logical * new_scale
        let anchor_logical = self.to_logical(anchor_screen);
        let new_offset = PointF::new(
            anchor_screen.x - anchor_logical.x * new_scale,
            anchor_screen.y - anchor_logical.y * new_scale,
        );
        Self {
            offset: new_offset,
            scale: new_scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_logical_roundtrip() {
        let vp = Viewport::new(PointF::new(50.0, 30.0), 2.0);
        let logical = PointF::new(10.0, 5.0);
        let screen = vp.to_screen(logical);
        assert_eq!(screen, PointF::new(70.0, 40.0));
        assert_eq!(vp.to_logical(screen), logical);
    }

    #[test]
    fn zoom_around_keeps_anchor_fixed() {
        let vp = Viewport::new(PointF::new(0.0, 0.0), 1.0);
        let anchor = PointF::new(100.0, 100.0);
        let zoomed = vp.zoom_around(anchor, 2.0);
        // The anchor point in logical space should map to the same screen point.
        let anchor_logical = vp.to_logical(anchor);
        assert_eq!(zoomed.to_screen(anchor_logical), anchor);
    }
}
