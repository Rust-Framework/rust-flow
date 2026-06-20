//! Viewport zoom helpers — all on-screen dimensions scale with the camera zoom.

use gpui::*;

/// Zoom multiplier for layout px values (matches `ResolvedNode::zoom`).
#[derive(Debug, Clone, Copy)]
pub struct Z(f32);

impl Z {
    pub fn new(zoom: f32) -> Self {
        Self(zoom)
    }

    pub fn raw(self) -> f32 {
        self.0
    }

    pub fn px(self, value: f32) -> Pixels {
        px(value * self.0)
    }

    pub fn size(self, width: f32, height: f32) -> Size<Pixels> {
        size(self.px(width), self.px(height))
    }

    pub fn text_xs(self) -> Pixels {
        self.px(12.0)
    }

    pub fn text_sm(self) -> Pixels {
        self.px(14.0)
    }

    /// Apply scaled base font size so child text inherits zoom (unless overridden).
    pub fn cascade_text(self, el: Div) -> Div {
        el.text_size(self.text_sm())
    }
}
