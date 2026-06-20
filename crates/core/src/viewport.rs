use crate::math::{Point, Size};

/// Camera transform for panning and zooming the canvas.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub pan: Point,
    pub zoom: f32,
    pub screen_size: Size,
    pub snap_enabled: bool,
    pub snap_grid_size: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            pan: Point::new(0.0, 0.0),
            zoom: 1.0,
            screen_size: Size::new(1280.0, 720.0),
            snap_enabled: true,
            snap_grid_size: 20.0,
        }
    }
}

impl Viewport {
    pub fn new(zoom: f32) -> Self {
        Self {
            zoom,
            ..Default::default()
        }
    }

    pub fn zoom_at(&mut self, zoom_delta: f32, screen_center: Point) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * zoom_delta).clamp(0.05, 3.0);
        if old_zoom == self.zoom {
            return;
        }

        let world = Point::new(
            (screen_center.x - self.pan.x) / old_zoom,
            (screen_center.y - self.pan.y) / old_zoom,
        );
        self.pan.x = screen_center.x - world.x * self.zoom;
        self.pan.y = screen_center.y - world.y * self.zoom;
    }

    pub fn pan_by(&mut self, delta: Point) {
        self.pan.x += delta.x;
        self.pan.y += delta.y;
    }

    pub fn screen_to_world(&self, screen: Point) -> Point {
        Point::new(
            (screen.x - self.pan.x) / self.zoom,
            (screen.y - self.pan.y) / self.zoom,
        )
    }

    pub fn world_to_screen(&self, world: Point) -> Point {
        Point::new(
            world.x * self.zoom + self.pan.x,
            world.y * self.zoom + self.pan.y,
        )
    }

    pub fn reset(&mut self) {
        self.pan = Point::new(0.0, 0.0);
        self.zoom = 1.0;
    }

    /// Fit the viewport to a world-space content rectangle (React Flow `fitView`).
    pub fn fit_to_content(&mut self, origin: Point, size: Size, padding: f32) {
        if size.width <= 1.0 || size.height <= 1.0 {
            return;
        }
        let avail_w = self.screen_size.width - padding * 2.0;
        let avail_h = self.screen_size.height - padding * 2.0;
        if avail_w <= 0.0 || avail_h <= 0.0 {
            return;
        }
        let zoom_x = avail_w / size.width;
        let zoom_y = avail_h / size.height;
        self.zoom = zoom_x.min(zoom_y).clamp(0.05, 3.0);

        let cx = origin.x + size.width / 2.0;
        let cy = origin.y + size.height / 2.0;
        self.pan.x = self.screen_size.width / 2.0 - cx * self.zoom;
        self.pan.y = self.screen_size.height / 2.0 - cy * self.zoom;
    }

    pub fn toggle_snap(&mut self) {
        self.snap_enabled = !self.snap_enabled;
    }

    pub fn snap_point(&self, pos: Point) -> Point {
        if !self.snap_enabled {
            return pos;
        }
        let g = self.snap_grid_size;
        Point::new(
            (pos.x / g).round() * g,
            (pos.y / g).round() * g,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_fit_to_content_centers_graph() {
        let mut vp = Viewport::default();
        vp.fit_to_content(Point::new(0.0, 0.0), Size::new(3000.0, 2000.0), 0.0);
        assert!(vp.zoom < 1.0);
        let center = vp.world_to_screen(Point::new(1500.0, 1000.0));
        assert!((center.x - vp.screen_size.width / 2.0).abs() < 1.0);
        assert!((center.y - vp.screen_size.height / 2.0).abs() < 1.0);
    }

    #[test]
    fn viewport_roundtrip() {
        let vp = Viewport::new(1.5);
        let world = Point::new(100.0, 200.0);
        let screen = vp.world_to_screen(world);
        let back = vp.screen_to_world(screen);
        assert!((back.x - world.x).abs() < 0.01);
        assert!((back.y - world.y).abs() < 0.01);
    }
}
